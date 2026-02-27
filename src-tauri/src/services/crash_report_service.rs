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

/// Saves a crash report from the frontend to a JSON file on disk.
///
/// Called by the `log_frontend_error` IPC command when the React frontend
/// catches an error via ErrorBoundary, window.onerror, or unhandledrejection.
pub fn save_frontend_crash_report(app: &AppHandle, report: CrashReport) -> Result<(), String> {
    let dir = crashes_dir(app);
    let filename = format!("crash-{}.json", report.timestamp.replace(':', "-"));
    let path = dir.join(filename);

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("Failed to serialize crash report: {e}"))?;

    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write crash report: {e}"))?;

    log::info!("Frontend crash report saved: {}", path.display());
    Ok(())
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
