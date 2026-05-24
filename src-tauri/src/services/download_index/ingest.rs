// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License.
//
// Download Index — first-startup ingest (#875 EPIC A M1b).
//
// On the very first launch where this DB is opened, ingest:
//   1. `history.json` → `downloads` table
//   2. JSONL activity-log files → `activity_events` table
//
// The ingest is gated by a meta-table flag (`first_startup_ingest_v1`)
// so it runs exactly once per database. Re-running MeedyaDL with the
// DB intact never re-ingests — the M2/M3 dual-write paths keep the
// DB current after this initial bulk migration.
//
// `queue.json` (in-flight queue state) is intentionally NOT migrated.
// The EPIC explicitly excludes it: queue.json is in-flight state, not
// a historical record, and the queue restoration path on startup
// uses queue.json directly. History.json contains the TERMINAL queue
// items (completed / cancelled / errored) which is what users want
// to see indexed.

use std::path::Path;

use rusqlite::params;

use super::{DownloadIndex, IndexError};

/// Meta-key flag indicating the first-startup ingest has already
/// completed. Presence in the `meta` table means "skip the ingest";
/// absence means "ingest now".
const INGEST_DONE_KEY: &str = "first_startup_ingest_v1";

/// Default look-back window for activity-log JSONL ingest. The
/// activity-log directory may contain weeks of historical files —
/// for the FIRST ingest we limit to a reasonable window so a fresh
/// SQLite DB doesn't end up with 100,000+ events from older runs.
/// Per the EPIC, the JSONL files remain on disk as the forensic
/// record (#541); the DB is the indexed query surface.
pub const DEFAULT_ACTIVITY_LOG_INGEST_DAYS: u64 = 30;

/// Summary of what got ingested during the first-startup migration.
/// Returned to the caller so it can be surfaced in the activity log
/// or settings panel.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct IngestSummary {
    /// Number of `downloads` rows inserted from `history.json`.
    pub downloads_ingested: usize,
    /// Number of `activity_events` rows inserted from JSONL files.
    pub activity_events_ingested: usize,
    /// Number of JSONL activity-log files read during ingest.
    pub activity_log_files_read: usize,
    /// True when the ingest actually ran (vs. was skipped because
    /// it had already completed previously).
    pub ran: bool,
}

/// Idempotent first-startup ingest entry point.
///
/// * Returns immediately with `ran=false` if the ingest has run
///   before (presence of the meta flag).
/// * Otherwise: reads `history.json` and the JSONL activity log
///   files, inserts rows into the DB, sets the meta flag.
/// * `app_data_dir` is the resolved app data directory (the
///   parent of `history.json` and `queue.json`).
/// * `log_dir` is the resolved activity-log directory.
/// * `activity_days_back` controls how many days of JSONL files
///   to ingest; 0 means "skip activity-log ingest entirely".
pub fn run_first_startup_ingest(
    idx: &mut DownloadIndex,
    app_data_dir: &Path,
    log_dir: &Path,
    activity_days_back: u64,
) -> Result<IngestSummary, IndexError> {
    if idx.meta_get(INGEST_DONE_KEY)?.is_some() {
        return Ok(IngestSummary {
            ran: false,
            ..Default::default()
        });
    }

    let downloads_ingested = ingest_history_json(idx, app_data_dir)?;
    let (activity_events_ingested, activity_log_files_read) =
        ingest_activity_log_jsonl(idx, log_dir, activity_days_back)?;

    // Mark complete only after both ingests finished without error.
    // If either step errors midway, we want the NEXT launch to retry
    // — partial DB state is acceptable because the JSONL files on
    // disk remain the forensic record (#541).
    idx.meta_set(
        INGEST_DONE_KEY,
        &format!(
            "downloads={downloads_ingested}, activity_events={activity_events_ingested}, log_files={activity_log_files_read}"
        ),
    )?;

    Ok(IngestSummary {
        downloads_ingested,
        activity_events_ingested,
        activity_log_files_read,
        ran: true,
    })
}

/// Read `history.json` from `app_data_dir` and insert each entry into
/// the `downloads` table. Skips silently when the file is missing or
/// malformed — graceful degradation matches the rest of MeedyaDL's
/// recovery posture.
///
/// Returns the number of rows actually inserted (which may be less
/// than the number of entries in the file if the UNIQUE constraint
/// rejects duplicates).
pub fn ingest_history_json(
    idx: &mut DownloadIndex,
    app_data_dir: &Path,
) -> Result<usize, IndexError> {
    let path = app_data_dir.join("history.json");
    let Ok(json) = std::fs::read_to_string(&path) else {
        log::debug!(
            "history.json absent at {}; skipping ingest",
            path.display()
        );
        return Ok(0);
    };

    let entries: Vec<HistoryEntryShape> = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(e) => {
            log::warn!(
                "history.json at {} failed to parse for ingest: {e}; skipping",
                path.display()
            );
            return Ok(0);
        }
    };

    let tx = idx.conn_mut().transaction()?;
    let mut inserted = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO downloads (
                service, service_url, isrc, artist, album, title, codec,
                file_path, downloaded_at, manifest_path, content_rating
             ) VALUES (
                'apple_music', ?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL
             )",
        )?;
        for entry in &entries {
            let n = stmt.execute(params![
                entry.url,
                entry.artist.as_deref(),
                entry.album.as_deref(),
                entry.title.as_deref(),
                entry.codec.as_deref(),
                entry.file_path.as_deref(),
                entry.completed_at,
            ])?;
            inserted += n;
        }
    }
    tx.commit()?;
    Ok(inserted)
}

/// Walk `log_dir` for files matching `activity-YYYY-MM-DD.log`, read
/// each JSONL line as an [`ActivityLogShape`], and insert into the
/// `activity_events` table.
///
/// `days_back == 0` short-circuits to a no-op (caller asked to skip
/// activity-log ingest entirely). Otherwise the most-recently-modified
/// `days_back` files are processed; older files stay on disk and can
/// be ingested manually in M4 if needed.
///
/// Returns `(events_inserted, files_read)`.
pub fn ingest_activity_log_jsonl(
    idx: &mut DownloadIndex,
    log_dir: &Path,
    days_back: u64,
) -> Result<(usize, usize), IndexError> {
    if days_back == 0 || !log_dir.exists() {
        return Ok((0, 0));
    }

    // Collect candidate activity-YYYY-MM-DD.log files. We don't rely
    // on the existing `activity_log_writer::list_log_files` helper
    // because pulling that into the ingest module would create a
    // dep cycle — re-implement the same shape inline.
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let entries = match std::fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(e) => {
            log::debug!(
                "Could not read activity-log dir {}: {e}; skipping",
                log_dir.display()
            );
            return Ok((0, 0));
        }
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if name.starts_with("activity-") && name.ends_with(".log") {
            files.push(p);
        }
    }
    // Newest first (lexicographic sort of `activity-YYYY-MM-DD.log`
    // names is also chronological).
    files.sort();
    files.reverse();
    let files: Vec<_> = files.into_iter().take(days_back as usize).collect();

    let tx = idx.conn_mut().transaction()?;
    let mut events_inserted = 0;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO activity_events (
                timestamp, level, source, message, download_id, metadata_json
             ) VALUES (?1, ?2, ?3, ?4, NULL, NULL)",
        )?;
        for file in &files {
            let Ok(contents) = std::fs::read_to_string(file) else {
                log::debug!(
                    "Activity-log file {} unreadable; skipping",
                    file.display()
                );
                continue;
            };
            for line in contents.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let event: ActivityLogShape = match serde_json::from_str(trimmed) {
                    Ok(v) => v,
                    Err(_) => continue, // skip malformed lines silently
                };
                // Activity-log events carry their own severity which
                // maps directly to a log-level string. Default to
                // `"INFO"` for entries that lack the field.
                let level = event.severity.unwrap_or_else(|| "INFO".to_string());
                let source = if event.download_id == "system" {
                    "system".to_string()
                } else {
                    format!("download:{}", event.download_id)
                };
                events_inserted +=
                    stmt.execute(params![event.timestamp, level, source, event.line])?;
            }
        }
    }
    tx.commit()?;
    Ok((events_inserted, files.len()))
}

/// Minimal `serde::Deserialize` shape that matches the
/// `HistoryEntry` JSON layout in `services::history_service`.
/// Re-declared here (rather than importing) to keep the ingest
/// module decoupled from the history service's evolving Rust API.
#[derive(serde::Deserialize)]
struct HistoryEntryShape {
    url: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    codec: Option<String>,
    file_path: Option<String>,
    completed_at: String,
}

/// Minimal `serde::Deserialize` shape that matches the
/// `ActivityLogEvent` JSONL layout in `utils::activity_log`.
/// Same decoupling rationale as `HistoryEntryShape`.
#[derive(serde::Deserialize)]
struct ActivityLogShape {
    download_id: String,
    line: String,
    timestamp: String,
    #[serde(default)]
    severity: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "meedyadl_ingest_{label}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn ingest_history_into_downloads() {
        let app_dir = temp_dir("history");
        let history = r#"[
            {"id":"a","url":"https://music.apple.com/us/album/foo/1","title":"Foo","artist":"AAA","album":"X","codec":"alac","file_path":"/tmp/x","started_at":"2026-01-01T00:00:00Z","completed_at":"2026-01-01T00:00:01Z","status":"success"},
            {"id":"b","url":"https://music.apple.com/us/album/bar/2","title":"Bar","artist":"BBB","album":"Y","codec":"aac","file_path":"/tmp/y","started_at":"2026-01-02T00:00:00Z","completed_at":"2026-01-02T00:00:01Z","status":"success"}
        ]"#;
        std::fs::write(app_dir.join("history.json"), history).unwrap();

        let db_path = app_dir.join("test.db");
        let mut idx = DownloadIndex::open(&db_path).unwrap();
        let count = ingest_history_json(&mut idx, &app_dir).unwrap();
        assert_eq!(count, 2);

        // Re-running the ingest is idempotent against the same data
        // because of the UNIQUE(service, service_url, codec) constraint.
        let count_again = ingest_history_json(&mut idx, &app_dir).unwrap();
        assert_eq!(count_again, 0, "re-ingest must dedupe via UNIQUE constraint");

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[test]
    fn ingest_activity_log_jsonl_reads_recent_files() {
        let log_dir = temp_dir("activity");

        // Two activity-log files; both should be picked up at
        // days_back=30. The newer file is named lexicographically
        // later so it sorts first under the newest-first ordering.
        let event_a = r#"{"download_id":"system","stream":"internal","line":"first event","timestamp":"2026-05-23T12:00:00Z"}"#;
        let event_b = r#"{"download_id":"system","stream":"internal","line":"second event","timestamp":"2026-05-24T12:00:00Z"}"#;
        std::fs::write(log_dir.join("activity-2026-05-23.log"), format!("{event_a}\n")).unwrap();
        std::fs::write(log_dir.join("activity-2026-05-24.log"), format!("{event_b}\n")).unwrap();

        let db_path = log_dir.join("test.db");
        let mut idx = DownloadIndex::open(&db_path).unwrap();
        let (events, files) = ingest_activity_log_jsonl(&mut idx, &log_dir, 30).unwrap();
        assert_eq!(files, 2);
        assert_eq!(events, 2);

        std::fs::remove_dir_all(&log_dir).ok();
    }

    #[test]
    fn run_first_startup_ingest_is_idempotent_on_second_call() {
        let app_dir = temp_dir("ingest");
        std::fs::write(
            app_dir.join("history.json"),
            r#"[{"id":"a","url":"https://music.apple.com/us/album/foo/1","started_at":"2026-01-01T00:00:00Z","completed_at":"2026-01-01T00:00:01Z","status":"success"}]"#,
        )
        .unwrap();

        let log_dir = app_dir.join("logs");
        std::fs::create_dir_all(&log_dir).unwrap();

        let db_path = app_dir.join("test.db");
        let mut idx = DownloadIndex::open(&db_path).unwrap();

        let first = run_first_startup_ingest(&mut idx, &app_dir, &log_dir, 30).unwrap();
        assert!(first.ran);
        assert_eq!(first.downloads_ingested, 1);

        let second = run_first_startup_ingest(&mut idx, &app_dir, &log_dir, 30).unwrap();
        assert!(!second.ran, "second call must short-circuit on the meta flag");
        assert_eq!(second.downloads_ingested, 0);

        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[test]
    fn ingest_history_handles_missing_file_gracefully() {
        let app_dir = temp_dir("missing");
        let db_path = app_dir.join("test.db");
        let mut idx = DownloadIndex::open(&db_path).unwrap();
        // No history.json present — must succeed with 0 inserts.
        let count = ingest_history_json(&mut idx, &app_dir).unwrap();
        assert_eq!(count, 0);
        std::fs::remove_dir_all(&app_dir).ok();
    }

    #[test]
    fn ingest_activity_log_handles_missing_dir_gracefully() {
        let app_dir = temp_dir("missing_logs");
        let missing_log_dir = app_dir.join("does_not_exist");
        let db_path = app_dir.join("test.db");
        let mut idx = DownloadIndex::open(&db_path).unwrap();
        let (events, files) =
            ingest_activity_log_jsonl(&mut idx, &missing_log_dir, 30).unwrap();
        assert_eq!(events, 0);
        assert_eq!(files, 0);
        std::fs::remove_dir_all(&app_dir).ok();
    }
}
