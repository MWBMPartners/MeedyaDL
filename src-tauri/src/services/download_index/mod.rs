// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License.
//
// Download Index — SQLite-backed indexed cache (#875 EPIC A M1).
//
// Foundation for the multi-service dedupe / library gap-fill /
// searchable activity log work. `.meedyadl` manifests on disk REMAIN
// the source of truth — this DB is rebuildable from them via
// `scan_folder_for_manifests`.
//
// M1 scope (this commit):
//   * `rusqlite` dependency wired (`bundled` + `chrono` features).
//   * Schema v1 SQL embedded via `include_str!` from `schema_v1.sql`.
//   * `DownloadIndex` wrapper around `rusqlite::Connection`.
//   * Idempotent migration runner that creates the schema on first
//     open and inserts the schema_version row, or applies subsequent
//     migrations (v2 / v3 …) when added in M2+.
//   * Unit tests verify schema applies cleanly, idempotent open
//     succeeds twice, and schema_version row is present.
//
// NOT in M1 (deferred to M1b/M2):
//   * Manifest ingest from `scan_folder_for_manifests` IPC.
//   * Download/activity insert paths.
//   * Query helpers (`find_by_isrc`, `find_gaps`, etc.).
//   * Tauri-managed state plumbing.
//   * Replacement of `history.json` reads.
//
// The DB file lives at `{app_data_dir}/meedyadl.db`. On user-data
// resets / "Clear all data" flows, removing this file is safe — the
// next launch rebuilds from `.meedyadl` manifests.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

/// Embedded schema v1 SQL. Applied as a single transaction by
/// `apply_schema_v1`. Future migrations (v2, v3, …) will append new
/// SQL files (`schema_v2.sql`, `schema_v3.sql`) consumed by the same
/// migration runner.
const SCHEMA_V1_SQL: &str = include_str!("schema_v1.sql");

/// Schema v2 SQL — adds the `meta` table for tracking one-shot
/// operations like the first-startup ingest (#875 M1b).
const SCHEMA_V2_SQL: &str = include_str!("schema_v2.sql");

/// The current schema version this build of MeedyaDL knows how to
/// produce. Migration runner refuses to open a DB whose version is
/// strictly higher than this (forwards compatibility is opt-in only).
pub const CURRENT_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("database open error: {0}")]
    Open(#[from] rusqlite::Error),

    #[error("schema is at version {found} but this build only knows up to v{known}")]
    SchemaTooNew { found: i64, known: i64 },
}

/// Owned SQLite connection plus a tiny API surface. Construct via
/// [`DownloadIndex::open`]; the connection is closed when the value
/// is dropped.
pub struct DownloadIndex {
    conn: Connection,
}

impl DownloadIndex {
    /// Open (or create) the download-index database at `path`,
    /// applying any pending migrations.
    ///
    /// Idempotent — calling `open` on an existing DB at the correct
    /// schema version is a fast no-op.
    pub fn open(path: &Path) -> Result<Self, IndexError> {
        let conn = Connection::open(path)?;
        let mut idx = Self { conn };
        idx.migrate()?;
        Ok(idx)
    }

    /// Open an in-memory database. Used by tests and one-off
    /// query-validation flows; never persisted.
    #[cfg(test)]
    fn open_in_memory() -> Result<Self, IndexError> {
        let conn = Connection::open_in_memory()?;
        let mut idx = Self { conn };
        idx.migrate()?;
        Ok(idx)
    }

    /// The current schema version installed in this database.
    pub fn schema_version(&self) -> Result<i64, IndexError> {
        // schema_version doesn't exist yet on the very first open —
        // probe the sqlite_master catalog instead of querying directly
        // so the existence check itself can't fail.
        let table_exists: Option<String> = self
            .conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()?;

        if table_exists.is_none() {
            return Ok(0);
        }

        let version: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )?;
        Ok(version)
    }

    /// Run all pending migrations to bring the DB up to
    /// [`CURRENT_SCHEMA_VERSION`].
    fn migrate(&mut self) -> Result<(), IndexError> {
        let current = self.schema_version()?;

        if current > CURRENT_SCHEMA_VERSION {
            return Err(IndexError::SchemaTooNew {
                found: current,
                known: CURRENT_SCHEMA_VERSION,
            });
        }

        if current < 1 {
            self.apply_schema_v1()?;
        }

        if current < 2 {
            self.apply_schema_v2()?;
        }

        // Future: if current < 3 { self.apply_schema_v3()?; } …

        Ok(())
    }

    /// Apply the v1 schema in a single transaction. Idempotent —
    /// every `CREATE TABLE` / `CREATE INDEX` is `IF NOT EXISTS`.
    fn apply_schema_v1(&mut self) -> Result<(), IndexError> {
        let tx = self.conn.transaction()?;
        tx.execute_batch(SCHEMA_V1_SQL)?;
        tx.execute(
            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (1, ?1)",
            [chrono::Utc::now().to_rfc3339()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Apply the v2 schema in a single transaction. Adds the `meta`
    /// key/value table for tracking one-shot operations. Idempotent.
    fn apply_schema_v2(&mut self) -> Result<(), IndexError> {
        let tx = self.conn.transaction()?;
        tx.execute_batch(SCHEMA_V2_SQL)?;
        tx.execute(
            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (2, ?1)",
            [chrono::Utc::now().to_rfc3339()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Look up a value from the `meta` key/value table. Returns
    /// `Ok(None)` when the key is absent (still a successful read).
    ///
    /// Used to gate one-shot operations like the first-startup ingest:
    /// a non-None value means the operation has already run; absent
    /// means it hasn't. Always reflects the latest write — no in-memory
    /// caching.
    pub fn meta_get(&self, key: &str) -> Result<Option<String>, IndexError> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    /// Insert or replace a meta key/value entry. The `set_at` column
    /// is populated with the current UTC ISO 8601 timestamp so the
    /// caller never has to.
    pub fn meta_set(&self, key: &str, value: &str) -> Result<(), IndexError> {
        self.conn.execute(
            "INSERT INTO meta (key, value, set_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, set_at = excluded.set_at",
            [key, value, &chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Borrow the underlying connection. Public so the M2+
    /// modules (manifest ingest, query helpers) can extend the API
    /// without churning this module on every change.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Mutable borrow of the underlying connection. Required for
    /// transactional batch inserts (the `ingest` submodule, future
    /// M2/M3 write paths). The borrow checker prevents callers from
    /// double-borrowing the connection from elsewhere while a
    /// transaction is open, which is exactly the invariant rusqlite
    /// needs for sound transaction semantics.
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

pub mod ingest;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_applies_on_fresh_open() {
        let idx = DownloadIndex::open_in_memory().expect("open in-memory");
        assert_eq!(idx.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn schema_apply_is_idempotent_across_reopens() {
        // Open a tempfile-backed DB twice; second open should be a no-op.
        let dir = std::env::temp_dir().join(format!(
            "meedyadl_test_download_index_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");

        {
            let idx = DownloadIndex::open(&path).expect("first open");
            assert_eq!(idx.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        }
        {
            let idx = DownloadIndex::open(&path).expect("second open");
            assert_eq!(idx.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn refuses_to_open_schema_newer_than_known() {
        let conn = Connection::open_in_memory().unwrap();
        // Hand-craft a future-version DB to simulate a downgrade attempt.
        conn.execute_batch(
            "CREATE TABLE schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
             INSERT INTO schema_version VALUES (99, '2030-01-01T00:00:00Z');",
        )
        .unwrap();
        let mut idx = DownloadIndex { conn };
        let err = idx.migrate().expect_err("should refuse newer schema");
        match err {
            IndexError::SchemaTooNew { found, known } => {
                assert_eq!(found, 99);
                assert_eq!(known, CURRENT_SCHEMA_VERSION);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn schema_v2_migration_from_existing_v1_db_succeeds() {
        // Simulate an existing DB at schema v1 (the M1 state) and
        // confirm `open` applies the v2 migration cleanly. This is
        // the upgrade path real users will hit after pulling the
        // M1b build.
        let dir = std::env::temp_dir().join(format!(
            "meedyadl_test_v1_to_v2_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");

        // Build a v1-only DB by applying just the v1 schema.
        {
            let conn = Connection::open(&path).unwrap();
            let mut idx = DownloadIndex { conn };
            idx.apply_schema_v1().unwrap();
            assert_eq!(idx.schema_version().unwrap(), 1);
        }

        // Re-open via the real entry point — `migrate()` must apply v2.
        {
            let idx = DownloadIndex::open(&path).unwrap();
            assert_eq!(idx.schema_version().unwrap(), 2);
            assert!(
                idx.meta_get("nonexistent").unwrap().is_none(),
                "meta table must exist + be queryable after v1→v2 migration"
            );
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn meta_get_and_set_roundtrip() {
        let idx = DownloadIndex::open_in_memory().unwrap();
        assert_eq!(idx.meta_get("foo").unwrap(), None);
        idx.meta_set("foo", "bar").unwrap();
        assert_eq!(idx.meta_get("foo").unwrap(), Some("bar".to_string()));
        // Overwrite an existing key (ON CONFLICT path).
        idx.meta_set("foo", "baz").unwrap();
        assert_eq!(idx.meta_get("foo").unwrap(), Some("baz".to_string()));
    }

    #[test]
    fn schema_creates_every_required_table() {
        let idx = DownloadIndex::open_in_memory().unwrap();
        let tables: Vec<String> = idx
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .filter(|n: &String| !n.starts_with("sqlite_"))
            .collect();
        // 8 user tables: downloads, recordings, recording_downloads,
        // manifests, activity_events, known_tracks, schema_version, meta.
        for required in [
            "downloads",
            "recordings",
            "recording_downloads",
            "manifests",
            "activity_events",
            "known_tracks",
            "schema_version",
            "meta",
        ] {
            assert!(
                tables.iter().any(|t| t == required),
                "missing table: {required} (have: {tables:?})"
            );
        }
    }
}
