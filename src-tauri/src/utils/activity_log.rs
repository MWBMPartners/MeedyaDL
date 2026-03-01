// Copyright (c) 2024-2026 MeedyaDL
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

use tauri::Emitter;

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
