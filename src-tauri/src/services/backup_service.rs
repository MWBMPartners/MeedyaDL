// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Snapshot + restore for MeedyaDL's essential state (#466).
//!
//! Bundles `settings.json`, `queue.json`, and `history.json` into a
//! single timestamped zip-less directory under
//! `{app_data_dir}/backups/YYYYMMDD-HHMMSS/`. We keep the **last 10**
//! backups (configurable via the constant below) and prune older
//! snapshots after each successful write — keeps disk usage bounded
//! without growing forever in long-running installs.
//!
//! ## Why directories not zip
//!
//! The three files together are ~10–500 KB on a typical install. Zip
//! adds a Rust dependency (`zip` crate) and a binary-format opacity
//! that makes "I want to restore one file by hand" annoying. A flat
//! directory of plain JSON is trivial to inspect with any text editor,
//! works on every platform (no zip tool needed), and is well within
//! the disk-cheap regime.
//!
//! ## Restore contract
//!
//! `restore_from_backup(app, snapshot_dir)` overwrites the current
//! state files with the contents of the supplied snapshot. Each file
//! is copied atomically (`.tmp` + rename), missing source files in the
//! snapshot are skipped silently (a user might have nuked their
//! queue.json after the snapshot was taken — we still want
//! settings.json to restore). Returns a [`RestoreSummary`] reporting
//! exactly which files landed so the UI can surface a precise toast.
//!
//! Restoration does NOT trigger a settings reload — the caller is
//! expected to prompt the user to restart MeedyaDL because the
//! in-memory `SettingsCache` and the active queue lock would
//! otherwise diverge from disk.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use tauri::AppHandle;

use crate::utils::platform;

/// Files included in every snapshot. Order doesn't matter — we copy
/// each one independently and skip any that don't exist on disk at
/// snapshot time (e.g., a fresh install with no history yet).
const SNAPSHOT_FILES: &[&str] = &["settings.json", "queue.json", "history.json"];

/// Maximum number of timestamped snapshots to keep on disk. Older
/// snapshots are removed by `prune_old_backups` after each successful
/// write. Per #466.
const MAX_BACKUPS: usize = 10;

/// Resolved path of the backups root (`{app_data_dir}/backups/`).
/// Created on first call.
fn backups_root(app: &AppHandle) -> PathBuf {
    let root = platform::get_app_data_dir(app).join("backups");
    if let Err(e) = fs::create_dir_all(&root) {
        log::warn!("Failed to create backups dir {root:?}: {e}");
    }
    root
}

/// Generates a deterministic snapshot directory name like
/// `20260518-001530`. Uses UTC so backups sort correctly regardless
/// of the user's TZ (which can shift across DST).
fn snapshot_dir_name() -> String {
    Utc::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Summary returned to the caller (and on to the frontend via IPC).
/// Lists every file that was successfully copied into the snapshot.
#[derive(Debug, Serialize)]
pub struct BackupSummary {
    /// Absolute path of the snapshot directory.
    pub snapshot_path: String,
    /// Files that were successfully written into the snapshot.
    pub files: Vec<String>,
    /// Total snapshots on disk after this write (post-prune).
    pub total_snapshots: usize,
}

/// Summary returned by `restore_from_backup`. Reports which files
/// landed and which were missing in the snapshot.
#[derive(Debug, Serialize)]
pub struct RestoreSummary {
    pub snapshot_path: String,
    pub restored: Vec<String>,
    pub skipped: Vec<String>,
}

/// Listing entry for the UI's backup picker. One per existing
/// snapshot directory.
#[derive(Debug, Serialize)]
pub struct BackupEntry {
    /// Absolute path of the snapshot directory (used as the restore
    /// target).
    pub path: String,
    /// Directory name (e.g., `20260518-001530`). The UI parses this
    /// to render a human-friendly timestamp.
    pub name: String,
    /// Size of the snapshot in bytes (sum of contained files).
    pub size_bytes: u64,
    /// Number of files in the snapshot (0–3 in practice).
    pub file_count: usize,
}

/// Take a snapshot of the current state files. Called at app exit
/// (best-effort — never blocks shutdown) and on-demand from the
/// Settings > Tools UI. After writing the snapshot, prunes any
/// snapshots beyond [`MAX_BACKUPS`].
pub fn create_backup(app: &AppHandle) -> Result<BackupSummary, String> {
    let app_data = platform::get_app_data_dir(app);
    let root = backups_root(app);
    let snapshot_dir = root.join(snapshot_dir_name());

    fs::create_dir_all(&snapshot_dir)
        .map_err(|e| format!("Failed to create snapshot dir {snapshot_dir:?}: {e}"))?;

    let mut copied = Vec::new();
    for name in SNAPSHOT_FILES {
        let source = app_data.join(name);
        if !source.exists() {
            continue;
        }
        let dest = snapshot_dir.join(name);
        match fs::copy(&source, &dest) {
            Ok(_) => copied.push((*name).to_string()),
            Err(e) => {
                log::warn!("Backup: failed to copy {name}: {e}");
            }
        }
    }

    if copied.is_empty() {
        // Nothing to back up — remove the empty dir we just made so
        // the listing doesn't fill with phantom snapshots.
        let _ = fs::remove_dir(&snapshot_dir);
        return Err("Nothing to back up — no state files exist yet.".to_string());
    }

    prune_old_backups(&root, MAX_BACKUPS);
    let total = list_backups(app).map(|v| v.len()).unwrap_or(0);

    Ok(BackupSummary {
        snapshot_path: snapshot_dir.to_string_lossy().into_owned(),
        files: copied,
        total_snapshots: total,
    })
}

/// Restore the supplied snapshot directory into `{app_data_dir}`.
/// Missing source files in the snapshot are skipped (not errored) so
/// a partial snapshot from an earlier MeedyaDL version still restores
/// what it has.
///
/// Writes are atomic (`.tmp` + rename) so a crash mid-restore can't
/// leave a truncated `settings.json` that the load path would refuse.
pub fn restore_from_backup(app: &AppHandle, snapshot_dir: &Path) -> Result<RestoreSummary, String> {
    if !snapshot_dir.exists() || !snapshot_dir.is_dir() {
        return Err(format!(
            "Snapshot directory does not exist: {}",
            snapshot_dir.display()
        ));
    }

    let app_data = platform::get_app_data_dir(app);
    let mut restored = Vec::new();
    let mut skipped = Vec::new();

    for name in SNAPSHOT_FILES {
        let source = snapshot_dir.join(name);
        if !source.exists() {
            skipped.push((*name).to_string());
            continue;
        }
        let dest = app_data.join(name);
        let tmp = app_data.join(format!("{name}.restore.tmp"));
        match fs::copy(&source, &tmp).and_then(|_| fs::rename(&tmp, &dest)) {
            Ok(_) => restored.push((*name).to_string()),
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                log::warn!("Restore: failed to write {name}: {e}");
                skipped.push((*name).to_string());
            }
        }
    }

    Ok(RestoreSummary {
        snapshot_path: snapshot_dir.to_string_lossy().into_owned(),
        restored,
        skipped,
    })
}

/// Enumerate every snapshot directory under `{app_data_dir}/backups/`.
/// Returns newest-first so the UI picker shows recent snapshots at the
/// top. Silently skips entries that don't look like snapshot dirs
/// (e.g., user-dropped files).
pub fn list_backups(app: &AppHandle) -> Result<Vec<BackupEntry>, String> {
    let root = backups_root(app);
    let mut entries = Vec::new();

    let dir_iter = match fs::read_dir(&root) {
        Ok(it) => it,
        Err(e) => return Err(format!("Failed to read backups dir: {e}")),
    };

    for entry in dir_iter.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }

        let (file_count, size_bytes) = match fs::read_dir(&path) {
            Ok(it) => {
                let mut count = 0_usize;
                let mut size = 0_u64;
                for f in it.flatten() {
                    if f.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                        count += 1;
                        if let Ok(meta) = f.metadata() {
                            size += meta.len();
                        }
                    }
                }
                (count, size)
            }
            Err(_) => continue,
        };

        if file_count == 0 {
            continue;
        }

        entries.push(BackupEntry {
            path: path.to_string_lossy().into_owned(),
            name,
            size_bytes,
            file_count,
        });
    }

    // Newest first. Names follow YYYYMMDD-HHMMSS so lexicographic
    // sort is equivalent to chronological.
    entries.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(entries)
}

/// Remove a specific snapshot directory by absolute path. Used by the
/// UI's per-entry delete button.
pub fn delete_backup(snapshot_dir: &Path) -> Result<(), String> {
    if !snapshot_dir.exists() {
        return Ok(());
    }
    fs::remove_dir_all(snapshot_dir)
        .map_err(|e| format!("Failed to delete snapshot {snapshot_dir:?}: {e}"))
}

/// Trim the backups directory down to `keep` most-recent snapshots.
/// Older snapshots are removed in lexicographic order (oldest first).
fn prune_old_backups(root: &Path, keep: usize) {
    let mut dirs: Vec<PathBuf> = match fs::read_dir(root) {
        Ok(it) => it
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.is_dir() {
                    Some(p)
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => return,
    };

    // Sort ascending by name (YYYYMMDD-HHMMSS → chronological).
    dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    if dirs.len() <= keep {
        return;
    }
    let to_remove = dirs.len() - keep;
    for dir in dirs.into_iter().take(to_remove) {
        if let Err(e) = fs::remove_dir_all(&dir) {
            log::warn!("Failed to prune old backup {dir:?}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;

    fn touch(p: &Path, content: &str) {
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        write(p, content).unwrap();
    }

    #[test]
    fn prune_keeps_only_newest() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // Create 12 timestamp-named dirs in lex order.
        for i in 0..12 {
            let dir = root.join(format!("2026010{:02}-000000", i));
            std::fs::create_dir(&dir).unwrap();
            touch(&dir.join("settings.json"), "{}");
        }

        prune_old_backups(root, 10);

        let remaining: Vec<String> = std::fs::read_dir(root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(remaining.len(), 10, "should keep the 10 newest only");
        // Oldest two (00, 01) should be gone; newest (11) should remain.
        assert!(!remaining.iter().any(|n| n.ends_with("00-000000")));
        assert!(!remaining.iter().any(|n| n.ends_with("01-000000")));
        assert!(remaining.iter().any(|n| n.ends_with("11-000000")));
    }

    #[test]
    fn snapshot_dir_name_is_utc_timestamp_pattern() {
        let name = snapshot_dir_name();
        // Expected shape: YYYYMMDD-HHMMSS (15 chars including hyphen).
        assert_eq!(name.len(), 15, "name = {name:?}");
        assert!(name.chars().nth(8) == Some('-'));
        assert!(name[..8].chars().all(|c| c.is_ascii_digit()));
        assert!(name[9..].chars().all(|c| c.is_ascii_digit()));
    }
}
