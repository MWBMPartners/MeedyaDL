// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Activity log IPC commands (#541).
// =================================
//
// Exposes the persistent on-disk activity log (`activity-*.log` files
// written by `services::activity_log_writer`) to the frontend.
//
// Two commands:
//
// * `export_disk_activity_log(days_back)` — opens a native save dialog
//   and writes the concatenated contents of the last `days_back` daily
//   log files (default 3). Unlike `export_activity_log` in `gamdl.rs`
//   (which only exports the in-memory, possibly-trimmed entries), this
//   captures the complete forensic record.
//
// * `reveal_logs_folder()` — opens the logs directory in Finder /
//   Explorer / the default Linux file manager so the user can
//   browse / attach files to bug reports directly.

use tauri::AppHandle;

use crate::services::{activity_log_writer, config_service};
use crate::utils::platform::get_app_data_dir;

/// Default number of days to include in an export when the frontend
/// doesn't specify. Three days is usually enough to capture a bug
/// reproduction session plus context.
const DEFAULT_DAYS_BACK: u32 = 3;

/// Resolves the active activity log directory, mirroring the logic in
/// `lib.rs::resolve_activity_log_dir` but without any side-effects
/// (doesn't create the directory). Returns the default
/// `{app_data_dir}/logs/` when the override is empty or missing.
fn active_log_dir(app: &AppHandle) -> std::path::PathBuf {
    let app_data_dir = get_app_data_dir(app);
    let override_path = config_service::load_settings(app)
        .ok()
        .map(|s| s.activity_log_path_override)
        .unwrap_or_default();
    let trimmed = override_path.trim();
    if trimmed.is_empty() {
        app_data_dir.join("logs")
    } else {
        std::path::PathBuf::from(trimmed)
    }
}

/// Concatenates the most recent `days_back` on-disk activity log files
/// (newest first) and saves the result via a native save dialog.
///
/// # Arguments
/// * `days_back` — Number of daily log files to include (default 3).
///   Passing 0 falls back to the default.
///
/// # Returns
/// * `Ok(usize)` — Number of bytes written to the chosen file.
/// * `Err(String)` — Dialog cancelled, no files found, or I/O error.
#[tauri::command]
pub async fn export_disk_activity_log(
    app: AppHandle,
    days_back: Option<u32>,
) -> Result<usize, String> {
    use tauri_plugin_dialog::DialogExt;

    let days = days_back.unwrap_or(DEFAULT_DAYS_BACK).max(1) as usize;
    let log_dir = active_log_dir(&app);
    let files = activity_log_writer::list_log_files(&log_dir);

    if files.is_empty() {
        return Err(format!(
            "No on-disk activity log files found in {}",
            log_dir.display()
        ));
    }

    // Take the newest `days` files; `list_log_files` already returns
    // them in descending date order.
    let selected: Vec<_> = files.into_iter().take(days).collect();

    // Concatenate in chronological order (oldest first) so the exported
    // file reads top-to-bottom as the session unfolded.
    let mut buf = String::new();
    buf.push_str(&format!(
        "MeedyaDL On-Disk Activity Log — Exported {} (last {} day(s))\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        selected.len()
    ));
    buf.push_str(&format!("Source directory: {}\n\n", log_dir.display()));

    for path in selected.iter().rev() {
        buf.push_str(&format!(
            "==== {} ====\n",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "<unnamed>".to_string())
        ));
        match std::fs::read_to_string(path) {
            Ok(contents) => buf.push_str(&contents),
            Err(e) => buf.push_str(&format!("(failed to read file: {e})\n")),
        }
        if !buf.ends_with('\n') {
            buf.push('\n');
        }
    }

    let default_name = chrono::Local::now()
        .format("MeedyaDL-activity-disk_%Y-%m-%d_%Hh%Mm.log")
        .to_string();

    let file_path = app
        .dialog()
        .file()
        .add_filter("Log File", &["log"])
        .set_file_name(&default_name)
        .blocking_save_file();

    match file_path {
        Some(path) => {
            let resolved = path
                .as_path()
                .ok_or_else(|| "Failed to resolve export file path".to_string())?;
            std::fs::write(resolved, &buf)
                .map_err(|e| format!("Failed to write log file: {e}"))?;
            let bytes = buf.len();
            log::info!(
                "Exported {} byte(s) of on-disk activity log to {}",
                bytes,
                resolved.display()
            );
            Ok(bytes)
        }
        None => Err("Export cancelled".to_string()),
    }
}

/// Returns the absolute path of the active on-disk activity log
/// directory, creating it if necessary. The frontend passes the
/// returned path to `@tauri-apps/plugin-shell`'s `open()` to reveal
/// it in Finder / Explorer / the default Linux file manager (same
/// pattern as `QueueItem`'s "Open Folder" action).
///
/// Doing the `open()` on the frontend avoids the deprecated Rust-side
/// `Shell::open` API and keeps the platform-specific reveal logic
/// in the JS plugin where it already lives.
#[tauri::command]
pub async fn get_logs_folder_path(app: AppHandle) -> Result<String, String> {
    let log_dir = active_log_dir(&app);
    // Create the directory if it doesn't exist yet — avoids a
    // "folder not found" dialog on a brand-new install.
    std::fs::create_dir_all(&log_dir)
        .map_err(|e| format!("Failed to create log directory: {e}"))?;
    Ok(log_dir.to_string_lossy().into_owned())
}
