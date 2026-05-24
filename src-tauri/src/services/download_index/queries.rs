// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License.
//
// Download Index — read-only query helpers (#875 EPIC A M2).
//
// Thin read-only API for consumers that historically walked
// `history.json` or `manifest.meedyadl` files. Each helper opens the
// DB read-only, runs a single indexed query, and returns. SQLite is
// fast enough that per-call open is comparable to (or faster than) a
// JSON parse, and avoids us having to manage a Connection in Tauri's
// managed state (rusqlite::Connection is not Send).
//
// `.meedyadl` manifests on disk REMAIN the source of truth — these
// helpers prefer the DB when populated but every consumer also has
// a JSON-backed fallback path for the case where the DB is missing,
// corrupt, or hasn't been populated yet.

use std::collections::HashSet;
use std::path::Path;

use super::{DownloadIndex, IndexError};

/// Compact data shape returned by [`find_download_by_url`]. Captures
/// just what the redownload-status check needs — the URL is the join
/// key; we omit the heavier columns (audio_traits_json, file_sha256)
/// to keep the row narrow.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadLookup {
    pub url: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub codec: Option<String>,
    pub downloaded_at: String,
    pub last_modified_date: Option<String>,
}

/// Look up a previously-downloaded URL in the index. Returns the most
/// recent matching row (newest `downloaded_at`) when multiple codecs
/// of the same URL exist (companion downloads).
///
/// Used by `check_redownload_status` as the fast path: history.json is
/// linearly scanned in O(n); this is an indexed lookup.
pub fn find_download_by_url(
    db_path: &Path,
    url: &str,
) -> Result<Option<DownloadLookup>, IndexError> {
    let idx = DownloadIndex::open(db_path)?;
    let row: Option<DownloadLookup> = idx
        .conn()
        .query_row(
            "SELECT service_url, title, artist, album, codec, downloaded_at, last_modified_date
             FROM downloads
             WHERE service_url = ?1
             ORDER BY downloaded_at DESC
             LIMIT 1",
            [url],
            |row| {
                Ok(DownloadLookup {
                    url: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    album: row.get(3)?,
                    codec: row.get(4)?,
                    downloaded_at: row.get(5)?,
                    last_modified_date: row.get(6)?,
                })
            },
        )
        .ok();
    Ok(row)
}

/// Walk every URL in the `downloads` table and extract Apple Music
/// `song_id` values from each `?i=` query parameter. Used by the
/// duplicate-detector's history-scope path to augment the manifest
/// walk: rows that have a history entry but no manifest (e.g., old
/// pre-manifest downloads, or manifests deleted by the user) still
/// participate in dedup.
///
/// Returns a `HashSet<String>` of song IDs. Caller is responsible for
/// keying these into whatever dedup strategy is configured (the
/// duplicate_detector already has a `build_track_key_from_parts`
/// helper that takes optional song_id + optional ISRC).
pub fn collect_song_ids_from_db(db_path: &Path) -> Result<HashSet<String>, IndexError> {
    let idx = DownloadIndex::open(db_path)?;
    let mut stmt = idx
        .conn()
        .prepare("SELECT service_url FROM downloads")?;
    let mut keys = HashSet::new();
    let rows = stmt.query_map([], |row| {
        let url: String = row.get(0)?;
        Ok(url)
    })?;
    for url in rows.flatten() {
        if let Some(song_id) = extract_song_id_from_url(&url) {
            keys.insert(song_id);
        }
    }
    Ok(keys)
}

/// Extract the `song_id` from a `?i={id}` query parameter,
/// case-insensitively. Copy of the same helper in `duplicate_detector`
/// so this module stays self-contained.
fn extract_song_id_from_url(url: &str) -> Option<String> {
    let idx = url.find("?i=").or_else(|| url.find("&i="))?;
    let rest = &url[idx + 3..];
    let end = rest.find('&').unwrap_or(rest.len());
    let candidate = &rest[..end];
    if candidate.chars().all(|c| c.is_ascii_digit()) && !candidate.is_empty() {
        Some(candidate.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "meedyadl_query_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("test.db")
    }

    fn seed_downloads(db_path: &Path, urls: &[(&str, Option<&str>, Option<&str>, &str)]) {
        let mut idx = DownloadIndex::open(db_path).unwrap();
        let tx = idx.conn_mut().transaction().unwrap();
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO downloads (service, service_url, codec, downloaded_at) VALUES ('apple_music', ?1, ?2, ?3)",
                )
                .unwrap();
            for (url, codec, _title, downloaded_at) in urls {
                stmt.execute(rusqlite::params![url, codec, downloaded_at]).unwrap();
            }
        }
        tx.commit().unwrap();
    }

    #[test]
    fn find_download_by_url_returns_none_when_missing() {
        let db = temp_db();
        let _ = DownloadIndex::open(&db).unwrap();
        let result = find_download_by_url(&db, "https://nope.example.com/album/0").unwrap();
        assert!(result.is_none());
        std::fs::remove_file(&db).ok();
    }

    #[test]
    fn find_download_by_url_returns_newest_matching_row() {
        let db = temp_db();
        // Two rows for the same URL at different codecs (companion download);
        // the lookup must return the newest by downloaded_at.
        seed_downloads(
            &db,
            &[
                (
                    "https://music.apple.com/us/album/foo/1",
                    Some("aac"),
                    None,
                    "2026-01-01T00:00:00Z",
                ),
                (
                    "https://music.apple.com/us/album/foo/1",
                    Some("alac"),
                    None,
                    "2026-01-02T00:00:00Z",
                ),
            ],
        );
        let result = find_download_by_url(&db, "https://music.apple.com/us/album/foo/1")
            .unwrap()
            .unwrap();
        assert_eq!(result.downloaded_at, "2026-01-02T00:00:00Z");
        assert_eq!(result.codec.as_deref(), Some("alac"));
        std::fs::remove_file(&db).ok();
    }

    #[test]
    fn collect_song_ids_extracts_from_query_params() {
        let db = temp_db();
        seed_downloads(
            &db,
            &[
                // Track-level URL with ?i=
                (
                    "https://music.apple.com/us/album/foo/1?i=123",
                    None,
                    None,
                    "2026-01-01T00:00:00Z",
                ),
                // Album URL — no ?i=, no key extracted
                (
                    "https://music.apple.com/us/album/bar/2",
                    None,
                    None,
                    "2026-01-02T00:00:00Z",
                ),
                // Different track ID
                (
                    "https://music.apple.com/us/album/baz/3?i=456",
                    None,
                    None,
                    "2026-01-03T00:00:00Z",
                ),
            ],
        );
        let keys = collect_song_ids_from_db(&db).unwrap();
        assert!(keys.contains("123"));
        assert!(keys.contains("456"));
        assert_eq!(keys.len(), 2, "album URL has no ?i=, must be skipped");
        std::fs::remove_file(&db).ok();
    }
}
