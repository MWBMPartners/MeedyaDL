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
use tauri::{Emitter, Manager};

use crate::services::download_queue::QueueHandle;

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

/// Best-effort lookup of the human-readable media label
/// (`Artist — Album — Track`) for a given download ID, sourced from the
/// queue's Tauri managed state.
///
/// Used by [`emit_download_log`] and [`emit_verbose_download_log`] to
/// auto-enrich every `[MeedyaDL]` log line with the affected media item,
/// so downstream readers (Activity Log UI, on-disk forensic file) don't
/// have to cross-reference the 8-char download ID against the queue page
/// to know which album / track a message refers to.
///
/// **Best-effort by design.** Uses `try_lock` on the queue mutex, so:
///   - If the caller already holds the queue lock (common pattern in
///     `download_queue.rs` — emit happens inside `lock().await { … }`),
///     the lookup returns `None` and the message is emitted unchanged.
///   - If the queue is unavailable (e.g. before Tauri `setup` runs, or
///     in unit tests), the lookup returns `None`.
///
/// Trading off "guaranteed every line gets a label" for "never blocks
/// the emitter, never deadlocks". The 95% of emissions that happen
/// outside held-lock blocks get the label; the rest fall back to the
/// pre-#706-style bare `[MeedyaDL]` line.
fn lookup_media_label(app: &tauri::AppHandle, download_id: &str) -> Option<String> {
    if download_id == SYSTEM_LOG_ID {
        return None;
    }
    let queue = app.try_state::<QueueHandle>()?;
    let q = queue.try_lock().ok()?;
    q.media_label_for(download_id)
}

/// Appends the media label to a message, separated by ` — `, when a
/// label is provided. No-op when label is `None` so callers can wrap
/// every emit unconditionally.
fn append_label(message: &str, label: Option<String>) -> String {
    match label {
        Some(label) => format!("{message} — {label}"),
        None => message.to_string(),
    }
}

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

/// Single-source-of-truth implementation for all four public emit
/// helpers (Phase 3.5c refactor).
///
/// Pre-refactor each of `emit_download_log`, `emit_app_log`,
/// `emit_verbose_download_log`, `emit_verbose_app_log` had its own
/// near-duplicate body — same `ActivityLogEvent` construction, same
/// `app.emit("activity-log", …)` call, same `write_to_disk` fan-out,
/// same media-label enrichment for the per-download variants. Five
/// places to update when the event shape changed (e.g. the #712
/// label-enrichment change had to be applied to two of the four).
///
/// This function does the work in one place; the four public helpers
/// remain as thin facades so the 264 existing call sites need not
/// change. Future emission rules (rate-limiting, redaction, sampling)
/// only need to touch this function.
///
/// `download_id == None` is rendered as the [`SYSTEM_LOG_ID`] sentinel
/// (UI displays `[System]`); `Some(id)` is rendered as the 8-char ID.
/// `verbose == true` adds the `[VERBOSE]` prefix and routes through
/// the `VERBOSE_LOGGING` flag — disk fan-out always happens (we want
/// the full record on disk for bug hunting), but the Tauri event is
/// dropped when the user has verbose logging disabled.
fn emit_inner(
    app: &tauri::AppHandle,
    download_id: Option<&str>,
    message: &str,
    verbose: bool,
) {
    let id = download_id.unwrap_or(SYSTEM_LOG_ID);

    // Per-download messages get media-label enrichment via the queue
    // (Phase 3a / #712); system-level messages don't have a queue
    // item to look up.
    let enriched = if download_id.is_some() {
        let label = lookup_media_label(app, id);
        append_label(message, label)
    } else {
        message.to_string()
    };

    // Tracing-log line — `[id] msg` or `[System] msg`, with `[VERBOSE]`
    // prefix in the verbose case. Always emitted regardless of the
    // VERBOSE_LOGGING flag (the flag gates the activity-log UI, not
    // the on-disk tracing log).
    if verbose {
        log::debug!("[{id}] [VERBOSE] {enriched}");
    } else if download_id.is_some() {
        log::info!("[{id}] {enriched}");
    } else {
        log::info!("[System] {enriched}");
    }

    // Build the activity-log event payload. Verbose messages get the
    // `[VERBOSE]` prefix in the line so the UI can render them with a
    // distinct style.
    let line = if verbose {
        format!("[VERBOSE] {enriched}")
    } else {
        enriched
    };
    let event = ActivityLogEvent {
        download_id: id.to_string(),
        stream: "internal",
        line,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    // Always fan out to disk — bug-hunting sessions need the full
    // record even when the UI filter is hiding things.
    write_to_disk(&event);

    // Only emit the Tauri event for the UI when (a) the message is
    // non-verbose, OR (b) the user has verbose logging enabled.
    if !verbose || VERBOSE_LOGGING.load(Ordering::Relaxed) {
        let _ = app.emit("activity-log", &event);
    }
}

/// Emits an internal activity log event tied to a specific download.
///
/// Used for enrichment progress, companion downloads, fallback decisions,
/// and other per-download diagnostic messages. Also writes to the tracing
/// file log so enrichment progress is captured on disk.
pub fn emit_download_log(app: &tauri::AppHandle, download_id: &str, message: &str) {
    emit_inner(app, Some(download_id), message, false);
}

/// Emits a system-level activity log event (not tied to any download).
///
/// Used for queue operations, update checks, dependency installs, cookie
/// imports, settings changes, and app lifecycle events.
pub fn emit_app_log(app: &tauri::AppHandle, message: &str) {
    emit_inner(app, None, message, false);
}

/// Emits a verbose download-specific activity log event.
///
/// Only emits to the activity-log UI when `verbose_activity_log` is
/// enabled in settings. Always written to the tracing file log as
/// `debug` AND to the on-disk activity log file (the latter is
/// intentional: verbose events stay on disk regardless of UI filter
/// so bug-hunting sessions have the full record).
///
/// Used for detailed debugging information that may contain sensitive
/// data (URLs with query params, cookie paths, API responses, etc.).
/// The message is prefixed with `[VERBOSE]` in the activity log to
/// distinguish it from normal messages.
pub fn emit_verbose_download_log(app: &tauri::AppHandle, download_id: &str, message: &str) {
    emit_inner(app, Some(download_id), message, true);
}

/// Emits a verbose system-level activity log event.
///
/// Only emits to the activity-log UI when `verbose_activity_log` is
/// enabled in settings. Always written to the tracing file log as
/// `debug` AND to the on-disk activity log file.
pub fn emit_verbose_app_log(app: &tauri::AppHandle, message: &str) {
    emit_inner(app, None, message, true);
}

/// Emits a raw subprocess-stream activity log event (Phase 3.5e).
///
/// Used by the stdout/stderr readers in `services/download_queue.rs`
/// and `services/companion_supervisor.rs` to forward GAMDL's own
/// output to the activity log with the correct stream tag (`"stdout"`
/// or `"stderr"`) — distinct from the `"internal"` stream used by
/// MeedyaDL-generated messages.
///
/// Pre-Phase 3.5e, three sites in `download_queue.rs` (the line
/// emitter and the two stdout/stderr readers with Python-traceback
/// suppression from #660) constructed `ActivityLogEvent` and called
/// `app.emit("activity-log", …)` + `write_to_disk` directly,
/// duplicating the same 6-line block three times. Centralising via
/// this helper means future emission rules (rate-limiting, redaction
/// of sensitive subprocess output, sampling) only need to touch one
/// place.
///
/// `show_in_ui` lets callers suppress noisy lines (Python traceback
/// frames in non-verbose mode, repetitive progress lines coalesced
/// by `\r` handling) from the in-memory UI feed while still recording
/// them on disk for forensic diagnosis.
pub fn emit_subprocess_line(
    app: &tauri::AppHandle,
    download_id: &str,
    stream: &'static str,
    line: String,
    show_in_ui: bool,
) {
    let event = ActivityLogEvent {
        download_id: download_id.to_string(),
        stream,
        line,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    if show_in_ui {
        let _ = app.emit("activity-log", &event);
    }
    // Disk fan-out is unconditional — the on-disk activity log file
    // is the forensic record, kept regardless of UI filtering.
    write_to_disk(&event);
}
