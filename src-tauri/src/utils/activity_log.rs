// Copyright (c) 2026 MeedyaSuite
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

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
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

/// Per-download activity-event counter (#846).
///
/// The queue watchdog (`services/queue_watchdog.rs`) compares a tuple of
/// progress fields across poll cycles to detect stalls. During long-running
/// passes that don't move the per-item progress bar (e.g. the once-per-item
/// companion lyrics conversion from #843) the tuple is bit-identical for
/// the duration even though hundreds of activity-log events fire — so the
/// watchdog issues a false-positive "no progress signal for 10 min" warn.
///
/// Mirroring an emit-counter from `emit_inner` into the watchdog snapshot
/// gives the watchdog a cheap, accurate "is work happening?" signal: any
/// per-download activity-log event bumps the counter, which the watchdog
/// reads under [`get_activity_count`].
///
/// Map grows monotonically (one u64 per ever-seen download_id), which is
/// fine — even 10k downloads is ~240 KB. Cleanup on terminal-state would
/// be possible but adds complexity for no real win.
static ACTIVITY_COUNTERS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();

fn activity_counters() -> &'static Mutex<HashMap<String, u64>> {
    ACTIVITY_COUNTERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn bump_activity_count(download_id: &str) {
    if let Ok(mut map) = activity_counters().lock() {
        let entry = map.entry(download_id.to_string()).or_insert(0);
        *entry = entry.wrapping_add(1);
    }
}

/// Reads the activity-event count for a download (#846).
///
/// Returns 0 if no events have been emitted for this id yet, or if the
/// counter lock is poisoned (extremely unlikely; would only happen if a
/// panic occurred while holding it).
pub fn get_activity_count(download_id: &str) -> u64 {
    activity_counters()
        .lock()
        .ok()
        .and_then(|map| map.get(download_id).copied())
        .unwrap_or(0)
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

/// Severity classification for activity-log entries (#793).
///
/// Drives the frontend's colour-coded rendering — red for `Error`,
/// amber for `Warning`, default colour for `Info`. The serde
/// representation is lowercase (`"info"` / `"warning"` / `"error"`)
/// so it round-trips through JSON without ceremony.
///
/// `Info` is the default — existing emit sites get this implicitly
/// via `default_severity()`, so the entire migration to severity-
/// tagged emits is incremental: only sites that genuinely represent
/// a warning or error need to opt in via the dedicated helpers.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum LogSeverity {
    #[default]
    Info,
    Warning,
    Error,
}

/// Helper for `#[serde(default = "default_severity")]` on
/// [`ActivityLogEvent`] so older persisted records / older
/// frontends that don't know about severity default cleanly to
/// `Info`.
fn default_severity() -> LogSeverity {
    LogSeverity::Info
}

/// Payload for the `"activity-log"` Tauri event.
///
/// Stream values:
///   - `"stdout"` — GAMDL subprocess stdout
///   - `"stderr"` — GAMDL subprocess stderr
///   - `"internal"` — MeedyaDL internal actions (enrichment, companions, system events)
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ActivityLogEvent {
    pub download_id: String,
    pub stream: &'static str,
    pub line: String,
    pub timestamp: String,
    /// Severity classification (#793). `Info` by default; the
    /// `warn` / `error` emit helpers set this explicitly. The
    /// frontend uses it to pick the right design-token text class
    /// (red / amber / default), staying theme-aware across light /
    /// dark / high-contrast / colour-blind palettes.
    #[serde(default = "default_severity")]
    pub severity: LogSeverity,
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
    severity: LogSeverity,
) {
    let id = download_id.unwrap_or(SYSTEM_LOG_ID);

    // #846: bump the per-download activity counter so the queue watchdog
    // sees activity-log emission as a progress signal. System-level
    // messages (no download_id) intentionally don't bump anything —
    // they're not tied to a specific stalled item.
    if download_id.is_some() {
        bump_activity_count(id);
    }

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
    // the on-disk tracing log). Severity drives the tracing level
    // so the file log groups Error/Warning/Info correctly too
    // (#793 — same source of truth on disk as in the UI).
    let id_prefix = if download_id.is_some() { id } else { "System" };
    if verbose {
        log::debug!("[{id_prefix}] [VERBOSE] {enriched}");
    } else {
        match severity {
            LogSeverity::Error => log::error!("[{id_prefix}] {enriched}"),
            LogSeverity::Warning => log::warn!("[{id_prefix}] {enriched}"),
            LogSeverity::Info => log::info!("[{id_prefix}] {enriched}"),
        }
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
        severity,
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
///
/// Defaults to `LogSeverity::Info` — use [`emit_download_warn`] /
/// [`emit_download_error`] when the message represents an actual
/// warning or error so the UI can colour-code it (#793).
pub fn emit_download_log(app: &tauri::AppHandle, download_id: &str, message: &str) {
    emit_inner(app, Some(download_id), message, false, LogSeverity::Info);
}

/// Emits an internal activity-log event with [`LogSeverity::Warning`]
/// (#793). Frontend renders the line in `text-status-warning` (amber)
/// across all themes.
pub fn emit_download_warn(app: &tauri::AppHandle, download_id: &str, message: &str) {
    emit_inner(app, Some(download_id), message, false, LogSeverity::Warning);
}

/// Emits an internal activity-log event with [`LogSeverity::Error`]
/// (#793). Frontend renders the line in `text-status-error` (red)
/// across all themes.
pub fn emit_download_error(app: &tauri::AppHandle, download_id: &str, message: &str) {
    emit_inner(app, Some(download_id), message, false, LogSeverity::Error);
}

/// Emits a system-level activity log event (not tied to any download).
///
/// Used for queue operations, update checks, dependency installs, cookie
/// imports, settings changes, and app lifecycle events. See
/// [`emit_app_warn`] / [`emit_app_error`] for severity-tagged variants.
pub fn emit_app_log(app: &tauri::AppHandle, message: &str) {
    emit_inner(app, None, message, false, LogSeverity::Info);
}

/// Emits a system-level activity-log event with [`LogSeverity::Warning`].
pub fn emit_app_warn(app: &tauri::AppHandle, message: &str) {
    emit_inner(app, None, message, false, LogSeverity::Warning);
}

/// Emits a system-level activity-log event with [`LogSeverity::Error`].
pub fn emit_app_error(app: &tauri::AppHandle, message: &str) {
    emit_inner(app, None, message, false, LogSeverity::Error);
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
    emit_inner(app, Some(download_id), message, true, LogSeverity::Info);
}

/// Emits a verbose system-level activity log event.
///
/// Only emits to the activity-log UI when `verbose_activity_log` is
/// enabled in settings. Always written to the tracing file log as
/// `debug` AND to the on-disk activity log file.
pub fn emit_verbose_app_log(app: &tauri::AppHandle, message: &str) {
    emit_inner(app, None, message, true, LogSeverity::Info);
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
    // #846: subprocess output is a strong progress signal — bump the
    // watchdog counter so GAMDL's per-track lines count as activity
    // even when the in-UI line is suppressed for noise.
    bump_activity_count(download_id);

    // Severity inferred from GAMDL's structlog level prefix (#793).
    // GAMDL 3.0+ emits lines like `[WARNING  12:10:05] [Track 2/4]
    // Skipping…` so we can colour-code without changing the
    // subprocess output itself. Pre-v3.0 GAMDL had no level prefix
    // → defaults to Info.
    let severity = infer_severity_from_subprocess_line(&line, stream);
    let event = ActivityLogEvent {
        download_id: download_id.to_string(),
        stream,
        line,
        timestamp: chrono::Utc::now().to_rfc3339(),
        severity,
    };
    if show_in_ui {
        let _ = app.emit("activity-log", &event);
    }
    // Disk fan-out is unconditional — the on-disk activity log file
    // is the forensic record, kept regardless of UI filtering.
    write_to_disk(&event);
}

/// Infer the severity of a raw subprocess output line for the
/// frontend's colour-coded rendering (#793).
///
/// Three heuristics, applied in order:
///
///   1. **GAMDL structlog level prefix** (`[LEVEL    HH:MM:SS]`
///      shape) — most reliable signal on GAMDL v3.0+. Maps
///      `ERROR` / `CRITICAL` → Error and `WARNING` → Warning.
///      Other levels (INFO / DEBUG) fall through.
///
///   2. **Stream tag** — anything on `stderr` defaults to Warning
///      (most Python tooling sends genuine warnings + errors
///      there). Avoids over-classifying because GAMDL's own
///      `ERROR` lines also land on stderr and the level prefix
///      already promoted them to `Error`.
///
///   3. **Default** — Info.
///
/// Deliberately conservative: we'd rather under-colour than
/// mis-colour a line (e.g. a sentence containing "error" inside
/// an otherwise informational message shouldn't go red).
#[must_use]
fn infer_severity_from_subprocess_line(line: &str, stream: &str) -> LogSeverity {
    // Trim a leading `\r` overwrite that the GAMDL reader may have
    // coalesced past; structlog prefix is always after that.
    let trimmed = line.trim_start_matches('\r').trim_start();

    if let Some(rest) = trimmed.strip_prefix('[') {
        // Take everything up to the first ']' — should be
        // `LEVEL    HH:MM:SS` — then split on whitespace and
        // check the first token.
        if let Some(end_idx) = rest.find(']') {
            let inside = &rest[..end_idx];
            let level = inside.split_whitespace().next().unwrap_or("");
            match level {
                "ERROR" | "CRITICAL" | "FATAL" => return LogSeverity::Error,
                "WARNING" | "WARN" => return LogSeverity::Warning,
                _ => {}
            }
        }
    }

    if stream == "stderr" {
        return LogSeverity::Warning;
    }

    LogSeverity::Info
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_default_is_info() {
        assert_eq!(LogSeverity::default(), LogSeverity::Info);
    }

    #[test]
    fn severity_serialises_as_lowercase() {
        assert_eq!(
            serde_json::to_string(&LogSeverity::Info).unwrap(),
            "\"info\""
        );
        assert_eq!(
            serde_json::to_string(&LogSeverity::Warning).unwrap(),
            "\"warning\""
        );
        assert_eq!(
            serde_json::to_string(&LogSeverity::Error).unwrap(),
            "\"error\""
        );
    }

    #[test]
    fn infer_severity_recognises_gamdl_error_prefix() {
        let line = "[ERROR    12:10:05] [Track 2/4] Failed to download";
        assert_eq!(
            infer_severity_from_subprocess_line(line, "stderr"),
            LogSeverity::Error
        );
    }

    #[test]
    fn infer_severity_recognises_gamdl_warning_prefix() {
        let line = "[WARNING  12:10:05] [Track 2/4] Skipping format";
        assert_eq!(
            infer_severity_from_subprocess_line(line, "stderr"),
            LogSeverity::Warning
        );
    }

    #[test]
    fn infer_severity_info_prefix_falls_through_to_stream_default() {
        // INFO on stderr → still Warning by stream-default rule
        // (some tools route everything through stderr including
        // genuine INFO; we prefer the conservative Warning over
        // misleading Info).
        let line = "[INFO     12:10:05] Starting download";
        assert_eq!(
            infer_severity_from_subprocess_line(line, "stderr"),
            LogSeverity::Warning
        );
        // INFO on stdout → Info default.
        assert_eq!(
            infer_severity_from_subprocess_line(line, "stdout"),
            LogSeverity::Info
        );
    }

    #[test]
    fn infer_severity_unprefixed_stdout_is_info() {
        let line = "Saved to: /Users/me/Music/Album/01 Track.m4a";
        assert_eq!(
            infer_severity_from_subprocess_line(line, "stdout"),
            LogSeverity::Info
        );
    }

    #[test]
    fn infer_severity_unprefixed_stderr_is_warning() {
        let line = "Some unexpected output";
        assert_eq!(
            infer_severity_from_subprocess_line(line, "stderr"),
            LogSeverity::Warning
        );
    }

    /// Conservative: a sentence MENTIONING "error" inside otherwise
    /// informational text should NOT be promoted to Error — only
    /// the explicit bracketed level prefix triggers a promotion.
    #[test]
    fn infer_severity_does_not_overreact_to_keyword_mentions() {
        let line = "Skipping cover error retry as a no-op (#774)";
        assert_eq!(
            infer_severity_from_subprocess_line(line, "stdout"),
            LogSeverity::Info
        );
    }

    /// A leading `\r` overwrite (common in GAMDL progress output)
    /// shouldn't confuse the prefix matcher.
    #[test]
    fn infer_severity_handles_leading_cr_overwrite() {
        let line = "\r[ERROR    12:10:05] [Track 2/4] Boom";
        assert_eq!(
            infer_severity_from_subprocess_line(line, "stderr"),
            LogSeverity::Error
        );
    }
}
