-- Copyright (c) 2026 MeedyaSuite
-- Licensed under the MIT License.
--
-- MeedyaDL download index — schema v1 (#875 EPIC A M1)
--
-- A SQLite-backed indexed cache that replaces history.json (1000-row
-- cap) and powers cross-service dedupe, library gap-fill, and
-- searchable activity-log queries. `.meedyadl` manifests on disk
-- REMAIN the source of truth — this DB is rebuildable from them via
-- the existing scan_folder_for_manifests IPC.
--
-- Schema is applied in a single transaction by `apply_schema_v1` in
-- download_index/mod.rs. The schema_version table is the migration
-- canary: presence of a row with version=1 means the schema is
-- applied; absence means apply it now.

-- Per-service download instance (one row per file downloaded).
CREATE TABLE IF NOT EXISTS downloads (
  id                    INTEGER PRIMARY KEY AUTOINCREMENT,
  service               TEXT    NOT NULL,           -- 'apple_music' | 'spotify' | 'youtube' | 'bbc_iplayer'
  service_url           TEXT    NOT NULL,
  service_track_id      TEXT,                       -- Apple Music song_id, Spotify ID, etc.
  isrc                  TEXT,
  acoustid_recording_id TEXT,
  artist                TEXT,
  album                 TEXT,
  title                 TEXT,
  codec                 TEXT,                       -- 'alac' | 'atmos' | 'aac' | 'aac-web' | ...
  duration_seconds      REAL,
  file_path             TEXT,                       -- where the file actually lives
  file_sha256           TEXT,                       -- audio file integrity hash (optional)
  downloaded_at         TEXT    NOT NULL,           -- ISO 8601 UTC
  manifest_path         TEXT,                       -- FK-style pointer to .meedyadl file
  last_modified_date    TEXT,                       -- Apple Music's lastModifiedDate (content-refresh signal)
  content_rating        TEXT,                       -- 'explicit' | 'clean' | NULL
  apple_digital_master  INTEGER,                    -- BOOLEAN as 0/1
  audio_traits_json     TEXT,                       -- per-service blob; deliberately unstructured
  UNIQUE(service, service_url, codec)
);
CREATE INDEX IF NOT EXISTS idx_downloads_service_url      ON downloads(service_url);
CREATE INDEX IF NOT EXISTS idx_downloads_isrc             ON downloads(isrc);
CREATE INDEX IF NOT EXISTS idx_downloads_acoustid         ON downloads(acoustid_recording_id);
CREATE INDEX IF NOT EXISTS idx_downloads_artist_album     ON downloads(artist, album);
CREATE INDEX IF NOT EXISTS idx_downloads_downloaded_at    ON downloads(downloaded_at);

-- Logical track identity (cross-service join).
CREATE TABLE IF NOT EXISTS recordings (
  id                    INTEGER PRIMARY KEY AUTOINCREMENT,
  isrc                  TEXT    UNIQUE,             -- Most reliable cross-service join key
  acoustid_recording_id TEXT,
  artist                TEXT,
  title                 TEXT,
  duration_seconds      REAL
);
CREATE INDEX IF NOT EXISTS idx_recordings_acoustid ON recordings(acoustid_recording_id);

-- M:N: one recording can be downloaded from multiple services & codecs.
CREATE TABLE IF NOT EXISTS recording_downloads (
  recording_id INTEGER NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
  download_id  INTEGER NOT NULL REFERENCES downloads(id)  ON DELETE CASCADE,
  PRIMARY KEY (recording_id, download_id)
);

-- Pointer back to .meedyadl files on disk (for rebuild + truth check).
CREATE TABLE IF NOT EXISTS manifests (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  manifest_path   TEXT    NOT NULL UNIQUE,
  album_artist    TEXT,
  album           TEXT,
  service         TEXT,
  storefront      TEXT,
  source_url      TEXT,
  schema_version  INTEGER,                          -- the manifest file's own schema version
  scanned_at      TEXT    NOT NULL                  -- ISO 8601 UTC
);

-- Searchable activity log. JSONL on disk REMAINS the forensic record
-- (#541); this table is the indexed query surface.
CREATE TABLE IF NOT EXISTS activity_events (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  timestamp       TEXT    NOT NULL,                 -- ISO 8601 UTC, always UTC (frontend converts)
  level           TEXT    NOT NULL,                 -- 'DEBUG' | 'INFO' | 'WARN' | 'ERROR' | 'VERBOSE'
  source          TEXT    NOT NULL,                 -- 'system' | 'download:<id>' | 'companion:<id>' | 'engine:<id>'
  message         TEXT    NOT NULL,
  download_id     INTEGER REFERENCES downloads(id) ON DELETE SET NULL,
  metadata_json   TEXT                              -- structured fields (per-event blob)
);
CREATE INDEX IF NOT EXISTS idx_activity_timestamp   ON activity_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_activity_level       ON activity_events(level);
CREATE INDEX IF NOT EXISTS idx_activity_download_id ON activity_events(download_id);

-- Gap-fill: tracks we know exist but haven't downloaded.
CREATE TABLE IF NOT EXISTS known_tracks (
  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
  service            TEXT    NOT NULL,
  service_track_id   TEXT    NOT NULL,
  service_url        TEXT    NOT NULL,
  isrc               TEXT,
  artist             TEXT,
  album              TEXT,
  title              TEXT,
  duration_seconds   REAL,
  source             TEXT,                          -- 'apple_music_library' | 'apple_music_api' | 'musicbrainz' | 'user_added'
  discovered_at      TEXT    NOT NULL,              -- ISO 8601 UTC
  UNIQUE(service, service_track_id)
);
CREATE INDEX IF NOT EXISTS idx_known_tracks_isrc ON known_tracks(isrc);

-- Migration tracker. Every applied schema migration appends a row.
-- The presence of version=N means schema is at AT LEAST version N.
-- The migration runner reads MAX(version) and applies subsequent
-- migrations sequentially.
CREATE TABLE IF NOT EXISTS schema_version (
  version    INTEGER PRIMARY KEY,
  applied_at TEXT    NOT NULL                       -- ISO 8601 UTC
);
