// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Crash report IPC command handlers.
// ====================================
//
// Thin wrappers around `services::crash_report_service` that expose
// crash report operations to the React frontend via Tauri's `invoke()`.
//
// Commands:
//   - `list_crash_reports` -- List all saved crash reports
//   - `get_crash_report` -- Get a single report by ID
//   - `delete_crash_report` -- Delete a report by ID
//   - `delete_all_crash_reports` -- Delete all reports at once
//   - `export_crash_report` -- Export as formatted Markdown
//   - `log_frontend_error` -- Save a frontend error as a crash report
//   - `get_github_issue_url` -- Build a pre-filled GitHub new-issue URL
//   - `redact_for_public_report` -- Clean free text before it goes into
//     a pre-filled issue on a THIRD PARTY's public tracker (#1231)

use std::collections::HashMap;
use tauri::AppHandle;

use crate::models::crash_report::CrashReport;
use crate::services::crash_report_service;

/// Returns all crash reports, sorted by timestamp (newest first).
///
/// Called by the frontend to populate a crash reports viewer or to
/// check if there are any crash reports to display after startup.
#[tauri::command]
pub fn list_crash_reports(app: AppHandle) -> Vec<CrashReport> {
    crash_report_service::list_crash_reports(&app)
}

/// Returns a single crash report by its ID.
///
/// Returns `Ok(report)` if found, or `Err` if the ID does not match
/// any stored report.
#[tauri::command]
pub fn get_crash_report(app: AppHandle, id: String) -> Result<CrashReport, String> {
    crash_report_service::get_crash_report(&app, &id)
        .ok_or_else(|| format!("Crash report not found: {id}"))
}

/// Deletes a crash report by its ID.
///
/// Removes the JSON file from the crashes directory. Idempotent:
/// deleting a non-existent report is not an error.
#[tauri::command]
pub fn delete_crash_report(app: AppHandle, id: String) -> Result<(), String> {
    crash_report_service::delete_crash_report(&app, &id)
}

/// Deletes all crash reports from disk.
///
/// Removes every `.json` file in the crashes directory without parsing.
/// Returns the number of files deleted.
#[tauri::command]
pub fn delete_all_crash_reports(app: AppHandle) -> Result<u32, String> {
    crash_report_service::delete_all_crash_reports(&app)
}

/// Exports a crash report as a formatted Markdown string.
///
/// The returned string is formatted for pasting into a GitHub issue
/// with headers, code blocks, and metadata sections.
#[tauri::command]
pub fn export_crash_report(app: AppHandle, id: String) -> Result<String, String> {
    crash_report_service::export_crash_report(&app, &id)
}

/// Logs a frontend error as a crash report.
///
/// Called by the React ErrorBoundary, `window.onerror`, and
/// `unhandledrejection` handlers to persist frontend errors to
/// the crash report system. This ensures frontend errors are
/// captured alongside Rust panics for unified diagnostics.
///
/// # Arguments
/// * `source` -- Error source: `"frontend_error"` or `"unhandled_rejection"`
/// * `message` -- The error message string
/// * `stack` -- Optional JavaScript stack trace
/// * `component_stack` -- Optional React component stack (from ErrorBoundary)
/// * `url` -- Optional URL/page where the error occurred
#[tauri::command]
pub fn log_frontend_error(
    app: AppHandle,
    source: String,
    message: String,
    stack: Option<String>,
    component_stack: Option<String>,
    url: Option<String>,
) -> Result<String, String> {
    let mut context = HashMap::new();
    if let Some(ref cs) = component_stack {
        context.insert("component_stack".to_string(), cs.clone());
    }
    if let Some(ref u) = url {
        context.insert("url".to_string(), u.clone());
    }

    let report = CrashReport {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        source,
        panic_message: Some(message),
        location: None,
        backtrace: stack,
        context,
    };

    // Also log to the tracing file log for correlation
    log::error!(
        "Frontend error: {}",
        report.panic_message.as_deref().unwrap_or("unknown")
    );

    let report_id = report.id.clone();
    crash_report_service::save_frontend_crash_report(&app, report)?;
    Ok(report_id)
}

/// Builds and returns a pre-filled GitHub new-issue URL for a crash report.
///
/// The URL opens `github.com/MWBMPartners/MeedyaDL/issues/new` with
/// pre-filled title, body (Markdown-formatted crash details), and labels
/// (`bug`, `crash-report`). The body is truncated if it would exceed
/// browser URL length limits (~3500 chars raw).
///
/// Called by the frontend's CrashReportDialog "Open GitHub Issue" button.
/// The URL is opened in the user's default browser via the Tauri shell
/// plugin, where the user reviews and submits the issue.
#[tauri::command]
pub fn get_github_issue_url(app: AppHandle, id: String) -> Result<String, String> {
    crash_report_service::build_github_issue_url(&app, &id)
}

/// Compose a diagnostic bundle (#572 Phase 1).
///
/// Takes a caller-supplied input bundle (activity log slice + version
/// list + optional summary) and returns a pre-filled GitHub issue
/// URL plus the redacted Markdown body for review.
///
/// Privacy-first: no credentials, no file contents, no auto-submit.
/// See [`crate::services::diagnostic_bundle`] for the full contract.
///
/// **Frontend caller:** `buildDiagnosticBundle(input)` in
/// `src/lib/tauri-commands.ts`, wired to the
/// `Settings > Advanced > Diagnostics > Generate Bundle` button.
#[tauri::command]
pub fn build_diagnostic_bundle(
    app: AppHandle,
    input: crate::services::diagnostic_bundle::DiagnosticBundleInput,
) -> Result<crate::services::diagnostic_bundle::DiagnosticBundle, String> {
    crate::services::diagnostic_bundle::build_diagnostic_bundle(&app, input)
}

/// Cleans a piece of free text before it is placed into a pre-filled
/// issue on a THIRD PARTY's public GitHub repository — today that
/// means `glomatico/gamdl` (issue #1231, "Report this bug to GAMDL" in
/// the History/Queue error display).
///
/// A download error's text routinely carries a file path, and on most
/// computers a file path carries the person's own account name (for
/// example `/Users/<name>/Music/...`). It can also carry a web address
/// whose query string embeds a sign-in token (the wrapper account
/// address is the recurring example in this codebase). Both are the
/// exact two shapes MeedyaDL already knows how to clean before a crash
/// report reaches a public GitHub issue —
/// [`crate::services::diagnostic_bundle::redact_path_usernames`] and
/// [`crate::services::crash_report_service::redact_urls_in_text`] — so
/// this reuses them rather than writing a second cleaner in
/// TypeScript, which the maintainer's standing rule against duplicate
/// logic across the IPC boundary rules out anyway.
///
/// Order matters a little: usernames are stripped first (the same
/// order `build_github_issue_url`'s crash-report path already uses),
/// then URLs — a path never contains a `?` query string for the URL
/// pass to catch, and a URL never contains `/Users/<name>/` for the
/// username pass to catch, so in practice the order is safe either
/// way; kept identical to the existing precedent so a reader comparing
/// the two call sites sees the same shape.
///
/// This is deliberately narrow: it does not touch settings values, and
/// it does not attempt to find every kind of secret a message could in
/// principle carry — only the two shapes named above. A message with
/// neither comes back byte-for-byte unchanged.
///
/// Returns the cleaned text directly (not wrapped in `Result`) because
/// neither redaction step has a failure mode of its own — both are
/// pure string transforms over caller-supplied text. The frontend
/// still wraps its call in a try/catch: an IPC call can fail for
/// reasons that have nothing to do with this function's own logic
/// (the WebView losing its bridge, for instance), and the caller
/// treats that the same way it would treat a real failure here — by
/// refusing to open the link with unredacted text, not by falling
/// back to sending the raw message.
/// Cleans ONE web address before it goes into a public report: the
/// query string and any sign-in details are removed, then user names in
/// file paths, as for the message.
///
/// A separate command from [`redact_for_public_report`] on purpose. That
/// one finds web addresses inside free text, and has to decide where an
/// address ends -- it stops at a quote mark, because error text usually
/// wraps addresses in quotes. Given a whole address that contains an
/// apostrophe (`...?token=abc'SECRET`), it cleaned only up to the
/// apostrophe and left the rest (Codex, review of 9257863f). Here the
/// whole value is known to be one address, so it is cleaned as one.
#[tauri::command]
pub fn redact_url_for_public_report(url: String) -> String {
    let cleaned = crash_report_service::redact_single_url(url.trim());
    crate::services::diagnostic_bundle::redact_path_usernames(&cleaned)
}

#[tauri::command]
pub fn redact_for_public_report(text: String) -> String {
    let without_usernames = crate::services::diagnostic_bundle::redact_path_usernames(&text);
    crash_report_service::redact_urls_in_text(&without_usernames)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A whole address with an apostrophe in its query is cleaned whole
    /// (Codex, review of 9257863f).
    #[test]
    fn a_whole_address_is_cleaned_as_one_address() {
        let out = redact_url_for_public_report(
            "https://music.apple.com/us/album/a/1?token=abc'SECRET".to_string(),
        );
        assert_eq!(out, "https://music.apple.com/us/album/a/1");
    }
}
