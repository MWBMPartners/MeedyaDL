// Copyright (c) 2024-2026 MWBM Partners Ltd
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
//     +-- check_app_update()     --> GitHub Releases API (repos/.../releases/latest)
//     |                               Compares running version with latest release tag
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
    /// Human-readable component name (e.g., "GAMDL", "Python Runtime", "GAMDL GUI")
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

/// Compares two semver version strings and returns true if `latest` is strictly newer than `current`.
///
/// Uses simple tuple comparison on (major, minor, patch). Unparseable parts default to 0.
/// Equal versions return false (not newer).
///
/// Examples:
/// - is_newer("1.0.0", "1.0.1") => true  (patch bump)
/// - is_newer("1.0.0", "1.0.0") => false (same version)
/// - is_newer("2.0.0", "1.9.9") => false (downgrade)
fn is_newer(current: &str, latest: &str) -> bool {
    // Parse version string into (major, minor, patch) tuple.
    // Missing or unparseable parts default to 0, making this forgiving
    // of version strings like "2.0" (treated as 2.0.0) or "v2.1" (0.0.0 — the "v" makes it unparseable).
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<&str> = v.split('.').collect();
        let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };

    let c = parse(current);
    let l = parse(latest);
    // Rust's tuple comparison is lexicographic: compares major first, then minor, then patch
    l > c
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
pub async fn check_all_updates(app: &AppHandle) -> UpdateCheckResult {
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
    match check_app_update(app).await {
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
    })
}

/// Checks for app self-updates by querying GitHub Releases.
///
/// Compares the running app version (from tauri.conf.json) with the
/// latest GitHub release tag.
async fn check_app_update(app: &AppHandle) -> Result<ComponentUpdate, String> {
    // Get the current app version from Tauri's package info.
    // This reads the version from tauri.conf.json, set at build time.
    let current_version = app.package_info().version.to_string();

    // Query the GitHub Releases API for the latest release.
    // Ref: https://docs.github.com/en/rest/releases/releases#get-the-latest-release
    // Required headers:
    // - User-Agent: GitHub API requires a UA string (can be anything)
    // - Accept: Request v3 JSON format
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/repos/MWBM-Partners-Ltd/gamdl-GUI/releases/latest")
        .header("User-Agent", "gamdl-gui")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if !response.status().is_success() {
        // 404 means no releases have been published yet — not an error condition.
        // This is expected for new repositories that haven't made their first release.
        if response.status().as_u16() == 404 {
            return Ok(ComponentUpdate {
                name: "GAMDL GUI".to_string(),
                current_version: Some(current_version),
                latest_version: None,
                update_available: false,
                is_compatible: true,
                description: None,
                release_url: None,
            });
        }
        return Err(format!("GitHub API returned HTTP {}", response.status()));
    }

    // Parse the JSON response. The GitHub Releases API returns:
    // { "tag_name": "v0.2.0", "html_url": "...", "body": "Release notes...", ... }
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))?;

    // Extract the tag name (e.g., "v0.2.0") and strip the "v" prefix
    // to get a bare semver string for comparison with the current version.
    let tag = json["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();

    // Extract the release page URL for the "View Release" button in the UI
    let html_url = json["html_url"].as_str().map(|s| s.to_string());
    // Extract and truncate the release notes for display in the update card.
    // Long release notes are cut to 200 characters to keep the UI compact.
    let body = json["body"].as_str().map(|s| {
        if s.len() > 200 {
            format!("{}...", &s[..200])
        } else {
            s.to_string()
        }
    });

    let update_available = if tag.is_empty() {
        false
    } else {
        is_newer(&current_version, &tag)
    };

    Ok(ComponentUpdate {
        name: "GAMDL GUI".to_string(),
        current_version: Some(current_version),
        latest_version: if tag.is_empty() { None } else { Some(tag) },
        update_available,
        // App updates are always "compatible" — the new version replaces the old one entirely.
        // Unlike GAMDL (which has a CLI interface contract), the app is self-contained.
        is_compatible: true,
        description: body,
        release_url: html_url,
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
    })
}

// ============================================================
// Unit tests for version comparison and compatibility checking
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that is_newer() correctly handles all semver comparison cases:
    /// patch bumps, minor bumps, major bumps, equal versions, and downgrades.
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
