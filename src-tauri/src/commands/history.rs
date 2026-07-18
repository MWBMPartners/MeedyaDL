// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download history IPC command handlers.
// ========================================
//
// Thin wrappers around `services::history_service` that expose
// history operations to the React frontend via Tauri's `invoke()`.
//
// Commands:
//   - `list_history`   -- Returns all history entries (newest first)
//   - `clear_history`  -- Deletes all history entries
//   - `search_history` -- Case-insensitive search on title/artist/album/URL

use tauri::AppHandle;

use crate::services::history_service;
use crate::services::history_service::HistoryEntry;

/// Returns all download history entries, sorted newest first.
///
/// Called by the frontend to populate the History page.
#[tauri::command]
pub fn list_history(app: AppHandle) -> Vec<HistoryEntry> {
    history_service::list_history(&app)
}

/// Deletes all download history entries from disk.
///
/// Called by the frontend's "Clear History" button.
#[tauri::command]
pub fn clear_history(app: AppHandle) -> Result<(), String> {
    history_service::clear_history(&app);
    Ok(())
}

/// Searches history entries by a case-insensitive substring match
/// on title, artist, album, and URL fields.
///
/// Called by the frontend's search input on the History page.
#[tauri::command]
pub fn search_history(app: AppHandle, query: String) -> Vec<HistoryEntry> {
    history_service::search_history(&app, &query)
}

/// Removes a single history entry by ID (#685).
///
/// Sibling of `clear_history` (bulk). Returns `Err` only when the ID
/// doesn't match any row, so the frontend can surface "already gone"
/// distinctly from a no-op.
#[tauri::command]
pub fn delete_history_entry(app: AppHandle, id: String) -> Result<(), String> {
    if history_service::delete_entry(&app, &id) {
        Ok(())
    } else {
        Err(format!("History entry {id} not found"))
    }
}

/// Aggregate lifetime download analytics (#464). Returns totals,
/// success rate, codec distribution, top artist / album, and
/// last-7-day activity computed on-demand from the persistent history.
///
/// **Frontend caller:** `getLifetimeStats()` in
/// `src/lib/tauri-commands.ts`, wired to the StatisticsPanel.
///
/// Pure read — no side effects, safe to call as often as the UI
/// likes. Computed on each call rather than cached because the
/// history is small (≤1000 entries per MAX_HISTORY_ENTRIES) and
/// the aggregation is sub-ms.
#[tauri::command]
pub fn get_lifetime_stats(
    app: AppHandle,
) -> crate::services::stats_service::LifetimeStats {
    crate::services::stats_service::get_lifetime_stats(&app)
}

/// Resolves the folder to reveal for a history entry's stored path (#992).
///
/// `HistoryEntry.file_path` is ambiguous by construction: album downloads
/// record the album *directory*, while single-file downloads record the
/// *file* itself. There's no `output_is_directory` flag on the struct (and
/// older rows predate any such flag), so this stats the path directly —
/// the path itself if it is a directory, otherwise its parent.
///
/// Called by the frontend's "Open Folder" action instead of the old
/// unconditional `path.replace(/[/\\][^/\\]+$/, '')` regex strip, which
/// incorrectly climbed one level above album directories and revealed the
/// parent artist folder instead of the album folder.
///
/// Falls back to the input string unchanged if it has no parent (e.g. a
/// root path) — the caller's regex-based fallback handles the case where
/// this command itself is unreachable (IPC failure).
#[tauri::command]
pub fn resolve_reveal_path(file_path: String) -> String {
    let p = std::path::Path::new(&file_path);
    if p.is_dir() {
        file_path
    } else {
        p.parent()
            .map(|q| q.to_string_lossy().into_owned())
            .unwrap_or(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real directory resolves to itself — this is the album-download
    /// case that #992 was about (the stored path IS the folder to reveal).
    #[test]
    fn resolve_reveal_path_returns_directory_itself() {
        let dir = tempfile::TempDir::new().unwrap();
        let dir_path = dir.path().to_string_lossy().into_owned();

        let resolved = resolve_reveal_path(dir_path.clone());

        assert_eq!(resolved, dir_path);
    }

    /// A real file resolves to its parent directory — the single-file
    /// download case, matching the old regex-strip behaviour.
    #[test]
    fn resolve_reveal_path_returns_parent_of_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let file_path = dir.path().join("track.m4a");
        std::fs::write(&file_path, b"fake audio").unwrap();

        let resolved = resolve_reveal_path(file_path.to_string_lossy().into_owned());

        assert_eq!(resolved, dir.path().to_string_lossy().into_owned());
    }

    /// A nonexistent path (e.g. the user moved/deleted the file after
    /// download) with a parent still resolves to that parent — `is_dir()`
    /// is `false` for missing paths, so the file branch (parent) applies.
    /// This preserves today's UX: users can still open the containing
    /// folder even if the exact file is gone.
    #[test]
    fn resolve_reveal_path_nonexistent_path_returns_parent() {
        let dir = tempfile::TempDir::new().unwrap();
        let missing_path = dir.path().join("no-such-file.m4a");

        let resolved = resolve_reveal_path(missing_path.to_string_lossy().into_owned());

        assert_eq!(resolved, dir.path().to_string_lossy().into_owned());
    }
}
