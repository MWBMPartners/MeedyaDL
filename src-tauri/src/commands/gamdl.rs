// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// GAMDL download execution IPC commands.
// Handles starting downloads, cancelling active downloads, retrying
// failed downloads, clearing completed items, and querying the download
// queue status. Downloads are managed by the download_queue service
// which handles concurrent execution, fallback quality, and retries.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::models::download::{DownloadRequest, QueueItemStatus};
use crate::services::download_queue::{self, QueueHandle};

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
/// processed when a slot becomes available (default: 1 concurrent).
///
/// Returns a unique download ID for tracking progress and cancellation.
///
/// # Arguments
/// * `request` - The download request containing URLs and option overrides
#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    request: DownloadRequest,
) -> Result<String, String> {
    // Load current settings for merging with per-download overrides
    let settings = crate::services::config_service::load_settings(&app)
        .unwrap_or_default();

    // Enqueue the download
    let download_id = {
        let mut q = queue.lock().await;
        q.enqueue(request, &settings)
    };

    log::info!("Download {} queued", download_id);

    // Notify the frontend that the download has been queued
    app.emit("download-queued", &download_id)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    // Trigger queue processing (will start the download if a slot is available)
    let queue_handle = queue.inner().clone();
    download_queue::process_queue(app, queue_handle).await;

    Ok(download_id)
}

/// Cancels an active or queued download.
///
/// If the download is currently active, the GAMDL subprocess is killed.
/// If it's queued, it's moved to the Cancelled state.
///
/// # Arguments
/// * `download_id` - The unique ID returned by start_download
#[tauri::command]
pub async fn cancel_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Cancel requested for download: {}", download_id);

    let cancelled = {
        let mut q = queue.lock().await;
        q.cancel(&download_id)
    };

    if cancelled {
        let _ = app.emit("download-cancelled", &download_id);
        Ok(())
    } else {
        Err(format!("Download {} not found or already finished", download_id))
    }
}

/// Retries a failed or cancelled download.
///
/// Resets the download to the Queued state with fresh options and
/// triggers queue processing.
///
/// # Arguments
/// * `download_id` - The unique ID of the download to retry
#[tauri::command]
pub async fn retry_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Retry requested for download: {}", download_id);

    let settings = crate::services::config_service::load_settings(&app)
        .unwrap_or_default();

    let retried = {
        let mut q = queue.lock().await;
        q.retry(&download_id, &settings)
    };

    if retried {
        let _ = app.emit("download-queued", &download_id);
        let queue_handle = queue.inner().clone();
        download_queue::process_queue(app, queue_handle).await;
        Ok(())
    } else {
        Err(format!("Download {} cannot be retried", download_id))
    }
}

/// Clears all completed, failed, and cancelled items from the queue.
///
/// Returns the number of items removed.
#[tauri::command]
pub async fn clear_queue(
    queue: State<'_, QueueHandle>,
) -> Result<usize, String> {
    let mut q = queue.lock().await;
    Ok(q.clear_finished())
}

/// Returns the current status of all items in the download queue.
///
/// Used by the frontend to render the download queue UI with progress
/// bars, status indicators, and action buttons.
#[tauri::command]
pub async fn get_queue_status(
    queue: State<'_, QueueHandle>,
) -> Result<QueueStatus, String> {
    let q = queue.lock().await;
    let (total, active, queued, completed, failed) = q.get_counts();
    let items = q.get_status();

    Ok(QueueStatus {
        total,
        active,
        queued,
        completed,
        failed,
        items,
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
