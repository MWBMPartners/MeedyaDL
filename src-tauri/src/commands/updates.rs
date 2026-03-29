// Copyright (c) 2026 MeedyaDL
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
// Emitter trait for sending events to the frontend (used for download progress).
use tauri::Emitter;

// config_service: loads user settings (including check_pre_releases preference)
// from the app data directory.
use crate::services::config_service;
// update_checker module contains the core update checking logic.
// ComponentUpdate: per-component update status (name, current version,
//   latest version, whether an update is available).
// UpdateCheckResult: aggregated result containing a list of ComponentUpdates
//   and a top-level `has_updates` flag for quick checking.
use crate::services::update_checker::{self, ComponentUpdate, UpdateCheckResult};
use crate::utils::activity_log::emit_app_log;

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
/// * `app` - Tauri `AppHandle` for accessing installed versions and Python path.
///
/// # Returns
/// * `Ok(UpdateCheckResult)` - Aggregated update status for all components.
///   The `has_updates` field provides a quick boolean check.
///   The `components` field contains per-component details including
///   current version, latest version, and whether an update is available.
///
/// # Errors
/// This function is infallible in practice -- individual component check failures
/// are captured per-component in the result rather than failing the entire operation.
/// Returns `Result` to satisfy the Tauri IPC command signature convention.
///
/// # Logging
/// Logs which components have updates available, or "all up to date"
/// if no updates are found.
#[tauri::command]
pub async fn check_all_updates(app: AppHandle) -> Result<UpdateCheckResult, String> {
    log::info!("Checking for updates...");
    emit_app_log(&app, "Checking for updates...");
    // Load settings to check the user's pre-release preference.
    // If settings fail to load, default to stable-only (check_pre_releases: false).
    let settings = config_service::load_settings(&app).unwrap_or_default();

    // check_all_updates() runs all component checks concurrently and
    // aggregates the results. Individual check failures are captured
    // per-component rather than failing the entire operation.
    let result = update_checker::check_all_updates(&app, settings.check_pre_releases).await;

    // Log the result for debugging — list components with available updates
    if result.has_updates {
        let names = result
            .components
            .iter()
            .filter(|c| c.update_available)
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        log::info!("Updates available for: {names}");
        emit_app_log(&app, &format!("Updates available: {names}"));
    } else {
        log::info!("All components are up to date");
        emit_app_log(&app, "All components up to date");
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
/// * `app` - Tauri `AppHandle` for locating the Python/pip binaries.
///
/// # Returns
/// * `Ok(String)` - The new version string after upgrade (e.g., "2.9.0").
/// * `Err(String)` - pip upgrade failure message (network error, dependency conflict, etc.).
///
/// # Errors
/// Returns an error if the pip upgrade subprocess fails (network timeout,
/// dependency conflict, disk full, missing Python runtime, etc.).
#[tauri::command]
pub async fn upgrade_gamdl(app: AppHandle) -> Result<String, String> {
    log::info!("Upgrading GAMDL...");
    emit_app_log(&app, "Upgrading GAMDL...");
    // Reuses install_gamdl() which runs `pip install --upgrade gamdl`.
    // The --upgrade flag ensures pip upgrades to the latest version if
    // an older version is already installed.
    let version = crate::services::gamdl_service::install_gamdl(&app).await?;
    log::info!("GAMDL upgraded to {version}");
    emit_app_log(&app, &format!("GAMDL upgraded to v{version}"));
    Ok(version)
}

/// Upgrades any pip-based engine to the latest version.
///
/// **Frontend caller:** `upgradePipEngine(package)` in `src/lib/tauri-commands.ts`
///
/// Uses `pip install --upgrade` to update the package. Works for any
/// pip engine: votify, yt-dlp, ofscraper, etc. GAMDL has its own
/// upgrade path via `upgrade_gamdl()` with compatibility gating.
#[tauri::command]
pub async fn upgrade_pip_engine(app: AppHandle, package: String) -> Result<String, String> {
    log::info!("Upgrading {package}...");
    emit_app_log(&app, &format!("Upgrading {package}..."));
    let version =
        crate::services::pip_engine_service::install_pip_engine(&app, &package).await?;
    log::info!("{package} upgraded to {version}");
    emit_app_log(&app, &format!("{package} upgraded to v{version}"));
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
/// * `app` - Tauri `AppHandle` for version detection.
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
/// # Errors
/// Returns an error if no component name matches the given search string
/// (case-insensitive substring match).
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
    // Load settings for the pre-release preference, then run all update checks
    // (currently no way to check individual components independently)
    let settings = config_service::load_settings(&app).unwrap_or_default();
    let result = update_checker::check_all_updates(&app, settings.check_pre_releases).await;

    // Find the component whose name contains the search string (case-insensitive).
    // into_iter() consumes the Vec, avoiding cloning the ComponentUpdate structs.
    result
        .components
        .into_iter()
        .find(|c| c.name.to_lowercase().contains(&name.to_lowercase()))
        .ok_or_else(|| format!("Unknown component: {name}"))
}

/// Downloads and installs a `MeedyaDL` app update from GitHub Releases.
///
/// **Frontend caller:** `downloadAndInstallAppUpdate(tag)` in `src/lib/tauri-commands.ts`
///
/// Uses the Tauri updater plugin's Rust API (`UpdaterExt`) to download a
/// signed update binary from a specific GitHub release tag. The updater
/// verifies the binary's cryptographic signature before applying the update.
///
/// The endpoint URL is constructed dynamically from the tag to support both
/// stable and pre-release channels:
///   `https://github.com/MWBMPartners/MeedyaDL/releases/download/{tag}/latest.json`
///
/// During the download, progress events are emitted to the frontend:
///   - `app-update-progress` with `{ event: "started", total }` — download started
///   - `app-update-progress` with `{ event: "progress", chunk_length }` — chunk received
///   - `app-update-progress` with `{ event: "finished" }` — download complete
///
/// After the download completes and the update is applied, the user should
/// restart the application (via `relaunch()` from `@tauri-apps/plugin-process`)
/// to load the new version.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for accessing the updater plugin
/// * `tag` - Git tag of the release to install (e.g., "v0.3.7")
///
/// # Returns
/// * `Ok(String)` - Success message indicating the update was installed
/// * `Err(String)` - Error message if download, verification, or installation failed
///
/// # Errors
/// Returns an error if:
/// - The endpoint URL cannot be parsed.
/// - The updater builder fails to initialize.
/// - The `latest.json` manifest cannot be downloaded or parsed.
/// - No update is found at the specified release tag.
/// - The update binary download, signature verification, or installation fails.
///
/// # Prerequisites
/// - The release must contain a `latest.json` manifest and signed update artifacts
/// - The app must be built with a `TAURI_SIGNING_PRIVATE_KEY` for signature generation
/// - The public key must be configured in `tauri.conf.json` → `plugins.updater.pubkey`
#[tauri::command]
pub async fn download_and_install_app_update(
    app: AppHandle,
    tag: String,
) -> Result<String, String> {
    // Import UpdaterExt at the top of the function body to satisfy
    // clippy::items_after_statements (items must precede statements).
    // `UpdaterExt` trait provides `updater_builder()` on the AppHandle.
    use tauri_plugin_updater::UpdaterExt;

    log::info!("Downloading app update from tag: {tag}");
    emit_app_log(&app, &format!("Downloading app update {tag}..."));

    // Construct the endpoint URL for the specific release tag.
    // Each GitHub Release contains a `latest.json` manifest that describes
    // the available update binaries and their signatures for each platform.
    let endpoint_url =
        format!("https://github.com/MWBMPartners/MeedyaDL/releases/download/{tag}/latest.json");

    // Parse the endpoint URL and build the updater with the custom endpoint.
    // `endpoints()` returns a Result (validates the URLs), so we unwrap before `.build()`.
    let endpoint = endpoint_url
        .parse()
        .map_err(|e: url::ParseError| format!("Invalid endpoint URL: {e}"))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("Failed to set updater endpoints: {e}"))?
        .build()
        .map_err(|e| format!("Failed to build updater: {e}"))?;

    // Check if an update is available at the specified endpoint.
    // This downloads and parses the `latest.json` manifest.
    let update: Option<tauri_plugin_updater::Update> = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for update: {e}"))?;

    let Some(update) = update else {
        return Err("No update found at the specified release tag".to_string());
    };

    log::info!(
        "Update found: {} -> {}",
        update.current_version,
        update.version
    );

    // Clone the app handle for use in the progress callback closures.
    let app_for_progress = app.clone();

    // Download and install the update with progress reporting.
    // The download callback is called for each chunk received, allowing
    // the frontend to display a progress bar. Explicit closure parameter
    // types are required for Rust type inference.
    update
        .download_and_install(
            // Progress callback: called for each downloaded chunk
            move |chunk_length: usize, content_length: Option<u64>| {
                // Emit a progress event to the frontend for the download progress bar.
                // `content_length` is the total download size (if known from Content-Length header).
                // `chunk_length` is the number of bytes in this chunk.
                let _ = app_for_progress.emit(
                    "app-update-progress",
                    serde_json::json!({
                        "event": "progress",
                        "chunk_length": chunk_length,
                        "content_length": content_length,
                    }),
                );
            },
            // Finished callback: called when the download is complete
            || {
                log::info!("App update download complete, installing...");
            },
        )
        .await
        .map_err(|e| format!("Failed to download and install update: {e}"))?;

    log::info!("App update installed successfully. Restart required.");
    emit_app_log(&app, "App update installed — restart required");

    // Emit a final "finished" event so the frontend can show a "Restart Now" button
    let _ = app.emit(
        "app-update-progress",
        serde_json::json!({ "event": "finished" }),
    );

    Ok("Update installed successfully. Restart the application to apply.".to_string())
}
