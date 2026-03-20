// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// App-wide activity log emission helpers.
// ==========================================
//
// Provides a shared `ActivityLogEvent` struct and two public helper functions
// for emitting activity log events from anywhere in the Rust backend. These
// events are received by the frontend via the "activity-log" Tauri event and
// displayed in the Activity Log page.
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
use tauri::Emitter;

/// Global flag controlling verbose activity log output. When `true`,
/// `emit_verbose_download_log()` and `emit_verbose_app_log()` emit to
/// the frontend activity log. When `false`, verbose messages are silently
/// dropped (they still go to the tracing file log via `log::debug!()`).
///
/// Set by `set_verbose_logging()` whenever settings are loaded or changed.
/// Uses `Relaxed` ordering — eventual consistency is fine for a logging flag.
static VERBOSE_LOGGING: AtomicBool = AtomicBool::new(false);

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
/// and other per-download diagnostic messages.
pub fn emit_download_log(app: &tauri::AppHandle, download_id: &str, message: &str) {
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

/// Emits a system-level activity log event (not tied to any download).
///
/// Used for queue operations, update checks, dependency installs, cookie
/// imports, settings changes, and app lifecycle events.
pub fn emit_app_log(app: &tauri::AppHandle, message: &str) {
    let _ = app.emit(
        "activity-log",
        &ActivityLogEvent {
            download_id: SYSTEM_LOG_ID.to_string(),
            stream: "internal",
            line: message.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    );
}

/// Emits a verbose download-specific activity log event.
///
/// Only emits when `verbose_activity_log` is enabled in settings.
/// Used for detailed debugging information that may contain sensitive
/// data (URLs with query params, cookie paths, API responses, etc.).
///
/// The message is prefixed with `[VERBOSE]` in the activity log to
/// distinguish it from normal messages.
pub fn emit_verbose_download_log(app: &tauri::AppHandle, download_id: &str, message: &str) {
    if !VERBOSE_LOGGING.load(Ordering::Relaxed) {
        return;
    }
    let _ = app.emit(
        "activity-log",
        &ActivityLogEvent {
            download_id: download_id.to_string(),
            stream: "internal",
            line: format!("[VERBOSE] {message}"),
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    );
}

/// Emits a verbose system-level activity log event.
///
/// Only emits when `verbose_activity_log` is enabled in settings.
pub fn emit_verbose_app_log(app: &tauri::AppHandle, message: &str) {
    if !VERBOSE_LOGGING.load(Ordering::Relaxed) {
        return;
    }
    let _ = app.emit(
        "activity-log",
        &ActivityLogEvent {
            download_id: SYSTEM_LOG_ID.to_string(),
            stream: "internal",
            line: format!("[VERBOSE] {message}"),
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    );
}
