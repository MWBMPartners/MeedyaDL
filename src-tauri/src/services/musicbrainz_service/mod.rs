// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// MusicBrainz recording lookup service.
// ======================================
//
// Queries the MusicBrainz database to discover recording metadata,
// relationships, and cross-platform URLs using ISRC codes. This serves
// two purposes:
//
// 1. **Music video companion fallback** — find music videos via
//    MusicBrainz when MusicKit credentials aren't configured.
//    No Apple Developer account needed.
//
// 2. **Cross-platform discovery groundwork** — MusicBrainz links
//    recordings to Apple Music, YouTube, Spotify, Deezer, Tidal etc.
//    This enables future "if unavailable on one platform, try another"
//    functionality when additional service engines are added.
//
// ## API Details
//
// - Endpoint: `https://musicbrainz.org/ws/2/`
// - Rate limit: 1 request/second (enforced via sleep between requests)
// - Authentication: None needed for read-only lookups
// - User-Agent: Required by MusicBrainz ToS (identifies the application)
// - Format: JSON (`fmt=json`)
//
// ## Endpoint policy (post 2026-11-30 search upgrade)
//
// MusicBrainz's 2026-11-30 search-service upgrade (Solr 9 → 10) changes
// the behaviour of `query=`-style SEARCH requests. This service therefore
// uses only non-search LOOKUP/BROWSE endpoints:
//
// - ISRC:      `GET /ws/2/isrc/{isrc}?inc=…`      (the live production path)
// - Recording: `GET /ws/2/recording/{mbid}?inc=…` (MBID lookup)
// - URL:       `GET /ws/2/url?resource={url}&inc=…` (browse by exact resource;
//              Tier 1 groundwork — not yet wired by any production caller)
//
// ## Lookup Chain
//
// ISRC code (from Apple Music API metadata) → MusicBrainz recording(s) →
// relationships → video recording → external URLs (Apple Music, YouTube)
//
// ## Integration
//
// Called from the enrichment pipeline (Step 6b) as a fallback when the
// MusicKit-based music video lookup (Step 6) finds no videos. Discovered
// cross-platform URLs are currently logged for diagnostics; persisting
// them as file metadata is planned follow-up work.

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;

// ============================================================
// Module split (2026-11-30 search-upgrade readiness work, #1120)
// ============================================================
//
// `musicbrainz_service.rs` becomes a directory module so the shared
// shape-tolerant relation parser (`relations.rs`) and the guarded search
// fallback tier (`search.rs`) each get their own file instead of growing
// this one further — mirroring the precedent at
// `services/download_queue/` (`mod.rs` + topic submodules).
//
// `relations.rs` now owns `classify_url` / `parse_recording_relations`
// (moved out of this file) plus the new shared `collect_relations` /
// `RelationView` relation iterator — the single home of the 2026-11-30
// search-upgrade shape rules. `search.rs` is still a skeleton; its
// content (the guarded S1/S2 search fallback tier) lands in a later
// tranche.
mod relations;
mod search;

// Re-export the two relation-parsing primitives that moved to
// `relations.rs` so every existing call site in this file — production
// code and this file's own `mod tests` alike — keeps working
// unqualified after the split. `classify_url` has no production caller
// left in THIS file (only `parse_recording_relations`, inside
// `relations.rs`, calls it now) — its own re-export is `cfg(test)`-only
// so its 7 pre-existing tests below keep passing unmodified.
#[cfg(test)]
use relations::classify_url;
use relations::parse_recording_relations;

// `search.rs` now carries real content (Tranche D): the guarded S1/S2
// search fallback tier + the advisory era probe. Named imports (not a
// glob) so every symbol this file actually calls is visible at a
// glance, matching the `relations::` re-export style two lines above.
use search::{
    current_search_era, search_recording_mbid, search_url_resources, should_attempt_search_tier,
    should_run_url_search, validate_s2_resource, SearchEra, MAX_SEARCH_ATTEMPTS_PER_ALBUM,
};

// ============================================================
// Constants
// ============================================================

/// Base URL for the MusicBrainz web service API (v2).
const MB_API_BASE: &str = "https://musicbrainz.org/ws/2";

/// Rate limit delay between MusicBrainz API requests.
/// MusicBrainz requires max 1 request/second for unauthenticated clients.
const RATE_LIMIT_DELAY: Duration = Duration::from_millis(1100);

/// HTTP request timeout for MusicBrainz API calls.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// User-Agent header required by MusicBrainz Terms of Service.
/// Must identify the application and provide a contact URL.
///
/// Aliased to the shared `APP_USER_AGENT` constant (was previously a
/// hardcoded `"MeedyaDL/0.6"` literal that silently drifted out of sync
/// with the app's actual version as it moved through the 1.x line —
/// exactly the stale-version ToS defect this alias fixes).
const USER_AGENT: &str = crate::utils::http_client::APP_USER_AGENT;

// ============================================================
// Process-global rate limiter (2026-11-30 readiness, closes §7.7)
// ============================================================

/// Timestamp of the last MusicBrainz request sent by this process,
/// across every call site and every concurrent enrichment task.
///
/// **§7.7 fix.** Pacing used to be loop-local: `if request_count > 0 {
/// sleep(RATE_LIMIT_DELAY) }` inside `lookup_videos_for_tracks_enhanced`,
/// keyed on a per-call counter that started fresh at zero every time
/// the function ran, and duplicated (with its own independent counter
/// or `attempted` flag) in `lookup_recording_by_url` and
/// `lookup_external_urls_for_tracks` besides. That only ever paced
/// requests emitted from inside the SAME loop — it was never actually
/// "1 request/second from this process", just "1 request/second within
/// whichever loop happens to be running", and every one of this
/// module's `pub` entry points is a distinct call path a future caller
/// (a second lookup task, Tranche D's search/probe additions, or
/// anything else added later) could invoke without going through that
/// loop at all. A process-global gate closes that gap for good: it is
/// the single pacing authority that actually enforces "1 request per
/// second, from this whole app, period" — regardless of which function
/// triggers the next request or how many independent call paths exist.
///
/// Mirrors the verified pattern at `odesli_service.rs:46-96` — the
/// `Mutex` is held across the `sleep`, not just the read-then-write of
/// the timestamp, so two callers racing to acquire the lock can't both
/// observe the same stale "last request was N ago" instant and slip
/// under the floor together.
static MB_LAST_REQUEST: std::sync::LazyLock<tokio::sync::Mutex<Option<std::time::Instant>>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(None));

/// Await at least [`RATE_LIMIT_DELAY`] (1.1 s) since the previous
/// MusicBrainz request, process-wide, then stamp "now" as the new
/// last-request time before releasing the lock.
///
/// EVERY MusicBrainz HTTP call site — ISRC lookup, MBID lookup, URL
/// browse, and (from Tranche D) recording/URL search + the era probe —
/// awaits this immediately before `send()`. It replaces every
/// loop-local `tokio::time::sleep(RATE_LIMIT_DELAY)` that used to be
/// scattered across `lookup_videos_for_tracks_enhanced`,
/// `lookup_recording_by_url`, and `lookup_external_urls_for_tracks`;
/// none of those call sites need their own pacing logic any more —
/// they get it for free by calling into a function that awaits this
/// one first.
async fn mb_rate_limit() {
    let mut last = MB_LAST_REQUEST.lock().await;
    if let Some(prev) = *last {
        let elapsed = prev.elapsed();
        if elapsed < RATE_LIMIT_DELAY {
            tokio::time::sleep(RATE_LIMIT_DELAY - elapsed).await;
        }
    }
    *last = Some(std::time::Instant::now());
}

// ============================================================
// Internal: Endpoint-Anomaly Classification + URL Encoding
// ============================================================

/// Builds the error string for an "endpoint answered, but wrongly"
/// condition — a non-2xx status (other than a legitimate 404
/// not-found) or an HTTP 200 whose body lacks the expected top-level
/// shape. These are the two signatures of a server-side API change
/// (e.g. the 2026-11-30 search-service upgrade) as opposed to an
/// ordinary transport failure, so `lookup_videos_for_tracks_enhanced`
/// surfaces the FIRST one per album as a non-verbose warning instead
/// of letting a regression masquerade as "no music videos found".
///
/// Deliberately contains NO query string, NO ISRC, and NO looked-up
/// URL — endpoint kind + status (+ a static shape note) only, per the
/// credential-redaction norms for non-verbose log lines.
fn endpoint_anomaly_error(endpoint_kind: &str, status: u16, detail: Option<&str>) -> String {
    match detail {
        Some(d) => format!("MusicBrainz {endpoint_kind} endpoint anomaly (HTTP {status}): {d}"),
        None => format!("MusicBrainz {endpoint_kind} endpoint anomaly (HTTP {status})"),
    }
}

/// True when an error string produced inside this module represents an
/// endpoint anomaly (see [`endpoint_anomaly_error`]). Substring-based
/// classifier in the house idiom of `process::is_io_error` — transport
/// failures ("MusicBrainz API request failed: …") and JSON-parse
/// failures deliberately do NOT match: they are ordinary network
/// conditions, not API-change signals.
fn is_endpoint_anomaly(error: &str) -> bool {
    error.contains("endpoint anomaly (HTTP ")
}

/// Once-per-invocation guard for the anomaly warning. Pure so the
/// at-most-once behaviour is unit-testable without an AppHandle.
/// Returns `true` exactly when a warning should be emitted now, and
/// flips the flag.
fn should_emit_endpoint_warning(error: &str, already_emitted: &mut bool) -> bool {
    if is_endpoint_anomaly(error) && !*already_emitted {
        *already_emitted = true;
        true
    } else {
        false
    }
}

/// Minimal RFC 3986 §2.3 component encoder — everything that is not
/// unreserved (`A–Z a–z 0–9 - _ . ~`) is percent-encoded. Used to embed
/// a full URL into the `?resource=` query parameter of the MB URL
/// browse endpoint. Mirrors `best_cover_art_service::urlencode` (kept
/// private there); avoids pulling in an extra crate for one call site.
/// Closes the old hand-rolled-encoder gap where only `:` and `/` were
/// encoded and a `?i=…`/`&`/`#` in an Apple Music URL corrupted the
/// request.
fn percent_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

// ============================================================
// Public Types
// ============================================================

/// A MusicBrainz recording with its metadata and discovered URLs.
///
/// Represents a single track/song as identified by MusicBrainz,
/// including all external platform URLs found in its relationships.
#[derive(Debug, Clone, Serialize)]
pub struct MusicBrainzRecording {
    /// MusicBrainz recording UUID (e.g., "5aa053a9-5b84-418f-bb3c-d61df67b3880")
    pub recording_id: String,
    /// Track title from MusicBrainz
    pub title: String,
    /// Artist credit string (may differ from Apple Music's artist name)
    pub artist: Option<String>,
    /// External platform URLs found in recording relationships.
    /// Key = platform identifier, value = URL.
    /// Common keys: "apple_music", "youtube", "spotify", "deezer", "tidal"
    pub external_urls: HashMap<String, String>,
    /// Music video URLs discovered for this recording.
    pub video_urls: Vec<MusicVideoUrl>,
}

/// A discovered music video URL from MusicBrainz relationships.
///
/// Represents a video linked to a recording via MusicBrainz's
/// recording-recording "performance" relationships or URL relationships.
#[derive(Debug, Clone, Serialize)]
pub struct MusicVideoUrl {
    /// Platform identifier: "apple_music", "youtube", etc.
    pub platform: String,
    /// Full URL to the music video on the platform
    pub url: String,
    /// Title of the video (from MusicBrainz, may differ from audio track)
    pub title: Option<String>,
}

// ============================================================
// Public API
// ============================================================

/// Look up a MusicBrainz recording by ISRC code.
///
/// Uses the dedicated non-search ISRC lookup endpoint
/// (`GET /ws/2/isrc/{isrc}`) — NOT the `query=isrc:` recording search,
/// which is affected by MusicBrainz's 2026-11-30 search-service
/// upgrade. The `inc=` parameters inline URL/recording relationships
/// and artist credits into each returned recording, so one request
/// yields the fully-populated result the old search + detail-lookup
/// pair needed two requests for.
///
/// # Arguments
/// * `isrc` - International Standard Recording Code (e.g., "USUG12345678")
///
/// # Returns
/// The first recording bearing the ISRC, with its relationships, or
/// `None` if MusicBrainz has no recording for it (HTTP 404, or an
/// empty `recordings` array).
pub async fn lookup_recording_by_isrc(isrc: &str) -> Result<Option<MusicBrainzRecording>, String> {
    if isrc.is_empty() {
        return Ok(None);
    }

    // Non-search ISRC lookup. All recordings in the response bear this
    // exact ISRC, so "first element" ≈ "any correct match" — the same
    // first-element behaviour the old search path had (scores were
    // never inspected; the lookup endpoint has none to inspect).
    let url = format!(
        "{MB_API_BASE}/isrc/{isrc}?inc=url-rels+recording-rels+artist-credits&fmt=json"
    );

    log::debug!("MusicBrainz: looking up ISRC {isrc}");

    // Create HTTP client with timeout and required User-Agent header
    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    mb_rate_limit().await;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz API request failed: {e}"))?;

    let status = response.status().as_u16();

    // 404 is the endpoint's legitimate "this ISRC is not in the
    // database" answer (lookup semantics — unlike the old search,
    // which answered 200 + empty array). Quiet no-match, NOT an
    // anomaly.
    if status == 404 {
        log::debug!("MusicBrainz: no recording found for ISRC {isrc} (404)");
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(endpoint_anomaly_error("isrc", status, None));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    extract_recording_from_isrc_response(&json)
}

/// Pure extraction half of [`lookup_recording_by_isrc`] — parses the
/// `/ws/2/isrc/{isrc}` response body. Split out so the parse logic is
/// unit-testable from inline JSON fixtures without a live client
/// (house pattern; cf. `extract_syllable_ttml_from_response`).
///
/// Expected shape (HTTP 200):
/// `{"isrc": "…", "recordings": [ { "id", "title", "artist-credit",
/// "relations", … } ]}` — relations and artist-credit inlined per
/// recording because of the `inc=` parameters.
///
/// - `recordings` present but empty → `Ok(None)` (legitimate no-match).
/// - `recordings` missing or not an array → endpoint-anomaly `Err`
///   (a 200 that doesn't look like the documented response is the
///   signature of a server-side API change, and must be
///   distinguishable from a legitimate empty result).
///
/// **§7.3 fix**: a single ISRC can legitimately fan out across MULTIPLE
/// MusicBrainz recordings (different masters/mixes/regional releases
/// sharing the same code), and each one may carry its own, disjoint set
/// of relations. Identity fields (`id`/`title`/`artist`) still come from
/// the FIRST recording — the endpoint returns them in no documented
/// priority order, so "first" is an arbitrary but stable choice — but
/// `external_urls`/`video_urls` are now the UNION across every
/// recording in the response, not just the first one. The pre-fix
/// behaviour (parse only `recordings.first()`) silently dropped
/// relations attached to a later recording; pinned by
/// `isrc_response_merges_relations_across_all_recordings`, which
/// REPLACES the old `isrc_endpoint_takes_first_recording` test that
/// pinned the bug (m1 — the one sanctioned pre-existing-test rewrite in
/// this work).
fn extract_recording_from_isrc_response(
    json: &serde_json::Value,
) -> Result<Option<MusicBrainzRecording>, String> {
    let Some(recordings) = json.get("recordings").and_then(|r| r.as_array()) else {
        return Err(endpoint_anomaly_error(
            "isrc",
            200,
            Some("response body is missing the expected 'recordings' array"),
        ));
    };

    let Some(first) = recordings.first() else {
        log::debug!("MusicBrainz: ISRC known but has no recordings attached");
        return Ok(None);
    };

    let recording_id = first
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if recording_id.is_empty() {
        // A recording object with no usable MBID — treat as no-match
        // (mirrors the old extract_first_recording_from_search guard).
        return Ok(None);
    }

    let title = first
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let artist = first
        .get("artist-credit")
        .and_then(|ac| ac.as_array())
        .and_then(|arr| arr.first())
        .and_then(|a| a.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Relations are inlined on each recording object itself (that is
    // what the inc= parameters buy us) — union them across every
    // recording bearing this ISRC. First-writer-wins per platform key
    // for `external_urls` (mirrors the "identity comes from the first
    // recording" precedent above rather than letting a later recording
    // silently overwrite an earlier match); dedup by URL for
    // `video_urls`.
    let mut external_urls: HashMap<String, String> = HashMap::new();
    let mut video_urls: Vec<MusicVideoUrl> = Vec::new();
    let mut seen_video_urls: std::collections::HashSet<String> = std::collections::HashSet::new();

    for rec in recordings {
        let (urls, videos) = parse_recording_relations(rec);

        for (platform, url) in urls {
            external_urls.entry(platform).or_insert(url);
        }

        for video in videos {
            if seen_video_urls.insert(video.url.clone()) {
                video_urls.push(video);
            }
        }
    }

    log::debug!(
        "MusicBrainz: found recording {} — \"{}\" by {} ({} external URL(s), {} video URL(s))",
        recording_id,
        title,
        artist.as_deref().unwrap_or("unknown"),
        external_urls.len(),
        video_urls.len()
    );

    Ok(Some(MusicBrainzRecording {
        recording_id,
        title,
        artist,
        external_urls,
        video_urls,
    }))
}

/// Information for looking up a track on MusicBrainz.
///
/// Carries all available identifiers for a track, used by the
/// identifier-lookup priority chain (T1 URL → T2 ISRC → T3 recording
/// ID) and, when those all miss, the guarded search fallback tier (S1,
/// Tranche D).
#[derive(Debug, Clone)]
pub struct TrackLookupInfo {
    /// Internal song/download ID (for logging)
    pub song_id: String,
    /// Apple Music song URL (primary discovery path — search MB external links)
    pub apple_music_url: Option<String>,
    /// ISRC code from Apple Music API metadata (secondary path)
    pub isrc: Option<String>,
    /// MusicBrainz recording ID from AcoustID fingerprinting (tertiary path)
    pub musicbrainz_recording_id: Option<String>,
    /// Track artist name — S1's `artist:"…"` search clause input AND
    /// the artist-credit confidence check on the top hit (M2a: per-track
    /// artist, populated by the call site — Tranche E — from
    /// `TrackMetadata.artist_name` with an album-artist fallback, NOT
    /// the album artist alone; a compilation/various-artists album has
    /// per-track artists that legitimately differ from the album
    /// artist, and using the wrong one would reject every genuine S1
    /// match on such an album).
    pub artist: Option<String>,
    /// Track title — S1's `recording:"…"` search clause input.
    pub title: Option<String>,
}

/// Album-scoped inputs the per-track [`TrackLookupInfo`] chain can't
/// carry — S2 (Tranche D's once-per-album URL search) needs the
/// album's own URL, and both S1 and S2 need to know whether the
/// operator has the search fallback tier switched on at all.
#[derive(Debug, Clone)]
pub struct AlbumLookupContext {
    /// The queue item's own Apple Music album URL — S2's once-per-album
    /// search input (`canonicalise_apple_music_url` → numeric album ID
    /// → `url:https\://music.apple.com/*/album/*{id}*`). `None` when
    /// the caller has no album URL in scope (e.g. a track-only queue
    /// item), which simply skips S2 for that album.
    pub album_url: Option<String>,
    /// Mirror of the `musicbrainz_search_fallback` setting (Tranche F).
    /// `false` makes S1 and S2 completely inert — bit-for-bit identical
    /// behaviour to today's T1/T2/T3-only chain. The setting defaults
    /// to **true** (opt-out, not opt-in — see its doc-comment in
    /// `settings.rs`); this field is inert in practice only because
    /// the whole `lookup_videos_for_tracks_enhanced` call is itself
    /// gated behind `musicbrainz_lookup` / `music_video_companion`
    /// (both default-off) at Step 6b. `musicbrainz_search_fallback` is
    /// the kill switch for the search tier alone, once that gate has
    /// already let the call through.
    pub search_fallback: bool,
}

/// Enhanced track video lookup with the exact-identifier priority chain
/// (T1–T3), falling back per [`AlbumLookupContext::search_fallback`] to
/// the guarded search tier (S1/S2, wired in Tranche D) when a track's
/// identifiers all miss.
///
/// Priority chain per track:
/// 1. Apple Music URL → MusicBrainz external link search (T1)
/// 2. ISRC → MusicBrainz recording search (T2, the live production path)
/// 3. MusicBrainz recording ID → direct lookup, from AcoustID (T3)
///
/// `found` (§0.2 M1) means **exact-identifier resolution** — any of
/// T1/T2/T3 returning `Ok(Some(_))`, regardless of whether the resolved
/// recording carried video relations. It is deliberately NOT "found a
/// video": with the §7.3 merge-across-all-ISRC-recordings fix
/// (`relations.rs`), an ISRC-resolved-but-videoless track has already
/// had every relation MusicBrainz has for that recording inspected, so
/// re-running a text search would spend two more requests to
/// rediscover the same recording for ~zero yield. Pinned by
/// `search_tier_skipped_when_isrc_resolved_even_without_videos`
/// (Tranche D).
pub async fn lookup_videos_for_tracks_enhanced(
    app: &tauri::AppHandle,
    download_id: &str,
    tracks: &[TrackLookupInfo],
    ctx: &AlbumLookupContext,
) -> Result<Vec<MusicVideoUrl>, String> {
    use crate::utils::activity_log::{emit_download_log, emit_download_warn, emit_verbose_download_log};

    emit_verbose_download_log(
        app,
        download_id,
        &format!(
            "MusicBrainz: starting lookup for {} track(s) (search fallback: {})",
            tracks.len(),
            if ctx.search_fallback { "enabled" } else { "disabled" }
        ),
    );

    let mut all_videos = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();
    // At most ONE non-verbose endpoint-anomaly warning per invocation
    // (== per album — this fn is called once per album by the Step 6b
    // lookup task). Everything else stays verbose-only. Extended in
    // this tranche to cover the new "recording-search" / "url-search"
    // endpoint kinds S1/S2 introduce — same flag, same once-per-album
    // contract, no per-tier-kind bookkeeping needed.
    let mut endpoint_warning_emitted = false;
    // S1's per-album SEARCH-request cap (§0.4 request budget;
    // MAX_SEARCH_ATTEMPTS_PER_ALBUM = 10) — counts every search
    // attempt, accepted or rejected, across the whole per-track loop
    // below. S2 needs no equivalent counter: it runs at most once,
    // after the loop, by construction.
    let mut search_attempts: usize = 0;
    // Tracks whether S2 (once-per-album, after the loop) is worth
    // attempting at all — an album where every track already resolved
    // via T1/T2/T3/S1 gains nothing from a URL search that could only
    // rediscover duplicate hits.
    let mut any_unresolved = false;

    for track in tracks {
        // §0.2 M1 — see the doc-comment above for the full rationale;
        // "found" = exact-identifier resolution, not "video found".
        let mut found = false;

        // Tier 1: Try Apple Music URL lookup on MusicBrainz
        // (search for recordings that have this URL as an external link)
        if let Some(ref am_url) = track.apple_music_url {
            // Pacing across T1/T2/T3 is handled by `mb_rate_limit()`
            // inside `lookup_recording_by_url` itself (§7.7) — no
            // loop-local sleep/counter needed here any more.
            emit_verbose_download_log(
                app,
                download_id,
                &format!(
                    "MusicBrainz: Tier 1 — looking up song {} via Apple Music URL",
                    track.song_id
                ),
            );

            // Search MusicBrainz for recordings linked to this Apple Music URL
            match lookup_recording_by_url(am_url).await {
                Ok(Some(recording)) => {
                    for video in &recording.video_urls {
                        if seen_urls.insert(video.url.clone()) {
                            all_videos.push(video.clone());
                        }
                    }
                    found = true;
                    emit_download_log(
                        app,
                        download_id,
                        &format!(
                            "MusicBrainz: matched song {} via Apple Music URL (Tier 1)",
                            track.song_id
                        ),
                    );
                }
                Ok(None) => {
                    emit_verbose_download_log(
                        app,
                        download_id,
                        &format!("MusicBrainz: Tier 1 — no match for URL {am_url}"),
                    );
                }
                Err(e) => {
                    emit_verbose_download_log(
                        app,
                        download_id,
                        &format!("MusicBrainz: Tier 1 — URL lookup failed: {e}"),
                    );
                    if should_emit_endpoint_warning(&e, &mut endpoint_warning_emitted) {
                        emit_download_warn(
                            app,
                            download_id,
                            &format!(
                                "MusicBrainz: unexpected API response — {e}. \
                                 Music-video discovery may be incomplete for this album; \
                                 the download itself is not affected."
                            ),
                        );
                    }
                }
            }
        }

        // Tier 2: Try ISRC lookup (if Tier 1 didn't find anything)
        if !found {
            if let Some(ref isrc) = track.isrc {
                // Pacing handled by `mb_rate_limit()` inside
                // `lookup_recording_by_isrc` itself (§7.7).
                emit_verbose_download_log(
                    app,
                    download_id,
                    &format!(
                        "MusicBrainz: Tier 2 — looking up song {} via ISRC {isrc}",
                        track.song_id
                    ),
                );

                match lookup_recording_by_isrc(isrc).await {
                    Ok(Some(recording)) => {
                        for video in &recording.video_urls {
                            if seen_urls.insert(video.url.clone()) {
                                all_videos.push(video.clone());
                            }
                        }
                        found = true;
                        emit_download_log(
                            app,
                            download_id,
                            &format!(
                                "MusicBrainz: matched song {} via ISRC {isrc} (Tier 2)",
                                track.song_id
                            ),
                        );
                    }
                    Ok(None) => {
                        emit_verbose_download_log(
                            app,
                            download_id,
                            &format!("MusicBrainz: Tier 2 — no match for ISRC {isrc}"),
                        );
                    }
                    Err(e) => {
                        emit_verbose_download_log(
                            app,
                            download_id,
                            &format!("MusicBrainz: Tier 2 — ISRC lookup failed: {e}"),
                        );
                        if should_emit_endpoint_warning(&e, &mut endpoint_warning_emitted) {
                            emit_download_warn(
                                app,
                                download_id,
                                &format!(
                                    "MusicBrainz: unexpected API response — {e}. \
                                     Music-video discovery may be incomplete for this album; \
                                     the download itself is not affected."
                                ),
                            );
                        }
                    }
                }
            }
        }

        // Tier 3: Try direct MusicBrainz recording ID lookup (from AcoustID)
        if !found {
            if let Some(ref mb_id) = track.musicbrainz_recording_id {
                // Pacing handled by `mb_rate_limit()` inside
                // `lookup_recording_by_id` itself (§7.7).
                emit_verbose_download_log(
                    app,
                    download_id,
                    &format!(
                        "MusicBrainz: Tier 3 — looking up song {} via recording ID {mb_id}",
                        track.song_id
                    ),
                );

                match lookup_recording_by_id(mb_id).await {
                    Ok(Some(recording)) => {
                        for video in &recording.video_urls {
                            if seen_urls.insert(video.url.clone()) {
                                all_videos.push(video.clone());
                            }
                        }
                        // §7.9 audit-defect fix: T3 used to leave
                        // `found` at `false` on a successful lookup —
                        // the ONLY tier that didn't set it. Harmless
                        // while T3 was the last tier in the chain, but
                        // now that S1 (Tranche D) follows T3, an
                        // MBID-resolved-but-videoless track would have
                        // silently fallen through into a redundant text
                        // search instead of being recognised as already
                        // exact-identifier-resolved (§0.2 M1).
                        found = true;
                        emit_download_log(
                            app,
                            download_id,
                            &format!(
                                "MusicBrainz: matched song {} via recording ID {mb_id} (Tier 3)",
                                track.song_id
                            ),
                        );
                    }
                    Ok(None) => {
                        emit_verbose_download_log(
                            app,
                            download_id,
                            &format!("MusicBrainz: Tier 3 — no match for recording ID {mb_id}"),
                        );
                    }
                    Err(e) => {
                        emit_verbose_download_log(
                            app,
                            download_id,
                            &format!("MusicBrainz: Tier 3 — ID lookup failed: {e}"),
                        );
                        if should_emit_endpoint_warning(&e, &mut endpoint_warning_emitted) {
                            emit_download_warn(
                                app,
                                download_id,
                                &format!(
                                    "MusicBrainz: unexpected API response — {e}. \
                                     Music-video discovery may be incomplete for this album; \
                                     the download itself is not affected."
                                ),
                            );
                        }
                    }
                }
            }
        }

        // S1: recording search fallback, only reached when T1/T2/T3 all
        // missed. `should_attempt_search_tier` is inert (guaranteed
        // zero extra requests) unless the operator has
        // `musicbrainz_search_fallback` on AND this track carries both
        // a non-blank artist and title — m5: `Some("")` and
        // whitespace-only both collapse to "absent" via the trim check
        // below, mirrored by
        // `should_attempt_search_tier_requires_nonblank_artist_and_title`
        // in `search.rs`.
        let has_artist_and_title = track.artist.as_deref().is_some_and(|s| !s.trim().is_empty())
            && track.title.as_deref().is_some_and(|s| !s.trim().is_empty());

        if should_attempt_search_tier(
            found,
            search_attempts,
            has_artist_and_title,
            ctx.search_fallback,
        ) {
            // `has_artist_and_title` guarantees both are `Some` with
            // non-blank content at this point.
            let artist = track.artist.as_deref().unwrap_or_default();
            let title = track.title.as_deref().unwrap_or_default();

            search_attempts += 1;
            if search_attempts == MAX_SEARCH_ATTEMPTS_PER_ALBUM {
                emit_verbose_download_log(
                    app,
                    download_id,
                    "MusicBrainz: S1 — per-album search attempt cap reached; \
                     remaining unresolved tracks won't be text-searched",
                );
            }

            emit_verbose_download_log(
                app,
                download_id,
                &format!("MusicBrainz: S1 — searching for song {}", track.song_id),
            );

            match search_recording_mbid(artist, title).await {
                Ok(Some(mbid)) => match lookup_recording_by_id(&mbid).await {
                    Ok(Some(recording)) => {
                        for video in &recording.video_urls {
                            if seen_urls.insert(video.url.clone()) {
                                all_videos.push(video.clone());
                            }
                        }
                        found = true;
                        emit_download_log(
                            app,
                            download_id,
                            &format!(
                                "MusicBrainz: matched song {} via text search (S1)",
                                track.song_id
                            ),
                        );
                    }
                    Ok(None) => {
                        emit_verbose_download_log(
                            app,
                            download_id,
                            &format!(
                                "MusicBrainz: S1 — candidate recording {mbid} for song {} \
                                 had no lookup match",
                                track.song_id
                            ),
                        );
                    }
                    Err(e) => {
                        emit_verbose_download_log(
                            app,
                            download_id,
                            &format!("MusicBrainz: S1 — follow-up lookup failed: {e}"),
                        );
                        if should_emit_endpoint_warning(&e, &mut endpoint_warning_emitted) {
                            emit_download_warn(
                                app,
                                download_id,
                                &format!(
                                    "MusicBrainz: unexpected API response — {e}. \
                                     Music-video discovery may be incomplete for this album; \
                                     the download itself is not affected."
                                ),
                            );
                        }
                    }
                },
                Ok(None) => {
                    emit_verbose_download_log(
                        app,
                        download_id,
                        &format!(
                            "MusicBrainz: S1 — candidate rejected (score/artist) for song {}",
                            track.song_id
                        ),
                    );
                }
                Err(e) => {
                    emit_verbose_download_log(
                        app,
                        download_id,
                        &format!("MusicBrainz: S1 — search request failed: {e}"),
                    );
                    if should_emit_endpoint_warning(&e, &mut endpoint_warning_emitted) {
                        emit_download_warn(
                            app,
                            download_id,
                            &format!(
                                "MusicBrainz: unexpected API response — {e}. \
                                 Music-video discovery may be incomplete for this album; \
                                 the download itself is not affected."
                            ),
                        );
                    }
                }
            }
        }

        if !found {
            any_unresolved = true;
            emit_verbose_download_log(
                app,
                download_id,
                &format!(
                    "MusicBrainz: song {} — no match via any identifier or search tier",
                    track.song_id
                ),
            );
        }
    }

    // S2: once per album, after the per-track loop — only worth
    // attempting when at least one track above never resolved (an
    // already-fully-matched album would only rediscover duplicate
    // hits) and the album URL canonicalises to a numeric, non-library
    // Apple Music album ID (m4).
    if ctx.search_fallback && any_unresolved {
        // m4: both a `canonicalise_apple_music_url` success AND a raw
        // numeric (non-`l.`-prefixed) album ID from
        // `parse_apple_music_url` are required.
        // `canonicalise_apple_music_url` already performs exactly this
        // same library-URL rejection internally and returns `None` for
        // it, but its own return value is a formatted
        // "music.apple.com/{type}/{id}" display string, not the raw
        // numeric ID S2's query needs — so the ID is extracted
        // separately via `parse_apple_music_url` once canonicalisation
        // has confirmed the URL is a genuine, non-library Apple Music
        // resource.
        let album_id = ctx.album_url.as_deref().and_then(|album_url| {
            super::apple_music_api::canonicalise_apple_music_url(album_url)?;
            let normalised = super::apple_music_api::normalize_apple_music_url(album_url);
            let parsed = super::apple_music_api::parse_apple_music_url(&normalised)?;
            if parsed.album_id.is_empty() || parsed.album_id.starts_with("l.") {
                return None;
            }
            Some(parsed.album_id)
        });

        match album_id {
            None => {
                emit_verbose_download_log(
                    app,
                    download_id,
                    "MusicBrainz: S2 — album URL not canonicalisable — skipped",
                );
            }
            Some(album_id) => {
                // Era probe (lazy — only ever consulted here, once per
                // album). Advisory ONLY: it decides whether the
                // request is worth sending, never which parser runs
                // (see `search.rs`'s module doc-comment and the "DO
                // NOT DO" list in the readiness plan).
                let era = current_search_era().await;

                if should_run_url_search(era, false, ctx.search_fallback) {
                    emit_verbose_download_log(
                        app,
                        download_id,
                        &format!("MusicBrainz: S2 — searching for album {album_id}"),
                    );

                    match search_url_resources(&album_id).await {
                        Ok(resources) => {
                            // B1 GUARD: only validated (exact-album-ID)
                            // resources ever reach the browse -> lookup
                            // pipeline; the 2-resource cap is applied
                            // AFTER validation, never before.
                            let mut valid: Vec<&String> = Vec::new();
                            for resource in &resources {
                                if validate_s2_resource(&album_id, resource) {
                                    valid.push(resource);
                                } else {
                                    emit_verbose_download_log(
                                        app,
                                        download_id,
                                        "MusicBrainz: S2 — discarding off-album resource",
                                    );
                                }
                            }

                            for resource in valid.into_iter().take(2) {
                                match browse_recording_id_by_resource(resource).await {
                                    Ok(Some(mbid)) => match lookup_recording_by_id(&mbid).await {
                                        Ok(Some(recording)) => {
                                            for video in &recording.video_urls {
                                                if seen_urls.insert(video.url.clone()) {
                                                    all_videos.push(video.clone());
                                                }
                                            }
                                            emit_download_log(
                                                app,
                                                download_id,
                                                &format!(
                                                    "MusicBrainz: matched album {album_id} \
                                                     via URL search (S2)"
                                                ),
                                            );
                                        }
                                        Ok(None) => {}
                                        Err(e) => {
                                            emit_verbose_download_log(
                                                app,
                                                download_id,
                                                &format!(
                                                    "MusicBrainz: S2 — follow-up lookup failed: {e}"
                                                ),
                                            );
                                            if should_emit_endpoint_warning(
                                                &e,
                                                &mut endpoint_warning_emitted,
                                            ) {
                                                emit_download_warn(
                                                    app,
                                                    download_id,
                                                    &format!(
                                                        "MusicBrainz: unexpected API response — {e}. \
                                                         Music-video discovery may be incomplete for this album; \
                                                         the download itself is not affected."
                                                    ),
                                                );
                                            }
                                        }
                                    },
                                    Ok(None) => {}
                                    Err(e) => {
                                        emit_verbose_download_log(
                                            app,
                                            download_id,
                                            &format!("MusicBrainz: S2 — resource browse failed: {e}"),
                                        );
                                        if should_emit_endpoint_warning(
                                            &e,
                                            &mut endpoint_warning_emitted,
                                        ) {
                                            emit_download_warn(
                                                app,
                                                download_id,
                                                &format!(
                                                    "MusicBrainz: unexpected API response — {e}. \
                                                     Music-video discovery may be incomplete for this album; \
                                                     the download itself is not affected."
                                                ),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            emit_verbose_download_log(
                                app,
                                download_id,
                                &format!("MusicBrainz: S2 — URL search failed: {e}"),
                            );
                            if should_emit_endpoint_warning(&e, &mut endpoint_warning_emitted) {
                                emit_download_warn(
                                    app,
                                    download_id,
                                    &format!(
                                        "MusicBrainz: unexpected API response — {e}. \
                                         Music-video discovery may be incomplete for this album; \
                                         the download itself is not affected."
                                    ),
                                );
                            }
                        }
                    }
                } else if era == SearchEra::PreSolr10 {
                    emit_verbose_download_log(
                        app,
                        download_id,
                        "MusicBrainz: search era: pre-Solr-10 — skipping URL-search sub-tier",
                    );
                }
            }
        }
    }

    Ok(all_videos)
}

/// Look up a MusicBrainz recording via the URL entity that carries the
/// given resource.
///
/// Uses the non-search URL browse endpoint
/// (`GET /ws/2/url?resource={url}&inc=recording-rels`) — NOT the old
/// `query=url:` recording search, which (a) is affected by the
/// 2026-11-30 search-service upgrade, and (b) could never have worked
/// well for recordings anyway: recording–URL relationships were not
/// indexed by the search server at all before that upgrade
/// (SEARCH-452). The browse endpoint reads the database directly, so
/// it sees every recording–URL relationship today.
///
/// #807 storefront-independence: the browse endpoint is EXACT-match on
/// the stored resource — the old leading-wildcard Lucene glob has no
/// browse equivalent. As a bounded substitute, when the exact user URL
/// misses we retry once with the storefront-less canonical form
/// (`https://music.apple.com/{type}/{id}`). Storefronted permutations
/// other than the user's own are not probed; if Tier 1 is ever wired
/// for real, extending the candidate list (e.g. `/us/…`) is the place
/// to do it.
///
/// Currently LATENT: no production caller populates
/// `TrackLookupInfo.apple_music_url` — this is groundwork hardening.
pub async fn lookup_recording_by_url(
    external_url: &str,
) -> Result<Option<MusicBrainzRecording>, String> {
    if external_url.is_empty() {
        return Ok(None);
    }

    // Candidate resources, tried in order: the user's exact URL, then
    // the storefront-and-slug-less canonical form (skipped when it
    // would repeat the exact URL).
    let mut candidates: Vec<String> = vec![external_url.to_string()];
    if let Some(canonical) = super::apple_music_api::canonicalise_apple_music_url(external_url) {
        // canonicalise returns scheme-less "music.apple.com/{type}/{id}";
        // MB stores full URLs, so re-add the scheme.
        let canonical_resource = format!("https://{canonical}");
        if canonical_resource != external_url {
            log::debug!(
                "MusicBrainz: will fall back to canonical resource form if the exact URL misses (#807)"
            );
            candidates.push(canonical_resource);
        }
    }

    // Pacing between the (at most two) candidate browse attempts, and
    // before the follow-up recording lookup, is now handled by
    // `mb_rate_limit()` inside `browse_recording_id_by_resource` /
    // `lookup_recording_by_id` themselves — no loop-local sleep needed
    // here any more (§7.7 process-global rate limiter).
    let mut recording_id: Option<String> = None;
    for resource in &candidates {
        if let Some(mbid) = browse_recording_id_by_resource(resource).await? {
            recording_id = Some(mbid);
            break;
        }
    }

    let Some(mbid) = recording_id else {
        return Ok(None);
    };

    lookup_recording_by_id(&mbid).await
}

/// Browse the MB `url` entity for an exact resource and return the
/// MBID of the first related recording, if any.
///
/// Endpoint: `GET /ws/2/url?resource={percent-encoded url}&inc=recording-rels&fmt=json`.
/// The resource value is fully percent-encoded (RFC 3986 component
/// rules) — the old hand-rolled encoder only covered `:` and `/`,
/// so a `?i=…` query, `&`, `=` or `#` in an Apple Music URL corrupted
/// the request.
async fn browse_recording_id_by_resource(resource: &str) -> Result<Option<String>, String> {
    let url = format!(
        "{MB_API_BASE}/url?resource={}&inc=recording-rels&fmt=json",
        percent_encode_component(resource)
    );

    log::debug!("MusicBrainz: browsing url entity by resource");

    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    mb_rate_limit().await;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz URL browse request failed: {e}"))?;

    let status = response.status().as_u16();

    // 404 = "no url entity stored for this resource" — the endpoint's
    // legitimate miss answer. Quiet, not an anomaly.
    if status == 404 {
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(endpoint_anomaly_error("url-browse", status, None));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    extract_recording_id_from_url_browse(&json)
}

/// Pure extraction half of [`browse_recording_id_by_resource`] —
/// parses a `/ws/2/url?resource=…` response body and returns the MBID
/// of the first related recording.
///
/// Handles BOTH documented response shapes defensively:
/// - single-entity shape (one `resource` param): the url entity at the
///   root — `{"id": …, "resource": …, "relations": […]}`;
/// - browse-list shape: `{"urls": [ {…url entity…} ], "url-count": n}`
///   → the first entity is used.
///
/// Relation scanning goes through the shared shape-tolerant parser
/// (`relations::collect_relations` / `RelationView`) rather than a
/// hand-rolled loop — this used to be the one relation-reading path in
/// the crate that read `rel["target"]` directly instead of through
/// `RelationView::target_mbid()`'s UUID-shape guard, so it accepted
/// ANY non-empty non-http scalar as a recording MBID (audit finding
/// F1). A garbled/truncated value (e.g. `"5aa053a9-5b84"`, a truncated
/// UUID) would silently become a bogus `lookup_recording_by_id`
/// request instead of a rejected one — see
/// `url_browse_legacy_target_garbage_scalar_rejected` below. Sniffing
/// of a missing `target-type` via the entity key (`recording`) is
/// handled uniformly by `RelationView::from_raw`.
///
/// - Recognised shape but no recording relation → `Ok(None)`.
/// - A 200 body with none of `relations` / `urls` / `id` at the root →
///   endpoint-anomaly `Err` (shape mismatch, distinct from a
///   legitimate empty result).
fn extract_recording_id_from_url_browse(
    json: &serde_json::Value,
) -> Result<Option<String>, String> {
    // Resolve which JSON object is the url entity.
    let entity: &serde_json::Value = if json.get("relations").is_some() || json.get("id").is_some()
    {
        // Single-entity shape.
        json
    } else if let Some(urls) = json.get("urls").and_then(|u| u.as_array()) {
        // Browse-list shape.
        match urls.first() {
            Some(first) => first,
            None => return Ok(None), // empty list = legitimate miss
        }
    } else {
        return Err(endpoint_anomaly_error(
            "url-browse",
            200,
            Some("response body has none of the expected 'relations', 'urls', or 'id' keys"),
        ));
    };

    // NOTE: there is deliberately no `entity.get("relations").is_none()`
    // early-return here. Such a guard would be flat-shape-only and would
    // short-circuit BEFORE `collect_relations` got the chance to flatten a
    // nested `relation-list[].relations[]` body (SEARCH-444's pre-upgrade
    // shape), silently missing a legacy-shaped response on the very path
    // S2 hits feed. `collect_relations` already returns an empty Vec when
    // an entity carries no relations in EITHER shape, so falling through
    // the loop to `Ok(None)` preserves the "entity found but no relations
    // — a miss, not an anomaly" semantics for free (#1120 RR-2).
    for rel in relations::collect_relations(entity) {
        if rel.target_type != "recording" {
            continue;
        }
        if let Some(id) = rel.target_mbid() {
            return Ok(Some(id.to_string()));
        }
    }

    Ok(None)
}

/// Get all discovered external URLs for a batch of tracks.
///
/// Similar to [`lookup_videos_for_tracks_enhanced`] but returns all
/// platform URLs (not just video URLs). Useful for cross-platform
/// discovery.
///
/// # Returns
/// A map of song_id → HashMap<platform, url> for all tracks that had
/// MusicBrainz matches.
pub async fn lookup_external_urls_for_tracks(
    tracks: &[(String, Option<String>)],
) -> Result<HashMap<String, HashMap<String, String>>, String> {
    let mut results = HashMap::new();

    // Pacing (including between consecutive MISSES, not just hits) is
    // now handled by `mb_rate_limit()` inside `lookup_recording_by_isrc`
    // itself — this function gets the process-global limiter for free
    // (§7.7) and no longer needs its own attempted-request counter.
    for (song_id, isrc) in tracks {
        let Some(isrc) = isrc else {
            continue;
        };

        match lookup_recording_by_isrc(isrc).await {
            Ok(Some(recording)) if !recording.external_urls.is_empty() => {
                results.insert(song_id.clone(), recording.external_urls);
            }
            _ => {}
        }
    }

    Ok(results)
}

/// Look up a MusicBrainz recording directly by its recording ID.
///
/// Used when the recording ID is already known (e.g., from AcoustID
/// fingerprint results). Skips the ISRC search step entirely.
///
/// # Arguments
/// * `recording_id` - MusicBrainz recording UUID
///
/// # Returns
/// Recording with relationships and external URLs.
pub async fn lookup_recording_by_id(
    recording_id: &str,
) -> Result<Option<MusicBrainzRecording>, String> {
    if recording_id.is_empty() {
        return Ok(None);
    }

    log::debug!("MusicBrainz: looking up recording by ID {recording_id}");

    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    // Fetch recording with URL and recording relationships
    let url = format!(
        "{MB_API_BASE}/recording/{recording_id}?inc=url-rels+recording-rels+artist-credits&fmt=json"
    );

    mb_rate_limit().await;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz API request failed: {e}"))?;

    let status = response.status().as_u16();

    // 404 = recording deleted or merged away — a legitimate miss for
    // an MBID that may have come from stale AcoustID data.
    if status == 404 {
        log::debug!("MusicBrainz: recording {recording_id} not found (404)");
        return Ok(None);
    }

    if !response.status().is_success() {
        return Err(endpoint_anomaly_error("recording", status, None));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let artist = json
        .get("artist-credit")
        .and_then(|ac| ac.as_array())
        .and_then(|arr| arr.first())
        .and_then(|a| a.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Parse relationships (same logic as in lookup_recording_by_isrc)
    let (external_urls, video_urls) = parse_recording_relations(&json);

    Ok(Some(MusicBrainzRecording {
        recording_id: recording_id.to_string(),
        title,
        artist,
        external_urls,
        video_urls,
    }))
}

/// Rewrite an Apple Music URL's storefront code.
///
/// If the URL contains a different storefront than the user's preferred
/// one, returns a new URL with the storefront replaced. Useful when
/// MusicBrainz returns an Apple Music URL for a different region.
///
/// # Examples
/// ```ignore
/// rewrite_apple_music_storefront(
///     "https://music.apple.com/de/album/550152190",
///     "gb"
/// ) // → "https://music.apple.com/gb/album/550152190"
/// ```
#[must_use]
pub fn rewrite_apple_music_storefront(url: &str, target_storefront: &str) -> String {
    // Apple Music URL pattern: https://music.apple.com/{storefront}/{type}/{name}/{id}
    // The storefront is a 2-letter country code after the domain.
    if let Some(domain_end) = url.find("music.apple.com/") {
        let after_domain = &url[domain_end + "music.apple.com/".len()..];
        // Check if the next segment is a 2-3 letter storefront code
        if let Some(slash_pos) = after_domain.find('/') {
            let current_sf = &after_domain[..slash_pos];
            // Only rewrite if it looks like a storefront (2-3 lowercase letters)
            if current_sf.len() >= 2
                && current_sf.len() <= 3
                && current_sf.chars().all(|c| c.is_ascii_lowercase())
                && current_sf != target_storefront
            {
                let prefix = &url[..domain_end + "music.apple.com/".len()];
                let rest = &after_domain[slash_pos..]; // includes the leading /
                return format!("{prefix}{target_storefront}{rest}");
            }
        }
    }
    // Return original if no rewrite needed or URL doesn't match pattern
    url.to_string()
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // URL classification
    // ----------------------------------------------------------

    #[test]
    fn classify_apple_music_url() {
        let result = classify_url("https://music.apple.com/gb/music-video/291812351");
        assert_eq!(
            result,
            Some((
                "apple_music",
                "https://music.apple.com/gb/music-video/291812351"
            ))
        );
    }

    #[test]
    fn classify_youtube_url() {
        let result = classify_url("https://www.youtube.com/watch?v=Eo-KmOd3i7s");
        assert_eq!(
            result,
            Some(("youtube", "https://www.youtube.com/watch?v=Eo-KmOd3i7s"))
        );
    }

    #[test]
    fn classify_youtube_short_url() {
        let result = classify_url("https://youtu.be/Eo-KmOd3i7s");
        assert_eq!(result, Some(("youtube", "https://youtu.be/Eo-KmOd3i7s")));
    }

    #[test]
    fn classify_spotify_url() {
        let result = classify_url("https://open.spotify.com/track/abc123");
        assert_eq!(
            result,
            Some(("spotify", "https://open.spotify.com/track/abc123"))
        );
    }

    #[test]
    fn classify_deezer_url() {
        let result = classify_url("https://www.deezer.com/track/12345");
        assert_eq!(
            result,
            Some(("deezer", "https://www.deezer.com/track/12345"))
        );
    }

    #[test]
    fn classify_tidal_url() {
        let result = classify_url("https://tidal.com/browse/track/12345");
        assert_eq!(
            result,
            Some(("tidal", "https://tidal.com/browse/track/12345"))
        );
    }

    #[test]
    fn classify_unknown_url_returns_none() {
        let result = classify_url("https://example.com/some/path");
        assert_eq!(result, None);
    }

    // ----------------------------------------------------------
    // MusicVideoUrl struct
    // ----------------------------------------------------------

    #[test]
    fn music_video_url_serializes() {
        let mv = MusicVideoUrl {
            platform: "apple_music".to_string(),
            url: "https://music.apple.com/gb/music-video/291812351".to_string(),
            title: Some("Test Video".to_string()),
        };
        let json = serde_json::to_string(&mv).unwrap();
        assert!(json.contains("apple_music"));
        assert!(json.contains("291812351"));
    }

    // ----------------------------------------------------------
    // TrackLookupInfo / AlbumLookupContext structs (Tranche C: the two
    // new S1-input fields, `artist` + `title`, and the new
    // album-scoped context struct)
    // ----------------------------------------------------------

    #[test]
    fn track_lookup_info_serialises_with_new_fields() {
        // TrackLookupInfo doesn't derive Serialize, so "serialises"
        // here means: the struct literal still constructs cleanly with
        // the two new S1-input fields (`artist` + `title`, §0.2 M2a)
        // present alongside the four pre-existing identifier fields,
        // and both `Debug` and `Clone` (still derived) see them.
        let info = TrackLookupInfo {
            song_id: "song-123".to_string(),
            apple_music_url: None,
            isrc: Some("USUG12345678".to_string()),
            musicbrainz_recording_id: None,
            artist: Some("Test Artist".to_string()),
            title: Some("Test Title".to_string()),
        };

        let cloned = info.clone();
        assert_eq!(cloned.artist.as_deref(), Some("Test Artist"));
        assert_eq!(cloned.title.as_deref(), Some("Test Title"));
        assert_eq!(cloned.song_id, "song-123");

        let debug_str = format!("{cloned:?}");
        assert!(debug_str.contains("Test Artist"));
        assert!(debug_str.contains("Test Title"));
    }

    #[test]
    fn album_lookup_context_search_fallback_false_is_inert_default() {
        // Mirrors the now-deleted legacy `(song_id, isrc)`-only compat
        // wrapper's inert default (Tranche E, m2 — the wrapper was
        // removed once its sole caller migrated to building
        // `TrackLookupInfo`/`AlbumLookupContext` directly):
        // `search_fallback: false` is the bit-compat default that kept
        // S1/S2 (Tranche D) completely off.
        let ctx = AlbumLookupContext {
            album_url: None,
            search_fallback: false,
        };
        assert!(!ctx.search_fallback);
        assert!(ctx.album_url.is_none());

        let ctx_with_url = AlbumLookupContext {
            album_url: Some("https://music.apple.com/us/album/1456105020".to_string()),
            search_fallback: true,
        };
        assert!(ctx_with_url.search_fallback);
        assert_eq!(
            ctx_with_url.album_url.as_deref(),
            Some("https://music.apple.com/us/album/1456105020")
        );
    }

    // ----------------------------------------------------------
    // MusicBrainzRecording struct
    // ----------------------------------------------------------

    #[test]
    fn recording_with_empty_urls() {
        let rec = MusicBrainzRecording {
            recording_id: "test-id".to_string(),
            title: "Test Song".to_string(),
            artist: Some("Test Artist".to_string()),
            external_urls: HashMap::new(),
            video_urls: Vec::new(),
        };
        assert!(rec.external_urls.is_empty());
        assert!(rec.video_urls.is_empty());
    }

    #[test]
    fn recording_with_multiple_platforms() {
        let mut urls = HashMap::new();
        urls.insert(
            "apple_music".to_string(),
            "https://music.apple.com/...".to_string(),
        );
        urls.insert("youtube".to_string(), "https://youtube.com/...".to_string());
        urls.insert(
            "spotify".to_string(),
            "https://open.spotify.com/...".to_string(),
        );

        let rec = MusicBrainzRecording {
            recording_id: "test-id".to_string(),
            title: "Test Song".to_string(),
            artist: None,
            external_urls: urls,
            video_urls: Vec::new(),
        };
        assert_eq!(rec.external_urls.len(), 3);
        assert!(rec.external_urls.contains_key("apple_music"));
        assert!(rec.external_urls.contains_key("youtube"));
        assert!(rec.external_urls.contains_key("spotify"));
    }

    // ----------------------------------------------------------
    // Storefront rewriting
    // ----------------------------------------------------------

    #[test]
    fn rewrite_storefront_de_to_gb() {
        let result =
            rewrite_apple_music_storefront("https://music.apple.com/de/album/test/550152190", "gb");
        assert_eq!(result, "https://music.apple.com/gb/album/test/550152190");
    }

    #[test]
    fn rewrite_storefront_us_to_gb() {
        let result = rewrite_apple_music_storefront(
            "https://music.apple.com/us/music-video/test/291812351",
            "gb",
        );
        assert_eq!(
            result,
            "https://music.apple.com/gb/music-video/test/291812351"
        );
    }

    #[test]
    fn rewrite_storefront_same_no_change() {
        let url = "https://music.apple.com/gb/album/test/12345";
        let result = rewrite_apple_music_storefront(url, "gb");
        assert_eq!(result, url);
    }

    #[test]
    fn rewrite_storefront_non_apple_url_unchanged() {
        let url = "https://youtube.com/watch?v=abc";
        let result = rewrite_apple_music_storefront(url, "gb");
        assert_eq!(result, url);
    }

    #[test]
    fn rewrite_storefront_no_storefront_segment() {
        // URL without storefront (geo-non-specific)
        let url = "https://music.apple.com/album/test/12345";
        let result = rewrite_apple_music_storefront(url, "gb");
        // "album" is not a 2-3 char lowercase code, so no rewrite
        assert_eq!(result, url);
    }

    // ----------------------------------------------------------
    // Relationship parsing
    // ----------------------------------------------------------

    #[test]
    fn parse_relations_with_url_relationships() {
        let json = serde_json::json!({
            "title": "Test Song",
            "relations": [
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": {
                        "resource": "https://music.apple.com/gb/album/test/12345"
                    }
                },
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": {
                        "resource": "https://www.youtube.com/watch?v=abc123"
                    }
                }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 2);
        assert!(urls.contains_key("apple_music"));
        assert!(urls.contains_key("youtube"));
        assert!(videos.is_empty()); // No music-video URLs in this test
    }

    #[test]
    fn parse_relations_with_video_url() {
        let json = serde_json::json!({
            "title": "Test Video",
            "relations": [
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": {
                        "resource": "https://music.apple.com/gb/music-video/test/12345"
                    }
                }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 1);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].platform, "apple_music");
    }

    #[test]
    fn parse_relations_empty() {
        let json = serde_json::json!({
            "title": "No Relations"
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert!(urls.is_empty());
        assert!(videos.is_empty());
    }

    // ----------------------------------------------------------
    // ISRC endpoint (Decision A) — extract_recording_from_isrc_response
    // ----------------------------------------------------------

    fn isrc_fixture() -> serde_json::Value {
        serde_json::json!({
            "isrc": "USUG12345678",
            "recordings": [
                {
                    "id": "5aa053a9-5b84-418f-bb3c-d61df67b3880",
                    "title": "Test Song",
                    "artist-credit": [ { "name": "Test Artist" } ],
                    "relations": [
                        {
                            "type": "streaming",
                            "target-type": "url",
                            "url": { "resource": "https://music.apple.com/gb/music-video/test/291812351" }
                        },
                        {
                            "type": "streaming",
                            "target-type": "url",
                            "url": { "resource": "https://open.spotify.com/track/abc123" }
                        }
                    ]
                },
                {
                    "id": "ffffffff-0000-0000-0000-000000000000",
                    "title": "Second Recording Same ISRC",
                    "artist-credit": [ { "name": "Other Artist" } ],
                    "relations": []
                }
            ]
        })
    }

    #[test]
    fn isrc_endpoint_response_extracts_recording_and_urls() {
        let result = extract_recording_from_isrc_response(&isrc_fixture());
        let rec = result.unwrap().unwrap();
        assert_eq!(rec.recording_id, "5aa053a9-5b84-418f-bb3c-d61df67b3880");
        assert_eq!(rec.title, "Test Song");
        assert_eq!(rec.artist.as_deref(), Some("Test Artist"));
        assert_eq!(rec.external_urls.len(), 2);
        assert!(rec.external_urls.contains_key("apple_music"));
        assert!(rec.external_urls.contains_key("spotify"));
        assert_eq!(rec.video_urls.len(), 1);
        assert_eq!(rec.video_urls[0].platform, "apple_music");
        assert_eq!(rec.video_urls[0].title.as_deref(), Some("Test Song"));
    }

    /// Extended from the real single-recording capture at
    /// `{MB}/prod-A1-isrc.json` (2026-09-01, ISRC `GBAYE0601498`,
    /// "Yellow Submarine" by The Beatles): recordings[0] keeps a
    /// representative subset of the real recording-recording relations
    /// but has its url-rel REMOVED, and a second, synthetic recording
    /// is appended carrying ONLY that same real Spotify url-rel. This
    /// is the exact shape the §7.3 fix targets — a real ISRC
    /// legitimately fans out across multiple recordings, and the one
    /// bearing the useful relation isn't always first in the array. The
    /// pre-fix `recordings.first()`-only parser returned zero external
    /// URLs for this response; the fixed union-across-all-recordings
    /// parser must not.
    fn isrc_fixture_multi_recording_merge() -> serde_json::Value {
        serde_json::json!({
            "isrc": "GBAYE0601498",
            "recordings": [
                {
                    "id": "b2181aae-5cba-496c-bb0c-b4cc0109ebf8",
                    "title": "Yellow Submarine",
                    "artist-credit": [ { "name": "The Beatles" } ],
                    "relations": [
                        {
                            "type": "mashes up",
                            "target-type": "recording",
                            "ended": false,
                            "recording": {
                                "id": "60f68854-d44c-4c6e-9e23-000103b1669d",
                                "title": "Lovely NYC"
                            }
                        },
                        {
                            "type": "remix",
                            "target-type": "recording",
                            "ended": false,
                            "recording": {
                                "id": "8417ac27-57b8-4160-b07b-32772bd897d1",
                                "title": "Yellow Submarine",
                                "disambiguation": "1999 remix"
                            }
                        }
                    ]
                },
                {
                    "id": "63612e24-0000-0000-0000-000000000000",
                    "title": "Yellow Submarine",
                    "artist-credit": [ { "name": "The Beatles" } ],
                    "relations": [
                        {
                            "type": "free streaming",
                            "target-type": "url",
                            "ended": false,
                            "url": {
                                "id": "63612e24-efed-4db5-81de-9bb1c768a715",
                                "resource": "https://open.spotify.com/track/7zRmGvtSy36Jr19U5OInJT"
                            }
                        }
                    ]
                }
            ]
        })
    }

    #[test]
    fn isrc_response_merges_relations_across_all_recordings() {
        // REPLACES `isrc_endpoint_takes_first_recording` (m1 / §7.3 fix
        // — the one sanctioned pre-existing-test rewrite in this work):
        // the old test pinned the exact behaviour this fix corrects
        // (`recordings.first()`-only relation parsing silently dropping
        // relations attached to a later recording). Identity fields
        // still come from the first recording, unchanged; the union of
        // external_urls/video_urls across ALL recordings is new.
        let result = extract_recording_from_isrc_response(&isrc_fixture_multi_recording_merge());
        let rec = result.unwrap().unwrap();

        // Identity: still the FIRST recording, unchanged from before.
        assert_eq!(rec.recording_id, "b2181aae-5cba-496c-bb0c-b4cc0109ebf8");
        assert_eq!(rec.title, "Yellow Submarine");

        // Relations: the Spotify URL lives ONLY on the SECOND
        // recording — a first-recording-only parser would see zero
        // external URLs here.
        assert_eq!(rec.external_urls.len(), 1);
        assert_eq!(
            rec.external_urls.get("spotify"),
            Some(&"https://open.spotify.com/track/7zRmGvtSy36Jr19U5OInJT".to_string())
        );
    }

    #[test]
    fn isrc_endpoint_empty_recordings_returns_none_quietly() {
        let json = serde_json::json!({"isrc": "GBAAA0000001", "recordings": []});
        let result = extract_recording_from_isrc_response(&json);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn isrc_endpoint_recording_without_id_returns_none() {
        let json = serde_json::json!({
            "isrc": "GBAAA0000001",
            "recordings": [ { "title": "No Id Here" } ]
        });
        let result = extract_recording_from_isrc_response(&json);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn isrc_endpoint_missing_recordings_key_is_anomaly() {
        let json = serde_json::json!({"error": "something else entirely"});
        let result = extract_recording_from_isrc_response(&json);
        let e = result.unwrap_err();
        assert!(is_endpoint_anomaly(&e));
        assert!(e.contains("isrc"));
        assert!(e.contains("HTTP 200"));
        assert!(e.contains("recordings"));
    }

    // ----------------------------------------------------------
    // parse_recording_relations hardening (SEARCH-752/751/753)
    // ----------------------------------------------------------

    #[test]
    fn parse_relations_missing_target_type_falls_back_to_entity_object() {
        let json = serde_json::json!({
            "title": "Shape Shift",
            "relations": [
                { "type": "streaming", "url": { "resource": "https://music.apple.com/gb/music-video/x/999" } }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 1);
        assert!(urls.contains_key("apple_music"));
        assert_eq!(videos.len(), 1);
    }

    #[test]
    fn parse_relations_tolerates_legacy_target_scalar() {
        let json = serde_json::json!({
            "title": "Legacy Shape",
            "relations": [
                { "type": "streaming", "target-type": "url",
                  "target": "https://music.apple.com/gb/music-video/y/123" },
                { "type": "streaming", "target-type": "url",
                  "target": "0f0f0f0f-1111-2222-3333-444444444444" }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 1);
        assert_eq!(videos.len(), 1);
    }

    #[test]
    fn parse_relations_prefers_entity_object_over_legacy_target() {
        let json = serde_json::json!({
            "title": "Prefer Entity",
            "relations": [
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": { "resource": "https://tidal.com/browse/track/1" },
                    "target": "https://open.spotify.com/track/zzz"
                }
            ]
        });

        let (urls, _videos) = parse_recording_relations(&json);
        assert!(urls.contains_key("tidal"));
        assert!(!urls.contains_key("spotify"));
    }

    #[test]
    fn parse_relations_video_classification_unchanged() {
        let json = serde_json::json!({
            "title": "Multi",
            "relations": [
                { "type": "streaming", "target-type": "url",
                  "url": { "resource": "https://music.apple.com/gb/music-video/test/1" } },
                { "type": "streaming", "target-type": "url",
                  "url": { "resource": "https://www.youtube.com/video/abc" } },
                { "type": "streaming", "target-type": "url",
                  "url": { "resource": "https://music.apple.com/gb/album/test/2" } }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        // The two apple_music URLs collide on the same map key (HashMap
        // insert overwrites), so only 2 distinct platform keys survive.
        assert_eq!(urls.len(), 2);
        assert!(urls.contains_key("apple_music"));
        assert!(urls.contains_key("youtube"));
        assert_eq!(videos.len(), 2);
    }

    // ----------------------------------------------------------
    // URL browse endpoint (Decision B) — extract_recording_id_from_url_browse
    // ----------------------------------------------------------

    #[test]
    fn url_browse_single_entity_shape_yields_recording_id() {
        let json = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "resource": "https://music.apple.com/us/album/1729264859",
            "relations": [
                { "type": "free streaming", "target-type": "recording",
                  "recording": { "id": "5aa053a9-5b84-418f-bb3c-d61df67b3880", "title": "Test Song" } }
            ]
        });

        let result = extract_recording_id_from_url_browse(&json);
        assert_eq!(
            result.unwrap(),
            Some("5aa053a9-5b84-418f-bb3c-d61df67b3880".to_string())
        );
    }

    #[test]
    fn url_browse_list_shape_yields_recording_id() {
        let entity = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "resource": "https://music.apple.com/us/album/1729264859",
            "relations": [
                { "type": "free streaming", "target-type": "recording",
                  "recording": { "id": "5aa053a9-5b84-418f-bb3c-d61df67b3880", "title": "Test Song" } }
            ]
        });
        let json = serde_json::json!({
            "url-count": 1,
            "url-offset": 0,
            "urls": [ entity ]
        });

        let result = extract_recording_id_from_url_browse(&json);
        assert_eq!(
            result.unwrap(),
            Some("5aa053a9-5b84-418f-bb3c-d61df67b3880".to_string())
        );
    }

    #[test]
    fn url_browse_empty_list_returns_none() {
        let json = serde_json::json!({"url-count": 0, "url-offset": 0, "urls": []});
        let result = extract_recording_id_from_url_browse(&json);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn url_browse_entity_without_recording_rel_returns_none() {
        let json = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "relations": [
                { "type": "member of band", "target-type": "artist",
                  "artist": { "id": "some-artist-id", "name": "Someone" } }
            ]
        });
        let result = extract_recording_id_from_url_browse(&json);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn url_browse_legacy_target_mbid_tolerated() {
        let json = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "relations": [
                { "type": "x", "target-type": "recording",
                  "target": "5aa053a9-5b84-418f-bb3c-d61df67b3880" }
            ]
        });
        let result = extract_recording_id_from_url_browse(&json);
        assert_eq!(
            result.unwrap(),
            Some("5aa053a9-5b84-418f-bb3c-d61df67b3880".to_string())
        );

        // A url-shaped target must NOT be tolerated as an MBID.
        let json_url_target = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "relations": [
                { "type": "x", "target-type": "recording",
                  "target": "https://music.apple.com/us/album/1" }
            ]
        });
        let result2 = extract_recording_id_from_url_browse(&json_url_target);
        assert!(matches!(result2, Ok(None)));
    }

    #[test]
    fn url_browse_legacy_target_garbage_scalar_rejected() {
        // Audit finding F1: the pre-fix hand-rolled loop accepted ANY
        // non-empty non-http scalar `target` as a recording MBID — a
        // truncated/garbled value like this 13-byte fragment of a real
        // UUID would have silently produced a bogus
        // lookup_recording_by_id request. Post-fix, relation scanning
        // goes through `RelationView::target_mbid()`'s UUID-shape guard
        // (`is_uuid_shaped` — exactly 36 bytes), so a non-UUID-shaped
        // scalar is rejected rather than trusted.
        let json = serde_json::json!({
            "id": "aaaa1111-bbbb-cccc-dddd-eeee22223333",
            "relations": [
                { "type": "x", "target-type": "recording",
                  "target": "5aa053a9-5b84" }
            ]
        });
        let result = extract_recording_id_from_url_browse(&json);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn url_browse_unrecognised_shape_is_anomaly() {
        let json = serde_json::json!({"whatever": true});
        let result = extract_recording_id_from_url_browse(&json);
        let e = result.unwrap_err();
        assert!(is_endpoint_anomaly(&e));
        assert!(e.contains("url-browse"));
    }

    // ----------------------------------------------------------
    // percent_encode_component
    // ----------------------------------------------------------

    #[test]
    fn percent_encode_component_encodes_query_reserved() {
        assert_eq!(
            percent_encode_component("https://music.apple.com/gb/album/x/123?i=456&l=en#frag"),
            "https%3A%2F%2Fmusic.apple.com%2Fgb%2Falbum%2Fx%2F123%3Fi%3D456%26l%3Den%23frag"
        );
    }

    #[test]
    fn percent_encode_component_preserves_unreserved() {
        assert_eq!(percent_encode_component("AbZ-09._~"), "AbZ-09._~");
    }

    // ----------------------------------------------------------
    // Endpoint-anomaly classification
    // ----------------------------------------------------------

    #[test]
    fn endpoint_anomaly_error_formats_and_classifies() {
        let e = endpoint_anomaly_error("isrc", 503, None);
        assert_eq!(e, "MusicBrainz isrc endpoint anomaly (HTTP 503)");
        assert!(is_endpoint_anomaly(&e));
        let e2 = endpoint_anomaly_error("url-browse", 200, Some("shape note"));
        assert_eq!(e2, "MusicBrainz url-browse endpoint anomaly (HTTP 200): shape note");
        assert!(is_endpoint_anomaly(&e2));
    }

    #[test]
    fn transport_and_parse_errors_are_not_anomalies() {
        assert!(!is_endpoint_anomaly(
            "MusicBrainz API request failed: connection refused"
        ));
        assert!(!is_endpoint_anomaly(
            "Failed to parse MusicBrainz response: EOF"
        ));
        assert!(!is_endpoint_anomaly(
            "MusicBrainz URL browse request failed: timeout"
        ));
    }

    #[test]
    fn endpoint_warning_emitted_at_most_once() {
        let mut flag = false;
        let anomaly = endpoint_anomaly_error("isrc", 500, None);
        assert!(should_emit_endpoint_warning(&anomaly, &mut flag)); // first anomaly → emit
        assert!(!should_emit_endpoint_warning(&anomaly, &mut flag)); // second anomaly → suppressed

        let mut flag2 = false;
        assert!(!should_emit_endpoint_warning(
            "MusicBrainz API request failed: x",
            &mut flag2
        ));
        assert!(!flag2); // transport error neither emits nor consumes the budget
    }
}
