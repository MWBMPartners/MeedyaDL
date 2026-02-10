// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Update checker service.
// Checks for new versions of all application components: GAMDL (via PyPI),
// the app itself (via GitHub Releases), Python runtime, and external tools.
// Includes a compatibility gate so only known-compatible GAMDL versions
// are offered for upgrade.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::services::{gamdl_service, python_manager};
use crate::utils::platform;

// ============================================================
// Update status model
// ============================================================

/// Represents the update status of a single component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentUpdate {
    /// Human-readable component name (e.g., "GAMDL", "Python", "FFmpeg")
    pub name: String,
    /// Currently installed version (None if not installed)
    pub current_version: Option<String>,
    /// Latest available version (None if check failed)
    pub latest_version: Option<String>,
    /// Whether an update is available (latest > current)
    pub update_available: bool,
    /// Whether this update is compatible with the current app version
    pub is_compatible: bool,
    /// Human-readable description of the update (e.g., changelog summary)
    pub description: Option<String>,
    /// URL to the release page (for user reference)
    pub release_url: Option<String>,
}

/// Combined update status for all components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    /// Timestamp of the last check (ISO 8601)
    pub checked_at: String,
    /// Whether any updates are available
    pub has_updates: bool,
    /// Per-component update status
    pub components: Vec<ComponentUpdate>,
    /// Errors that occurred during the check (non-fatal)
    pub errors: Vec<String>,
}

// ============================================================
// GAMDL version compatibility
// ============================================================

/// Minimum GAMDL version known to be compatible with this app version.
/// Versions below this may have different CLI argument formats.
const MIN_COMPATIBLE_GAMDL: &str = "2.0.0";

/// Maximum GAMDL version known to be compatible (inclusive).
/// Set to a high version to allow future patches; update when breaking
/// changes are detected in GAMDL releases.
const MAX_COMPATIBLE_GAMDL: &str = "99.99.99";

/// Checks whether a GAMDL version is compatible with this app version.
///
/// This is a simple semver range check that prevents the user from
/// upgrading to a GAMDL version that may break the CLI interface.
///
/// # Arguments
/// * `version` - The GAMDL version string to check (e.g., "2.8.4")
fn is_gamdl_compatible(version: &str) -> bool {
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

    let Some(current) = parse(version) else {
        return false;
    };
    let Some(min) = parse(MIN_COMPATIBLE_GAMDL) else {
        return false;
    };
    let Some(max) = parse(MAX_COMPATIBLE_GAMDL) else {
        return false;
    };

    current >= min && current <= max
}

/// Compares two version strings and returns true if `latest` is newer than `current`.
fn is_newer(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<&str> = v.split('.').collect();
        let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };

    let c = parse(current);
    let l = parse(latest);
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

    // Check GAMDL updates (via PyPI)
    match check_gamdl_update(app).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("GAMDL check failed: {}", e)),
    }

    // Check app self-update (via GitHub Releases)
    match check_app_update(app).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("App update check failed: {}", e)),
    }

    // Check Python runtime update
    match check_python_update(app).await {
        Ok(update) => components.push(update),
        Err(e) => errors.push(format!("Python check failed: {}", e)),
    }

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
    // Get the currently installed GAMDL version
    let current = gamdl_service::get_gamdl_version(app)
        .await
        .unwrap_or(None);

    // Get the latest version from PyPI
    let latest = gamdl_service::check_latest_gamdl_version()
        .await
        .ok();

    // Determine if an update is available and compatible
    let update_available = match (&current, &latest) {
        (Some(c), Some(l)) => is_newer(c, l),
        (None, Some(_)) => true, // Not installed = "update" available
        _ => false,
    };

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
    // Get the current app version from Tauri's package info
    let current_version = app.package_info().version.to_string();

    // Query GitHub Releases API for the latest release
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.github.com/repos/MWBM-Partners-Ltd/gamdl-GUI/releases/latest")
        .header("User-Agent", "gamdl-gui")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if !response.status().is_success() {
        // 404 means no releases yet - not an error
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

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))?;

    // Extract the tag name (e.g., "v0.2.0") and strip the "v" prefix
    let tag = json["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();

    let html_url = json["html_url"].as_str().map(|s| s.to_string());
    let body = json["body"].as_str().map(|s| {
        // Truncate the release notes to a reasonable length for the UI
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
        is_compatible: true, // App updates are always "compatible" with themselves
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
    // Get the installed Python version
    let python_dir = platform::get_python_dir(app);
    let current = python_manager::get_installed_python_version(&python_dir).await;

    // The target version is the one defined in python_manager
    let target = python_manager::get_target_python_version();

    let update_available = match &current {
        Some(c) => is_newer(c, target),
        None => false, // Can't update what's not installed
    };

    Ok(ComponentUpdate {
        name: "Python Runtime".to_string(),
        current_version: current,
        latest_version: Some(target.to_string()),
        update_available,
        is_compatible: true,
        description: if update_available {
            Some(format!("Python {} available (portable runtime)", target))
        } else {
            None
        },
        release_url: Some(
            "https://github.com/indygreg/python-build-standalone/releases".to_string(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer() {
        assert!(is_newer("1.0.0", "1.0.1"));
        assert!(is_newer("1.0.0", "1.1.0"));
        assert!(is_newer("1.0.0", "2.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("2.0.0", "1.0.0"));
    }

    #[test]
    fn test_is_gamdl_compatible() {
        assert!(is_gamdl_compatible("2.8.4"));
        assert!(is_gamdl_compatible("2.0.0"));
        assert!(!is_gamdl_compatible("1.9.9"));
        assert!(!is_gamdl_compatible("invalid"));
    }
}
