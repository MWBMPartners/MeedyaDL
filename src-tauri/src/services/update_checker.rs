// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Update checker service.
// Checks for new versions of all application components: GAMDL (via PyPI),
// the app itself (via GitHub Releases), Python runtime, and external tools.
// Includes a compatibility gate so only known-compatible GAMDL versions
// are offered for upgrade.
//
// ## Architecture Overview
//
// This service is invoked periodically (on app launch or user request) to
// check whether any component has a newer version available. It runs all
// checks concurrently and aggregates results into an UpdateCheckResult.
//
// ```
// check_all_updates()
//     |
//     +-- check_gamdl_update()   --> PyPI JSON API (https://pypi.org/pypi/gamdl/json)
//     |                               Compares installed version (pip show) with PyPI latest
//     |
//     +-- check_app_update()     --> GitHub Releases API (repos/MWBMPartners/MeedyaDL/releases)
//     |                               Compares running version with latest release tag
//     |                               Supports pre-release channel (releases?per_page=5)
//     |
//     +-- check_python_update()  --> Local comparison against python_manager::PYTHON_VERSION
//                                     Compares installed binary version with configured target
// ```
//
// ## Version Comparison
//
// Versions are compared as semver tuples (major, minor, patch). The `is_newer()`
// function parses "X.Y.Z" strings into (u32, u32, u32) and uses tuple comparison.
//
// ## Compatibility Gating
//
// GAMDL updates are subject to a compatibility check (`is_gamdl_compatible()`).
// This prevents the user from upgrading to a GAMDL version that may have changed
// its CLI interface in incompatible ways. The range [MIN_COMPATIBLE_GAMDL,
// MAX_COMPATIBLE_GAMDL] defines the known-compatible window.
//
// ## References
//
// - PyPI JSON API: https://pypi.org/pypi/{package}/json
//   Response format: { "info": { "version": "X.Y.Z", ... }, "releases": { ... } }
// - GitHub Releases API: https://docs.github.com/en/rest/releases/releases#get-the-latest-release
// - Reqwest HTTP client: https://docs.rs/reqwest/latest/reqwest/
// - Chrono for timestamps: https://docs.rs/chrono/latest/chrono/

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

// gamdl_service: provides get_gamdl_version() and check_latest_gamdl_version() for GAMDL update checks.
// python_manager: provides get_installed_python_version() and get_target_python_version() for Python update checks.
use crate::services::{gamdl_service, python_manager};
// platform: provides get_python_dir() for resolving the Python installation directory.
use crate::utils::platform;

// ============================================================
// Update status model
// ============================================================

/// Represents the update status of a single component.
///
/// This struct is serialized to JSON and sent to the frontend via a Tauri command.
/// The frontend renders an update card for each ComponentUpdate with the
/// current version, latest version, and an "Update" button if applicable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentUpdate {
    /// Human-readable component name (e.g., "GAMDL", "Python Runtime", "MeedyaDL")
    pub name: String,
    /// Currently installed version (None if not installed).
    /// For GAMDL: from `pip show gamdl`. For Python: from `python --version`.
    /// For the app: from tauri.conf.json package version.
    pub current_version: Option<String>,
    /// Latest available version (None if the check failed or no releases exist).
    /// For GAMDL: from PyPI JSON API. For the app: from GitHub Releases.
    /// For Python: from python_manager::PYTHON_VERSION constant.
    pub latest_version: Option<String>,
    /// Whether an update is available (latest > current via semver comparison).
    /// True if not installed and a version is available on the remote source.
    pub update_available: bool,
    /// Whether this update is compatible with the current app version.
    /// For GAMDL: checked via is_gamdl_compatible() range gate.
    /// For app and Python: always true (updates are self-compatible).
    pub is_compatible: bool,
    /// Human-readable description of the update (e.g., release notes excerpt).
    /// For app updates: truncated first 200 chars of the GitHub release body.
    pub description: Option<String>,
    /// URL to the release page for the user to review before updating.
    /// For GAMDL: PyPI project page. For app: GitHub release page.
    pub release_url: Option<String>,
    /// Full release notes body from GitHub Releases (untruncated).
    /// Only populated for app updates. Used by the Updates page to display
    /// the complete changelog. `description` is the truncated 200-char excerpt.
    pub release_body: Option<String>,
    /// Whether this release is a pre-release (beta/RC).
    /// Only relevant for app updates (GitHub Releases `prerelease` field).
    pub is_prerelease: bool,
    /// Git tag name for this release (e.g., "v0.3.7").
    /// Used by the frontend to construct the download URL for the Tauri updater.
    pub tag_name: Option<String>,
    /// Whether this update is a rollback from a pre-release to a stable version.
    /// When true, the frontend labels the action as "Roll Back to Stable" instead
    /// of "Download & Install". Set by check_app_update() when the user has
    /// `prefer_stable_rollback` enabled and is running a pre-release version.
    pub is_rollback: bool,
}

/// Combined update status for all components.
///
/// Returned by check_all_updates() and sent to the frontend as a single response.
/// The frontend uses `has_updates` to show/hide an update badge in the toolbar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    /// Timestamp of the check in ISO 8601 format (e.g., "2026-02-10T12:00:00Z").
    /// Used by the frontend to display "Last checked: X minutes ago".
    pub checked_at: String,
    /// Whether any compatible updates are available (quick check for badge display).
    /// True if any component has update_available && is_compatible.
    pub has_updates: bool,
    /// Per-component update status (one entry per checked component).
    pub components: Vec<ComponentUpdate>,
    /// Non-fatal errors that occurred during individual checks.
    /// For example, a network timeout on PyPI doesn't prevent checking GitHub.
    pub errors: Vec<String>,
}

// ============================================================
// GAMDL version compatibility
// ============================================================

/// Minimum GAMDL version known to be compatible with this app version.
/// Versions below this may have different CLI argument formats or missing features.
/// When GAMDL makes a breaking CLI change, update this to exclude old versions.
/// For example, GAMDL 2.0.0 introduced the current CLI argument format.
const MIN_COMPATIBLE_GAMDL: &str = "2.0.0";

/// Maximum GAMDL version known to be compatible (inclusive).
/// Set to a deliberately high value (99.99.99) to allow all future patch and
/// minor releases by default. Update this to a specific version only when a
/// known-incompatible GAMDL release is published (e.g., a major version bump
/// that changes CLI argument names or output format).
const MAX_COMPATIBLE_GAMDL: &str = "99.99.99";

/// Checks whether a GAMDL version is compatible with this app version.
///
/// This is a simple semver range check that prevents the user from
/// upgrading to a GAMDL version that may break the CLI interface.
///
/// # Arguments
/// * `version` - The GAMDL version string to check (e.g., "2.8.4")
fn is_gamdl_compatible(version: &str) -> bool {
    // Inner closure that parses "X.Y.Z" into (major, minor, patch).
    // Handles both "X.Y.Z" (3 parts) and "X.Y" (2 parts, patch defaults to 0).
    // Returns None for unparseable strings (e.g., "invalid", "1", "1.x.2").
    let parse = |v: &str| -> Option<(u32, u32, u32)> {
        let parts: Vec<&str> = v.split('.').collect();
        if parts.len() >= 3 {
            Some((
                parts[0].parse().ok()?,
                parts[1].parse().ok()?,
                parts[2].parse().ok()?,
            ))
        } else if parts.len() == 2 {
            Some((parts[0].parse().ok()?, parts[1].parse().ok()?, 0))
        } else {
            None
        }
    };

    // Parse all three versions; return false (incompatible) if any fails to parse
    let Some(current) = parse(version) else {
        return false;
    };
    let Some(min) = parse(MIN_COMPATIBLE_GAMDL) else {
        return false;
    };
    let Some(max) = parse(MAX_COMPATIBLE_GAMDL) else {
        return false;
    };

    // Check that the version falls within the inclusive range [min, max].
    // Rust's tuple comparison is lexicographic, which matches semver ordering:
    // (2, 8, 4) >= (2, 0, 0) && (2, 8, 4) <= (99, 99, 99)
    current >= min && current <= max
}

/// Strips the pre-release suffix from a version string.
///
/// Pre-release versions follow the format `X.Y.Z-label.N` (e.g., "0.4.0-alpha.1").
/// This function returns just the base version ("0.4.0") by splitting on the first
/// hyphen. If there's no hyphen, the string is returned unchanged.
///
/// Also strips a leading `v` prefix (e.g., "v0.4.0-alpha.1" → "0.4.0").
fn strip_prerelease(v: &str) -> &str {
    let stripped = v.trim_start_matches('v');
    stripped.split('-').next().unwrap_or(stripped)
}

/// Returns true if the version string contains a pre-release suffix.
///
/// Checks for a hyphen after stripping the `v` prefix (e.g., "0.4.0-alpha.1"
/// contains `-alpha.1`, so it's a pre-release; "0.3.22" has no hyphen, so it's stable).
fn is_prerelease_version(v: &str) -> bool {
    let stripped = v.trim_start_matches('v');
    stripped.contains('-')
}

/// Compares two semver version strings and returns true if `latest` is strictly newer than `current`.
///
/// Handles pre-release suffixes correctly by stripping them before numeric comparison.
/// When base versions are equal, a stable release is considered "newer" than a pre-release
/// (e.g., "0.4.0" is newer than "0.4.0-alpha.1"). This ensures that users on pre-release
/// versions see the stable release as an available update.
///
/// Examples:
/// - is_newer("1.0.0", "1.0.1") => true  (patch bump)
/// - is_newer("1.0.0", "1.0.0") => false (same version)
/// - is_newer("2.0.0", "1.9.9") => false (downgrade)
/// - is_newer("0.4.0-alpha.1", "0.4.0") => true  (stable beats pre-release at same base)
/// - is_newer("0.4.0", "0.4.0-alpha.1") => false (pre-release is not newer than stable)
fn is_newer(current: &str, latest: &str) -> bool {
    // Parse version string into (major, minor, patch) tuple after stripping
    // the pre-release suffix. This fixes the previous bug where "0.4.0-alpha.1"
    // had its patch part "0-alpha" fail parse::<u32>() and silently default to 0.
    let parse = |v: &str| -> (u32, u32, u32) {
        let base = strip_prerelease(v);
        let parts: Vec<&str> = base.split('.').collect();
        let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };

    let c = parse(current);
    let l = parse(latest);

    // If base versions differ, standard tuple comparison applies
    if l != c {
        return l > c;
    }

    // Base versions are equal — stable is "newer" than pre-release.
    // This handles the case where a user is on "0.4.0-alpha.1" and the
    // latest stable release is "0.4.0": the stable release should be offered.
    let current_is_pre = is_prerelease_version(current);
    let latest_is_pre = is_prerelease_version(latest);

    // "0.4.0-alpha.1" (pre) vs "0.4.0" (stable) => stable is newer
    // "0.4.0" (stable) vs "0.4.0-alpha.1" (pre) => pre is not newer
    // "0.4.0-alpha.1" vs "0.4.0-beta.1" => not newer (we don't rank pre-release labels)
    current_is_pre && !latest_is_pre
}

// ============================================================
// Update check functions
// ============================================================

/// Checks for updates to all application components.
///
/// Runs all checks concurrently (GAMDL, app, Python) and returns
/// a combined result. Non-fatal errors are collected rather than
/// causing the entire check to fail.
///
/// # Arguments
/// * `app` - Tauri app handle for version info and path resolution
/// * `check_pre_releases` - Whether to include pre-release versions when
///   checking for app updates. When true, queries all recent GitHub releases
///   (including betas/RCs); when false, only checks the latest stable release.
/// * `prefer_stable_rollback` - When true and the current version is a pre-release,
///   forces the app update check to query stable releases only and offers the latest
///   stable as a rollback target, even if it's numerically older.
pub async fn check_all_updates(
    app: &AppHandle,
    check_pre_releases: bool,
    prefer_stable_rollback: bool,
) -> UpdateCheckResult {
    let mut components = Vec::new();
    let mut errors = Vec::new();

    // Check GAMDL updates via PyPI JSON API.
    // This is the most important check since GAMDL receives frequent updates.
    match check_gamdl_update(app).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("GAMDL check failed: {}", e)),
    }

    // Check for app self-updates via GitHub Releases API.
    // Compares the running app version against the latest GitHub release tag.
    // When check_pre_releases is true, includes beta/RC releases in the check.
    // When prefer_stable_rollback is true, forces stable-only check for rollback.
    match check_app_update(app, check_pre_releases, prefer_stable_rollback).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("App update check failed: {}", e)),
    }

    // Check Python runtime update by comparing the installed version
    // against the target version defined in python_manager.rs constants.
    match check_python_update(app).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("Python check failed: {}", e)),
    }

    // Aggregate: an update is "available" only if it's both newer AND compatible.
    // This prevents the UI from showing incompatible GAMDL versions as available.
    let has_updates = components.iter().any(|c| c.update_available && c.is_compatible);

    UpdateCheckResult {
        checked_at: chrono::Utc::now().to_rfc3339(),
        has_updates,
        components,
        errors,
    }
}

/// Checks for GAMDL updates by comparing the installed version with PyPI.
///
/// # Returns
/// A `ComponentUpdate` with the current and latest GAMDL versions.
async fn check_gamdl_update(app: &AppHandle) -> Result<ComponentUpdate, String> {
    // Get the currently installed GAMDL version via `pip show gamdl`.
    // Returns None if GAMDL is not installed (Python not found, or package not installed).
    let current = gamdl_service::get_gamdl_version(app)
        .await
        .unwrap_or(None);

    // Get the latest version from PyPI JSON API.
    // Queries https://pypi.org/pypi/gamdl/json and extracts info.version.
    // Returns None if the request failed (network error, PyPI down, etc.).
    let latest = gamdl_service::check_latest_gamdl_version()
        .await
        .ok();

    // Determine if an update is available:
    // - If both current and latest are known: compare versions (latest > current)
    // - If only latest is known (not installed): treat as "update available"
    // - Otherwise: no update available
    let update_available = match (&current, &latest) {
        (Some(c), Some(l)) => is_newer(c, l),
        (None, Some(_)) => true, // Not installed = "update" available (install prompted)
        _ => false,
    };

    // Apply the compatibility gate: only offer the update if the latest version
    // falls within [MIN_COMPATIBLE_GAMDL, MAX_COMPATIBLE_GAMDL].
    // This prevents upgrading to a GAMDL version with incompatible CLI changes.
    let is_compatible = latest
        .as_ref()
        .map(|v| is_gamdl_compatible(v))
        .unwrap_or(false);

    Ok(ComponentUpdate {
        name: "GAMDL".to_string(),
        current_version: current,
        latest_version: latest.clone(),
        update_available,
        is_compatible,
        description: if update_available {
            Some("New GAMDL version available on PyPI".to_string())
        } else {
            None
        },
        release_url: latest.map(|v| format!("https://pypi.org/project/gamdl/{}/", v)),
        release_body: None,
        // GAMDL updates are from PyPI, not GitHub Releases — no pre-release concept
        is_prerelease: false,
        tag_name: None,
        // GAMDL updates are never rollbacks (no pre-release concept)
        is_rollback: false,
    })
}

/// Checks for app self-updates by querying GitHub Releases.
///
/// Compares the running app version (from tauri.conf.json) with the
/// latest GitHub release tag. When `check_pre_releases` is true, queries
/// all recent releases (including betas/RCs); otherwise only checks the
/// latest stable release.
///
/// When `prefer_stable_rollback` is true and the current version is a pre-release,
/// forces a stable-only query regardless of `check_pre_releases`. If a stable release
/// exists, it's offered as a rollback target (even if numerically older), with
/// `is_rollback: true` to let the frontend render a "Roll Back to Stable" action.
///
/// # Arguments
/// * `app` - Tauri app handle for reading the current app version
/// * `check_pre_releases` - Whether to include pre-release versions.
///   When true: queries `releases?per_page=5` and takes the newest (which may be a pre-release).
///   When false: queries `releases/latest` (GitHub automatically excludes pre-releases).
/// * `prefer_stable_rollback` - When true and current version is pre-release,
///   overrides `check_pre_releases` to force stable-only mode and marks the result
///   as a rollback.
async fn check_app_update(
    app: &AppHandle,
    check_pre_releases: bool,
    prefer_stable_rollback: bool,
) -> Result<ComponentUpdate, String> {
    // Get the current app version from Tauri's package info.
    // This reads the version from tauri.conf.json, set at build time.
    let current_version = app.package_info().version.to_string();

    // Determine if the current version is a pre-release (e.g., "0.4.0-alpha.1").
    // This drives the rollback logic below.
    let current_is_prerelease = is_prerelease_version(&current_version);

    // Rollback mode: when the user is on a pre-release and has opted into stable
    // rollback, force the query to stable-only regardless of check_pre_releases.
    // This ensures we get the latest stable release as the rollback target.
    let rollback_mode = current_is_prerelease && prefer_stable_rollback;

    // Choose the GitHub API endpoint based on pre-release preference.
    // - Stable only: `releases/latest` returns a single release object (excludes pre-releases)
    // - Include pre-releases: `releases?per_page=5` returns an array sorted newest-first
    // - Rollback mode: always queries stable-only, overriding check_pre_releases
    let (url, is_list) = if rollback_mode || !check_pre_releases {
        (
            "https://api.github.com/repos/MWBMPartners/MeedyaDL/releases/latest",
            false,
        )
    } else {
        (
            "https://api.github.com/repos/MWBMPartners/MeedyaDL/releases?per_page=5",
            true,
        )
    };

    // Query the GitHub Releases API.
    // Ref: https://docs.github.com/en/rest/releases/releases
    // Required headers:
    // - User-Agent: GitHub API requires a UA string (can be anything)
    // - Accept: Request v3 JSON format
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", "meedyadl")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if !response.status().is_success() {
        // 404 means no releases have been published yet — not an error condition.
        // This is expected for new repositories that haven't made their first release.
        if response.status().as_u16() == 404 {
            return Ok(ComponentUpdate {
                name: "MeedyaDL".to_string(),
                current_version: Some(current_version),
                latest_version: None,
                update_available: false,
                is_compatible: true,
                description: None,
                release_url: None,
                release_body: None,
                is_prerelease: false,
                tag_name: None,
                is_rollback: false,
            });
        }
        return Err(format!("GitHub API returned HTTP {}", response.status()));
    }

    // Parse the JSON response.
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))?;

    // Extract the release object: either the single response (stable mode)
    // or the first item from the array (pre-release mode, newest first).
    let release = if is_list {
        let releases = json
            .as_array()
            .ok_or("GitHub API returned unexpected format (expected array)")?;
        if releases.is_empty() {
            return Ok(ComponentUpdate {
                name: "MeedyaDL".to_string(),
                current_version: Some(current_version),
                latest_version: None,
                update_available: false,
                is_compatible: true,
                description: None,
                release_url: None,
                release_body: None,
                is_prerelease: false,
                tag_name: None,
                is_rollback: false,
            });
        }
        // Index 0 is the newest release (may be a pre-release)
        &releases[0]
    } else {
        // Stable mode: response is a single release object
        &json
    };

    // Extract the tag name (e.g., "v0.3.7") and strip the "v" prefix
    // to get a bare semver string for comparison with the current version.
    let raw_tag = release["tag_name"].as_str().unwrap_or("");
    let tag = raw_tag.trim_start_matches('v').to_string();

    // Extract the release page URL for the "View Release" button in the UI
    let html_url = release["html_url"].as_str().map(|s| s.to_string());
    // Extract the full release notes body for the Updates page (untruncated).
    let full_body = release["body"].as_str().map(|s| s.to_string());
    // Extract and truncate the release notes for display in the update card.
    // Long release notes are cut to 200 characters to keep the UI compact.
    let body = release["body"].as_str().map(|s| {
        if s.len() > 200 {
            format!("{}...", &s[..200])
        } else {
            s.to_string()
        }
    });
    // Extract the pre-release flag from the GitHub release metadata.
    // This is `true` for releases marked as pre-release on GitHub.
    let is_prerelease = release["prerelease"].as_bool().unwrap_or(false);

    // Determine if an update is available, with special handling for rollback mode.
    //
    // In rollback mode: any stable release is a valid target, even if numerically older.
    // For example, current "0.4.0-alpha.1" → latest stable "0.3.22" is a valid rollback.
    // We also check is_newer() to cover the case where the stable release is also
    // numerically newer (e.g., "0.4.0-alpha.1" → "0.4.0").
    //
    // In normal mode: standard is_newer() comparison (now handles pre-release suffixes).
    let (update_available, is_rollback) = if tag.is_empty() {
        (false, false)
    } else if rollback_mode {
        // In rollback mode, offer the stable release if:
        // 1. It's genuinely newer (is_newer handles stable > pre-release at same base), OR
        // 2. It's a valid rollback target (current is pre-release, target is stable)
        let newer = is_newer(&current_version, &tag);
        let rollback_target = current_is_prerelease && !is_prerelease_version(&tag);
        // It's a "rollback" (not a standard upgrade) when the base version is lower or
        // equal — i.e., not a standard version bump. For example:
        //   "0.4.0-alpha.1" → "0.3.22" = rollback (older base)
        //   "0.4.0-alpha.1" → "0.4.0"  = rollback (same base, stable vs pre-release)
        //   "0.4.0-alpha.1" → "0.5.0"  = NOT a rollback (newer base)
        let base_current = strip_prerelease(&current_version);
        let base_target = strip_prerelease(&tag);
        let is_base_upgrade = is_newer(base_current, base_target);
        (newer || rollback_target, rollback_target && !is_base_upgrade)
    } else {
        (is_newer(&current_version, &tag), false)
    };

    Ok(ComponentUpdate {
        name: "MeedyaDL".to_string(),
        current_version: Some(current_version),
        latest_version: if tag.is_empty() { None } else { Some(tag) },
        update_available,
        // App updates are always "compatible" — the new version replaces the old one entirely.
        // Unlike GAMDL (which has a CLI interface contract), the app is self-contained.
        is_compatible: true,
        description: body,
        release_url: html_url,
        release_body: full_body,
        is_prerelease,
        // Store the raw tag (e.g., "v0.3.7") for use by the Tauri updater
        // when constructing the download URL for a specific release.
        tag_name: if raw_tag.is_empty() {
            None
        } else {
            Some(raw_tag.to_string())
        },
        // Set by the rollback logic above: true when offering a stable release
        // as a rollback target to a user on a pre-release version.
        is_rollback,
    })
}

/// Checks for Python runtime updates by comparing with python-build-standalone.
///
/// Compares the installed Python version with the version constant in
/// `python_manager.rs`. In the future, this could also check GitHub
/// for newer python-build-standalone releases.
async fn check_python_update(app: &AppHandle) -> Result<ComponentUpdate, String> {
    // Get the installed Python version by running the binary with --version.
    // Returns None if Python is not installed.
    let python_dir = platform::get_python_dir(app);
    let current = python_manager::get_installed_python_version(&python_dir).await;

    // The "target" version is the one defined in python_manager::PYTHON_VERSION.
    // This is a local comparison — we don't query any remote API for Python.
    // When we update PYTHON_VERSION in the code, users will see an update available
    // next time they check. The actual update requires reinstalling Python.
    let target = python_manager::get_target_python_version();

    // Only show an update if Python is installed AND the installed version is
    // older than the target. If Python is not installed, the setup wizard
    // handles installation — we don't show it as an "update".
    let update_available = match &current {
        Some(c) => is_newer(c, target),
        None => false, // Can't update what's not installed
    };

    Ok(ComponentUpdate {
        name: "Python Runtime".to_string(),
        current_version: current,
        latest_version: Some(target.to_string()),
        update_available,
        // Python updates are always compatible since we control the version
        // and test it with GAMDL before shipping.
        is_compatible: true,
        description: if update_available {
            Some(format!("Python {} available (portable runtime)", target))
        } else {
            None
        },
        // Link to the python-build-standalone releases page for user reference
        release_url: Some(
            "https://github.com/indygreg/python-build-standalone/releases".to_string(),
        ),
        release_body: None,
        // Python updates are local version comparisons, not GitHub Releases
        is_prerelease: false,
        tag_name: None,
        // Python updates are never rollbacks
        is_rollback: false,
    })
}

// ============================================================
// Unit tests for version comparison and compatibility checking
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that strip_prerelease() correctly removes pre-release suffixes
    /// and v-prefixes from version strings.
    #[test]
    fn test_strip_prerelease() {
        // Standard pre-release suffix
        assert_eq!(strip_prerelease("0.4.0-alpha.1"), "0.4.0");
        assert_eq!(strip_prerelease("0.4.0-beta.2"), "0.4.0");
        assert_eq!(strip_prerelease("0.4.0-rc.1"), "0.4.0");
        // No suffix: returned unchanged
        assert_eq!(strip_prerelease("0.3.22"), "0.3.22");
        // With v prefix
        assert_eq!(strip_prerelease("v0.4.0-alpha.1"), "0.4.0");
        assert_eq!(strip_prerelease("v0.3.22"), "0.3.22");
    }

    /// Tests that is_prerelease_version() correctly identifies pre-release suffixes.
    #[test]
    fn test_is_prerelease_version() {
        // Pre-release versions (contain hyphen after v-strip)
        assert!(is_prerelease_version("0.4.0-alpha.1"));
        assert!(is_prerelease_version("0.4.0-beta.2"));
        assert!(is_prerelease_version("0.4.0-rc.1"));
        assert!(is_prerelease_version("v0.4.0-alpha.1"));
        // Stable versions (no hyphen)
        assert!(!is_prerelease_version("0.3.22"));
        assert!(!is_prerelease_version("v0.3.22"));
        assert!(!is_prerelease_version("1.0.0"));
    }

    /// Tests that is_newer() correctly handles all semver comparison cases:
    /// patch bumps, minor bumps, major bumps, equal versions, downgrades,
    /// and pre-release vs stable at the same base version.
    #[test]
    fn test_is_newer() {
        // Patch bump: 1.0.1 is newer than 1.0.0
        assert!(is_newer("1.0.0", "1.0.1"));
        // Minor bump: 1.1.0 is newer than 1.0.0
        assert!(is_newer("1.0.0", "1.1.0"));
        // Major bump: 2.0.0 is newer than 1.0.0
        assert!(is_newer("1.0.0", "2.0.0"));
        // Same version: not newer
        assert!(!is_newer("1.0.0", "1.0.0"));
        // Downgrade: 1.0.0 is not newer than 2.0.0
        assert!(!is_newer("2.0.0", "1.0.0"));
    }

    /// Tests is_newer() with pre-release version strings.
    /// This is the critical fix: "0.4.0-alpha.1" previously misparsed
    /// because "0-alpha" failed parse::<u32>() and defaulted to 0.
    #[test]
    fn test_is_newer_prerelease() {
        // Stable is newer than pre-release at the same base version
        assert!(is_newer("0.4.0-alpha.1", "0.4.0"));
        // Pre-release is NOT newer than stable at the same base
        assert!(!is_newer("0.4.0", "0.4.0-alpha.1"));
        // Higher base version is newer, even if current is pre-release
        assert!(is_newer("0.4.0-alpha.1", "0.5.0"));
        // Lower base version is not newer, even if it's stable vs pre-release
        assert!(!is_newer("0.4.0-alpha.1", "0.3.22"));
        // Two pre-releases at the same base: neither is newer (we don't rank labels)
        assert!(!is_newer("0.4.0-alpha.1", "0.4.0-beta.1"));
        // Higher base pre-release IS newer than lower base pre-release
        assert!(is_newer("0.3.0-rc.1", "0.4.0-alpha.1"));
    }

    /// Tests that is_gamdl_compatible() correctly identifies versions within
    /// and outside the [MIN_COMPATIBLE_GAMDL, MAX_COMPATIBLE_GAMDL] range.
    #[test]
    fn test_is_gamdl_compatible() {
        // Within range: compatible
        assert!(is_gamdl_compatible("2.8.4"));
        // At minimum boundary: compatible (inclusive)
        assert!(is_gamdl_compatible("2.0.0"));
        // Below minimum: incompatible (old CLI format)
        assert!(!is_gamdl_compatible("1.9.9"));
        // Unparseable string: incompatible (safe default)
        assert!(!is_gamdl_compatible("invalid"));
    }
}
