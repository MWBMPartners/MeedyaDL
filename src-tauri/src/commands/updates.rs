// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Update checking IPC commands.
// Exposes the update_checker service to the frontend for checking
// component versions, upgrading GAMDL, and dismissing update notifications.
//
// ## Architecture
//
// The update system checks multiple components for available updates:
//   1. **GAMDL** - Checked against PyPI (https://pypi.org/pypi/gamdl/json)
//   2. **App** - Checked against GitHub releases (future: Tauri updater)
//   3. **Python** - Checked against python-build-standalone releases
//
// The frontend calls `check_all_updates()` on app startup (if the user has
// `auto_check_updates` enabled in settings) and when the user manually
// triggers an update check from the settings or About page.
//
// GAMDL upgrades are performed in-place via `pip install --upgrade gamdl`
// using the managed Python runtime. App updates will use Tauri's built-in
// updater in a future release.
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command            | TypeScript Function         | Line |
// |-------------------------|-----------------------------|------|
// | check_all_updates       | checkAllUpdates()           | ~152 |
// | upgrade_gamdl           | upgradeGamdl()              | ~157 |
// | check_component_update  | checkComponentUpdate(name)  | ~162 |
//
// ## References
//
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - PyPI JSON API: https://wiki.python.org/moin/PyPIJSON

// AppHandle for accessing the managed Python runtime path (needed for
// version detection) and app configuration (for installed version comparison).
use tauri::AppHandle;

// update_checker module contains the core update checking logic.
// ComponentUpdate: per-component update status (name, current version,
//   latest version, whether an update is available).
// UpdateCheckResult: aggregated result containing a list of ComponentUpdates
//   and a top-level `has_updates` flag for quick checking.
use crate::services::update_checker::{self, ComponentUpdate, UpdateCheckResult};

/// Checks for updates to all application components.
///
/// **Frontend caller:** `checkAllUpdates()` in `src/lib/tauri-commands.ts`
///
/// Returns the combined update status for GAMDL, the app, Python,
/// and external tools. Non-fatal errors (e.g., network timeout for one
/// component) are included in the result per-component rather than
/// failing the entire check.
///
/// The frontend calls this:
/// - On app startup, if `auto_check_updates` is enabled in settings
/// - When the user manually clicks "Check for Updates" in the settings page
///
/// # Arguments
/// * `app` - Tauri AppHandle for accessing installed versions and Python path.
///
/// # Returns
/// * `Ok(UpdateCheckResult)` - Aggregated update status for all components.
///   The `has_updates` field provides a quick boolean check.
///   The `components` field contains per-component details including
///   current version, latest version, and whether an update is available.
///
/// # Logging
/// Logs which components have updates available, or "all up to date"
/// if no updates are found.
#[tauri::command]
pub async fn check_all_updates(app: AppHandle) -> Result<UpdateCheckResult, String> {
    log::info!("Checking for updates...");
    // check_all_updates() runs all component checks concurrently and
    // aggregates the results. Individual check failures are captured
    // per-component rather than failing the entire operation.
    let result = update_checker::check_all_updates(&app).await;

    // Log the result for debugging — list components with available updates
    if result.has_updates {
        log::info!(
            "Updates available for: {}",
            result
                .components
                .iter()
                .filter(|c| c.update_available) // Only include components with updates
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(", ") // e.g., "GAMDL, Python"
        );
    } else {
        log::info!("All components are up to date");
    }

    Ok(result)
}

/// Upgrades GAMDL to the latest compatible version via pip.
///
/// **Frontend caller:** `upgradeGamdl()` in `src/lib/tauri-commands.ts`
///
/// Runs `pip install --upgrade gamdl` using the managed Python runtime.
/// This reuses the same `install_gamdl()` service function used during
/// initial setup — pip's `--upgrade` flag handles both fresh installs
/// and upgrades seamlessly.
///
/// This is a long-running operation (network download + pip install).
/// The frontend shows a loading/progress indicator while awaiting the result.
///
/// # Arguments
/// * `app` - Tauri AppHandle for locating the Python/pip binaries.
///
/// # Returns
/// * `Ok(String)` - The new version string after upgrade (e.g., "2.9.0").
/// * `Err(String)` - pip upgrade failure message (network error, dependency conflict, etc.).
#[tauri::command]
pub async fn upgrade_gamdl(app: AppHandle) -> Result<String, String> {
    log::info!("Upgrading GAMDL...");
    // Reuses install_gamdl() which runs `pip install --upgrade gamdl`.
    // The --upgrade flag ensures pip upgrades to the latest version if
    // an older version is already installed.
    let version = crate::services::gamdl_service::install_gamdl(&app).await?;
    log::info!("GAMDL upgraded to {}", version);
    Ok(version)
}

/// Returns the update status for a specific component by name.
///
/// **Frontend caller:** `checkComponentUpdate(name)` in `src/lib/tauri-commands.ts`
///
/// Useful for checking a single component without the frontend needing
/// to parse the full `UpdateCheckResult`. Currently this still runs all
/// checks internally (via `check_all_updates`) and filters the result,
/// so it doesn't save network calls — but it simplifies the frontend API.
///
/// # Arguments
/// * `app` - Tauri AppHandle for version detection.
/// * `name` - Component name to look up. Uses case-insensitive substring
///   matching, so "gamdl", "GAMDL", or "Gamdl" all work. Valid values:
///   - `"gamdl"` - The GAMDL Python package
///   - `"app"` - This desktop application
///   - `"python"` - The managed Python runtime
///
/// # Returns
/// * `Ok(ComponentUpdate)` - Update status for the matched component.
/// * `Err(String)` - If no component name matches the given string.
///
/// # Note
/// The name matching uses `contains()` rather than exact equality, so
/// "gamdl" would also match a hypothetical "gamdl-extras" component.
/// This is intentional for flexibility but could be tightened if needed.
#[tauri::command]
pub async fn check_component_update(
    app: AppHandle,
    name: String,
) -> Result<ComponentUpdate, String> {
    // Run all update checks (currently no way to check individual components)
    let result = update_checker::check_all_updates(&app).await;

    // Find the component whose name contains the search string (case-insensitive).
    // into_iter() consumes the Vec, avoiding cloning the ComponentUpdate structs.
    result
        .components
        .into_iter()
        .find(|c| c.name.to_lowercase().contains(&name.to_lowercase()))
        .ok_or_else(|| format!("Unknown component: {}", name))
}
