// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Crash report service.
// ======================
//
// Provides CRUD operations for crash report files stored in
// `{app_data_dir}/crashes/`. Crash reports are JSON files written by
// the Rust panic handler and the frontend error logging command.
//
// Functions in this module are called by the IPC command handlers in
// `commands/crash_reports.rs`.

use std::path::PathBuf;
use tauri::AppHandle;

use crate::models::crash_report::CrashReport;
use crate::utils::platform;

/// Returns the path to the crash reports directory.
/// Creates the directory if it does not exist.
fn crashes_dir(app: &AppHandle) -> PathBuf {
    let dir = platform::get_app_data_dir(app).join("crashes");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Lists all crash reports, sorted by timestamp (newest first).
///
/// Reads all `crash-*.json` files from the crashes directory and
/// deserializes them into `CrashReport` structs. Files that fail
/// to parse are silently skipped.
pub fn list_crash_reports(app: &AppHandle) -> Vec<CrashReport> {
    let dir = crashes_dir(app);
    let mut reports: Vec<CrashReport> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(report) = serde_json::from_str::<CrashReport>(&contents) {
                        reports.push(report);
                    }
                }
            }
        }
    }

    // Sort by timestamp descending (newest first)
    reports.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    reports
}

/// Retrieves a single crash report by its ID.
///
/// Scans all crash report files and returns the one matching the
/// given ID, or `None` if not found.
pub fn get_crash_report(app: &AppHandle, id: &str) -> Option<CrashReport> {
    list_crash_reports(app)
        .into_iter()
        .find(|r| r.id == id)
}

/// Deletes a crash report by its ID.
///
/// Removes the JSON file from disk. Returns `Ok(())` if the file was
/// deleted or did not exist, `Err` if the deletion failed.
pub fn delete_crash_report(app: &AppHandle, id: &str) -> Result<(), String> {
    let dir = crashes_dir(app);

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(report) = serde_json::from_str::<CrashReport>(&contents) {
                        if report.id == id {
                            std::fs::remove_file(&path)
                                .map_err(|e| format!("Failed to delete crash report: {e}"))?;
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Ok(()) // Not found is not an error
}

/// Exports a crash report as a formatted Markdown string suitable for
/// pasting into a GitHub issue or sharing with a developer.
pub fn export_crash_report(app: &AppHandle, id: &str) -> Result<String, String> {
    let report = get_crash_report(app, id)
        .ok_or_else(|| format!("Crash report not found: {id}"))?;

    let mut md = String::new();
    md.push_str("## MeedyaDL Crash Report\n\n");
    md.push_str(&format!("**ID:** `{}`\n", report.id));
    md.push_str(&format!("**Timestamp:** {}\n", report.timestamp));
    md.push_str(&format!("**App Version:** {}\n", report.app_version));
    md.push_str(&format!("**OS:** {} ({})\n", report.os, report.arch));
    md.push_str(&format!("**Source:** {}\n", report.source));

    if let Some(ref msg) = report.panic_message {
        md.push_str(&format!("\n### Error Message\n\n```\n{msg}\n```\n"));
    }

    if let Some(ref loc) = report.location {
        md.push_str(&format!("\n**Location:** `{loc}`\n"));
    }

    if let Some(ref bt) = report.backtrace {
        md.push_str(&format!("\n### Backtrace\n\n```\n{bt}\n```\n"));
    }

    if !report.context.is_empty() {
        md.push_str("\n### Context\n\n");
        for (key, value) in &report.context {
            md.push_str(&format!("- **{key}:** {value}\n"));
        }
    }

    Ok(md)
}

/// Maximum character count for the issue body before backtrace truncation.
///
/// GitHub supports ~8192 chars in a URL, and modern browsers handle even more
/// (Chrome ~32KB, Safari ~80KB, Firefox ~65KB). However, percent-encoding can
/// roughly double the size for content with backticks, newlines, and special
/// characters. A 3500-char raw body keeps the final encoded URL well under
/// 8000 characters on all platforms.
const MAX_BODY_CHARS: usize = 3500;

/// GitHub repository path used in the issue URL.
const GITHUB_REPO: &str = "MWBMPartners/MeedyaDL";

/// Builds a pre-filled GitHub new-issue URL for a crash report.
///
/// The URL opens `github.com/MWBMPartners/MeedyaDL/issues/new` with
/// query parameters for `title`, `body`, and `labels`. The body contains
/// formatted Markdown with crash details, a "Steps to Reproduce" template
/// for the user to fill in, and a footer attribution.
///
/// If the body (including full backtrace) would exceed [`MAX_BODY_CHARS`],
/// the backtrace is truncated to the first 15 + last 5 lines with a
/// `[truncated]` marker, and a note directs the user to the locally saved
/// full report.
///
/// Uses the `url` crate's `Url::parse()` + `query_pairs_mut()` for proper
/// percent-encoding of all query parameter values.
pub fn build_github_issue_url(app: &AppHandle, id: &str) -> Result<String, String> {
    let report = get_crash_report(app, id)
        .ok_or_else(|| format!("Crash report not found: {id}"))?;

    // Build the issue title -- cap error message at 80 chars for readability.
    // Use different prefix and labels for download errors vs crashes.
    let is_download_error = report.source == "download_error";
    let title_prefix = if is_download_error {
        "Error Report"
    } else {
        "Crash Report"
    };
    let error_summary: String = report
        .panic_message
        .as_deref()
        .unwrap_or("Unknown error")
        .chars()
        .take(80)
        .collect();
    let title = format!(
        "[{title_prefix}] {} - {} ({})",
        error_summary, report.os, report.arch
    );

    // Build the Markdown body
    let mut body = String::new();
    body.push_str(&format!("## {title_prefix}\n\n"));
    body.push_str(&format!("**App Version:** {}\n", report.app_version));
    body.push_str(&format!("**OS:** {} ({})\n", report.os, report.arch));
    body.push_str(&format!("**Source:** {}\n", report.source));
    body.push_str(&format!("**Timestamp:** {}\n", report.timestamp));
    body.push_str(&format!("**Report ID:** `{}`\n", report.id));

    if let Some(ref msg) = report.panic_message {
        body.push_str(&format!("\n### Error Message\n\n```\n{msg}\n```\n"));
    }

    if let Some(ref loc) = report.location {
        body.push_str(&format!("\n**Location:** `{loc}`\n"));
    }

    // Backtrace with truncation logic for URL length limits
    if let Some(ref bt) = report.backtrace {
        let lines: Vec<&str> = bt.lines().collect();
        if body.len() + bt.len() + 50 > MAX_BODY_CHARS && lines.len() > 25 {
            // Truncate: keep first 15 + last 5 lines (most diagnostic)
            let head = lines[..15].join("\n");
            let tail = lines[lines.len().saturating_sub(5)..].join("\n");
            let truncated = format!(
                "{head}\n\n... [truncated — full backtrace saved locally] ...\n\n{tail}"
            );
            body.push_str(&format!(
                "\n### Backtrace (truncated)\n\n```\n{truncated}\n```\n"
            ));
        } else {
            body.push_str(&format!("\n### Backtrace\n\n```\n{bt}\n```\n"));
        }
    }

    // Context key-value pairs
    if !report.context.is_empty() {
        body.push_str("\n### Context\n\n");
        for (key, value) in &report.context {
            body.push_str(&format!("- **{key}:** {value}\n"));
        }
    }

    // User-fillable sections and footer
    body.push_str("\n---\n\n");
    body.push_str("### Steps to Reproduce\n\n");
    body.push_str("*Please describe what you were doing when the crash occurred:*\n\n");
    body.push_str("1. \n2. \n3. \n\n");
    body.push_str("### Additional Context\n\n");
    body.push_str("*Add any other context about the problem here.*\n\n");
    body.push_str("---\n\n");
    body.push_str(&format!(
        "> *This crash report was generated by MeedyaDL v{}. \
         Full crash report saved locally with ID: `{}`*\n",
        report.app_version, report.id
    ));

    // Final hard truncation if body is still too long after backtrace
    // truncation (e.g., very long error messages or context sections)
    if body.len() > MAX_BODY_CHARS {
        body.truncate(MAX_BODY_CHARS - 100);
        body.push_str("\n\n... [body truncated for URL length limits] ...");
    }

    // Build the URL using the `url` crate for proper percent-encoding.
    // `query_pairs_mut().append_pair()` delegates to `form_urlencoded`
    // which handles all special characters (backticks, newlines, etc.).
    let base = format!("https://github.com/{GITHUB_REPO}/issues/new");
    let mut url = url::Url::parse(&base)
        .map_err(|e| format!("Failed to parse base URL: {e}"))?;

    let labels = if is_download_error {
        "bug,error-report"
    } else {
        "bug,crash-report"
    };
    url.query_pairs_mut()
        .append_pair("title", &title)
        .append_pair("body", &body)
        .append_pair("labels", labels);

    Ok(url.to_string())
}

/// Saves an error report (crash, download error, or other) to a JSON file.
///
/// This is the generic save function used by all report sources:
/// - Frontend errors (via `log_frontend_error` IPC command)
/// - Download errors (via download queue terminal failure)
/// - Rust panics (via `setup_panic_handler()`)
///
/// Reports are stored in `{app_data_dir}/crashes/` as JSON files and can
/// be listed, viewed, exported, or reported to GitHub Issues via the
/// Settings > Advanced > Crash Reporting section.
pub fn save_error_report(app: &AppHandle, report: CrashReport) -> Result<(), String> {
    let dir = crashes_dir(app);
    let filename = format!("crash-{}.json", report.timestamp.replace(':', "-"));
    let path = dir.join(filename);

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("Failed to serialize error report: {e}"))?;

    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write error report: {e}"))?;

    log::info!(
        "Error report saved: {} (source: {})",
        path.display(),
        report.source
    );
    Ok(())
}

/// Saves a crash report from the frontend to a JSON file on disk.
///
/// Called by the `log_frontend_error` IPC command when the React frontend
/// catches an error via ErrorBoundary, window.onerror, or unhandledrejection.
/// Delegates to [`save_error_report`].
pub fn save_frontend_crash_report(app: &AppHandle, report: CrashReport) -> Result<(), String> {
    save_error_report(app, report)
}

/// Deletes crash reports older than 30 days.
///
/// Called during application startup to prevent the crashes directory
/// from growing indefinitely. Uses the file's modification time rather
/// than parsing the JSON timestamp for efficiency.
pub fn clear_old_reports(app: &AppHandle) {
    let dir = crashes_dir(app);
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(30 * 24 * 60 * 60); // 30 days

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if modified < cutoff {
                            let _ = std::fs::remove_file(&path);
                            log::info!("Cleaned old crash report: {}", path.display());
                        }
                    }
                }
            }
        }
    }
}
