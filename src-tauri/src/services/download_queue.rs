// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download queue manager service.
// Manages a queue of download jobs with concurrent execution limits,
// automatic processing of queued items, fallback quality retries,
// and child process tracking for cancellation support.
//
// ## Architecture Overview
//
// The download queue is the central orchestrator for all GAMDL downloads.
// It sits between the frontend (React) and the GAMDL subprocess execution:
//
// ```
// Frontend (React)                  Download Queue                    GAMDL Process
// ================                  ==============                    =============
// "Add to Queue" button  -->  enqueue() --> QueueItem (Queued)
//                             process_queue() --> next_pending()
//                                                    |
//                             run_download_with_events() --> spawn GAMDL
//                                    |                          |
//                             update_item_progress() <-- parse stdout/stderr
//                                    |
//                             emit("gamdl-output") --> frontend listener
//                                    |
//                             on_task_finished() --> process_queue() (cascade)
// ```
//
// ## Key Design Decisions
//
// 1. **Arc<Mutex<DownloadQueue>>**: The queue is wrapped in Arc<Mutex<>> for
//    thread-safe access from multiple Tauri command handlers and background tasks.
//    Ref: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html
//
// 2. **VecDeque for FIFO ordering**: Items are processed front-to-back, with
//    new items added to the back. This provides natural queue ordering.
//
// 3. **Recursive process_queue()**: After each download completes, process_queue()
//    is called again to start the next item, creating a cascade effect.
//    Uses Pin<Box<dyn Future>> to support this recursive async pattern.
//
// 4. **Fallback codec chains**: When a download fails with a codec error, the
//    queue automatically retries with the next codec in the fallback chain
//    (e.g., alac -> aac-he -> aac-binaural). Configurable in settings.
//
// 5. **Network retries**: Network errors trigger automatic retries (up to 3 by default)
//    with the same options, giving transient errors a chance to resolve.
//
// 6. **Cancellation polling**: Running downloads are checked for cancellation every
//    250ms via try_wait() + is_cancelled(). The process is killed on cancellation.
//
// ## Event Emission Pattern
//
// Real-time progress is reported to the frontend via Tauri's event system:
// - "download-started" - Emitted when a queued item begins downloading
// - "gamdl-output" - Emitted for each parsed line of GAMDL output (progress, track info, etc.)
// - "download-complete" - Emitted when a download finishes successfully
// - "download-error" - Emitted when a download fails (includes error category for UI routing)
// Ref: https://v2.tauri.app/develop/calling-rust/#events
//
// ## References
//
// - Tokio Mutex (async-aware): https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html
// - Tokio process spawning: https://docs.rs/tokio/latest/tokio/process/
// - Pin and Box for recursive futures: https://doc.rust-lang.org/std/pin/
// - Tauri event system: https://v2.tauri.app/develop/calling-rust/#events

use std::collections::{HashSet, VecDeque};
// Future and Pin are needed for the recursive async pattern in process_queue().
// Recursive async functions cannot use normal `async fn` syntax because the
// compiler cannot determine the size of the future at compile time.
// Instead, we return Pin<Box<dyn Future<Output = ()> + Send>>.
// Ref: https://doc.rust-lang.org/std/pin/index.html
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
// Tokio's Mutex is used instead of std::sync::Mutex because the lock is held
// across .await points. std::sync::Mutex would block the entire thread;
// tokio::sync::Mutex yields the task instead.
// Ref: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html
use tokio::sync::Mutex;

// Emitter trait provides app.emit() for sending events to the frontend.
use tauri::{AppHandle, Emitter};

// DownloadRequest: The user's download request from the frontend (URLs + optional overrides).
// DownloadState: Enum of lifecycle states (Queued, Downloading, Processing, Complete, Error, Cancelled).
// QueueItemStatus: The public-facing status struct sent to the frontend for UI rendering.
use crate::models::download::{DownloadRequest, DownloadState, QueueItemStatus};
// GamdlOptions: Typed representation of GAMDL CLI arguments, used as the "effective" options
// after merging per-download overrides with global settings.
// SongCodec: Enum of audio codec options, used for companion download planning and
// codec suffix logic.
use crate::models::gamdl_options::{GamdlOptions, LyricsFormat, SongCodec};
// AppSettings: The full application settings, used for merging defaults and fallback chain config.
// CompanionMode: Enum controlling companion download behavior (Disabled, AtmosToLossless, etc.).
use crate::models::settings::{AppSettings, CompanionMode};
// config_service: Used to load settings during fallback decisions.
// gamdl_service: Provides build_gamdl_command_public() and GamdlProgress for subprocess execution.
// crash_report_service: Used to save download error reports for user-reportable diagnostics.
use crate::services::{config_service, crash_report_service, gamdl_service};
// CrashReport: Reused for download error reports (source: "download_error").
use crate::models::crash_report::CrashReport;
// process: Provides parse_gamdl_output() for parsing GAMDL output lines and
// classify_error() for categorizing errors (codec, network, etc.) for retry logic.
use crate::utils::process;

// ============================================================
// Activity log event (raw subprocess output for live log viewer)
// ============================================================

/// Payload for the "activity-log" Tauri event, emitted for every raw
/// stdout/stderr line from GAMDL subprocesses before parsing, as well as
/// for internal MeedyaDL actions (enrichment, companions, diagnostics).
/// The Activity Log page subscribes to these events for a live terminal view.
///
/// Stream values:
///   - `"stdout"` -- GAMDL subprocess stdout
///   - `"stderr"` -- GAMDL subprocess stderr
///   - `"internal"` -- MeedyaDL internal actions (enrichment, companion downloads)
#[derive(Clone, serde::Serialize)]
struct ActivityLogEvent {
    download_id: String,
    stream: &'static str,
    line: String,
    timestamp: String,
}

/// Emits an internal MeedyaDL activity log event (not from GAMDL subprocess).
/// Used to surface enrichment progress, companion downloads, and diagnostic
/// information in the Activity Log so users can see what MeedyaDL is doing
/// behind the scenes.
fn emit_internal_log(app: &tauri::AppHandle, download_id: &str, message: &str) {
    let _ = app.emit(
        "activity-log",
        &ActivityLogEvent {
            download_id: download_id.to_string(),
            stream: "internal",
            line: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    );
}

/// Checks if the given path contains any .m4a or .m4v audio/video files.
fn has_audio_files(dir: &std::path::Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("m4a") || ext.eq_ignore_ascii_case("m4v") {
                        return true;
                    }
                }
            } else if path.is_dir() {
                // Recurse into subdirectories (GAMDL creates Artist/Album/ structure)
                if has_audio_files(&path) {
                    return true;
                }
            }
        }
    }
    false
}

/// Finds the most recently modified subdirectory containing audio files (.m4a/.m4v).
/// Used by the partial-success recovery path to find the actual album directory
/// within the base output directory, instead of returning the base directory itself.
fn find_album_directory(base_dir: &std::path::Path) -> Option<String> {
    let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;

    if let Ok(entries) = std::fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Check recursively for audio files in this subdirectory
                if has_audio_files(&path) {
                    if let Ok(meta) = std::fs::metadata(&path) {
                        if let Ok(modified) = meta.modified() {
                            if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                                best = Some((modified, path));
                            }
                        }
                    }
                }
            }
        }
    }

    // If no subdirectory with audio files found, check if base itself has audio
    if best.is_none() && has_audio_files(base_dir) {
        return Some(base_dir.to_string_lossy().to_string());
    }

    best.map(|(_, p)| p.to_string_lossy().to_string())
}

// ============================================================
// Queue item (internal representation with extra tracking fields)
// ============================================================

/// Internal representation of a download job in the queue.
///
/// Contains the public-facing `QueueItemStatus` (sent to the frontend) plus
/// additional private tracking fields used by the queue manager for retry
/// and fallback logic. The frontend never sees `fallback_index` or
/// `network_retries_left` directly.
#[derive(Debug, Clone)]
struct QueueItem {
    /// Public status sent to the frontend via `get_status()`.
    /// This is the serializable subset of the item's state.
    pub status: QueueItemStatus,
    /// The original download request as submitted by the user.
    /// Preserved for retry operations (retry resets options from this).
    pub request: DownloadRequest,
    /// Merged GAMDL options (user overrides merged with global settings).
    /// These are the "effective" options passed to GAMDL for this download.
    /// Updated during fallback (e.g., codec changes from alac to aac-he).
    pub merged_options: GamdlOptions,
    /// Index into the `settings.music_fallback_chain` array.
    /// 0 = preferred codec (initial attempt), 1 = first fallback, etc.
    /// Incremented by `try_fallback()` on codec-related errors.
    pub fallback_index: usize,
    /// Number of network retry attempts remaining before giving up.
    /// Decremented by `try_network_retry()` on network-related errors.
    pub network_retries_left: u32,
}

// ============================================================
// Persistence types (crash recovery + export/import)
// ============================================================

/// Persistable snapshot of a queue item, saved to `queue.json` for crash recovery.
///
/// Items in active states (Queued/Downloading/Processing) and failed items (Error)
/// are persisted; only Complete and Cancelled items are discarded on restart.
/// Failed items are restored in their Error state so the user can review and
/// manually retry them — they are not auto-retried.
///
/// The original `DownloadRequest` is preserved so that on restore, options are
/// re-merged with the current device's settings (rather than using stale merged
/// options from the previous session).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedQueueItem {
    /// The unique download ID (UUID v4), matching the original `QueueItem`'s ID.
    pub id: String,
    /// The original download request as submitted by the user (URLs + optional overrides).
    pub request: DownloadRequest,
    /// ISO 8601 timestamp of when the download was originally queued.
    pub created_at: String,
    /// Error message for failed items (`None` for active/queued items).
    /// Preserved so the failure reason is visible after app restart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Top-level schema for a `.meedyadl` export file (JSON content inside).
///
/// Used for cross-device queue transfer: export on one machine, import on another.
/// The `version` field enables forward-compatible schema evolution — importers
/// should reject files with `version > 1` until a newer schema is defined.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueExportFile {
    /// Schema version (always 1 for the initial format).
    pub version: u32,
    /// Application identifier (always "`MeedyaDL`").
    pub app: String,
    /// ISO 8601 timestamp of when the export was created.
    pub exported_at: String,
    /// The queue items included in the export.
    pub items: Vec<ExportedItem>,
}

/// A single item within a `.meedyadl` export file.
///
/// Contains only the URLs and per-download overrides; the importing device
/// merges these with its own global settings on import. This means an export
/// created with ALAC settings can be imported on a device configured for AAC
/// and the import will respect the importing device's defaults (unless the
/// original download had explicit per-download overrides).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportedItem {
    /// Apple Music URL(s) for this download.
    pub urls: Vec<String>,
    /// Per-download quality/format overrides (None = use importing device's defaults).
    pub options: Option<GamdlOptions>,
}

// ============================================================
// Download queue manager
// ============================================================

/// The download queue manager. Wrapped in Arc<Mutex<>> for thread-safe
/// access from multiple Tauri commands and background tasks.
///
/// The queue manages the full lifecycle of downloads:
/// Queued -> Downloading -> Processing -> Complete (happy path)
/// Queued -> Downloading -> Error -> (retry/fallback) -> Queued (retry path)
/// Queued -> Cancelled (user cancellation)
///
/// Only `max_concurrent` downloads run simultaneously. When a download
/// finishes, the queue automatically starts the next queued item.
#[derive(Debug)]
pub struct DownloadQueue {
    /// The queue of download jobs (front = next to process).
    /// `VecDeque` allows efficient `push_back` (enqueue) and iteration
    /// to find the next Queued item.
    items: VecDeque<QueueItem>,
    /// Maximum number of concurrent downloads (default: 1).
    /// Limiting to 1 avoids Apple Music rate limiting and reduces
    /// memory usage from concurrent GAMDL processes.
    max_concurrent: usize,
    /// Number of currently active (Downloading/Processing) downloads.
    /// Incremented by `next_pending()`, decremented by `on_task_finished()`.
    active_count: usize,
    /// Maximum number of network retry attempts per download (default: 3).
    /// Each download starts with this many retries; decremented on network errors.
    max_network_retries: u32,
    /// Cached GAMDL version string, populated once on first `process_queue()` call.
    /// Used to determine whether to use native `--song-codec-priority` (>= 2.9.1)
    /// or `MeedyaDL`'s own `try_fallback` system for older versions.
    gamdl_version: Option<String>,
    /// Timestamp of the last pre-flight health check run. Used to avoid running
    /// checks on every `process_queue()` call during a single batch (the function
    /// is called recursively for each item). Reset to `None` when the queue drains.
    /// A 60-second cooldown prevents duplicate warnings during rapid re-processing.
    last_preflight_at: Option<std::time::Instant>,
}

/// Thread-safe handle to the download queue, stored as Tauri managed state.
///
/// This type alias is used throughout the codebase when accessing the queue.
/// Tauri's `State<QueueHandle>` injector provides this to command handlers.
/// Ref: <https://v2.tauri.app/develop/calling-rust/#accessing-managed-state>
pub type QueueHandle = Arc<Mutex<DownloadQueue>>;

/// Creates a new queue handle for use as Tauri managed state.
/// Called once during app initialization (typically in main.rs setup).
#[must_use]
pub fn new_queue_handle() -> QueueHandle {
    Arc::new(Mutex::new(DownloadQueue::new()))
}

impl Default for DownloadQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadQueue {
    /// Creates a new empty download queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            items: VecDeque::new(),
            max_concurrent: 1,
            active_count: 0,
            max_network_retries: 3,
            gamdl_version: None,
            last_preflight_at: None,
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
        // Generate a unique download ID using UUID v4.
        // This ID is used to track the download across the queue, events, and frontend.
        let download_id = uuid::Uuid::new_v4().to_string();

        // Merge per-download overrides (from the frontend's "custom options" UI)
        // with global settings to produce the final set of GAMDL options.
        // For example, a user might override the codec for a specific download
        // while keeping the global output path from settings.
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
                codec_used: Some(merged_options.song_codec.as_ref().map_or_else(
                    || settings.default_song_codec.to_cli_string().to_string(),
                    |c| c.to_cli_string().to_string(),
                )),
                fallback_occurred: false,
                used_wrapper: merged_options.use_wrapper.unwrap_or(false),
                output_is_directory: false,
                warnings: Vec::new(),
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

    /// Returns the public status of all queue items for display in the frontend.
    /// The frontend calls this (via a Tauri command) to render the queue list.
    /// Returns cloned statuses to avoid holding the lock during serialization.
    #[must_use]
    pub fn get_status(&self) -> Vec<QueueItemStatus> {
        self.items.iter().map(|item| item.status.clone()).collect()
    }

    /// Returns summary counts for the queue: (total, active, queued, completed, failed).
    /// Used by the frontend to display queue statistics in the header/badge.
    #[must_use]
    pub fn get_counts(&self) -> (usize, usize, usize, usize, usize) {
        let total = self.items.len();
        // Active includes both Downloading and Processing states
        let active = self
            .items
            .iter()
            .filter(|i| {
                i.status.state == DownloadState::Downloading
                    || i.status.state == DownloadState::Processing
            })
            .count();
        let queued = self
            .items
            .iter()
            .filter(|i| i.status.state == DownloadState::Queued)
            .count();
        let completed = self
            .items
            .iter()
            .filter(|i| i.status.state == DownloadState::Complete)
            .count();
        let failed = self
            .items
            .iter()
            .filter(|i| i.status.state == DownloadState::Error)
            .count();
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
                    log::info!("Download {download_id} cancelled (was queued)");
                    true
                }
                DownloadState::Downloading | DownloadState::Processing => {
                    item.status.state = DownloadState::Cancelled;
                    // The active_count will be decremented when the running task
                    // detects the cancellation and stops
                    log::info!("Download {download_id} marked for cancellation");
                    true
                }
                _ => {
                    log::debug!("Download {download_id} already in terminal state");
                    false
                }
            }
        } else {
            log::warn!("Download {download_id} not found in queue");
            false
        }
    }

    /// Removes completed and cancelled items from the queue.
    /// Errored items are kept so the user can review and retry them.
    ///
    /// # Returns
    /// Number of items removed.
    pub fn clear_finished(&mut self) -> usize {
        let before = self.items.len();
        self.items.retain(|item| {
            !matches!(
                item.status.state,
                DownloadState::Complete | DownloadState::Cancelled
            )
        });
        let removed = before - self.items.len();
        if removed > 0 {
            log::info!("Cleared {removed} finished items from queue");
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
    ///
    /// Called by the stdout/stderr reader tasks in `run_download_with_events()`
    /// each time a line is parsed from GAMDL's output. The event type determines
    /// which status fields are updated:
    ///
    /// - `DownloadProgress`: Updates percentage, speed, ETA (shown in progress bar)
    /// - `TrackInfo`: Updates current track name (shown above progress bar)
    /// - `ProcessingStep`: Transitions state to Processing (e.g., remuxing, tagging)
    /// - Complete: Sets output path and 100% progress
    /// - Error: Records the error message for display
    pub fn update_item_progress(&mut self, download_id: &str, event: &process::GamdlOutputEvent) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            match event {
                process::GamdlOutputEvent::DownloadProgress {
                    percent,
                    speed,
                    eta,
                } => {
                    // Update real-time progress metrics from GAMDL's tqdm-style progress bar
                    item.status.progress = *percent;
                    item.status.speed = Some(speed.clone());
                    item.status.eta = Some(eta.clone());
                    item.status.state = DownloadState::Downloading;
                }
                process::GamdlOutputEvent::TrackInfo { title, artist, .. } => {
                    // Format the current track as "Artist - Title" or just "Title"
                    let track_name = if artist.is_empty() {
                        title.clone()
                    } else {
                        format!("{artist} - {title}")
                    };
                    item.status.current_track = Some(track_name);
                }
                process::GamdlOutputEvent::ProcessingStep { .. } => {
                    // Processing state covers post-download steps like remuxing,
                    // metadata tagging, and cover art embedding
                    item.status.state = DownloadState::Processing;
                }
                process::GamdlOutputEvent::Complete { path } => {
                    // Set the output file/directory path for the "Open" button in the UI
                    item.status.output_path = Some(path.clone());
                    item.status.progress = 100.0;
                }
                process::GamdlOutputEvent::Error { message } => {
                    // Record the error but don't change state yet — the process
                    // may still be running and the error handling in process_queue()
                    // will determine the final state (retry, fallback, or Error).
                    item.status.error = Some(message.clone());
                }
                // Unknown events are raw GAMDL output lines that don't match
                // any recognized pattern; they're logged for debugging but
                // don't affect queue item state.
                process::GamdlOutputEvent::Unknown { .. } => {}
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

    /// Appends non-fatal warnings to a download item. These are displayed
    /// in the queue UI as amber text below the URL, indicating that the
    /// download succeeded but encountered issues during the run.
    pub fn add_warnings(&mut self, download_id: &str, warnings: &[String]) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.warnings.extend(warnings.iter().cloned());
        }
    }

    /// Checks if a download should attempt a fallback codec/resolution.
    ///
    /// The fallback chain is defined in `AppSettings::music_fallback_chain`, e.g.:
    /// `[Alac, AacHe, AacBinaural]`
    ///
    /// On each codec error, we advance to the next codec in the chain.
    /// This handles the case where Apple Music doesn't offer a track in the
    /// user's preferred codec (e.g., ALAC not available for all tracks).
    ///
    /// The item is reset to Queued state so `process_queue()` will pick it up again.
    ///
    /// # Returns
    /// `Some(new_options)` if fallback should be attempted, `None` if all fallbacks exhausted.
    pub fn try_fallback(
        &mut self,
        download_id: &str,
        settings: &AppSettings,
    ) -> Option<GamdlOptions> {
        let item = self.items.iter_mut().find(|i| i.status.id == download_id)?;

        // Only attempt fallback if the user has enabled it in settings
        if !settings.fallback_enabled {
            return None;
        }

        // Advance to the next codec in the fallback chain
        item.fallback_index += 1;

        if item.fallback_index < settings.music_fallback_chain.len() {
            // Get the next codec to try from the fallback chain
            let next_codec = &settings.music_fallback_chain[item.fallback_index];
            let mut new_options = item.merged_options.clone();
            new_options.song_codec = Some(next_codec.clone());

            // Override song_codec_priority with just this single codec.
            // This serves two purposes:
            // 1. Prevents process_download_item() from rebuilding the full
            //    priority chain (the is_none() check will be false).
            // 2. Overrides the config.ini song_codec_priority (which still
            //    has the full chain) so GAMDL tries only this one codec.
            // Using --song-codec-priority with one codec is equivalent to
            // --song-codec but also overrides the config.ini key.
            new_options.song_codec_priority = Some(next_codec.to_cli_string().to_string());

            // If the companion mode would produce companions for this fallback
            // codec, apply the codec suffix to file templates so the specialist
            // format files don't collide with the companion files.
            if needs_primary_suffix(next_codec, &settings.companion_mode) {
                apply_codec_suffix(&mut new_options);
            }

            // Update tracking info for the frontend to display
            item.status.codec_used = Some(next_codec.to_cli_string().to_string());
            item.status.fallback_occurred = true;
            // Reset the item to Queued so process_queue() will start it again
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
            // All codecs in the fallback chain have been tried and failed.
            // The download will remain in the Error state.
            log::info!("Download {download_id} exhausted all fallback codecs");
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
                log::info!("Download {download_id} exhausted network retries");
                false
            }
        } else {
            false
        }
    }

    /// Gets the next queued item's download ID and options for execution.
    ///
    /// This is the "scheduler" — it decides whether a new download can start.
    /// Returns None if:
    /// - No items are in the Queued state
    /// - The max concurrent limit has been reached
    ///
    /// When an item is selected, it transitions from Queued -> Downloading
    /// and the active count is incremented. The caller (`process_queue`) must
    /// eventually call `on_task_finished()` when the download completes.
    pub fn next_pending(&mut self) -> Option<(String, Vec<String>, GamdlOptions)> {
        // Check if we're at the concurrent download limit
        if self.active_count >= self.max_concurrent {
            return None;
        }

        // Find the first Queued item (FIFO order from VecDeque front)
        let item = self
            .items
            .iter_mut()
            .find(|i| i.status.state == DownloadState::Queued)?;
        // Transition to Downloading and increment active count
        item.status.state = DownloadState::Downloading;
        self.active_count += 1;

        // Return the data needed to start the download
        Some((
            item.status.id.clone(),
            item.status.urls.clone(),
            item.merged_options.clone(),
        ))
    }

    /// Called when a download task finishes (success, error, or cancel).
    /// Decrements the active count so new downloads can start.
    /// This must be called exactly once per `next_pending()` call to keep
    /// the `active_count` accurate. The guard `if self.active_count > 0`
    /// prevents underflow in edge cases.
    pub const fn on_task_finished(&mut self) {
        if self.active_count > 0 {
            self.active_count -= 1;
        }
    }

    /// Checks if a download has been cancelled by the user.
    /// Called by the cancellation polling loop in `run_download_with_events()`
    /// every 250ms to detect if the user cancelled while the process is running.
    /// If true, the caller should kill the GAMDL subprocess.
    #[must_use]
    pub fn is_cancelled(&self, download_id: &str) -> bool {
        self.items
            .iter()
            .find(|i| i.status.id == download_id)
            .is_some_and(|i| i.status.state == DownloadState::Cancelled)
    }

    /// Retries a failed or cancelled download by fully resetting it to the Queued state.
    ///
    /// This is a "full reset" — the download starts from scratch with fresh options
    /// (re-merged from the original request + current settings), a reset fallback
    /// index, and full network retry budget. This differs from automatic retries
    /// (`try_fallback`, `try_network_retry`) which only adjust specific fields.
    ///
    /// Called by the frontend's "Retry" button via a Tauri command.
    ///
    /// # Returns
    /// `true` if the item was found and reset, `false` otherwise.
    pub fn retry(&mut self, download_id: &str, settings: &AppSettings) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            if item.status.state == DownloadState::Error
                || item.status.state == DownloadState::Cancelled
            {
                // Re-merge options from the original request with current settings.
                // This picks up any settings changes the user made since the original attempt.
                item.merged_options = merge_options(item.request.options.as_ref(), settings);
                // Reset fallback and retry counters to their initial values
                item.fallback_index = 0;
                item.network_retries_left = self.max_network_retries;
                // Reset status fields for a fresh start
                item.status.state = DownloadState::Queued;
                item.status.error = None;
                item.status.progress = 0.0;
                item.status.fallback_occurred = false;
                item.status.used_wrapper = item.merged_options.use_wrapper.unwrap_or(false);
                item.status.codec_used = Some(item.merged_options.song_codec.as_ref().map_or_else(
                    || settings.default_song_codec.to_cli_string().to_string(),
                    |c| c.to_cli_string().to_string(),
                ));
                log::info!("Download {download_id} reset for retry");
                return true;
            }
        }
        false
    }

    /// Retries a failed download with wrapper authentication disabled.
    ///
    /// Clones the item's original request, disables wrapper in the merged
    /// options, and resets the item to Queued state. This allows users to
    /// fall back to cookie-based authentication when the wrapper service
    /// is down or misconfigured.
    ///
    /// Only applies to items that were originally attempted with wrapper
    /// enabled and are in an error or cancelled state.
    ///
    /// # Arguments
    /// * `download_id` - The unique ID of the failed download to retry
    /// * `settings` - Current app settings for option re-merging
    ///
    /// # Returns
    /// `true` if the item was found, was wrapper-enabled, and was reset.
    pub fn retry_without_wrapper(&mut self, download_id: &str, settings: &AppSettings) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            if (item.status.state == DownloadState::Error
                || item.status.state == DownloadState::Cancelled)
                && item.status.used_wrapper
            {
                // Re-merge options from the original request with current settings
                item.merged_options = merge_options(item.request.options.as_ref(), settings);
                // Override wrapper settings: disable wrapper, clear wrapper URLs
                item.merged_options.use_wrapper = Some(false);
                item.merged_options.wrapper_account_url = None;
                item.merged_options.wrapper_decrypt_ip = None;
                // Reset counters and state
                item.fallback_index = 0;
                item.network_retries_left = self.max_network_retries;
                item.status.state = DownloadState::Queued;
                item.status.error = None;
                item.status.progress = 0.0;
                item.status.fallback_occurred = false;
                item.status.used_wrapper = false;
                item.status.codec_used = Some(item.merged_options.song_codec.as_ref().map_or_else(
                    || settings.default_song_codec.to_cli_string().to_string(),
                    |c| c.to_cli_string().to_string(),
                ));
                log::info!("Download {download_id} reset for retry without wrapper");
                return true;
            }
        }
        false
    }

    // ==========================================================
    // Pre-flight health check helpers
    // ==========================================================

    /// Returns true if pre-flight checks should run.
    ///
    /// Pre-flight checks run once per queue batch (not per-item) with a
    /// 60-second cooldown to prevent duplicate warnings when `process_queue()`
    /// is called recursively for cascading items.
    #[must_use]
    pub fn should_run_preflight(&self) -> bool {
        match self.last_preflight_at {
            None => true,
            Some(t) => t.elapsed() > std::time::Duration::from_secs(60),
        }
    }

    /// Marks that pre-flight checks have been run, starting the cooldown timer.
    pub fn mark_preflight_run(&mut self) {
        self.last_preflight_at = Some(std::time::Instant::now());
    }

    // ==========================================================
    // Persistence and export/import methods
    // ==========================================================

    /// Returns persistable snapshots of all non-terminal queue items.
    ///
    /// Called by `save_queue_to_disk()` to capture queue state for crash recovery.
    /// Only items in Queued, Downloading, or Processing states are included;
    /// completed/failed/cancelled items are not persisted (they are cleared
    /// on restart per the user's preference).
    #[must_use]
    pub fn get_persistable_items(&self) -> Vec<PersistedQueueItem> {
        self.items
            .iter()
            .filter(|item| {
                // Persist active items AND failed items. Only Complete and
                // Cancelled are discarded — errored items stay so the user
                // can retry them after restarting the app.
                matches!(
                    item.status.state,
                    DownloadState::Queued
                        | DownloadState::Downloading
                        | DownloadState::Processing
                        | DownloadState::Error
                )
            })
            .map(|item| PersistedQueueItem {
                id: item.status.id.clone(),
                request: item.request.clone(),
                created_at: item.status.created_at.clone(),
                error: item.status.error.clone(),
            })
            .collect()
    }

    /// Restores items from persisted data, re-merging with current settings.
    ///
    /// Called during startup to recover the queue after a crash or app close.
    /// Active items (Queued/Downloading/Processing) are reset to Queued so
    /// they re-download from scratch. Failed items (those with a persisted
    /// error message) are restored in Error state so the user can review the
    /// failure reason and manually retry — they are not auto-retried.
    ///
    /// Options are re-merged with the current device's settings so any
    /// changes made since the last session are respected.
    ///
    /// # Arguments
    /// * `persisted` - The items loaded from `queue.json`
    /// * `settings` - The current app settings for option merging
    pub fn restore_items(&mut self, persisted: Vec<PersistedQueueItem>, settings: &AppSettings) {
        for p in persisted {
            // Re-merge the original request's overrides with the current settings.
            // This ensures setting changes made between sessions are respected.
            let merged_options = merge_options(p.request.options.as_ref(), settings);

            // Items with a persisted error are restored in Error state so the
            // user sees the failure reason and can choose to retry. Active
            // items are reset to Queued for automatic re-processing.
            let (state, error) = if p.error.is_some() {
                (DownloadState::Error, p.error)
            } else {
                (DownloadState::Queued, None)
            };

            let item = QueueItem {
                status: QueueItemStatus {
                    id: p.id.clone(),
                    urls: p.request.urls.clone(),
                    state,
                    progress: 0.0,
                    current_track: None,
                    total_tracks: None,
                    completed_tracks: None,
                    speed: None,
                    eta: None,
                    error,
                    output_path: None,
                    codec_used: Some(merged_options.song_codec.as_ref().map_or_else(
                        || settings.default_song_codec.to_cli_string().to_string(),
                        |c| c.to_cli_string().to_string(),
                    )),
                    fallback_occurred: false,
                    used_wrapper: merged_options.use_wrapper.unwrap_or(false),
                    output_is_directory: false,
                    warnings: Vec::new(),
                    created_at: p.created_at,
                },
                request: p.request,
                merged_options,
                fallback_index: 0,
                network_retries_left: self.max_network_retries,
            };
            self.items.push_back(item);
        }
        if !self.items.is_empty() {
            log::info!(
                "Restored {} item(s) from queue persistence",
                self.items.len()
            );
        }
    }

    /// Returns exportable items for the `.meedyadl` export file format.
    ///
    /// Includes all non-terminal items (Queued/Downloading/Processing).
    /// Each item contains only the original URLs and per-download overrides,
    /// so the importing device will merge them with its own settings.
    #[must_use]
    pub fn get_exportable_items(&self) -> Vec<ExportedItem> {
        self.items
            .iter()
            .filter(|item| {
                matches!(
                    item.status.state,
                    DownloadState::Queued | DownloadState::Downloading | DownloadState::Processing
                )
            })
            .map(|item| ExportedItem {
                urls: item.request.urls.clone(),
                options: item.request.options.clone(),
            })
            .collect()
    }

    /// Imports items from an export file, enqueuing each as a new download.
    ///
    /// Each imported item is treated as a fresh download request: a new UUID
    /// is generated, options are merged with the importing device's current
    /// settings, and the item is placed at the back of the queue.
    ///
    /// # Returns
    /// The download IDs of the newly created queue items.
    pub fn import_items(
        &mut self,
        items: Vec<ExportedItem>,
        settings: &AppSettings,
    ) -> Vec<String> {
        items
            .into_iter()
            .map(|exported| {
                let request = DownloadRequest {
                    urls: exported.urls,
                    options: exported.options,
                };
                self.enqueue(request, settings)
            })
            .collect()
    }
}

// ============================================================
// Helper: merge per-download overrides with global settings
// ============================================================

/// Merges per-download option overrides with the global app settings
/// to produce the final set of GAMDL CLI options.
///
/// The merge follows a two-layer priority system:
/// 1. **Global settings** (from `AppSettings`) form the base layer
/// 2. **Per-download overrides** (from the frontend) override specific fields
///
/// This allows users to set global defaults (e.g., always use ALAC) while
/// still customizing individual downloads (e.g., this one in AAC-HE).
///
/// The resulting `GamdlOptions` struct is what actually gets passed to
/// `gamdl_service::build_gamdl_command_public()` to construct the CLI command.
#[allow(clippy::field_reassign_with_default)]
fn merge_options(overrides: Option<&GamdlOptions>, settings: &AppSettings) -> GamdlOptions {
    let mut options = GamdlOptions::default();

    // === Layer 1: Apply global settings as the base ===
    // These come from the user's saved settings (settings.json).
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

    // Resolve temp_path: use the user's custom path if set, otherwise fall
    // back to a "MeedyaDL" subdirectory within the OS temp directory (e.g.
    // /var/folders/.../MeedyaDL on macOS, %TEMP%\MeedyaDL on Windows,
    // /tmp/MeedyaDL on Linux). Using a dedicated subdirectory keeps
    // intermediate files isolated and easy to clean up. This avoids GAMDL's
    // default of "." which is unwritable from /Applications on macOS.
    if settings.temp_path.is_empty() {
        options.temp_path = Some(
            std::env::temp_dir()
                .join("MeedyaDL")
                .to_string_lossy()
                .to_string(),
        );
    } else {
        options.temp_path = Some(settings.temp_path.clone());
    }

    // Apply tool paths from settings
    options.cookies_path.clone_from(&settings.cookies_path);
    options.ffmpeg_path.clone_from(&settings.ffmpeg_path);
    options
        .mp4decrypt_path
        .clone_from(&settings.mp4decrypt_path);
    options.mp4box_path.clone_from(&settings.mp4box_path);
    options.nm3u8dlre_path.clone_from(&settings.nm3u8dlre_path);

    // Set download and remux modes
    options.download_mode = Some(settings.download_mode.clone());
    options.remux_mode = Some(settings.remux_mode.clone());

    // Apply metadata options
    options.fetch_extra_tags = Some(settings.fetch_extra_tags);

    // Apply exclude tags
    if !settings.exclude_tags.is_empty() {
        options.exclude_tags = Some(settings.exclude_tags.join(","));
    }

    // Apply artist auto-selection mode (GAMDL >= 2.9.1)
    options
        .artist_auto_select
        .clone_from(&settings.artist_auto_select);

    // === Layer 2: Apply per-download overrides (highest priority) ===
    // Only non-None fields from the override replace the global values.
    // This selective merge allows partial overrides (e.g., only change codec).
    if let Some(overrides) = overrides {
        if overrides.song_codec.is_some() {
            options.song_codec.clone_from(&overrides.song_codec);
        }
        if overrides.music_video_resolution.is_some() {
            options
                .music_video_resolution
                .clone_from(&overrides.music_video_resolution);
        }
        if overrides.music_video_codec_priority.is_some() {
            options
                .music_video_codec_priority
                .clone_from(&overrides.music_video_codec_priority);
        }
        if overrides.music_video_remux_format.is_some() {
            options
                .music_video_remux_format
                .clone_from(&overrides.music_video_remux_format);
        }
        if overrides.output_path.is_some() {
            options.output_path.clone_from(&overrides.output_path);
        }
        if overrides.overwrite.is_some() {
            options.overwrite = overrides.overwrite;
        }
        if overrides.artist_auto_select.is_some() {
            options
                .artist_auto_select
                .clone_from(&overrides.artist_auto_select);
        }
    }

    // === Layer 3: Lyrics embed + sidecar enforcement ===
    // When the user has enabled "Embed Lyrics and Keep Sidecar", ensure that:
    // 1. Lyrics are NOT excluded from metadata embedding (remove "lyrics" from
    //    exclude_tags if present, so GAMDL embeds them in the audio file's tags).
    // 2. Synced lyrics sidecar files are NOT disabled (force no_synced_lyrics
    //    to false, so GAMDL creates the .lrc/.srt/.ttml sidecar alongside).
    // This provides maximum compatibility: embedded for players that read tags,
    // sidecar for those that look for external lyrics files.
    if settings.embed_lyrics_and_sidecar {
        // Remove "lyrics" from the exclude_tags comma-separated string so
        // GAMDL embeds lyrics in the audio file's metadata atoms.
        if let Some(ref tags) = options.exclude_tags {
            let filtered: Vec<&str> = tags
                .split(',')
                .map(str::trim)
                .filter(|t| !t.eq_ignore_ascii_case("lyrics"))
                .collect();
            if filtered.is_empty() {
                options.exclude_tags = None;
            } else {
                options.exclude_tags = Some(filtered.join(","));
            }
        }
        // Force sidecar lyrics creation regardless of the no_synced_lyrics setting.
        options.no_synced_lyrics = Some(false);
    }

    // === Layer 4: Enhanced LRC enforcement ===
    // When Enhanced LRC is enabled, force TTML as the primary lyrics format
    // so the raw word-level timing data is preserved in the sidecar file.
    // GAMDL's TTML output retains the full XML including <span> word timestamps,
    // which are then converted to Enhanced LRC in the enrichment pipeline.
    // This runs after Layer 3 so it overrides the lyrics format regardless
    // of what the user selected, ensuring TTML is always available for conversion.
    if settings.enhanced_lrc {
        options.synced_lyrics_format = Some(LyricsFormat::Ttml);
        // Ensure sidecar creation (needed for TTML → Enhanced LRC conversion)
        options.no_synced_lyrics = Some(false);
    }

    options
}

// ============================================================
// Helper: codec-based filename suffix system
// ============================================================

/// Returns the filename suffix for a given audio codec, or `None` if the
/// codec should use a clean (unsuffixed) filename.
///
/// Suffix rules:
/// - **Lossy codecs** (AAC, AAC-Legacy, AAC-Binaural, AC3, etc.) get no
///   suffix, as they represent the "standard" download a user would expect.
/// - **Lossless** (ALAC) gets `[Lossless]` to distinguish from lossy versions.
/// - **Spatial audio** (Dolby Atmos) gets `[Dolby Atmos]` to clearly identify
///   the immersive mix.
///
/// When companion downloads are enabled, multiple codec versions of the same
/// track can coexist in the same album folder. The suffix prevents filename
/// collisions and makes the format instantly visible in file browsers.
const fn codec_suffix(codec: &SongCodec) -> Option<&'static str> {
    match codec {
        SongCodec::Alac => Some("[Lossless]"),
        SongCodec::Atmos => Some("[Dolby Atmos]"),
        // Dolby Digital gets a suffix when used as a companion alongside Atmos
        SongCodec::Ac3 => Some("[Dolby Digital]"),
        // Lossy, legacy, and experimental codecs use clean filenames (no suffix)
        SongCodec::Aac
        | SongCodec::AacLegacy
        | SongCodec::AacBinaural
        | SongCodec::AacHeLegacy
        | SongCodec::AacHe
        | SongCodec::AacDownmix
        | SongCodec::AacHeBinaural
        | SongCodec::AacHeDownmix => None,
    }
}

/// Determines whether the primary download's file templates should have a
/// codec suffix applied, based on the companion mode and the download's codec.
///
/// A suffix is needed when the companion mode will produce at least one
/// companion with a clean filename alongside the primary download. This
/// prevents filename collisions in the same album directory.
///
/// # Rules per mode
///
/// | Mode                       | Atmos gets suffix? | ALAC gets suffix? | Others? |
/// |----------------------------|--------------------|-------------------|---------|
/// | `Disabled`                 | No                 | No                | No      |
/// | `AtmosToLossless`          | Yes                | No                | No      |
/// | `AtmosToLosslessAndLossy`  | Yes                | Yes               | No      |
/// | `SpecialistToLossy`        | Yes                | Yes               | No      |
/// | `AtmosToAllFormats`        | Yes                | No                | No      |
const fn needs_primary_suffix(codec: &SongCodec, mode: &CompanionMode) -> bool {
    match mode {
        // No companions → no suffix needed (only one version exists)
        CompanionMode::Disabled => false,
        // Only Atmos gets a companion (ALAC or all formats), so only Atmos
        // needs a suffix. ALAC downloads in these modes have no companion → no suffix.
        CompanionMode::AtmosToLossless | CompanionMode::AtmosToAllFormats => {
            matches!(codec, SongCodec::Atmos)
        }
        // Both Atmos and ALAC get companions (at least a lossy one),
        // so both need suffixes to coexist with the clean-filename companion.
        // Same for SpecialistToLossy: any specialist codec gets a lossy companion.
        CompanionMode::AtmosToLosslessAndLossy | CompanionMode::SpecialistToLossy => {
            matches!(codec, SongCodec::Atmos | SongCodec::Alac)
        }
    }
}

/// A planned companion download tier. Each tier represents one additional
/// GAMDL invocation to download the same content in a different codec.
struct CompanionTier {
    /// Codecs to try in order for this companion tier. The first codec that
    /// succeeds ends the tier (remaining codecs are skipped). If all fail,
    /// the tier is skipped silently.
    codecs_to_try: Vec<SongCodec>,
    /// Whether to apply a codec suffix to this companion's file templates.
    /// `true` means this companion gets a suffixed filename (e.g., `[Lossless]`);
    /// `false` means this companion gets the clean (unsuffixed) filename.
    apply_suffix: bool,
}

/// Plans the companion downloads to perform after a primary download
/// succeeds, based on the companion mode and the primary codec used.
///
/// Returns an ordered list of `CompanionTier` structs. Each tier is
/// processed sequentially (to avoid concurrent GAMDL processes writing to
/// the same directory). Within a tier, codecs are tried in order until one
/// succeeds.
///
/// # Examples
///
/// `AtmosToLossless` with primary `"atmos"`:
/// → `[CompanionTier { codecs: [ALAC], suffix: false }]`
///
/// `AtmosToLosslessAndLossy` with primary `"atmos"`:
/// → `[CompanionTier { codecs: [ALAC], suffix: true },
///     CompanionTier { codecs: [AAC, AacLegacy], suffix: false }]`
fn plan_companions(mode: &CompanionMode, primary_codec: &str) -> Vec<CompanionTier> {
    match mode {
        // No companions in any scenario
        CompanionMode::Disabled => vec![],

        // Atmos → ALAC companion (clean filename); nothing for other codecs
        CompanionMode::AtmosToLossless => {
            if primary_codec == "atmos" {
                vec![CompanionTier {
                    codecs_to_try: vec![SongCodec::Alac],
                    apply_suffix: false, // ALAC companion gets clean filename
                }]
            } else {
                vec![]
            }
        }

        // Maximum coverage:
        //   Atmos → ALAC [Lossless] + AAC (clean)
        //   ALAC → AAC (clean)
        CompanionMode::AtmosToLosslessAndLossy => {
            if primary_codec == "atmos" {
                vec![
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Alac],
                        apply_suffix: true, // ALAC gets [Lossless] suffix (AAC exists too)
                    },
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Aac, SongCodec::AacLegacy],
                        apply_suffix: false, // Lossy AAC gets clean filename
                    },
                ]
            } else if primary_codec == "alac" {
                vec![CompanionTier {
                    codecs_to_try: vec![SongCodec::Aac, SongCodec::AacLegacy],
                    apply_suffix: false, // Lossy AAC gets clean filename
                }]
            } else {
                vec![]
            }
        }

        // Any specialist → lossy companion (clean filename)
        CompanionMode::SpecialistToLossy => {
            if primary_codec == "atmos" || primary_codec == "alac" {
                vec![CompanionTier {
                    codecs_to_try: vec![SongCodec::Aac, SongCodec::AacLegacy],
                    apply_suffix: false, // Lossy AAC gets clean filename
                }]
            } else {
                vec![]
            }
        }

        // Atmos → all formats: AC3 [Dolby Digital] + ALAC [Lossless] + AAC (clean)
        // 4 files per track total (Atmos + 3 companions)
        CompanionMode::AtmosToAllFormats => {
            if primary_codec == "atmos" {
                vec![
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Ac3],
                        apply_suffix: true, // AC3 gets [Dolby Digital] suffix
                    },
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Alac],
                        apply_suffix: true, // ALAC gets [Lossless] suffix
                    },
                    CompanionTier {
                        codecs_to_try: vec![SongCodec::Aac, SongCodec::AacLegacy],
                        apply_suffix: false, // AAC gets clean filename
                    },
                ]
            } else {
                vec![]
            }
        }
    }
}

/// Appends a codec-specific suffix to all file naming templates in a
/// `GamdlOptions` struct.
///
/// When companion downloads are enabled, multiple codec versions of the
/// same track land in the same album directory. To prevent filename
/// collisions and clearly identify the format, this function appends the
/// codec's suffix (e.g., ` [Lossless]` or ` [Dolby Atmos]`) to the
/// filename portion of every file template.
///
/// The most universally compatible companion uses the original (unsuffixed)
/// template, so it gets a "clean" filename (e.g., `01 Song Title.m4a`)
/// while specialist formats get tagged filenames (e.g.,
/// `01 Song Title [Lossless].m4a` or `01 Song Title [Dolby Atmos].m4a`).
///
/// This modifies the following templates:
/// - `single_disc_file_template` (most common: `{track:02d} {title}`)
/// - `multi_disc_file_template` (`{disc}-{track:02d} {title}`)
/// - `no_album_file_template` (`{title}`)
/// - `playlist_file_template` (`Playlists/{playlist_artist}/{playlist_title}`)
///
/// Returns `true` if a suffix was applied, `false` if the codec has no suffix.
fn apply_codec_suffix(options: &mut GamdlOptions) -> bool {
    // Determine the suffix for the current codec, if any
    let suffix = match &options.song_codec {
        Some(codec) => match codec_suffix(codec) {
            Some(s) => s,
            None => return false, // Lossy codecs get no suffix
        },
        None => return false, // No codec specified
    };

    // For each file template, append the suffix to the existing value.
    // If the template is None (not set), use the GAMDL default with the suffix.
    if let Some(ref template) = options.single_disc_file_template {
        options.single_disc_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.single_disc_file_template = Some(format!("{{track:02d}} {{title}} {suffix}"));
    }

    if let Some(ref template) = options.multi_disc_file_template {
        options.multi_disc_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.multi_disc_file_template =
            Some(format!("{{disc}}-{{track:02d}} {{title}} {suffix}"));
    }

    if let Some(ref template) = options.no_album_file_template {
        options.no_album_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.no_album_file_template = Some(format!("{{title}} {suffix}"));
    }

    if let Some(ref template) = options.playlist_file_template {
        options.playlist_file_template = Some(format!("{template} {suffix}"));
    } else {
        options.playlist_file_template = Some(format!(
            "Playlists/{{playlist_artist}}/{{playlist_title}} {suffix}"
        ));
    }

    true
}

// ============================================================
// Queue processing: runs downloads and handles fallback/retry
/// Spawns companion and lyrics companion downloads as background tasks.
///
/// Companions are independent downloads of the same content in different
/// codecs (e.g., ALAC companion for an Atmos primary). They fire regardless
/// of whether the primary download succeeded or failed — the companion codec
/// may succeed where the primary format was unavailable.
///
/// This is called after any terminal download outcome (success or final
/// failure), but NOT after fallback/network retries (where the download
/// will be re-attempted and companions will fire on the final outcome).
fn spawn_companion_downloads(
    app: &tauri::AppHandle,
    dl_id: &str,
    urls: &[String],
    primary_codec_str: &str,
    companion_base_options: &GamdlOptions,
) {
    let companion_settings = load_settings_for_queue(app);
    let companion_tiers = plan_companions(&companion_settings.companion_mode, primary_codec_str);

    if !companion_tiers.is_empty() {
        let comp_app = app.clone();
        let comp_urls = urls.to_vec();
        let comp_base_opts = companion_base_options.clone();
        let comp_dl_id = dl_id.to_string();

        emit_internal_log(
            app,
            dl_id,
            &format!(
                "Starting companion downloads ({} tier(s), mode: {:?})",
                companion_tiers.len(),
                companion_settings.companion_mode,
            ),
        );

        tokio::spawn(async move {
            // Process each companion tier sequentially
            for (tier_idx, tier) in companion_tiers.iter().enumerate() {
                let mut tier_succeeded = false;

                // Try each codec in the tier until one succeeds
                for codec in &tier.codecs_to_try {
                    let mut opts = comp_base_opts.clone();
                    opts.song_codec = Some(codec.clone());
                    // Companions always target a specific single codec,
                    // so clear any native priority to force --song-codec.
                    opts.song_codec_priority = None;

                    // If this tier needs a suffix (e.g., ALAC
                    // companion in AtmosToLosslessAndLossy mode
                    // gets [Lossless]), apply it to the options.
                    if tier.apply_suffix {
                        apply_codec_suffix(&mut opts);
                    }
                    // If not suffixed, the base options already
                    // have clean (unsuffixed) templates.

                    // Build the GAMDL CLI command for the companion
                    let mut cmd = match gamdl_service::build_gamdl_command_public(
                        &comp_app, &comp_urls, &opts,
                    ) {
                        Ok(c) => c,
                        Err(e) => {
                            log::debug!(
                                "Companion tier {}: failed to build \
                                 command ({}) for {}: {}",
                                tier_idx,
                                codec.to_cli_string(),
                                comp_dl_id,
                                e
                            );
                            continue; // Try next codec in tier
                        }
                    };

                    // Pipe stdout/stderr for the companion process.
                    // We don't parse progress events (fire-and-forget),
                    // but we capture output for error diagnosis.
                    cmd.stdout(std::process::Stdio::piped());
                    cmd.stderr(std::process::Stdio::piped());

                    match cmd.spawn() {
                        Ok(child) => {
                            match child.wait_with_output().await {
                                Ok(output) if output.status.success() => {
                                    log::info!(
                                        "Companion tier {} ({}) downloaded \
                                         for {}",
                                        tier_idx,
                                        codec.to_cli_string(),
                                        comp_dl_id
                                    );
                                    emit_internal_log(
                                        &comp_app,
                                        &comp_dl_id,
                                        &format!(
                                            "Companion download complete (tier {}, codec: {})",
                                            tier_idx,
                                            codec.to_cli_string(),
                                        ),
                                    );
                                    let _ = comp_app.emit("companion-downloaded", &comp_dl_id);

                                    // Apply custom metadata tags for specialist
                                    // companion codecs (e.g., isLossless=Y for
                                    // ALAC). Only ALAC and Atmos have custom
                                    // tags; lossy codecs return immediately.
                                    if let Some(ref output_dir) = opts.output_path {
                                        match super::metadata_tag_service::apply_codec_metadata_tags(
                                            output_dir, codec,
                                        ) {
                                            Ok(count) if count > 0 => {
                                                log::info!(
                                                    "Tagged {} companion file(s) \
                                                     with {} metadata for {}",
                                                    count,
                                                    codec.to_cli_string(),
                                                    comp_dl_id
                                                );
                                            }
                                            Ok(_) => {}
                                            Err(e) => {
                                                log::debug!(
                                                    "Companion metadata tagging \
                                                     failed for {comp_dl_id}: {e}"
                                                );
                                            }
                                        }
                                    }

                                    tier_succeeded = true;
                                    break; // This tier done, move to next
                                }
                                Ok(output) => {
                                    // Non-zero exit: codec may be
                                    // unavailable for this content
                                    let stderr = String::from_utf8_lossy(&output.stderr);
                                    log::debug!(
                                        "Companion tier {} ({}) failed \
                                         for {}: {}",
                                        tier_idx,
                                        codec.to_cli_string(),
                                        comp_dl_id,
                                        stderr.lines().last().unwrap_or("")
                                    );
                                    // Continue to next codec in tier
                                }
                                Err(e) => {
                                    log::debug!("Companion process error: {e}");
                                }
                            }
                        }
                        Err(e) => {
                            log::debug!("Failed to spawn companion: {e}");
                        }
                    }
                }

                if !tier_succeeded {
                    log::debug!("Companion tier {tier_idx} exhausted all codecs for {comp_dl_id}");
                    emit_internal_log(
                        &comp_app,
                        &comp_dl_id,
                        &format!("Companion tier {tier_idx}: all codecs exhausted"),
                    );
                }
            }
        });
    }

    // === Lyrics companion downloads (background, fire-and-forget) ===
    // When the user has selected additional lyrics formats beyond the
    // primary, spawn a lightweight GAMDL invocation for each extra
    // format using --synced-lyrics-only. This produces sidecar lyrics
    // files (.lrc, .srt, .ttml) without re-downloading audio.
    let lyrics_settings = load_settings_for_queue(app);
    if !lyrics_settings.companion_lyrics_formats.is_empty()
        && (!lyrics_settings.no_synced_lyrics || lyrics_settings.embed_lyrics_and_sidecar)
    {
        let lyrics_app = app.clone();
        let lyrics_urls = urls.to_vec();
        let lyrics_base_opts = companion_base_options.clone();
        let lyrics_dl_id = dl_id.to_string();
        let lyrics_formats = lyrics_settings.companion_lyrics_formats.clone();

        tokio::spawn(async move {
            for format in &lyrics_formats {
                let mut opts = lyrics_base_opts.clone();
                opts.synced_lyrics_format = Some(format.clone());
                opts.synced_lyrics_only = Some(true);

                let mut cmd = match gamdl_service::build_gamdl_command_public(
                    &lyrics_app,
                    &lyrics_urls,
                    &opts,
                ) {
                    Ok(c) => c,
                    Err(e) => {
                        log::debug!(
                            "Lyrics companion ({}) command error: {e}",
                            format.to_cli_string()
                        );
                        continue;
                    }
                };

                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());

                match cmd.spawn() {
                    Ok(child) => match child.wait_with_output().await {
                        Ok(output) if output.status.success() => {
                            log::info!(
                                "Lyrics companion ({}) \
                                     downloaded for {}",
                                format.to_cli_string(),
                                lyrics_dl_id
                            );
                            let _ = lyrics_app.emit(
                                "lyrics-companion-downloaded",
                                serde_json::json!({
                                    "download_id": lyrics_dl_id,
                                    "format": format.to_cli_string(),
                                }),
                            );
                        }
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            log::debug!(
                                "Lyrics companion ({}) \
                                     failed for {}: {}",
                                format.to_cli_string(),
                                lyrics_dl_id,
                                stderr.lines().last().unwrap_or("")
                            );
                        }
                        Err(e) => {
                            log::debug!(
                                "Lyrics companion process \
                                     error: {e}"
                            );
                        }
                    },
                    Err(e) => {
                        log::debug!("Failed to spawn lyrics companion: {e}");
                    }
                }
            }
        });
    }
}

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
// Monolithic by necessity: this function orchestrates the entire queue lifecycle
// including async closures with complex move semantics, cancellation polling,
// fallback/retry decision trees, companion spawning, and recursive self-calls.
// Extracting helper functions would require passing many Arc/Mutex handles and
// would fragment the sequential orchestration logic without meaningful benefit.
#[allow(clippy::too_many_lines)]
pub fn process_queue(
    app: AppHandle,
    queue: QueueHandle,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        // === Pre-flight health checks (once per queue batch) ===
        // Run lightweight health checks before processing to warn the user about
        // potential issues (no internet, expired cookies, wrapper down). These are
        // non-blocking warnings — the queue proceeds regardless. A 60-second
        // cooldown prevents duplicate warnings during cascading process_queue() calls.
        {
            let should_run = {
                let q = queue.lock().await;
                q.should_run_preflight()
            };

            if should_run {
                // Mark as run first to prevent concurrent duplicate checks
                {
                    let mut q = queue.lock().await;
                    q.mark_preflight_run();
                }

                let settings = load_settings_for_queue(&app);

                // Run internet + wrapper checks concurrently (both are HTTP GETs)
                let internet_future =
                    crate::services::health_check_service::check_internet_connectivity();
                let wrapper_future = if settings.use_wrapper {
                    Some(crate::services::health_check_service::check_wrapper_health(
                        &settings.wrapper_account_url,
                    ))
                } else {
                    None
                };

                // Cookie check is synchronous (file I/O only) — run when not using wrapper
                let cookie_warning = if !settings.use_wrapper {
                    if let Some(ref path) = settings.cookies_path {
                        crate::services::health_check_service::validate_cookies(path)
                    } else {
                        Some(crate::services::health_check_service::PreflightWarning {
                        check: crate::services::health_check_service::PreflightCheck::Cookies,
                        message: "No cookies file configured. Apple Music downloads require authentication via cookies or wrapper.".to_string(),
                    })
                    }
                } else {
                    None
                };

                // Await the async checks
                let internet_warning = internet_future.await;
                let wrapper_warning = match wrapper_future {
                    Some(fut) => fut.await,
                    None => None,
                };

                // Build the list of checks that were actually run, paired with their result.
                // Each entry is (check_type, warning_option). We use this to:
                //   - Emit "preflight-warning" for checks that failed
                //   - Emit "preflight-cleared" for checks that passed (so the frontend
                //     can dismiss stale warning toasts from a previous cycle)
                let checks_run: Vec<(
                    crate::services::health_check_service::PreflightCheck,
                    Option<crate::services::health_check_service::PreflightWarning>,
                )> = {
                    use crate::services::health_check_service::PreflightCheck;
                    let mut v = vec![(PreflightCheck::Internet, internet_warning)];
                    if !settings.use_wrapper {
                        v.push((PreflightCheck::Cookies, cookie_warning));
                    }
                    if settings.use_wrapper {
                        v.push((PreflightCheck::Wrapper, wrapper_warning));
                    }
                    v
                };

                for (check, warning) in checks_run {
                    if let Some(w) = warning {
                        log::warn!("Pre-flight warning ({check:?}): {}", w.message);
                        let _ = app.emit("preflight-warning", &w);
                    } else {
                        // Check passed — dismiss any stale toast for this check type
                        let _ = app.emit(
                            "preflight-cleared",
                            &serde_json::json!({ "check": format!("{check:?}") }),
                        );
                    }
                }
            }
        }

        // Acquire the queue lock briefly to check for the next pending item.
        // The lock is released immediately after to avoid holding it during the download.
        let pending = {
            let mut q = queue.lock().await;
            q.next_pending()
        };

        // If no items are pending (queue empty or max concurrent reached), exit.
        let Some((download_id, urls, mut options)) = pending else {
            return;
        };

        log::info!("Processing download {download_id}");

        // === GAMDL version detection (cached, runs once per queue lifetime) ===
        // Detect the installed GAMDL version on the first download so we can
        // decide whether to use native `--song-codec-priority` (>= 2.9.1) or
        // our own `try_fallback` system for older versions.
        let gamdl_version = {
            let mut q = queue.lock().await;
            if q.gamdl_version.is_none() {
                // First download in this queue session — detect version once
                match gamdl_service::get_gamdl_version(&app).await {
                    Ok(Some(ver)) => {
                        log::info!("Detected GAMDL version: {ver}");
                        q.gamdl_version = Some(ver);
                    }
                    Ok(None) => {
                        log::warn!("GAMDL not installed — version detection skipped");
                    }
                    Err(e) => {
                        log::warn!("Failed to detect GAMDL version: {e}");
                    }
                }
            }
            q.gamdl_version.clone()
        };

        // === Native codec priority (GAMDL >= 2.9.1) ===
        // When GAMDL supports `--song-codec-priority`, build the priority string
        // from the user's preferred codec + fallback chain. GAMDL tries each codec
        // in order within a single process, which is much more efficient than our
        // `try_fallback` system (which spawns one process per codec attempt).
        // If native priority fails, MeedyaDL's own `try_fallback()` still runs as
        // a safety net, retrying each codec individually via `--song-codec-priority`
        // with a single codec (see `try_fallback()` and the "codec" error handler).
        let uses_native_priority = if let Some(ref ver) = gamdl_version {
            let settings_for_priority = load_settings_for_queue(&app);
            if gamdl_service::is_version_at_least(ver, "2.9.1")
                && settings_for_priority.fallback_enabled
                && !settings_for_priority.music_fallback_chain.is_empty()
                && options.song_codec_priority.is_none()
            {
                // Build priority string: preferred codec first, then remaining
                // fallback chain entries (deduped to avoid redundant attempts)
                let mut seen = HashSet::new();
                let mut priority_codecs: Vec<String> = Vec::new();

                // Start with the preferred codec from the merged options
                if let Some(ref codec) = options.song_codec {
                    let cli_str = codec.to_cli_string().to_string();
                    seen.insert(cli_str.clone());
                    priority_codecs.push(cli_str);
                }

                // Append remaining fallback chain entries, skipping duplicates
                for codec in &settings_for_priority.music_fallback_chain {
                    let cli_str = codec.to_cli_string().to_string();
                    if seen.insert(cli_str.clone()) {
                        priority_codecs.push(cli_str);
                    }
                }

                if priority_codecs.len() > 1 {
                    let priority_str = priority_codecs.join(",");
                    log::info!(
                        "Download {download_id} using native codec priority: {priority_str}"
                    );
                    emit_internal_log(
                        &app,
                        &download_id,
                        &format!(
                            "Using GAMDL native format priority: {}",
                            priority_str.replace(',', " → ")
                        ),
                    );
                    options.song_codec_priority = Some(priority_str);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        // === Codec suffix: modify file templates for companion coexistence ===
        // When the companion mode would produce companions for this codec,
        // add a suffix to file naming templates so specialist format files
        // get tagged filenames (e.g., "01 Song Title [Lossless].m4a") while
        // the companion download uses clean filenames ("01 Song Title.m4a").
        // Keep the original (unsuffixed) options for companion downloads later.
        let companion_base_options = options.clone();
        let mut download_options = options;
        let settings_for_companion = load_settings_for_queue(&app);
        if let Some(ref codec) = download_options.song_codec {
            if needs_primary_suffix(codec, &settings_for_companion.companion_mode) {
                apply_codec_suffix(&mut download_options);
                log::info!(
                    "Download {} using codec with file suffix (companion mode: {:?})",
                    download_id,
                    settings_for_companion.companion_mode
                );
            }
        }

        // Notify the frontend that this download is starting.
        // The frontend uses this event to transition the download card's UI state.
        let _ = app.emit("download-started", &download_id);

        // Extract the primary codec string for companion planning. This is the
        // codec the user requested (e.g., "atmos"), used to determine which
        // companion tiers to spawn after the download attempt completes.
        let primary_codec_for_companions = download_options
            .song_codec
            .as_ref()
            .map_or_else(String::new, |c| c.to_cli_string().to_string());

        // Capture wrapper URL for logging inside the tokio::spawn block.
        // download_options will be moved into the spawn, so extract this before.
        let wrapper_url_for_logging = if download_options.use_wrapper == Some(true) {
            download_options.wrapper_account_url.clone()
        } else {
            None
        };

        // Log wrapper URL at download start for troubleshooting connectivity
        if let Some(ref url) = wrapper_url_for_logging {
            log::info!("Download {download_id} using wrapper at {url}");
        }

        // Spawn the download in a separate tokio task so it runs independently.
        // This allows process_queue() to return immediately while the download runs.
        let app_clone = app.clone();
        let queue_clone = queue.clone();
        let dl_id = download_id;

        tokio::spawn(async move {
            // Run the GAMDL download with real-time event forwarding.
            // This function handles subprocess spawning, output parsing,
            // and cancellation polling. See run_download_with_events() below.
            let result = run_download_with_events(
                &app_clone,
                &dl_id,
                &urls,
                &download_options,
                &queue_clone,
            )
            .await;

            // Handle the result of the download attempt
            match result {
                Ok(warnings) => {
                    // === Success path ===
                    // GAMDL exited with code 0. Check whether output files were
                    // actually produced before declaring success.
                    let (output_path_for_artwork, completed_codec) = {
                        let mut q = queue_clone.lock().await;

                        // Check if GAMDL emitted a "Saved to:" line during the run.
                        // If not, no files were produced despite a clean exit.
                        let has_output = q
                            .items
                            .iter()
                            .find(|i| i.status.id == dl_id)
                            .and_then(|i| i.status.output_path.as_ref())
                            .is_some();

                        if !has_output {
                            // No "Saved to:" line was emitted by GAMDL despite exit 0.
                            // Classify the warnings to determine the right recovery
                            // strategy: codec fallback, IO error recovery, or terminal.
                            let has_codec_error =
                                warnings.iter().any(|w| process::is_codec_error(w));
                            let has_io_error = warnings.iter().any(|w| process::is_io_error(w));

                            if has_codec_error {
                                // Codec-related failure on success path — try fallback
                                let error_msg = warnings.last().cloned().unwrap_or_else(|| {
                                    "Requested format is not available".to_string()
                                });

                                if uses_native_priority {
                                    // GAMDL >= 2.9.1 already tried all codecs natively
                                    // via --song-codec-priority. No point falling back
                                    // to our own try_fallback system — all codecs were
                                    // already attempted in the single GAMDL process.
                                    log::info!(
                                        "Download {dl_id} codec error with native priority \
                                     (all codecs exhausted by GAMDL): {error_msg}"
                                    );
                                    emit_internal_log(
                                    &app_clone,
                                    &dl_id,
                                    "GAMDL tried all formats in priority chain — none available for this content",
                                );
                                    // Fall through to terminal error below
                                } else {
                                    // GAMDL < 2.9.1 — use MeedyaDL's own fallback system
                                    log::info!(
                                        "Download {dl_id} exited 0 with codec error, \
                                     attempting fallback: {error_msg}"
                                    );
                                    let settings = load_settings_for_queue(&app_clone);
                                    q.set_error(&dl_id, &error_msg);
                                    q.on_task_finished();

                                    if let Some(new_options) = q.try_fallback(&dl_id, &settings) {
                                        let fallback_codec = new_options
                                            .song_codec
                                            .as_ref()
                                            .map(|c| c.to_cli_string().to_string())
                                            .unwrap_or_else(|| "unknown".to_string());
                                        log::info!(
                                        "Download {dl_id} will retry with fallback codec: {fallback_codec}"
                                    );
                                        drop(q);
                                        emit_internal_log(
                                        &app_clone,
                                        &dl_id,
                                        &format!("Format not available — retrying with: {fallback_codec}"),
                                    );
                                        save_queue_to_disk(&app_clone, &queue_clone).await;
                                        process_queue(app_clone.clone(), queue_clone.clone()).await;
                                        return;
                                    }
                                    // Fallback chain exhausted — fall through to error below
                                    emit_internal_log(
                                        &app_clone,
                                        &dl_id,
                                        "All audio formats exhausted — no compatible format found",
                                    );
                                }
                            }

                            // Partial-success recovery: GAMDL exited 0 with codec
                            // skip warnings, meaning some tracks were unavailable in
                            // the requested format but others downloaded successfully.
                            // Find the actual album directory (not the base output dir)
                            // containing audio files. This handles GAMDL 2.9.1+ which
                            // doesn't emit "Saved to:" lines for album downloads.
                            if has_codec_error && !has_io_error {
                                if let Some(item) =
                                    q.items.iter_mut().find(|i| i.status.id == dl_id)
                                {
                                    if let Some(ref base_dir) =
                                        item.merged_options.output_path.clone()
                                    {
                                        let base_path = std::path::Path::new(base_dir);
                                        // Find the actual album directory within the base output dir
                                        if let Some(album_dir) = find_album_directory(base_path) {
                                            item.status.output_path = Some(album_dir);
                                            item.status.output_is_directory = true;
                                            log::info!(
                                                "Download {dl_id} partial success: found album \
                                             directory with files despite codec skip warnings"
                                            );
                                        }
                                    }
                                }
                            }

                            if has_io_error && !has_codec_error {
                                // Filesystem I/O error recovery: GAMDL exited 0 with IO
                                // errors (e.g., cloud storage timeout writing Cover.jpg)
                                // but no codec errors. Audio files were likely downloaded
                                // to the output directory despite the IO errors on
                                // ancillary operations (cover art, lyrics sidecar).
                                //
                                // Find the actual album directory so "Open Folder" works.
                                if let Some(item) =
                                    q.items.iter_mut().find(|i| i.status.id == dl_id)
                                {
                                    if let Some(ref base_dir) =
                                        item.merged_options.output_path.clone()
                                    {
                                        let base_path = std::path::Path::new(base_dir);
                                        if let Some(album_dir) = find_album_directory(base_path) {
                                            item.status.output_path = Some(album_dir);
                                            item.status.output_is_directory = true;
                                        } else {
                                            // Fallback: use base dir if no album subdir found
                                            item.status.output_path = Some(base_dir.clone());
                                            item.status.output_is_directory = true;
                                        }
                                    }
                                }
                                log::warn!(
                                    "Download {dl_id} completed with IO errors — recovering with \
                                 configured output path"
                                );
                                // Fall through to normal completion with IO warnings
                            }

                            // Re-check output: partial-success or IO recovery may
                            // have set output_path above.
                            let has_output_now = q
                                .items
                                .iter()
                                .find(|i| i.status.id == dl_id)
                                .and_then(|i| i.status.output_path.as_ref())
                                .is_some();

                            if !has_output_now {
                                // Terminal: no output, no IO recovery, no files on
                                // disk despite codec fallback exhausted.
                                let error_msg = if has_io_error {
                                    format!(
                                        "Output path may be unreachable or too slow. \
                                     Check your storage connection and try a local \
                                     output path. Details: {}",
                                        warnings.last().cloned().unwrap_or_default()
                                    )
                                } else if let Some(last_warning) = warnings.last() {
                                    format!(
                                        "Download completed but no output files were \
                                     produced: {last_warning}"
                                    )
                                } else {
                                    "Download completed but no output files were produced. \
                                 Check the Activity Log for details."
                                        .to_string()
                                };
                                log::warn!(
                                    "Download {dl_id} exited 0 but produced no output: {error_msg}"
                                );
                                q.set_error(&dl_id, &error_msg);
                                q.on_task_finished();
                                drop(q);
                                emit_internal_log(
                                    &app_clone,
                                    &dl_id,
                                    &format!("Download failed: {error_msg}"),
                                );

                                // Auto-retry without wrapper on success-path
                                // terminal errors (same logic as the Err path).
                                if wrapper_url_for_logging.is_some() {
                                    let ar_settings = load_settings_for_queue(&app_clone);
                                    if ar_settings.auto_retry_without_wrapper {
                                        log::info!(
                                            "Auto-retrying download {dl_id} without \
                                         wrapper (success-path terminal error)"
                                        );
                                        emit_internal_log(
                                            &app_clone,
                                            &dl_id,
                                            "Wrapper failed — auto-retrying without wrapper",
                                        );
                                        let retried = {
                                            let mut q = queue_clone.lock().await;
                                            q.retry_without_wrapper(&dl_id, &ar_settings)
                                        };
                                        if retried {
                                            save_queue_to_disk(&app_clone, &queue_clone).await;
                                            let _ = app_clone.emit("download-queued", &dl_id);
                                            process_queue(app_clone.clone(), queue_clone.clone())
                                                .await;
                                            return;
                                        }
                                    }
                                }

                                // Save error report for user-reportable diagnostics
                                let mut err_ctx = std::collections::HashMap::new();
                                if let Some(ref url) = urls.first() {
                                    err_ctx.insert("url".to_string(), url.to_string());
                                }
                                err_ctx.insert(
                                    "error_category".to_string(),
                                    if has_io_error { "io" } else { "codec" }.to_string(),
                                );
                                let report = CrashReport {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                    app_version: env!("CARGO_PKG_VERSION").to_string(),
                                    os: std::env::consts::OS.to_string(),
                                    arch: std::env::consts::ARCH.to_string(),
                                    source: "download_error".to_string(),
                                    panic_message: Some(error_msg.clone()),
                                    location: None,
                                    backtrace: None,
                                    context: err_ctx,
                                };
                                if let Err(e) =
                                    crash_report_service::save_error_report(&app_clone, report)
                                {
                                    log::debug!("Failed to save download error report: {e}");
                                }

                                save_queue_to_disk(&app_clone, &queue_clone).await;
                                let _ = app_clone.emit(
                                    "download-error",
                                    serde_json::json!({
                                        "download_id": dl_id,
                                        "error": error_msg,
                                    }),
                                );
                                // Spawn companion downloads even on failure —
                                // the companion codec may succeed where the primary
                                // format was unavailable.
                                spawn_companion_downloads(
                                    &app_clone,
                                    &dl_id,
                                    &urls,
                                    &primary_codec_for_companions,
                                    &companion_base_options,
                                );

                                // Continue processing remaining queued items
                                process_queue(app_clone.clone(), queue_clone.clone()).await;
                                return;
                            }
                        }

                        // Mark as complete and attach any non-fatal warnings.
                        // Reached when has_output=true (normal) or IO error
                        // recovery set output_path from config.
                        q.set_complete(&dl_id);
                        let mut all_warnings = warnings.clone();
                        if warnings.iter().any(|w| process::is_io_error(w)) {
                            all_warnings.push(
                                "Some files may be incomplete due to storage I/O \
                             errors. Consider using a local output path instead \
                             of cloud storage."
                                    .to_string(),
                            );
                        }
                        if !all_warnings.is_empty() {
                            q.add_warnings(&dl_id, &all_warnings);
                        }
                        q.on_task_finished();

                        // Extract output_path and codec_used while we have the lock
                        let status = q.get_status();
                        let item = status.iter().find(|s| s.id == dl_id);
                        let result = (
                            item.and_then(|s| s.output_path.clone()),
                            item.and_then(|s| s.codec_used.clone()),
                        );
                        drop(q);
                        result
                    };

                    // Verify the output path actually exists on disk. If GAMDL
                    // reported a path but the file/folder is missing, add a warning.
                    if let Some(ref path) = output_path_for_artwork {
                        if !std::path::Path::new(path).exists() {
                            log::warn!("Download {dl_id} output path does not exist: {path}");
                            let mut q = queue_clone.lock().await;
                            q.add_warnings(
                                &dl_id,
                                &["Output path reported by GAMDL does not exist on disk"
                                    .to_string()],
                            );
                            drop(q);
                        }
                    }

                    log::info!("Download {dl_id} completed successfully");

                    // Persist queue state: completed item is now in terminal state,
                    // so it will be excluded from the persistence file (only
                    // Queued/Downloading/Processing items are persisted).
                    save_queue_to_disk(&app_clone, &queue_clone).await;

                    // Notify frontend of successful completion
                    let _ = app_clone.emit("download-complete", &dl_id);

                    // === Unified post-download enrichment (background, fire-and-forget) ===
                    // After a successful download, run all post-processing in a single
                    // background task:
                    //   1. Metadata enrichment (codec tags + source tags + channel
                    //      detection + API metadata)
                    //   2. Enhanced LRC conversion (TTML → word-by-word LRC, opt-in)
                    //   3. Animated artwork download (reuses API data from step 1)
                    //   4. AcousticID fingerprinting (opt-in, requires fpcalc)
                    //   5. ReplayGain loudness analysis (opt-in, uses FFmpeg)
                    //
                    // The enrichment fetches Apple Music API data once and shares it
                    // with animated artwork, avoiding duplicate API calls.
                    //
                    // This runs in a separate tokio task so it doesn't block the queue
                    // from processing the next download. Failures are logged but never
                    // propagate to the user or affect the download status.
                    if let Some(output_dir) = output_path_for_artwork {
                        let enrich_app = app_clone.clone();
                        let enrich_urls = urls.clone();
                        let enrich_dl_id = dl_id.clone();
                        let enrich_codec_str = completed_codec.clone();
                        tokio::spawn(async move {
                            // Determine the album directory from the output path.
                            // For single tracks, output_path is a file -- use its parent.
                            // For albums, output_path is already the directory.
                            let dir = std::path::Path::new(&output_dir);
                            let album_dir = if dir.is_dir() {
                                output_dir.clone()
                            } else {
                                dir.parent().map_or_else(
                                    || output_dir.clone(),
                                    |p| p.to_string_lossy().to_string(),
                                )
                            };

                            // Log enrichment settings summary so users can see
                            // which steps will run in the Activity Log.
                            let enrich_settings = load_settings_for_queue(&enrich_app);
                            emit_internal_log(
                            &enrich_app,
                            &enrich_dl_id,
                            &format!(
                                "Post-download enrichment starting (enhanced_lrc: {}, artwork: {}, acoustid: {}, replaygain: {})",
                                if enrich_settings.enhanced_lrc { "on" } else { "off" },
                                if enrich_settings.animated_artwork_enabled { "on" } else { "off" },
                                if enrich_settings.acoustid_enabled { "on" } else { "off" },
                                if enrich_settings.replaygain_enabled { "on" } else { "off" },
                            ),
                        );

                            // --- Step 1: Enriched metadata tagging ---
                            // Parse the codec string and run full enrichment (codec tags,
                            // source tags, channel detection, API metadata). Returns the
                            // fetched AlbumMetadata for reuse by animated artwork.
                            let codec = enrich_codec_str.as_deref().and_then(|s| match s {
                                "alac" => Some(SongCodec::Alac),
                                "atmos" => Some(SongCodec::Atmos),
                                "aac" => Some(SongCodec::Aac),
                                "aac-legacy" => Some(SongCodec::AacLegacy),
                                "aac-he" => Some(SongCodec::AacHe),
                                "aac-binaural" => Some(SongCodec::AacBinaural),
                                "aac-downmix" => Some(SongCodec::AacDownmix),
                                "ac3" => Some(SongCodec::Ac3),
                                _ => None,
                            });

                            emit_internal_log(
                            &enrich_app,
                            &enrich_dl_id,
                            "Enriching metadata (codec tags, source tags, channel detection, Apple Music API)...",
                        );
                            let album_metadata = if let Some(ref codec) = codec {
                                match super::metadata_tag_service::apply_enriched_metadata_tags(
                                    &enrich_app,
                                    &album_dir,
                                    codec,
                                    &enrich_urls,
                                    None, // No pre-fetched metadata; will fetch from API if possible
                                    Some((&enrich_app, &enrich_dl_id)),
                                )
                                .await
                                {
                                    Ok((count, metadata)) => {
                                        if count > 0 {
                                            let api_note = if metadata.is_some() {
                                                " (including Apple Music API metadata: ISRC, UPC, genre)"
                                            } else {
                                                " (Apple Music API metadata unavailable)"
                                            };
                                            log::info!(
                                            "Enriched {count} file(s) with metadata for {enrich_dl_id}"
                                        );
                                            emit_internal_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Enriched {count} file(s) with metadata tags{api_note}"),
                                        );
                                        }
                                        metadata
                                    }
                                    Err(e) => {
                                        log::debug!(
                                            "Metadata enrichment failed for {enrich_dl_id}: {e}"
                                        );
                                        emit_internal_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Metadata enrichment failed: {e}"),
                                        );
                                        None
                                    }
                                }
                            } else {
                                None
                            };

                            // --- Step 2: Enhanced LRC conversion (opt-in, default on) ---
                            // When enabled, converts TTML sidecar files to Enhanced LRC
                            // with word-by-word timestamps. Saves a `.lrc` sidecar file
                            // and embeds the Enhanced LRC in M4A/M4V metadata.
                            // Falls back to standard line-level LRC for songs without
                            // word-level timing in their TTML.
                            if enrich_settings.enhanced_lrc {
                                emit_internal_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Converting TTML lyrics to Enhanced LRC (word-by-word sync)...",
                                );
                                match super::enhanced_lyrics_service::process_enhanced_lyrics_for_directory(
                                &album_dir,
                            ).await {
                                Ok(count) if count > 0 => {
                                    log::info!(
                                        "Enhanced LRC generated for {count} file(s) for {enrich_dl_id}"
                                    );
                                    emit_internal_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        &format!("Enhanced LRC generated for {count} file(s)"),
                                    );
                                }
                                Ok(_) => {
                                    emit_internal_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        "No TTML lyrics files found for Enhanced LRC conversion",
                                    );
                                }
                                Err(e) => {
                                    log::debug!(
                                        "Enhanced LRC conversion skipped for {enrich_dl_id}: {e}"
                                    );
                                    emit_internal_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        &format!("Enhanced LRC conversion skipped: {e}"),
                                    );
                                }
                            }
                            }

                            // --- Step 3: Animated artwork download ---
                            // Reuse the AlbumMetadata from enrichment to avoid a
                            // duplicate API call. Falls back to a fresh API call
                            // if enrichment didn't produce metadata.
                            if enrich_settings.animated_artwork_enabled {
                                emit_internal_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Downloading animated artwork...",
                                );
                                let artwork_result = if let Some(ref metadata) = album_metadata {
                                    super::animated_artwork_service::process_album_artwork_from_metadata(
                                    &enrich_app,
                                    metadata,
                                    &album_dir,
                                ).await
                                } else {
                                    super::animated_artwork_service::process_album_artwork(
                                        &enrich_app,
                                        &enrich_urls,
                                        &album_dir,
                                    )
                                    .await
                                };

                                match artwork_result {
                                    Ok(result) => {
                                        if result.square_downloaded || result.portrait_downloaded {
                                            log::info!(
                                                "Animated artwork downloaded for {enrich_dl_id}"
                                            );
                                            emit_internal_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!(
                                                "Animated artwork downloaded (square: {}, portrait: {})",
                                                if result.square_downloaded { "yes" } else { "no" },
                                                if result.portrait_downloaded { "yes" } else { "no" },
                                            ),
                                        );

                                            // Hide artwork files if enabled in settings
                                            if enrich_settings.hide_animated_artwork {
                                                let dir = std::path::Path::new(&album_dir);
                                                if result.square_downloaded {
                                                    if let Err(e) =
                                                        super::animated_artwork_service::hide_file(
                                                            &dir.join("FrontCover.mp4"),
                                                        )
                                                        .await
                                                    {
                                                        log::debug!(
                                                            "Failed to hide FrontCover.mp4: {e}"
                                                        );
                                                    }
                                                }
                                                if result.portrait_downloaded {
                                                    if let Err(e) =
                                                        super::animated_artwork_service::hide_file(
                                                            &dir.join("PortraitCover.mp4"),
                                                        )
                                                        .await
                                                    {
                                                        log::debug!(
                                                            "Failed to hide PortraitCover.mp4: {e}"
                                                        );
                                                    }
                                                }
                                            }

                                            let _ = enrich_app
                                                .emit("artwork-downloaded", &enrich_dl_id);
                                        } else {
                                            emit_internal_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                "No animated artwork available for this album",
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        log::debug!(
                                            "Animated artwork skipped for {enrich_dl_id}: {e}"
                                        );
                                        emit_internal_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("Animated artwork skipped: {e}"),
                                        );
                                    }
                                }
                            }

                            // --- Step 4: AcousticID fingerprinting (opt-in) ---
                            // When enabled, generates Chromaprint fingerprints using the
                            // embedded rusty-chromaprint library and looks up AcousticID
                            // identifiers from acoustid.org.
                            if enrich_settings.acoustid_enabled {
                                if enrich_settings.acoustid_api_key.is_empty() {
                                    emit_internal_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "AcousticID skipped: no API key configured (register at https://acoustid.org/new-application)",
                                );
                                } else {
                                    emit_internal_log(
                                        &enrich_app,
                                        &enrich_dl_id,
                                        "Running AcousticID fingerprinting...",
                                    );
                                    match super::acoustid_service::process_acoustid_for_directory(
                                        &album_dir,
                                        &enrich_settings.acoustid_api_key,
                                    )
                                    .await
                                    {
                                        Ok(count) if count > 0 => {
                                            log::info!(
                                            "AcousticID tagged {count} file(s) for {enrich_dl_id}"
                                        );
                                            emit_internal_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                &format!("AcousticID tagged {count} file(s)"),
                                            );
                                        }
                                        Ok(_) => {
                                            emit_internal_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                "AcousticID: no matches found for any files",
                                            );
                                        }
                                        Err(e) => {
                                            log::debug!(
                                                "AcousticID skipped for {enrich_dl_id}: {e}"
                                            );
                                            emit_internal_log(
                                                &enrich_app,
                                                &enrich_dl_id,
                                                &format!("AcousticID failed: {e}"),
                                            );
                                        }
                                    }
                                }
                            }

                            // --- Step 5: ReplayGain loudness analysis (opt-in) ---
                            // When enabled, analyses each file's loudness via FFmpeg's
                            // ebur128 filter and writes non-destructive ReplayGain tags.
                            if enrich_settings.replaygain_enabled {
                                emit_internal_log(
                                    &enrich_app,
                                    &enrich_dl_id,
                                    "Analysing loudness (ReplayGain EBU R128)...",
                                );
                                match super::replaygain_service::process_replaygain_for_directory(
                                    &enrich_app,
                                    &album_dir,
                                )
                                .await
                                {
                                    Ok(count) if count > 0 => {
                                        log::info!(
                                        "ReplayGain analysed {count} file(s) for {enrich_dl_id}"
                                    );
                                        emit_internal_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("ReplayGain analysed {count} file(s)"),
                                        );
                                    }
                                    Ok(_) => {
                                        emit_internal_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            "ReplayGain: no audio files to analyse",
                                        );
                                    }
                                    Err(e) => {
                                        log::debug!("ReplayGain skipped for {enrich_dl_id}: {e}");
                                        emit_internal_log(
                                            &enrich_app,
                                            &enrich_dl_id,
                                            &format!("ReplayGain failed: {e}"),
                                        );
                                    }
                                }
                            }

                            emit_internal_log(
                                &enrich_app,
                                &enrich_dl_id,
                                "Post-download enrichment complete",
                            );
                        });
                    }

                    // Spawn companion downloads (codec + lyrics) as background tasks.
                    // Uses the primary codec from pre-match extraction since
                    // completed_codec may differ (e.g., after fallback).
                    spawn_companion_downloads(
                        &app_clone,
                        &dl_id,
                        &urls,
                        &primary_codec_for_companions,
                        &companion_base_options,
                    );
                }
                Err(error_msg) => {
                    // === Error path ===
                    // Classify the error to determine the appropriate retry strategy.
                    // process::classify_error() returns "codec", "network", or "unknown".
                    let error_category = process::classify_error(&error_msg);
                    log::error!("Download {dl_id} failed ({error_category}): {error_msg}");

                    // Add wrapper-specific context for network errors to aid troubleshooting
                    if error_category == "network" {
                        if let Some(ref url) = wrapper_url_for_logging {
                            log::error!(
                                "Wrapper URL was: {url} -- check that the wrapper \
                             service is running and reachable"
                            );
                        }
                    }

                    // Determine if we should retry or fallback based on error category
                    let should_retry =
                        match error_category {
                            "codec" => {
                                // Codec/format error: the requested audio codec isn't
                                // available for this track. Try the next codec in the
                                // fallback chain (e.g., atmos -> alac -> aac).
                                //
                                // Even when GAMDL >= 2.9.1 native priority was used
                                // (--song-codec-priority), we still run MeedyaDL's own
                                // fallback as a safety net. GAMDL's native priority
                                // loop may fail without exhausting all codecs (e.g.,
                                // AttributeError on None stream_info, DRM issues, or
                                // partial chain traversal). Per-codec retries via
                                // --song-codec are more reliable as a fallback path.
                                if uses_native_priority {
                                    log::warn!(
                                        "Download {dl_id} codec error despite native \
                                 priority — trying per-codec fallback as \
                                 safety net"
                                    );
                                    emit_internal_log(
                                &app_clone,
                                &dl_id,
                                "GAMDL native priority failed — trying each format individually",
                            );
                                }

                                let settings = load_settings_for_queue(&app_clone);
                                let mut q = queue_clone.lock().await;
                                q.set_error(&dl_id, &error_msg);
                                q.on_task_finished();

                                if let Some(new_options) = q.try_fallback(&dl_id, &settings) {
                                    let fallback_codec = new_options
                                        .song_codec
                                        .as_ref()
                                        .map(|c| c.to_cli_string().to_string())
                                        .unwrap_or_else(|| "unknown".to_string());
                                    log::info!(
                                        "Download {dl_id} will retry with \
                                 fallback codec: {fallback_codec}"
                                    );
                                    drop(q);
                                    emit_internal_log(
                                &app_clone,
                                &dl_id,
                                &format!("Format not available — retrying with: {fallback_codec}"),
                            );
                                    true
                                } else {
                                    drop(q);
                                    emit_internal_log(
                                        &app_clone,
                                        &dl_id,
                                        "All audio formats exhausted — download failed",
                                    );
                                    false
                                }
                            }
                            "network" => {
                                // Network error: transient connection issue.
                                // Retry with the same options (up to max_network_retries times).
                                let mut q = queue_clone.lock().await;
                                q.set_error(&dl_id, &error_msg);
                                q.on_task_finished();

                                if q.try_network_retry(&dl_id) {
                                    // try_network_retry resets the item to Queued with same options
                                    log::info!("Download {dl_id} will retry (network error)");
                                    drop(q);
                                    emit_internal_log(
                                        &app_clone,
                                        &dl_id,
                                        "Network error — retrying download",
                                    );
                                    true
                                } else {
                                    drop(q);
                                    emit_internal_log(
                                        &app_clone,
                                        &dl_id,
                                        "Network error — all retries exhausted",
                                    );
                                    false
                                }
                            }
                            _ => {
                                // Non-retriable error (e.g., authentication, invalid URL).
                                // Mark as failed and don't retry.
                                let mut q = queue_clone.lock().await;
                                q.set_error(&dl_id, &error_msg);
                                q.on_task_finished();
                                drop(q);
                                emit_internal_log(
                                    &app_clone,
                                    &dl_id,
                                    &format!("Download failed ({error_category}): {error_msg}"),
                                );
                                false
                            }
                        };

                    // Persist queue state after error handling (whether retrying or terminal)
                    save_queue_to_disk(&app_clone, &queue_clone).await;

                    // If no retry will occur, check auto-retry-without-wrapper
                    // before falling through to the terminal error path.
                    if !should_retry {
                        // Auto-retry without wrapper: if the download used wrapper
                        // auth and the user has opted in, automatically re-queue
                        // with wrapper disabled (cookie-based auth) instead of
                        // treating this as a terminal failure.
                        log::debug!(
                            "Download {dl_id} terminal error path: \
                         wrapper_url={}, error_category={error_category}",
                            wrapper_url_for_logging.as_deref().unwrap_or("none"),
                        );
                        if wrapper_url_for_logging.is_some() {
                            let ar_settings = load_settings_for_queue(&app_clone);
                            log::debug!(
                                "Download {dl_id} auto_retry_without_wrapper={}",
                                ar_settings.auto_retry_without_wrapper,
                            );
                            if ar_settings.auto_retry_without_wrapper {
                                log::info!(
                                    "Auto-retrying download {dl_id} without wrapper \
                                 (auto_retry_without_wrapper enabled)"
                                );
                                emit_internal_log(
                                    &app_clone,
                                    &dl_id,
                                    "Wrapper failed — auto-retrying without wrapper",
                                );
                                let retried = {
                                    let mut q = queue_clone.lock().await;
                                    let item_state =
                                        q.items.iter().find(|i| i.status.id == dl_id).map(|i| {
                                            (i.status.state.clone(), i.status.used_wrapper)
                                        });
                                    log::debug!(
                                        "Download {dl_id} before retry_without_wrapper: \
                                     state={:?}",
                                        item_state,
                                    );
                                    q.retry_without_wrapper(&dl_id, &ar_settings)
                                };
                                log::debug!(
                                    "Download {dl_id} retry_without_wrapper returned {retried}"
                                );
                                if retried {
                                    save_queue_to_disk(&app_clone, &queue_clone).await;
                                    let _ = app_clone.emit("download-queued", &dl_id);
                                    // Skip the terminal error path entirely.
                                    process_queue(app_clone, queue_clone).await;
                                    return;
                                }
                                // If retry_without_wrapper returned false (item not
                                // found or not eligible), fall through to normal
                                // terminal error handling below.
                            }
                        }

                        // Save a download error report so the user can optionally
                        // report it to GitHub Issues via Settings > Advanced.
                        // Skip for network errors — these are connectivity issues,
                        // not application bugs, and would just add noise.
                        if error_category != "network" {
                            let mut context = std::collections::HashMap::new();
                            context
                                .insert("error_category".to_string(), error_category.to_string());
                            if let Some(ref url) = urls.first() {
                                context.insert("url".to_string(), url.to_string());
                            }
                            if let Some(ref ver) = gamdl_version {
                                context.insert("gamdl_version".to_string(), ver.to_string());
                            }
                            context.insert(
                                "native_priority".to_string(),
                                uses_native_priority.to_string(),
                            );
                            let report = CrashReport {
                                id: uuid::Uuid::new_v4().to_string(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                app_version: env!("CARGO_PKG_VERSION").to_string(),
                                os: std::env::consts::OS.to_string(),
                                arch: std::env::consts::ARCH.to_string(),
                                source: "download_error".to_string(),
                                panic_message: Some(error_msg.clone()),
                                location: None,
                                backtrace: None,
                                context,
                            };
                            if let Err(e) =
                                crash_report_service::save_error_report(&app_clone, report)
                            {
                                log::debug!("Failed to save download error report: {e}");
                            }
                        } // end: skip error reports for network errors

                        let _ = app_clone.emit(
                            "download-error",
                            serde_json::json!({
                                "download_id": dl_id,
                                "error": error_msg,
                                "category": error_category,
                            }),
                        );

                        // Spawn companion downloads even on failure — the companion
                        // codec may succeed where the primary format was unavailable.
                        spawn_companion_downloads(
                            &app_clone,
                            &dl_id,
                            &urls,
                            &primary_codec_for_companions,
                            &companion_base_options,
                        );
                    }
                }
            }

            // Cascade: process the next item in the queue.
            // This recursive call ensures continuous queue processing — when one
            // download finishes, the next one starts automatically.
            process_queue(app_clone, queue_clone).await;
        });
    }) // close Box::pin(async move {
}

/// Extracts the actual Python exception message from raw stderr lines.
///
/// Python tracebacks have this structure:
/// ```text
/// Traceback (most recent call last):
///   File "foo.py", line 42, in bar
///     some_call()
/// TypeError: 'NoneType' object has no attribute 'x'
/// ```
///
/// The actual exception is the LAST non-empty, non-indented line after
/// the "Traceback" header. This function finds the last traceback block
/// in the stderr output and extracts that exception line.
///
/// Returns `None` if no traceback is found in the stderr lines.
fn extract_python_exception(stderr_lines: &[String]) -> Option<String> {
    // Find the last occurrence of "Traceback" in stderr
    let traceback_idx = stderr_lines
        .iter()
        .rposition(|line| line.trim().to_lowercase().contains("traceback"))?;

    // Walk forward from the traceback line to find the exception.
    // The exception is the last non-empty, non-indented line in the traceback block.
    let mut exception_line: Option<&str> = None;
    for line in &stderr_lines[traceback_idx + 1..] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Empty line after we've found an exception means end of traceback
            if exception_line.is_some() {
                break;
            }
            continue;
        }
        // Exception lines are NOT indented (don't start with space/tab).
        // Indented lines are stack frame details (File "...", code).
        if !line.starts_with(' ') && !line.starts_with('\t') {
            exception_line = Some(trimmed);
        }
    }

    exception_line.map(std::string::ToString::to_string)
}

/// Runs a GAMDL download while forwarding parsed events to both
/// the queue item (for status tracking) and the frontend (for UI updates).
///
/// This is the queue's version of `gamdl_service::run_gamdl()`, with two
/// key differences:
/// 1. It updates the queue item's progress (for status queries)
/// 2. It polls for cancellation every 250ms (for user cancel support)
///
/// The function builds the GAMDL command, spawns it with piped stdio,
/// starts two reader tasks (stdout + stderr), and enters a poll loop
/// that alternates between checking for process exit and cancellation.
///
/// Error messages from GAMDL's output are collected in a Vec<String>
/// (behind Arc<Mutex>) so the last error can be used as the failure
/// message if the process exits with a non-zero code. On success, returns
/// any non-fatal warning messages collected during the run (error-pattern
/// lines from GAMDL output that didn't cause a non-zero exit).
// Monolithic by necessity: manages subprocess lifecycle (spawn, stdout/stderr
// readers, cancellation polling loop, exit status handling) with shared
// Arc<Mutex> state that makes extraction impractical.
#[allow(clippy::too_many_lines)]
async fn run_download_with_events(
    app: &AppHandle,
    download_id: &str,
    urls: &[String],
    options: &GamdlOptions,
    queue: &QueueHandle,
) -> Result<Vec<String>, String> {
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
        .map_err(|e| format!("Failed to start GAMDL process: {e}"))?;

    // Take stdout/stderr handles
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture GAMDL stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture GAMDL stderr".to_string())?;

    // Collect error messages from GAMDL's output for post-process error reporting.
    // These are shared between the stdout and stderr reader tasks via Arc<Mutex>.
    // After the process exits, the last collected error is used as the failure message,
    // which is more informative than just the exit code.
    let collected_errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Collect ALL stderr lines (raw) for Python traceback extraction.
    // When GAMDL crashes with a Python traceback, the parsed error list may
    // only contain "Traceback (most recent call last):" because intermediate
    // lines and the final exception line may not match any error keyword.
    // The raw stderr buffer lets us extract the actual exception post-mortem.
    let raw_stderr_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Deduplication set for Activity Log emissions.
    // GAMDL and its Python dependencies (yt-dlp, tqdm) may write the same
    // output to both stdout and stderr, causing each line to appear twice
    // in the Activity Log. This shared set tracks lines already emitted
    // so the second reader skips duplicates. The set is bounded by the
    // total GAMDL output size (typically a few hundred to a few thousand
    // lines per album) so memory usage is negligible.
    let seen_lines: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    // Spawn stdout reader
    let stdout_task = {
        let download_id = download_id.to_string();
        let app = app.clone();
        let queue = queue.clone();
        let errors = collected_errors.clone();
        let seen = seen_lines.clone();
        tokio::spawn(async move {
            let reader = tokio::io::BufReader::new(stdout);
            let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
            while let Ok(Some(raw_line)) = lines.next_line().await {
                // Split on \r to handle yt-dlp download progress updates.
                // yt-dlp uses \r (carriage return) for in-place terminal
                // updates, but AsyncBufReadExt::lines() only splits on \n.
                // Without this split, all \r-separated progress updates
                // concatenate into one massive line (~127KB for albums).
                let segments: Vec<&str> = raw_line.split('\r').collect();
                for segment in &segments {
                    let segment = segment.trim();
                    if segment.is_empty() {
                        continue;
                    }

                    // Strip ANSI escape codes (e.g., \x1b[32m) that GAMDL
                    // outputs for terminal colouring but render as garbage
                    // in the Activity Log's HTML view.
                    let clean_line = process::strip_ansi_codes(segment);
                    log::debug!("[gamdl stdout] {clean_line}");

                    // Deduplicate: only emit to Activity Log if this line
                    // hasn't already been emitted by the other reader.
                    let is_new = {
                        let mut set = seen.lock().await;
                        set.insert(clean_line.clone())
                    };
                    if is_new {
                        let _ = app.emit(
                            "activity-log",
                            &ActivityLogEvent {
                                download_id: download_id.clone(),
                                stream: "stdout",
                                line: clean_line.clone(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            },
                        );
                    }

                    let event = process::parse_gamdl_output(&clean_line);

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

                    // Emit parsed event to frontend
                    let progress = gamdl_service::GamdlProgress {
                        download_id: download_id.clone(),
                        event,
                    };
                    let _ = app.emit("gamdl-output", &progress);
                }
            }
        })
    };

    // Spawn stderr reader
    let stderr_task = {
        let download_id = download_id.to_string();
        let app = app.clone();
        let queue = queue.clone();
        let errors = collected_errors.clone();
        let raw_stderr = raw_stderr_lines.clone();
        let seen = seen_lines.clone();
        tokio::spawn(async move {
            let reader = tokio::io::BufReader::new(stderr);
            let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
            while let Ok(Some(raw_line)) = lines.next_line().await {
                // Split on \r for yt-dlp progress updates (same as stdout)
                let segments: Vec<&str> = raw_line.split('\r').collect();
                for segment in &segments {
                    let segment = segment.trim();
                    if segment.is_empty() {
                        continue;
                    }

                    // Strip ANSI escape codes before display and parsing
                    let clean_line = process::strip_ansi_codes(segment);
                    log::debug!("[gamdl stderr] {clean_line}");

                    // Deduplicate: only emit to Activity Log if this line
                    // hasn't already been emitted by the stdout reader.
                    let is_new = {
                        let mut set = seen.lock().await;
                        set.insert(clean_line.clone())
                    };
                    if is_new {
                        let _ = app.emit(
                            "activity-log",
                            &ActivityLogEvent {
                                download_id: download_id.clone(),
                                stream: "stderr",
                                line: clean_line.clone(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            },
                        );
                    }

                    let event = process::parse_gamdl_output(&clean_line);

                    // Collect raw stderr lines for Python traceback extraction
                    // (always, regardless of dedup — needed for exception analysis)
                    {
                        let mut raw = raw_stderr.lock().await;
                        raw.push(clean_line.clone());
                    }

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
            }
        })
    };

    // Cancellation polling loop: alternate between checking for user cancellation
    // and checking if the GAMDL process has exited naturally.
    // This loop runs every 250ms and provides responsive cancellation support
    // without consuming excessive CPU.
    let status = loop {
        // Step 1: Check if the user cancelled this download.
        // The cancel() method on the queue sets the item's state to Cancelled,
        // which we detect here. The lock is held very briefly (just a read check).
        {
            let q = queue.lock().await;
            if q.is_cancelled(download_id) {
                log::info!("Download {download_id} cancelled, killing process");
                // Kill the GAMDL process and wait for cleanup
                let _ = child.kill().await;
                let _ = child.wait().await;
                // Wait for reader tasks to finish draining any buffered output
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                return Err("Download cancelled by user".to_string());
            }
        }

        // Step 2: Check if the process has exited (non-blocking check).
        // try_wait() returns Ok(Some(status)) if the process has exited,
        // Ok(None) if it's still running, or Err on OS-level error.
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                // Process still running — sleep briefly before next poll iteration.
                // 250ms provides a good balance between responsiveness and CPU usage.
                tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
            }
            Err(e) => return Err(format!("Failed to wait for GAMDL process: {e}")),
        }
    };

    // Wait for output reader tasks to finish
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    // Check the exit status and construct an appropriate error message.
    if status.success() {
        // Return any error-pattern lines as non-fatal warnings.
        // These are issues GAMDL logged during the run but didn't consider
        // fatal enough to exit with a non-zero code.
        let warnings = collected_errors.lock().await.clone();
        Ok(warnings)
    } else {
        // Use the last collected error message from GAMDL's output for a meaningful
        // error message. This is more informative than just "exited with code N".
        // The error message is also used by classify_error() to determine the
        // retry/fallback strategy (codec error vs network error vs unknown).
        let errors = collected_errors.lock().await;
        let raw_lines = raw_stderr_lines.lock().await;

        errors.last().map_or_else(
            || {
                // Fallback to exit code if no error messages were collected
                // (e.g., GAMDL crashed without printing an error)
                let code = status.code().unwrap_or(-1);
                Err(format!("GAMDL process exited with code {code}"))
            },
            |last_error| {
                // If the last collected error is just the "Traceback" header,
                // try to extract the actual Python exception from raw stderr.
                // The parser processes lines independently, so the intermediate
                // traceback frames and the final exception line may not have been
                // captured as Error events.
                let lower = last_error.to_lowercase();
                if lower.contains("traceback") {
                    extract_python_exception(&raw_lines)
                        .map_or_else(|| Err(last_error.clone()), Err)
                } else {
                    Err(last_error.clone())
                }
            },
        )
    }
}

/// Loads the current app settings for use during queue processing decisions.
///
/// This is called during the error handling path of `process_queue()` to
/// access the fallback chain configuration. It uses `config_service::load_settings()`
/// rather than cached settings to ensure the latest user preferences are used
/// (the user might change settings while downloads are running).
///
/// Returns `AppSettings::default()` on load failure to avoid blocking queue processing.
fn load_settings_for_queue(app: &AppHandle) -> AppSettings {
    match config_service::load_settings(app) {
        Ok(settings) => settings,
        Err(e) => {
            log::warn!("Failed to load settings for fallback: {e}, using defaults");
            AppSettings::default()
        }
    }
}

// ============================================================
// Queue persistence: save/load/clear (crash recovery)
// ============================================================

/// Saves the current queue state to disk for crash recovery.
///
/// Writes only non-terminal items (Queued/Downloading/Processing) to
/// `{app_data_dir}/queue.json` as a JSON array of `PersistedQueueItem`.
///
/// Uses a clone-then-release pattern: persistable items are cloned while
/// the Mutex lock is held, then the lock is released before performing
/// file I/O. This avoids holding the lock during potentially slow disk writes.
///
/// Called after every queue mutation (enqueue, cancel, retry, clear, fallback,
/// network retry, completion, error) to ensure the on-disk state is always
/// up-to-date for crash recovery.
pub async fn save_queue_to_disk(app: &AppHandle, queue: &QueueHandle) {
    // Clone persistable items while holding the lock (very fast — just cloning URLs + IDs)
    let items = {
        let q = queue.lock().await;
        q.get_persistable_items()
    };

    // Write to disk after releasing the lock
    let queue_path = crate::utils::platform::get_app_data_dir(app).join("queue.json");
    match serde_json::to_string_pretty(&items) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&queue_path, json) {
                log::debug!("Failed to save queue to disk: {e}");
            }
        }
        Err(e) => {
            log::debug!("Failed to serialize queue: {e}");
        }
    }
}

/// Loads persisted queue items from disk.
///
/// Returns an empty `Vec` on missing or invalid file (graceful degradation
/// for first run or file corruption). This is intentional: the queue should
/// start empty rather than crash if persistence data is unavailable.
#[must_use]
pub fn load_queue_from_disk(app: &AppHandle) -> Vec<PersistedQueueItem> {
    let queue_path = crate::utils::platform::get_app_data_dir(app).join("queue.json");
    std::fs::read_to_string(&queue_path).map_or_else(
        |_| vec![], // File doesn't exist (first run) — not an error
        |json| match serde_json::from_str::<Vec<PersistedQueueItem>>(&json) {
            Ok(items) => {
                if !items.is_empty() {
                    log::info!("Loaded {} persisted queue item(s) from disk", items.len());
                }
                items
            }
            Err(e) => {
                log::debug!("Failed to parse queue.json: {e}");
                vec![]
            }
        },
    )
}

/// Deletes the `queue.json` persistence file.
///
/// Called when the queue is intentionally cleared to avoid restoring
/// stale items on next startup.
pub fn clear_queue_file(app: &AppHandle) {
    let queue_path = crate::utils::platform::get_app_data_dir(app).join("queue.json");
    let _ = std::fs::remove_file(queue_path);
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::download::{DownloadRequest, DownloadState};
    use crate::models::gamdl_options::{GamdlOptions, SongCodec};
    use crate::models::settings::AppSettings;
    use crate::utils::process::GamdlOutputEvent;

    // ----------------------------------------------------------
    // Test Helpers
    // ----------------------------------------------------------

    /// Creates default AppSettings suitable for test use.
    /// The returned settings have sensible defaults matching AppSettings::default(),
    /// which includes fallback_enabled=true and a full music_fallback_chain.
    fn test_settings() -> AppSettings {
        AppSettings::default()
    }

    /// Creates a minimal DownloadRequest with a single URL and no overrides.
    /// The URL is a placeholder Apple Music URL for test purposes only.
    fn test_request() -> DownloadRequest {
        DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test-song/123456789".to_string()],
            options: None,
        }
    }

    /// Creates a DownloadRequest with per-download codec override.
    fn test_request_with_codec_override(codec: SongCodec) -> DownloadRequest {
        DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test/999".to_string()],
            options: Some(GamdlOptions {
                song_codec: Some(codec),
                ..Default::default()
            }),
        }
    }

    /// Helper: enqueues a single item and returns its download ID.
    fn enqueue_one(queue: &mut DownloadQueue) -> String {
        let settings = test_settings();
        queue.enqueue(test_request(), &settings)
    }

    /// Helper: enqueues N items and returns their download IDs.
    fn enqueue_n(queue: &mut DownloadQueue, n: usize) -> Vec<String> {
        let settings = test_settings();
        (0..n)
            .map(|_| queue.enqueue(test_request(), &settings))
            .collect()
    }

    // ==========================================================
    // 1. new() tests
    // ==========================================================

    /// Verifies that DownloadQueue::new() creates an empty queue with no items,
    /// zero active count, and default concurrency settings.
    #[test]
    fn new_creates_empty_queue() {
        let queue = DownloadQueue::new();
        assert!(queue.items.is_empty(), "New queue should have no items");
        assert_eq!(
            queue.active_count, 0,
            "New queue should have zero active count"
        );
        assert_eq!(
            queue.max_concurrent, 1,
            "Default max_concurrent should be 1"
        );
        assert_eq!(
            queue.max_network_retries, 3,
            "Default max_network_retries should be 3"
        );
    }

    /// Verifies that the Default trait implementation delegates to new().
    #[test]
    fn default_delegates_to_new() {
        let queue = DownloadQueue::default();
        assert!(queue.items.is_empty());
        assert_eq!(queue.active_count, 0);
        assert_eq!(queue.max_concurrent, 1);
    }

    // ==========================================================
    // 2. enqueue() tests
    // ==========================================================

    /// Verifies that enqueue() returns a unique download ID (UUID v4 format)
    /// and that successive calls produce different IDs.
    #[test]
    fn enqueue_returns_unique_ids() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();

        let id1 = queue.enqueue(test_request(), &settings);
        let id2 = queue.enqueue(test_request(), &settings);

        assert!(!id1.is_empty(), "Download ID should not be empty");
        assert!(!id2.is_empty(), "Download ID should not be empty");
        assert_ne!(id1, id2, "Each enqueue should produce a unique ID");
    }

    /// Verifies that an enqueued item starts in the Queued state and appears
    /// in the status list with correct initial fields.
    #[test]
    fn enqueue_sets_queued_state() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = test_request();
        let expected_url = request.urls[0].clone();

        let id = queue.enqueue(request, &settings);
        let statuses = queue.get_status();

        assert_eq!(statuses.len(), 1, "Queue should have exactly one item");
        let status = &statuses[0];
        assert_eq!(status.id, id);
        assert_eq!(status.state, DownloadState::Queued);
        assert_eq!(status.urls, vec![expected_url]);
        assert_eq!(status.progress, 0.0);
        assert!(status.current_track.is_none());
        assert!(status.error.is_none());
        assert!(status.output_path.is_none());
        assert!(!status.fallback_occurred);
    }

    /// Verifies that enqueue() merges global settings into the item's options.
    /// Since merge_options is private, we test it indirectly: the enqueued item's
    /// codec_used field should reflect the default settings codec (ALAC).
    #[test]
    fn enqueue_merges_default_settings() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();

        let _id = queue.enqueue(test_request(), &settings);
        let statuses = queue.get_status();

        assert_eq!(
            statuses[0].codec_used.as_deref(),
            Some("alac"),
            "Default codec should be ALAC from settings"
        );
    }

    /// Verifies that per-download codec overrides take precedence over
    /// global settings when enqueuing. This indirectly tests merge_options().
    #[test]
    fn enqueue_applies_per_download_overrides() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = test_request_with_codec_override(SongCodec::Aac);

        let _id = queue.enqueue(request, &settings);
        let statuses = queue.get_status();

        assert_eq!(
            statuses[0].codec_used.as_deref(),
            Some("aac"),
            "Per-download override should replace default ALAC with AAC"
        );
    }

    /// Verifies that multiple items can be enqueued and they all appear in
    /// the status list in FIFO order.
    #[test]
    fn enqueue_preserves_fifo_order() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 3);
        let statuses = queue.get_status();

        assert_eq!(statuses.len(), 3);
        assert_eq!(
            statuses[0].id, ids[0],
            "First enqueued should be first in status"
        );
        assert_eq!(statuses[1].id, ids[1]);
        assert_eq!(
            statuses[2].id, ids[2],
            "Last enqueued should be last in status"
        );
    }

    /// Verifies that enqueue sets the network_retries_left field to
    /// the queue's max_network_retries value (tested via try_network_retry).
    #[test]
    fn enqueue_sets_network_retries_from_queue_config() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        // Exhaust all 3 retries
        assert!(
            queue.try_network_retry(&id),
            "Should succeed on retry 1 of 3"
        );
        // Need to set back to Error/Queued for next retry test, but try_network_retry
        // itself resets to Queued. We need to simulate re-error.
        // Actually try_network_retry sets to Queued. Let's set back to error.
        queue.set_error(&id, "network error");
        assert!(
            queue.try_network_retry(&id),
            "Should succeed on retry 2 of 3"
        );
        queue.set_error(&id, "network error");
        assert!(
            queue.try_network_retry(&id),
            "Should succeed on retry 3 of 3"
        );
        queue.set_error(&id, "network error");
        assert!(
            !queue.try_network_retry(&id),
            "Should fail after 3 retries exhausted"
        );
    }

    // ==========================================================
    // 3. get_status() tests
    // ==========================================================

    /// Verifies that get_status() returns an empty vector when the queue
    /// has no items.
    #[test]
    fn get_status_empty_queue() {
        let queue = DownloadQueue::new();
        let statuses = queue.get_status();
        assert!(
            statuses.is_empty(),
            "Empty queue should return empty status vec"
        );
    }

    /// Verifies that get_status() returns all items in the queue, each with
    /// the correct state and URL information.
    #[test]
    fn get_status_returns_all_items() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 4);
        let statuses = queue.get_status();

        assert_eq!(statuses.len(), 4);
        for (i, status) in statuses.iter().enumerate() {
            assert_eq!(status.id, ids[i]);
            assert_eq!(status.state, DownloadState::Queued);
        }
    }

    /// Verifies that get_status() returns cloned data (modifications to the
    /// returned vec do not affect the queue).
    #[test]
    fn get_status_returns_cloned_data() {
        let mut queue = DownloadQueue::new();
        let _id = enqueue_one(&mut queue);

        let statuses1 = queue.get_status();
        let statuses2 = queue.get_status();

        assert_eq!(statuses1.len(), statuses2.len());
        assert_eq!(statuses1[0].id, statuses2[0].id);
    }

    // ==========================================================
    // 4. get_counts() tests
    // ==========================================================

    /// Verifies that get_counts() returns all zeros for an empty queue.
    /// Returns tuple: (total, active, queued, completed, failed).
    #[test]
    fn get_counts_empty_queue() {
        let queue = DownloadQueue::new();
        assert_eq!(queue.get_counts(), (0, 0, 0, 0, 0));
    }

    /// Verifies that get_counts() correctly counts items in different states.
    /// Sets up items in Queued, Downloading, Complete, and Error states
    /// and checks that each counter is accurate.
    #[test]
    fn get_counts_various_states() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 5);

        // Leave ids[0] as Queued
        // Set ids[1] to Downloading via next_pending
        let _ = queue.next_pending(); // ids[0] becomes Downloading
                                      // Set ids[2] to Complete
        queue.set_complete(&ids[2]);
        // Set ids[3] to Error
        queue.set_error(&ids[3], "test error");
        // ids[4] stays Queued

        // ids[0]=Downloading, ids[1]=Queued, ids[2]=Complete, ids[3]=Error, ids[4]=Queued
        let (total, active, queued, completed, failed) = queue.get_counts();
        assert_eq!(total, 5, "Total should be 5");
        assert_eq!(active, 1, "One item is Downloading");
        assert_eq!(queued, 2, "Two items are Queued (ids[1] and ids[4])");
        assert_eq!(completed, 1, "One item is Complete");
        assert_eq!(failed, 1, "One item is Error");
    }

    /// Verifies that get_counts() counts Processing state items as active.
    #[test]
    fn get_counts_processing_counted_as_active() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 2);

        queue.update_item_state(&ids[0], DownloadState::Processing);

        let (total, active, queued, _completed, _failed) = queue.get_counts();
        assert_eq!(total, 2);
        assert_eq!(active, 1, "Processing items should count as active");
        assert_eq!(queued, 1);
    }

    // ==========================================================
    // 5. cancel() tests
    // ==========================================================

    /// Verifies that cancelling a Queued item sets its state to Cancelled
    /// and returns true.
    #[test]
    fn cancel_queued_item() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let result = queue.cancel(&id);
        assert!(result, "cancel() should return true for Queued items");

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].state,
            DownloadState::Cancelled,
            "Cancelled queued item should be in Cancelled state"
        );
    }

    /// Verifies that cancelling a Downloading item sets its state to Cancelled
    /// and returns true. The active_count is NOT decremented here; that happens
    /// when the running task detects cancellation and calls on_task_finished().
    #[test]
    fn cancel_downloading_item() {
        let mut queue = DownloadQueue::new();
        let _id = enqueue_one(&mut queue);

        // Move to Downloading state via next_pending
        let (dl_id, _, _) = queue.next_pending().expect("Should have a pending item");
        assert_eq!(queue.active_count, 1);

        let result = queue.cancel(&dl_id);
        assert!(result, "cancel() should return true for Downloading items");

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Cancelled);
        // active_count is NOT decremented by cancel() -- that's the task's job
        assert_eq!(
            queue.active_count, 1,
            "active_count should not be decremented by cancel()"
        );
    }

    /// Verifies that cancelling a Processing item sets its state to Cancelled
    /// and returns true.
    #[test]
    fn cancel_processing_item() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 1);
        queue.update_item_state(&ids[0], DownloadState::Processing);

        let result = queue.cancel(&ids[0]);
        assert!(result, "cancel() should return true for Processing items");

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Cancelled);
    }

    /// Verifies that cancel() returns false for items already in a terminal
    /// state (Complete, Error, Cancelled).
    #[test]
    fn cancel_returns_false_for_terminal_states() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 3);

        queue.set_complete(&ids[0]);
        queue.set_error(&ids[1], "some error");
        queue.cancel(&ids[2]); // First cancel succeeds

        assert!(
            !queue.cancel(&ids[0]),
            "Should return false for Complete item"
        );
        assert!(!queue.cancel(&ids[1]), "Should return false for Error item");
        assert!(
            !queue.cancel(&ids[2]),
            "Should return false for already Cancelled item"
        );
    }

    /// Verifies that cancel() returns false for a non-existent download ID.
    #[test]
    fn cancel_returns_false_for_nonexistent_id() {
        let mut queue = DownloadQueue::new();
        let _ = enqueue_one(&mut queue);

        assert!(
            !queue.cancel("nonexistent-id-12345"),
            "Should return false for unknown ID"
        );
    }

    // ==========================================================
    // 6. clear_finished() tests
    // ==========================================================

    /// Verifies that clear_finished() removes items in terminal states
    /// (Complete, Error, Cancelled) and keeps items in active/pending states.
    #[test]
    fn clear_finished_removes_terminal_keeps_active() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 5);

        // ids[0] = Queued (keep)
        // ids[1] = Downloading (keep)
        queue.update_item_state(&ids[1], DownloadState::Downloading);
        // ids[2] = Complete (remove)
        queue.set_complete(&ids[2]);
        // ids[3] = Error (keep — errored items persist for review)
        queue.set_error(&ids[3], "error msg");
        // ids[4] = Cancelled (remove)
        queue.cancel(&ids[4]);

        let removed = queue.clear_finished();

        assert_eq!(removed, 2, "Should remove 2 items (Complete + Cancelled)");
        let statuses = queue.get_status();
        assert_eq!(statuses.len(), 3, "Should have 3 remaining items");
        assert_eq!(statuses[0].id, ids[0], "Queued item should remain");
        assert_eq!(statuses[1].id, ids[1], "Downloading item should remain");
        assert_eq!(statuses[2].id, ids[3], "Errored item should remain");
    }

    /// Verifies that clear_finished() returns 0 when there are no terminal items.
    #[test]
    fn clear_finished_returns_zero_when_nothing_to_clear() {
        let mut queue = DownloadQueue::new();
        let _ = enqueue_n(&mut queue, 3);

        let removed = queue.clear_finished();
        assert_eq!(
            removed, 0,
            "Nothing should be removed when all items are Queued"
        );
        assert_eq!(queue.get_status().len(), 3, "All items should remain");
    }

    /// Verifies that clear_finished() works correctly on an empty queue.
    #[test]
    fn clear_finished_on_empty_queue() {
        let mut queue = DownloadQueue::new();
        let removed = queue.clear_finished();
        assert_eq!(removed, 0, "Should return 0 for empty queue");
    }

    // ==========================================================
    // 7. next_pending() tests
    // ==========================================================

    /// Verifies that next_pending() returns None for an empty queue.
    #[test]
    fn next_pending_empty_queue() {
        let mut queue = DownloadQueue::new();
        assert!(
            queue.next_pending().is_none(),
            "Empty queue should return None"
        );
    }

    /// Verifies that next_pending() returns the first Queued item, transitions
    /// it to Downloading, and increments active_count.
    #[test]
    fn next_pending_returns_first_queued_item() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 3);

        let result = queue.next_pending();
        assert!(result.is_some(), "Should return Some for non-empty queue");

        let (dl_id, urls, _options) = result.unwrap();
        assert_eq!(dl_id, ids[0], "Should return the first queued item");
        assert_eq!(urls.len(), 1, "Should include the URLs from the request");
        assert_eq!(
            queue.active_count, 1,
            "active_count should be incremented to 1"
        );

        // Verify the item's state changed to Downloading
        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Downloading);
        assert_eq!(
            statuses[1].state,
            DownloadState::Queued,
            "Other items should remain Queued"
        );
    }

    /// Verifies that next_pending() returns None when max_concurrent is reached.
    /// Default max_concurrent is 1, so after one next_pending(), the next call
    /// should return None even if there are queued items.
    #[test]
    fn next_pending_respects_max_concurrent() {
        let mut queue = DownloadQueue::new();
        let _ = enqueue_n(&mut queue, 3);

        // First call succeeds (active_count goes from 0 to 1)
        let first = queue.next_pending();
        assert!(first.is_some(), "First next_pending should succeed");

        // Second call should return None (active_count == max_concurrent == 1)
        let second = queue.next_pending();
        assert!(
            second.is_none(),
            "Second next_pending should return None when at max_concurrent"
        );
    }

    /// Verifies that next_pending() skips non-Queued items and finds the first
    /// Queued item in the deque.
    #[test]
    fn next_pending_skips_non_queued_items() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 3);

        // Set first item to Complete (not Queued)
        queue.set_complete(&ids[0]);
        // Set second item to Error
        queue.set_error(&ids[1], "error");

        // next_pending should skip ids[0] and ids[1], returning ids[2]
        let result = queue.next_pending();
        assert!(result.is_some());
        let (dl_id, _, _) = result.unwrap();
        assert_eq!(
            dl_id, ids[2],
            "Should return the first Queued item, skipping terminal items"
        );
    }

    /// Verifies that next_pending() returns the merged GamdlOptions for the item.
    #[test]
    fn next_pending_returns_merged_options() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = test_request_with_codec_override(SongCodec::AacHe);
        let _id = queue.enqueue(request, &settings);

        let (_, _, options) = queue.next_pending().expect("Should return pending item");
        assert_eq!(
            options.song_codec,
            Some(SongCodec::AacHe),
            "Returned options should reflect the per-download override"
        );
    }

    // ==========================================================
    // 8. on_task_finished() tests
    // ==========================================================

    /// Verifies that on_task_finished() decrements active_count.
    #[test]
    fn on_task_finished_decrements_active_count() {
        let mut queue = DownloadQueue::new();
        let _ = enqueue_one(&mut queue);
        let _ = queue.next_pending(); // active_count = 1

        assert_eq!(queue.active_count, 1);
        queue.on_task_finished();
        assert_eq!(
            queue.active_count, 0,
            "active_count should be 0 after on_task_finished"
        );
    }

    /// Verifies that on_task_finished() does not underflow below zero.
    /// Calling it when active_count is already 0 should be a no-op.
    #[test]
    fn on_task_finished_does_not_underflow() {
        let mut queue = DownloadQueue::new();
        assert_eq!(queue.active_count, 0);

        queue.on_task_finished();
        assert_eq!(queue.active_count, 0, "active_count should not go below 0");

        // Call it multiple times to be sure
        queue.on_task_finished();
        queue.on_task_finished();
        assert_eq!(queue.active_count, 0);
    }

    /// Verifies that after on_task_finished(), the queue can start new items
    /// since a concurrent slot has been freed.
    #[test]
    fn on_task_finished_frees_slot_for_next_pending() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 2);

        // Start first item
        let first = queue.next_pending();
        assert!(first.is_some());
        // Verify second item can't start yet
        assert!(
            queue.next_pending().is_none(),
            "Should be at max_concurrent"
        );

        // Finish first task
        queue.on_task_finished();

        // Now second item should be startable
        let second = queue.next_pending();
        assert!(
            second.is_some(),
            "Should be able to start next item after finishing"
        );
        let (dl_id, _, _) = second.unwrap();
        assert_eq!(dl_id, ids[1]);
    }

    // ==========================================================
    // 9. is_cancelled() tests
    // ==========================================================

    /// Verifies that is_cancelled() returns true for cancelled items.
    #[test]
    fn is_cancelled_returns_true_for_cancelled() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);
        queue.cancel(&id);

        assert!(
            queue.is_cancelled(&id),
            "Should return true for cancelled item"
        );
    }

    /// Verifies that is_cancelled() returns false for non-cancelled items
    /// in various states (Queued, Downloading, Complete, Error).
    #[test]
    fn is_cancelled_returns_false_for_non_cancelled() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 4);

        // ids[0] = Queued
        assert!(
            !queue.is_cancelled(&ids[0]),
            "Queued item should not be cancelled"
        );

        // ids[1] = Downloading (via update_item_state)
        queue.update_item_state(&ids[1], DownloadState::Downloading);
        assert!(
            !queue.is_cancelled(&ids[1]),
            "Downloading item should not be cancelled"
        );

        // ids[2] = Complete
        queue.set_complete(&ids[2]);
        assert!(
            !queue.is_cancelled(&ids[2]),
            "Complete item should not be cancelled"
        );

        // ids[3] = Error
        queue.set_error(&ids[3], "error");
        assert!(
            !queue.is_cancelled(&ids[3]),
            "Error item should not be cancelled"
        );
    }

    /// Verifies that is_cancelled() returns false for a non-existent download ID.
    #[test]
    fn is_cancelled_returns_false_for_nonexistent_id() {
        let queue = DownloadQueue::new();
        assert!(
            !queue.is_cancelled("does-not-exist"),
            "Should return false for unknown ID"
        );
    }

    // ==========================================================
    // 10. update_item_progress() tests
    // ==========================================================

    /// Verifies that a DownloadProgress event updates the item's progress,
    /// speed, eta, and sets state to Downloading.
    #[test]
    fn update_item_progress_download_progress() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::DownloadProgress {
            percent: 45.5,
            speed: "2.5MiB/s".to_string(),
            eta: "00:30".to_string(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        let s = &statuses[0];
        assert!((s.progress - 45.5).abs() < 0.001, "Progress should be 45.5");
        assert_eq!(s.speed.as_deref(), Some("2.5MiB/s"));
        assert_eq!(s.eta.as_deref(), Some("00:30"));
        assert_eq!(s.state, DownloadState::Downloading);
    }

    /// Verifies that a TrackInfo event updates the current_track field
    /// with the formatted "Artist - Title" string.
    #[test]
    fn update_item_progress_track_info_with_artist() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::TrackInfo {
            title: "Anti-Hero".to_string(),
            artist: "Taylor Swift".to_string(),
            album: String::new(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].current_track.as_deref(),
            Some("Taylor Swift - Anti-Hero"),
            "Should format as 'Artist - Title'"
        );
    }

    /// Verifies that a TrackInfo event with an empty artist just uses the title.
    #[test]
    fn update_item_progress_track_info_without_artist() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::TrackInfo {
            title: "Bohemian Rhapsody".to_string(),
            artist: String::new(),
            album: String::new(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].current_track.as_deref(),
            Some("Bohemian Rhapsody"),
            "Should use title only when artist is empty"
        );
    }

    /// Verifies that a ProcessingStep event sets the state to Processing.
    #[test]
    fn update_item_progress_processing_step() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::ProcessingStep {
            step: "Remuxing to M4A".to_string(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].state,
            DownloadState::Processing,
            "ProcessingStep event should set state to Processing"
        );
    }

    /// Verifies that a Complete event sets the output_path and progress to 100%.
    #[test]
    fn update_item_progress_complete() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::Complete {
            path: "/output/song.m4a".to_string(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].output_path.as_deref(),
            Some("/output/song.m4a"),
            "Complete event should set output_path"
        );
        assert!(
            (statuses[0].progress - 100.0).abs() < 0.001,
            "Complete event should set progress to 100%"
        );
    }

    /// Verifies that an Error event sets the error field on the item.
    #[test]
    fn update_item_progress_error() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::Error {
            message: "Codec not available".to_string(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].error.as_deref(),
            Some("Codec not available"),
            "Error event should set the error field"
        );
        // Note: Error event does NOT change state -- that's handled by set_error()
        // after the process exits and retry/fallback logic is evaluated.
        assert_eq!(
            statuses[0].state,
            DownloadState::Queued,
            "Error event should NOT change state (that's set_error's job)"
        );
    }

    /// Verifies that an Unknown event does not change any item fields.
    #[test]
    fn update_item_progress_unknown_event_is_no_op() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::Unknown {
            raw: "some random output".to_string(),
        };
        queue.update_item_progress(&id, &event);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].state,
            DownloadState::Queued,
            "Unknown event should not change state"
        );
        assert_eq!(
            statuses[0].progress, 0.0,
            "Unknown event should not change progress"
        );
    }

    /// Verifies that update_item_progress is a no-op for non-existent IDs
    /// (does not panic).
    #[test]
    fn update_item_progress_nonexistent_id_is_safe() {
        let mut queue = DownloadQueue::new();
        let _ = enqueue_one(&mut queue);

        let event = GamdlOutputEvent::DownloadProgress {
            percent: 50.0,
            speed: "1MiB/s".to_string(),
            eta: "00:10".to_string(),
        };
        // Should not panic
        queue.update_item_progress("nonexistent-id", &event);

        // Original item should be unchanged
        let statuses = queue.get_status();
        assert_eq!(statuses[0].progress, 0.0);
    }

    // ==========================================================
    // 11. set_error() and set_complete() tests
    // ==========================================================

    /// Verifies that set_error() sets the state to Error and records the
    /// error message.
    #[test]
    fn set_error_sets_state_and_message() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        queue.set_error(&id, "Network timeout occurred");

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Error);
        assert_eq!(
            statuses[0].error.as_deref(),
            Some("Network timeout occurred")
        );
    }

    /// Verifies that set_error() is a no-op for non-existent IDs.
    #[test]
    fn set_error_nonexistent_id_is_safe() {
        let mut queue = DownloadQueue::new();
        // Should not panic
        queue.set_error("nonexistent", "some error");
    }

    /// Verifies that set_complete() sets the state to Complete and progress
    /// to 100%.
    #[test]
    fn set_complete_sets_state_and_progress() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        queue.set_complete(&id);

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Complete);
        assert!(
            (statuses[0].progress - 100.0).abs() < 0.001,
            "set_complete should set progress to 100%"
        );
    }

    /// Verifies that set_complete() is a no-op for non-existent IDs.
    #[test]
    fn set_complete_nonexistent_id_is_safe() {
        let mut queue = DownloadQueue::new();
        // Should not panic
        queue.set_complete("nonexistent");
    }

    // ==========================================================
    // 12. try_network_retry() tests
    // ==========================================================

    /// Verifies that try_network_retry() resets the item to Queued state when
    /// retries remain, and decrements the retry counter.
    #[test]
    fn try_network_retry_resets_to_queued_when_retries_remain() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        // Simulate an error state
        queue.set_error(&id, "Network timeout");

        let result = queue.try_network_retry(&id);
        assert!(result, "Should return true when retries remain");

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].state,
            DownloadState::Queued,
            "Should reset to Queued for retry"
        );
        assert!(
            statuses[0].error.is_none(),
            "Error should be cleared on retry"
        );
        assert_eq!(statuses[0].progress, 0.0, "Progress should reset to 0");
    }

    /// Verifies that try_network_retry() returns false when all retries have
    /// been exhausted (default: 3 retries).
    #[test]
    fn try_network_retry_returns_false_when_exhausted() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        // Use up all 3 retries
        for _ in 0..3 {
            queue.set_error(&id, "network error");
            let ok = queue.try_network_retry(&id);
            assert!(ok, "Should succeed while retries remain");
        }

        // 4th attempt should fail
        queue.set_error(&id, "network error");
        let result = queue.try_network_retry(&id);
        assert!(!result, "Should return false after all retries exhausted");
    }

    /// Verifies that try_network_retry() returns false for a non-existent ID.
    #[test]
    fn try_network_retry_nonexistent_id() {
        let mut queue = DownloadQueue::new();
        assert!(!queue.try_network_retry("nonexistent"));
    }

    // ==========================================================
    // 13. try_fallback() tests
    // ==========================================================

    /// Verifies that try_fallback() returns new options with the next codec in
    /// the fallback chain, resets the item to Queued, and marks fallback_occurred.
    #[test]
    fn try_fallback_returns_next_codec_in_chain() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let id = queue.enqueue(test_request(), &settings);

        // Simulate an error requiring fallback
        queue.set_error(&id, "Codec not available");

        // First fallback: chain[0] = Alac (initial), so next = chain[1] = Atmos
        let result = queue.try_fallback(&id, &settings);
        assert!(result.is_some(), "First fallback should succeed");

        let new_opts = result.unwrap();
        assert_eq!(
            new_opts.song_codec,
            Some(SongCodec::Atmos),
            "First fallback should be Atmos (index 1 in chain)"
        );
        // song_codec_priority should be set to just the single fallback codec
        // to prevent process_download_item() from rebuilding the full chain
        // and to override config.ini's full priority chain.
        assert_eq!(
            new_opts.song_codec_priority,
            Some("atmos".to_string()),
            "song_codec_priority should be overridden to single fallback codec"
        );

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].state,
            DownloadState::Queued,
            "Should reset to Queued"
        );
        assert!(
            statuses[0].fallback_occurred,
            "Should mark fallback_occurred as true"
        );
        assert_eq!(statuses[0].codec_used.as_deref(), Some("atmos"));
        assert!(
            statuses[0].error.is_none(),
            "Error should be cleared on fallback"
        );
        assert_eq!(statuses[0].progress, 0.0, "Progress should be reset");
    }

    /// Verifies that try_fallback() returns None when all codecs in the
    /// fallback chain have been exhausted.
    #[test]
    fn try_fallback_returns_none_when_chain_exhausted() {
        let mut queue = DownloadQueue::new();
        let mut settings = test_settings();
        // Use a short chain for testing
        settings.music_fallback_chain = vec![SongCodec::Alac, SongCodec::Aac];
        let id = queue.enqueue(test_request(), &settings);

        // First fallback: Alac (0) -> Aac (1)
        queue.set_error(&id, "codec error");
        let result1 = queue.try_fallback(&id, &settings);
        assert!(result1.is_some(), "First fallback to Aac should succeed");

        // Second fallback: chain exhausted (index 2 >= chain.len() of 2)
        queue.set_error(&id, "codec error again");
        let result2 = queue.try_fallback(&id, &settings);
        assert!(result2.is_none(), "Should return None when chain exhausted");
    }

    /// Verifies that try_fallback() returns None when fallback is disabled
    /// in settings, regardless of chain contents.
    #[test]
    fn try_fallback_returns_none_when_disabled() {
        let mut queue = DownloadQueue::new();
        let mut settings = test_settings();
        settings.fallback_enabled = false;
        let id = queue.enqueue(test_request(), &settings);

        queue.set_error(&id, "codec error");
        let result = queue.try_fallback(&id, &settings);
        assert!(
            result.is_none(),
            "Should return None when fallback_enabled is false"
        );
    }

    /// Verifies that try_fallback() returns None for a non-existent ID.
    #[test]
    fn try_fallback_nonexistent_id() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let result = queue.try_fallback("nonexistent", &settings);
        assert!(result.is_none());
    }

    /// Verifies that multiple successive fallbacks advance through the entire
    /// fallback chain correctly.
    #[test]
    fn try_fallback_advances_through_full_chain() {
        let mut queue = DownloadQueue::new();
        let mut settings = test_settings();
        settings.music_fallback_chain = vec![SongCodec::Alac, SongCodec::Atmos, SongCodec::Aac];
        let id = queue.enqueue(test_request(), &settings);

        // Fallback 1: Alac -> Atmos
        queue.set_error(&id, "codec error");
        let r1 = queue.try_fallback(&id, &settings);
        let r1_opts = r1.unwrap();
        assert_eq!(r1_opts.song_codec, Some(SongCodec::Atmos));
        assert_eq!(
            r1_opts.song_codec_priority,
            Some("atmos".to_string()),
            "Priority should be single codec on fallback"
        );

        // Fallback 2: Atmos -> Aac
        queue.set_error(&id, "codec error");
        let r2 = queue.try_fallback(&id, &settings);
        let r2_opts = r2.unwrap();
        assert_eq!(r2_opts.song_codec, Some(SongCodec::Aac));
        assert_eq!(
            r2_opts.song_codec_priority,
            Some("aac".to_string()),
            "Priority should be single codec on fallback"
        );

        // Fallback 3: exhausted
        queue.set_error(&id, "codec error");
        let r3 = queue.try_fallback(&id, &settings);
        assert!(r3.is_none(), "Chain should be exhausted after 3 codecs");
    }

    // ==========================================================
    // 14. retry() tests
    // ==========================================================

    /// Verifies that retry() resets an errored item fully to Queued state
    /// with fresh options, reset counters, and cleared error/progress.
    #[test]
    fn retry_resets_errored_item_to_queued() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let id = queue.enqueue(test_request(), &settings);

        queue.set_error(&id, "Download failed");

        let result = queue.retry(&id, &settings);
        assert!(result, "retry() should return true for Error items");

        let statuses = queue.get_status();
        let s = &statuses[0];
        assert_eq!(s.state, DownloadState::Queued, "Should be reset to Queued");
        assert!(s.error.is_none(), "Error should be cleared");
        assert_eq!(s.progress, 0.0, "Progress should be reset");
        assert!(!s.fallback_occurred, "fallback_occurred should be reset");
        assert_eq!(
            s.codec_used.as_deref(),
            Some("alac"),
            "Codec should be re-merged from settings"
        );
    }

    /// Verifies that retry() resets a cancelled item to Queued state.
    #[test]
    fn retry_resets_cancelled_item() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let id = queue.enqueue(test_request(), &settings);

        queue.cancel(&id);
        assert_eq!(queue.get_status()[0].state, DownloadState::Cancelled);

        let result = queue.retry(&id, &settings);
        assert!(result, "retry() should return true for Cancelled items");

        let statuses = queue.get_status();
        assert_eq!(statuses[0].state, DownloadState::Queued);
    }

    /// Verifies that retry() returns false for non-terminal states (Queued,
    /// Downloading, Processing, Complete).
    #[test]
    fn retry_returns_false_for_non_terminal_states() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let ids = enqueue_n(&mut queue, 4);

        // ids[0] = Queued
        assert!(
            !queue.retry(&ids[0], &settings),
            "Should not retry Queued item"
        );

        // ids[1] = Downloading
        queue.update_item_state(&ids[1], DownloadState::Downloading);
        assert!(
            !queue.retry(&ids[1], &settings),
            "Should not retry Downloading item"
        );

        // ids[2] = Processing
        queue.update_item_state(&ids[2], DownloadState::Processing);
        assert!(
            !queue.retry(&ids[2], &settings),
            "Should not retry Processing item"
        );

        // ids[3] = Complete
        queue.set_complete(&ids[3]);
        assert!(
            !queue.retry(&ids[3], &settings),
            "Should not retry Complete item"
        );
    }

    /// Verifies that retry() returns false for a non-existent download ID.
    #[test]
    fn retry_returns_false_for_nonexistent_id() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        assert!(!queue.retry("nonexistent", &settings));
    }

    /// Verifies that retry() re-merges options from the original request
    /// with the current settings. This means if settings changed between the
    /// original enqueue and the retry, the retry picks up the new settings.
    #[test]
    fn retry_remerges_options_from_original_request() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let request = test_request_with_codec_override(SongCodec::AacHe);
        let id = queue.enqueue(request, &settings);

        queue.set_error(&id, "error");

        // Retry with the same settings
        let result = queue.retry(&id, &settings);
        assert!(result);

        let statuses = queue.get_status();
        assert_eq!(
            statuses[0].codec_used.as_deref(),
            Some("aac-he"),
            "Retry should preserve the original per-download override"
        );
    }

    /// Verifies that retry() resets the network retries counter and fallback
    /// index, which can be verified by subsequent try_network_retry calls
    /// succeeding again after a retry.
    #[test]
    fn retry_resets_retry_counters() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let id = queue.enqueue(test_request(), &settings);

        // Exhaust network retries
        for _ in 0..3 {
            queue.set_error(&id, "network");
            queue.try_network_retry(&id);
        }
        queue.set_error(&id, "network");
        assert!(!queue.try_network_retry(&id), "Retries should be exhausted");

        // Now use retry() to do a full reset
        queue.set_error(&id, "network");
        queue.retry(&id, &settings);

        // Network retries should be available again
        queue.set_error(&id, "network");
        assert!(
            queue.try_network_retry(&id),
            "After retry(), network retries should be reset to max"
        );
    }

    // ==========================================================
    // update_item_state() tests
    // ==========================================================

    /// Verifies that update_item_state() changes the state of the item.
    #[test]
    fn update_item_state_changes_state() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        queue.update_item_state(&id, DownloadState::Downloading);
        assert_eq!(queue.get_status()[0].state, DownloadState::Downloading);

        queue.update_item_state(&id, DownloadState::Processing);
        assert_eq!(queue.get_status()[0].state, DownloadState::Processing);
    }

    /// Verifies that update_item_state() is a no-op for non-existent IDs.
    #[test]
    fn update_item_state_nonexistent_id_is_safe() {
        let mut queue = DownloadQueue::new();
        // Should not panic
        queue.update_item_state("nonexistent", DownloadState::Downloading);
    }

    // ==========================================================
    // new_queue_handle() test
    // ==========================================================

    /// Verifies that new_queue_handle() creates an Arc<Mutex<DownloadQueue>>
    /// that can be locked and used.
    #[tokio::test]
    async fn new_queue_handle_creates_usable_handle() {
        let handle = new_queue_handle();
        let queue = handle.lock().await;
        assert!(
            queue.items.is_empty(),
            "New queue handle should wrap an empty queue"
        );
        assert_eq!(queue.get_counts(), (0, 0, 0, 0, 0));
    }

    // ==========================================================
    // Integration / workflow tests
    // ==========================================================

    /// Simulates a full download lifecycle: enqueue -> next_pending (Downloading)
    /// -> progress updates -> set_complete -> on_task_finished. Verifies state
    /// transitions at each step.
    #[test]
    fn full_lifecycle_happy_path() {
        let mut queue = DownloadQueue::new();
        let settings = test_settings();
        let id = queue.enqueue(test_request(), &settings);

        // Step 1: Item is Queued
        assert_eq!(queue.get_status()[0].state, DownloadState::Queued);

        // Step 2: Start downloading
        let (dl_id, _, _) = queue.next_pending().unwrap();
        assert_eq!(dl_id, id);
        assert_eq!(queue.get_status()[0].state, DownloadState::Downloading);
        assert_eq!(queue.active_count, 1);

        // Step 3: Progress updates
        queue.update_item_progress(
            &id,
            &GamdlOutputEvent::TrackInfo {
                title: "Test Song".to_string(),
                artist: "Test Artist".to_string(),
                album: String::new(),
            },
        );
        queue.update_item_progress(
            &id,
            &GamdlOutputEvent::DownloadProgress {
                percent: 50.0,
                speed: "5MiB/s".to_string(),
                eta: "00:10".to_string(),
            },
        );
        assert!((queue.get_status()[0].progress - 50.0).abs() < 0.001);
        assert_eq!(
            queue.get_status()[0].current_track.as_deref(),
            Some("Test Artist - Test Song")
        );

        // Step 4: Processing
        queue.update_item_progress(
            &id,
            &GamdlOutputEvent::ProcessingStep {
                step: "Remuxing to M4A".to_string(),
            },
        );
        assert_eq!(queue.get_status()[0].state, DownloadState::Processing);

        // Step 5: Complete
        queue.update_item_progress(
            &id,
            &GamdlOutputEvent::Complete {
                path: "/output/test.m4a".to_string(),
            },
        );
        queue.set_complete(&id);
        queue.on_task_finished();

        let final_status = &queue.get_status()[0];
        assert_eq!(final_status.state, DownloadState::Complete);
        assert!((final_status.progress - 100.0).abs() < 0.001);
        assert_eq!(
            final_status.output_path.as_deref(),
            Some("/output/test.m4a")
        );
        assert_eq!(queue.active_count, 0);
    }

    /// Simulates a download that fails with a codec error and successfully
    /// falls back to the next codec in the chain.
    #[test]
    fn lifecycle_with_codec_fallback() {
        let mut queue = DownloadQueue::new();
        let mut settings = test_settings();
        settings.music_fallback_chain = vec![SongCodec::Alac, SongCodec::Aac, SongCodec::AacLegacy];
        let id = queue.enqueue(test_request(), &settings);

        // Start and fail with codec error
        let _ = queue.next_pending();
        queue.set_error(&id, "Codec not available for ALAC");
        queue.on_task_finished();

        // Fallback to AAC
        let fallback = queue.try_fallback(&id, &settings);
        assert!(fallback.is_some());
        assert_eq!(fallback.unwrap().song_codec, Some(SongCodec::Aac));

        // Item should be re-queued
        assert_eq!(queue.get_status()[0].state, DownloadState::Queued);
        assert!(queue.get_status()[0].fallback_occurred);
    }

    /// Simulates a download that fails with a network error and retries
    /// successfully.
    #[test]
    fn lifecycle_with_network_retry() {
        let mut queue = DownloadQueue::new();
        let id = enqueue_one(&mut queue);

        // Start and fail with network error
        let _ = queue.next_pending();
        queue.set_error(&id, "Network timeout");
        queue.on_task_finished();

        // Network retry should succeed
        let retried = queue.try_network_retry(&id);
        assert!(retried);

        // Item should be re-queued
        let s = &queue.get_status()[0];
        assert_eq!(s.state, DownloadState::Queued);
        assert!(s.error.is_none());
        assert_eq!(s.progress, 0.0);
    }

    /// Verifies that multiple items can cycle through the queue when only
    /// one concurrent slot is available. The second item should start only
    /// after the first completes.
    #[test]
    fn sequential_processing_with_single_concurrency() {
        let mut queue = DownloadQueue::new();
        let ids = enqueue_n(&mut queue, 3);

        // Start first item
        let (id1, _, _) = queue.next_pending().unwrap();
        assert_eq!(id1, ids[0]);

        // Can't start second while first is running
        assert!(queue.next_pending().is_none());

        // Finish first
        queue.set_complete(&ids[0]);
        queue.on_task_finished();

        // Now second can start
        let (id2, _, _) = queue.next_pending().unwrap();
        assert_eq!(id2, ids[1]);

        // Finish second
        queue.set_complete(&ids[1]);
        queue.on_task_finished();

        // Third can start
        let (id3, _, _) = queue.next_pending().unwrap();
        assert_eq!(id3, ids[2]);
    }

    // ----------------------------------------------------------
    // Python Traceback Extraction Tests
    // ----------------------------------------------------------

    #[test]
    fn extracts_type_error_from_traceback() {
        let lines = vec![
            "Traceback (most recent call last):".to_string(),
            "  File \"foo.py\", line 42, in bar".to_string(),
            "    some_call()".to_string(),
            "TypeError: 'NoneType' object has no attribute 'x'".to_string(),
        ];
        assert_eq!(
            extract_python_exception(&lines),
            Some("TypeError: 'NoneType' object has no attribute 'x'".to_string())
        );
    }

    #[test]
    fn extracts_dotted_exception_from_traceback() {
        let lines = vec![
            "Traceback (most recent call last):".to_string(),
            "  File \"api.py\", line 10".to_string(),
            "    response.raise_for_status()".to_string(),
            "requests.exceptions.HTTPError: 403 Client Error".to_string(),
        ];
        assert_eq!(
            extract_python_exception(&lines),
            Some("requests.exceptions.HTTPError: 403 Client Error".to_string())
        );
    }

    #[test]
    fn returns_none_without_traceback() {
        let lines = vec!["Normal output line".to_string(), "Another line".to_string()];
        assert_eq!(extract_python_exception(&lines), None);
    }

    #[test]
    fn handles_multiple_tracebacks_uses_last() {
        let lines = vec![
            "Traceback (most recent call last):".to_string(),
            "  File \"a.py\", line 1".to_string(),
            "RuntimeError: first".to_string(),
            "".to_string(),
            "Traceback (most recent call last):".to_string(),
            "  File \"b.py\", line 2".to_string(),
            "ValueError: second".to_string(),
        ];
        assert_eq!(
            extract_python_exception(&lines),
            Some("ValueError: second".to_string())
        );
    }

    #[test]
    fn handles_traceback_with_no_exception_after() {
        let lines = vec![
            "Traceback (most recent call last):".to_string(),
            "  File \"foo.py\", line 1".to_string(),
            "  File \"bar.py\", line 2".to_string(),
        ];
        // All lines after Traceback are indented, so no exception found
        assert_eq!(extract_python_exception(&lines), None);
    }
}
