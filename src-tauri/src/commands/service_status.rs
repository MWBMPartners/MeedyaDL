// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Service status IPC command.
//
// Exposes the service status checker to the frontend so it can fetch
// the remote enable/disable configuration for each media service.
// Called on app startup and periodically (every 4 hours) by the frontend.
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command          | TypeScript Function       |
// |-----------------------|---------------------------|
// | check_service_status  | checkServiceStatus()      |

use tauri::AppHandle;

use crate::models::service_status::ServiceStatusConfig;
use crate::services::service_status;

/// Fetches the current service status configuration.
///
/// **Frontend caller:** `checkServiceStatus()` in `src/lib/tauri-commands.ts`
///
/// Returns the service status from the remote endpoint, local cache,
/// or all-enabled default (in that priority order). The fail-open design
/// ensures the app always returns a usable config.
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving the app data directory.
///
/// # Returns
/// * `Ok(ServiceStatusConfig)` - The resolved service status configuration.
/// * `Err(String)` - Unexpected error (should not occur due to fail-open).
#[tauri::command]
pub async fn check_service_status(app: AppHandle) -> Result<ServiceStatusConfig, String> {
    log::info!("Checking service status...");
    service_status::fetch_service_status(&app).await
}
