// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// GAMDL download execution IPC commands.
// Handles starting downloads, cancelling active downloads, retrying
// failed downloads, clearing completed items, and querying the download
// queue status. Downloads are managed by the download_queue service
// which handles concurrent execution, fallback quality, and retries.
//
// ## Architecture
//
// These are Tauri IPC command handlers — the bridge between the React/TypeScript
// frontend and the Rust backend. Each `#[tauri::command]` function is callable
// from the frontend via `invoke()` in `src/lib/tauri-commands.ts`.
//
// The download lifecycle is:
//   1. Frontend calls `startDownload(request)` -> `start_download()`
//   2. Download is enqueued in the `QueueHandle` (a Tokio Mutex-wrapped queue)
//   3. `process_queue()` picks up the item and spawns a GAMDL subprocess
//   4. Frontend polls `getQueueStatus()` -> `get_queue_status()` for progress
//   5. Frontend can cancel via `cancelDownload(id)` -> `cancel_download()`
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command         | TypeScript Function    | Line |
// |----------------------|------------------------|------|
// | start_download       | startDownload()        | ~99  |
// | cancel_download      | cancelDownload()       | ~104 |
// | retry_download       | retryDownload()        | ~109 |
// | clear_queue          | clearQueue()           | ~114 |
// | get_queue_status     | getQueueStatus()       | ~119 |
// | check_gamdl_update   | checkGamdlUpdate()     | ~124 |
//
// ## References
//
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - Tauri State management: https://v2.tauri.app/develop/state-management/
// - Tauri Events (emit): https://v2.tauri.app/develop/calling-frontend/

// serde::Serialize is required for any struct returned to the frontend — Tauri
// serializes return values to JSON before sending them over the IPC bridge.
use serde::Serialize;
// AppHandle provides access to Tauri's managed state and app-level APIs.
// Emitter allows sending events from Rust to the frontend (e.g., "download-queued").
// State<'_, T> is Tauri's dependency injection for managed state (see main.rs setup).
use tauri::{AppHandle, Emitter, State};

// DownloadRequest: the deserialized JSON payload from the frontend containing
// URLs and optional per-download quality/format overrides.
// QueueItemStatus: per-item status info (id, state, progress, error message).
use crate::models::download::{DownloadRequest, QueueItemStatus};
// download_queue module contains the queue processing logic (process_queue).
// QueueHandle is an Arc<Mutex<DownloadQueue>> shared across all command invocations.
use crate::services::download_queue::{self, QueueHandle};

/// Status of all items in the download queue.
///
/// This struct is serialized to JSON and returned to the frontend by
/// `get_queue_status()`. The frontend uses it to render the download queue
/// UI panel with progress bars, status badges, and action buttons.
///
/// Implements `Serialize` (required by Tauri for IPC return values).
/// See: https://v2.tauri.app/develop/calling-rust/#return-types
#[derive(Debug, Clone, Serialize)]
pub struct QueueStatus {
    /// Total number of items in the queue (all states combined)
    pub total: usize,
    /// Number of items currently downloading (state == Active)
    pub active: usize,
    /// Number of items waiting to start (state == Queued)
    pub queued: usize,
    /// Number of items that completed successfully (state == Completed)
    pub completed: usize,
    /// Number of items that failed with errors (state == Failed)
    pub failed: usize,
    /// Detailed status for each queue item, including per-item progress,
    /// error messages, and the original download request parameters.
    pub items: Vec<QueueItemStatus>,
}

/// Starts a new download by adding it to the queue.
///
/// **Frontend caller:** `startDownload(request)` in `src/lib/tauri-commands.ts`
///
/// The download request includes the Apple Music URL(s) and optional
/// quality/format overrides. If no overrides are specified, the global
/// settings are used. The download is added to the queue and will be
/// processed when a slot becomes available (default: 1 concurrent).
///
/// Returns a unique download ID (UUID) for tracking progress and cancellation.
///
/// # Arguments
/// * `app` - Tauri AppHandle, injected automatically by the IPC runtime.
///   Used to access managed state, emit events, and resolve paths.
/// * `queue` - The download queue state, injected via `State<'_, QueueHandle>`.
///   This is a Tokio Mutex-wrapped `DownloadQueue` registered in `main.rs`.
///   See: https://v2.tauri.app/develop/state-management/
/// * `request` - The download request payload deserialized from the frontend JSON.
///   Contains `urls: Vec<String>` and optional override fields.
///
/// # Returns
/// * `Ok(String)` - The unique download ID (UUID v4) assigned to this download.
/// * `Err(String)` - Human-readable error message if the event emission fails.
///
/// # Events Emitted
/// * `"download-queued"` - Emitted with the download ID after successful enqueue.
///   The frontend listens for this to update the queue UI immediately.
///   See: https://v2.tauri.app/develop/calling-frontend/
#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    request: DownloadRequest,
) -> Result<String, String> {
    // Load current settings for merging with per-download overrides.
    // If settings can't be loaded (corrupted file, etc.), fall back to defaults
    // so the download can still proceed with sensible quality/format choices.
    let settings = crate::services::config_service::load_settings(&app)
        .unwrap_or_default();

    // Acquire the queue lock and enqueue the download. The lock is scoped
    // to this block to release it before the async process_queue() call,
    // avoiding potential deadlocks.
    let download_id = {
        let mut q = queue.lock().await;
        q.enqueue(request, &settings)
    };

    log::info!("Download {} queued", download_id);

    // Emit a Tauri event to notify the frontend that the download has been queued.
    // The frontend listens for "download-queued" events to refresh the queue UI.
    app.emit("download-queued", &download_id)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    // Clone the Arc<Mutex<...>> handle so we can pass it to process_queue().
    // State::inner() returns a reference to the inner Arc, which we clone.
    let queue_handle = queue.inner().clone();
    // Trigger queue processing — this will start the download immediately if
    // there are available concurrency slots, or leave it queued for later.
    download_queue::process_queue(app, queue_handle).await;

    Ok(download_id)
}

/// Cancels an active or queued download.
///
/// **Frontend caller:** `cancelDownload(downloadId)` in `src/lib/tauri-commands.ts`
///
/// If the download is currently active, the GAMDL subprocess is killed via
/// its stored `Child` process handle. If it's still queued (not yet started),
/// it's moved directly to the Cancelled state without ever spawning a process.
///
/// # Arguments
/// * `app` - Tauri AppHandle for event emission.
/// * `queue` - Managed download queue state.
/// * `download_id` - The unique ID (UUID) returned by `start_download`.
///   The frontend passes this as `downloadId` (camelCase) and Tauri
///   automatically converts it to `download_id` (snake_case).
///   See: https://v2.tauri.app/develop/calling-rust/#command-arguments
///
/// # Returns
/// * `Ok(())` - The download was successfully cancelled.
/// * `Err(String)` - The download ID was not found or the item already finished.
///
/// # Events Emitted
/// * `"download-cancelled"` - Emitted with the download ID on successful cancellation.
#[tauri::command]
pub async fn cancel_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Cancel requested for download: {}", download_id);

    // Acquire lock, attempt cancellation, then release lock.
    // q.cancel() returns true if the item was found and successfully cancelled.
    let cancelled = {
        let mut q = queue.lock().await;
        q.cancel(&download_id)
    };

    if cancelled {
        // Notify the frontend so it can update the item's UI state immediately.
        // We use `let _ =` to ignore emission errors — the cancellation itself
        // already succeeded, so a failed event is non-critical.
        let _ = app.emit("download-cancelled", &download_id);
        Ok(())
    } else {
        // The download ID was not found, or the item has already completed/failed.
        Err(format!("Download {} not found or already finished", download_id))
    }
}

/// Retries a failed or cancelled download.
///
/// **Frontend caller:** `retryDownload(downloadId)` in `src/lib/tauri-commands.ts`
///
/// Resets the download item to the Queued state with freshly-loaded settings
/// (in case the user changed quality/format options since the original attempt)
/// and triggers queue processing to start it.
///
/// # Arguments
/// * `app` - Tauri AppHandle for settings access and event emission.
/// * `queue` - Managed download queue state.
/// * `download_id` - The unique ID of the failed/cancelled download to retry.
///
/// # Returns
/// * `Ok(())` - The download was reset to Queued and queue processing triggered.
/// * `Err(String)` - The download ID was not found, or the item is in a state
///   that cannot be retried (e.g., currently active or already completed).
///
/// # Events Emitted
/// * `"download-queued"` - Emitted with the download ID after successful re-queue.
#[tauri::command]
pub async fn retry_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Retry requested for download: {}", download_id);

    // Re-load settings so retries pick up any changes the user made
    // (e.g., switching from AAC to ALAC after a failed attempt).
    let settings = crate::services::config_service::load_settings(&app)
        .unwrap_or_default();

    // Attempt to reset the download item to Queued state.
    // q.retry() returns true only if the item exists and is in a retryable state
    // (Failed or Cancelled).
    let retried = {
        let mut q = queue.lock().await;
        q.retry(&download_id, &settings)
    };

    if retried {
        // Notify frontend and kick off queue processing, same as start_download()
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
/// **Frontend caller:** `clearQueue()` in `src/lib/tauri-commands.ts`
///
/// Removes all items whose state is Completed, Failed, or Cancelled,
/// leaving only Active and Queued items. This is typically called when
/// the user clicks "Clear Finished" in the download queue panel.
///
/// # Arguments
/// * `queue` - Managed download queue state (injected by Tauri).
///
/// # Returns
/// * `Ok(usize)` - The number of items that were removed from the queue.
#[tauri::command]
pub async fn clear_queue(
    queue: State<'_, QueueHandle>,
) -> Result<usize, String> {
    let mut q = queue.lock().await;
    // clear_finished() drains all terminal-state items and returns the count
    Ok(q.clear_finished())
}

/// Returns the current status of all items in the download queue.
///
/// **Frontend caller:** `getQueueStatus()` in `src/lib/tauri-commands.ts`
///
/// Used by the frontend to render the download queue UI with progress
/// bars, status indicators, and action buttons. The frontend typically
/// polls this command on an interval (or after receiving a download event)
/// to keep the UI synchronized with the backend state.
///
/// # Arguments
/// * `queue` - Managed download queue state (injected by Tauri).
///
/// # Returns
/// * `Ok(QueueStatus)` - Aggregated counts plus per-item status details.
///   This struct is serialized to JSON by Tauri's IPC layer.
#[tauri::command]
pub async fn get_queue_status(
    queue: State<'_, QueueHandle>,
) -> Result<QueueStatus, String> {
    let q = queue.lock().await;
    // get_counts() returns a tuple of (total, active, queued, completed, failed)
    let (total, active, queued, completed, failed) = q.get_counts();
    // get_status() returns a Vec<QueueItemStatus> with per-item details
    let items = q.get_status();

    // Assemble and return the complete queue snapshot
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
/// **Frontend caller:** `checkGamdlUpdate()` in `src/lib/tauri-commands.ts`
///
/// Used by the update checker to notify the user when a new GAMDL
/// version is available. Queries the PyPI JSON API at:
///   https://pypi.org/pypi/gamdl/json
///
/// This command takes no parameters because it only needs network access.
/// It does not require the `AppHandle` or `State` since it doesn't access
/// any local state or managed resources.
///
/// # Returns
/// * `Ok(String)` - The latest version string (e.g., "2.8.4").
/// * `Err(String)` - Network error or PyPI API parsing failure.
#[tauri::command]
pub async fn check_gamdl_update() -> Result<String, String> {
    // Delegates to the gamdl_service which handles the HTTP request and
    // JSON parsing of the PyPI API response.
    crate::services::gamdl_service::check_latest_gamdl_version().await
}
