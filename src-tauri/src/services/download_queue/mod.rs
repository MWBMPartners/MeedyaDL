// Copyright (c) 2024-2026 MeedyaSuite
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
//                             release_slot(attempt) --> process_queue() (cascade)
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
// 6. **Cancellation polling**: Running downloads are checked every 250ms via
//    try_wait() + should_keep_running(attempt). The process is killed once the
//    item is cancelled, retried or removed.
//
// 7. **Attempts (#1432)**: every run of an item carries an `Attempt` (the
//    item's ID plus a number that changes whenever the item starts, is put
//    back in the queue or is cancelled). State changes and the queue slot
//    belong to one attempt, so an old run can never finish, fail or restart
//    a retried item, nor free the slot of the item that started next.
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

pub(crate) use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
// Future and Pin are needed for the recursive async pattern in process_queue().
// Recursive async functions cannot use normal `async fn` syntax because the
// compiler cannot determine the size of the future at compile time.
// Instead, we return Pin<Box<dyn Future<Output = ()> + Send>>.
// Ref: https://doc.rust-lang.org/std/pin/index.html
pub(crate) use std::future::Future;
pub(crate) use std::pin::Pin;
pub(crate) use std::sync::atomic::{AtomicBool, Ordering};
pub(crate) use std::sync::{Arc, Mutex as StdMutex};
// Tokio's Mutex is used instead of std::sync::Mutex because the lock is held
// across .await points. std::sync::Mutex would block the entire thread;
// tokio::sync::Mutex yields the task instead.
// Ref: https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html
pub(crate) use tokio::sync::Mutex;

// Emitter trait provides app.emit() for sending events to the frontend.
pub(crate) use tauri::{AppHandle, Emitter};

// DownloadRequest: The user's download request from the frontend (URLs + optional overrides).
// DownloadState: Enum of lifecycle states (Queued, Downloading, Processing, Complete, Error, Cancelled).
// QueueItemStatus: The public-facing status struct sent to the frontend for UI rendering.
pub(crate) use crate::models::download::{DownloadRequest, DownloadState, QueueItemStatus};
// GamdlOptions: Typed representation of GAMDL CLI arguments, used as the "effective" options
// after merging per-download overrides with global settings.
// SongCodec: Enum of audio codec options, used for companion download planning and
// codec suffix logic.
pub(crate) use crate::models::codec_registry::{
    codec_suffix_from_registry, song_codec_to_registry_id,
};
pub(crate) use crate::models::gamdl_options::{
    ArtistAutoSelect, GamdlOptions, LyricsFormat, SongCodec,
};
// AppSettings: The full application settings, used for merging defaults and fallback chain config.
// CompanionMode: Enum controlling companion download behavior (Disabled, AtmosToLossless, etc.).
pub(crate) use crate::models::settings::{AppSettings, CompanionMode};
// config_service: Used to load settings during fallback decisions.
// gamdl_service: Provides build_gamdl_command_public() and GamdlProgress for subprocess execution.
// crash_report_service: Used to save download error reports for user-reportable diagnostics.
pub(crate) use crate::services::{
    config_service, crash_report_service, gamdl_service, history_service,
};
// CrashReport: Reused for download error reports (source: "download_error").
pub(crate) use crate::models::crash_report::CrashReport;
// process: Provides parse_gamdl_output() for parsing GAMDL output lines and
// classify_error() for categorizing errors (codec, network, etc.) for retry logic.
pub(crate) use crate::utils::process;
// Activity log helpers: emit_download_log for per-download messages,
// emit_app_log for system-level messages. `ActivityLogEvent` is no
// longer imported here — Phase 3.5e moved every direct
// `app.emit("activity-log", &event)` site through the new
// `emit_subprocess_line` helper, so download_queue.rs no longer needs
// to construct events directly.
pub(crate) use crate::utils::activity_log::{
    emit_app_log, emit_download_error, emit_download_log, emit_download_warn,
    emit_verbose_download_log,
};

// ============================================================
// Graceful shutdown signal
// ============================================================

/// Application-wide shutdown signal for fire-and-forget background tasks.
///
/// When the user closes the app window or clicks "Quit" in the tray menu,
/// this flag is set to `true`. Fire-and-forget tasks (companion downloads,
/// lyrics companions, and the enrichment pipeline) check this flag between
/// iterations and exit early instead of starting new work.
///
/// Uses `AtomicBool` with `Ordering::Relaxed` for minimal overhead — the
/// shutdown signal only needs to propagate eventually (within one loop
/// iteration), not with strict memory ordering guarantees.
///
/// # Cloning
/// The `Clone` derive produces a cheap `Arc` reference count increment,
/// making it safe to pass to multiple `tokio::spawn` tasks.
#[derive(Clone, Default)]
pub struct ShutdownSignal(Arc<AtomicBool>);

impl ShutdownSignal {
    /// Creates a new shutdown signal in the non-triggered state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Signals all background tasks to stop at their next check point.
    pub fn trigger(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Returns `true` if shutdown has been requested.
    pub fn is_triggered(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// Exposes the inner `Arc<AtomicBool>` so long-lived background
    /// tasks (e.g. the on-disk activity log writer in #541) can poll
    /// the shutdown flag without holding a Tauri `State` reference.
    pub fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0)
    }
}

// ============================================================
// Activity log event helpers
// ============================================================
// ActivityLogEvent, emit_download_log, and emit_app_log are now in
// crate::utils::activity_log (shared across commands and services).
// The import is at the top of this file.

// ============================================================
// Submodules
//
// The former single-file `download_queue.rs` was split into topic
// modules (behaviour-preserving). Each submodule does `use super::*;`
// to pull in the shared imports (re-exported `pub(crate)` above) and
// the sibling items re-exported below, so the code reads as if it were
// still one module. The `process_queue` pump module is named
// `processing` (not `process`) to avoid colliding with the
// `crate::utils::process` import re-exported above.
// ============================================================

mod companion_lyrics;
mod companions;
mod helpers;
mod lyrics_pass;
pub(crate) mod lyrics_steps;
mod notifications;
mod options;
mod persistence;
mod processing;

// Flatten each submodule's items back into the `download_queue` namespace
// so intra-crate references and the `#[cfg(test)] mod tests` block resolve
// exactly as before the split.
pub(crate) use companions::*;
pub(crate) use helpers::*;
pub(crate) use notifications::*;
pub(crate) use options::*;
pub(crate) use persistence::*;

// `processing`'s only cross-module symbol used by non-test code is
// `process_queue` (re-exported `pub` below). Its remaining `pub(crate)`
// helpers are exercised only by the `#[cfg(test)] mod tests` submodule,
// so re-export the two the tests reach for under `cfg(test)` alone — a
// plain `pub(crate) use processing::*;` glob would be an unused import in
// the lib build.
#[cfg(test)]
pub(crate) use processing::{extract_python_exception, is_structlog_line_start};

// Sibling `services::*` modules the moved code reaches via `super::` (the
// former single file lived directly under `services`, so `super` meant
// `services`; the submodules are one level deeper). Re-exported here so
// those verbatim `super::X` paths keep resolving. `config_service`,
// `crash_report_service`, `gamdl_service`, and `history_service` are
// already re-exported via the top-of-file imports.
pub(crate) use super::{
    acoustid_service, animated_artwork_service, apple_music_api, ass_subtitle_service,
    companion_supervisor, dependency_manager, enhanced_lyrics_service, gamdl_capabilities,
    lyricsfile_service, mediainfo_service, metadata_tag_service, music_video_cover_embed,
    music_video_subtitle_service, musicbrainz_service, progress_stages, replaygain_service,
    rich_srt_service, settings_cache, webvtt_service,
};

// Progress-stage helpers used across several submodules. The original
// single file had one module-level `use super::progress_stages::{…}`;
// re-exporting the items here lets every submodule reach them via
// `use super::*` exactly as before.
pub(crate) use progress_stages::{
    set_label_only_for, set_stage_with_label, set_stage_with_label_for, ProgressStage,
};

// Preserve the exact public function surface: these were `pub fn` in the
// former single file (and reachable as `download_queue::X`), so re-export
// them as `pub` — the `pub(crate) use *` globs above would otherwise only
// expose them crate-locally, changing the public API and (for the
// currently-callerless `clear_queue_file`) tripping dead_code.
pub use notifications::test_desktop_notification;
pub use persistence::{clear_queue_file, load_queue_from_disk, save_queue_to_disk};
pub(crate) use processing::finish_queue_if_drained;
pub use processing::process_queue;

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
pub(crate) struct QueueItem {
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
    /// Index into the engine chain for this platform (from engines.toml).
    /// 0 = primary engine, 1 = first fallback engine, etc.
    /// Incremented by `try_engine_fallback()` on tool errors (#320).
    pub engine_fallback_index: usize,
    /// Whether [`try_storefront_fallback`] has already rewritten this
    /// item's URL once (#666). Budget is one attempt — without this flag
    /// the same item could ping-pong between two storefronts forever
    /// when neither catalog has the content. Reset by [`retry`] so a
    /// user-driven manual retry from the UI is allowed to try again.
    pub storefront_fallback_attempted: bool,
    /// The format suffixes MeedyaDL itself appended to `merged_options`'
    /// file templates (newest last) — today only the format fallback
    /// (`try_fallback`) does. Lets them be removed exactly
    /// (`strip_recorded_codec_suffixes`) without ever touching a suffix the
    /// person typed into their own template. Empty whenever the options
    /// are merged afresh (enqueue, retry, restore). Not saved to disk: a
    /// saved queue keeps only the original request and is merged afresh on
    /// restore, so a restored item has nothing added to strip.
    pub added_template_suffixes: Vec<&'static str>,
    /// Every file path the main download runs reported, in the order seen
    /// (GAMDL 4.x `✓`/`↷ … → path` lines; GAMDL 3.x reports none), without
    /// repeats and capped at
    /// [`MAX_REPORTED_PATHS`]. Enrichment reads it to find this download's
    /// files in other album folders (#1446). In memory only: a restored item
    /// starts empty. Cleared where a main download attempt starts
    /// ([`DownloadQueue::begin_run`]), never before the gap-fill run.
    pub reported_paths: Vec<PathBuf>,
    /// The number of the attempt that is allowed to change this item now
    /// (#1432). A new number is given whenever the item starts, is put back
    /// in the queue, or is cancelled, so a run that belongs to an earlier
    /// attempt can see it has been replaced and leaves the item alone.
    pub attempt: u64,
    /// Set by the stall check to the number of the attempt it marked failed
    /// (#1432). That attempt is left running (it is not stopped) but may no
    /// longer change the item; if it later finishes, the item is queued again
    /// once. Cleared whenever the item gets a new attempt number.
    pub stalled_attempt: Option<u64>,
    /// Whether the current attempt has reported saving at least one file
    /// (#1432 review): set by each saved-file report from the attempt that
    /// owns the item, and by the reports of a run the stall check marked
    /// failed. Cleared whenever the item gets a new attempt number, so files
    /// an EARLIER attempt saved (the output path is kept for "Open Folder")
    /// never count. Read only for a stalled run: a late clean exit counts as
    /// "finished after all" only when files were really saved; a run that
    /// ends without saving anything is not queued again.
    pub stalled_run_saved_files: bool,
    /// Set when MeedyaDL queued this item again by itself because a run the
    /// stall check had marked failed finished after all (#1432 review).
    /// While set, a second "stalled, then finished after all" only writes a
    /// line and does not queue it again, so an album that hangs in the same
    /// place every time cannot loop for ever. Cleared only by the person's
    /// own Retry (a new item starts with it clear).
    pub auto_retried_after_stall: bool,
    /// Set together with `auto_retried_after_stall`: the stall check has
    /// already counted this item as failed for the "Queue finished" summary.
    /// When the automatic retry's own result is counted, that earlier
    /// failure is taken back first, so one item is counted once (#1432
    /// review). Cleared once used, and by the person's Retry.
    pub stall_failure_to_withdraw: bool,
}

/// What happens when a run that the stall check marked failed comes to an
/// end (#1432 review). Decided under the queue lock by
/// [`DownloadQueue::on_late_finish`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LateFinish {
    /// Nothing to say or do: the run did not end as "stalled", or it did not
    /// genuinely finish (it timed out, was stopped, or saved nothing).
    Nothing,
    /// It finished after all and the item has been queued again once.
    QueuedAgain,
    /// It finished after all, but it was already the one automatic retry, so
    /// it is not queued again; the person is told to use Retry.
    NotQueuedAgain,
}

/// One run of one queue item: which item, and which time round (#1432).
///
/// Every piece of work that runs for an item (the download itself, the
/// tidying up afterwards, the extra-format copies) carries one of these.
/// The queue only lets it change the item while the item's own attempt
/// number is still this one, so an old run can never finish, fail, restart
/// or take the slot of a later run of the same item. The number is unique
/// across the whole queue for as long as the app runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    /// The download ID of the item.
    pub id: String,
    /// Which run of that item this is.
    pub number: u64,
}

/// What [`DownloadQueue::next_pending`] hands out when it starts an item:
/// the attempt (which also owns the queue slot just taken) plus what is
/// needed to run it.
#[derive(Debug, Clone)]
pub(crate) struct StartedAttempt {
    /// The new attempt; its number holds the slot until it is released.
    pub attempt: Attempt,
    /// The links to download.
    pub urls: Vec<String>,
    /// The merged options for this run.
    pub options: GamdlOptions,
    /// The service the links belong to, when known.
    pub service: Option<String>,
}

/// How an attempt finds its item when it looks again after doing some work
/// (#1432). `None` from [`DownloadQueue::ended_outside`] means the item is
/// still this attempt's to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndedOutside {
    /// The person cancelled it.
    Cancelled,
    /// The stall check marked this attempt failed (and has already counted
    /// it, written History and released its slot).
    StoppedAsStuck,
    /// It was put back in the queue (retried, re-queued) since this attempt
    /// started; a later attempt owns it now.
    Replaced,
    /// It is no longer in the queue.
    Removed,
}

/// Gives out the next attempt number and moves the counter on (#1432).
/// A free function so it can be used while one queue item is borrowed.
fn take_attempt_number(next: &mut u64) -> u64 {
    let number = *next;
    *next += 1;
    number
}

/// Gives an item a fresh attempt number and forgets any stall mark (#1432).
/// Called whenever the item is put back in the queue or cancelled, so the
/// attempt that was running for it is retired at once.
fn retire_running_attempt(
    item: &mut QueueItem,
    next: &mut u64,
    live_stalled: &mut Vec<LiveStalledRun>,
) {
    // An attempt marked stuck may still be running. Once it is retired (for
    // any reason) it no longer counts as work the queue must wait for, so
    // its "still running" entry ends here too (#1432 review).
    if let Some(n) = item.stalled_attempt {
        live_stalled.retain(|r| r.number != n);
    }
    item.attempt = take_attempt_number(next);
    item.stalled_attempt = None;
    item.stalled_run_saved_files = false;
}

/// Finds the item `attempt` may still change (#1432): the same item, still
/// on this attempt's number, and not marked failed by the stall check for
/// this attempt. Anything else -- a later attempt, a cancel, a removed
/// item, or this attempt after the stall check -- gives `None`, so the
/// caller changes nothing.
fn find_owned<'a>(
    items: &'a mut VecDeque<QueueItem>,
    attempt: &Attempt,
) -> Option<&'a mut QueueItem> {
    items.iter_mut().find(|i| {
        i.status.id == attempt.id
            && i.attempt == attempt.number
            && i.stalled_attempt != Some(attempt.number)
    })
}

/// Resets an item for a fresh run from the start (#1432): options merged
/// afresh from the original request and the current settings, every retry
/// budget refilled, the status cleared, the item back to Queued, and a new
/// attempt number so the run that went before can no longer change it.
///
/// Shared by the person's Retry and by the automatic re-queue after a run
/// that the stall check had marked failed finishes after all. It does not
/// narrow the links (that is the person's smart retry, done by the caller).
fn reset_for_fresh_run(
    item: &mut QueueItem,
    settings: &AppSettings,
    max_network_retries: u32,
    next_attempt: &mut u64,
    live_stalled: &mut Vec<LiveStalledRun>,
) {
    // Re-merge options from the original request with current settings.
    // This picks up any settings changes the user made since the original
    // attempt. The service is worked out from the link again rather than
    // read back from the stored text (#829). Fresh options: nothing
    // MeedyaDL added is left in them.
    item.added_template_suffixes.clear();
    item.merged_options = merge_options(
        item.request.options.as_ref(),
        settings,
        item.request
            .urls
            .first()
            .and_then(|u| crate::models::media_service::MediaServiceId::from_url(u)),
    );
    // Reset fallback and retry counters to their initial values.
    item.fallback_index = 0;
    item.network_retries_left = max_network_retries;
    // A fresh run may try the storefront rewrite once again (#666).
    item.storefront_fallback_attempted = false;
    // Forget the music-video count (#776) so the next run's enrichment
    // finds it again rather than sizing its deadline from a stale one.
    item.status.mv_companion_count = None;
    // Reset status fields for a fresh start.
    item.status.state = DownloadState::Queued;
    item.status.error = None;
    item.status.progress = 0.0;
    item.status.fallback_occurred = false;
    item.status.used_wrapper = item.merged_options.use_wrapper.unwrap_or(false);
    item.status.codec_used = Some(item.merged_options.song_codec.as_ref().map_or_else(
        || settings.default_song_codec.to_cli_string().to_string(),
        |c| c.to_cli_string().to_string(),
    ));
    retire_running_attempt(item, next_attempt, live_stalled);
}

/// Whether an item is part-way through a run (downloading or tidying up).
/// Captions and progress fractions are only changed while it is (#1432),
/// so a run that has been replaced or stopped cannot paint over a failed,
/// cancelled or finished row.
const fn is_running_state(state: &DownloadState) -> bool {
    matches!(
        state,
        DownloadState::Downloading | DownloadState::Processing
    )
}

/// The most file paths one queue item remembers from its main runs.
pub const MAX_REPORTED_PATHS: usize = 5000;

/// Adds `path` to a reported-path list unless it is empty, already there, or
/// the list is full ([`MAX_REPORTED_PATHS`]).
fn push_reported_path(list: &mut Vec<PathBuf>, path: &str) {
    let path = path.trim();
    if path.is_empty() || list.len() >= MAX_REPORTED_PATHS {
        return;
    }
    let candidate = PathBuf::from(path);
    if !list.contains(&candidate) {
        list.push(candidate);
    }
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
    /// The detected media service (e.g., "apple-music"). Added in multi-service
    /// architecture; older persisted items will have `None` (backwards compat).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
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
/// Summary returned by [`DownloadQueue::abort_all`] describing how many
/// items were stopped, grouped by their pre-abort state. Used by the
/// `abort_all_downloads` IPC to surface a meaningful toast / activity-log
/// line without re-iterating the queue on the frontend (#620).
///
/// Items already in a terminal state (`Complete`, `Cancelled`, `Error`)
/// are NOT counted — they were not stopped by this action.
#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbortSummary {
    /// Items that were `Queued` (not yet started) and are now `Cancelled`.
    pub queued_cancelled: u32,
    /// Items that were actively `Downloading` when the abort fired. The
    /// download task will reap its subprocess on the next cancellation
    /// poll and transition out of the main loop.
    pub downloading_stopped: u32,
    /// Items that were in post-download `Processing` (enrichment,
    /// companion downloads, etc.) when the abort fired.
    pub processing_stopped: u32,
    /// Runs already marked stuck (failed by the stall check but still going)
    /// that this abort stopped. Not counted in `total()` and not sent to the
    /// frontend; used only for the activity-log line.
    #[serde(skip)]
    pub stuck_runs_stopped: u32,
}

impl AbortSummary {
    /// Total number of items affected (sum of all three fields).
    #[must_use]
    pub fn total(&self) -> u32 {
        self.queued_cancelled + self.downloading_stopped + self.processing_stopped
    }
}

/// Only `max_concurrent` downloads run simultaneously. When a download
/// finishes, the queue automatically starts the next queued item.
/// What the connection watcher needs to know about the queue (#1156).
///
/// Taken in one look under the lock, so every field describes the same moment.
#[derive(Debug, Clone)]
pub struct QueueWatchSnapshot {
    /// How many downloads are waiting to start.
    pub queued: usize,
    /// Whether anything is running right now.
    pub active: bool,
    /// Whether the user has paused the queue.
    pub paused: bool,
    /// The link of the first waiting download, used to decide which service
    /// needs to be reachable before starting. `None` when nothing is waiting.
    pub first_queued_url: Option<String>,
}

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
    /// The attempts that hold a queue slot right now (#1432). Each slot
    /// belongs to one attempt number: `next_pending()` adds it and
    /// `release_slot()` removes exactly that number, so no run can ever free
    /// another run's slot. Never longer than `max_concurrent`.
    active: Vec<u64>,
    /// The next attempt number to give out (#1432). Starts at 1 and only
    /// goes up, so a number is never reused while the app runs. Not saved:
    /// no running attempt survives a restart.
    next_attempt: u64,
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
    /// One-shot flag set by [`Self::abort_all`] (#620) and consumed by
    /// [`Self::take_recently_aborted`] when the queue would otherwise fire
    /// its post-queue action. Abort is a user-directed terminal action; the
    /// user did not want their system to shut down / play a sound / open a
    /// folder just because they stopped the queue. The flag is automatically
    /// cleared on consumption so the next legitimate queue-drain still runs
    /// the configured post-queue action.
    recently_aborted: bool,

    /// Whether any download has actually started since the last time the queue
    /// went quiet (#1169).
    ///
    /// Guards the after-queue action, which is meant to run when the queue
    /// *drains* — and which the user can set to shut down or restart their
    /// computer. Without this it also ran when the queue was never busy in the
    /// first place, so pressing Start Queue on an empty queue could shut the
    /// machine down.
    ///
    /// Set when an item is handed out to be downloaded, and read-and-cleared
    /// by [`Self::take_ran_since_drain`], the same shape as
    /// [`Self::recently_aborted`] above.
    ran_since_drain: bool,
    /// **Non-destructive pause flag** (#889). When `true`, [`Self::next_pending`]
    /// returns `None` even if there are items in `Queued` state and a slot
    /// is free — effectively freezing the scheduler. Currently
    /// `Downloading` / `Processing` items continue to completion and never
    /// see this flag; only fresh-item scheduling is gated. Distinct from
    /// [`Self::abort_all`] (which is destructive) and from the
    /// `auto_start_queue` setting (which controls whether newly-added
    /// items kick off processing).
    paused: bool,
    /// Attempt numbers whose run the stall check marked failed but which
    /// have not ended yet (#1432 review). The stall check leaves such a run
    /// going, and it may still queue its one retry when it ends, so while
    /// any is here the queue is not "idle": the "Queue finished" message and
    /// the after-queue action (which can shut the computer down) wait. A run
    /// is taken off when it ends ([`DownloadQueue::on_late_finish`], or its
    /// slot guard if its task ends some other way); the queue is then
    /// started again, so the action fires once, properly.
    ///
    /// The wait is not for ever (maintainer decision, 17 Nov 2026): once a
    /// run has been marked stuck for [`STALLED_RUN_WAIT`], it no longer
    /// holds the queue back, so an unattended overnight queue still ends
    /// (and still shuts the computer down, if that was asked for).
    live_stalled_runs: Vec<LiveStalledRun>,
}

/// How long a run the stall check marked failed, but which is still going,
/// may hold back "Queue finished" and the after-queue action (#1432,
/// maintainer decision 17 Nov 2026): one hour from when it was marked stuck.
///
/// Long enough for a slow run that is really still working to end and
/// queue its one retry inside the same queue run; short enough that a run
/// that hangs for good cannot stop an overnight queue from ever finishing
/// (and so stop a chosen shut-down or restart from ever happening). After
/// the hour the queue finishes as normal, with one activity-log line
/// naming the download that was still running.
pub(crate) const STALLED_RUN_WAIT: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// One run the stall check marked failed that has not ended yet (#1432).
#[derive(Debug, Clone)]
pub(crate) struct LiveStalledRun {
    /// The attempt number of the run.
    number: u64,
    /// The item it belongs to, to name it in the activity log.
    id: String,
    /// When the stall check marked it failed.
    since: std::time::Instant,
    /// The watchdog has already started the queue once because this run's
    /// wait ran out, so it does not do it again every tick.
    nudged: bool,
    /// A "Queue finished" summary (or an abort) has already gone out while
    /// this run was still going, so its stall failure was in that summary.
    /// Its later result must not change the next queue run's counts.
    summarised: bool,
}

/// Has a stalled run's wait of [`STALLED_RUN_WAIT`] run out at `now`, for a
/// run marked stuck at `since`? Pure, so the hour is tested without waiting.
#[must_use]
pub(crate) fn stalled_wait_over(since: std::time::Instant, now: std::time::Instant) -> bool {
    now.saturating_duration_since(since) >= STALLED_RUN_WAIT
}

/// What happens when a Spotify run the stall check marked failed comes to
/// an end (#1432, maintainer decision 17 Nov 2026). Decided under the lock
/// by [`DownloadQueue::finish_stalled_spotify_run`]. Spotify items are never
/// retried automatically after a stall: there is no tidying-up to redo, so
/// a run that really finished simply makes the item complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpotifyLateFinish {
    /// Nothing changes: the run did not finish cleanly with new files, or
    /// the item is no longer this run's (retried or removed). It stays
    /// failed.
    Nothing,
    /// The run finished after all; the item is now Complete. `summarised`:
    /// a "Queue finished" summary already counted it as failed, so it is
    /// not counted again.
    Completed {
        /// See [`LiveStalledRun::summarised`].
        summarised: bool,
    },
}

/// Does a Spotify run that the stall check marked failed count as
/// "finished after all" (#1432)? Only when votify ended cleanly AND at
/// least one new audio file landed. Pure, for tests.
#[must_use]
pub(crate) const fn spotify_late_run_finished(clean_exit: bool, new_files: usize) -> bool {
    clean_exit && new_files > 0
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

/// A guard that makes sure the queue slot one attempt holds is given back,
/// however that attempt's work ends (#706, #1432).
///
/// Rust runs a value's clean-up code (its `Drop`) the moment the value goes
/// out of scope -- on a normal return, an early return or a panic. One of
/// these guards is made the moment `next_pending()` hands out a slot, and it
/// travels with the attempt's work (the download task, then the tidying-up
/// task, or the wait for a failed item's extra-format copies). Every normal
/// way out releases the slot by hand first; the guard is the safety net for
/// the ways nobody coded for, so a panic can never leave the queue stuck.
///
/// **Why it is keyed by attempt (#1432).** It used to release "a" slot, so a
/// guard dropped late -- after its attempt had already given the slot back
/// and the next item had started -- freed the NEXT item's slot and let two
/// downloads run at once. Now it releases only its own attempt's slot, which
/// does nothing if that slot has already been given back. Dropping it twice,
/// or after a hand release, is therefore harmless.
///
/// When it really does free a slot (nobody released it by hand), it also
/// starts the queue again if it was given the app handle, so the next item
/// is not left waiting for something else to start the queue.
pub(crate) struct ActiveSlotGuard {
    /// `Some` while armed, `None` once `disarm` has run. `Drop`
    /// inspects this to decide whether to fire its release task.
    queue: Option<QueueHandle>,
    /// The attempt whose slot this guard gives back.
    attempt: Attempt,
    /// Used to start the queue again after a release by the guard itself.
    /// `None` in tests, which have no app.
    app: Option<AppHandle>,
}

impl ActiveSlotGuard {
    /// Makes an armed guard for `attempt`'s slot (no queue restart).
    pub(crate) fn new(queue: QueueHandle, attempt: Attempt) -> Self {
        Self {
            queue: Some(queue),
            attempt,
            app: None,
        }
    }

    /// The same guard, but one that starts the queue again if it is the
    /// guard that frees the slot.
    pub(crate) fn with_app(mut self, app: AppHandle) -> Self {
        self.app = Some(app);
        self
    }

    /// Disarms the guard after the slot was released by hand, so `Drop`
    /// does not even start its (harmless) release task.
    pub(crate) fn disarm(mut self) {
        self.queue = None;
    }
}

impl Drop for ActiveSlotGuard {
    /// Gives back this attempt's slot if it is still held, and if it was,
    /// starts the queue again. `Drop` cannot wait, so this runs as a small
    /// background task; with no runtime (the app shutting down) there is
    /// nothing left to keep in step, so it does nothing.
    fn drop(&mut self) {
        let Some(queue) = self.queue.take() else {
            return; // disarmed after a release by hand
        };
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let attempt = self.attempt.clone();
        let app = self.app.take();
        runtime.spawn(async move {
            let (freed, stalled_run_ended) = {
                let mut q = queue.lock().await;
                // A run the stall check marked failed whose task ends some
                // way other than through `finish_ended_outside` (a panic, an
                // early return) has ended all the same, so it no longer
                // holds back the queue's "finished" (#1432 review).
                (
                    q.release_slot(&attempt),
                    q.end_live_stalled_run(attempt.number),
                )
            };
            if freed || stalled_run_ended {
                log::debug!(
                    "Queue slot of download {} (attempt {}) given back by its guard",
                    attempt.id,
                    attempt.number
                );
                if let Some(app) = app {
                    process_queue(app, queue).await;
                }
            }
        });
    }
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
            active: Vec::new(),
            next_attempt: 1,
            max_network_retries: 3,
            gamdl_version: None,
            last_preflight_at: None,
            recently_aborted: false,
            ran_since_drain: false,
            paused: false,
            live_stalled_runs: Vec::new(),
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
    pub fn enqueue(&mut self, mut request: DownloadRequest, settings: &AppSettings) -> String {
        self.remove_terminal_duplicates_for_urls(&request.urls);

        let mut seen_urls = HashSet::new();
        request
            .urls
            .retain(|url| seen_urls.insert(normalize_url_for_dedup(url)));

        // Generate a unique download ID using UUID v4.
        // This ID is used to track the download across the queue, events, and frontend.
        let download_id = uuid::Uuid::new_v4().to_string();

        // Merge per-download overrides (from the frontend's "custom options" UI)
        // with global settings to produce the final set of GAMDL options.
        // For example, a user might override the codec for a specific download
        // while keeping the global output path from settings.
        // Detect which media service this URL belongs to (Apple Music, Spotify, etc.)
        // and resolve the primary download engine from the engine registry.
        //
        // Worked out BEFORE merging the options, because the naming patterns
        // can contain {platform} and it has to become this download's actual
        // service (#829). It used to be worked out just below, which is why
        // {platform} was hard-coded to Apple Music.
        let first_url = request.urls.first().map(String::as_str).unwrap_or("");
        let detected_service = crate::models::media_service::MediaServiceId::from_url(first_url);
        let service_str = detected_service
            .as_ref()
            .map(std::string::ToString::to_string);

        // Merge per-download overrides (from the frontend's "custom options" UI)
        // with global settings to produce the final set of GAMDL options.
        let merged_options = merge_options(request.options.as_ref(), settings, detected_service);

        // Resolve the primary engine for this service via the engine registry
        let engine_str = detected_service.as_ref().and_then(|svc| {
            let registry = crate::services::engine_registry::EngineRegistry::load();
            let platform_id = svc.to_string();
            registry.resolve_engine(&platform_id).map(|e| e.id.clone())
        });

        let item = QueueItem {
            status: {
                // Extract album name and artist from URL at enqueue time for
                // immediate display in the progress bar and queue list.
                let (album_name, artist_name) = extract_album_info_from_url(
                    request.urls.first().map(String::as_str).unwrap_or(""),
                );

                QueueItemStatus {
                    id: download_id.clone(),
                    urls: request.urls.clone(),
                    service: service_str,
                    engine: engine_str,
                    state: DownloadState::Queued,
                    progress: 0.0,
                    current_track: None,
                    album_name,
                    artist_name,
                    artwork_url: None,
                    total_tracks: None,
                    completed_tracks: None,
                    speed: None,
                    eta: None,
                    processing_label: None,
                    processing_progress: None,
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
                    audio_traits: Vec::new(),
                    mv_companion_count: None,
                    created_at: crate::utils::time_compat::now_rfc3339(),
                }
            },
            request,
            merged_options,
            fallback_index: 0,
            network_retries_left: self.max_network_retries,
            engine_fallback_index: 0,
            storefront_fallback_attempted: false,
            added_template_suffixes: Vec::new(),
            reported_paths: Vec::new(),
            // A fresh attempt number for a new item (#1432).
            attempt: take_attempt_number(&mut self.next_attempt),
            stalled_attempt: None,
            stalled_run_saved_files: false,
            auto_retried_after_stall: false,
            stall_failure_to_withdraw: false,
        };

        log::info!(
            "Enqueued download {} ({} URL(s))",
            download_id,
            item.status.urls.len()
        );

        self.items.push_back(item);
        download_id
    }

    /// Removes terminal queue rows that match the incoming URL set.
    ///
    /// Retried history entries are requeued as fresh items, so any older
    /// failed/cancelled/completed row for the same link should disappear
    /// from Queue instead of accumulating as a duplicate.
    fn remove_terminal_duplicates_for_urls(&mut self, urls: &[String]) -> usize {
        if urls.is_empty() {
            return 0;
        }

        let incoming: HashSet<String> = urls.iter().map(|u| normalize_url_for_dedup(u)).collect();
        let original_len = self.items.len();
        self.items.retain(|item| {
            let is_terminal = matches!(
                item.status.state,
                DownloadState::Complete | DownloadState::Error | DownloadState::Cancelled
            );
            let matches_incoming = item
                .status
                .urls
                .iter()
                .any(|u| incoming.contains(&normalize_url_for_dedup(u)));
            !(is_terminal && matches_incoming)
        });

        original_len - self.items.len()
    }

    /// Checks whether any of the given URLs already exist in the queue in an
    /// active or pending state (Queued, Downloading, or Processing).
    ///
    /// Returns `true` if at least one URL matches an existing queue item.
    /// Completed, cancelled, and errored items are ignored since those are
    /// effectively inert and cleared on restart.
    ///
    /// URL comparison uses [`normalize_url_for_dedup`] for case-insensitive,
    /// trailing-slash-insensitive, and query-parameter-stripped matching.
    #[must_use]
    pub fn has_duplicate_urls(&self, urls: &[String]) -> bool {
        // Build a set of normalised incoming URLs for O(n+m) comparison.
        let incoming: HashSet<String> = urls.iter().map(|u| normalize_url_for_dedup(u)).collect();

        self.items.iter().any(|item| {
            // Only check active/pending items — terminal states are irrelevant.
            matches!(
                item.status.state,
                DownloadState::Queued | DownloadState::Downloading | DownloadState::Processing
            ) && item
                .status
                .urls
                .iter()
                .any(|u| incoming.contains(&normalize_url_for_dedup(u)))
        })
    }

    /// Returns only URLs that are not already in a queued/active item.
    #[must_use]
    pub fn filter_out_active_duplicate_urls(&self, urls: &[String]) -> Vec<String> {
        let active_urls: HashSet<String> = self
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item.status.state,
                    DownloadState::Queued | DownloadState::Downloading | DownloadState::Processing
                )
            })
            .flat_map(|item| item.status.urls.iter())
            .map(|url| normalize_url_for_dedup(url))
            .collect();

        let mut seen_in_request = HashSet::new();
        urls.iter()
            .filter(|url| {
                let normalized = normalize_url_for_dedup(url);
                seen_in_request.insert(normalized.clone()) && !active_urls.contains(&normalized)
            })
            .cloned()
            .collect()
    }

    /// Returns the public status of all queue items for display in the frontend.
    /// The frontend calls this (via a Tauri command) to render the queue list.
    /// Returns cloned statuses to avoid holding the lock during serialization.
    #[must_use]
    pub fn get_status(&self) -> Vec<QueueItemStatus> {
        self.items.iter().map(|item| item.status.clone()).collect()
    }

    /// Returns the formatted media label
    /// (`Artist — Album — Track`, with URL fallback) for a given
    /// download ID, or `None` if no item with that ID exists or the
    /// label is empty.
    ///
    /// Used by [`crate::utils::activity_log::emit_download_log`] to
    /// auto-enrich every `[MeedyaDL]` activity-log line so users don't
    /// have to cross-reference the 8-char download ID against the queue
    /// page to know which item a message refers to.
    #[must_use]
    pub fn media_label_for(&self, download_id: &str) -> Option<String> {
        let item = self.items.iter().find(|i| i.status.id == download_id)?;
        let label = format_content_label(&item.status);
        if label.is_empty() {
            None
        } else {
            Some(label)
        }
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
        // An item the stall check marked failed whose run is still going
        // (#1432 review): the person chose to stop it, so retire its attempt
        // (the run stops and can neither queue its automatic retry nor
        // complete the item) and stop it holding the queue back. The item
        // stays failed, as the stall check left it.
        if let Some(number) = self.live_stuck_run_of(download_id) {
            if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
                retire_running_attempt(item, &mut self.next_attempt, &mut self.live_stalled_runs);
            }
            self.end_live_stalled_run(number);
            log::info!("Download {download_id}: stopped its run that was marked as stuck");
            return true;
        }
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            match item.status.state {
                DownloadState::Queued => {
                    item.status.state = DownloadState::Cancelled;
                    // A new attempt number, so nothing from the cancelled
                    // run can act on the item any more (#1432).
                    retire_running_attempt(
                        item,
                        &mut self.next_attempt,
                        &mut self.live_stalled_runs,
                    );
                    // #895: evict counter on terminal transition.
                    crate::utils::activity_log::evict_activity_counter(download_id);
                    log::info!("Download {download_id} cancelled (was queued)");
                    true
                }
                DownloadState::Downloading | DownloadState::Processing => {
                    item.status.state = DownloadState::Cancelled;
                    // Retire the running attempt at once (#1432): its run
                    // sees the number change within a quarter of a second,
                    // stops, and gives back its own slot (only its own).
                    retire_running_attempt(
                        item,
                        &mut self.next_attempt,
                        &mut self.live_stalled_runs,
                    );
                    // #895: evict counter on terminal transition.
                    crate::utils::activity_log::evict_activity_counter(download_id);
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

    /// The attempt number of `download_id`'s run that the stall check marked
    /// failed but which is still going (#1432 review), if there is one.
    fn live_stuck_run_of(&self, download_id: &str) -> Option<u64> {
        let item = self.items.iter().find(|i| i.status.id == download_id)?;
        if item.status.state != DownloadState::Error {
            return None;
        }
        item.stalled_attempt
            .filter(|n| self.live_stalled_runs.iter().any(|r| r.number == *n))
    }

    /// Removes a single item from the queue by ID.
    ///
    /// Refuses to remove `Downloading` / `Processing` items — the user must
    /// `cancel()` them first. Without this guard, deleting a live row would
    /// orphan the GAMDL subprocess (which writes to disk and would emit
    /// progress events with no queue entry to update), and the cancellation
    /// poll loop would have nothing to find on its next tick.
    ///
    /// # Returns
    /// - `Ok(true)` if the item was found and removed.
    /// - `Ok(false)` if the item was not found (already removed by a prior
    ///   call, or the ID was wrong).
    /// - `Err(message)` if the item exists but is in an active state.
    pub fn delete(&mut self, download_id: &str) -> Result<bool, String> {
        let Some(idx) = self.items.iter().position(|i| i.status.id == download_id) else {
            return Ok(false);
        };

        match self.items[idx].status.state {
            DownloadState::Downloading | DownloadState::Processing => Err(format!(
                "Cannot delete download {download_id} — currently active. \
                 Cancel it first."
            )),
            _ => {
                // A run marked stuck may still be going; the removed item can
                // no longer own it, so stop it holding the queue back.
                if let Some(n) = self.items[idx].stalled_attempt {
                    self.end_live_stalled_run(n);
                }
                self.items.remove(idx);
                log::info!("Deleted download {download_id} from queue");
                Ok(true)
            }
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

    /// Removes ALL non-active items from the queue (completed, cancelled,
    /// errored, and queued). Active downloads (Downloading/Processing) are
    /// kept to avoid interrupting in-progress work.
    ///
    /// # Returns
    /// Number of items removed.
    pub fn clear_all(&mut self) -> usize {
        let before = self.items.len();
        // Runs marked stuck that belong to items about to be removed no
        // longer hold the queue back.
        let gone: Vec<u64> = self
            .items
            .iter()
            .filter(|i| {
                !matches!(
                    i.status.state,
                    DownloadState::Downloading | DownloadState::Processing
                )
            })
            .filter_map(|i| i.stalled_attempt)
            .collect();
        for n in gone {
            self.end_live_stalled_run(n);
        }
        self.items.retain(|item| {
            matches!(
                item.status.state,
                DownloadState::Downloading | DownloadState::Processing
            )
        });
        let removed = before - self.items.len();
        if removed > 0 {
            log::info!("Cleared all {removed} items from queue (active downloads preserved)");
        }
        removed
    }

    /// Aborts every non-terminal item in the queue and returns a summary of
    /// what was stopped.
    ///
    /// Each matching item is transitioned directly to `DownloadState::Cancelled`
    /// in the same way the per-item `cancel()` does.
    ///
    /// What that actually stops, and when:
    ///
    /// * An item still on its FIRST download stops within about a
    ///   quarter of a second — that loop checks for cancellation on
    ///   every tick and the program is stopped when the task is dropped.
    /// * An item already making its extra copies in other formats stops
    ///   at the boundary between one format and the next, which is where
    ///   the other stop conditions are checked too. A format already
    ///   under way finishes first.
    /// * The pass that adds information to finished files is not
    ///   interrupted. Letting a file finish being written is right —
    ///   stopping half-way through leaves it partly done.
    ///
    ///   **But that pass is not only tagging, and this comment used to
    ///   imply it was.** It can also start whole music-video downloads
    ///   of its own, and then go looking for more. A reviewer pointed
    ///   out that describing it as a short pass over finished files was
    ///   the justification for leaving it alone — and the description
    ///   was wrong, so the justification did not hold. Cancelling can
    ///   therefore still leave real downloading under way, with the
    ///   queue's one slot held behind it. Recorded here rather than
    ///   quietly fixed, because interrupting that safely is a change to
    ///   the enrichment pass itself, not to this function.
    ///
    /// **This used to claim more than it did.** It said the companion
    /// and enrichment work would be short-circuited, and nothing in
    /// either of them asked about cancellation at all — so Cancel marked
    /// the item cancelled on screen while the work carried on, sometimes
    /// for many minutes, still holding the queue's one slot so the next
    /// item waited behind it. A full review of the codebase found the
    /// gap, and found this comment asserting it was already handled,
    /// which is why nobody had looked.
    ///
    /// Items already in `Complete`, `Cancelled`, or `Error` are untouched so
    /// the user keeps their history intact -- except that an `Error` item
    /// the stall check marked failed while its run is still going has that
    /// run stopped (#1432 review): its attempt is retired, so the run can
    /// neither queue its automatic retry nor complete the item, and it no
    /// longer holds the queue back. Such items stay `Error` and are not
    /// counted in the summary (which counts only items that were queued or
    /// running), but they do arm the after-queue suppression.
    ///
    /// The returned [`AbortSummary`] exposes per-pre-abort-state counts so
    /// the caller can produce a meaningful activity-log line + toast without
    /// having to iterate the queue themselves (#620).
    pub fn abort_all(&mut self) -> AbortSummary {
        let mut summary = AbortSummary::default();
        // #895: collect ids to evict, then call evict_activity_counter
        // OUTSIDE the borrow of `self.items` (the eviction function
        // doesn't touch the queue, so this is just cleanliness).
        let mut ids_to_evict: Vec<String> = Vec::new();
        // Attempt numbers of stuck runs still going that this abort stops.
        let mut stuck_runs: Vec<u64> = Vec::new();
        for item in &mut self.items {
            match item.status.state {
                DownloadState::Queued => {
                    item.status.state = DownloadState::Cancelled;
                    // Same as `cancel`: retire the attempt (#1432).
                    retire_running_attempt(
                        item,
                        &mut self.next_attempt,
                        &mut self.live_stalled_runs,
                    );
                    summary.queued_cancelled += 1;
                    ids_to_evict.push(item.status.id.clone());
                }
                DownloadState::Downloading => {
                    item.status.state = DownloadState::Cancelled;
                    retire_running_attempt(
                        item,
                        &mut self.next_attempt,
                        &mut self.live_stalled_runs,
                    );
                    summary.downloading_stopped += 1;
                    ids_to_evict.push(item.status.id.clone());
                }
                DownloadState::Processing => {
                    item.status.state = DownloadState::Cancelled;
                    retire_running_attempt(
                        item,
                        &mut self.next_attempt,
                        &mut self.live_stalled_runs,
                    );
                    summary.processing_stopped += 1;
                    ids_to_evict.push(item.status.id.clone());
                }
                DownloadState::Error
                    if item
                        .stalled_attempt
                        .is_some_and(|n| self.live_stalled_runs.iter().any(|r| r.number == n)) =>
                {
                    // Failed by the stall check, but its run is still going
                    // (#1432 review). Abort must reach it too: retire the
                    // attempt so the run stops and, if it ends anyway, can
                    // neither queue its automatic retry nor complete the
                    // item. The item stays failed (already counted and in
                    // History) and is not counted in the summary.
                    stuck_runs.extend(item.stalled_attempt);
                    retire_running_attempt(
                        item,
                        &mut self.next_attempt,
                        &mut self.live_stalled_runs,
                    );
                }
                DownloadState::Complete | DownloadState::Cancelled | DownloadState::Error => {
                    // Terminal — leave alone.
                }
            }
        }
        // The stopped stuck runs no longer hold the queue back, so it can
        // count as finished once their runs have gone.
        for number in &stuck_runs {
            self.end_live_stalled_run(*number);
        }
        summary.stuck_runs_stopped = stuck_runs.len() as u32;
        for id in &ids_to_evict {
            crate::utils::activity_log::evict_activity_counter(id);
        }
        if !stuck_runs.is_empty() {
            log::info!(
                "Abort: stopped {} run(s) already marked as stuck",
                stuck_runs.len()
            );
        }
        if summary.total() > 0 || !stuck_runs.is_empty() {
            log::info!(
                "Abort: cancelled {queued} queued, stopped {dl} downloading, stopped {proc} processing",
                queued = summary.queued_cancelled,
                dl = summary.downloading_stopped,
                proc = summary.processing_stopped,
            );
            // Arm the post-queue-action suppression flag. Abort is a
            // user-directed terminal action; suppress the configured
            // post-queue action (shutdown, hibernate, play sound,
            // etc.) that would otherwise fire when the queue drains.
            // Consumed once by `take_recently_aborted` on the next
            // would-be post-action trigger, so subsequent legitimate
            // drains still run the configured action.
            self.recently_aborted = true;
        }
        summary
    }

    /// Consumes the one-shot "recently aborted" flag set by
    /// [`Self::abort_all`]. Returns `true` and clears the flag if the
    /// queue was aborted since the last drain; returns `false`
    /// otherwise. The post-queue-action dispatch path calls this to
    /// decide whether to suppress the configured action (#620).
    /// Whether any download has run since the queue last went quiet, clearing
    /// the record as it answers.
    ///
    /// Used to decide whether the after-queue action should fire. It should
    /// run when the queue has finished doing something, not when it was asked
    /// to start with nothing to do (#1169).
    ///
    /// Cleared on read so one drain fires the action once.
    pub fn take_ran_since_drain(&mut self) -> bool {
        let ran = self.ran_since_drain;
        self.ran_since_drain = false;
        ran
    }

    pub fn take_recently_aborted(&mut self) -> bool {
        let was_aborted = self.recently_aborted;
        self.recently_aborted = false;
        was_aborted
    }

    /// Pauses the queue scheduler — **non-destructive** (#889).
    ///
    /// While paused, [`Self::next_pending`] refuses to start any new
    /// item. Currently `Downloading` / `Processing` items are
    /// untouched and run to completion (the pause flag is only
    /// checked at the *start-new-item* gate, not inside the per-item
    /// task). Resume with [`Self::resume`].
    ///
    /// Idempotent: calling `pause()` when already paused is a no-op
    /// and returns the previous state, so the caller can tell whether
    /// the call actually changed anything.
    ///
    /// # Returns
    /// `true` if the queue was already paused before this call,
    /// `false` if this call transitioned `running → paused`.
    pub fn pause(&mut self) -> bool {
        let was_paused = self.paused;
        self.paused = true;
        was_paused
    }

    /// Resumes the queue scheduler (#889). Items in `Queued` state
    /// become eligible to start on the next [`Self::next_pending`]
    /// call (i.e. the next `process_queue` iteration). The caller is
    /// expected to invoke `process_queue` after `resume` to kick the
    /// scheduler — `resume` itself does not start any item.
    ///
    /// Idempotent: calling `resume()` when already running is a no-op
    /// and returns the previous state.
    ///
    /// # Returns
    /// `true` if the queue was paused before this call (i.e. this
    /// call transitioned `paused → running`), `false` otherwise.
    pub fn resume(&mut self) -> bool {
        let was_paused = self.paused;
        self.paused = false;
        was_paused
    }

    /// Returns `true` when [`Self::pause`] has been called without a
    /// subsequent [`Self::resume`] (#889).
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// Sets an item's state directly. Tests only (#1432): the app itself
    /// changes a state only through the attempt-checked methods below.
    #[cfg(test)]
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
    ///
    /// Only the attempt that owns the item can change it (#1432): output
    /// from an earlier run of the same item, or from a run the stall check
    /// already marked failed, is ignored. A progress line also never moves
    /// an item out of Cancelled, Error or Complete -- before, a line landing
    /// just after Cancel turned the item back into "Downloading", and the
    /// run then never noticed it had been cancelled.
    pub(crate) fn update_item_progress(
        &mut self,
        attempt: &Attempt,
        event: &process::GamdlOutputEvent,
    ) {
        // A run the stall check marked failed may no longer change the item,
        // but whether it saved a file is still worth knowing: only a run that
        // saved files counts as "finished after all" (#1432 review).
        if let process::GamdlOutputEvent::Complete { .. } = event {
            if let Some(item) = self.items.iter_mut().find(|i| {
                i.status.id == attempt.id
                    && i.attempt == attempt.number
                    && i.stalled_attempt == Some(attempt.number)
            }) {
                item.stalled_run_saved_files = true;
                return;
            }
        }
        if let Some(item) = find_owned(&mut self.items, attempt) {
            let may_change_state = !matches!(
                item.status.state,
                DownloadState::Cancelled | DownloadState::Error | DownloadState::Complete
            );
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
                    if may_change_state {
                        item.status.state = DownloadState::Downloading;
                    }
                }
                process::GamdlOutputEvent::TrackInfo {
                    title,
                    artist,
                    track_number,
                    track_total,
                    ..
                } => {
                    // Format the current track as "Artist - Title" or just "Title"
                    let track_name = if artist.is_empty() {
                        title.clone()
                    } else {
                        format!("{artist} - {title}")
                    };
                    item.status.current_track = Some(track_name);
                    // Wire the parsed "[Track N/M]" counters through to
                    // `QueueItemStatus` so the UI can display "(Track N
                    // of M)" context on album / playlist / artist-bucket
                    // downloads. GAMDL v3.1 also emits `[Track 1/1]` for
                    // single-song URLs; the frontend suppresses the
                    // counter when `total_tracks == 1` (#609).
                    if let Some(n) = track_number {
                        item.status.completed_tracks = Some(*n as usize);
                    }
                    if let Some(t) = track_total {
                        item.status.total_tracks = Some(*t as usize);
                    }
                }
                process::GamdlOutputEvent::ProcessingStep { .. } => {
                    // Processing state covers post-download steps like remuxing,
                    // metadata tagging, and cover art embedding
                    if may_change_state {
                        item.status.state = DownloadState::Processing;
                    }
                }
                process::GamdlOutputEvent::Complete { path } => {
                    // Set the output file/directory path for the "Open" button in the UI
                    item.status.output_path = Some(path.clone());
                    item.status.progress = 100.0;
                    // This attempt has saved a file (#1432 review). Read only
                    // if the stall check later marks this attempt failed.
                    item.stalled_run_saved_files = true;
                    // Also remember every path, not just the last: a download
                    // can write to several album folders (#1446).
                    push_reported_path(&mut item.reported_paths, path);
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
                // Traceback frames from upstream Python noise (#660). They
                // do not represent a state transition — the actual exception
                // summary line is captured separately via the Error variant
                // (PYTHON_EXCEPTION_REGEX).
                process::GamdlOutputEvent::TracebackFrame { .. } => {}
                // Per-track codec-availability skips (#698) are normal
                // catalog behaviour, not download failures. Don't set
                // `item.status.error` from these — the queue's terminal
                // classifier inspects the collected `errors` Vec at exit
                // time and produces a meaningful "no audio available"
                // message when every recorded warning is a codec skip.
                // Setting `error` here would surface the misleading
                // `[WARNING] Skipping ...` text mid-download even if the
                // remaining tracks succeed and the item ends Complete.
                process::GamdlOutputEvent::CodecSkip { .. } => {}
            }
        }
    }

    /// Forgets the paths reported so far: called where a main download
    /// attempt starts. Not called before the gap-fill run, which belongs to
    /// the same download, so the two runs' lists add up.
    /// Does nothing for an attempt that no longer owns the item (#1432).
    pub(crate) fn begin_run(&mut self, attempt: &Attempt) {
        if let Some(item) = find_owned(&mut self.items, attempt) {
            item.reported_paths.clear();
        }
    }

    /// Marks a download as errored and sets the error message.
    ///
    /// Refuses to overwrite a `Cancelled` or `Complete` terminal state
    /// (#661). The cancellation path explicitly transitions an active
    /// item to `Cancelled` first, and a late-arriving error from a
    /// subprocess that was being torn down must not flip that to
    /// `Error`. Likewise, a `Complete` item should not regress to
    /// `Error` if a tail-end async task fails after enrichment ended.
    ///
    /// Returns whether the item has only NOW become failed (#1431): `false`
    /// when it is unknown, already finished or cancelled, or was ALREADY
    /// failed (the message is still updated). Callers announce and count a
    /// failure only on `true`, so one download is never announced twice --
    /// for instance after the stall watchdog failed it and the run then
    /// errored as well.
    ///
    /// Only the attempt that owns the item can fail it (#1432): an earlier
    /// run of a retried item, or a run the stall check already marked
    /// failed, gets `false` and changes nothing.
    pub(crate) fn set_error(&mut self, attempt: &Attempt, error: &str) -> bool {
        let download_id = attempt.id.as_str();
        if let Some(item) = find_owned(&mut self.items, attempt) {
            if matches!(
                item.status.state,
                DownloadState::Cancelled | DownloadState::Complete
            ) {
                log::debug!(
                    "set_error skipped for {} — already in terminal state {:?}",
                    download_id,
                    item.status.state,
                );
                return false;
            }
            let newly_failed = item.status.state != DownloadState::Error;
            item.status.state = DownloadState::Error;
            item.status.error = Some(error.to_string());
            // #895: evict the per-download activity counter so the
            // ACTIVITY_COUNTERS HashMap doesn't grow monotonically.
            crate::utils::activity_log::evict_activity_counter(download_id);
            return newly_failed;
        }
        false
    }

    /// Marks a download as in post-processing (enrichment, companions, etc.).
    /// The item stays in this state until all background tasks finish.
    ///
    /// Returns whether it did (#1432). Only the attempt that owns the item,
    /// and only from Downloading (or Processing, which changes nothing):
    /// before, this moved a failed, waiting or cancelled item to Processing,
    /// which is how an old run brought a retried item back to life.
    pub(crate) fn set_processing(&mut self, attempt: &Attempt) -> bool {
        match find_owned(&mut self.items, attempt) {
            Some(item) if is_running_state(&item.status.state) => {
                item.status.state = DownloadState::Processing;
                true
            }
            _ => false,
        }
    }

    /// Updates the processing label for a download item.
    /// Shows what's currently happening during Processing state.
    /// Only while the item is downloading or tidying up (#1432), so a late
    /// caption cannot appear on a failed, cancelled or finished row.
    pub fn set_processing_label(&mut self, download_id: &str, label: &str) {
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|i| i.status.id == download_id && is_running_state(&i.status.state))
        {
            item.status.processing_label = Some(label.to_string());
        }
    }

    /// Updates the intra-Processing progress fraction for a download
    /// item (#576). `progress` is clamped to `[0.0, 1.0]` before storing.
    ///
    /// Drives the queue-level progress bar's within-Processing forward
    /// motion. Called by each enrichment stage with its cumulative
    /// contribution (see `ENRICHMENT_STAGE_WEIGHTS`) so the user sees
    /// the aggregate bar advancing through the enrichment phase rather
    /// than sitting on a fixed "partial credit" value for 20+ minutes
    /// on large box sets.
    ///
    /// Only while the item is downloading or tidying up (#1432).
    pub fn set_processing_progress(&mut self, download_id: &str, progress: f32) {
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|i| i.status.id == download_id && is_running_state(&i.status.state))
        {
            item.status.processing_progress = Some(progress.clamp(0.0, 1.0));
        }
    }

    /// Clears the processing label for a download. Called by the
    /// companion supervisor when the post-processing phase finishes
    /// (#503), so the queue UI returns to its normal caption.
    pub fn clear_processing_label(&mut self, download_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            item.status.processing_label = None;
        }
    }

    /// [`Self::set_processing_label`] for one attempt only (#1432 review):
    /// does nothing once the item belongs to a later attempt, was
    /// cancelled, or was marked failed as stuck, so an old run's caption
    /// never lands on the row of the run that replaced it. Used where the
    /// attempt is in hand (the tidying-up and the extra-format copies).
    pub(crate) fn set_processing_label_for(&mut self, attempt: &Attempt, label: &str) {
        if self.ended_outside(attempt).is_none() {
            self.set_processing_label(&attempt.id, label);
        }
    }

    /// [`Self::set_processing_progress`] for one attempt only (#1432 review).
    pub(crate) fn set_processing_progress_for(&mut self, attempt: &Attempt, progress: f32) {
        if self.ended_outside(attempt).is_none() {
            self.set_processing_progress(&attempt.id, progress);
        }
    }

    /// [`Self::clear_processing_label`] for one attempt only (#1432
    /// review), so an old run cannot wipe the caption of the run that
    /// replaced it.
    pub(crate) fn clear_processing_label_for(&mut self, attempt: &Attempt) {
        if self.ended_outside(attempt).is_none() {
            self.clear_processing_label(&attempt.id);
        }
    }

    /// Marks a download as complete (#416).
    /// Clears processing label and speed/ETA to prevent stale data
    /// appearing in the UI after completion.
    ///
    /// Refuses to overwrite an `Error` or `Cancelled` terminal state
    /// (#661). The completion task at the bottom of the per-item
    /// pipeline always calls `set_complete` after the post-companion
    /// advisory pass, even if the download itself failed minutes
    /// earlier — without this guard, failed items would silently
    /// "revive" to Complete in the UI, contradicting both the in-app
    /// error toast and the prior activity-log error entry.
    ///
    /// Returns whether the item has only NOW become complete (#1431):
    /// `false` when it is unknown, failed, cancelled or already complete.
    /// Callers announce and count a finished download only on `true`.
    ///
    /// Only the attempt that owns the item can finish it (#1432): an
    /// earlier run of a retried item, or a run the stall check already
    /// marked failed, gets `false` and changes nothing.
    pub(crate) fn set_complete(&mut self, attempt: &Attempt) -> bool {
        let download_id = attempt.id.as_str();
        if let Some(item) = find_owned(&mut self.items, attempt) {
            if matches!(
                item.status.state,
                DownloadState::Error | DownloadState::Cancelled
            ) {
                log::debug!(
                    "set_complete skipped for {} — already in terminal state {:?}",
                    download_id,
                    item.status.state,
                );
                return false;
            }
            let newly_complete = item.status.state != DownloadState::Complete;
            item.status.state = DownloadState::Complete;
            item.status.progress = 100.0;
            item.status.processing_label = None;
            item.status.speed = None;
            item.status.eta = None;
            // #895: evict the per-download activity counter — same
            // rationale as in `set_error`. The counter is only
            // meaningful for IN-FLIGHT downloads.
            crate::utils::activity_log::evict_activity_counter(download_id);
            return newly_complete;
        }
        false
    }

    /// Appends non-fatal warnings to a download item. These are displayed
    /// in the queue UI as amber text below the URL, indicating that the
    /// download succeeded but encountered issues during the run.
    /// Only for the attempt that owns the item (#1432).
    pub(crate) fn add_warnings(&mut self, attempt: &Attempt, warnings: &[String]) {
        if let Some(item) = find_owned(&mut self.items, attempt) {
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
    /// `Some((new_options, fallback_index, chain_len))` if fallback should be
    /// attempted. `fallback_index` is 1-indexed (first fallback = 1, since
    /// index 0 is the initial codec). `chain_len` is the total length of the
    /// fallback chain. Returns `None` if all fallbacks are exhausted.
    ///
    /// Only the attempt that owns the item can re-queue it (#1432); when it
    /// does, the item gets a new attempt number, so this run (and anything
    /// left over from it) can no longer change it.
    pub(crate) fn try_fallback(
        &mut self,
        attempt: &Attempt,
        settings: &AppSettings,
    ) -> Option<(GamdlOptions, usize, usize)> {
        let download_id = attempt.id.as_str();
        let item = find_owned(&mut self.items, attempt)?;

        // Only attempt fallback if the user has enabled it in settings
        if !settings.fallback_enabled {
            return None;
        }

        // This function's chain is `settings.music_fallback_chain` — the
        // AUDIO codec chain. A music video must never fall into it. Without
        // this check, a music video that GAMDL could not download in any
        // of the allowed VIDEO codecs would be retried here with a
        // different AUDIO codec instead: the activity log would say
        // something like "trying atmos", but nothing about the video would
        // actually change, and the retry would download nothing — GAMDL
        // already tried every video codec it was allowed to use, by
        // itself, in the one run it already made (see the doc comment on
        // `VideoCodec`: GAMDL steps through the whole video codec chain in
        // a single process, so there is no separate "next video codec"
        // step for MeedyaDL to drive the way there is for audio).
        if urls_are_all_music_videos(&item.status.urls) {
            return None;
        }

        // Advance to the next codec in the fallback chain
        item.fallback_index += 1;

        if item.fallback_index < settings.music_fallback_chain.len() {
            // Get the next codec to try from the fallback chain
            let next_codec = &settings.music_fallback_chain[item.fallback_index];
            let mut new_options = item.merged_options.clone();
            // Clear song_codec — GAMDL >= 2.9.1 removed the --song-codec flag.
            // We use --song-codec-priority with a single codec instead.
            new_options.song_codec = None;

            // Override song_codec_priority with just this single codec.
            // This serves two purposes:
            // 1. Prevents process_download_item() from rebuilding the full
            //    priority chain (the is_none() check will be false).
            // 2. Overrides the config.ini song_codec_priority (which still
            //    has the full chain) so GAMDL tries only this one codec.
            // Using --song-codec-priority with one codec is equivalent to
            // --song-codec but also overrides the config.ini key.
            new_options.song_codec_priority = Some(next_codec.to_runtime_cli_string().to_string());

            // Remove exactly the suffix an earlier fallback added, so
            // suffixes never pile up (`{title} [Dolby Atmos] [Lossless]`) and
            // a format that needs none does not keep the previous one's. A
            // suffix the person typed into their own template stays.
            strip_recorded_codec_suffixes(&mut new_options, &item.added_template_suffixes);
            item.added_template_suffixes.clear();
            // If the companion mode would produce companions for this fallback
            // codec, apply the codec suffix to file templates so the specialist
            // format files don't collide with the companion files.
            if needs_primary_suffix(
                next_codec,
                &settings.companion_mode,
                &settings.custom_companion_codecs,
            ) {
                let added = codec_suffix_for_options(&new_options).filter(|s| !s.is_empty());
                if apply_codec_suffix(&mut new_options) {
                    item.added_template_suffixes.extend(added);
                }
            }

            // Update tracking info for the frontend to display
            item.status.codec_used = Some(next_codec.to_cli_string().to_string());
            item.status.fallback_occurred = true;
            // Reset the item to Queued so process_queue() will start it again,
            // under a new attempt number (#1432).
            item.status.state = DownloadState::Queued;
            retire_running_attempt(item, &mut self.next_attempt, &mut self.live_stalled_runs);
            item.status.error = None;
            item.status.progress = 0.0;
            item.merged_options = new_options.clone();

            let chain_len = settings.music_fallback_chain.len();
            log::info!(
                "Download {} falling back to codec: {} (fallback {} of {})",
                download_id,
                next_codec.to_cli_string(),
                item.fallback_index,
                chain_len.saturating_sub(1),
            );

            Some((new_options, item.fallback_index, chain_len))
        } else {
            // All codecs in the fallback chain have been tried and failed.
            // The download will remain in the Error state.
            log::info!("Download {download_id} exhausted all fallback codecs");
            None
        }
    }

    /// The per-format retry as one step (#1432 part 2): re-queues the item
    /// in the next format of the fallback chain and, only if it did, gives
    /// back this attempt's slot. When no format is left (or format fallback
    /// is off), nothing at all changes -- the item keeps its state and the
    /// attempt keeps its slot, so carrying on with the files that did land
    /// cannot let the next item start alongside it.
    pub(crate) fn per_format_retry(
        &mut self,
        attempt: &Attempt,
        settings: &AppSettings,
    ) -> Option<(GamdlOptions, usize, usize)> {
        let next = self.try_fallback(attempt, settings)?;
        self.release_slot(attempt);
        Some(next)
    }

    /// Attempts to fall back to the next engine in the platform's engine chain.
    ///
    /// When the primary engine fails with a **tool error** (binary missing,
    /// crash, unsupported format), the download queue can try the next engine
    /// in the platform's priority order (defined in engines.toml).
    ///
    /// **Network and auth errors skip engine fallback** — if the network is
    /// down or credentials are invalid, a different engine won't help.
    ///
    /// # Returns
    /// `Some((engine_id, fallback_index, chain_len))` if another engine is
    /// available. Returns `None` if all engines have been tried or the
    /// platform has no fallback engines.
    ///
    /// Only the attempt that owns the item can re-queue it (#1432).
    pub fn try_engine_fallback(&mut self, attempt: &Attempt) -> Option<(String, usize, usize)> {
        let download_id = attempt.id.as_str();
        let item = find_owned(&mut self.items, attempt)?;

        // Resolve the engine chain for this item's service
        let service_id = item.status.service.as_deref()?;
        let registry = crate::services::engine_registry::EngineRegistry::load();
        let chain = registry.resolve_engine_chain(service_id);

        if chain.len() <= 1 {
            // Single engine or unknown platform — no fallback possible
            return None;
        }

        // Advance to the next engine in the chain
        item.engine_fallback_index += 1;

        if item.engine_fallback_index < chain.len() {
            let next_engine = &chain[item.engine_fallback_index];
            let next_engine_id = next_engine.id.clone();

            // Update the queue item's engine field
            item.status.engine = Some(next_engine_id.clone());
            // Reset the item to Queued so process_queue() will start it again,
            // under a new attempt number (#1432).
            item.status.state = DownloadState::Queued;
            retire_running_attempt(item, &mut self.next_attempt, &mut self.live_stalled_runs);
            item.status.error = None;
            item.status.progress = 0.0;

            let chain_len = chain.len();
            log::info!(
                "Download {} engine fallback: trying {} (fallback {} of {})",
                download_id,
                next_engine_id,
                item.engine_fallback_index,
                chain_len.saturating_sub(1),
            );

            Some((next_engine_id, item.engine_fallback_index, chain_len))
        } else {
            log::info!("Download {download_id} exhausted all engine fallbacks");
            None
        }
    }

    /// Checks if a download should retry due to a network error.
    ///
    /// # Returns
    /// `Some((attempt, total))` — 1-indexed attempt number and total attempts
    /// (initial + retries). For example, with `max_network_retries = 3`,
    /// total = 4 and attempts are 2, 3, 4 (attempt 1 was the initial try).
    /// Returns `None` if retries are exhausted or the item doesn't exist.
    ///
    /// Only the attempt that owns the item can re-queue it (#1432); the
    /// re-queued item gets a new attempt number.
    pub(crate) fn try_network_retry(&mut self, attempt: &Attempt) -> Option<(u32, u32)> {
        let download_id = attempt.id.as_str();
        let max = self.max_network_retries;
        if let Some(item) = find_owned(&mut self.items, attempt) {
            if item.network_retries_left > 0 {
                item.network_retries_left -= 1;
                item.status.state = DownloadState::Queued;
                retire_running_attempt(item, &mut self.next_attempt, &mut self.live_stalled_runs);
                item.status.error = None;
                item.status.progress = 0.0;
                // Total attempts = initial try + max retries.
                // Attempt number = total - retries_left (after decrement).
                let total = max + 1;
                let attempt = total - item.network_retries_left;
                log::info!(
                    "Download {} network retry (attempt {} of {}, {} remaining)",
                    download_id,
                    attempt,
                    total,
                    item.network_retries_left
                );
                Some((attempt, total))
            } else {
                log::info!("Download {download_id} exhausted network retries");
                None
            }
        } else {
            None
        }
    }

    /// Attempts a one-shot storefront-fallback rewrite for a failed item (#666).
    ///
    /// Rewrites every URL on the item from its current storefront to the
    /// user's account-region storefront (settings.storefront, falling back
    /// to OS locale, then `"us"`), resets the item to `Queued`, and returns
    /// `Some((from, to))` describing the swap so the activity-log writer
    /// can surface it. Returns `None` and leaves the item untouched when:
    ///
    /// * the user disabled `storefront_fallback_on_failure`, or
    /// * the budget of one attempt has already been spent for this item
    ///   (`storefront_fallback_attempted == true`), or
    /// * none of the URLs match the standard `/<storefront>/<type>/…`
    ///   shape (e.g. `/library/…` URLs use a Music-User-Token endpoint
    ///   that doesn't accept a free storefront), or
    /// * the resolved fallback storefront is the same as the existing one
    ///   on every URL (nothing to rewrite — typically when the user is
    ///   already on the storefront the URL specifies).
    ///
    /// Budget is reset to fresh by [`Self::retry`], so a manual user retry
    /// from the UI gets to try the rewrite again.
    ///
    /// Only the attempt that owns the item can re-queue it (#1432); the
    /// re-queued item gets a new attempt number.
    pub(crate) fn try_storefront_fallback(
        &mut self,
        attempt: &Attempt,
        settings: &AppSettings,
    ) -> Option<(String, String)> {
        let download_id = attempt.id.as_str();
        if !settings.storefront_fallback_on_failure {
            return None;
        }
        let item = find_owned(&mut self.items, attempt)?;
        if item.storefront_fallback_attempted {
            return None;
        }

        // Resolve the user's account region. settings.storefront is the
        // canonical user-configurable value (auto-derived from locale, can
        // be overridden in Settings > General). Fallbacks: OS locale via
        // login_window_service::detect_storefront, then "us" as a last-
        // resort default that's always a valid Apple storefront.
        let target = if !settings.storefront.is_empty() {
            settings.storefront.to_ascii_lowercase()
        } else {
            super::login_window_service::detect_storefront().unwrap_or_else(|| "us".to_string())
        };

        // Capture the current URL storefront from the *first* URL that
        // parses cleanly; we rewrite every URL but only need one source
        // value for the activity-log line.
        let from_storefront = item
            .status
            .urls
            .iter()
            .find_map(|u| super::apple_music_api::parse_apple_music_url(u))
            .map(|p| p.storefront)?;

        // Skip if the URL storefront already matches the user region.
        // This is the "user pasted their own region's URL" case — there's
        // nothing useful to rewrite to, and the failure is genuine.
        if from_storefront == target {
            return None;
        }

        // Rewrite every URL on the item. The helper is a no-op for any URL
        // shape it can't safely rewrite, so /library/ URLs and other novel
        // forms in a multi-URL request pass through unchanged — the queue
        // item just doesn't benefit from the swap for those entries.
        let new_urls: Vec<String> = item
            .status
            .urls
            .iter()
            .map(|u| super::apple_music_api::rewrite_url_storefront(u, &target))
            .collect();

        // Sanity check: at least one URL must have actually changed,
        // otherwise we'd be retrying the exact same set with the same
        // failure expected.
        if new_urls == item.status.urls {
            return None;
        }

        item.status.urls = new_urls.clone();
        item.request.urls = new_urls;
        item.storefront_fallback_attempted = true;
        item.status.state = DownloadState::Queued;
        retire_running_attempt(item, &mut self.next_attempt, &mut self.live_stalled_runs);
        item.status.error = None;
        item.status.progress = 0.0;

        log::info!(
            "Storefront fallback for {download_id}: rewriting URLs '{from_storefront}' -> '{target}'"
        );
        Some((from_storefront, target))
    }

    // ============================================================
    // Queue reorder API (#782)
    // ============================================================
    //
    // Reordering operates only on `Queued` items and only relative to
    // other `Queued` items. Active items (`Downloading` / `Processing`)
    // and terminal items (`Complete` / `Error` / `Cancelled`) keep
    // their absolute positions in the `VecDeque`. The four move methods
    // share the same outline:
    //
    //   1. Find the target item's current absolute index.
    //   2. Refuse if the item isn't `Queued` (no preempting actives,
    //      no shuffling completed history).
    //   3. Compute the destination absolute index from the desired
    //      logical position (top / up / down / bottom of the Queued
    //      sub-sequence), refusing the move when it would be a no-op.
    //   4. Pop the item and re-insert it at the destination index.
    //
    // Race-safety: callers acquire the same `Mutex<DownloadQueue>` as
    // `next_pending`, so a move can never interleave with item
    // selection. The moved item is visible to the very next
    // `next_pending` call.
    //
    // All four return `true` when the queue mutated, `false` when the
    // call was a no-op (item not found, not Queued, or already at the
    // requested position) so the caller can short-circuit the
    // `queue-updated` event + disk write when nothing changed.

    /// Returns a human-friendly label for an item — Album name first,
    /// URL fallback, item-id last-resort. Used by the IPC layer to
    /// produce traceable activity-log entries for queue mutations
    /// (#782) without exposing the private `items` field.
    #[must_use]
    pub fn friendly_label(&self, download_id: &str) -> Option<String> {
        let item = self.items.iter().find(|i| i.status.id == download_id)?;
        Some(
            item.status
                .album_name
                .clone()
                .or_else(|| item.status.urls.first().cloned())
                .unwrap_or_else(|| download_id.to_string()),
        )
    }

    /// Helper: indices of every `Queued` item in deque order.
    /// Empty when no items are queued.
    fn queued_indices(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                if item.status.state == DownloadState::Queued {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Move a `Queued` item to the top of the pending sub-sequence —
    /// it becomes the next item `next_pending` will pick.
    pub fn move_to_top(&mut self, download_id: &str) -> bool {
        let queued = self.queued_indices();
        if queued.len() < 2 {
            return false; // 0 or 1 queued items — no-op
        }
        let first_queued_idx = queued[0];
        let Some(current_idx) = self.items.iter().position(|i| i.status.id == download_id) else {
            return false;
        };
        if self.items[current_idx].status.state != DownloadState::Queued {
            return false;
        }
        if current_idx == first_queued_idx {
            return false; // Already at top
        }
        // current_idx > first_queued_idx is guaranteed: the item is
        // Queued and not already at first_queued_idx, so it must come
        // later in the deque. Removing it shifts no earlier index, so
        // first_queued_idx remains the correct insert position.
        let removed = self.items.remove(current_idx).expect("index just verified");
        self.items.insert(first_queued_idx, removed);
        true
    }

    /// Move a `Queued` item to the bottom of the pending sub-sequence.
    pub fn move_to_bottom(&mut self, download_id: &str) -> bool {
        let queued = self.queued_indices();
        if queued.len() < 2 {
            return false;
        }
        let last_queued_idx = *queued.last().expect("checked len >= 2");
        let Some((current_idx, _)) = self
            .items
            .iter()
            .enumerate()
            .find(|(_, i)| i.status.id == download_id)
        else {
            return false;
        };
        if self.items[current_idx].status.state != DownloadState::Queued {
            return false;
        }
        if current_idx == last_queued_idx {
            return false; // Already at bottom
        }
        let removed = self.items.remove(current_idx).expect("index just verified");
        // After removal, indices > current_idx shift down by 1. We
        // want to insert at the OLD last_queued_idx position, which
        // becomes last_queued_idx - 1 after removal (because the
        // current_idx < last_queued_idx removal shifted it down).
        let insert_at = if current_idx < last_queued_idx {
            last_queued_idx - 1
        } else {
            // Defensive — would mean current_idx > last_queued_idx,
            // which contradicts queued.last() being the largest queued
            // index. Reject.
            return false;
        };
        self.items.insert(insert_at + 1, removed);
        true
    }

    /// Swap a `Queued` item with the `Queued` item immediately above
    /// it in the pending sub-sequence (skipping any intervening
    /// non-`Queued` items).
    pub fn move_up(&mut self, download_id: &str) -> bool {
        let queued = self.queued_indices();
        // Find the target's position within the queued sub-sequence.
        let Some(queued_pos) = queued.iter().position(|&idx| {
            self.items
                .get(idx)
                .is_some_and(|item| item.status.id == download_id)
        }) else {
            return false; // Not in queue or not Queued
        };
        if queued_pos == 0 {
            return false; // Already at top of queued sub-sequence
        }
        let above_idx = queued[queued_pos - 1];
        let target_idx = queued[queued_pos];
        self.items.swap(above_idx, target_idx);
        true
    }

    /// Swap a `Queued` item with the `Queued` item immediately below
    /// it in the pending sub-sequence.
    pub fn move_down(&mut self, download_id: &str) -> bool {
        let queued = self.queued_indices();
        let Some(queued_pos) = queued.iter().position(|&idx| {
            self.items
                .get(idx)
                .is_some_and(|item| item.status.id == download_id)
        }) else {
            return false;
        };
        if queued_pos + 1 >= queued.len() {
            return false; // Already at bottom of queued sub-sequence
        }
        let below_idx = queued[queued_pos + 1];
        let target_idx = queued[queued_pos];
        self.items.swap(target_idx, below_idx);
        true
    }

    /// Starts the next queued item, if a slot is free (the "scheduler").
    ///
    /// Returns `None` when the queue is paused, every slot is taken, or
    /// nothing is waiting. Otherwise the first waiting item moves to
    /// Downloading under a NEW attempt number, and that number takes a slot
    /// (#1432). The caller must make sure the slot is given back with
    /// [`Self::release_slot`] for that attempt -- an [`ActiveSlotGuard`]
    /// does it if nothing else does.
    pub(crate) fn next_pending(&mut self) -> Option<StartedAttempt> {
        // #889 — Non-destructive pause. When the user has explicitly
        // paused the queue, refuse to start any new item. Items
        // currently in `Downloading` / `Processing` state are
        // unaffected (their tasks already have their slot) — they
        // run to completion. Only the scheduler is gated. The user
        // resumes via `Self::resume()` (Tauri IPC `resume_queue`).
        if self.paused {
            return None;
        }

        // Check if we're at the concurrent download limit
        if self.active.len() >= self.max_concurrent {
            return None;
        }

        // Find the first Queued item (FIFO order from VecDeque front)
        let item = self
            .items
            .iter_mut()
            .find(|i| i.status.state == DownloadState::Queued)?;
        // Transition to Downloading under a new attempt number, which takes
        // the slot (#1432).
        item.status.state = DownloadState::Downloading;
        retire_running_attempt(item, &mut self.next_attempt, &mut self.live_stalled_runs);
        let attempt = Attempt {
            id: item.status.id.clone(),
            number: item.attempt,
        };
        self.active.push(attempt.number);
        // Record that the queue has genuinely done something, so the
        // after-queue action fires on a real drain and not merely because the
        // queue was asked to start with nothing in it (#1169).
        self.ran_since_drain = true;
        // Work handed out after an Abort owns the next drain: the abort record
        // belongs to the batch that was aborted, not to whatever starts next
        // (Undo on the abort message, or new links pasted before the aborted
        // run reached its stop check). Left set, it would hide the new
        // batch's "Queue finished" and after-queue action.
        self.recently_aborted = false;

        // Return the data needed to start the download, including the
        // detected service ID for service-aware routing (#318).
        Some(StartedAttempt {
            attempt,
            urls: item.status.urls.clone(),
            options: item.merged_options.clone(),
            service: item.status.service.clone(),
        })
    }

    /// Gives back the slot held by `attempt`, and only that slot (#1432).
    ///
    /// Returns whether a slot was actually freed. Calling it again for the
    /// same attempt, or for an attempt whose slot the stall check already
    /// gave back, does nothing -- so a late or repeated release can never
    /// free the slot of the item that started next. (The old version just
    /// took one off a counter, whoever was holding the slot.)
    pub(crate) fn release_slot(&mut self, attempt: &Attempt) -> bool {
        self.release_slot_number(attempt.number)
    }

    /// [`Self::release_slot`] by number alone, for the stall check's sweep
    /// of slots held by attempts that have since been replaced.
    fn release_slot_number(&mut self, number: u64) -> bool {
        if let Some(pos) = self.active.iter().position(|n| *n == number) {
            self.active.remove(pos);
            true
        } else {
            false
        }
    }

    /// How many slots are held right now.
    #[must_use]
    pub fn active_len(&self) -> usize {
        self.active.len()
    }

    /// Slots held by attempts that are no longer the current attempt of
    /// any item (#1432): the item was cancelled, retried or removed while
    /// the old run still held its slot. The old run normally gives the slot
    /// back within seconds; the stall check frees any that it never does.
    pub(crate) fn orphaned_slots(&self) -> Vec<u64> {
        self.active
            .iter()
            .copied()
            .filter(|n| !self.items.iter().any(|i| i.attempt == *n))
            .collect()
    }

    /// Frees a slot found by [`Self::orphaned_slots`], but only if it is
    /// still orphaned at this moment. Returns whether it freed one.
    pub(crate) fn release_orphaned_slot(&mut self, number: u64) -> bool {
        if self.items.iter().any(|i| i.attempt == number) {
            return false;
        }
        self.release_slot_number(number)
    }

    /// The attempt that may change `download_id` right now. Tests only; the
    /// app gets its attempts from `next_pending`.
    #[cfg(test)]
    pub(crate) fn attempt_of(&self, download_id: &str) -> Attempt {
        Attempt {
            id: download_id.to_string(),
            number: self
                .items
                .iter()
                .find(|i| i.status.id == download_id)
                .map_or(0, |i| i.attempt),
        }
    }

    /// Should the work of this attempt carry on (#1432)?
    ///
    /// `true` while the item is still on this attempt's number and has not
    /// been cancelled. `false` once it has been cancelled, put back in the
    /// queue (retried or re-queued), or removed -- the run must then stop
    /// (a running GAMDL is killed within a quarter of a second).
    ///
    /// Deliberately still `true` after the stall check marked this attempt
    /// failed: the stall check does not stop the run (maintainer decision,
    /// 17 Nov 2026), it only stops the run from changing the item.
    #[must_use]
    pub(crate) fn should_keep_running(&self, attempt: &Attempt) -> bool {
        self.items.iter().any(|i| {
            i.status.id == attempt.id
                && i.attempt == attempt.number
                && i.status.state != DownloadState::Cancelled
        })
    }

    /// Has the item ended from outside this attempt (#1432)? `None` while
    /// the attempt still owns it -- whatever its state, so an Error this
    /// attempt set itself is never mistaken for someone else's.
    #[must_use]
    pub(crate) fn ended_outside(&self, attempt: &Attempt) -> Option<EndedOutside> {
        let Some(item) = self.items.iter().find(|i| i.status.id == attempt.id) else {
            return Some(EndedOutside::Removed);
        };
        if item.attempt != attempt.number {
            return Some(if item.status.state == DownloadState::Cancelled {
                EndedOutside::Cancelled
            } else {
                EndedOutside::Replaced
            });
        }
        if item.stalled_attempt == Some(attempt.number) {
            return Some(EndedOutside::StoppedAsStuck);
        }
        if item.status.state == DownloadState::Cancelled {
            return Some(EndedOutside::Cancelled);
        }
        None
    }

    /// The stall check's one action (#1432): marks `attempt` failed and
    /// gives back its slot, in one step under the lock.
    ///
    /// Only if that attempt still owns the item and the item is still
    /// downloading or tidying up -- so it can never fail a later attempt of
    /// the same item, an item that finished just before, or one that was
    /// cancelled. Returns whether it did; only then is anything released.
    ///
    /// The run itself is left going (maintainer decision, 17 Nov 2026). It
    /// can no longer change the item; if it finishes after all, it queues
    /// the item again once ([`Self::retry_after_late_finish`]).
    pub(crate) fn fail_stalled_attempt(&mut self, attempt: &Attempt, error: &str) -> bool {
        let Some(item) = find_owned(&mut self.items, attempt) else {
            return false;
        };
        if !is_running_state(&item.status.state) {
            return false;
        }
        item.status.state = DownloadState::Error;
        item.status.error = Some(error.to_string());
        item.stalled_attempt = Some(attempt.number);
        // `stalled_run_saved_files` already says whether THIS attempt
        // reported a saved file before the stall (it is cleared whenever the
        // item gets a new attempt number, so an earlier attempt's files do
        // not count); the run's later reports are added as they come in.
        crate::utils::activity_log::evict_activity_counter(&attempt.id);
        self.release_slot(attempt);
        // The run is left going; until it ends (or STALLED_RUN_WAIT
        // passes), the queue is not idle.
        if !self
            .live_stalled_runs
            .iter()
            .any(|r| r.number == attempt.number)
        {
            self.live_stalled_runs.push(LiveStalledRun {
                number: attempt.number,
                id: attempt.id.clone(),
                since: std::time::Instant::now(),
                nudged: false,
                summarised: false,
            });
        }
        true
    }

    /// Has the run the stall check marked failed (this `attempt`) saved at
    /// least one file (#1432 review)? `false` for any other attempt.
    #[must_use]
    pub(crate) fn stalled_run_saved_files(&self, attempt: &Attempt) -> bool {
        self.items.iter().any(|i| {
            i.status.id == attempt.id
                && i.attempt == attempt.number
                && i.stalled_attempt == Some(attempt.number)
                && i.stalled_run_saved_files
        })
    }

    /// Takes a stalled run off the "still running" list (#1432 review).
    /// Returns whether it was there. Safe to call more than once.
    pub(crate) fn end_live_stalled_run(&mut self, number: u64) -> bool {
        self.take_live_stalled_run(number).is_some()
    }

    /// Takes a stalled run off the "still running" list and hands it back,
    /// so the caller can see whether a queue summary already counted it.
    fn take_live_stalled_run(&mut self, number: u64) -> Option<LiveStalledRun> {
        let pos = self
            .live_stalled_runs
            .iter()
            .position(|r| r.number == number)?;
        Some(self.live_stalled_runs.remove(pos))
    }

    /// The watchdog's check (#1432): has the one-hour wait of any stalled
    /// run that is still going run out at `now`, and the queue not been
    /// checked for it yet? Marks those runs so this is `true` once per run.
    /// When `true`, the caller checks whether the queue has finished, so
    /// that -- if nothing else is queued or running -- "Queue finished" and
    /// the after-queue action happen now. Nothing is started.
    pub(crate) fn stalled_waits_just_ran_out(&mut self, now: std::time::Instant) -> bool {
        let mut any = false;
        for run in &mut self.live_stalled_runs {
            if !run.nudged && stalled_wait_over(run.since, now) {
                run.nudged = true;
                any = true;
            }
        }
        any
    }

    /// Called when the queue finishes (or is aborted) while stalled runs
    /// are still going -- which is only possible once their hour has run
    /// out (#1432). Marks each as already counted in that summary, so a
    /// later result does not change the next run's counts, and returns the
    /// names of those not reported before, for one activity-log line each.
    pub(crate) fn summarise_live_stalled_runs(&mut self) -> Vec<String> {
        let mut names = Vec::new();
        for run in &mut self.live_stalled_runs {
            if run.summarised {
                continue;
            }
            run.summarised = true;
            let name = self
                .items
                .iter()
                .find(|i| i.status.id == run.id)
                .map_or_else(|| run.id.clone(), |i| format_content_label(&i.status));
            names.push(name);
        }
        names
    }

    /// Ends a Spotify run the stall check marked failed (#1432, maintainer
    /// decision 17 Nov 2026: no automatic retry for Spotify). One step
    /// under the lock: the run comes off the "still running" list, and if
    /// it `finished` ([`spotify_late_run_finished`]) while still the item's
    /// latest attempt and the item is still in the Error state the stall
    /// check set, the item becomes Complete. Otherwise it stays failed.
    pub(crate) fn finish_stalled_spotify_run(
        &mut self,
        attempt: &Attempt,
        finished: bool,
    ) -> SpotifyLateFinish {
        let summarised = self
            .take_live_stalled_run(attempt.number)
            .is_some_and(|r| r.summarised);
        if !finished {
            return SpotifyLateFinish::Nothing;
        }
        let Some(item) = self.items.iter_mut().find(|i| {
            i.status.id == attempt.id
                && i.attempt == attempt.number
                && i.stalled_attempt == Some(attempt.number)
                && i.status.state == DownloadState::Error
        }) else {
            return SpotifyLateFinish::Nothing;
        };
        item.status.state = DownloadState::Complete;
        item.status.error = None;
        item.status.progress = 100.0;
        item.status.processing_label = None;
        item.status.speed = None;
        item.status.eta = None;
        // No longer "stopped as stuck": it finished.
        item.stalled_attempt = None;
        item.stalled_run_saved_files = false;
        SpotifyLateFinish::Completed { summarised }
    }

    /// Is a run the stall check marked failed still going (#1432 review)?
    /// Tests only; the queue reads the list through `is_idle`.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn has_live_stalled_runs(&self) -> bool {
        !self.live_stalled_runs.is_empty()
    }

    /// What to do now that the run of `attempt` has ended, after the item had
    /// already ended from outside it (`ended`) (#1432 review). One step under
    /// the lock, so nothing can find the queue idle in between:
    ///
    /// * the run comes off the "stalled but still running" list, so the
    ///   queue can count as finished again once nothing else is running;
    /// * if the stall check had marked this very run failed and it
    ///   `finished` -- genuinely: it ended cleanly, saved files, and its
    ///   tidying-up was not cut short -- the item is queued again once
    ///   ([`Self::retry_after_late_finish`]); if it was already that one
    ///   automatic retry, it is not queued again.
    ///
    /// A run that timed out, was stopped, or saved nothing gives `Nothing`:
    /// no line, no retry, so an album that hangs cannot loop.
    pub(crate) fn on_late_finish(
        &mut self,
        attempt: &Attempt,
        ended: EndedOutside,
        finished: bool,
        settings: &AppSettings,
    ) -> LateFinish {
        let summarised = self
            .take_live_stalled_run(attempt.number)
            .is_some_and(|r| r.summarised);
        if ended != EndedOutside::StoppedAsStuck || !finished {
            return LateFinish::Nothing;
        }
        if self.retry_after_late_finish(attempt, settings) {
            if summarised {
                // A "Queue finished" summary already counted the stall's
                // failure; the retry belongs to the next queue run, so its
                // result must not take back a failure from that run.
                if let Some(item) = self.items.iter_mut().find(|i| i.status.id == attempt.id) {
                    item.stall_failure_to_withdraw = false;
                }
            }
            return LateFinish::QueuedAgain;
        }
        // Not queued: either the person has since retried or removed it
        // (then nothing to say), or it was already the automatic retry.
        let already_retried = self.items.iter().any(|i| {
            i.status.id == attempt.id
                && i.attempt == attempt.number
                && i.stalled_attempt == Some(attempt.number)
                && i.auto_retried_after_stall
        });
        if already_retried {
            LateFinish::NotQueuedAgain
        } else {
            LateFinish::Nothing
        }
    }

    /// Takes back, once, the failure the stall check counted for this item,
    /// when its automatic retry's own result is about to be counted (#1432
    /// review), so "Queue finished: N downloaded, M failed" counts the item
    /// once. Returns whether there was one to take back.
    pub(crate) fn take_stall_failure_to_withdraw(&mut self, attempt: &Attempt) -> bool {
        match self
            .items
            .iter_mut()
            .find(|i| i.status.id == attempt.id && i.attempt == attempt.number)
        {
            Some(item) if item.stall_failure_to_withdraw => {
                item.stall_failure_to_withdraw = false;
                true
            }
            _ => false,
        }
    }

    /// The items being downloaded or tidied up right now, each with the
    /// attempt running it (#1432). Used by the stall check, so its timers
    /// belong to one attempt and a retried item starts a fresh one.
    pub(crate) fn running_attempts(&self) -> Vec<(Attempt, QueueItemStatus)> {
        self.items
            .iter()
            .filter(|i| is_running_state(&i.status.state))
            .map(|i| {
                (
                    Attempt {
                        id: i.status.id.clone(),
                        number: i.attempt,
                    },
                    i.status.clone(),
                )
            })
            .collect()
    }

    /// Queues an item again, once, when a run the stall check marked failed
    /// finishes after all (#1432, maintainer decision 17 Nov 2026).
    ///
    /// Only while that same run is still the item's latest attempt and the
    /// item is still in the Error state the stall check set -- so never
    /// after the person retried or removed it. The whole link is fetched
    /// again (no narrowing to missing songs), so the tidying-up runs over
    /// every file; files already there are kept unless the person has
    /// "Overwrite Existing Files" on. Returns whether it queued the item.
    pub(crate) fn retry_after_late_finish(
        &mut self,
        attempt: &Attempt,
        settings: &AppSettings,
    ) -> bool {
        let max = self.max_network_retries;
        let Some(item) = self.items.iter_mut().find(|i| {
            i.status.id == attempt.id
                && i.attempt == attempt.number
                && i.stalled_attempt == Some(attempt.number)
                && i.status.state == DownloadState::Error
                // At most once per run the person started (#1432 review):
                // the automatic retry itself is never retried automatically.
                && !i.auto_retried_after_stall
        }) else {
            return false;
        };
        reset_for_fresh_run(
            item,
            settings,
            max,
            &mut self.next_attempt,
            &mut self.live_stalled_runs,
        );
        // Remember that this one was queued by MeedyaDL, not the person, and
        // that the stall's failure was already counted.
        item.auto_retried_after_stall = true;
        item.stall_failure_to_withdraw = true;
        log::info!(
            "Download {} queued again after its stalled run finished",
            attempt.id
        );
        true
    }

    /// Returns true when the queue has no active or pending downloads.
    /// Used to trigger after-queue actions.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.is_idle_at(std::time::Instant::now())
    }

    /// [`Self::is_idle`] at a given moment, so the one-hour wait is tested
    /// without waiting.
    ///
    /// A run the stall check marked failed but left going still counts as
    /// work (#1432 review): it may queue its one retry when it ends. But
    /// only for [`STALLED_RUN_WAIT`] after it was marked stuck (maintainer
    /// decision, 17 Nov 2026); after that it no longer holds the queue back.
    #[must_use]
    pub(crate) fn is_idle_at(&self, now: std::time::Instant) -> bool {
        self.active.is_empty()
            && self
                .live_stalled_runs
                .iter()
                .all(|r| stalled_wait_over(r.since, now))
            && !self
                .items
                .iter()
                .any(|i| i.status.state == DownloadState::Queued)
    }

    /// Should we still be doing work for this item?
    ///
    /// `false` when it has been cancelled **or when it is no longer in
    /// the queue at all**, and that second half is the point.
    ///
    /// A reviewer found the first version of the companion-cancellation
    /// check asking only "is it cancelled?", which answers *no* for an
    /// item that is not there. Cancelled rows can be cleared straight
    /// away — that is what the Clear buttons do — so somebody who
    /// cancelled and then tidied up their queue would have had the work
    /// carry on, because the row saying to stop had been removed. An
    /// item nobody is tracking is not an item to keep working for.
    #[must_use]
    pub fn should_keep_working_on(&self, download_id: &str) -> bool {
        self.items
            .iter()
            .find(|i| i.status.id == download_id)
            .is_some_and(|i| i.status.state != DownloadState::Cancelled)
    }

    /// Checks if a download has been cancelled by the user.
    /// Called by the cancellation polling loop in `run_download_with_events()`
    /// every 250ms to detect if the user cancelled while the process is running.
    /// If true, the caller should kill the GAMDL subprocess.
    ///
    /// Note this answers `false` for an item that is no longer in the
    /// queue. For "should I keep working on this?", which is the
    /// question with the longer-running work, use
    /// [`Self::should_keep_working_on`].
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
    /// Peek at what a smart-retry plan *would* produce for `download_id`
    /// without mutating queue state (#667). Returns `None` when the item
    /// doesn't exist or has no output path — those cases just fall
    /// through to dumb retry without diagnostic value.
    ///
    /// Used by the `retry_download` IPC to give the frontend a precise
    /// "nothing to retry" message when the planner reports
    /// [`super::smart_retry_planner::PlanOutcome::AllPresent`], rather
    /// than the generic `Download cannot be retried` error.
    #[must_use]
    pub fn peek_smart_retry_outcome(
        &self,
        download_id: &str,
    ) -> Option<super::smart_retry_planner::PlanOutcome> {
        let item = self.items.iter().find(|i| i.status.id == download_id)?;
        let output = item.status.output_path.as_deref()?;
        if !item.status.output_is_directory {
            return None;
        }
        let first_url = item.request.urls.first()?;
        Some(super::smart_retry_planner::plan_retry(
            std::path::Path::new(output),
            first_url,
        ))
    }

    pub fn retry(&mut self, download_id: &str, settings: &AppSettings) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.status.id == download_id) {
            if item.status.state == DownloadState::Error
                || item.status.state == DownloadState::Cancelled
            {
                // Smart manifest-driven retry (#667). When the failed item
                // has a known output directory containing a
                // `manifest.meedyadl`, diff the expected track set against
                // disk and replace the queue item's URLs with a precise
                // per-track list so GAMDL only revisits the tracks that
                // actually failed. Falls through to the existing dumb
                // retry on any unsupported case (no manifest, no source
                // match, no per-track URLs recorded).
                let smart_outcome = item
                    .status
                    .output_path
                    .as_deref()
                    .filter(|_| item.status.output_is_directory)
                    .and_then(|out| {
                        let path = std::path::Path::new(out);
                        item.request.urls.first().map(|first_url| {
                            super::smart_retry_planner::plan_retry(path, first_url)
                        })
                    });

                if let Some(super::smart_retry_planner::PlanOutcome::AllPresent { total_tracks }) =
                    smart_outcome
                {
                    // Every expected track is on disk — there's nothing
                    // for the retry to fetch. Refuse the retry and let
                    // the caller surface "nothing to do". Returning
                    // `false` here means the IPC reports a clean refusal;
                    // the activity-log helper above has already logged
                    // the diagnostic. We do NOT clear the Error state —
                    // the user's previous attempt's error is still the
                    // most accurate description of what happened.
                    log::info!(
                        "Smart retry for {download_id}: all {total_tracks} track(s) already \
                         present on disk — refusing retry"
                    );
                    return false;
                }

                // Fresh options, counters and status, under a new attempt
                // number so nothing from the earlier run can act on the
                // item any more (#1432).
                reset_for_fresh_run(
                    item,
                    settings,
                    self.max_network_retries,
                    &mut self.next_attempt,
                    &mut self.live_stalled_runs,
                );
                // The person's own Retry starts a new run of their own: it may
                // again be retried once automatically after a stall, and any
                // earlier stall failure is no longer this run's to take back
                // (#1432 review).
                item.auto_retried_after_stall = false;
                item.stall_failure_to_withdraw = false;

                // Apply the smart-retry plan if one was produced. The plan
                // narrows the URL set to per-track entries that GAMDL can
                // fetch in a single invocation, sharply cutting wall time
                // (no metadata re-fetch for already-present tracks, no
                // companion re-traversal, smaller enrichment surface).
                if let Some(super::smart_retry_planner::PlanOutcome::Plan(plan)) = smart_outcome {
                    log::info!(
                        "Smart retry for {download_id}: targeting {} of {} track(s) — \
                         {} URL(s) queued",
                        plan.missing_tracks,
                        plan.total_tracks,
                        plan.urls_to_fetch.len(),
                    );
                    item.request.urls = plan.urls_to_fetch.clone();
                    item.status.urls = plan.urls_to_fetch;
                }
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
                // Work the service out from the link again rather than reading
                // the stored text back. Same call the queue made when this item
                // was first added, so the two can never disagree, and it needs
                // no reverse lookup from the stored form (#829).
                // Fresh options: nothing MeedyaDL added is left in them.
                item.added_template_suffixes.clear();
                item.merged_options = merge_options(
                    item.request.options.as_ref(),
                    settings,
                    item.request
                        .urls
                        .first()
                        .and_then(|u| crate::models::media_service::MediaServiceId::from_url(u)),
                );
                // Override wrapper settings: disable wrapper, clear wrapper URLs
                item.merged_options.use_wrapper = Some(false);
                item.merged_options.wrapper_account_url = None;
                item.merged_options.wrapper_decrypt_ip = None;
                item.merged_options.wrapper_m3u8_ip = None;
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
                // A new attempt number (#1432): the run that failed can no
                // longer change the re-queued item.
                retire_running_attempt(item, &mut self.next_attempt, &mut self.live_stalled_runs);
                log::info!("Download {download_id} reset for retry without wrapper");
                return true;
            }
        }
        false
    }

    /// The automatic form of [`Self::retry_without_wrapper`], used by a run
    /// that has just failed (#1432). It only acts while that run still owns
    /// the item, so it can never re-queue an item the person has since
    /// retried, cancelled or removed, or one the stall check marked failed.
    pub(crate) fn retry_without_wrapper_auto(
        &mut self,
        attempt: &Attempt,
        settings: &AppSettings,
    ) -> bool {
        if find_owned(&mut self.items, attempt).is_none() {
            return false;
        }
        self.retry_without_wrapper(&attempt.id, settings)
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
    /// A small picture of the queue, for the connection watcher (#1156).
    ///
    /// Answers the only four questions the watcher asks, in one look under the
    /// lock. Taking them separately would let the queue change between
    /// questions and give an answer that was never true at any single moment.
    ///
    /// Deliberately cheap: it counts and copies one link, rather than cloning
    /// every item the way `get_status` does. The watcher asks this often.
    pub fn watch_snapshot(&self) -> QueueWatchSnapshot {
        let queued = self
            .items
            .iter()
            .filter(|i| i.status.state == DownloadState::Queued)
            .count();
        QueueWatchSnapshot {
            queued,
            // Anything at all in flight. The watcher must never start work
            // while something is running — that is the rule the whole queue
            // is built on.
            active: !self.active.is_empty(),
            paused: self.paused,
            // Used to decide which service to check is reachable. The first
            // waiting item is the one that would start.
            first_queued_url: self
                .items
                .iter()
                .find(|i| i.status.state == DownloadState::Queued)
                .and_then(|i| i.request.urls.first().cloned()),
        }
    }

    /// Forgets when the last set of pre-download checks ran.
    ///
    /// Those checks are skipped if they ran in the last minute, which is right
    /// for ordinary use but wrong straight after an outage: the whole point is
    /// that the situation has just changed. Called by the connection watcher
    /// immediately before it starts the queue, so the checks genuinely re-run.
    pub fn reset_preflight_cooldown(&mut self) {
        self.last_preflight_at = None;
    }

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
                service: item.status.service.clone(),
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
            // Same as the retry paths: re-detect from the link rather than
            // parsing the stored service text back into the enum (#829).
            let merged_options = merge_options(
                p.request.options.as_ref(),
                settings,
                p.request
                    .urls
                    .first()
                    .and_then(|u| crate::models::media_service::MediaServiceId::from_url(u)),
            );

            // Items with a persisted error are restored in Error state so the
            // user sees the failure reason and can choose to retry. Active
            // items are reset to Queued for automatic re-processing.
            let (state, error) = if p.error.is_some() {
                (DownloadState::Error, p.error)
            } else {
                (DownloadState::Queued, None)
            };

            // Restore or re-detect service/engine from persisted data.
            // Older persisted items may not have the service field, so
            // fall back to URL-based detection for backwards compatibility.
            let service_str = p.service.or_else(|| {
                let url = p.request.urls.first()?;
                crate::models::media_service::MediaServiceId::from_url(url)
                    .map(|svc| svc.to_string())
            });
            let engine_str = service_str.as_ref().and_then(|svc_id| {
                let registry = crate::services::engine_registry::EngineRegistry::load();
                registry.resolve_engine(svc_id).map(|e| e.id.clone())
            });

            let (album_name, artist_name) = extract_album_info_from_url(
                p.request.urls.first().map(String::as_str).unwrap_or(""),
            );

            let item = QueueItem {
                status: QueueItemStatus {
                    id: p.id.clone(),
                    urls: p.request.urls.clone(),
                    service: service_str,
                    engine: engine_str,
                    state,
                    progress: 0.0,
                    current_track: None,
                    album_name,
                    artist_name,
                    artwork_url: None,
                    total_tracks: None,
                    completed_tracks: None,
                    speed: None,
                    eta: None,
                    processing_label: None,
                    processing_progress: None,
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
                    // Re-fetched from the Apple Music API on next attempt.
                    audio_traits: Vec::new(),
                    // Same — re-discovered when enrichment runs again.
                    mv_companion_count: None,
                    created_at: p.created_at,
                },
                request: p.request,
                merged_options,
                fallback_index: 0,
                engine_fallback_index: 0,
                network_retries_left: self.max_network_retries,
                storefront_fallback_attempted: false,
                added_template_suffixes: Vec::new(),
                reported_paths: Vec::new(),
                // A fresh attempt number for a restored item (#1432).
                attempt: take_attempt_number(&mut self.next_attempt),
                stalled_attempt: None,
                stalled_run_saved_files: false,
                auto_retried_after_stall: false,
                stall_failure_to_withdraw: false,
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
                    ..Default::default()
                };
                self.enqueue(request, settings)
            })
            .collect()
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests;
