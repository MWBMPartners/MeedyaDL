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
/// The frontend renders an update card for each `ComponentUpdate` with the
/// current version, latest version, and an "Update" button if applicable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentUpdate {
    /// Human-readable component name (e.g., "GAMDL", "Python Runtime", "`MeedyaDL`")
    pub name: String,
    /// Currently installed version (None if not installed).
    /// For GAMDL: from `pip show gamdl`. For Python: from `python --version`.
    /// For the app: from tauri.conf.json package version.
    pub current_version: Option<String>,
    /// Latest available version (None if the check failed or no releases exist).
    /// For GAMDL: from `PyPI` JSON API. For the app: from GitHub Releases.
    /// For Python: from `python_manager::PYTHON_VERSION` constant.
    pub latest_version: Option<String>,
    /// Whether an update is available (latest > current via semver comparison).
    /// True if not installed and a version is available on the remote source.
    pub update_available: bool,
    /// Whether this update is compatible with the current app version.
    /// For GAMDL: checked via `is_gamdl_compatible()` range gate.
    /// For app and Python: always true (updates are self-compatible).
    pub is_compatible: bool,
    /// Human-readable description of the update (e.g., release notes excerpt).
    /// For app updates: truncated first 200 chars of the GitHub release body.
    pub description: Option<String>,
    /// URL to the release page for the user to review before updating.
    /// For GAMDL: `PyPI` project page. For app: GitHub release page.
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
}

/// Combined update status for all components.
///
/// Returned by `check_all_updates()` and sent to the frontend as a single response.
/// The frontend uses `has_updates` to show/hide an update badge in the toolbar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    /// Timestamp of the check in ISO 8601 format (e.g., "2026-02-10T12:00:00Z").
    /// Used by the frontend to display "Last checked: X minutes ago".
    pub checked_at: String,
    /// Whether any compatible updates are available (quick check for badge display).
    /// True if any component has `update_available` && `is_compatible`.
    pub has_updates: bool,
    /// Per-component update status (one entry per checked component).
    pub components: Vec<ComponentUpdate>,
    /// Non-fatal errors that occurred during individual checks.
    /// For example, a network timeout on `PyPI` doesn't prevent checking GitHub.
    pub errors: Vec<String>,
}

// ============================================================
// GAMDL version compatibility
// ============================================================

/// Minimum GAMDL version known to be compatible with this app version.
/// Versions below this may have different CLI argument formats or missing features.
/// When GAMDL makes a breaking CLI change, update this to exclude old versions.
/// - 2.9.1: introduced `--song-codec-priority` (replaced `--song-codec`)
/// - 2.9.2: fixed artist download pagination bug
const MIN_COMPATIBLE_GAMDL: &str = "2.9.2";

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
/// - `is_newer("1.0.0`", "1.0.1") => true  (patch bump)
/// - `is_newer("1.0.0`", "1.0.0") => false (same version)
/// - `is_newer("2.0.0`", "1.9.9") => false (downgrade)
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
// Platform asset validation
// ============================================================

/// Returns the expected asset name substrings for the current platform.
///
/// These patterns are matched against GitHub release asset names to verify
/// that a downloadable binary exists for the user's OS and architecture.
/// Uses compile-time `cfg!()` macros, so only the current platform's
/// patterns are included in the binary.
///
/// ## Asset Naming Convention (from release.yml)
///
/// | Platform          | Pattern(s)                                    |
/// |-------------------|-----------------------------------------------|
/// | macOS ARM64       | `_aarch64.dmg`, `_aarch64.app.tar.gz`         |
/// | Windows x64       | `_x64-setup.exe`                              |
/// | Windows ARM64     | `_arm64-setup.exe`                             |
/// | Linux x64         | `_amd64.deb`, `_amd64.AppImage`               |
/// | Linux ARM64       | `_arm64.deb`                                  |
/// | Linux `ARMv7`       | `_armv7.deb`                                  |
///
/// # Returns
/// A list of substrings that should appear in at least one asset name.
/// Any single match is sufficient (the patterns are OR'd, not AND'd).
fn get_platform_asset_patterns() -> Vec<&'static str> {
    if cfg!(target_os = "macos") {
        // macOS currently only ships ARM64 (Apple Silicon).
        // The .dmg is the user-facing installer; the .app.tar.gz is the
        // Tauri updater artifact (used by in-app updates).
        vec!["_aarch64.dmg", "_aarch64.app.tar.gz"]
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            vec!["_arm64-setup.exe"]
        } else {
            // x86_64 (also works on ARM64 via Windows emulation)
            vec!["_x64-setup.exe"]
        }
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "aarch64") {
            vec!["_arm64.deb"]
        } else if cfg!(target_arch = "arm") {
            vec!["_armv7.deb", "_armhf.deb"]
        } else {
            // x86_64
            vec!["_amd64.deb", "_amd64.AppImage"]
        }
    } else {
        // Unknown platform — no expected assets, so the check will fail
        // gracefully (update won't be shown).
        vec![]
    }
}

/// Checks whether a GitHub release has downloadable assets for the current platform.
///
/// Performs two checks:
/// 1. **Updater manifest**: The `latest.json` file must exist. This is the
///    Tauri updater manifest that contains platform-specific download URLs
///    and signature hashes. Without it, the in-app updater cannot function.
/// 2. **Platform binary**: At least one asset must match the current OS/arch
///    patterns from [`get_platform_asset_patterns()`]. This ensures the user's
///    platform build actually succeeded.
///
/// ## Why Both Checks?
///
/// - `latest.json` missing → release was just created, no builds completed yet
/// - `latest.json` exists but no platform asset → other platforms built, but
///   this platform's build failed (e.g., macOS DMG bundling error)
///
/// # Arguments
/// * `assets` - The `assets` array from the GitHub Releases API response.
///   Each element is a JSON object with at least a `name` field.
///
/// # Returns
/// `true` if both the updater manifest and a platform-specific asset exist.
fn has_platform_assets(assets: Option<&Vec<serde_json::Value>>) -> bool {
    let Some(assets) = assets else {
        log::debug!("Release has no assets array");
        return false;
    };

    if assets.is_empty() {
        log::debug!("Release has an empty assets array");
        return false;
    }

    // Collect asset names for matching.
    let asset_names: Vec<&str> = assets.iter().filter_map(|a| a["name"].as_str()).collect();

    // Check 1: The Tauri updater manifest must exist.
    let has_manifest = asset_names.contains(&"latest.json");
    if !has_manifest {
        log::info!(
            "Release has {} assets but no latest.json manifest — builds may still be in progress",
            asset_names.len()
        );
        return false;
    }

    // Check 2: At least one asset must match the current platform.
    let patterns = get_platform_asset_patterns();
    if patterns.is_empty() {
        // Unknown platform — can't verify, assume unavailable.
        log::debug!("No platform asset patterns defined for this target");
        return false;
    }

    let has_platform = asset_names
        .iter()
        .any(|name| patterns.iter().any(|pattern| name.contains(pattern)));

    if !has_platform {
        log::info!(
            "Release has latest.json but no assets matching platform patterns {patterns:?} — \
             this platform's build may have failed"
        );
    }

    has_platform
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
pub async fn check_all_updates(app: &AppHandle, check_pre_releases: bool) -> UpdateCheckResult {
    let mut components = Vec::new();
    let mut errors = Vec::new();

    // Check GAMDL updates via PyPI JSON API.
    // This is the most important check since GAMDL receives frequent updates.
    match check_gamdl_update(app).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("GAMDL check failed: {e}")),
    }

    // Check for app self-updates via GitHub Releases API.
    // Compares the running app version against the latest GitHub release tag.
    // When check_pre_releases is true, includes beta/RC releases in the check.
    match check_app_update(app, check_pre_releases).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("App update check failed: {e}")),
    }

    // Check Python runtime update by comparing the installed version
    // against the target version defined in python_manager.rs constants.
    match check_python_update(app).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("Python check failed: {e}")),
    }

    // Aggregate: an update is "available" only if it's both newer AND compatible.
    // This prevents the UI from showing incompatible GAMDL versions as available.
    let has_updates = components
        .iter()
        .any(|c| c.update_available && c.is_compatible);

    UpdateCheckResult {
        checked_at: chrono::Utc::now().to_rfc3339(),
        has_updates,
        components,
        errors,
    }
}

/// Checks for GAMDL updates by comparing the installed version with `PyPI`.
///
/// # Returns
/// A `ComponentUpdate` with the current and latest GAMDL versions.
async fn check_gamdl_update(app: &AppHandle) -> Result<ComponentUpdate, String> {
    // Get the currently installed GAMDL version via `pip show gamdl`.
    // Returns None if GAMDL is not installed (Python not found, or package not installed).
    let current = gamdl_service::get_gamdl_version(app).await.unwrap_or(None);

    // Get the latest version from PyPI JSON API.
    // Queries https://pypi.org/pypi/gamdl/json and extracts info.version.
    // Returns None if the request failed (network error, PyPI down, etc.).
    let latest = gamdl_service::check_latest_gamdl_version().await.ok();

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
    let is_compatible = latest.as_ref().is_some_and(|v| is_gamdl_compatible(v));

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
        release_url: latest.map(|v| format!("https://pypi.org/project/gamdl/{v}/")),
        release_body: None,
        // GAMDL updates are from PyPI, not GitHub Releases — no pre-release concept
        is_prerelease: false,
        tag_name: None,
    })
}

/// Checks for app self-updates by querying GitHub Releases.
///
/// Compares the running app version (from tauri.conf.json) with the
/// latest GitHub release tag. When `check_pre_releases` is true, queries
/// all recent releases (including betas/RCs); otherwise only checks the
/// latest stable release.
///
/// # Arguments
/// * `app` - Tauri app handle for reading the current app version
/// * `check_pre_releases` - Whether to include pre-release versions.
///   When true: queries `releases?per_page=5` and takes the newest (which may be a pre-release).
///   When false: queries `releases/latest` (GitHub automatically excludes pre-releases).
async fn check_app_update(
    app: &AppHandle,
    check_pre_releases: bool,
) -> Result<ComponentUpdate, String> {
    // Get the current app version from Tauri's package info.
    // This reads the version from tauri.conf.json, set at build time.
    let current_version = app.package_info().version.to_string();

    // Choose the GitHub API endpoint based on pre-release preference.
    // - Stable only: `releases/latest` returns a single release object (excludes pre-releases)
    // - Include pre-releases: `releases?per_page=5` returns an array sorted newest-first
    let (url, is_list) = if check_pre_releases {
        (
            "https://api.github.com/repos/MWBMPartners/MeedyaDL/releases?per_page=5",
            true,
        )
    } else {
        (
            "https://api.github.com/repos/MWBMPartners/MeedyaDL/releases/latest",
            false,
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
        .map_err(|e| format!("GitHub API request failed: {e}"))?;

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
            });
        }
        return Err(format!("GitHub API returned HTTP {}", response.status()));
    }

    // Parse the JSON response.
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {e}"))?;

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
            });
        }
        // Index 0 is the newest release (may be a pre-release)
        &releases[0]
    } else {
        // Stable mode: response is a single release object
        &json
    };

    // Delegate to the response parser to extract fields and build the ComponentUpdate.
    Ok(parse_release_from_response(release, &current_version))
}

/// Parses a single GitHub release JSON object into a `ComponentUpdate`.
///
/// Extracts the tag name, release URL, release body, pre-release flag, and
/// asset list from the release object. Compares the tag version against the
/// current app version and verifies platform asset availability.
///
/// # Arguments
/// * `release` - A single GitHub release JSON object (from `releases/latest`
///   or `releases[0]` in the array endpoint).
/// * `current_version` - The running app version string (e.g., "0.3.7").
///
/// # Returns
/// A fully populated `ComponentUpdate` for the app.
fn parse_release_from_response(
    release: &serde_json::Value,
    current_version: &str,
) -> ComponentUpdate {
    // Extract the tag name (e.g., "v0.3.7") and strip the "v" prefix
    // to get a bare semver string for comparison with the current version.
    let raw_tag = release["tag_name"].as_str().unwrap_or("");
    let tag = raw_tag.trim_start_matches('v').to_string();

    // Extract the release page URL for the "View Release" button in the UI
    let html_url = release["html_url"]
        .as_str()
        .map(std::string::ToString::to_string);
    // Extract the full release notes body for the Updates page (untruncated).
    let full_body = release["body"]
        .as_str()
        .map(std::string::ToString::to_string);
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

    let mut update_available = if tag.is_empty() {
        false
    } else {
        is_newer(current_version, &tag)
    };

    // If a newer version exists, verify that the release actually has
    // downloadable assets for this platform. This prevents showing an
    // update prompt when:
    //   - The release was just created and builds haven't completed yet
    //   - This platform's build failed (e.g., macOS DMG bundling error)
    //   - The release is an empty tag with no binaries attached
    if update_available {
        let assets = release["assets"].as_array();
        if !has_platform_assets(assets) {
            log::info!(
                "Version {tag} is newer than {current_version} but has no assets for this platform — \
                 suppressing update notification"
            );
            update_available = false;
        }
    }

    ComponentUpdate {
        name: "MeedyaDL".to_string(),
        current_version: Some(current_version.to_string()),
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
    }
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
    let update_available = current.as_ref().is_some_and(|c| is_newer(c, target));

    Ok(ComponentUpdate {
        name: "Python Runtime".to_string(),
        current_version: current,
        latest_version: Some(target.to_string()),
        update_available,
        // Python updates are always compatible since we control the version
        // and test it with GAMDL before shipping.
        is_compatible: true,
        description: if update_available {
            Some(format!("Python {target} available (portable runtime)"))
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
        assert!(is_gamdl_compatible("2.9.2"));
        assert!(is_gamdl_compatible("2.10.0"));
        assert!(is_gamdl_compatible("3.0.0"));
        // At minimum boundary: compatible (inclusive)
        assert!(is_gamdl_compatible("2.9.2"));
        // Below minimum: incompatible (missing pagination fix, old CLI)
        assert!(!is_gamdl_compatible("2.9.1"));
        assert!(!is_gamdl_compatible("2.8.4"));
        assert!(!is_gamdl_compatible("1.9.9"));
        // Unparseable string: incompatible (safe default)
        assert!(!is_gamdl_compatible("invalid"));
    }

    /// Tests that get_platform_asset_patterns() returns a non-empty list
    /// of expected asset name substrings for the current compile target.
    #[test]
    fn test_get_platform_asset_patterns_returns_patterns() {
        let patterns = get_platform_asset_patterns();
        // Every supported platform should have at least one pattern.
        // This would only be empty for an unsupported/exotic target.
        assert!(
            !patterns.is_empty(),
            "Expected at least one asset pattern for the current platform"
        );
        // Every pattern should be a non-empty substring that appears in release asset names.
        for pattern in &patterns {
            assert!(!pattern.is_empty(), "Asset pattern must not be empty");
        }
    }

    /// Helper to build a mock GitHub release `assets` JSON array from a list
    /// of asset filenames.
    fn mock_assets(names: &[&str]) -> Vec<serde_json::Value> {
        names
            .iter()
            .map(|name| serde_json::json!({ "name": name }))
            .collect()
    }

    /// Tests has_platform_assets() with a complete set of assets (latest.json
    /// + platform binary). Should return true.
    #[test]
    fn test_has_platform_assets_with_full_release() {
        // Simulate a complete v0.3.23-style release with all platform assets.
        let assets = mock_assets(&[
            "latest.json",
            "MeedyaDL_0.3.23_aarch64.dmg",
            "MeedyaDL_0.3.23_aarch64.app.tar.gz",
            "MeedyaDL_0.3.23_aarch64.app.tar.gz.sig",
            "MeedyaDL_0.3.23_x64-setup.exe",
            "MeedyaDL_0.3.23_arm64-setup.exe",
            "MeedyaDL_0.3.23_amd64.deb",
            "MeedyaDL_0.3.23_amd64.AppImage",
            "MeedyaDL_0.3.23_arm64.deb",
            "MeedyaDL_0.3.23_armv7.deb",
        ]);
        assert!(
            has_platform_assets(Some(&assets)),
            "Should detect platform assets in a fully-populated release"
        );
    }

    /// Tests has_platform_assets() when the assets array is completely empty
    /// (release just created, no builds have completed yet).
    #[test]
    fn test_has_platform_assets_empty_assets() {
        let assets: Vec<serde_json::Value> = vec![];
        assert!(
            !has_platform_assets(Some(&assets)),
            "Empty assets array should return false"
        );
    }

    /// Tests has_platform_assets() when the assets array is None
    /// (unexpected API response format).
    #[test]
    fn test_has_platform_assets_none() {
        assert!(
            !has_platform_assets(None),
            "None assets should return false"
        );
    }

    /// Tests has_platform_assets() when latest.json is missing but platform
    /// binaries exist. This happens if the updater manifest upload failed.
    #[test]
    fn test_has_platform_assets_missing_manifest() {
        let assets = mock_assets(&[
            "MeedyaDL_0.3.23_aarch64.dmg",
            "MeedyaDL_0.3.23_x64-setup.exe",
            "MeedyaDL_0.3.23_amd64.deb",
        ]);
        assert!(
            !has_platform_assets(Some(&assets)),
            "Should return false when latest.json is missing"
        );
    }

    /// Tests has_platform_assets() when latest.json exists but no platform
    /// binaries match. This can happen if only some platform builds succeeded
    /// and this platform's build failed.
    #[test]
    fn test_has_platform_assets_manifest_only() {
        let assets = mock_assets(&["latest.json"]);
        assert!(
            !has_platform_assets(Some(&assets)),
            "Should return false when only latest.json exists (no platform binaries)"
        );
    }
}
