// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Remote feature-availability client.
// =====================================
//
// Resolves which MeedyaDL features are currently available, using a
// three-tier chain that mirrors `services::service_status` in shape:
//
//     in-memory snapshot  ->  sticky disk cache  ->  compiled defaults
//
// The remote endpoint is MWBM-IntAppsAPI (MeedyaDL's own backend, not a
// third party). Programme context, threat model and the honest limits of
// client-side enforcement live in
// `.claude/memory/project_remote_feature_control.md`. Two points from that
// document govern the code below:
//
//   * **Fail-open is the recommended default.** An unreachable server must
//     never brick an offline app. Every failure path here lands on the
//     previous verdicts, or on all-enabled defaults.
//   * **Client-side enforcement is advisory**, not a guarantee. Nothing in
//     this module should be documented or designed as though a determined
//     user could not defeat it.
//
// ## This module never returns an error for a resolution
//
// [`current`] and [`refresh`] both return a `FeatureFlagsSnapshot`, not a
// `Result`. There is no failure mode that should leave a caller without an
// answer: the worst case is "compiled defaults, everything enabled". The
// IPC wrappers in `commands::feature_flags` re-wrap in `Result` only
// because that is the Tauri command shape (and because the rate limiter
// can legitimately reject a call) — the resolution itself is infallible.
//
// ## Scope
//
// Backend only. This module resolves and caches verdicts; it does **not**
// gate or enforce anything. No call site anywhere else in the codebase
// consults it yet — enforcement lands as a separate change so the
// transport can be shipped, observed, and corrected before any feature's
// behaviour depends on it.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use tauri::AppHandle;

use crate::models::feature_flags::{
    FeatureFlagsSnapshot, FetchMeta, FlagNotice, FlagSource,
};
use crate::utils::activity_log::{emit_app_log, emit_verbose_app_log};
use crate::utils::http_client::{build_client, full_user_agent, ClientConfig};
use crate::utils::platform;

// ---------------------------------------------------------------------
// Build-time transport configuration
// ---------------------------------------------------------------------

/// Base URL of the MWBM-IntAppsAPI deployment, baked in at compile time.
///
/// Follows the same `option_env!` pattern as `ACOUSTID_API_KEY` in
/// `acoustid_service.rs` and `DEV_ACCESS_HASH` in `commands/credentials.rs`:
/// official release builds supply the value from a CI secret; forks and
/// local `cargo tauri dev` builds simply don't, and the whole feature goes
/// quietly inert (see [`remote_config`]).
const INTAPPS_BASE_URL: Option<&str> = option_env!("INTAPPS_BASE_URL");

/// Application identifier registered with MWBM-IntAppsAPI, sent as the
/// `X-App-ID` header.
const INTAPPS_APP_ID: Option<&str> = option_env!("INTAPPS_APP_ID");

/// API key for MWBM-IntAppsAPI, sent as the `X-API-Key` header.
///
/// This key ships inside the binary and is extractable by anyone holding
/// the app. It is **attribution and abuse filtering, not a security
/// boundary** — never design or document it as though it were. Its value
/// is never logged (see [`fetch_remote`]).
const INTAPPS_API_KEY: Option<&str> = option_env!("INTAPPS_API_KEY");

/// Path segment for MeedyaDL's flag set on the API.
const FEATURES_PATH: &str = "/v1/features/meedyadl";

/// HTTP request timeout, in seconds. Short by design — this is a
/// background refresh nobody is waiting on, and a hung request must not
/// keep a runtime task alive indefinitely.
const REQUEST_TIMEOUT_SECS: u64 = 10;

/// Filename of the sticky on-disk cache in the app data directory.
const CACHE_FILENAME: &str = "feature-flags-cache.json";

/// Context label used in cache write error messages.
const CACHE_CONTEXT: &str = "feature flags cache";

/// The single activity-log line a fetch failure is allowed to produce.
///
/// Deliberately says nothing about the endpoint, the error, or the retry
/// count. Per `.github/release-notes/STYLE_GUIDE.md` the delivery
/// mechanism is confidential; naming the host in a user-facing string
/// hands anyone wanting to defeat the switch the exact thing to block.
const REFRESH_FAILED_MESSAGE: &str =
    "Feature availability refresh failed — keeping last known status";

// ---------------------------------------------------------------------
// Sanity floor
// ---------------------------------------------------------------------

/// Flag keys this client refuses to let the server switch off.
///
/// The threat that most deserves design attention here is **a compromised
/// API or admin account pushing a malicious flag to the entire installed
/// base**. A payload that disabled the flag fetcher itself, or the
/// updater, would leave every install permanently blind and permanently
/// un-patchable — no subsequent corrective flag could ever reach it, and
/// no app update could fix it either. Those two capabilities are therefore
/// structurally ungateable: [`apply_sanity_floor`] forces them back on
/// regardless of what any payload says, and it runs on **every** load path
/// (network, cache, defaults) so there is no ordering by which a hostile
/// payload could slip past it.
///
/// This is containment, not perimeter: it does not stop a bad flag being
/// published, it stops a bad flag being unrecoverable.
pub const UNGATEABLE_KEYS: &[&str] = &["core.remote-config", "core.updater"];

// ---------------------------------------------------------------------
// Process-global in-memory snapshot
// ---------------------------------------------------------------------

/// The snapshot resolved for this process, if one has been resolved yet.
///
/// `std::sync::RwLock::new` is `const`, so this needs no `LazyLock`
/// wrapper. Reads vastly outnumber writes (every gate consults it; only a
/// refresh writes), which is exactly the access pattern `RwLock` suits.
///
/// A poisoned lock is treated as "no snapshot" rather than as a panic:
/// this module's whole contract is that it always answers, so even lock
/// poisoning degrades to compiled defaults rather than taking a caller
/// down with it.
static CURRENT: RwLock<Option<FeatureFlagsSnapshot>> = RwLock::new(None);

// ---------------------------------------------------------------------
// Public resolution API
// ---------------------------------------------------------------------

/// Returns the currently-resolved snapshot. **Synchronous, never touches
/// the network, never fails.**
///
/// Resolution order:
///   1. the in-memory snapshot, if this process has already resolved one;
///   2. the sticky on-disk cache;
///   3. compiled defaults (empty verdict map => every feature enabled).
///
/// The result always has [`apply_sanity_floor`] applied.
pub fn current(app: &AppHandle) -> FeatureFlagsSnapshot {
    // Tier 1 — in-memory. Already sanity-floored when it was stored.
    if let Ok(guard) = CURRENT.read() {
        if let Some(snapshot) = guard.as_ref() {
            return snapshot.clone();
        }
    }

    // Tier 2 — sticky disk cache.
    let path = cache_path(app);
    match load_cache(&path) {
        Ok(mut snapshot) => {
            // `meta` is diagnostics: rewrite `source` to reflect how THIS
            // process obtained the verdicts, while preserving the
            // `fetched_at` of the successful fetch that produced them.
            snapshot.meta.source = FlagSource::DiskCache;
            snapshot.meta.consecutive_failures = 0;
            snapshot.meta.last_error = None;
            apply_sanity_floor(&mut snapshot);
            store(&snapshot);
            log::debug!(
                "Feature flags loaded from disk cache ({} verdict(s))",
                snapshot.verdicts.len()
            );
            snapshot
        }
        Err(e) => {
            // Tier 3 — compiled defaults. Not an error condition: a fresh
            // install has no cache, and that is the expected state.
            log::debug!("No usable feature-flag cache ({e}); using compiled defaults");
            let mut snapshot = compiled_defaults();
            apply_sanity_floor(&mut snapshot);
            // Deliberately NOT stored in memory: leaving the slot empty
            // means a cache written later in this session (by a refresh on
            // another task) is still picked up on the next `current()`.
            snapshot
        }
    }
}

/// Attempts a remote refresh and returns the resulting snapshot.
/// **Never fails** — on any transport, HTTP, or parse failure this returns
/// the snapshot [`current`] would have returned, with `meta` updated.
///
/// Silent-failure contract (the headline requirement of this module):
///   * verdicts are left **unchanged** on failure;
///   * `meta.consecutive_failures` is incremented and `meta.last_error` set;
///   * exactly **one** activity-log line is emitted
///     ([`REFRESH_FAILED_MESSAGE`]) — no toast, no banner, no error
///     returned to the frontend;
///   * the raw error detail goes to `log::warn!` and the verbose activity
///     log only.
///
/// Note the scope of that silence: it covers **our inability to reach the
/// server**. Notices attached to genuinely *disabled* features are
/// server-authored user-facing content and are still shown — they live in
/// `verdicts`, not `meta`.
pub async fn refresh(app: &AppHandle) -> FeatureFlagsSnapshot {
    let base = current(app);
    let path = cache_path(app);

    let (snapshot, failure) = resolve_refresh(base, &path).await;

    if let Some(detail) = failure {
        // Raw detail is for us, not for the user.
        log::warn!("Feature availability refresh failed: {detail}");
        emit_verbose_app_log(app, &format!("Feature availability refresh failed: {detail}"));
        // The single permitted user-facing line. Conveys "nothing changed",
        // names nothing, and asks for nothing.
        emit_app_log(app, REFRESH_FAILED_MESSAGE);
    } else {
        log::debug!(
            "Feature availability refreshed from {:?} ({} verdict(s))",
            snapshot.meta.source,
            snapshot.verdicts.len()
        );
    }

    store(&snapshot);
    snapshot
}

/// Whether a feature key is available.
///
/// **Unknown keys are enabled.** Three cases converge on this rule: a
/// build newer than the server's flag set, a fork with no credentials, and
/// a fresh install that has never reached the network. All three must run
/// with every feature on rather than with every feature mysteriously off.
#[must_use]
pub fn is_enabled(snapshot: &FeatureFlagsSnapshot, key: &str) -> bool {
    snapshot
        .verdicts
        .get(key)
        // `is_none_or` reads as the rule itself: no verdict, or a verdict
        // that says enabled.
        .is_none_or(|verdict| verdict.enabled)
}

/// Returns the server-authored notice for a feature key, if any.
///
/// Unlike fetch failures, notices ARE user-facing: they are the mechanism
/// by which a deliberately-disabled feature explains itself.
#[must_use]
pub fn notice_for<'a>(
    snapshot: &'a FeatureFlagsSnapshot,
    key: &str,
) -> Option<&'a FlagNotice> {
    snapshot
        .verdicts
        .get(key)
        .and_then(|verdict| verdict.notice.as_ref())
}

/// Forces every [`UNGATEABLE_KEYS`] entry to `enabled = true`.
///
/// Applied on **every** load path — remote, cache, and defaults — so no
/// ordering of events can leave a hostile or corrupt payload's `false` in
/// place. Any notice attached to such a key is preserved (the server may
/// still have something legitimate to say about it); only the boolean is
/// overridden.
pub fn apply_sanity_floor(snapshot: &mut FeatureFlagsSnapshot) {
    for key in UNGATEABLE_KEYS {
        if let Some(verdict) = snapshot.verdicts.get_mut(*key) {
            if !verdict.enabled {
                log::warn!(
                    "Feature-flag payload tried to disable ungateable key '{key}' — \
                     overriding to enabled (client sanity floor)"
                );
                verdict.enabled = true;
            }
        }
    }
}

/// The compiled-in default snapshot: **no verdicts at all**.
///
/// An empty verdict map plus the missing-key-means-enabled rule in
/// [`is_enabled`] gives "everything on" without enumerating a single flag
/// key — which also means this default can never drift out of step with a
/// flag set that only exists server-side.
#[must_use]
pub fn compiled_defaults() -> FeatureFlagsSnapshot {
    FeatureFlagsSnapshot {
        generated_at: None,
        verdicts: std::collections::HashMap::new(),
        meta: FetchMeta {
            source: FlagSource::Defaults,
            ..FetchMeta::default()
        },
    }
}

// ---------------------------------------------------------------------
// Refresh core (AppHandle-free so it is unit-testable)
// ---------------------------------------------------------------------

/// Performs the network attempt and folds the outcome into a snapshot.
///
/// Split out of [`refresh`] so it can be exercised in unit tests without a
/// Tauri `AppHandle`. Returns `(snapshot, failure_detail)`; the caller owns
/// all logging and activity-log emission.
///
/// A **missing build-time credential is not a failure** — it is the
/// designed-inert state for forks and local dev builds, so it returns the
/// baseline snapshot with `failure_detail == None` and no counter bump.
async fn resolve_refresh(
    base: FeatureFlagsSnapshot,
    cache_path: &Path,
) -> (FeatureFlagsSnapshot, Option<String>) {
    let Some(config) = remote_config() else {
        log::debug!(
            "Feature availability check skipped — no build-time endpoint credentials \
             (this build is intentionally inert; all features remain enabled)"
        );
        return (base, None);
    };

    match fetch_remote(&config).await {
        Ok(mut fresh) => {
            // Preserve nothing from `base.meta` — a successful fetch resets
            // the diagnostics wholesale.
            fresh.meta = FetchMeta {
                source: FlagSource::Remote,
                fetched_at: Some(chrono::Utc::now().to_rfc3339()),
                consecutive_failures: 0,
                last_error: None,
            };
            apply_sanity_floor(&mut fresh);

            // Cache write failures are non-fatal: we still have a good
            // in-memory snapshot for this session.
            if let Err(e) = save_cache(cache_path, &fresh) {
                log::warn!("Failed to write feature-flag cache: {e}");
            }
            (fresh, None)
        }
        Err(detail) => {
            // Silent failure: verdicts untouched, diagnostics updated.
            let mut kept = base;
            kept.meta.consecutive_failures = kept.meta.consecutive_failures.saturating_add(1);
            kept.meta.last_error = Some(detail.clone());
            (kept, Some(detail))
        }
    }
}

// ---------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------

/// Resolved build-time endpoint configuration.
struct RemoteConfig {
    base_url: String,
    app_id: String,
    api_key: String,
}

/// Reads the three build-time constants, returning `None` unless **all**
/// are present and non-empty.
///
/// This is what makes a fork or a local dev build fully functional with
/// **zero network calls and zero errors**: no base URL, no app ID, or no
/// key means the network step is skipped entirely — not attempted and
/// failed. There is no fallback endpoint and no default host.
fn remote_config() -> Option<RemoteConfig> {
    let base_url = INTAPPS_BASE_URL?.trim().trim_end_matches('/').to_string();
    let app_id = INTAPPS_APP_ID?.trim().to_string();
    let api_key = INTAPPS_API_KEY?.trim().to_string();
    if base_url.is_empty() || app_id.is_empty() || api_key.is_empty() {
        return None;
    }
    Some(RemoteConfig {
        base_url,
        app_id,
        api_key,
    })
}

/// The platform token sent to the server, lowercase on the wire.
///
/// A closed vocabulary (matching the closed OS vocabulary in
/// `utils::http_client::os_name`) so server-side targeting rules can match
/// on stable values. Anything else passes through raw rather than being
/// flattened to a sentinel — a server that has never heard of it will
/// simply not match a rule, which is the fail-open outcome we want.
#[must_use]
fn platform_wire_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        other => other,
    }
}

/// Fetches and parses the flag payload.
///
/// The request carries exactly three pieces of information about this
/// install: its own app version, its platform, and its platform version —
/// the same class of data already sent with an update check, and already
/// carried by `full_user_agent()` on this endpoint. No install identifier,
/// account, locale, or settings data is sent.
///
/// The server performs the version / platform / rollout evaluation and
/// returns resolved booleans; the client does no condition evaluation of
/// its own.
async fn fetch_remote(config: &RemoteConfig) -> Result<FeatureFlagsSnapshot, String> {
    // The client-level user-agent produces the `User-Agent` header, so it
    // is not also set per-request. `full_user_agent()` — the
    // platform-bearing variant, not the reduced `APP_USER_AGENT` used for
    // third parties — is correct here and *only* for call sites like this
    // one: MWBM-IntAppsAPI is MeedyaDL's own backend, the single endpoint
    // with a legitimate use for OS/arch/OS-version detail (#1070).
    let client = build_client(
        ClientConfig::with_timeout(REQUEST_TIMEOUT_SECS).user_agent(full_user_agent()),
    )?;

    let url = format!("{}{FEATURES_PATH}", config.base_url);

    let response = client
        .get(&url)
        .query(&[
            ("app_version", env!("CARGO_PKG_VERSION")),
            ("platform", platform_wire_name()),
            ("platform_version", &tauri_plugin_os::version().to_string()),
        ])
        .header("X-App-ID", &config.app_id)
        // The key value is never logged — not here, not in the error paths
        // below, not in the verbose activity log.
        .header("X-API-Key", &config.api_key)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {status} from feature endpoint"));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    parse_envelope(&body)
}

/// Parses the `{"success": bool, "data": { ... }}` response envelope.
///
/// `success: false` is treated as a **failure**, not as an empty result —
/// falling through to the cached/default verdicts is strictly safer than
/// acting on a payload the server itself has disowned.
fn parse_envelope(body: &str) -> Result<FeatureFlagsSnapshot, String> {
    let envelope: ApiEnvelope =
        serde_json::from_str(body).map_err(|e| format!("Failed to parse response JSON: {e}"))?;

    if !envelope.success {
        return Err(format!(
            "Endpoint reported failure{}",
            envelope
                .error
                .as_deref()
                .map(|m| format!(": {m}"))
                .unwrap_or_default()
        ));
    }

    envelope
        .data
        .ok_or_else(|| "Response envelope had success=true but no data".to_string())
}

/// MWBM-IntAppsAPI's standard response envelope.
///
/// Permissive by design (see the forward-compatibility note in
/// `models::feature_flags`): unknown envelope fields are ignored so the
/// server can add signature/pagination/metadata members without breaking
/// already-shipped clients.
#[derive(serde::Deserialize)]
struct ApiEnvelope {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    data: Option<FeatureFlagsSnapshot>,
    /// Optional server-side error message, folded into our internal error
    /// string for the log only — never shown to the user.
    #[serde(default)]
    error: Option<String>,
}

// ---------------------------------------------------------------------
// Sticky disk cache
// ---------------------------------------------------------------------

/// Absolute path of the cache file in the app data directory.
fn cache_path(app: &AppHandle) -> PathBuf {
    platform::get_app_data_dir(app).join(CACHE_FILENAME)
}

/// Writes the snapshot to `path` atomically, plus a SHA-256 companion.
///
/// Takes a `&Path` rather than an `AppHandle` so it is unit-testable
/// against a `tempfile::TempDir`.
///
/// ## The cache deliberately has no TTL, max-age, or expiry
///
/// Do **not** add one. The compiled defaults are all-enabled, so an
/// expiring cache would silently **re-enable** a feature we deliberately
/// switched off the moment the user went offline long enough — turning a
/// deliberate kill switch into a timer. Expiry weakens the switch in
/// exactly the situation it exists for. Staleness is the price, and it is
/// the right price: a stale "off" is recoverable by the next successful
/// fetch, whereas a spontaneous "on" is not recoverable at all.
fn save_cache(path: &Path, snapshot: &FeatureFlagsSnapshot) -> Result<(), String> {
    // Serialise once for the checksum; `atomic_write_json` produces the
    // identical string from the same value, so the digest matches the
    // bytes that land on disk.
    let json = serde_json::to_string_pretty(snapshot)
        .map_err(|e| format!("Failed to serialize {CACHE_CONTEXT}: {e}"))?;

    crate::utils::atomic_write::atomic_write_json(path, snapshot, CACHE_CONTEXT)?;
    write_cache_checksum(path, &json);
    Ok(())
}

/// Reads the snapshot from `path`.
///
/// Mirrors the `settings.json` convention: a checksum mismatch logs a
/// `log::warn!` but the file is **still loaded**. Refusing to load would
/// mean discarding a deliberate "off" instruction because of a stray byte
/// — the opposite of what the sanity floor and the no-expiry rule are
/// protecting. Corruption severe enough to break JSON parsing does fall
/// through to defaults, which is unavoidable.
fn load_cache(path: &Path) -> Result<FeatureFlagsSnapshot, String> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {CACHE_CONTEXT}: {e}"))?;

    verify_cache_checksum(path, &json);

    serde_json::from_str(&json).map_err(|e| format!("Failed to parse {CACHE_CONTEXT}: {e}"))
}

/// Hex SHA-256 of a string. Same helper shape as `config_service`.
fn sha256_hex(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Path of the checksum companion file (`feature-flags-cache.json.sha256`).
fn checksum_path(path: &Path) -> PathBuf {
    path.with_extension("json.sha256")
}

/// Writes the SHA-256 companion. Best-effort: a failure here only costs us
/// integrity signalling on the next read, never the cache itself.
fn write_cache_checksum(path: &Path, json: &str) {
    let checksum_file = checksum_path(path);
    if let Err(e) = std::fs::write(&checksum_file, sha256_hex(json)) {
        log::warn!("Failed to write {CACHE_CONTEXT} checksum: {e}");
    }
}

/// Verifies the cache against its checksum companion, logging a warning on
/// mismatch. Advisory only — the caller loads the file either way.
fn verify_cache_checksum(path: &Path, json: &str) {
    let checksum_file = checksum_path(path);
    let Ok(expected) = std::fs::read_to_string(&checksum_file) else {
        // No companion file (or unreadable): nothing to compare against.
        // Written by an older build, or the write above failed. Not an
        // error — the cache is still usable.
        return;
    };
    if expected.trim() != sha256_hex(json) {
        log::warn!(
            "{CACHE_CONTEXT} integrity check failed: checksum mismatch. \
             Loading anyway (a stale-but-valid 'off' instruction is safer to keep \
             than to discard)."
        );
    }
}

// ---------------------------------------------------------------------
// In-memory store
// ---------------------------------------------------------------------

/// Replaces the process-global snapshot. A poisoned lock is logged and
/// ignored rather than propagated — see [`CURRENT`].
fn store(snapshot: &FeatureFlagsSnapshot) {
    match CURRENT.write() {
        Ok(mut guard) => *guard = Some(snapshot.clone()),
        Err(_) => log::warn!("Feature-flag snapshot lock poisoned — keeping previous value"),
    }
}

/// Test-only reset of the process-global snapshot so cases that touch
/// `CURRENT` don't leak state into one another.
#[cfg(test)]
fn reset_for_test() {
    if let Ok(mut guard) = CURRENT.write() {
        *guard = None;
    }
}

// ---------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::feature_flags::{FlagVerdict, NoticeSeverity};
    use std::collections::HashMap;
    use tempfile::TempDir;

    /// Builds a snapshot with a single verdict for `key`.
    fn snapshot_with(key: &str, enabled: bool) -> FeatureFlagsSnapshot {
        let mut verdicts = HashMap::new();
        verdicts.insert(
            key.to_string(),
            FlagVerdict {
                enabled,
                ..FlagVerdict::default()
            },
        );
        FeatureFlagsSnapshot {
            generated_at: Some("2026-07-27T00:00:00Z".to_string()),
            verdicts,
            meta: FetchMeta::default(),
        }
    }

    // -----------------------------------------------------------------
    // Defaults / fail-open
    // -----------------------------------------------------------------

    /// Compiled defaults carry no verdicts, and every key therefore reads
    /// as enabled.
    #[test]
    fn defaults_are_all_enabled() {
        let snap = compiled_defaults();
        assert!(snap.verdicts.is_empty());
        assert_eq!(snap.meta.source, FlagSource::Defaults);
        for key in ["core.updater", "service.apple-music", "anything.at.all"] {
            assert!(is_enabled(&snap, key), "{key} should be enabled by default");
        }
    }

    /// A key the payload has never heard of is enabled — the rule that
    /// keeps a newer build working against an older flag set.
    #[test]
    fn unknown_key_is_enabled() {
        let snap = snapshot_with("known.key", false);
        assert!(!is_enabled(&snap, "known.key"));
        assert!(is_enabled(&snap, "some.future.key"));
    }

    /// `notice_for` returns the server-authored notice for a disabled
    /// feature, and `None` when there isn't one.
    #[test]
    fn notice_for_returns_server_authored_notice() {
        let mut snap = snapshot_with("service.example", false);
        snap.verdicts.get_mut("service.example").unwrap().notice = Some(FlagNotice {
            message: "Temporarily unavailable".to_string(),
            severity: NoticeSeverity::Maintenance,
            url: None,
        });
        let notice = notice_for(&snap, "service.example").expect("notice present");
        assert_eq!(notice.message, "Temporarily unavailable");
        assert_eq!(notice.severity, NoticeSeverity::Maintenance);
        assert!(notice_for(&snap, "missing.key").is_none());
    }

    // -----------------------------------------------------------------
    // Sanity floor
    // -----------------------------------------------------------------

    /// A payload that tries to disable the fetcher or the updater is
    /// overridden — the containment measure against a compromised admin
    /// account blinding the installed base.
    #[test]
    fn sanity_floor_forces_ungateable_keys() {
        let mut snap = FeatureFlagsSnapshot::default();
        for key in UNGATEABLE_KEYS {
            snap.verdicts.insert(
                (*key).to_string(),
                FlagVerdict {
                    enabled: false,
                    ..FlagVerdict::default()
                },
            );
        }
        // A normal gateable key alongside them must be left alone.
        snap.verdicts.insert(
            "service.example".to_string(),
            FlagVerdict {
                enabled: false,
                ..FlagVerdict::default()
            },
        );

        apply_sanity_floor(&mut snap);

        for key in UNGATEABLE_KEYS {
            assert!(
                is_enabled(&snap, key),
                "{key} must be forced enabled by the sanity floor"
            );
        }
        assert!(
            !is_enabled(&snap, "service.example"),
            "the floor must not re-enable ordinary gateable keys"
        );
    }

    /// The floor preserves any notice attached to an ungateable key —
    /// only the boolean is overridden.
    #[test]
    fn sanity_floor_preserves_notice_on_ungateable_key() {
        let key = UNGATEABLE_KEYS[0];
        let mut snap = snapshot_with(key, false);
        snap.verdicts.get_mut(key).unwrap().notice = Some(FlagNotice {
            message: "Scheduled maintenance".to_string(),
            severity: NoticeSeverity::Maintenance,
            url: None,
        });

        apply_sanity_floor(&mut snap);

        assert!(is_enabled(&snap, key));
        assert!(notice_for(&snap, key).is_some());
    }

    /// The floor is a no-op on a payload that doesn't mention the
    /// ungateable keys at all (they're already enabled by the missing-key
    /// rule).
    #[test]
    fn sanity_floor_is_a_noop_when_keys_absent() {
        let mut snap = snapshot_with("service.example", false);
        let before = snap.clone();
        apply_sanity_floor(&mut snap);
        assert_eq!(snap, before);
    }

    // -----------------------------------------------------------------
    // Envelope parsing
    // -----------------------------------------------------------------

    /// `success: true` yields the payload's verdicts.
    #[test]
    fn envelope_success_true_yields_data() {
        let body = r#"{
            "success": true,
            "data": {
                "generated_at": "2026-07-27T12:00:00Z",
                "verdicts": {
                    "service.example": { "enabled": false, "label": "Example" }
                }
            }
        }"#;
        let snap = parse_envelope(body).expect("success envelope should parse");
        assert_eq!(snap.generated_at.as_deref(), Some("2026-07-27T12:00:00Z"));
        assert!(!is_enabled(&snap, "service.example"));
    }

    /// `success: false` is a failure, so the caller falls back to cache /
    /// defaults rather than acting on a disowned payload.
    #[test]
    fn envelope_success_false_is_a_failure() {
        let body = r#"{"success": false, "error": "temporarily unavailable", "data": null}"#;
        let err = parse_envelope(body).expect_err("success=false must be an error");
        assert!(err.contains("temporarily unavailable"), "got: {err}");
    }

    /// `success: true` with no `data` member is also a failure.
    #[test]
    fn envelope_success_true_without_data_is_a_failure() {
        let err = parse_envelope(r#"{"success": true}"#)
            .expect_err("missing data must be an error");
        assert!(err.contains("no data"), "got: {err}");
    }

    /// Malformed JSON is a failure, never a panic.
    #[test]
    fn envelope_malformed_json_is_a_failure() {
        assert!(parse_envelope("not json at all").is_err());
    }

    // -----------------------------------------------------------------
    // Cache
    // -----------------------------------------------------------------

    /// Round trip through the on-disk cache, including the SHA-256
    /// companion file.
    #[test]
    fn cache_round_trips_through_disk() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CACHE_FILENAME);

        let original = snapshot_with("service.example", false);
        save_cache(&path, &original).expect("cache write should succeed");

        assert!(path.exists(), "cache file should exist");
        assert!(
            checksum_path(&path).exists(),
            "SHA-256 companion should be written alongside the cache"
        );

        let loaded = load_cache(&path).expect("cache read should succeed");
        assert_eq!(loaded.verdicts, original.verdicts);
        assert_eq!(loaded.generated_at, original.generated_at);
    }

    /// A tampered / corrupted cache still loads (with a warning) — see the
    /// rationale on `verify_cache_checksum`.
    #[test]
    fn checksum_mismatch_still_loads() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CACHE_FILENAME);

        save_cache(&path, &snapshot_with("service.example", false)).unwrap();
        // Overwrite the companion with a digest that cannot match.
        std::fs::write(checksum_path(&path), "0".repeat(64)).unwrap();

        let loaded = load_cache(&path)
            .expect("a checksum mismatch must NOT prevent the cache from loading");
        assert!(!is_enabled(&loaded, "service.example"));
    }

    /// A cache written without a companion file (older build, or a failed
    /// checksum write) still loads.
    #[test]
    fn missing_checksum_companion_still_loads() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CACHE_FILENAME);

        save_cache(&path, &snapshot_with("service.example", false)).unwrap();
        std::fs::remove_file(checksum_path(&path)).unwrap();

        assert!(load_cache(&path).is_ok());
    }

    /// A missing cache file is a plain error the caller turns into
    /// "compiled defaults" — never a panic.
    #[test]
    fn load_cache_on_missing_file_is_an_error_not_a_panic() {
        let dir = TempDir::new().unwrap();
        assert!(load_cache(&dir.path().join("nope.json")).is_err());
    }

    // -----------------------------------------------------------------
    // Refresh
    // -----------------------------------------------------------------

    /// Without build-time credentials the refresh makes **no network call**
    /// and returns the baseline unchanged — no error, no failure counter
    /// bump, no cache write. This is the state every fork and local dev
    /// build runs in.
    ///
    /// (Unit tests never carry the `INTAPPS_*` build-time values, so
    /// `remote_config()` is `None` here by construction.)
    #[tokio::test]
    async fn refresh_with_absent_credentials_returns_defaults_and_never_errors() {
        reset_for_test();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CACHE_FILENAME);

        let (snapshot, failure) = resolve_refresh(compiled_defaults(), &path).await;

        assert!(
            failure.is_none(),
            "an intentionally-inert build must not report a failure"
        );
        assert_eq!(snapshot.meta.consecutive_failures, 0);
        assert!(snapshot.meta.last_error.is_none());
        assert_eq!(snapshot.meta.source, FlagSource::Defaults);
        assert!(is_enabled(&snapshot, "anything"));
        assert!(
            !path.exists(),
            "the skipped path must not write a cache file"
        );
    }

    /// The same skip path must leave a pre-existing snapshot's verdicts
    /// completely untouched.
    #[tokio::test]
    async fn refresh_without_credentials_preserves_existing_verdicts() {
        reset_for_test();
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CACHE_FILENAME);

        let base = snapshot_with("service.example", false);
        let (snapshot, failure) = resolve_refresh(base.clone(), &path).await;

        assert!(failure.is_none());
        assert_eq!(snapshot.verdicts, base.verdicts);
        assert!(!is_enabled(&snapshot, "service.example"));
    }

    /// `remote_config()` is `None` in any build that lacks the compile-time
    /// values — the guard that keeps forks entirely offline.
    #[test]
    fn remote_config_is_none_without_build_time_values() {
        if INTAPPS_BASE_URL.is_none() || INTAPPS_APP_ID.is_none() || INTAPPS_API_KEY.is_none() {
            assert!(remote_config().is_none());
        }
    }

    /// The platform token is one of the three documented lowercase values
    /// on every platform MeedyaDL ships to.
    #[test]
    fn platform_wire_name_is_lowercase_and_closed() {
        let name = platform_wire_name();
        assert_eq!(name, name.to_lowercase());
        if matches!(std::env::consts::OS, "macos" | "windows" | "linux") {
            assert!(matches!(name, "macos" | "windows" | "linux"));
        }
    }
}
