-- Copyright (c) 2026 MeedyaSuite
-- Licensed under the MIT License.
--
-- MeedyaDL download index — schema v2 (#875 EPIC A M1b)
--
-- Adds a single tiny `meta` table for tracking one-shot operations
-- like the first-startup ingest. Keeps the schema_version table
-- focused purely on migration ordering — sentinels and other
-- key/value flags live here.
--
-- The table is intentionally generic (key/value/set_at) so future
-- milestones (M2 / M3 / M4) can add flags without another schema
-- bump for each one.

CREATE TABLE IF NOT EXISTS meta (
  key     TEXT NOT NULL PRIMARY KEY,
  value   TEXT,
  set_at  TEXT NOT NULL                  -- ISO 8601 UTC
);
