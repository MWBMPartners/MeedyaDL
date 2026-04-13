// Copyright (c) 2026 MeedyaDL
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
// | Rust Command          | TypeScript Function        | Line |
// |-----------------------|----------------------------|------|
// | start_download        | startDownload()            | ~99  |
// | cancel_download       | cancelDownload()           | ~104 |
// | retry_download        | retryDownload()            | ~109 |
// | clear_queue           | clearQueue()               | ~114 |
// | get_queue_status      | getQueueStatus()           | ~119 |
// | check_gamdl_update    | checkGamdlUpdate()         | ~124 |
// | export_activity_log   | exportActivityLog(entries)  |      |
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
use crate::models::download::{DownloadRequest, QueueItemStatus, StartDownloadResult};
// download_queue module contains the queue processing logic (process_queue).
// QueueHandle is an Arc<Mutex<DownloadQueue>> shared across all command invocations.
use crate::services::download_queue::{self, QueueHandle};
use crate::utils::activity_log::{emit_app_log, emit_verbose_app_log};

/// Status of all items in the download queue.
///
/// This struct is serialized to JSON and returned to the frontend by
/// `get_queue_status()`. The frontend uses it to render the download queue
/// UI panel with progress bars, status badges, and action buttons.
///
/// Implements `Serialize` (required by Tauri for IPC return values).
/// See: <https://v2.tauri.app/develop/calling-rust/#return-types>
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
/// * `app` - Tauri `AppHandle`, injected automatically by the IPC runtime.
///   Used to access managed state, emit events, and resolve paths.
/// * `queue` - The download queue state, injected via `State<'_, QueueHandle>`.
///   This is a Tokio Mutex-wrapped `DownloadQueue` registered in `main.rs`.
///   See: <https://v2.tauri.app/develop/state-management>/
/// * `request` - The download request payload deserialized from the frontend JSON.
///   Contains `urls: Vec<String>` and optional override fields.
/// * `skip_auto_start` - When `Some(true)`, the item is queued but `process_queue()`
///   is NOT called, regardless of the `auto_start_queue` setting. Used by the
///   frontend when the device is offline — the item sits in `Queued` state until
///   the user manually starts the queue or a future online download triggers processing.
///
/// # Returns
/// * `Ok(StartDownloadResult)` - Contains the download ID (UUID v4) and an optional
///   `duplicate_warning` message when the URL is already in the active queue.
/// * `Err(String)` - Human-readable error message if the event emission fails.
///
/// # Errors
/// Returns an error if emitting the `"download-queued"` event to the frontend fails.
///
/// # Events Emitted
/// * `"download-queued"` - Emitted with the download ID after successful enqueue.
///   The frontend listens for this to update the queue UI immediately.
///   See: <https://v2.tauri.app/develop/calling-frontend>/
#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    request: DownloadRequest,
    skip_auto_start: Option<bool>,
) -> Result<StartDownloadResult, String> {
    // Rate limit: max 10 downloads per minute
    crate::utils::rate_limiter::check_rate_limit("start_download", 10, 60)?;

    // Load current settings for merging with per-download overrides.
    // If settings can't be loaded (corrupted file, etc.), fall back to defaults
    // so the download can still proceed with sensible quality/format choices.
    let settings = crate::services::config_service::load_settings(&app).unwrap_or_default();

    // Verbose: log key settings for debugging
    emit_verbose_app_log(
        &app,
        &format!(
            "Download settings: codec={:?}, wrapper={}, cookies={}, output={}",
            settings.default_song_codec,
            settings.use_wrapper,
            settings.cookies_path.as_deref().unwrap_or("(none)"),
            settings.output_path,
        ),
    );

    // Normalize non-geographic Apple Music URLs by injecting a storefront
    // code. GAMDL requires a 2-letter storefront in the URL path (e.g., /us/).
    // MeedyaDL detects URLs missing a storefront and injects one based on the
    // OS locale (or "us" as fallback). The storefront is cosmetic for GAMDL
    // (it uses the user's cookies/wrapper auth to determine the real storefront)
    // but structurally required for GAMDL's URL regex to match.
    let mut request = request;

    // Validate all URLs belong to supported domains (#459).
    // Reject any URL whose host does not exactly match an Apple Music,
    // Apple Music Classical, or legacy iTunes domain. Uses url::Url to
    // parse and extract host_str() so that substring tricks like
    // "evil.com/?next=music.apple.com/..." cannot bypass the check.
    const SUPPORTED_HOSTS: &[&str] = &[
        "music.apple.com",
        "classical.apple.com",
        "itunes.apple.com",
    ];
    for url in &request.urls {
        let parsed = url::Url::parse(url)
            .map_err(|e| format!("Invalid URL '{url}': {e}"))?;
        if parsed.scheme() == "http" || parsed.scheme() == "https" {
            let host = parsed.host_str().unwrap_or("").to_lowercase();
            // Check for an exact host match or a subdomain (e.g., "geo.music.apple.com").
            // Use strip_suffix to avoid per-iteration string allocations.
            let is_supported = SUPPORTED_HOSTS.iter().any(|&allowed| {
                host == allowed
                    || host
                        .strip_suffix(allowed)
                        .is_some_and(|prefix| prefix.ends_with('.'))
            });
            if !is_supported {
                return Err(format!(
                    "Unsupported URL domain: {url}. Only Apple Music, Apple Music Classical, and iTunes URLs are supported."
                ));
            }
        }
    }

    let original_urls = request.urls.clone();
    request.urls = request
        .urls
        .into_iter()
        .map(|url| crate::services::apple_music_api::normalize_apple_music_url(&url))
        .collect();

    // Log when normalization occurs so the user sees what happened.
    for (original, normalized) in original_urls.iter().zip(request.urls.iter()) {
        if original != normalized {
            log::info!("Normalized non-geographic URL: {original} → {normalized}");
            emit_app_log(
                &app,
                &format!("URL normalized — storefront auto-detected: {normalized}"),
            );
        }
    }

    // Check for duplicate URLs already in the active queue. This is a
    // non-blocking warning — the download proceeds regardless, but the
    // frontend can show a toast to let the user know.
    let duplicate_warning = {
        let q = queue.lock().await;
        if q.has_duplicate_urls(&request.urls) {
            log::info!("Duplicate URL detected in queue: {}", request.urls.join(", "));
            Some("This URL is already in the queue".to_string())
        } else {
            None
        }
    };

    // Multi-select artist auto-select: if the URL is an artist URL and
    // multiple modes are configured, split into N separate downloads (one
    // per mode). GAMDL only accepts a single --artist-auto-select value,
    // so MeedyaDL creates separate queue items for each selected mode.
    let is_artist_url = request.urls.iter().any(|u| u.contains("/artist/"));
    let artist_modes = &settings.artist_auto_select_multi;

    if is_artist_url && artist_modes.len() > 1 {
        // Capture URL display string before splitting
        let urls_display = request.urls.join(", ");
        let mut first_id = String::new();

        {
            let mut q = queue.lock().await;
            for (i, mode) in artist_modes.iter().enumerate() {
                // Clone the request and set the per-download artist override
                let mut split_request = request.clone();
                let overrides = split_request.options.get_or_insert_with(Default::default);
                overrides.artist_auto_select = Some(mode.clone());

                let download_id = q.enqueue(split_request, &settings);
                log::info!(
                    "Download {download_id} queued (artist mode: {})",
                    mode.to_cli_string()
                );
                if i == 0 {
                    first_id = download_id;
                }
            }
        }

        emit_app_log(
            &app,
            &format!(
                "Queued: {urls_display} ({} artist modes)",
                artist_modes.len()
            ),
        );

        let queue_handle = queue.inner().clone();
        download_queue::save_queue_to_disk(&app, &queue_handle).await;

        app.emit("download-queued", &first_id)
            .map_err(|e| format!("Failed to emit event: {e}"))?;

        if settings.auto_start_queue && !skip_auto_start.unwrap_or(false) {
            download_queue::process_queue(app, queue_handle).await;
        }

        return Ok(StartDownloadResult {
            download_id: first_id,
            duplicate_warning: duplicate_warning.clone(),
        });
    }

    // Single-mode path: standard enqueue (also handles single artist_auto_select_multi)
    let urls_display = request.urls.join(", ");

    // If exactly one multi-mode is set and it's an artist URL, apply it as an override
    let request = if is_artist_url && artist_modes.len() == 1 {
        let mut req = request;
        let overrides = req.options.get_or_insert_with(Default::default);
        if overrides.artist_auto_select.is_none() {
            overrides.artist_auto_select = Some(artist_modes[0].clone());
        }
        req
    } else {
        request
    };

    // Acquire the queue lock and enqueue the download. The lock is scoped
    // to this block to release it before the async process_queue() call,
    // avoiding potential deadlocks.
    let download_id = {
        let mut q = queue.lock().await;
        q.enqueue(request, &settings)
    };

    log::info!("Download {download_id} queued");
    emit_app_log(&app, &format!("Queued: {urls_display}"));

    // Persist the updated queue to disk for crash recovery.
    // This ensures the new item survives an unexpected app close/crash.
    let queue_handle = queue.inner().clone();
    download_queue::save_queue_to_disk(&app, &queue_handle).await;

    // Emit a Tauri event to notify the frontend that the download has been queued.
    // The frontend listens for "download-queued" events to refresh the queue UI.
    app.emit("download-queued", &download_id)
        .map_err(|e| format!("Failed to emit event: {e}"))?;

    // Trigger queue processing if auto-start is enabled AND the caller
    // didn't request skipping it. `skip_auto_start` is set by the frontend
    // when the device is offline — the item is queued but not processed
    // until the user retries or a future download triggers queue processing.
    if settings.auto_start_queue && !skip_auto_start.unwrap_or(false) {
        download_queue::process_queue(app, queue_handle).await;
    }

    Ok(StartDownloadResult {
        download_id,
        duplicate_warning,
    })
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
/// * `app` - Tauri `AppHandle` for event emission.
/// * `queue` - Managed download queue state.
/// * `download_id` - The unique ID (UUID) returned by `start_download`.
///   The frontend passes this as `downloadId` (camelCase) and Tauri
///   automatically converts it to `download_id` (`snake_case`).
///   See: <https://v2.tauri.app/develop/calling-rust/#command-arguments>
///
/// # Returns
/// * `Ok(())` - The download was successfully cancelled.
/// * `Err(String)` - The download ID was not found or the item already finished.
///
/// # Errors
/// Returns an error if the download ID was not found or the item has already
/// completed or failed (i.e., it is no longer in a cancellable state).
///
/// # Events Emitted
/// * `"download-cancelled"` - Emitted with the download ID on successful cancellation.
#[tauri::command]
pub async fn cancel_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Cancel requested for download: {download_id}");

    // Acquire lock, attempt cancellation, then release lock.
    // q.cancel() returns true if the item was found and successfully cancelled.
    let cancelled = {
        let mut q = queue.lock().await;
        q.cancel(&download_id)
    };

    if cancelled {
        // Persist the updated queue (cancelled item removed from active set)
        let queue_handle = queue.inner().clone();
        download_queue::save_queue_to_disk(&app, &queue_handle).await;

        let short = &download_id[..8.min(download_id.len())];
        emit_app_log(&app, &format!("Cancelled download [{short}]"));

        // Notify the frontend so it can update the item's UI state immediately.
        // We use `let _ =` to ignore emission errors — the cancellation itself
        // already succeeded, so a failed event is non-critical.
        let _ = app.emit("download-cancelled", &download_id);
        Ok(())
    } else {
        // The download ID was not found, or the item has already completed/failed.
        Err(format!(
            "Download {download_id} not found or already finished"
        ))
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
/// * `app` - Tauri `AppHandle` for settings access and event emission.
/// * `queue` - Managed download queue state.
/// * `download_id` - The unique ID of the failed/cancelled download to retry.
///
/// # Returns
/// * `Ok(())` - The download was reset to Queued and queue processing triggered.
/// * `Err(String)` - The download ID was not found, or the item is in a state
///   that cannot be retried (e.g., currently active or already completed).
///
/// # Errors
/// Returns an error if the download ID was not found or the item is not in a
/// retryable state (only Failed and Cancelled items can be retried).
///
/// # Events Emitted
/// * `"download-queued"` - Emitted with the download ID after successful re-queue.
#[tauri::command]
pub async fn retry_download(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Retry requested for download: {download_id}");

    // Re-load settings so retries pick up any changes the user made
    // (e.g., switching from AAC to ALAC after a failed attempt).
    let settings = crate::services::config_service::load_settings(&app).unwrap_or_default();

    // Attempt to reset the download item to Queued state.
    // q.retry() returns true only if the item exists and is in a retryable state
    // (Failed or Cancelled).
    let retried = {
        let mut q = queue.lock().await;
        q.retry(&download_id, &settings)
    };

    if retried {
        // Persist the updated queue (retried item now Queued again)
        let queue_handle = queue.inner().clone();
        download_queue::save_queue_to_disk(&app, &queue_handle).await;

        let short = &download_id[..8.min(download_id.len())];
        emit_app_log(&app, &format!("Retrying download [{short}]"));

        // Notify frontend and kick off queue processing if auto-start is enabled.
        let _ = app.emit("download-queued", &download_id);
        if settings.auto_start_queue {
            download_queue::process_queue(app, queue_handle).await;
        }
        Ok(())
    } else {
        Err(format!("Download {download_id} cannot be retried"))
    }
}

/// Retries a failed download with wrapper authentication disabled.
///
/// **Frontend caller:** `retryDownloadWithoutWrapper(downloadId)` in `src/lib/tauri-commands.ts`
///
/// Similar to `retry_download`, but explicitly disables the wrapper
/// system and clears wrapper URLs before re-queueing. This allows
/// users to fall back to cookie-based authentication when the wrapper
/// service is down or misconfigured.
///
/// Only applies to downloads that were originally attempted with wrapper
/// enabled and are in a retryable state (Failed or Cancelled).
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for settings access and event emission.
/// * `queue` - Managed download queue state.
/// * `download_id` - The unique ID of the failed download to retry.
///
/// # Returns
/// * `Ok(())` - The download was reset with wrapper disabled and queue processing triggered.
/// * `Err(String)` - The download ID was not found, not in a retryable state, or wasn't
///   originally attempted with wrapper enabled.
///
/// # Events Emitted
/// * `"download-queued"` - Emitted with the download ID after successful re-queue.
#[tauri::command]
pub async fn retry_download_without_wrapper(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
    download_id: String,
) -> Result<(), String> {
    log::info!("Retry without wrapper requested for download: {download_id}");

    // Re-load settings so retries pick up any changes the user made
    let settings = crate::services::config_service::load_settings(&app).unwrap_or_default();

    // Attempt to reset the download with wrapper disabled.
    // Returns true only if the item exists, is retryable, and was using wrapper.
    let retried = {
        let mut q = queue.lock().await;
        q.retry_without_wrapper(&download_id, &settings)
    };

    if retried {
        // Persist the updated queue (retried item now Queued again)
        let queue_handle = queue.inner().clone();
        download_queue::save_queue_to_disk(&app, &queue_handle).await;

        let short = &download_id[..8.min(download_id.len())];
        emit_app_log(&app, &format!("Retrying [{short}] without wrapper"));

        // Notify frontend and kick off queue processing if auto-start is enabled.
        let _ = app.emit("download-queued", &download_id);
        if settings.auto_start_queue {
            download_queue::process_queue(app, queue_handle).await;
        }
        Ok(())
    } else {
        Err(format!(
            "Download {download_id} cannot be retried without wrapper (not found, not retryable, or wasn't using wrapper)"
        ))
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
///
/// # Errors
/// This function is infallible in practice but returns `Result` to satisfy
/// the Tauri IPC command signature convention.
#[tauri::command]
pub async fn clear_queue(app: AppHandle, queue: State<'_, QueueHandle>) -> Result<usize, String> {
    let removed = {
        let mut q = queue.lock().await;
        // clear_finished() drains all terminal-state items and returns the count
        q.clear_finished()
    };

    // Persist the updated queue (or clear the file if nothing remains)
    let queue_handle = queue.inner().clone();
    download_queue::save_queue_to_disk(&app, &queue_handle).await;

    if removed > 0 {
        emit_app_log(&app, &format!("Cleared {removed} item(s) from queue"));
    }

    Ok(removed)
}

/// Clears ALL non-active items from the queue (completed, cancelled,
/// errored, and queued). Active downloads are preserved.
///
/// **Frontend caller:** `clearAllQueue()` in `src/lib/tauri-commands.ts`
///
/// # Returns
/// * `Ok(usize)` - The number of items removed.
#[tauri::command]
pub async fn clear_all_queue(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
) -> Result<usize, String> {
    let removed = {
        let mut q = queue.lock().await;
        q.clear_all()
    };

    let queue_handle = queue.inner().clone();
    download_queue::save_queue_to_disk(&app, &queue_handle).await;

    if removed > 0 {
        emit_app_log(&app, &format!("Cleared all {removed} item(s) from queue"));
    }

    Ok(removed)
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
///
/// # Errors
/// This function is infallible in practice but returns `Result` to satisfy
/// the Tauri IPC command signature convention.
#[tauri::command]
pub async fn get_queue_status(queue: State<'_, QueueHandle>) -> Result<QueueStatus, String> {
    let q = queue.lock().await;
    // get_counts() returns a tuple of (total, active, queued, completed, failed)
    let (total, active, queued, completed, failed) = q.get_counts();
    // get_status() returns a Vec<QueueItemStatus> with per-item details
    let items = q.get_status();
    // Release the queue lock as soon as possible to avoid holding it while
    // assembling the response struct (reduces resource contention).
    drop(q);

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

/// Checks the latest GAMDL version available on `PyPI`.
///
/// **Frontend caller:** `checkGamdlUpdate()` in `src/lib/tauri-commands.ts`
///
/// Used by the update checker to notify the user when a new GAMDL
/// version is available. Queries the `PyPI` JSON API at:
///   <https://pypi.org/pypi/gamdl/json>
///
/// This command takes no parameters because it only needs network access.
/// It does not require the `AppHandle` or `State` since it doesn't access
/// any local state or managed resources.
///
/// # Returns
/// * `Ok(String)` - The latest version string (e.g., "2.8.4").
/// * `Err(String)` - Network error or `PyPI` API parsing failure.
///
/// # Errors
/// Returns an error if the HTTP request to `PyPI` fails (network timeout,
/// DNS failure) or if the JSON response cannot be parsed.
#[tauri::command]
pub async fn check_gamdl_update() -> Result<String, String> {
    // Delegates to the gamdl_service which handles the HTTP request and
    // JSON parsing of the PyPI API response.
    crate::services::gamdl_service::check_latest_gamdl_version().await
}

/// Exports the current download queue to a `.meedyadl` file.
///
/// **Frontend caller:** `exportQueue()` in `src/lib/tauri-commands.ts`
///
/// Opens a native "Save As" dialog with the `.meedyadl` file filter.
/// Only non-terminal items (Queued/Downloading/Processing) are exported.
/// The exported file is a JSON document with the `QueueExportFile` schema.
///
/// # Returns
/// * `Ok(usize)` - The number of items exported.
/// * `Err(String)` - No items to export, dialog cancelled, or write error.
///
/// # Errors
/// Returns an error if:
/// - The queue has no exportable (non-terminal) items.
/// - The user cancels the native save dialog.
/// - JSON serialization fails.
/// - The file system write fails (permissions, disk full, etc.).
/// - The selected file path cannot be resolved.
#[tauri::command]
pub async fn export_queue(app: AppHandle, queue: State<'_, QueueHandle>) -> Result<usize, String> {
    // Import DialogExt at the top of the function body to satisfy
    // clippy::items_after_statements (items must precede statements).
    use tauri_plugin_dialog::DialogExt;

    // Get exportable items (non-terminal)
    let items = {
        let q = queue.lock().await;
        q.get_exportable_items()
    };

    if items.is_empty() {
        return Err("No items to export".to_string());
    }

    let count = items.len();

    // Build the export file structure
    let export_file = download_queue::QueueExportFile {
        version: 1,
        app: "MeedyaDL".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        items,
    };

    // Serialize to pretty-printed JSON
    let json = serde_json::to_string_pretty(&export_file)
        .map_err(|e| format!("Failed to serialize queue: {e}"))?;

    // Open a native save dialog with the .meedyadl file filter
    let file_path = app
        .dialog()
        .file()
        .add_filter("MeedyaDL Queue", &["meedyadl"])
        .set_file_name("queue.meedyadl")
        .blocking_save_file();

    match file_path {
        Some(path) => {
            let resolved = path
                .as_path()
                .ok_or_else(|| "Failed to resolve export file path".to_string())?;
            std::fs::write(resolved, &json)
                .map_err(|e| format!("Failed to write export file: {e}"))?;
            let filename = resolved
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("queue.meedyadl");
            log::info!("Exported {count} queue item(s) to file");
            emit_app_log(&app, &format!("Exported {count} item(s) to {filename}"));
            Ok(count)
        }
        None => Err("Export cancelled".to_string()),
    }
}

/// Imports download queue items from a `.meedyadl` file.
///
/// **Frontend caller:** `importQueue()` in `src/lib/tauri-commands.ts`
///
/// Opens a native file picker dialog with the `.meedyadl` file filter.
/// Imported items are enqueued as new downloads and queue processing is
/// started. The queue is persisted to disk after import.
///
/// # Returns
/// * `Ok(usize)` - The number of items imported.
/// * `Err(String)` - Dialog cancelled, invalid file, or parse error.
///
/// # Errors
/// Returns an error if:
/// - The user cancels the native file picker dialog.
/// - The selected file path cannot be resolved.
/// - The file cannot be read (permissions, not found, etc.).
/// - The file contents are not valid `QueueExportFile` JSON.
/// - The schema version is not 1 (unsupported format).
/// - The file contains no items.
#[tauri::command]
pub async fn import_queue(app: AppHandle, queue: State<'_, QueueHandle>) -> Result<usize, String> {
    // Open a native file picker with the .meedyadl file filter
    use tauri_plugin_dialog::DialogExt;
    let file_path = app
        .dialog()
        .file()
        .add_filter("MeedyaDL Queue", &["meedyadl"])
        .blocking_pick_file();

    let Some(path) = file_path else {
        return Err("Import cancelled".to_string());
    };

    // Read and parse the export file
    let resolved = path
        .as_path()
        .ok_or_else(|| "Failed to resolve import file path".to_string())?;
    let json = std::fs::read_to_string(resolved)
        .map_err(|e| format!("Failed to read import file: {e}"))?;

    let export_file: download_queue::QueueExportFile =
        serde_json::from_str(&json).map_err(|e| format!("Invalid queue file format: {e}"))?;

    // Validate schema version
    if export_file.version != 1 {
        return Err(format!(
            "Unsupported queue file version: {} (expected 1)",
            export_file.version
        ));
    }

    if export_file.items.is_empty() {
        return Err("Queue file contains no items".to_string());
    }

    // Load current settings for option merging on the importing device
    let settings = crate::services::config_service::load_settings(&app).unwrap_or_default();

    // Normalize non-geographic URLs in imported items (same as start_download).
    let items: Vec<_> = export_file
        .items
        .into_iter()
        .map(|mut item| {
            item.urls = item
                .urls
                .into_iter()
                .map(|url| crate::services::apple_music_api::normalize_apple_music_url(&url))
                .collect();
            item
        })
        .collect();

    // Import items into the queue. The lock is acquired inline and
    // released immediately after import_items() returns, avoiding
    // unnecessary resource contention (clippy::significant_drop_tightening).
    let count = queue.lock().await.import_items(items, &settings).len();

    // Persist the updated queue
    let queue_handle = queue.inner().clone();
    download_queue::save_queue_to_disk(&app, &queue_handle).await;

    // Notify the frontend that items were imported
    let _ = app.emit("queue-imported", count);

    let filename = resolved
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("queue file");
    log::info!("Imported {count} queue item(s) from file");
    emit_app_log(&app, &format!("Imported {count} item(s) from {filename}"));

    // Start processing the imported items if auto-start is enabled.
    if settings.auto_start_queue {
        download_queue::process_queue(app, queue_handle).await;
    }

    Ok(count)
}

/// Manually triggers download queue processing.
///
/// **Frontend caller:** `processQueue()` in `src/lib/tauri-commands.ts`
///
/// Used when `auto_start_queue` is disabled and the user wants to start
/// processing queued items from the Queue page. Also useful in auto mode
/// if processing stalled for any reason.
///
/// This is a no-op if no items are in the Queued state or if the
/// concurrency limit is already reached.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for subprocess management and event emission.
/// * `queue` - Managed download queue state (injected by Tauri).
///
/// # Returns
/// * `Ok(())` - Queue processing was triggered successfully.
///
/// # Errors
/// This function is infallible in practice but returns `Result` to satisfy
/// the Tauri IPC command signature convention.
#[tauri::command]
pub async fn process_queue_manual(
    app: AppHandle,
    queue: State<'_, QueueHandle>,
) -> Result<(), String> {
    log::info!("Manual queue processing triggered");
    emit_app_log(&app, "Queue processing started (manual)");
    let queue_handle = queue.inner().clone();
    download_queue::process_queue(app, queue_handle).await;
    Ok(())
}

/// Entry payload for activity log export (received from frontend).
///
/// The activity log entries live in the frontend Zustand store
/// (`activityStore.ts`) and are passed to this command for formatting
/// and writing to a file.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ActivityLogExportEntry {
    /// Unique download ID this log line belongs to
    pub download_id: String,
    /// Output stream: "stdout" or "stderr"
    pub stream: String,
    /// Raw line content from GAMDL subprocess
    pub line: String,
    /// ISO 8601 timestamp (UTC) when the line was emitted
    pub timestamp: String,
}

/// Formats an ISO 8601 timestamp to a short HH:MM:SS string for the export file.
fn format_timestamp_short(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map_or_else(|_| iso.to_string(), |dt| dt.format("%H:%M:%S").to_string())
}

/// Exports activity log entries to a `.log` file via a native save dialog.
///
/// **Frontend caller:** `exportActivityLog(entries)` in `src/lib/tauri-commands.ts`
///
/// Activity log entries are maintained in the frontend Zustand store and
/// passed to this command for formatting and writing. Each entry is
/// formatted as: `[HH:MM:SS] [download_id_prefix] [stream] line_content`
///
/// # Arguments
/// * `app` - The Tauri app handle (for dialog access)
/// * `entries` - Vec of activity log entries from the frontend store
///
/// # Returns
/// * `Ok(usize)` - Number of lines exported
/// * `Err(String)` if the dialog is cancelled, serialization fails, or I/O fails
#[tauri::command]
pub async fn export_activity_log(
    app: AppHandle,
    entries: Vec<ActivityLogExportEntry>,
) -> Result<usize, String> {
    use tauri_plugin_dialog::DialogExt;

    if entries.is_empty() {
        return Err("No log entries to export".to_string());
    }

    let count = entries.len();

    // Format entries as human-readable text lines
    let mut lines = Vec::with_capacity(count + 3);
    lines.push(format!(
        "MeedyaDL Activity Log -- Exported {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push(format!("{count} entries"));
    lines.push(String::new()); // blank separator

    for entry in &entries {
        let time = format_timestamp_short(&entry.timestamp);
        let id_prefix = if entry.download_id == "system" {
            "System"
        } else if entry.download_id.len() > 8 {
            &entry.download_id[..8]
        } else {
            &entry.download_id
        };
        lines.push(format!(
            "[{}] [{}] [{}] {}",
            time, id_prefix, entry.stream, entry.line
        ));
    }

    let text = lines.join("\n");

    // Open native save dialog with .log filter.
    // Filename includes the current date/time for easy identification.
    let default_name = chrono::Local::now()
        .format("MeedyaDL-activity-log_%Y-%m-%d_%Hh%Mm.log")
        .to_string();
    let file_path = app
        .dialog()
        .file()
        .add_filter("Log File", &["log"])
        .set_file_name(&default_name)
        .blocking_save_file();

    match file_path {
        Some(path) => {
            let resolved = path
                .as_path()
                .ok_or_else(|| "Failed to resolve export file path".to_string())?;
            std::fs::write(resolved, text).map_err(|e| format!("Failed to write log file: {e}"))?;
            log::info!("Exported {count} activity log entries to file");
            Ok(count)
        }
        None => Err("Export cancelled".to_string()),
    }
}

/// Imports a `.meedyadl` manifest file via a native open dialog.
///
/// **Frontend caller:** `importManifest()` in `src/lib/tauri-commands.ts`
///
/// Parses the manifest and returns the source URLs for the frontend
/// to populate the download form. Supports both single-source and
/// multi-platform manifests (returns all source URLs).
///
/// # Returns
/// * `Ok(Vec<String>)` - List of download URLs from the manifest sources
/// * `Err(String)` if the dialog is cancelled, file is invalid, or I/O fails
#[tauri::command]
pub async fn import_manifest(app: AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let file_path = app
        .dialog()
        .file()
        .add_filter("MeedyaDL Manifest", &["meedyadl"])
        .blocking_pick_file();

    match file_path {
        Some(path) => {
            let resolved = path
                .as_path()
                .ok_or_else(|| "Failed to resolve manifest file path".to_string())?;
            let contents = std::fs::read_to_string(resolved)
                .map_err(|e| format!("Failed to read manifest file: {e}"))?;
            let manifest: crate::models::manifest::ManifestFile =
                serde_json::from_str(&contents)
                    .map_err(|e| format!("Invalid manifest file: {e}"))?;

            let urls: Vec<String> = manifest
                .sources
                .iter()
                .map(|s| s.url.clone())
                .collect();

            if urls.is_empty() {
                return Err("Manifest contains no download sources".to_string());
            }

            log::info!(
                "Imported manifest with {} source(s)",
                urls.len()
            );
            Ok(urls)
        }
        None => Err("Import cancelled".to_string()),
    }
}

/// Result of scanning a folder for manifest files (#456).
///
/// Each entry represents one discovered `manifest.meedyadl` file with
/// metadata extracted for display in the UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScannedManifest {
    /// Path to the manifest file on disk
    pub manifest_path: String,
    /// Album directory containing the manifest
    pub album_dir: String,
    /// Source URLs extracted from the manifest
    pub urls: Vec<String>,
    /// Platform (e.g., "apple-music")
    pub platform: Option<String>,
    /// Artist name (from album directory structure)
    pub artist: Option<String>,
    /// Album name (from album directory name)
    pub album: Option<String>,
    /// When this source was last downloaded
    pub downloaded_at: Option<String>,
    /// Track count from the manifest
    pub track_count: usize,
    /// Current codec detected from files on disk (e.g., "aac", "alac") (#380)
    pub current_codec: Option<String>,
    /// Number of audio files in the album directory
    pub audio_file_count: usize,
    /// Apple Music lastModifiedDate from the manifest — used for content
    /// refresh detection (#380). If the current API response has a newer
    /// date, the album may have been updated (mix corrections, remasters,
    /// added tracks, Apple Digital Master certification).
    pub last_modified_date: Option<String>,
}

/// Recursively scan a directory for `manifest.meedyadl` files and return
/// metadata from each discovered manifest (#456).
///
/// Opens a native folder picker dialog, then walks the selected directory
/// tree looking for `manifest.meedyadl` (and legacy `.meedyadl`) files.
/// For each found manifest, extracts the source URLs, platform, download
/// date, and track count. Also infers artist/album names from the directory
/// structure (GAMDL's `Artist/Album/` convention).
///
/// Used by the "Re-download from Folder" feature to let users point at an
/// existing music library and re-queue downloads for metadata correction
/// or quality upgrades.
///
/// # Returns
/// * `Ok(Vec<ScannedManifest>)` — Manifests found (may be empty)
/// * `Err(String)` — Folder picker cancelled or I/O error
#[tauri::command]
pub async fn scan_folder_for_manifests(
    app: AppHandle,
) -> Result<Vec<ScannedManifest>, String> {
    use tauri_plugin_dialog::DialogExt;

    let folder = app
        .dialog()
        .file()
        .blocking_pick_folder();

    let folder_path = match folder {
        Some(path) => {
            path.as_path()
                .ok_or_else(|| "Failed to resolve folder path".to_string())?
                .to_path_buf()
        }
        None => return Err("Folder selection cancelled".to_string()),
    };

    log::info!(
        "Scanning folder for manifests: {}",
        folder_path.display()
    );

    let mut results = Vec::new();
    scan_dir_for_manifests_recursive(&folder_path, &mut results, 0, 10);

    log::info!(
        "Found {} manifest(s) in {}",
        results.len(),
        folder_path.display()
    );

    crate::utils::activity_log::emit_app_log(
        &app,
        &format!(
            "Folder scan: found {} manifest(s) in {}",
            results.len(),
            folder_path.display()
        ),
    );

    Ok(results)
}

/// Recursively scan directories for manifest files.
fn scan_dir_for_manifests_recursive(
    dir: &std::path::Path,
    results: &mut Vec<ScannedManifest>,
    depth: u32,
    max_depth: u32,
) {
    if depth > max_depth {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            scan_dir_for_manifests_recursive(&path, results, depth + 1, max_depth);
            continue;
        }

        // Check for manifest.meedyadl or legacy .meedyadl
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if file_name != "manifest.meedyadl" && file_name != ".meedyadl" {
            continue;
        }

        // Parse the manifest
        let Ok(contents) = std::fs::read_to_string(&path) else {
            log::debug!("Failed to read manifest: {}", path.display());
            continue;
        };

        let Ok(manifest) = serde_json::from_str::<crate::models::manifest::ManifestFile>(&contents) else {
            log::debug!("Failed to parse manifest: {}", path.display());
            continue;
        };

        // Extract the first (most recent) source
        let source = manifest.sources.first();

        // Infer artist/album from directory structure:
        // base/Artist/Album/manifest.meedyadl → parent = Album, grandparent = Artist
        let album_dir = path.parent().unwrap_or(dir);
        let album_name = album_dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from);
        let artist_name = album_dir
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(String::from);

        let urls: Vec<String> = manifest
            .sources
            .iter()
            .map(|s| s.url.clone())
            .collect();

        if urls.is_empty() {
            continue;
        }

        let track_count = source
            .map(|s| s.tracks.len())
            .unwrap_or(0);

        // Detect current codec from the first M4A file in the album dir (#380).
        // Reads the MeedyaMeta:SourceCodec or com.apple.iTunes:isLossless tag.
        let (current_codec, audio_file_count) = detect_album_codec(album_dir);

        results.push(ScannedManifest {
            manifest_path: path.to_string_lossy().to_string(),
            album_dir: album_dir.to_string_lossy().to_string(),
            urls,
            platform: source.map(|s| s.platform.clone()),
            artist: artist_name,
            album: album_name,
            downloaded_at: source.map(|s| s.downloaded_at.clone()),
            track_count,
            current_codec,
            audio_file_count,
            last_modified_date: source.and_then(|s| s.last_modified_date.clone()),
        });
    }
}

/// Checks whether a URL was previously downloaded and returns change
/// Detect the codec of the first M4A file in an album directory (#380).
///
/// Reads the `MeedyaMeta:SourceCodec` or `com.apple.iTunes:isLossless` tag
/// from the first `.m4a` file found. Returns `(codec_name, audio_file_count)`.
fn detect_album_codec(album_dir: &std::path::Path) -> (Option<String>, usize) {
    let mut count = 0;
    let mut codec: Option<String> = None;

    let Ok(entries) = std::fs::read_dir(album_dir) else {
        return (None, 0);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !ext.eq_ignore_ascii_case("m4a") && !ext.eq_ignore_ascii_case("m4v") {
            continue;
        }
        count += 1;

        // Only detect codec from the first file
        if codec.is_some() {
            continue;
        }

        if let Ok(tag) = mp4ameta::Tag::read_from_path(&path) {
            // Try MeedyaMeta:SourceCodec first (written by MeedyaDL enrichment)
            let source_codec = tag
                .strings_of(&mp4ameta::FreeformIdent::new_static(
                    "MeedyaMeta",
                    "SourceCodec",
                ))
                .next()
                .map(String::from);

            if let Some(c) = source_codec {
                codec = Some(c);
            } else {
                // Fallback: check isLossless tag
                let is_lossless = tag
                    .strings_of(&mp4ameta::FreeformIdent::new_static(
                        "com.apple.iTunes",
                        "isLossless",
                    ))
                    .next()
                    .map(|s| s.to_string());

                codec = match is_lossless.as_deref() {
                    Some("true") => Some("alac".to_string()),
                    Some("false") => Some("aac".to_string()),
                    _ => None,
                };
            }
        }
    }

    (codec, count)
}

/// detection metadata for smart re-download detection (#263).
///
/// Looks up the URL in download history. If found, returns the download
/// date and title. The frontend uses this to inform the user before
/// re-downloading. The actual `lastModifiedDate` comparison happens
/// after the API metadata is fetched during enrichment.
///
/// # Returns
/// * `Ok(Some(info))` - URL was previously downloaded; includes date + title
/// * `Ok(None)` - URL not in download history (first download)
/// * `Err` - History service error
#[tauri::command]
pub async fn check_redownload_status(
    app: AppHandle,
    url: String,
) -> Result<Option<RedownloadInfo>, String> {
    let history = crate::services::history_service::list_history(&app);
    let previous = history.iter().find(|e| e.url == url && e.status == "success");

    Ok(previous.map(|entry| RedownloadInfo {
        downloaded_at: entry.completed_at.clone(),
        title: entry.title.clone(),
        album: entry.album.clone(),
    }))
}

/// Fetches syllable-level (word-by-word) TTML lyrics for a single song from
/// the Apple Music API.
///
/// **Frontend caller:** `fetchSyllableLyrics(storefront, songId)` in `src/lib/tauri-commands.ts`
///
/// Requires MusicKit credentials (Team ID, Key ID, private key) and a valid
/// `media-user-token` cookie for subscriber authentication. Returns the raw
/// TTML XML string if available, or `None` if the song has no syllable-level
/// lyrics.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for settings and keychain access.
/// * `storefront` - Apple Music storefront code (e.g., "us", "gb").
/// * `song_id` - Numeric Apple Music song ID.
///
/// # Returns
/// * `Ok(Some(String))` - TTML XML lyrics content.
/// * `Ok(None)` - No syllable-level lyrics available for this song.
/// * `Err(String)` - Credentials missing, expired cookies, or API error.
#[tauri::command]
pub async fn fetch_syllable_lyrics(
    app: AppHandle,
    storefront: String,
    song_id: String,
) -> Result<Option<String>, String> {
    use crate::services::{apple_music_api, config_service};

    // Resolve MusicKit credentials and generate JWT
    let settings = config_service::load_settings(&app).unwrap_or_default();
    let private_key = apple_music_api::get_private_key_from_keychain()
        .map_err(|e| format!("Keychain error: {e}"))?;
    let jwt = apple_music_api::resolve_musickit_developer_token(
        settings.musickit_team_id.as_deref(),
        settings.musickit_key_id.as_deref(),
        private_key.as_deref(),
    )
    .map_err(|e| format!("JWT error: {e}"))?
    .ok_or(
        "MusicKit credentials not configured. Set up Team ID, Key ID, and private key in Settings > Advanced > API Credentials."
    )?;

    // Extract media-user-token from cookies file
    let cookies_path = settings.cookies_path.as_deref().ok_or(
        "Apple Music cookies not configured. Import cookies from your browser in Settings > Authentication.",
    )?;
    let music_user_token = apple_music_api::extract_media_user_token(cookies_path)
        .map_err(|e| format!("Cookie error: {e}"))?
        .ok_or(
            "Apple Music subscriber token not found or expired. Re-import cookies from your browser in Settings > Authentication."
        )?;

    // Fetch syllable lyrics from the Apple Music API
    apple_music_api::fetch_syllable_lyrics(&jwt, &storefront, &song_id, &music_user_token).await
}

/// Information about a previous download of the same URL.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RedownloadInfo {
    /// When the URL was last downloaded (ISO 8601).
    pub downloaded_at: String,
    /// Track/album title from the previous download.
    pub title: Option<String>,
    /// Album name from the previous download.
    pub album: Option<String>,
}
