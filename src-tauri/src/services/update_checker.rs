// Copyright (c) 2026 MeedyaDL
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
// This prevents the user from upgrading to a GAMDL version that may have
// changed its CLI interface in incompatible ways. The compatible range is
// declared in `tool-versions.toml` → `[gamdl]` (`minimum_version` …
// `maximum_tested_version`) and queried via
// `services::gamdl_capabilities::should_offer_upgrade`. Bumping the ceiling
// is a one-file change; see `services/gamdl_capabilities.rs` for the
// surrounding design.
//
// ## References
//
// - PyPI JSON API: https://pypi.org/pypi/{package}/json
//   Response format: { "info": { "version": "X.Y.Z", ... }, "releases": { ... } }
// - GitHub Releases API: https://docs.github.com/en/rest/releases/releases#get-the-latest-release
// - Reqwest HTTP client: https://docs.rs/reqwest/latest/reqwest/
// - Chrono for timestamps: https://docs.rs/chrono/latest/chrono/

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tauri::AppHandle;

// Cached semver-extraction regex.
//
// `check_component_update` runs on every app startup, every periodic update
// poll, and every manual "Check for updates" click. Compiling this trivial
// pattern per call wastes microseconds on a hot path; caching once via
// `LazyLock` matches the pattern used in `apple_music_api.rs` and
// `process.rs`. Pattern is a static literal so `.expect()` only fires on
// developer typos at boot — never at runtime.
static SEMVER_EXTRACT_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(\d+\.\d+(?:\.\d+)?)").expect("static regex"));

// gamdl_service: provides get_gamdl_version() and check_latest_gamdl_version() for GAMDL update checks.
// python_manager: provides get_installed_python_version() and get_target_python_version() for Python update checks.
use crate::models::settings::UpdateChannel;
use crate::services::{gamdl_capabilities, gamdl_service, python_manager};
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
    /// For GAMDL: any parseable semver passes (we only reject obviously
    /// malformed strings like `v3-rc`). The "untested vs tested" status
    /// is communicated separately via [`Self::is_untested`] so users see
    /// the new release as soon as upstream ships it. For app and Python:
    /// always true (updates are self-compatible).
    pub is_compatible: bool,
    /// Whether the latest available version is **above** the
    /// `maximum_tested_version` declared in this MeedyaDL build's
    /// `tool-versions.toml`.
    ///
    /// `true` here flags an upgrade we have not validated against the
    /// MeedyaDL CLI / INI surface — installation works (the user opts in
    /// via an explicit-version pin in `pip_target_spec`), but the
    /// frontend should render an amber "Untested" badge and a short
    /// disclaimer. Currently only set for the GAMDL component;
    /// always `false` for the app, Python, pip engines, and binary tools.
    #[serde(default)]
    pub is_untested: bool,
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
    /// PyPI package name for pip-based engines (e.g., "ofscraper", "votify").
    /// Used by the frontend to call `upgrade_pip_engine` with the correct identifier
    /// (display names like "OF-Scraper" don't match PyPI package names).
    pub pip_package: Option<String>,
    /// Canonical tool ID for binary tools (e.g., "ffmpeg", "nm3u8dlre").
    /// Used by the frontend to call `install_dependency` to re-download and
    /// upgrade the tool. Only set for external binary tools, not pip engines.
    pub tool_id: Option<String>,
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
    /// When running a pre-release, the latest stable version available for
    /// rollback (#267). `None` when running a stable version or if no stable
    /// release exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_version: Option<String>,
    /// Tag name for the rollback release (e.g., "v0.32.1").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_tag: Option<String>,
}

// ============================================================
// GAMDL version compatibility
// ============================================================
//
// The compatibility range was previously declared via two `const`
// version strings pinned inside this file. That was brittle because:
//
//   * bumping the range required a code change rather than a
//     tool-versions.toml bump, and
//   * the frontend had no way to surface the range to users, so
//     "why is this update hidden?" needed engineering support to
//     explain.
//
// Today both are fixed by delegating to `gamdl_capabilities`, which
// owns the single source of truth (`tool-versions.toml` → `[gamdl]`)
// and exposes typed classification helpers.

/// Checks whether a GAMDL version is safe to surface as an update.
///
/// Thin wrapper over [`gamdl_capabilities::should_offer_upgrade`] kept
/// so the surrounding `check_gamdl_update` call site stays readable.
/// Returns `false` only for unparseable version strings. Above-ceiling
/// versions DO pass — the "untested" state is communicated via the
/// separate [`ComponentUpdate::is_untested`] field rather than by
/// hiding the update entirely. See `gamdl_capabilities.rs` for the
/// rationale.
fn is_gamdl_compatible(version: &str) -> bool {
    gamdl_capabilities::should_offer_upgrade(version)
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

/// Returns the Tauri updater platform key for the current OS and architecture.
///
/// These keys must match the entries that `tauri-apps/tauri-action` writes into
/// `latest.json` during CI builds. The format is `{os}-{arch}`.
fn get_updater_platform_key() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "darwin-aarch64"
        } else {
            "darwin-x86_64"
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            // Tauri writes both `windows-aarch64` and `windows-aarch64-nsis`;
            // checking either suffices since they're always written together.
            "windows-aarch64"
        } else {
            "windows-x86_64"
        }
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "aarch64") {
            "linux-aarch64"
        } else {
            "linux-x86_64"
        }
    } else {
        "unknown"
    }
}

/// Downloads and parses `latest.json` from a release to verify it contains
/// a download entry for the current platform.
///
/// This catches the race condition where parallel CI builds each overwrite
/// `latest.json` — the last build to finish wins, and its manifest may only
/// contain that platform's entry. Without this check, the update banner would
/// appear even though the Tauri updater would fail with "no update found".
///
/// # Arguments
/// * `client` - Pre-configured HTTP client (reused from the update check call)
/// * `tag` - Release tag (e.g., "v0.29.0")
///
/// # Returns
/// `true` if `latest.json` contains a `platforms.{key}` entry for this platform,
/// `false` if the manifest is missing the entry, unparseable, or unreachable.
async fn verify_manifest_has_platform(client: &reqwest::Client, tag: &str) -> bool {
    let url =
        format!("https://github.com/MWBMPartners/MeedyaDL/releases/download/{tag}/latest.json");
    let platform_key = get_updater_platform_key();

    let response = match client
        .get(&url)
        .header("User-Agent", "meedyadl")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            log::warn!(
                "Failed to download latest.json for {tag}: HTTP {}",
                r.status()
            );
            // If we can't download it, don't suppress the update — let the
            // user try and get a proper error from the Tauri updater.
            return true;
        }
        Err(e) => {
            log::warn!("Failed to download latest.json for {tag}: {e}");
            return true;
        }
    };

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            log::warn!("Failed to parse latest.json for {tag}: {e}");
            return true;
        }
    };

    // Check if the platforms object contains the current platform key
    let has_platform = json
        .get("platforms")
        .and_then(|p| p.get(platform_key))
        .is_some();

    if has_platform {
        log::debug!("latest.json for {tag} contains platform entry for {platform_key}");
    } else {
        let available: Vec<&str> = json
            .get("platforms")
            .and_then(|p| p.as_object())
            .map(|obj| obj.keys().map(|k| k.as_str()).collect())
            .unwrap_or_default();
        log::warn!(
            "latest.json for {tag} is missing platform {platform_key} \
             (available: {available:?})"
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
pub async fn check_all_updates(
    app: &AppHandle,
    check_pre_releases: bool,
    user_channel: UpdateChannel,
) -> UpdateCheckResult {
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
    // `user_channel` is used to filter out releases that don't match the user's
    // subscribed stability tier (e.g., a beta user won't see nightly builds).
    match check_app_update(app, check_pre_releases, user_channel).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("App update check failed: {e}")),
    }

    // Check Python runtime update by comparing the installed version
    // against the target version defined in python_manager.rs constants.
    match check_python_update(app).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("Python check failed: {e}")),
    }

    // Check all enabled pip-based engines from engines.toml for updates.
    // Skips GAMDL (already checked above) and disabled engines.
    for (name, package) in get_enabled_pip_engines() {
        // Skip GAMDL — already checked with its own compatibility gate
        if package == "gamdl" {
            continue;
        }

        match check_pip_engine_update(app, &name, &package).await {
            Ok(update) => components.push(update),
            Err(e) => errors.push(format!("{name} check failed: {e}")),
        }
    }

    // Check external tools for updates (#273).
    // Compares installed tool versions (from tool-versions.toml minimums)
    // against the latest GitHub release for tools that have known repos.
    let tool_checks = [
        ("ffmpeg", "BtbN/FFmpeg-Builds", "FFmpeg"),
        ("nm3u8dlre", "nilaoda/N_m3u8DL-RE", "N_m3u8DL-RE"),
    ];
    for (tool_id, repo, display_name) in &tool_checks {
        let binary = crate::services::dependency_manager::get_tool_binary_path(app, tool_id);
        if binary.exists() {
            match check_github_tool_update(app, tool_id, repo, display_name).await {
                Ok(update) => components.push(update),
                Err(e) => {
                    log::debug!("Tool update check failed for {tool_id}: {e}");
                }
            }
        }
    }

    // Aggregate: an update is "available" only if it's both newer AND compatible.
    // This prevents the UI from showing incompatible GAMDL versions as available.
    let has_updates = components
        .iter()
        .any(|c| c.update_available && c.is_compatible);

    // Check for rollback opportunity (#267): when the current version is a
    // pre-release, fetch the latest stable release for rollback.
    let current_version = app.package_info().version.to_string();
    let is_pre = current_version.contains('-'); // e.g., "0.33.0-rc.1"
    let (rollback_version, rollback_tag) = if is_pre {
        match fetch_latest_stable_release().await {
            Ok(Some((ver, tag))) => (Some(ver), Some(tag)),
            _ => (None, None),
        }
    } else {
        (None, None)
    };

    UpdateCheckResult {
        checked_at: chrono::Utc::now().to_rfc3339(),
        has_updates,
        components,
        errors,
        rollback_version,
        rollback_tag,
    }
}

/// Fetch the latest stable (non-pre-release) GitHub release (#267).
async fn fetch_latest_stable_release() -> Result<Option<(String, String)>, String> {
    let url = "https://api.github.com/repos/MWBMPartners/MeedyaDL/releases/latest";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let response = client
        .get(url)
        .header("User-Agent", "meedyadl")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {e}"))?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    let tag = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .map(String::from);
    let version = tag
        .as_ref()
        .map(|t| t.trim_start_matches('v').to_string());

    match (version, tag) {
        (Some(v), Some(t)) => Ok(Some((v, t))),
        _ => Ok(None),
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

    // Compatibility now permits any parseable semver — the strict "is
    // it inside the validated window?" check moved to `is_untested`.
    // Hiding above-ceiling versions silently meant users couldn't see
    // newly released GAMDL versions until we'd audited them; surfacing
    // them with a warning badge is the better default.
    let is_compatible = latest.as_ref().is_some_and(|v| is_gamdl_compatible(v));
    let is_untested = latest
        .as_ref()
        .is_some_and(|v| gamdl_capabilities::is_above_tested_ceiling(v));

    let description = if update_available {
        Some(if is_untested {
            // Surface the warning in the description text so it shows up
            // even in places that don't render the dedicated badge.
            "New GAMDL version available on PyPI (untested with this MeedyaDL build)"
                .to_string()
        } else {
            "New GAMDL version available on PyPI".to_string()
        })
    } else {
        None
    };

    Ok(ComponentUpdate {
        name: "GAMDL".to_string(),
        current_version: current,
        latest_version: latest.clone(),
        update_available,
        is_compatible,
        is_untested,
        description,
        release_url: latest.map(|v| format!("https://pypi.org/project/gamdl/{v}/")),
        release_body: None,
        // GAMDL updates are from PyPI, not GitHub Releases — no pre-release concept
        is_prerelease: false,
        tag_name: None,
        pip_package: Some("gamdl".to_string()),
        tool_id: None,
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
    user_channel: UpdateChannel,
) -> Result<ComponentUpdate, String> {
    // Get the current app version from Tauri's package info.
    // This reads the version from tauri.conf.json, set at build time.
    let current_version = app.package_info().version.to_string();

    // Choose the GitHub API endpoint based on pre-release preference.
    // - Stable only: `releases/latest` returns a single release object (excludes pre-releases)
    // - Include pre-releases: `releases?per_page=20` returns an array sorted newest-first.
    //   We fetch up to 20 so that, after channel filtering, we still find the most
    //   recent release on the user's tier (nightly releases ship daily and can bury
    //   other channels in the first few results).
    let (url, is_list) = if check_pre_releases {
        (
            "https://api.github.com/repos/MWBMPartners/MeedyaDL/releases?per_page=20",
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
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;
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
                is_untested: false,
                description: None,
                release_url: None,
                release_body: None,
                is_prerelease: false,
                tag_name: None,
                pip_package: None,
                tool_id: None,
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
    // or the newest channel-matching entry from the array (pre-release mode).
    let release = if is_list {
        let releases = json
            .as_array()
            .ok_or("GitHub API returned unexpected format (expected array)")?;
        // Pick the newest release whose channel is at least as stable as
        // the user's subscribed channel. A Beta user sees Beta, Rc, and
        // Stable; a Stable user sees only Stable. Releases from a less-stable
        // channel (e.g. a fresh nightly when the user is on Beta) are skipped
        // so the UI never surfaces an update that crosses down the stability
        // ladder. The release list is newest-first by GitHub's API contract,
        // so `find` returns the newest qualifying entry.
        let matched = releases.iter().find(|r| {
            let tag = r["tag_name"].as_str().unwrap_or("");
            UpdateChannel::from_tag(tag) >= user_channel
        });
        let Some(release) = matched else {
            return Ok(ComponentUpdate {
                name: "MeedyaDL".to_string(),
                current_version: Some(current_version),
                latest_version: None,
                update_available: false,
                is_compatible: true,
                is_untested: false,
                description: None,
                release_url: None,
                release_body: None,
                is_prerelease: false,
                tag_name: None,
                pip_package: None,
                tool_id: None,
            });
        };
        release
    } else {
        // Stable mode: response is a single release object.
        &json
    };

    // Delegate to the response parser to extract fields and build the ComponentUpdate.
    let mut update = parse_release_from_response(release, &current_version);

    // Verify that `latest.json` actually contains an entry for this platform.
    // The `has_platform_assets()` check above only verifies that the `latest.json`
    // *file* exists in the release, not that it contains a download entry for the
    // current OS/arch. This can happen due to the tauri-action race condition where
    // parallel platform builds each overwrite `latest.json` — the last build to
    // finish wins, and its manifest only contains that platform's entry.
    if update.update_available {
        if let Some(tag) = &update.tag_name {
            if !verify_manifest_has_platform(&client, tag).await {
                log::info!(
                    "Version {} is newer but latest.json has no entry for this platform — \
                     suppressing update notification",
                    update.latest_version.as_deref().unwrap_or("unknown")
                );
                update.update_available = false;
            }
        }
    }

    // For multi-version jumps (e.g., v0.13.0 → v0.15.0), aggregate release notes
    // from all intermediate versions so the user sees a complete changelog.
    if update.update_available {
        let combined_body =
            aggregate_intermediate_release_notes(&client, &current_version, check_pre_releases)
                .await;
        if let Some(body) = combined_body {
            update.release_body = Some(body);
        }
    }

    Ok(update)
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
        // Untested-vs-tested only applies to GAMDL (whose CLI surface
        // we audit per-version). MeedyaDL releases are self-contained.
        is_untested: false,
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
        pip_package: None,
        tool_id: None,
    }
}

/// Aggregates release notes from all versions between the user's current version
/// and the latest available version.
///
/// When a user jumps multiple versions (e.g., v0.13.0 → v0.15.0), simply showing
/// the latest release's notes omits everything that changed in intermediate versions.
/// This function collects release bodies for all versions newer than `current_version`,
/// ordered newest-first with markdown headers for each version.
///
/// # Arguments
/// * `client` - Pre-configured HTTP client (reused from the initial API call)
/// * `current_version` - The running app version (bare semver, e.g., "0.13.0")
/// * `check_pre_releases` - Whether pre-releases should be included
///
/// # Returns
/// `Some(combined_body)` if multiple intermediate releases were found, `None` if only
/// one release is newer (no aggregation needed) or on any API error (graceful fallback).
async fn aggregate_intermediate_release_notes(
    client: &reqwest::Client,
    current_version: &str,
    check_pre_releases: bool,
) -> Option<String> {
    // Always fetch the full release list (up to 20) for aggregation.
    // The initial update check may only fetch 5 releases (per_page=5 in pre-release mode),
    // which is insufficient when the user is many versions behind.
    let url = "https://api.github.com/repos/MWBMPartners/MeedyaDL/releases?per_page=20";
    let response = client
        .get(url)
        .header("User-Agent", "meedyadl")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let releases: Vec<serde_json::Value> = response.json().await.ok()?;

    // Filter to releases that are newer than current_version, ordered newest-first
    // (GitHub API returns them in reverse chronological order).
    let intermediate: Vec<&serde_json::Value> = releases
        .iter()
        .filter(|r| {
            let tag = r["tag_name"].as_str().unwrap_or("").trim_start_matches('v');
            if tag.is_empty() {
                return false;
            }
            // Skip pre-releases if user only wants stable
            if !check_pre_releases && r["prerelease"].as_bool().unwrap_or(false) {
                return false;
            }
            is_newer(current_version, tag)
        })
        .collect();

    // If only one release is newer, no aggregation needed — the caller already has its body.
    if intermediate.len() <= 1 {
        return None;
    }

    // Combine release bodies newest-first, each with a version header.
    let mut combined = String::new();
    for release in &intermediate {
        let tag = release["tag_name"].as_str().unwrap_or("unknown");
        let body = release["body"].as_str().unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        if !combined.is_empty() {
            combined.push_str("\n\n---\n\n");
        }
        // Check if the body already starts with a version heading (e.g., "## [0.15.0]")
        // to avoid duplicating it. Release-please bodies typically start with the version.
        let body_starts_with_version = body.starts_with('#');
        if !body_starts_with_version {
            combined.push_str(&format!("## {tag}\n\n"));
        }
        combined.push_str(body);
    }

    if combined.is_empty() {
        return None;
    }

    log::info!(
        "Aggregated release notes from {} intermediate versions (current: v{current_version})",
        intermediate.len()
    );

    Some(combined)
}

/// Checks for Python runtime updates by comparing with python-build-standalone.
///
/// Compares the installed Python version with the version constant in
/// `python_manager.rs`. In the future, this could also check GitHub
/// for newer python-build-standalone releases.
/// Check for external tool updates via GitHub Releases API (#273).
///
/// Queries the latest release from the tool's GitHub repo and compares
/// the tag against the minimum version from tool-versions.toml. Since
/// we don't track the exact installed version (only minimum requirements),
/// this reports "update may be available" when the latest release is newer
/// than the minimum.
async fn check_github_tool_update(
    app: &AppHandle,
    tool_id: &str,
    github_repo: &str,
    display_name: &str,
) -> Result<ComponentUpdate, String> {
    // Reuse the centralized dependency-manager logic so each tool uses its
    // configured version flag and parser instead of assuming `--version`.
    let binary = crate::services::dependency_manager::get_tool_binary_path(app, tool_id);
    let current_version = if binary.exists() {
        crate::services::dependency_manager::get_tool_version(&binary, tool_id)
            .await
            .ok()
    } else {
        None
    };
    let url = format!("https://api.github.com/repos/{github_repo}/releases/latest");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let response = client
        .get(&url)
        .header("User-Agent", "meedyadl")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed for {tool_id}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API returned {} for {tool_id}",
            response.status()
        ));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response for {tool_id}: {e}"))?;

    let latest_tag = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim_start_matches('v').to_string())
        .unwrap_or_default();

    // Extract a normalized semver from the GitHub tag for comparison.
    // Some repos (e.g., BtbN/FFmpeg-Builds) use non-semver tags like "latest"
    // or "autobuild-2024-12-19", which can't be compared numerically.
    // Others (e.g., nilaoda/N_m3u8DL-RE) use "v0.5.1-beta" — we strip
    // the pre-release suffix to match how extract_version_from_output() works.
    let latest_semver = SEMVER_EXTRACT_RE
        .find(&latest_tag)
        .map(|m| m.as_str().to_string());

    let latest_version = if latest_tag.is_empty() {
        None
    } else {
        Some(latest_tag.clone())
    };

    // Compare using proper semver comparison via is_newer().
    // Only report an update if:
    // 1. Both versions are known
    // 2. The GitHub tag contains a parseable semver (skip non-version tags like "latest")
    // 3. The latest semver is genuinely newer than the installed version
    let update_available = match (&current_version, &latest_semver) {
        (Some(cur), Some(latest)) => is_newer(cur, latest),
        _ => false,
    };

    Ok(ComponentUpdate {
        name: display_name.to_string(),
        current_version,
        latest_version: latest_semver.or(latest_version),
        update_available,
        is_compatible: true,
        is_untested: false,
        description: if update_available {
            Some(format!("Newer version of {display_name} available"))
        } else {
            None
        },
        release_url: Some(format!("https://github.com/{github_repo}/releases/latest")),
        release_body: None,
        is_prerelease: false,
        tag_name: None,
        pip_package: None,
        tool_id: Some(tool_id.to_string()),
    })
}

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
        is_untested: false,
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
        pip_package: None,
        tool_id: None,
    })
}

// ============================================================
// Pip engine update checking (engines.toml-driven)
// ============================================================

/// Parses `engines.toml` and returns (display_name, pip_package) pairs for
/// all engines that are `enabled = true` AND `install_method = "pip"`.
fn get_enabled_pip_engines() -> Vec<(String, String)> {
    let toml_str = include_str!("../../engines.toml");
    let parsed: toml::Value = match toml::from_str(toml_str) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Failed to parse engines.toml: {e}");
            return vec![];
        }
    };

    let Some(engines) = parsed.get("engines").and_then(|e| e.as_table()) else {
        return vec![];
    };

    engines
        .iter()
        .filter_map(|(_id, def)| {
            let enabled = def.get("enabled")?.as_bool().unwrap_or(false);
            let method = def.get("install_method")?.as_str().unwrap_or("");
            if !enabled || method != "pip" {
                return None;
            }
            let name = def.get("name")?.as_str()?.to_string();
            let package = def.get("pip_package")?.as_str()?.to_string();
            Some((name, package))
        })
        .collect()
}

/// Checks for updates to a pip-based engine by comparing the installed
/// version against the latest version on PyPI.
///
/// Follows the same pattern as `check_gamdl_update()` but works for any
/// pip package. The `required` flag from engines.toml determines whether
/// a missing engine counts as "update available" (required=true) or is
/// simply reported as "not installed" (required=false).
async fn check_pip_engine_update(
    app: &AppHandle,
    display_name: &str,
    pip_package: &str,
) -> Result<ComponentUpdate, String> {
    // Get installed version via pip show
    let installed = super::pip_engine_service::get_pip_engine_version(app, pip_package).await?;

    // Get latest version from PyPI
    let latest = super::pip_engine_service::check_latest_pypi_version(pip_package).await?;

    let update_available = match (&installed, &latest) {
        (Some(current), Some(newest)) => is_newer(current, newest),
        (None, Some(_)) => true, // Not installed but available
        _ => false,
    };

    let description = match (&installed, &latest) {
        (Some(_), Some(newest)) if update_available => {
            Some(format!("Version {newest} available on PyPI"))
        }
        (None, Some(newest)) => Some(format!("Not installed (latest: {newest})")),
        _ => None,
    };

    let release_url = Some(format!("https://pypi.org/project/{pip_package}/"));

    Ok(ComponentUpdate {
        name: display_name.to_string(),
        current_version: installed,
        latest_version: latest,
        update_available,
        is_compatible: true, // Pip engines don't have compatibility gates (unlike GAMDL)
        is_untested: false,  // Untested-vs-tested only applies to GAMDL
        description,
        release_url,
        release_body: None,
        is_prerelease: false,
        tag_name: None,
        pip_package: Some(pip_package.to_string()),
        tool_id: None,
    })
}

// ============================================================
// Unit tests for version comparison and compatibility checking
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that UpdateChannel::from_tag correctly maps release tag suffixes
    /// to their channel. Stable is the fallback for anything unrecognised so
    /// an unexpected tag never downgrades a user's stability selection.
    #[test]
    fn test_update_channel_from_tag() {
        assert_eq!(
            UpdateChannel::from_tag("v1.0.0"),
            UpdateChannel::Stable
        );
        assert_eq!(
            UpdateChannel::from_tag("1.0.0"),
            UpdateChannel::Stable
        );
        assert_eq!(
            UpdateChannel::from_tag("v1.0.0-nightly.20260420"),
            UpdateChannel::Nightly
        );
        assert_eq!(
            UpdateChannel::from_tag("v1.0.0-weekly.202616"),
            UpdateChannel::Weekly
        );
        assert_eq!(
            UpdateChannel::from_tag("v1.0.0-monthly.202604"),
            UpdateChannel::Monthly
        );
        assert_eq!(
            UpdateChannel::from_tag("v1.0.0-alpha.1"),
            UpdateChannel::Alpha
        );
        assert_eq!(
            UpdateChannel::from_tag("v1.0.0-beta.1"),
            UpdateChannel::Beta
        );
        assert_eq!(
            UpdateChannel::from_tag("v1.0.0-rc.1"),
            UpdateChannel::Rc
        );
        // Unknown suffix: fall back to Stable (safe default)
        assert_eq!(
            UpdateChannel::from_tag("v1.0.0-unreleased.x"),
            UpdateChannel::Stable
        );
    }

    /// Verifies the channel ordering: Nightly < Weekly < Monthly < Alpha
    /// < Beta < Rc < Stable. This ordering is what allows the discovery
    /// filter (`tag_channel >= user_channel`) to surface Stable releases
    /// to a Beta-subscribed user without surfacing Beta releases to a
    /// Stable-subscribed user.
    #[test]
    fn test_update_channel_ordering() {
        assert!(UpdateChannel::Nightly < UpdateChannel::Weekly);
        assert!(UpdateChannel::Weekly < UpdateChannel::Monthly);
        assert!(UpdateChannel::Monthly < UpdateChannel::Alpha);
        assert!(UpdateChannel::Alpha < UpdateChannel::Beta);
        assert!(UpdateChannel::Beta < UpdateChannel::Rc);
        assert!(UpdateChannel::Rc < UpdateChannel::Stable);
    }

    /// A user subscribed to Beta should receive Beta, Rc, and Stable
    /// releases via the discovery filter (`tag_channel >= user_channel`)
    /// but NOT Alpha or below. A user subscribed to Stable receives
    /// only Stable.
    #[test]
    fn test_channel_filter_promotion() {
        let user = UpdateChannel::Beta;
        assert!(UpdateChannel::from_tag("v1.0.0") >= user);
        assert!(UpdateChannel::from_tag("v1.0.0-rc.1") >= user);
        assert!(UpdateChannel::from_tag("v1.0.0-beta.1") >= user);
        assert!(UpdateChannel::from_tag("v1.0.0-alpha.1") < user);
        assert!(UpdateChannel::from_tag("v1.0.0-nightly.20260101") < user);

        let user = UpdateChannel::Stable;
        assert!(UpdateChannel::from_tag("v1.0.0") >= user);
        assert!(UpdateChannel::from_tag("v1.0.0-rc.1") < user);
        assert!(UpdateChannel::from_tag("v1.0.0-beta.1") < user);
    }

    /// requires_dev_access() should return true only for the four channels
    /// hidden behind the Konami-code Dev Access gate.
    #[test]
    fn test_requires_dev_access() {
        assert!(UpdateChannel::Nightly.requires_dev_access());
        assert!(UpdateChannel::Weekly.requires_dev_access());
        assert!(UpdateChannel::Monthly.requires_dev_access());
        assert!(UpdateChannel::Alpha.requires_dev_access());
        assert!(!UpdateChannel::Beta.requires_dev_access());
        assert!(!UpdateChannel::Rc.requires_dev_access());
        assert!(!UpdateChannel::Stable.requires_dev_access());
    }

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

    /// `is_gamdl_compatible` is a thin alias over
    /// `gamdl_capabilities::should_offer_upgrade`, which now permits any
    /// parseable semver. The "untested vs tested" gate moved to
    /// `is_above_tested_ceiling` and the `ComponentUpdate.is_untested`
    /// field — that way users see new GAMDL releases as soon as upstream
    /// ships them instead of having to wait for a MeedyaDL audit + ceiling
    /// bump before the Updates page admits the upgrade.
    #[test]
    fn test_is_gamdl_compatible() {
        // Inside the validated window: surface as a normal upgrade.
        assert!(is_gamdl_compatible("2.9.1"));
        assert!(is_gamdl_compatible("2.9.2"));
        assert!(is_gamdl_compatible("3.0"));
        assert!(is_gamdl_compatible("3.0.0"));

        // Above the validated ceiling: STILL surfaced (the
        // `is_untested` flag carries the warning context to the UI).
        assert!(is_gamdl_compatible("99.0.0"));

        // Unparseable string: refuse to offer (we can't reason about it,
        // and downstream version comparisons would coerce it to 0.0.0).
        assert!(!is_gamdl_compatible("invalid"));
        assert!(!is_gamdl_compatible(""));
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

    /// Tests that get_updater_platform_key() returns a known Tauri updater
    /// platform key for the current compile target.
    #[test]
    fn test_get_updater_platform_key_returns_known_key() {
        let key = get_updater_platform_key();
        let known_keys = [
            "darwin-aarch64",
            "darwin-x86_64",
            "windows-x86_64",
            "windows-aarch64",
            "linux-x86_64",
            "linux-aarch64",
        ];
        assert!(
            known_keys.contains(&key),
            "Platform key '{key}' should be a known Tauri updater key"
        );
    }
}
