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

/// Filter for [`search_activity_events`]. All fields are optional;
/// `None` means "no filter on this dimension". `limit` is required so
/// large windows can be paged from the frontend.
///
/// Used by the future searchable Activity Log UI (#875 M4 foundation
/// for the search/filter work that lands later). The DB is the indexed
/// query surface; the JSONL files remain the forensic record.
#[derive(Debug, Clone, Default)]
pub struct ActivityEventFilter<'a> {
    /// Case-insensitive substring match on the `message` column.
    pub message_contains: Option<&'a str>,
    /// One of "INFO" / "WARN" / "ERROR" / "DEBUG" / "VERBOSE". `None`
    /// returns all levels.
    pub level: Option<&'a str>,
    /// `"system"` or `"download:{id}"`. `None` returns all sources.
    pub source: Option<&'a str>,
    /// ISO 8601 UTC lower bound (inclusive). `None` returns all.
    pub since: Option<&'a str>,
    /// Max rows to return. The query is `ORDER BY timestamp DESC`
    /// then `LIMIT`.
    pub limit: usize,
}

/// A single row returned by [`search_activity_events`]. Mirrors the
/// `activity_events` table's user-visible columns.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ActivityEventRow {
    pub timestamp: String,
    pub level: String,
    pub source: String,
    pub message: String,
}

/// Search the `activity_events` table with the given filter. Returns
/// newest-first up to `filter.limit` rows. Empty filter returns the
/// most recent `limit` rows across all levels and sources.
///
/// SQLite handles 100k+ rows in this table with sub-10ms queries
/// when the indexes (idx_activity_timestamp, idx_activity_level)
/// are present — which is what schema v1 set up.
pub fn search_activity_events(
    db_path: &Path,
    filter: &ActivityEventFilter,
) -> Result<Vec<ActivityEventRow>, IndexError> {
    let idx = DownloadIndex::open(db_path)?;
    // Assemble the WHERE clause dynamically so we don't pay for
    // LIKE/comparison work on dimensions the caller didn't filter on.
    let mut sql = String::from(
        "SELECT timestamp, level, source, message FROM activity_events WHERE 1=1",
    );
    let mut params: Vec<String> = Vec::new();
    if let Some(needle) = filter.message_contains {
        sql.push_str(" AND lower(message) LIKE ?");
        params.push(format!("%{}%", needle.to_lowercase()));
    }
    if let Some(level) = filter.level {
        sql.push_str(" AND level = ?");
        params.push(level.to_string());
    }
    if let Some(source) = filter.source {
        sql.push_str(" AND source = ?");
        params.push(source.to_string());
    }
    if let Some(since) = filter.since {
        sql.push_str(" AND timestamp >= ?");
        params.push(since.to_string());
    }
    sql.push_str(" ORDER BY timestamp DESC LIMIT ?");
    params.push(filter.limit.to_string());

    let mut stmt = idx.conn().prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> =
        params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(ActivityEventRow {
            timestamp: row.get(0)?,
            level: row.get(1)?,
            source: row.get(2)?,
            message: row.get(3)?,
        })
    })?;
    let collected: Vec<ActivityEventRow> = rows.filter_map(Result::ok).collect();
    Ok(collected)
}

/// Compact data shape for the `known_tracks` table (#875 M4
/// gap-fill foundation). The Library Scan UI will surface these
/// rows as "tracks you have on Apple Music but haven't downloaded
/// yet" — the gap-fill driver.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnownTrackRow {
    pub service: String,
    pub service_track_id: String,
    pub service_url: String,
    pub isrc: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub duration_seconds: Option<f64>,
    pub source: Option<String>,
    pub discovered_at: String,
}

/// Insert (or update on conflict) a row in the `known_tracks`
/// table. The `UNIQUE(service, service_track_id)` constraint means
/// re-running the Apple Music library sync is idempotent.
///
/// The Apple Music library sync code that PRODUCES rows is out of
/// scope for M4 — this is the persistence half. Future work wires
/// up a periodic sync that calls this helper with discovered
/// tracks.
pub fn upsert_known_track(
    db_path: &Path,
    track: &KnownTrackRow,
) -> Result<(), IndexError> {
    let idx = DownloadIndex::open(db_path)?;
    idx.conn().execute(
        "INSERT INTO known_tracks (
            service, service_track_id, service_url, isrc, artist, album,
            title, duration_seconds, source, discovered_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(service, service_track_id) DO UPDATE SET
            service_url    = excluded.service_url,
            isrc           = excluded.isrc,
            artist         = excluded.artist,
            album          = excluded.album,
            title          = excluded.title,
            duration_seconds = excluded.duration_seconds,
            source         = excluded.source,
            discovered_at  = excluded.discovered_at",
        rusqlite::params![
            track.service,
            track.service_track_id,
            track.service_url,
            track.isrc,
            track.artist,
            track.album,
            track.title,
            track.duration_seconds,
            track.source,
            track.discovered_at,
        ],
    )?;
    Ok(())
}

/// Returns the set of tracks present in `known_tracks` BUT NOT in
/// `downloads` (any codec). This is the canonical "gap-fill"
/// query — every row returned is a track the user has in their
/// Apple Music library but hasn't downloaded yet.
///
/// The join uses ISRC when available (most reliable cross-service
/// key) and falls back to `service_track_id` otherwise. The query
/// is bounded by `limit` so the Library Scan UI can page.
pub fn list_library_gaps(
    db_path: &Path,
    limit: usize,
) -> Result<Vec<KnownTrackRow>, IndexError> {
    let idx = DownloadIndex::open(db_path)?;
    // A track is "missing" when no row in `downloads` shares its
    // ISRC (when both are non-NULL) AND no row in `downloads`
    // shares its service_track_id (when both are non-NULL).
    let mut stmt = idx.conn().prepare(
        "SELECT service, service_track_id, service_url, isrc, artist, album,
                title, duration_seconds, source, discovered_at
         FROM known_tracks kt
         WHERE NOT EXISTS (
            SELECT 1 FROM downloads d
            WHERE
                (kt.isrc IS NOT NULL AND d.isrc = kt.isrc)
                OR
                (kt.service_track_id IS NOT NULL AND d.service = kt.service AND d.service_track_id = kt.service_track_id)
         )
         ORDER BY discovered_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(KnownTrackRow {
            service: row.get(0)?,
            service_track_id: row.get(1)?,
            service_url: row.get(2)?,
            isrc: row.get(3)?,
            artist: row.get(4)?,
            album: row.get(5)?,
            title: row.get(6)?,
            duration_seconds: row.get(7)?,
            source: row.get(8)?,
            discovered_at: row.get(9)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
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

    fn seed_activity_events(
        db_path: &Path,
        rows: &[(&str, &str, &str, &str)],
    ) {
        let mut idx = DownloadIndex::open(db_path).unwrap();
        let tx = idx.conn_mut().transaction().unwrap();
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO activity_events (timestamp, level, source, message) VALUES (?1, ?2, ?3, ?4)",
                )
                .unwrap();
            for (ts, level, source, msg) in rows {
                stmt.execute(rusqlite::params![ts, level, source, msg]).unwrap();
            }
        }
        tx.commit().unwrap();
    }

    #[test]
    fn search_activity_events_returns_newest_first_within_limit() {
        let db = temp_db();
        seed_activity_events(
            &db,
            &[
                ("2026-05-23T10:00:00Z", "INFO", "system", "first"),
                ("2026-05-24T10:00:00Z", "WARN", "system", "second"),
                ("2026-05-25T10:00:00Z", "ERROR", "download:abc", "third"),
            ],
        );
        let results = search_activity_events(
            &db,
            &ActivityEventFilter {
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].message, "third");
        assert_eq!(results[2].message, "first");
        std::fs::remove_file(&db).ok();
    }

    #[test]
    fn search_activity_events_filters_by_level_and_message() {
        let db = temp_db();
        seed_activity_events(
            &db,
            &[
                ("2026-05-23T10:00:00Z", "INFO", "system", "no match"),
                ("2026-05-24T10:00:00Z", "WARN", "system", "needle here"),
                ("2026-05-25T10:00:00Z", "ERROR", "download:abc", "NEEDLE in error"),
            ],
        );
        let only_warn = search_activity_events(
            &db,
            &ActivityEventFilter {
                level: Some("WARN"),
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(only_warn.len(), 1);
        assert_eq!(only_warn[0].message, "needle here");

        // Case-insensitive message match.
        let needle_rows = search_activity_events(
            &db,
            &ActivityEventFilter {
                message_contains: Some("Needle"),
                limit: 10,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(needle_rows.len(), 2);
        std::fs::remove_file(&db).ok();
    }

    #[test]
    fn upsert_and_list_library_gaps_returns_only_missing_tracks() {
        let db = temp_db();
        // Insert 3 known tracks; 1 of them is also in `downloads` so it's NOT a gap.
        {
            let idx = DownloadIndex::open(&db).unwrap();
            // Mark track A as already downloaded (by ISRC).
            idx.conn()
                .execute(
                    "INSERT INTO downloads (service, service_url, codec, isrc, downloaded_at) VALUES ('apple_music', 'https://x/A', 'alac', 'AAA', '2026-05-25T00:00:00Z')",
                    [],
                )
                .unwrap();
        }
        // Three known tracks; only A overlaps with downloads above.
        upsert_known_track(
            &db,
            &KnownTrackRow {
                service: "apple_music".into(),
                service_track_id: "tA".into(),
                service_url: "https://x/A".into(),
                isrc: Some("AAA".into()),
                artist: None,
                album: None,
                title: Some("AlphaTrack".into()),
                duration_seconds: None,
                source: Some("apple_music_library".into()),
                discovered_at: "2026-05-25T11:00:00Z".into(),
            },
        )
        .unwrap();
        upsert_known_track(
            &db,
            &KnownTrackRow {
                service: "apple_music".into(),
                service_track_id: "tB".into(),
                service_url: "https://x/B".into(),
                isrc: Some("BBB".into()),
                artist: None,
                album: None,
                title: Some("BravoTrack".into()),
                duration_seconds: None,
                source: Some("apple_music_library".into()),
                discovered_at: "2026-05-25T11:01:00Z".into(),
            },
        )
        .unwrap();
        upsert_known_track(
            &db,
            &KnownTrackRow {
                service: "apple_music".into(),
                service_track_id: "tC".into(),
                service_url: "https://x/C".into(),
                isrc: None,
                artist: None,
                album: None,
                title: Some("CharlieTrack".into()),
                duration_seconds: None,
                source: Some("apple_music_library".into()),
                discovered_at: "2026-05-25T11:02:00Z".into(),
            },
        )
        .unwrap();

        let gaps = list_library_gaps(&db, 50).unwrap();
        let titles: Vec<&str> = gaps.iter().filter_map(|r| r.title.as_deref()).collect();
        assert!(!titles.contains(&"AlphaTrack"), "AlphaTrack is downloaded — must not appear");
        assert!(titles.contains(&"BravoTrack"), "BravoTrack ISRC has no download — must appear");
        assert!(titles.contains(&"CharlieTrack"), "CharlieTrack has no ISRC and no download — must appear");
        assert_eq!(gaps.len(), 2);

        // Upsert idempotency — re-inserting the same row updates rather than duplicates.
        upsert_known_track(
            &db,
            &KnownTrackRow {
                service: "apple_music".into(),
                service_track_id: "tC".into(),
                service_url: "https://x/C-updated".into(),
                isrc: None,
                artist: Some("New Artist".into()),
                album: None,
                title: Some("CharlieTrack (updated)".into()),
                duration_seconds: None,
                source: Some("apple_music_library".into()),
                discovered_at: "2026-05-25T11:30:00Z".into(),
            },
        )
        .unwrap();
        let gaps2 = list_library_gaps(&db, 50).unwrap();
        assert_eq!(gaps2.len(), 2, "upsert must not create a duplicate row");
        let charlie = gaps2
            .iter()
            .find(|r| r.service_track_id == "tC")
            .expect("Charlie row present");
        assert_eq!(charlie.title.as_deref(), Some("CharlieTrack (updated)"));
        assert_eq!(charlie.service_url, "https://x/C-updated");

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
