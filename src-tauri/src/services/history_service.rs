// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download history service.
// ==========================
//
// Provides persistent download history stored as a JSON file in the
// app data directory (`{app_data_dir}/history.json`). Each completed
// or terminally failed download is recorded as a `HistoryEntry`.
//
// Design decisions:
//   - JSON file (not SQLite) for consistency with `queue.json` and to
//     avoid adding a new dependency (tauri-plugin-sql).
//   - Maximum 1000 entries; oldest are trimmed on save.
//   - All entries are loaded into memory; filtering is done in Rust
//     (no pagination needed for 1000 entries).
//   - Clone-then-release pattern for Mutex-free file I/O (no shared
//     state needed -- the file is the source of truth).

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

/// Maximum number of history entries retained on disk.
/// When this limit is exceeded, the oldest entries are trimmed.
const MAX_HISTORY_ENTRIES: usize = 1000;

fn normalize_url_for_history_dedup(url: &str) -> String {
    crate::services::download_queue::normalize_url_for_dedup(url)
}

// ============================================================
// Data model
// ============================================================

/// A single download history entry.
///
/// Recorded after a download reaches a terminal state (success or
/// failure). Contains enough metadata to identify the download and
/// its outcome without storing the full queue item state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// The Apple Music URL that was downloaded.
    pub url: String,
    /// Track or album title (if known from GAMDL output).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Artist name (if known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    /// Album name (if known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    /// Audio codec used (e.g., "alac", "aac", "atmos").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    /// Absolute path to the output file or directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// ISO 8601 timestamp when the download was queued.
    pub started_at: String,
    /// ISO 8601 timestamp when the download completed or failed.
    pub completed_at: String,
    /// Outcome: `"success"` or `"failed"`.
    pub status: String,
    /// Error message (only for failed downloads).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

// ============================================================
// File I/O helpers
// ============================================================

/// Returns the path to the history JSON file.
fn history_path(app: &AppHandle) -> std::path::PathBuf {
    crate::utils::platform::get_app_data_dir(app).join("history.json")
}

/// Loads all history entries from disk.
///
/// Returns an empty `Vec` on missing or invalid file (graceful
/// degradation for first run or file corruption).
fn load_history_from_disk(app: &AppHandle) -> Vec<HistoryEntry> {
    let path = history_path(app);
    std::fs::read_to_string(&path).map_or_else(
        |_| vec![],
        |json| match serde_json::from_str::<Vec<HistoryEntry>>(&json) {
            Ok(entries) => {
                let original_len = entries.len();
                let entries = dedupe_history_entries(entries);
                if entries.len() != original_len {
                    log::info!(
                        "Cleaned up {} duplicate history {}",
                        original_len - entries.len(),
                        if original_len - entries.len() == 1 {
                            "entry"
                        } else {
                            "entries"
                        }
                    );
                    save_history_to_disk(app, &entries);
                }
                entries
            }
            Err(e) => {
                log::debug!("Failed to parse history.json: {e}");
                vec![]
            }
        },
    )
}

/// Writes the history entries to disk as a JSON array.
///
/// Migrated to the shared `utils::atomic_write::atomic_write_json`
/// helper (#716/8, v1.0.4 prep). This is a small **durability
/// upgrade** over the pre-migration code: the previous implementation
/// did a non-atomic `fs::write`, which on crash / power-loss could
/// leave a truncated history.json that the load path would refuse to
/// parse (and silently start with an empty history). The new helper
/// uses the canonical write-tmp + rename pattern that's atomic on
/// APFS / ext4 / NTFS — either the old history is intact or the new
/// one is fully written.
fn save_history_to_disk(app: &AppHandle, entries: &[HistoryEntry]) {
    let path = history_path(app);
    if let Err(e) = crate::utils::atomic_write::atomic_write_json(&path, entries, "history") {
        log::warn!("{e}");
    }
}

fn dedupe_history_entries(mut entries: Vec<HistoryEntry>) -> Vec<HistoryEntry> {
    entries.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));

    let mut seen = std::collections::HashSet::new();
    entries.retain(|entry| seen.insert(normalize_url_for_history_dedup(&entry.url)));
    entries
}

// ============================================================
// Public API
// ============================================================

/// Returns all history entries, sorted newest first.
pub fn list_history(app: &AppHandle) -> Vec<HistoryEntry> {
    let mut entries = load_history_from_disk(app);
    // Sort by completed_at descending (newest first)
    entries.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));
    entries
}

/// Appends a new entry to the history file.
///
/// If the total number of entries exceeds `MAX_HISTORY_ENTRIES`, the
/// oldest entries are trimmed to stay within the limit.
pub fn save_history_entry(app: &AppHandle, entry: HistoryEntry) {
    let mut entries = load_history_from_disk(app);
    let incoming_url = normalize_url_for_history_dedup(&entry.url);
    entries.retain(|existing| normalize_url_for_history_dedup(&existing.url) != incoming_url);
    entries.push(entry);

    // Sort newest first, then trim to the limit
    entries.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));
    entries.truncate(MAX_HISTORY_ENTRIES);

    save_history_to_disk(app, &entries);
}

/// Removes any history rows whose URL matches one of the supplied URLs.
///
/// Used when a history row is retried/requeued: the fresh queue attempt
/// becomes the live source of truth, and the new terminal result will write
/// back a single replacement history row when it finishes.
pub fn remove_entries_for_urls(app: &AppHandle, urls: &[String]) {
    if urls.is_empty() {
        return;
    }

    let incoming: std::collections::HashSet<String> =
        urls.iter().map(|url| normalize_url_for_history_dedup(url)).collect();
    let mut entries = load_history_from_disk(app);
    let original_len = entries.len();
    entries.retain(|entry| !incoming.contains(&normalize_url_for_history_dedup(&entry.url)));

    if entries.len() != original_len {
        save_history_to_disk(app, &entries);
    }
}

/// Compacts history on startup after upgrades that may have produced
/// duplicate rows in earlier versions.
pub fn cleanup_duplicate_entries(app: &AppHandle) {
    let _ = load_history_from_disk(app);
}

/// Removes a single history entry by its ID.
///
/// Returns `true` if an entry matched and was removed, `false` if no entry
/// with the given ID was found (the file is left untouched in the latter
/// case to avoid an unnecessary disk write).
pub fn delete_entry(app: &AppHandle, id: &str) -> bool {
    let mut entries = load_history_from_disk(app);
    let original_len = entries.len();
    entries.retain(|entry| entry.id != id);

    if entries.len() != original_len {
        save_history_to_disk(app, &entries);
        true
    } else {
        false
    }
}

/// Deletes all history entries.
pub fn clear_history(app: &AppHandle) {
    let path = history_path(app);
    if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("Failed to remove history.json: {e}");
        }
    }
}

/// Searches history entries by a case-insensitive substring match
/// on the title, artist, album, and URL fields.
///
/// Returns matching entries sorted newest first.
pub fn search_history(app: &AppHandle, query: &str) -> Vec<HistoryEntry> {
    let query_lower = query.to_lowercase();
    let entries = load_history_from_disk(app);

    let mut results: Vec<HistoryEntry> = entries
        .into_iter()
        .filter(|e| {
            e.url.to_lowercase().contains(&query_lower)
                || e.title
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().contains(&query_lower))
                || e.artist
                    .as_deref()
                    .is_some_and(|a| a.to_lowercase().contains(&query_lower))
                || e.album
                    .as_deref()
                    .is_some_and(|a| a.to_lowercase().contains(&query_lower))
        })
        .collect();

    results.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));
    results
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a test `HistoryEntry` with the given title and status.
    fn make_entry(title: &str, status: &str) -> HistoryEntry {
        HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            url: format!("https://music.apple.com/us/album/{title}/123"),
            title: Some(title.to_string()),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            codec: Some("alac".to_string()),
            file_path: None,
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: chrono::Utc::now().to_rfc3339(),
            status: status.to_string(),
            error_message: None,
        }
    }

    #[test]
    fn max_entries_constant_is_reasonable() {
        assert_eq!(MAX_HISTORY_ENTRIES, 1000);
    }

    #[test]
    fn history_entry_serialization_roundtrip() {
        let entry = make_entry("Test Song", "success");
        let json = serde_json::to_string(&entry).expect("serialize");
        let deserialized: HistoryEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.id, entry.id);
        assert_eq!(deserialized.url, entry.url);
        assert_eq!(deserialized.title, entry.title);
        assert_eq!(deserialized.status, entry.status);
    }

    #[test]
    fn optional_fields_can_be_omitted() {
        let json = r#"{
            "id": "test-id",
            "url": "https://example.com",
            "started_at": "2024-01-01T00:00:00Z",
            "completed_at": "2024-01-01T00:01:00Z",
            "status": "success"
        }"#;
        let entry: HistoryEntry = serde_json::from_str(json).expect("deserialize");
        assert!(entry.title.is_none());
        assert!(entry.artist.is_none());
        assert!(entry.album.is_none());
        assert!(entry.codec.is_none());
        assert!(entry.file_path.is_none());
        assert!(entry.error_message.is_none());
    }

    #[test]
    fn dedupe_history_entries_keeps_newest_matching_url() {
        let mut older = make_entry("Older", "failed");
        older.url = "https://music.apple.com/us/album/test/123?ls=1".to_string();
        older.completed_at = "2026-05-04T10:00:00Z".to_string();

        let mut newer = make_entry("Newer", "success");
        newer.url = "https://Music.Apple.Com/us/album/test/123/".to_string();
        newer.completed_at = "2026-05-04T11:00:00Z".to_string();

        let entries = dedupe_history_entries(vec![older, newer.clone()]);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, newer.id);
    }
}
