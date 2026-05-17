// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// API field audit service — discover new/unknown Apple Music API fields
// =====================================================================
//
// Fetches a real album from the Apple Music API and compares all JSON
// attribute paths against the known tag definitions in `tags.toml`.
// Reports fields that are:
//   - **Known**: present in both the API response and tags.toml
//   - **Unknown**: present in the API response but NOT in tags.toml
//   - **Missing**: defined in tags.toml but absent from this particular album
//
// This is a developer/diagnostic tool — it does NOT auto-embed unknown
// fields. Its purpose is to surface new API additions for human review.
//
// @see models/tag_registry.rs — Tag definitions loaded from tags.toml
// @see services/apple_music_api.rs — API client used for fetching

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::models::tag_registry::{self, TAG_REGISTRY};
use crate::services::{apple_music_api, config_service};

// ============================================================
// Public Types
// ============================================================

/// Result of auditing an Apple Music API response against the tag registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiAuditResult {
    /// Apple Music album ID that was audited.
    pub album_id: String,
    /// Album name (for display).
    pub album_name: Option<String>,
    /// Total unique JSON attribute paths found in the album-level response.
    pub total_album_fields: usize,
    /// Total unique JSON attribute paths found in the first track's response.
    pub total_track_fields: usize,
    /// Fields that are mapped to tags in tags.toml.
    pub known_fields: Vec<AuditField>,
    /// Fields in the API response but NOT in tags.toml.
    pub unknown_fields: Vec<AuditField>,
    /// Tag IDs defined in tags.toml but not found in this API response.
    pub missing_fields: Vec<String>,
    /// Number of tracks in the album.
    pub track_count: usize,
}

/// A single field discovered in the API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditField {
    /// Scope: "album" or "track".
    pub scope: String,
    /// Dotted JSON path (e.g., "attributes.playParams.id").
    pub json_path: String,
    /// JSON value type ("string", "number", "boolean", "array", "object", "null").
    pub value_type: String,
    /// Sample value (truncated to 200 chars for display safety).
    pub sample_value: Option<String>,
    /// Which tag ID in tags.toml maps to this field (None for unknown fields).
    pub mapped_tag_id: Option<String>,
}

// ============================================================
// Public API
// ============================================================

/// Fetch an album from the Apple Music API and audit its fields against tags.toml.
///
/// Requires MusicKit credentials (Team ID, Key ID, private key in keychain).
///
/// # Arguments
/// * `app` - Tauri AppHandle for settings and keychain access
/// * `album_url` - Apple Music album URL to audit
///
/// # Returns
/// An `ApiAuditResult` with known/unknown/missing field breakdowns.
pub async fn audit_album_fields(
    app: &AppHandle,
    album_url: String,
) -> Result<ApiAuditResult, String> {
    // Parse the URL
    let normalized = apple_music_api::normalize_apple_music_url(&album_url);
    let parsed = apple_music_api::parse_apple_music_url(&normalized)
        .ok_or("Invalid Apple Music URL — must be an album URL")?;

    if parsed.content_type != "album" {
        return Err("API audit requires an album URL (not a song or music video)".to_string());
    }

    // Load MusicKit credentials
    let settings = config_service::load_settings(app).unwrap_or_default();
    let team_id = settings
        .musickit_team_id
        .as_ref()
        .filter(|s| !s.is_empty())
        .ok_or("MusicKit Team ID not configured")?;
    let key_id = settings
        .musickit_key_id
        .as_ref()
        .filter(|s| !s.is_empty())
        .ok_or("MusicKit Key ID not configured")?;

    let private_key = apple_music_api::get_private_key_from_keychain()
        .map_err(|e| format!("Failed to read private key: {e}"))?
        .ok_or("MusicKit private key not found in keychain")?;

    // Generate JWT and fetch raw JSON
    let jwt = apple_music_api::generate_musickit_jwt(team_id, key_id, &private_key)?;
    let metadata = apple_music_api::fetch_album_metadata_with_fallback(
        &jwt,
        &parsed.storefront,
        &parsed.album_id,
    )
    .await?
    .ok_or("Album not found in Apple Music catalog")?;

    // Build the audit result by comparing raw JSON against tags.toml
    let registry = &*TAG_REGISTRY;
    let album_json = &metadata.raw_json;

    // Flatten all JSON paths from the album object
    let mut album_paths = BTreeSet::new();
    flatten_json_paths(album_json, "", &mut album_paths);

    // Flatten the first track's JSON paths (representative sample)
    let mut track_paths = BTreeSet::new();
    let first_track_json = metadata.tracks.first().map(|t| &t.raw_json);
    if let Some(track_json) = first_track_json {
        flatten_json_paths(track_json, "", &mut track_paths);
    }

    // Build known paths set from tag registry
    let known_tag_paths = tag_registry::all_known_paths(registry);

    let mut known_fields = Vec::new();
    let mut unknown_fields = Vec::new();
    let mut found_tag_ids = BTreeSet::new();

    // Check album paths against known album tag definitions
    for path in &album_paths {
        let matched = known_tag_paths
            .iter()
            .find(|(scope, _, json_path)| scope == "album" && json_path == path);

        if let Some((_, tag_id, _)) = matched {
            found_tag_ids.insert(format!("album.{tag_id}"));
            known_fields.push(AuditField {
                scope: "album".to_string(),
                json_path: path.clone(),
                value_type: json_type_at_path(album_json, path),
                sample_value: sample_value_at_path(album_json, path),
                mapped_tag_id: Some(tag_id.clone()),
            });
        } else {
            unknown_fields.push(AuditField {
                scope: "album".to_string(),
                json_path: path.clone(),
                value_type: json_type_at_path(album_json, path),
                sample_value: sample_value_at_path(album_json, path),
                mapped_tag_id: None,
            });
        }
    }

    // Check track paths against known track tag definitions
    if let Some(track_json) = first_track_json {
        for path in &track_paths {
            let matched = known_tag_paths
                .iter()
                .find(|(scope, _, json_path)| scope == "track" && json_path == path);

            if let Some((_, tag_id, _)) = matched {
                found_tag_ids.insert(format!("track.{tag_id}"));
                known_fields.push(AuditField {
                    scope: "track".to_string(),
                    json_path: path.clone(),
                    value_type: json_type_at_path(track_json, path),
                    sample_value: sample_value_at_path(track_json, path),
                    mapped_tag_id: Some(tag_id.clone()),
                });
            } else {
                unknown_fields.push(AuditField {
                    scope: "track".to_string(),
                    json_path: path.clone(),
                    value_type: json_type_at_path(track_json, path),
                    sample_value: sample_value_at_path(track_json, path),
                    mapped_tag_id: None,
                });
            }
        }
    }

    // Find tags defined in tags.toml but missing from this API response
    let missing_fields: Vec<String> = known_tag_paths
        .iter()
        .map(|(scope, tag_id, _)| format!("{scope}.{tag_id}"))
        .filter(|full_id| !found_tag_ids.contains(full_id))
        .collect();

    Ok(ApiAuditResult {
        album_id: metadata.album_id,
        album_name: metadata.album_name,
        total_album_fields: album_paths.len(),
        total_track_fields: track_paths.len(),
        known_fields,
        unknown_fields,
        missing_fields,
        track_count: metadata.tracks.len(),
    })
}

// ============================================================
// Internal: JSON Path Flattening
// ============================================================

/// Recursively flatten a JSON value into dotted path strings.
///
/// Objects emit paths for each key and recurse into nested values.
/// Arrays emit a `[0]` indexed path for the first element only
/// (representative sample, avoids enumerating every array element).
/// Leaf values (string, number, bool, null) are recorded by the parent.
fn flatten_json_paths(value: &serde_json::Value, prefix: &str, paths: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                paths.insert(path.clone());
                flatten_json_paths(val, &path, paths);
            }
        }
        serde_json::Value::Array(arr) => {
            if let Some(first) = arr.first() {
                let indexed = format!("{prefix}[0]");
                paths.insert(indexed.clone());
                flatten_json_paths(first, &indexed, paths);
            }
        }
        _ => {
            // Leaf value — already recorded by parent traversal
        }
    }
}

/// Get the JSON type name at a given dotted path.
fn json_type_at_path(json: &serde_json::Value, path: &str) -> String {
    match tag_registry::extract_json_value(json, path) {
        Some(val) => json_type_name(&val).to_string(),
        None => "missing".to_string(),
    }
}

/// Get a truncated sample value at a given dotted path.
fn sample_value_at_path(json: &serde_json::Value, path: &str) -> Option<String> {
    let val = tag_registry::extract_json_value(json, path)?;
    let sample = match &val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            let items: Vec<&str> = arr.iter().filter_map(serde_json::Value::as_str).collect();
            items.join(", ")
        }
        other => other.to_string(),
    };

    // Truncate long values for display safety
    if sample.len() > 200 {
        Some(format!("{}...", &sample[..200]))
    } else {
        Some(sample)
    }
}

/// Classify a JSON value's type as a human-readable string.
fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Null => "null",
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_simple_object() {
        let json = serde_json::json!({
            "attributes": {
                "name": "Test",
                "isrc": "US1234"
            }
        });
        let mut paths = BTreeSet::new();
        flatten_json_paths(&json, "", &mut paths);
        assert!(paths.contains("attributes"));
        assert!(paths.contains("attributes.name"));
        assert!(paths.contains("attributes.isrc"));
    }

    #[test]
    fn flatten_nested_object() {
        let json = serde_json::json!({
            "attributes": {
                "editorialNotes": {
                    "short": "Great album"
                }
            }
        });
        let mut paths = BTreeSet::new();
        flatten_json_paths(&json, "", &mut paths);
        assert!(paths.contains("attributes.editorialNotes"));
        assert!(paths.contains("attributes.editorialNotes.short"));
    }

    #[test]
    fn flatten_array_indexes_first_element() {
        let json = serde_json::json!({
            "attributes": {
                "previews": [
                    { "url": "https://example.com" }
                ]
            }
        });
        let mut paths = BTreeSet::new();
        flatten_json_paths(&json, "", &mut paths);
        assert!(paths.contains("attributes.previews"));
        assert!(paths.contains("attributes.previews[0]"));
        assert!(paths.contains("attributes.previews[0].url"));
    }

    #[test]
    fn json_type_name_string() {
        assert_eq!(json_type_name(&serde_json::json!("test")), "string");
    }

    #[test]
    fn json_type_name_number() {
        assert_eq!(json_type_name(&serde_json::json!(42)), "number");
    }

    #[test]
    fn json_type_name_boolean() {
        assert_eq!(json_type_name(&serde_json::json!(true)), "boolean");
    }

    #[test]
    fn json_type_name_array() {
        assert_eq!(json_type_name(&serde_json::json!(["a", "b"])), "array");
    }

    #[test]
    fn json_type_name_object() {
        assert_eq!(json_type_name(&serde_json::json!({"a": 1})), "object");
    }

    #[test]
    fn json_type_name_null() {
        assert_eq!(json_type_name(&serde_json::json!(null)), "null");
    }

    #[test]
    fn sample_value_truncates_long_strings() {
        let long_string = "a".repeat(300);
        let json = serde_json::json!({ "long": long_string });
        let sample = sample_value_at_path(&json, "long");
        assert!(sample.is_some());
        assert!(sample.unwrap().len() <= 204); // 200 + "..."
    }
}
