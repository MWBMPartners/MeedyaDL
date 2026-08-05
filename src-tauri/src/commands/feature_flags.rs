// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Remote feature-flag IPC commands.
// ===================================
//
// Thin wrappers over `services::feature_flag_service`. The service layer's
// resolution chain is infallible by design (it always answers, worst case
// with all-enabled compiled defaults), so neither command can fail on
// resolution grounds — the `Result<_, String>` return type exists because
// that is the Tauri command shape, and because `refresh_feature_flags`
// legitimately rejects calls that breach its rate limit.
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command            | TypeScript Function        |
// |-------------------------|----------------------------|
// | get_feature_flags       | getFeatureFlags()          |
// | refresh_feature_flags   | refreshFeatureFlags()      |
//
// No frontend caller exists yet — this package is backend-only. The UI
// (notice rendering) and the enforcement call sites land separately.
//
// Reference: https://v2.tauri.app/develop/calling-rust/

use tauri::AppHandle;

use crate::models::feature_flags::FeatureFlagsSnapshot;
use crate::services::feature_flag_service;
use crate::utils::rate_limiter::check_rate_limit;

/// Returns the currently-resolved feature-flag snapshot **without making a
/// network call**.
///
/// Reads the process-global snapshot, falling back to the sticky disk
/// cache and then to compiled defaults. Cheap enough to call on every
/// render pass that needs it; the frontend should treat it as the
/// authoritative read and use [`refresh_feature_flags`] only when it
/// actually wants to go out to the network.
///
/// **Frontend caller:** `getFeatureFlags()` in `src/lib/tauri-commands.ts`
///
/// # Returns
/// * `Ok(FeatureFlagsSnapshot)` — always. The resolution chain has no
///   failure mode; see `services::feature_flag_service`.
#[tauri::command]
pub async fn get_feature_flags(app: AppHandle) -> Result<FeatureFlagsSnapshot, String> {
    Ok(feature_flag_service::current(&app))
}

/// Attempts a remote refresh, then returns the resulting snapshot.
///
/// Rate-limited to **1 call per minute**, matching `check_all_updates`:
/// this is a background-shaped operation and there is no user-visible
/// benefit to hammering it. A limit breach returns `Err` — that is an IPC
/// guard against a buggy or compromised frontend, not a resolution
/// failure.
///
/// On any transport / HTTP / parse failure the underlying service keeps
/// the previous verdicts and emits a single activity-log line; it does not
/// surface an error here. The frontend therefore never needs to handle
/// (or display) a fetch failure — by design.
///
/// **Frontend caller:** `refreshFeatureFlags()` in
/// `src/lib/tauri-commands.ts`
///
/// # Returns
/// * `Ok(FeatureFlagsSnapshot)` — the refreshed snapshot, or the previous
///   one unchanged if the network attempt did not succeed.
/// * `Err(String)` — only when the rate limit rejects the call.
#[tauri::command]
pub async fn refresh_feature_flags(app: AppHandle) -> Result<FeatureFlagsSnapshot, String> {
    // Same window as `check_all_updates` (1 call / 60 s).
    check_rate_limit("refresh_feature_flags", 1, 60)?;
    Ok(feature_flag_service::refresh(&app).await)
}
