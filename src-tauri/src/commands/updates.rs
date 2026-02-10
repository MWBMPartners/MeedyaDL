// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Update checking IPC commands.
// Exposes the update_checker service to the frontend for checking
// component versions, upgrading GAMDL, and dismissing update notifications.

use tauri::AppHandle;

use crate::services::update_checker::{self, ComponentUpdate, UpdateCheckResult};

/// Checks for updates to all application components.
///
/// Returns the combined update status for GAMDL, the app, Python,
/// and external tools. Non-fatal errors are included in the result.
///
/// The frontend calls this on startup (if auto_check_updates is enabled)
/// and when the user manually triggers an update check.
#[tauri::command]
pub async fn check_all_updates(app: AppHandle) -> Result<UpdateCheckResult, String> {
    log::info!("Checking for updates...");
    let result = update_checker::check_all_updates(&app).await;

    if result.has_updates {
        log::info!(
            "Updates available for: {}",
            result
                .components
                .iter()
                .filter(|c| c.update_available)
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    } else {
        log::info!("All components are up to date");
    }

    Ok(result)
}

/// Upgrades GAMDL to the latest compatible version via pip.
///
/// Runs `pip install --upgrade gamdl` using the managed Python runtime.
/// Returns the new version string on success.
#[tauri::command]
pub async fn upgrade_gamdl(app: AppHandle) -> Result<String, String> {
    log::info!("Upgrading GAMDL...");
    let version = crate::services::gamdl_service::install_gamdl(&app).await?;
    log::info!("GAMDL upgraded to {}", version);
    Ok(version)
}

/// Returns the update status for a specific component by name.
///
/// Useful for checking a single component without running all checks.
///
/// # Arguments
/// * `name` - Component name ("gamdl", "app", "python")
#[tauri::command]
pub async fn check_component_update(
    app: AppHandle,
    name: String,
) -> Result<ComponentUpdate, String> {
    let result = update_checker::check_all_updates(&app).await;

    result
        .components
        .into_iter()
        .find(|c| c.name.to_lowercase().contains(&name.to_lowercase()))
        .ok_or_else(|| format!("Unknown component: {}", name))
}
