// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download queue manager service.
// Manages a queue of download jobs with concurrent execution limits,
// automatic processing of queued items, fallback quality retries,
// and child process tracking for cancellation support.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

use tauri::{AppHandle, Emitter};

use crate::models::download::{DownloadRequest, DownloadState, QueueItemStatus};
use crate::models::gamdl_options::GamdlOptions;
use crate::models::settings::AppSettings;
use crate::services::{config_service, gamdl_service};
use crate::utils::process;

// ============================================================
// Queue item (internal representation with extra tracking fields)
// ============================================================

/// Internal representation of a download job in the queue.
/// Contains the public status plus additional tracking fields
/// that are not exposed to the frontend.
#[derive(Debug, Clone)]
struct QueueItem {
    /// Public status sent to the frontend
    pub status: QueueItemStatus,
    /// The original download request (URLs + options)
    pub request: DownloadRequest,
    /// Merged GAMDL options (user overrides merged with global settings)
    pub merged_options: GamdlOptions,
    /// Index into the fallback chain (0 = preferred codec, 1 = first fallback, etc.)
    pub fallback_index: usize,
    /// Number of network retry attempts remaining
    pub network_retries_left: u32,
}

// ============================================================
// Download queue manager
// ============================================================

/// The download queue manager. Wrapped in Arc<Mutex<>> for thread-safe
/// access from multiple Tauri commands and background tasks.
#[derive(Debug)]
pub struct DownloadQueue {
    /// The queue of download jobs (front = next to process)
    items: VecDeque<QueueItem>,
    /// Maximum number of concurrent downloads
    max_concurrent: usize,
    /// Number of currently active downloads
    active_count: usize,
    /// Maximum number of network retry attempts per download
    max_network_retries: u32,
}

/// Thread-safe handle to the download queue, stored as Tauri managed state.
pub type QueueHandle = Arc<Mutex<DownloadQueue>>;

/// Creates a new queue handle for use as Tauri managed state.
pub fn new_queue_handle() -> QueueHandle {
    Arc::new(Mutex::new(DownloadQueue::new()))
}

impl DownloadQueue {
    /// Creates a new empty download queue.
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
            max_concurrent: 1,
            active_count: 0,
            max_network_retries: 3,
        }
    }

    /// Adds a new download to the queue and returns its unique ID.
    ///
    /// The download is placed at the back of the queue in the Queued state.
    /// The caller should call `process_queue()` after adding to start
    /// processing if slots are available.
    ///
    /// # Arguments
    /// * `request` - The download request with URLs and optional overrides
    /// * `settings` - Current app settings for merging default options
    ///
    /// # Returns
    /// The unique download ID for tracking this job.
    pub fn enqueue(&mut self, request: DownloadRequest, settings: &AppSettings) -> String {
        let download_id = uuid::Uuid::new_v4().to_string();

        // Merge per-download overrides with global settings to produce final options
        let merged_options = merge_options(request.options.as_ref(), settings);

        let item = QueueItem {
            status: QueueItemStatus {
                id: download_id.clone(),
                urls: request.urls.clone(),
                state: DownloadState::Queued,
                progress: 0.0,
                current_track: None,
                total_tracks: None,
                completed_tracks: None,
                speed: None,
                eta: None,
                error: None,
                output_path: None,
                codec_used: Some(
                    merged_options
                        .song_codec
                        .as_ref()
                        .map(|c| c.to_cli_string().to_string())
                        .unwrap_or_else(|| settings.default_song_codec.to_cli_string().to_string()),
                ),
                fallback_occurred: false,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
            request,
            merged_options,
            fallback_index: 0,
            network_retries_left: self.max_network_retries,
        };

        log::info!(
            "Enqueued download {} ({} URL(s))",
            download_id,
            item.status.urls.len()
        );

        self.items.push_back(item);
        download_id
    }

    /// Returns the public status of all queue items.
    pub fn get_status(&self) -> Vec<QueueItemStatus> {
        self.items.iter().map(|item| item.status.clone()).collect()
    }

    /// Returns summary counts for the queue.
    pub fn get_counts(&self) -> (usize, usize, usize, usize, usize) {
        let total = self.items.len();
        let active = self.items.iter().filter(|i| {
            i.status.state == DownloadState::Downloading
                || i.status.state == DownloadState::Processing
        }).count();
        let queued = self.items.iter().filter(|i| i.status.state == DownloadState::Queued).count();
        let completed = self.items.iter().filter(|i| i.status.state == DownloadState::Complete).count();
        let failed = self.items.iter().filter(|i| i.status.state == DownloadState::Error).count();
        (total, active, queued, completed, failed)
    }

    /// Cancels a download by ID.
    ///
    /// If the download is queued, it's moved to the Cancelled state.
    /// If it's active, we mark it cancelled (the running task will check this).
    ///
    /// # Returns
    /// `true` if the item was found, `false` otherwise.
    pub fn cancel(&mut self, download_id: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            match item.status.state {
                DownloadState::Queued => {
                    item.status.state = DownloadState::Cancelled;
                    log::info!("Download {} cancelled (was queued)", download_id);
                    true
                }
                DownloadState::Downloading | DownloadState::Processing => {
                    item.status.state = DownloadState::Cancelled;
                    // The active_count will be decremented when the running task
                    // detects the cancellation and stops
                    log::info!("Download {} marked for cancellation", download_id);
                    true
                }
                _ => {
                    log::debug!("Download {} already in terminal state", download_id);
                    false
                }
            }
        } else {
            log::warn!("Download {} not found in queue", download_id);
            false
        }
    }

    /// Removes completed/failed/cancelled items from the queue.
    ///
    /// # Returns
    /// Number of items removed.
    pub fn clear_finished(&mut self) -> usize {
        let before = self.items.len();
        self.items.retain(|item| {
            !matches!(
                item.status.state,
                DownloadState::Complete | DownloadState::Error | DownloadState::Cancelled
            )
        });
        let removed = before - self.items.len();
        if removed > 0 {
            log::info!("Cleared {} finished items from queue", removed);
        }
        removed
    }

    /// Updates the state of a queue item.
    /// Used by the download task to report progress.
    pub fn update_item_state(&mut self, download_id: &str, state: DownloadState) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.state = state;
        }
    }

    /// Updates progress information for a queue item based on a parsed GAMDL event.
    pub fn update_item_progress(
        &mut self,
        download_id: &str,
        event: &process::GamdlOutputEvent,
    ) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            match event {
                process::GamdlOutputEvent::DownloadProgress { percent, speed, eta } => {
                    item.status.progress = *percent;
                    item.status.speed = Some(speed.clone());
                    item.status.eta = Some(eta.clone());
                    item.status.state = DownloadState::Downloading;
                }
                process::GamdlOutputEvent::TrackInfo { title, artist, .. } => {
                    let track_name = if artist.is_empty() {
                        title.clone()
                    } else {
                        format!("{} - {}", artist, title)
                    };
                    item.status.current_track = Some(track_name);
                }
                process::GamdlOutputEvent::ProcessingStep { .. } => {
                    item.status.state = DownloadState::Processing;
                }
                process::GamdlOutputEvent::Complete { path } => {
                    item.status.output_path = Some(path.clone());
                    item.status.progress = 100.0;
                }
                process::GamdlOutputEvent::Error { message } => {
                    item.status.error = Some(message.clone());
                }
                _ => {}
            }
        }
    }

    /// Marks a download as errored and sets the error message.
    pub fn set_error(&mut self, download_id: &str, error: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.state = DownloadState::Error;
            item.status.error = Some(error.to_string());
        }
    }

    /// Marks a download as complete.
    pub fn set_complete(&mut self, download_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.state = DownloadState::Complete;
            item.status.progress = 100.0;
        }
    }

    /// Checks if a download should attempt a fallback codec/resolution.
    ///
    /// # Returns
    /// `Some(new_options)` if fallback should be attempted, `None` if all fallbacks exhausted.
    pub fn try_fallback(
        &mut self,
        download_id: &str,
        settings: &AppSettings,
    ) -> Option<GamdlOptions> {
        let item = self.items.iter_mut().find(|i| i.status.id == download_id)?;

        // Only attempt fallback if enabled in settings
        if !settings.fallback_enabled {
            return None;
        }

        // Try the next codec in the music fallback chain
        item.fallback_index += 1;

        if item.fallback_index < settings.music_fallback_chain.len() {
            let next_codec = &settings.music_fallback_chain[item.fallback_index];
            let mut new_options = item.merged_options.clone();
            new_options.song_codec = Some(next_codec.clone());

            // Update tracking info
            item.status.codec_used = Some(next_codec.to_cli_string().to_string());
            item.status.fallback_occurred = true;
            item.status.state = DownloadState::Queued;
            item.status.error = None;
            item.status.progress = 0.0;
            item.merged_options = new_options.clone();

            log::info!(
                "Download {} falling back to codec: {}",
                download_id,
                next_codec.to_cli_string()
            );

            Some(new_options)
        } else {
            log::info!(
                "Download {} exhausted all fallback codecs",
                download_id
            );
            None
        }
    }

    /// Checks if a download should retry due to a network error.
    ///
    /// # Returns
    /// `true` if retry should be attempted, `false` if retries exhausted.
    pub fn try_network_retry(&mut self, download_id: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            if item.network_retries_left > 0 {
                item.network_retries_left -= 1;
                item.status.state = DownloadState::Queued;
                item.status.error = None;
                item.status.progress = 0.0;
                log::info!(
                    "Download {} network retry ({} remaining)",
                    download_id,
                    item.network_retries_left
                );
                true
            } else {
                log::info!("Download {} exhausted network retries", download_id);
                false
            }
        } else {
            false
        }
    }

    /// Gets the next queued item's download ID and options for execution.
    /// Returns None if no queued items or max concurrent limit reached.
    pub fn next_pending(&mut self) -> Option<(String, Vec<String>, GamdlOptions)> {
        if self.active_count >= self.max_concurrent {
            return None;
        }

        let item = self.items.iter_mut().find(|i| i.status.state == DownloadState::Queued)?;
        item.status.state = DownloadState::Downloading;
        self.active_count += 1;

        Some((
            item.status.id.clone(),
            item.status.urls.clone(),
            item.merged_options.clone(),
        ))
    }

    /// Called when a download task finishes (success, error, or cancel).
    /// Decrements the active count so new downloads can start.
    pub fn on_task_finished(&mut self) {
        if self.active_count > 0 {
            self.active_count -= 1;
        }
    }

    /// Checks if a download is still active (not cancelled).
    /// Used by running tasks to detect if they should abort.
    pub fn is_cancelled(&self, download_id: &str) -> bool {
        self.items
            .iter()
            .find(|i| i.status.id == download_id)
            .map(|i| i.status.state == DownloadState::Cancelled)
            .unwrap_or(false)
    }

    /// Retries a failed download by resetting its state to Queued.
    /// Returns true if the item was found and reset.
    pub fn retry(&mut self, download_id: &str, settings: &AppSettings) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            if item.status.state == DownloadState::Error || item.status.state == DownloadState::Cancelled {
                // Reset to initial state with fresh options
                item.merged_options = merge_options(item.request.options.as_ref(), settings);
                item.fallback_index = 0;
                item.network_retries_left = self.max_network_retries;
                item.status.state = DownloadState::Queued;
                item.status.error = None;
                item.status.progress = 0.0;
                item.status.fallback_occurred = false;
                item.status.codec_used = Some(
                    item.merged_options
                        .song_codec
                        .as_ref()
                        .map(|c| c.to_cli_string().to_string())
                        .unwrap_or_else(|| settings.default_song_codec.to_cli_string().to_string()),
                );
                log::info!("Download {} reset for retry", download_id);
                return true;
            }
        }
        false
    }
}

// ============================================================
// Helper: merge per-download overrides with global settings
// ============================================================

/// Merges per-download option overrides with the global app settings
/// to produce the final set of GAMDL CLI options.
///
/// Per-download overrides take priority. For any field that is None
/// in the override, the global setting value is used.
fn merge_options(overrides: Option<&GamdlOptions>, settings: &AppSettings) -> GamdlOptions {
    let mut options = GamdlOptions::default();

    // Apply global settings as the base
    options.song_codec = Some(settings.default_song_codec.clone());
    options.music_video_resolution = Some(settings.default_video_resolution.clone());
    options.music_video_codec_priority = Some(settings.default_video_codec_priority.clone());
    options.music_video_remux_format = Some(settings.default_video_remux_format.clone());
    options.synced_lyrics_format = Some(settings.synced_lyrics_format.clone());
    options.no_synced_lyrics = Some(settings.no_synced_lyrics);
    options.synced_lyrics_only = Some(settings.synced_lyrics_only);
    options.save_cover = Some(settings.save_cover);
    options.cover_format = Some(settings.cover_format.clone());
    options.cover_size = Some(settings.cover_size);
    options.overwrite = Some(settings.overwrite);
    options.language = Some(settings.language.clone());
    options.album_folder_template = Some(settings.album_folder_template.clone());
    options.compilation_folder_template = Some(settings.compilation_folder_template.clone());
    options.no_album_folder_template = Some(settings.no_album_folder_template.clone());
    options.single_disc_file_template = Some(settings.single_disc_file_template.clone());
    options.multi_disc_file_template = Some(settings.multi_disc_file_template.clone());
    options.no_album_file_template = Some(settings.no_album_file_template.clone());
    options.playlist_file_template = Some(settings.playlist_file_template.clone());
    options.use_wrapper = Some(settings.use_wrapper);
    options.wrapper_account_url = Some(settings.wrapper_account_url.clone());
    options.truncate = settings.truncate;

    if !settings.output_path.is_empty() {
        options.output_path = Some(settings.output_path.clone());
    }

    // Apply tool paths from settings
    options.cookies_path = settings.cookies_path.clone();
    options.ffmpeg_path = settings.ffmpeg_path.clone();
    options.mp4decrypt_path = settings.mp4decrypt_path.clone();
    options.mp4box_path = settings.mp4box_path.clone();
    options.nm3u8dlre_path = settings.nm3u8dlre_path.clone();
    options.amdecrypt_path = settings.amdecrypt_path.clone();

    // Set download and remux modes
    options.download_mode = Some(settings.download_mode.clone());
    options.remux_mode = Some(settings.remux_mode.clone());

    // Apply exclude tags
    if !settings.exclude_tags.is_empty() {
        options.exclude_tags = Some(settings.exclude_tags.join(","));
    }

    // Apply per-download overrides (these take priority over globals)
    if let Some(overrides) = overrides {
        if overrides.song_codec.is_some() {
            options.song_codec = overrides.song_codec.clone();
        }
        if overrides.music_video_resolution.is_some() {
            options.music_video_resolution = overrides.music_video_resolution.clone();
        }
        if overrides.music_video_codec_priority.is_some() {
            options.music_video_codec_priority = overrides.music_video_codec_priority.clone();
        }
        if overrides.music_video_remux_format.is_some() {
            options.music_video_remux_format = overrides.music_video_remux_format.clone();
        }
        if overrides.output_path.is_some() {
            options.output_path = overrides.output_path.clone();
        }
        if overrides.overwrite.is_some() {
            options.overwrite = overrides.overwrite;
        }
    }

    options
}

// ============================================================
// Queue processing: runs downloads and handles fallback/retry
// ============================================================

/// Processes the next queued download if a slot is available.
///
/// This function is called after enqueueing a new item, after a download
/// completes, or after a retry/fallback. It spawns a background task
/// for the download and sets up event forwarding to the frontend.
///
/// Returns a boxed future to support recursive calls from within
/// `tokio::spawn` (standard pattern for recursive async in Rust).
///
/// # Arguments
/// * `app` - Tauri app handle for event emission and path resolution
/// * `queue` - Shared queue handle
pub fn process_queue(
    app: AppHandle,
    queue: QueueHandle,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
    // Get the next pending item (returns None if max concurrent reached or no items)
    let pending = {
        let mut q = queue.lock().await;
        q.next_pending()
    };

    let Some((download_id, urls, options)) = pending else {
        return;
    };

    log::info!("Processing download {}", download_id);

    // Notify the frontend that processing has started
    let _ = app.emit("download-started", &download_id);

    // Spawn the download in a background task
    let app_clone = app.clone();
    let queue_clone = queue.clone();
    let dl_id = download_id.clone();

    tokio::spawn(async move {
        // Run the GAMDL download
        let result = run_download_with_events(
            &app_clone,
            &dl_id,
            &urls,
            &options,
            &queue_clone,
        )
        .await;

        // Handle the result
        match result {
            Ok(()) => {
                let mut q = queue_clone.lock().await;
                q.set_complete(&dl_id);
                q.on_task_finished();
                log::info!("Download {} completed successfully", dl_id);
                let _ = app_clone.emit("download-complete", &dl_id);
            }
            Err(error_msg) => {
                let error_category = process::classify_error(&error_msg);
                log::error!("Download {} failed ({}): {}", dl_id, error_category, error_msg);

                // Determine if we should retry or fallback
                let should_retry = match error_category {
                    "codec" => {
                        // Load settings for fallback chain
                        let settings = load_settings_for_queue(&app_clone).await;
                        let mut q = queue_clone.lock().await;
                        q.set_error(&dl_id, &error_msg);
                        q.on_task_finished();

                        if let Some(_new_options) = q.try_fallback(&dl_id, &settings) {
                            log::info!("Download {} will retry with fallback codec", dl_id);
                            true
                        } else {
                            false
                        }
                    }
                    "network" => {
                        let mut q = queue_clone.lock().await;
                        q.set_error(&dl_id, &error_msg);
                        q.on_task_finished();

                        if q.try_network_retry(&dl_id) {
                            log::info!("Download {} will retry (network error)", dl_id);
                            true
                        } else {
                            false
                        }
                    }
                    _ => {
                        // Non-retriable error
                        let mut q = queue_clone.lock().await;
                        q.set_error(&dl_id, &error_msg);
                        q.on_task_finished();
                        false
                    }
                };

                if !should_retry {
                    let _ = app_clone.emit(
                        "download-error",
                        serde_json::json!({
                            "download_id": dl_id,
                            "error": error_msg,
                            "category": error_category,
                        }),
                    );
                }
            }
        }

        // Process the next item in the queue (cascade)
        process_queue(app_clone, queue_clone).await;
    });
    }) // close Box::pin(async move {
}

/// Runs a GAMDL download while forwarding parsed events to both
/// the queue item (for status tracking) and the frontend (for UI updates).
async fn run_download_with_events(
    app: &AppHandle,
    download_id: &str,
    urls: &[String],
    options: &GamdlOptions,
    queue: &QueueHandle,
) -> Result<(), String> {
    log::info!(
        "Starting GAMDL download {} for {} URL(s)",
        download_id,
        urls.len()
    );

    // Build the command with all arguments
    let mut cmd = gamdl_service::build_gamdl_command_public(app, urls, options)?;

    // Configure piped stdout/stderr for real-time parsing
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Spawn the GAMDL subprocess
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start GAMDL process: {}", e))?;

    // Take stdout/stderr handles
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture GAMDL stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture GAMDL stderr".to_string())?;

    // Collect error messages for fallback decision
    let collected_errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Spawn stdout reader
    let stdout_task = {
        let download_id = download_id.to_string();
        let app = app.clone();
        let queue = queue.clone();
        let errors = collected_errors.clone();
        tokio::spawn(async move {
            let reader = tokio::io::BufReader::new(stdout);
            let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
            while let Ok(Some(line)) = lines.next_line().await {
                let event = process::parse_gamdl_output(&line);
                log::debug!("[gamdl stdout] {}", line);

                // Update the queue item's progress
                {
                    let mut q = queue.lock().await;
                    q.update_item_progress(&download_id, &event);
                }

                // Collect errors for fallback decisions
                if let process::GamdlOutputEvent::Error { ref message } = event {
                    let mut errs = errors.lock().await;
                    errs.push(message.clone());
                }

                // Emit to frontend
                let progress = gamdl_service::GamdlProgress {
                    download_id: download_id.clone(),
                    event,
                };
                let _ = app.emit("gamdl-output", &progress);
            }
        })
    };

    // Spawn stderr reader
    let stderr_task = {
        let download_id = download_id.to_string();
        let app = app.clone();
        let queue = queue.clone();
        let errors = collected_errors.clone();
        tokio::spawn(async move {
            let reader = tokio::io::BufReader::new(stderr);
            let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
            while let Ok(Some(line)) = lines.next_line().await {
                let event = process::parse_gamdl_output(&line);
                log::debug!("[gamdl stderr] {}", line);

                {
                    let mut q = queue.lock().await;
                    q.update_item_progress(&download_id, &event);
                }

                if let process::GamdlOutputEvent::Error { ref message } = event {
                    let mut errs = errors.lock().await;
                    errs.push(message.clone());
                }

                let progress = gamdl_service::GamdlProgress {
                    download_id: download_id.clone(),
                    event,
                };
                let _ = app.emit("gamdl-output", &progress);
            }
        })
    };

    // Periodically check for cancellation while waiting for the process
    let status = loop {
        // Check if the download was cancelled
        {
            let q = queue.lock().await;
            if q.is_cancelled(download_id) {
                log::info!("Download {} cancelled, killing process", download_id);
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err("Download cancelled by user".to_string());
            }
        }

        // Check if the process has exited
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                // Process still running, wait a bit before checking again
                tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
            }
            Err(e) => return Err(format!("Failed to wait for GAMDL process: {}", e)),
        }
    };

    // Wait for output reader tasks to finish
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    // Check the exit status
    if status.success() {
        Ok(())
    } else {
        // Use collected errors for a meaningful error message
        let errors = collected_errors.lock().await;
        if let Some(last_error) = errors.last() {
            Err(last_error.clone())
        } else {
            let code = status.code().unwrap_or(-1);
            Err(format!("GAMDL process exited with code {}", code))
        }
    }
}

/// Loads the current app settings (used by queue processing for fallback decisions).
async fn load_settings_for_queue(app: &AppHandle) -> AppSettings {
    match config_service::load_settings(app) {
        Ok(settings) => settings,
        Err(e) => {
            log::warn!("Failed to load settings for fallback: {}, using defaults", e);
            AppSettings::default()
        }
    }
}
