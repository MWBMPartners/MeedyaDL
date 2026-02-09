// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// GAMDL download execution IPC commands.
// Handles starting downloads, cancelling active downloads, and querying
// the download queue status. Each download is executed by spawning a
// GAMDL CLI subprocess with the appropriate arguments.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::models::download::{DownloadRequest, QueueItemStatus};

/// Status of all items in the download queue.
#[derive(Debug, Clone, Serialize)]
pub struct QueueStatus {
    /// Total number of items in the queue (all states)
    pub total: usize,
    /// Number of items currently downloading
    pub active: usize,
    /// Number of items waiting to start
    pub queued: usize,
    /// Number of items that completed successfully
    pub completed: usize,
    /// Number of items that failed with errors
    pub failed: usize,
    /// Detailed status for each queue item
    pub items: Vec<QueueItemStatus>,
}

/// Starts a new download by adding it to the queue.
///
/// The download request includes the Apple Music URL(s) and optional
/// quality/format overrides. If no overrides are specified, the global
/// settings are used. The download is added to the queue and will be
/// processed when a slot becomes available.
///
/// Returns a unique download ID for tracking progress and cancellation.
///
/// # Arguments
/// * `request` - The download request containing URLs and option overrides
#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    request: DownloadRequest,
) -> Result<String, String> {
    // Generate a unique ID for this download
    let download_id = uuid::Uuid::new_v4().to_string();

    log::info!(
        "Download {} queued: {} URL(s)",
        download_id,
        request.urls.len()
    );

    // Notify the frontend that the download has been queued
    app.emit("download-queued", &download_id)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    // Spawn the GAMDL download in a background task so this command returns immediately
    let app_clone = app.clone();
    let dl_id = download_id.clone();
    let urls = request.urls.clone();
    let options = request.options.unwrap_or_default();

    tokio::spawn(async move {
        match crate::services::gamdl_service::run_gamdl(&app_clone, &dl_id, &urls, &options).await
        {
            Ok(()) => {
                log::info!("Download {} completed", dl_id);
                let _ = app_clone.emit("download-complete", &dl_id);
            }
            Err(e) => {
                log::error!("Download {} failed: {}", dl_id, e);
                let _ = app_clone.emit("download-error", serde_json::json!({
                    "download_id": dl_id,
                    "error": e,
                }));
            }
        }
    });

    Ok(download_id)
}

/// Cancels an active or queued download.
///
/// If the download is currently active, the GAMDL subprocess is killed.
/// If it's queued, it's removed from the queue. If it's already completed
/// or cancelled, this is a no-op.
///
/// # Arguments
/// * `download_id` - The unique ID returned by start_download
#[tauri::command]
pub async fn cancel_download(
    _app: AppHandle,
    download_id: String,
) -> Result<(), String> {
    log::info!("Cancel requested for download: {}", download_id);
    // TODO: Phase 4 - Implement download cancellation via the download queue service
    // The queue service will track active child processes and provide kill handles
    Err("Download cancellation not yet implemented (Phase 4)".to_string())
}

/// Returns the current status of all items in the download queue.
///
/// Used by the frontend to render the download queue UI with progress
/// bars, status indicators, and action buttons.
#[tauri::command]
pub async fn get_queue_status(_app: AppHandle) -> Result<QueueStatus, String> {
    // TODO: Phase 4 - Return actual queue status from download_queue service
    Ok(QueueStatus {
        total: 0,
        active: 0,
        queued: 0,
        completed: 0,
        failed: 0,
        items: Vec::new(),
    })
}

/// Checks the latest GAMDL version available on PyPI.
///
/// Used by the update checker to notify the user when a new GAMDL
/// version is available. Queries the PyPI JSON API.
///
/// # Returns
/// The latest version string (e.g., "2.8.4")
#[tauri::command]
pub async fn check_gamdl_update() -> Result<String, String> {
    crate::services::gamdl_service::check_latest_gamdl_version().await
}
