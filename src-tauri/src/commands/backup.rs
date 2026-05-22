// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! IPC commands for snapshot + restore (#466).
//!
//! Thin wrappers around `services::backup_service`. Each command runs
//! its work on a blocking thread (the snapshot copies and `read_dir`
//! traversals are filesystem-bound; we don't want them on the main
//! Tauri runtime).

use std::path::PathBuf;
use tauri::AppHandle;

use crate::services::backup_service::{
    self, BackupEntry, BackupSummary, RestoreSummary,
};
use crate::utils::activity_log::emit_app_log;

/// Take a snapshot of settings / queue / history into
/// `{app_data_dir}/backups/<timestamp>/`.
///
/// **Frontend caller:** `createBackup()` in `src/lib/tauri-commands.ts`.
#[tauri::command]
pub async fn create_backup(app: AppHandle) -> Result<BackupSummary, String> {
    let app_clone = app.clone();
    let summary = tauri::async_runtime::spawn_blocking(move || {
        backup_service::create_backup(&app_clone)
    })
    .await
    .map_err(|e| format!("Backup task panicked: {e}"))??;

    emit_app_log(
        &app,
        &format!(
            "Created backup with {} file(s) — {} total snapshot(s) on disk",
            summary.files.len(),
            summary.total_snapshots,
        ),
    );
    Ok(summary)
}

/// Enumerate every snapshot directory under
/// `{app_data_dir}/backups/`. Newest first.
///
/// **Frontend caller:** `listBackups()` in `src/lib/tauri-commands.ts`.
#[tauri::command]
pub async fn list_backups(app: AppHandle) -> Result<Vec<BackupEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || backup_service::list_backups(&app))
        .await
        .map_err(|e| format!("List backups task panicked: {e}"))?
}

/// Restore a specific snapshot directory back into
/// `{app_data_dir}`. Overwrites the live state files atomically.
///
/// The user is expected to be prompted to **restart the app** after
/// restore — the in-memory SettingsCache and live queue lock would
/// otherwise diverge from disk.
///
/// **Frontend caller:** `restoreFromBackup(snapshotPath)` in
/// `src/lib/tauri-commands.ts`.
#[tauri::command]
pub async fn restore_from_backup(
    app: AppHandle,
    snapshot_path: String,
) -> Result<RestoreSummary, String> {
    let path = PathBuf::from(&snapshot_path);
    let app_clone = app.clone();
    let summary = tauri::async_runtime::spawn_blocking(move || {
        backup_service::restore_from_backup(&app_clone, &path)
    })
    .await
    .map_err(|e| format!("Restore task panicked: {e}"))??;

    emit_app_log(
        &app,
        &format!(
            "Restored {} file(s) from {} — please restart MeedyaDL to apply.",
            summary.restored.len(),
            summary.snapshot_path,
        ),
    );
    Ok(summary)
}

/// Delete a specific snapshot directory. No-op if it doesn't exist.
///
/// **Frontend caller:** `deleteBackup(snapshotPath)` in
/// `src/lib/tauri-commands.ts`.
#[tauri::command]
pub async fn delete_backup(snapshot_path: String) -> Result<(), String> {
    let path = PathBuf::from(snapshot_path);
    tauri::async_runtime::spawn_blocking(move || backup_service::delete_backup(&path))
        .await
        .map_err(|e| format!("Delete backup task panicked: {e}"))?
}
