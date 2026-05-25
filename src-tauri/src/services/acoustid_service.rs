// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// acoustid_service.rs -- AcoustID fingerprinting and lookup service
// ===================================================================
//
// Generates Chromaprint audio fingerprints using the embedded
// rusty-chromaprint library (pure Rust) and looks up AcoustID
// identifiers via the acoustid.org API. This enables music
// identification compatible with MusicBrainz Picard and other tools
// that use the AcoustID ecosystem.
//
// ## meedya-core Integration (#353)
//
// This module is a candidate for delegation to meedya-core's
// `Fingerprinter` trait. Current implementation uses rusty-chromaprint
// directly via Symphonia for audio decoding. meedya-core would abstract
// this behind a trait enabling alternative fingerprinting backends
// (e.g., essentia for musical key detection alongside fingerprinting).
// Migration is low priority since the current implementation is stable.
//
// ## How it works
//
// 1. For each M4A file, decodes the audio to raw PCM using Symphonia
//    (pure Rust audio decoder), then generates a Chromaprint fingerprint
//    using rusty-chromaprint's Fingerprinter.
// 2. Compresses the fingerprint using FingerprintCompressor and encodes
//    it in URL-safe base64 format for the AcoustID API.
// 3. Sends the fingerprint + duration to the AcoustID lookup API
//    (`https://api.acoustid.org/v2/lookup`) to find a matching AcoustID.
// 4. Writes two freeform atoms to the M4A file:
//    - `----:com.apple.iTunes:Acoustid Id` — the AcoustID UUID
//    - `----:com.apple.iTunes:Acoustid Fingerprint` — encoded fingerprint
//
// ## No External Dependencies
//
// All fingerprinting is done in pure Rust via rusty-chromaprint and
// Symphonia. No external fpcalc binary is required, which also enables
// AcoustID support on ARM Linux (where no fpcalc binaries exist).
//
// ## Rate limiting
//
// The AcoustID API allows ~3 requests/second for free API keys.
// A 334ms delay is enforced between lookup requests to stay within limits.
//
// ## Opt-in
//
// AcoustID processing is opt-in (`acoustid_enabled` in settings) because
// it's CPU-intensive (decoding + fingerprinting each file) and requires
// network requests per track.
//
// @see https://acoustid.org/webservice
// @see https://docs.rs/rusty-chromaprint/
// @see https://docs.rs/symphonia/

use std::path::{Path, PathBuf};
use std::time::Duration;

use mp4ameta::{Data, FreeformIdent, Tag};
use tokio::time::sleep;

/// Apple iTunes freeform atom namespace (matches `MusicBrainz` Picard's convention).
const ITUNES_NAMESPACE: &str = "com.apple.iTunes";

// `AcoustID` API endpoint constant moved to the shared
// `meedya-fingerprint::acoustid` module (#353 Phase 1). MeedyaDL
// keeps the rate-limit helper below because the per-request sleep
// is driven by our scheduler shape, not by the shared crate's
// stateless lookup.

/// Delay between `AcoustID` API requests (~3 req/sec rate limit).
const API_RATE_LIMIT_DELAY: Duration = Duration::from_millis(334);

/// Compile-time embedded `AcoustID` application API key. Set via the
/// `ACOUSTID_API_KEY` environment variable at build time (e.g., from a
/// GitHub Actions secret). When not set (local dev builds), this is `None`
/// and users must provide their own key in Settings > Metadata.
const EMBEDDED_API_KEY: Option<&str> = option_env!("ACOUSTID_API_KEY");

// ============================================================
// Public API
// ============================================================

/// Resolve the effective `AcoustID` API key.
///
/// Priority:
/// 1. User-provided key from Settings > Metadata (if non-empty)
/// 2. Compile-time embedded application key (if built with `ACOUSTID_API_KEY`)
/// 3. `None` — `AcoustID` lookups are not possible
///
/// This allows release builds to ship with a pre-configured key while
/// still letting users override it with their own registered key.
pub fn resolve_api_key(user_key: &str) -> Option<String> {
    // User override takes priority
    if !user_key.is_empty() {
        return Some(user_key.to_string());
    }
    // Fall back to compile-time embedded key
    EMBEDDED_API_KEY.map(String::from)
}

/// Process all M4A files in the output directory for `AcoustID` fingerprinting.
///
/// For each file: generates a Chromaprint fingerprint using the embedded
/// rusty-chromaprint library, looks up the `AcoustID` via the web service,
/// and writes both the fingerprint and `AcoustID` UUID as metadata tags.
///
/// # Arguments
/// * `output_path` - Download output path (file or album directory)
/// * `api_key` - AcoustID application API key (registered at acoustid.org)
/// * `on_progress` - Called BEFORE each file is processed with
///   `(current_index_one_based, total_files)`. Used by the
///   enrichment task to update the per-item progress-bar caption
///   so users see "AcoustID: track 5 of 19" instead of a static
///   "AcoustID fingerprinting…" for the entire stage (#574).
///   Pass `|_, _| {}` if you don't need progress updates.
/// * `file_locks` - Optional shared per-file write-coordination map
///   (#779 Option 2). When supplied, each per-file
///   `Tag::write_to_path` call acquires the lock for that file so
///   it serialises with any other stage that's writing to the same
///   file (notably ReplayGain). Pass `None` for standalone use.
///
/// # Errors
///
/// Returns `Err(String)` if the output path is invalid or file processing fails.
///
/// # Returns
/// * `Ok(count)` - Number of files successfully fingerprinted and tagged
/// * `Err(message)` - Output path invalid or no M4A files found
pub async fn process_acoustid_for_directory(
    output_path: &str,
    api_key: &str,
    on_progress: impl Fn(usize, usize) + Send,
    file_locks: Option<&std::sync::Arc<crate::utils::file_locks::FileWriteLocks>>,
) -> Result<usize, String> {
    if api_key.is_empty() {
        return Err("AcoustID API key not configured".to_string());
    }
    // Collect all M4A files
    let m4a_files = collect_m4a_files(output_path);
    if m4a_files.is_empty() {
        return Ok(0);
    }

    let mut tagged_count = 0;

    let total_files = m4a_files.len();
    for (i, file_path) in m4a_files.iter().enumerate() {
        on_progress(i + 1, total_files);
        log::info!(
            "AcoustID: fingerprinting file {}/{} — {}",
            i + 1,
            total_files,
            file_path.file_name().unwrap_or_default().to_string_lossy()
        );
        // Rate limit: wait before each API call (skip first iteration)
        if i > 0 {
            sleep(API_RATE_LIMIT_DELAY).await;
        }

        match process_single_file(file_path, api_key, file_locks).await {
            Ok(true) => tagged_count += 1,
            Ok(false) => {
                log::debug!("No AcoustID match for {}", file_path.display());
            }
            Err(e) => {
                log::debug!("AcoustID failed for {}: {}", file_path.display(), e);
            }
        }
    }

    if tagged_count > 0 {
        log::info!(
            "Tagged {} of {} file(s) with AcoustID metadata",
            tagged_count,
            m4a_files.len()
        );
    }

    Ok(tagged_count)
}

// ============================================================
// Internal: Per-File Processing
// ============================================================

/// Generate fingerprint, look up `AcoustID`, and write tags for a single file.
///
/// Returns `Ok(true)` if tags were written, `Ok(false)` if no match found.
///
/// `file_locks` (when supplied) is used to serialise the write phase with
/// any concurrent stage touching the same M4A file (#779 Option 2). The
/// lock is held ONLY around the `Tag::read_from_path` → `set_data` →
/// `Tag::write_to_path` cycle, so the slow fingerprint + API-lookup work
/// runs without blocking other stages.
async fn process_single_file(
    file_path: &Path,
    api_key: &str,
    file_locks: Option<&std::sync::Arc<crate::utils::file_locks::FileWriteLocks>>,
) -> Result<bool, String> {
    // Step 1: Generate Chromaprint fingerprint (embedded, no external binary).
    // This is CPU-intensive (decodes the entire audio file), so we run it
    // on a blocking thread to avoid starving the async runtime.
    let fp_path = file_path.to_path_buf();
    let (fingerprint, duration) =
        tokio::task::spawn_blocking(move || generate_fingerprint(&fp_path))
            .await
            .map_err(|e| format!("Fingerprint task panicked: {e}"))??;

    // Step 2: Look up AcoustID
    let Some(acoustid) = lookup_acoustid(&fingerprint, duration, api_key).await? else {
        return Ok(false); // No match found
    };

    // Step 3: Acquire the per-file write lock (#779 Option 2). When
    // supplied, this serialises against any other enrichment stage
    // (notably ReplayGain) writing to the same file. Acquired BEFORE
    // the spawn_blocking so the blocking task itself is short — read,
    // mutate, write, release. The slow CPU-bound fingerprint work above
    // already completed on its own thread without holding the lock.
    let _write_guard = match file_locks {
        Some(locks) => Some(locks.lock(file_path).await),
        None => None,
    };

    // Step 4: Write tags on a blocking thread to avoid starving the async
    // runtime. Tag::read_from_path / Tag::write_to_path are sync I/O that
    // can block for seconds on slow FUSE mounts (CloudMounter, NFS, etc.).
    let tag_path = file_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut tag =
            Tag::read_from_path(&tag_path).map_err(|e| format!("Failed to read M4A: {e}"))?;

        // Acoustid Id — the UUID from acoustid.org
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "Acoustid Id"),
            Data::Utf8(acoustid),
        );

        // Acoustid Fingerprint — encoded Chromaprint fingerprint string
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "Acoustid Fingerprint"),
            Data::Utf8(fingerprint),
        );

        tag.write_to_path(&tag_path)
            .map_err(|e| format!("Failed to write M4A: {e}"))?;

        log::debug!("AcoustID tagged: {}", tag_path.display());
        Ok::<bool, String>(true)
    })
    .await
    .map_err(|e| format!("AcoustID tag task panicked: {e}"))?
}

// ============================================================
// Internal: Fingerprint Generation (via shared meedya-fingerprint crate)
// ============================================================

/// Generate a Chromaprint fingerprint for an audio file (#353 Phase 3).
///
/// Thin wrapper over [`meedya_fingerprint::generate_fingerprint`] from
/// the shared `meedya-fingerprint` crate (gated behind that crate's
/// `chromaprint` feature, opted in via this crate's `Cargo.toml`).
/// The shared implementation is byte-for-byte identical to the
/// previous local one — same `preset_test2` Chromaprint config,
/// same Symphonia decode pipeline, same URL-safe-base64 encoding.
///
/// The error type is mapped back to `String` via
/// [`map_shared_chromaprint_result`] so existing call-sites (and the
/// `log::debug!` lines in `process_single_file`) keep their
/// historical user-facing strings.
fn generate_fingerprint(file_path: &Path) -> Result<(String, u32), String> {
    map_shared_chromaprint_result(meedya_fingerprint::generate_fingerprint(file_path))
}

/// Adapter mapping the shared crate's [`FingerprintError`] back to
/// MeedyaDL's historical `String` error surface. Extracted as a pure
/// function so it's unit-testable without spawning a Symphonia
/// decode — mirrors `map_shared_acoustid_result` (Phase 1) and
/// `map_shared_replaygain_result` (Phase 2).
fn map_shared_chromaprint_result(
    result: Result<(String, u32), meedya_fingerprint::FingerprintError>,
) -> Result<(String, u32), String> {
    use meedya_fingerprint::FingerprintError;
    match result {
        Ok(pair) => Ok(pair),
        Err(FingerprintError::IoError(e)) => Err(format!("Failed to open audio file: {e}")),
        Err(FingerprintError::DecodeError(msg)) => {
            // The shared crate's DecodeError covers probe/decode/missing-track failures.
            // Preserve historical phrasing for whichever stage failed.
            Err(format!("Audio decode error: {msg}"))
        }
        Err(FingerprintError::FingerprintFailed(msg)) => {
            Err(format!("Fingerprint generation failed: {msg}"))
        }
        Err(other) => Err(format!("Fingerprint task failed: {other}")),
    }
}

// ============================================================
// Internal: AcoustID API Lookup
// ============================================================

/// Look up an `AcoustID` by fingerprint and duration.
///
/// Sends a POST request to the `AcoustID` web service with the Chromaprint
/// fingerprint. Returns the `AcoustID` UUID if a match is found with
/// sufficient confidence (score >= 0.5).
///
/// # Returns
/// * `Ok(Some(uuid))` - `AcoustID` found
/// * `Ok(None)` - No match with sufficient confidence
/// * `Err(message)` - API request or parsing failed
async fn lookup_acoustid(
    fingerprint: &str,
    duration: u32,
    api_key: &str,
) -> Result<Option<String>, String> {
    // #353 Phase 1: AcoustID API client now lives in the shared
    // `meedya-fingerprint` crate (MeedyaSuite-core workspace).
    // MeedyaDL's contract here is unchanged — score < 0.5 → None,
    // score >= 0.5 → Some(acoustid) — but the HTTP plumbing
    // (request shape, response parsing, error mapping) is delegated
    // to the shared `AcoustIdClient::lookup`.
    use meedya_fingerprint::acoustid::AcoustIdClient;
    let client = AcoustIdClient::new(api_key.to_string());
    let shared_result = client.lookup(fingerprint, duration).await;
    map_shared_acoustid_result(shared_result)
}

/// Maps a `meedya-fingerprint::AcoustIdClient::lookup` result to
/// MeedyaDL's local `Result<Option<String>, String>` contract.
///
/// Pulled out of `lookup_acoustid` so it's unit-testable without a
/// real network call. The shared client itself is exercised by
/// its own tests in MeedyaSuite-core; what we test here is the
/// **MeedyaDL adapter** — the score-floor + error-mapping rules
/// that are local to this codebase.
///
/// Rules:
///   * `Ok(result)` with `score < 0.5` → `Ok(None)` (low-confidence
///     match is treated as no match; preserves the pre-#353 contract).
///   * `Ok(result)` with `score >= 0.5` → `Ok(Some(result.acoustid))`.
///     The first MusicBrainz recording ID is logged at debug for the
///     AcoustID → MusicBrainz bridge (#807).
///   * `Err(NoMatch)` → `Ok(None)`.
///   * `Err(NetworkError(_))` → `Err("AcoustID API request failed: …")`
///     — preserves the canonical prefix that downstream error
///     classification keys on.
///   * `Err(AcoustIdApiError(_))` → `Err("AcoustID API error: …")`.
///   * Any other shared-crate error → `Err("AcoustID lookup failed: …")`.
fn map_shared_acoustid_result(
    result: Result<
        meedya_fingerprint::AcoustIdResult,
        meedya_fingerprint::FingerprintError,
    >,
) -> Result<Option<String>, String> {
    match result {
        Ok(r) => {
            if r.score < 0.5 {
                log::debug!(
                    "AcoustID: best match below confidence threshold (score {:.3} < 0.5); treating as no match",
                    r.score
                );
                return Ok(None);
            }
            if let Some(mb_id) = r.recording_ids.first() {
                log::debug!("AcoustID: MusicBrainz recording ID: {mb_id}");
            }
            Ok(Some(r.acoustid))
        }
        Err(meedya_fingerprint::FingerprintError::NoMatch) => Ok(None),
        Err(meedya_fingerprint::FingerprintError::NetworkError(msg)) => {
            Err(format!("AcoustID API request failed: {msg}"))
        }
        Err(meedya_fingerprint::FingerprintError::AcoustIdApiError(msg)) => {
            Err(format!("AcoustID API error: {msg}"))
        }
        Err(other) => Err(format!("AcoustID lookup failed: {other}")),
    }
}

// ============================================================
// Internal: File Collection
// ============================================================

/// Collect all M4A file paths from the output path.
fn collect_m4a_files(output_path: &str) -> Vec<PathBuf> {
    let path = Path::new(output_path);
    let mut files = Vec::new();

    if path.is_file() {
        if is_m4a(path) {
            files.push(path.to_path_buf());
        }
    } else if path.is_dir() {
        // Migrated to walk_dir_depth in v1.0.8 (#716/1). max_depth=3
        // matches GAMDL's natural Output/Artist/Album/file shape; the
        // helper docs call this out as the conventional value for
        // album-scoped walkers. Filesystem sidecars (._*, .DS_Store,
        // Thumbs.db) skipped to avoid Chromaprint extraction noise
        // on non-audio binaries (#577).
        files.extend(crate::utils::fs_walk::walk_dir_depth(path, 3, |p| {
            if crate::utils::fs_safe::is_filesystem_sidecar(p) {
                return None;
            }
            if p.is_file() && is_m4a(p) {
                Some(p.to_path_buf())
            } else {
                None
            }
        }));
    }

    files
}

/// Checks whether a file path has an `.m4a` extension (case-insensitive).
fn is_m4a(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("m4a"))
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::{map_shared_acoustid_result, map_shared_chromaprint_result};

    // The pre-#353 `compressed_fingerprint_produces_valid_base64`
    // test was removed as part of Phase 3 — its coverage is now
    // provided upstream by `rusty_chromaprint_api_surface_unchanged`
    // and the chromaprint module's own tests in MeedyaSuite-core.

    // ----------------------------------------------------------
    // AcoustID response parsing tests
    // ----------------------------------------------------------

    #[test]
    fn parse_acoustid_response_with_match() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
            "status": "ok",
            "results": [
                {
                    "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                    "score": 0.95,
                    "recordings": []
                }
            ]
        }"#,
        )
        .unwrap();

        let results = json.get("results").and_then(|v| v.as_array()).unwrap();
        let best = results
            .iter()
            .find(|r| r.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) >= 0.5);
        assert!(best.is_some());
        let id = best.unwrap().get("id").and_then(|v| v.as_str()).unwrap();
        assert_eq!(id, "a1b2c3d4-e5f6-7890-abcd-ef1234567890");
    }

    #[test]
    fn parse_acoustid_response_low_score_returns_none() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
            "status": "ok",
            "results": [
                {
                    "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                    "score": 0.2,
                    "recordings": []
                }
            ]
        }"#,
        )
        .unwrap();

        let results = json.get("results").and_then(|v| v.as_array()).unwrap();
        let best = results
            .iter()
            .find(|r| r.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) >= 0.5);
        assert!(best.is_none());
    }

    #[test]
    fn parse_acoustid_response_empty_results() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
            "status": "ok",
            "results": []
        }"#,
        )
        .unwrap();

        let results = json.get("results").and_then(|v| v.as_array()).unwrap();
        assert!(results.is_empty());
    }

    // ----------------------------------------------------------
    // API key resolution tests
    // ----------------------------------------------------------

    #[test]
    fn resolve_api_key_prefers_user_provided() {
        let result = super::resolve_api_key("user-key-123");
        assert_eq!(result, Some("user-key-123".to_string()));
    }

    #[test]
    fn resolve_api_key_empty_falls_back_to_embedded() {
        let result = super::resolve_api_key("");
        // Result depends on whether ACOUSTID_API_KEY was set at compile time.
        // In test builds it's typically not set, so this returns None.
        // The important thing is it doesn't panic or return an empty string.
        if super::EMBEDDED_API_KEY.is_some() {
            assert!(result.is_some());
        } else {
            assert!(result.is_none());
        }
    }

    // ----------------------------------------------------------
    // AcoustID response parsing tests (continued)
    // ----------------------------------------------------------

    #[test]
    fn parse_acoustid_error_response() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
            "status": "error",
            "error": {
                "code": 2,
                "message": "invalid fingerprint"
            }
        }"#,
        )
        .unwrap();

        let status = json.get("status").and_then(|v| v.as_str()).unwrap();
        assert_eq!(status, "error");

        let error_msg = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(error_msg, "invalid fingerprint");
    }

    // ============================================================
    // #353 Phase 1 integration adapter tests
    //
    // The shared-crate `AcoustIdClient::lookup` is tested in
    // MeedyaSuite-core. What we test HERE is the MeedyaDL adapter
    // (`map_shared_acoustid_result`) — the score-floor +
    // error-mapping rules local to this codebase.
    // ============================================================

    /// Helper: construct a minimal `AcoustIdResult` with the given
    /// score + acoustid. Other fields use safe defaults.
    fn fake_result(
        acoustid: &str,
        score: f64,
        recording_ids: Vec<String>,
    ) -> meedya_fingerprint::AcoustIdResult {
        meedya_fingerprint::AcoustIdResult {
            acoustid: acoustid.to_string(),
            score,
            recording_ids,
            fingerprint: String::new(),
            duration_secs: 0,
        }
    }

    #[test]
    fn shared_result_with_high_score_returns_acoustid() {
        let r = map_shared_acoustid_result(Ok(fake_result(
            "abc-123",
            0.95,
            vec!["mb-001".into()],
        )));
        assert_eq!(r.unwrap(), Some("abc-123".to_string()));
    }

    #[test]
    fn shared_result_at_threshold_returns_acoustid() {
        // Score == 0.5 must pass the floor (the local doc says `< 0.5
        // → None`, so 0.5 should be inclusive).
        let r = map_shared_acoustid_result(Ok(fake_result(
            "abc-123",
            0.5,
            vec![],
        )));
        assert_eq!(r.unwrap(), Some("abc-123".to_string()));
    }

    #[test]
    fn shared_result_below_threshold_returns_none() {
        // Score < 0.5 is treated as no match, preserving the pre-#353
        // contract. This is the rule that protects against
        // mis-tagging tracks with low-confidence fingerprint matches.
        let r = map_shared_acoustid_result(Ok(fake_result(
            "abc-123",
            0.49,
            vec![],
        )));
        assert_eq!(r.unwrap(), None);
    }

    #[test]
    fn shared_result_no_match_error_returns_none_ok() {
        // The shared crate signals "no result found" via the typed
        // `FingerprintError::NoMatch` variant, NOT via Ok(empty).
        // MeedyaDL's contract maps it to Ok(None).
        let r = map_shared_acoustid_result(Err(
            meedya_fingerprint::FingerprintError::NoMatch,
        ));
        assert_eq!(r.unwrap(), None);
    }

    #[test]
    fn shared_result_network_error_preserves_canonical_prefix() {
        // The downstream error classifier
        // (utils::process::classify_error) routes any error containing
        // "AcoustID API request failed" / "network" / etc into the
        // 'network' bucket. The canonical prefix must survive the
        // shared-crate refactor or the classifier loses the rule.
        let r = map_shared_acoustid_result(Err(
            meedya_fingerprint::FingerprintError::NetworkError(
                "connection refused".to_string(),
            ),
        ));
        let err = r.expect_err("network error must propagate as Err");
        assert!(
            err.contains("AcoustID API request failed"),
            "canonical prefix missing: got {err:?}"
        );
        assert!(
            err.contains("connection refused"),
            "underlying cause must be preserved: got {err:?}"
        );
    }

    #[test]
    fn shared_result_api_error_preserves_canonical_prefix() {
        // The AcoustID server's own JSON-level "error" responses
        // (e.g. invalid client key, bad fingerprint) come through as
        // `FingerprintError::AcoustIdApiError`. Same prefix-preservation
        // contract.
        let r = map_shared_acoustid_result(Err(
            meedya_fingerprint::FingerprintError::AcoustIdApiError(
                "invalid client key".to_string(),
            ),
        ));
        let err = r.expect_err("api error must propagate as Err");
        assert!(
            err.contains("AcoustID API error"),
            "canonical prefix missing: got {err:?}"
        );
        assert!(
            err.contains("invalid client key"),
            "underlying cause must be preserved: got {err:?}"
        );
    }

    #[test]
    fn shared_crate_public_api_surface_unchanged() {
        // Pin the public types we rely on from meedya-fingerprint.
        // If the shared crate renames or reshapes one of these the
        // compiler error here flags the change BEFORE production
        // call sites break.
        let _: meedya_fingerprint::AcoustIdResult = fake_result("x", 0.0, vec![]);
        let _: meedya_fingerprint::FingerprintError =
            meedya_fingerprint::FingerprintError::NoMatch;
        let _client = meedya_fingerprint::acoustid::AcoustIdClient::new(
            "dummy-key".to_string(),
        );
    }

    // ============================================================
    // #353 Phase 3 — Chromaprint adapter tests
    //
    // The shared-crate `generate_fingerprint` itself is tested in
    // MeedyaSuite-core (`meedya-fingerprint::chromaprint::tests`).
    // What we test HERE is the MeedyaDL adapter
    // (`map_shared_chromaprint_result`) — error-variant mapping
    // back to the historical `String` shape so existing call-sites
    // and log scrapers stay aligned.
    // ============================================================

    #[test]
    fn chromaprint_adapter_passes_through_ok_result() {
        // The happy path is a clean pass-through: tuple comes back
        // unchanged.
        let r = map_shared_chromaprint_result(Ok(("AQAAB...".to_string(), 218)));
        assert_eq!(r.unwrap(), ("AQAAB...".to_string(), 218));
    }

    #[test]
    fn chromaprint_adapter_maps_io_error_with_canonical_prefix() {
        // File-open failures (missing file, permission denied) come
        // through the shared crate as `FingerprintError::IoError`
        // via its `#[from] std::io::Error` impl. MeedyaDL's
        // historical surface used "Failed to open audio file: {e}"
        // — preserve that so any log scrapers still match.
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let r = map_shared_chromaprint_result(Err(
            meedya_fingerprint::FingerprintError::IoError(io_err),
        ));
        let err = r.expect_err("io error must propagate as Err");
        assert!(
            err.starts_with("Failed to open audio file:"),
            "canonical prefix missing: got {err:?}"
        );
    }

    #[test]
    fn chromaprint_adapter_maps_decode_error() {
        // Symphonia probe / decode failures (unrecognised format,
        // malformed file, missing track) all surface as
        // `FingerprintError::DecodeError`. Historical MeedyaDL
        // phrasing used "Audio decode error:" for these.
        let r = map_shared_chromaprint_result(Err(
            meedya_fingerprint::FingerprintError::DecodeError(
                "format probe: unsupported codec".to_string(),
            ),
        ));
        let err = r.expect_err("decode error must propagate as Err");
        assert!(
            err.contains("Audio decode error"),
            "canonical prefix missing: got {err:?}"
        );
        assert!(
            err.contains("unsupported codec"),
            "underlying cause must be preserved: got {err:?}"
        );
    }

    #[test]
    fn chromaprint_adapter_maps_fingerprint_failed() {
        // `rusty-chromaprint::Fingerprinter::start` failure or the
        // empty-fingerprint "too short" guard come through as
        // `FingerprintError::FingerprintFailed`. Historical phrasing
        // was "Fingerprint generation failed:".
        let r = map_shared_chromaprint_result(Err(
            meedya_fingerprint::FingerprintError::FingerprintFailed(
                "generated empty fingerprint (file too short?)".to_string(),
            ),
        ));
        let err = r.expect_err("fingerprint-failed error must propagate as Err");
        assert!(
            err.starts_with("Fingerprint generation failed:"),
            "canonical prefix missing: got {err:?}"
        );
    }

    #[test]
    fn chromaprint_adapter_wraps_unrelated_errors_with_fallback_prefix() {
        // Any future shared-crate variant that doesn't fit the known
        // buckets should still produce a parseable Err string.
        let r = map_shared_chromaprint_result(Err(
            meedya_fingerprint::FingerprintError::NetworkError("dns".into()),
        ));
        let err = r.expect_err("err");
        assert!(err.starts_with("Fingerprint task failed:"), "got {err:?}");
    }

    #[test]
    fn chromaprint_shared_crate_api_surface_unchanged() {
        // Pin the symbol shape: `meedya_fingerprint::generate_fingerprint`
        // must exist and accept `&Path`, returning
        // `Result<(String, u32), FingerprintError>`. If upstream
        // renames the function or changes the signature, compilation
        // here breaks first.
        let _: fn(&std::path::Path) -> Result<
            (String, u32),
            meedya_fingerprint::FingerprintError,
        > = meedya_fingerprint::generate_fingerprint;
    }
}
