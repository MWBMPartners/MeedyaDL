// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// App-wide activity log emission helpers.
// ==========================================
//
// Provides a shared `ActivityLogEvent` struct and public helper functions
// for emitting activity log events from anywhere in the Rust backend. These
// events are received by the frontend via the "activity-log" Tauri event and
// displayed in the Activity Log page. All events are ALSO written to the
// tracing file log (via `log::info!`) so enrichment progress is visible
// in the on-disk log even when the frontend is unresponsive.
//
// Two emission patterns:
//   - `emit_download_log(app, download_id, message)` — for download-specific messages
//   - `emit_app_log(app, message)` — for system-level messages (download_id = "system")
//
// The "system" sentinel ID is rendered as [System] in the frontend UI instead
// of the usual 8-character download ID prefix.
//
// Used by: services::download_queue, services::metadata_tag_service,
//          commands::gamdl, commands::updates, commands::dependencies,
//          commands::cookies, commands::settings, commands::login_window

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri::Emitter;

/// Global flag controlling verbose activity log output. When `true`,
/// `emit_verbose_download_log()` and `emit_verbose_app_log()` emit to
/// the frontend activity log. When `false`, verbose messages are silently
/// dropped (they still go to the tracing file log via `log::debug!()`).
///
/// Set by `set_verbose_logging()` whenever settings are loaded or changed.
/// Uses `Relaxed` ordering — eventual consistency is fine for a logging flag.
static VERBOSE_LOGGING: AtomicBool = AtomicBool::new(false);

/// Global handle to the on-disk activity log writer (#541).
///
/// Registered once at startup by `lib.rs` via `register_disk_writer()`.
/// All emit helpers fan out to it after emitting the Tauri event, so
/// every activity log line is persisted to a daily-rotating file
/// regardless of the in-memory 10K cap or verbose filtering.
///
/// Verbose-gated emits still hit the disk writer — we want **every**
/// event on disk for bug hunting, even when the UI is filtering them.
///
/// `OnceLock` avoids the runtime cost of a `Mutex`/`RwLock` on the hot
/// path: after the single write in `register_disk_writer()`, all reads
/// are lock-free.
static DISK_WRITER: OnceLock<crate::services::activity_log_writer::ActivityLogWriterHandle> =
    OnceLock::new();

/// Registers the on-disk activity log writer. Called once from the
/// Tauri `setup` closure after `activity_log_writer::start()` has
/// spawned the background task.
pub fn register_disk_writer(
    handle: crate::services::activity_log_writer::ActivityLogWriterHandle,
) {
    if DISK_WRITER.set(handle).is_err() {
        log::warn!("activity_log: disk writer already registered — ignoring second call");
    }
}

/// Sends an event to the on-disk writer if one has been registered.
/// No-op (silently) when the writer is not yet configured (e.g.
/// before `setup` runs, or in unit tests).
///
/// Public so that direct-emit sites in `services/download_queue.rs`
/// can mirror their own `app.emit("activity-log", ...)` calls to disk
/// without going through the normal `emit_*` helpers.
pub fn write_to_disk(event: &ActivityLogEvent) {
    if let Some(writer) = DISK_WRITER.get() {
        writer.send(event.clone());
    }
}

/// Update the verbose logging flag. Called from settings load/save paths.
pub fn set_verbose_logging(enabled: bool) {
    VERBOSE_LOGGING.store(enabled, Ordering::Relaxed);
}

/// Check whether verbose logging is currently enabled.
pub fn is_verbose_logging() -> bool {
    VERBOSE_LOGGING.load(Ordering::Relaxed)
}

/// Sentinel `download_id` value for system-level (non-download) activity log entries.
/// The frontend recognises this value and renders `[System]` instead of a download
/// ID prefix.
pub const SYSTEM_LOG_ID: &str = "system";

/// Payload for the `"activity-log"` Tauri event.
///
/// Stream values:
///   - `"stdout"` — GAMDL subprocess stdout
///   - `"stderr"` — GAMDL subprocess stderr
///   - `"internal"` — MeedyaDL internal actions (enrichment, companions, system events)
#[derive(Clone, serde::Serialize)]
pub struct ActivityLogEvent {
    pub download_id: String,
    pub stream: &'static str,
    pub line: String,
    pub timestamp: String,
}

/// Emits an internal activity log event tied to a specific download.
///
/// Used for enrichment progress, companion downloads, fallback decisions,
/// and other per-download diagnostic messages. Also writes to the tracing
/// file log so enrichment progress is captured on disk.
pub fn emit_download_log(app: &tauri::AppHandle, download_id: &str, message: &str) {
    log::info!("[{download_id}] {message}");
    let event = ActivityLogEvent {
        download_id: download_id.to_string(),
        stream: "internal",
        line: message.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let _ = app.emit("activity-log", &event);
    write_to_disk(&event);
}

/// Emits a system-level activity log event (not tied to any download).
///
/// Used for queue operations, update checks, dependency installs, cookie
/// imports, settings changes, and app lifecycle events.
pub fn emit_app_log(app: &tauri::AppHandle, message: &str) {
    log::info!("[System] {message}");
    let event = ActivityLogEvent {
        download_id: SYSTEM_LOG_ID.to_string(),
        stream: "internal",
        line: message.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let _ = app.emit("activity-log", &event);
    write_to_disk(&event);
}

/// Emits a verbose download-specific activity log event.
///
/// Only emits when `verbose_activity_log` is enabled in settings.
/// Used for detailed debugging information that may contain sensitive
/// data (URLs with query params, cookie paths, API responses, etc.).
///
/// The message is prefixed with `[VERBOSE]` in the activity log to
/// distinguish it from normal messages. Always written to the tracing
/// file log as `debug` regardless of the verbose setting.
pub fn emit_verbose_download_log(app: &tauri::AppHandle, download_id: &str, message: &str) {
    log::debug!("[{download_id}] [VERBOSE] {message}");
    // Always persist verbose events to disk so bug-hunting sessions
    // have the full record, even when the UI is not showing them.
    let event = ActivityLogEvent {
        download_id: download_id.to_string(),
        stream: "internal",
        line: format!("[VERBOSE] {message}"),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    write_to_disk(&event);
    if !VERBOSE_LOGGING.load(Ordering::Relaxed) {
        return;
    }
    let _ = app.emit("activity-log", &event);
}

/// Emits a verbose system-level activity log event.
///
/// Only emits when `verbose_activity_log` is enabled in settings.
/// Always written to the tracing file log as `debug`.
pub fn emit_verbose_app_log(app: &tauri::AppHandle, message: &str) {
    log::debug!("[System] [VERBOSE] {message}");
    // Always persist verbose events to disk so bug-hunting sessions
    // have the full record, even when the UI is not showing them.
    let event = ActivityLogEvent {
        download_id: SYSTEM_LOG_ID.to_string(),
        stream: "internal",
        line: format!("[VERBOSE] {message}"),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    write_to_disk(&event);
    if !VERBOSE_LOGGING.load(Ordering::Relaxed) {
        return;
    }
    let _ = app.emit("activity-log", &event);
}
