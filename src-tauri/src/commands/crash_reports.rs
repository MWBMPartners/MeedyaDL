// Copyright (c) 2026 MeedyaSuite
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
