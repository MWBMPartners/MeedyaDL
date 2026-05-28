// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License.
//
// IPC surface for the SQLite download index (#875 M4).
//
// These commands expose the read-only query helpers from
// `services::download_index::queries` so the Library Scan + Activity
// Log frontends can:
//   * search the activity-log events with SQL-grade filters,
//   * enumerate library gaps (known tracks not yet downloaded),
//   * upsert known tracks discovered from external sources (Apple
//     Music library sync, MusicBrainz, user-added).
//
// All commands fail soft: returning an empty list / Ok(None) when
// the DB is unavailable rather than surfacing an error. The DB is
// an indexed cache; the existing JSON-backed flows remain
// authoritative.

use tauri::AppHandle;

use crate::services::download_index::queries::{
    list_library_gaps, search_activity_events, upsert_known_track, ActivityEventFilter,
    ActivityEventRow, KnownTrackRow,
};

/// IPC-friendly shape that mirrors [`ActivityEventFilter`] but owns
/// its strings so it can be deserialised from JSON. The query helper
/// borrows its filter slices, so we hold the owned strings here and
/// pass references in.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ActivityEventFilterPayload {
    #[serde(default)]
    pub message_contains: Option<String>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub since: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    200
}

/// Search the activity-log events table via the DB-backed indexed
/// query (#875 M4). Used by the Activity Log search/filter UI.
///
/// Returns an empty Vec on any DB error so the frontend can render
/// "no results" gracefully instead of surfacing a crash.
#[tauri::command]
pub async fn search_activity_log_db(
    app: AppHandle,
    filter: ActivityEventFilterPayload,
) -> Result<Vec<ActivityEventRow>, String> {
    let db_path = crate::utils::platform::get_app_data_dir(&app).join("meedyadl.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let f = ActivityEventFilter {
        message_contains: filter.message_contains.as_deref(),
        level: filter.level.as_deref(),
        source: filter.source.as_deref(),
        since: filter.since.as_deref(),
        limit: filter.limit,
    };
    match search_activity_events(&db_path, &f) {
        Ok(rows) => Ok(rows),
        Err(e) => {
            log::warn!("search_activity_log_db failed (returning empty result): {e}");
            Ok(Vec::new())
        }
    }
}

/// List "library gaps" — tracks present in the `known_tracks` table
/// but absent from `downloads` (any codec). Drives the gap-fill
/// surface in the Library Scan UI (#717 5b-5f groundwork via #875
/// M4). Returns `[]` on any DB error.
#[tauri::command]
pub async fn list_library_gaps_cmd(
    app: AppHandle,
    limit: Option<usize>,
) -> Result<Vec<KnownTrackRow>, String> {
    let db_path = crate::utils::platform::get_app_data_dir(&app).join("meedyadl.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let limit = limit.unwrap_or(500);
    match list_library_gaps(&db_path, limit) {
        Ok(rows) => Ok(rows),
        Err(e) => {
            log::warn!("list_library_gaps_cmd failed (returning empty result): {e}");
            Ok(Vec::new())
        }
    }
}

/// Insert (or update) a row in `known_tracks`. Surfaces the future
/// Apple Music library sync / MusicBrainz / user-added gap-fill
/// inputs through the same DB path so the gap query above sees
/// them. Returns `false` on DB error so the frontend can show a
/// non-fatal "couldn't index this track" message.
#[tauri::command]
pub async fn upsert_known_track_cmd(
    app: AppHandle,
    track: KnownTrackRow,
) -> Result<bool, String> {
    let db_path = crate::utils::platform::get_app_data_dir(&app).join("meedyadl.db");
    if !db_path.exists() {
        return Ok(false);
    }
    match upsert_known_track(&db_path, &track) {
        Ok(()) => Ok(true),
        Err(e) => {
            log::warn!("upsert_known_track_cmd failed: {e}");
            Ok(false)
        }
    }
}
