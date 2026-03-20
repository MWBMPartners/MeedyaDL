// Copyright (c) 2026 MeedyaDL
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
