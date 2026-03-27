// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Metadata tag registry — config-driven tag definitions from tags.toml
// =====================================================================
//
// Loads tag definitions from the compiled-in `tags.toml` file and provides
// functions to extract values from raw Apple Music API JSON responses and
// convert them to MP4 freeform atom strings.
//
// ## Design
//
// Adding a new metadata tag requires only editing `tags.toml` — zero Rust
// code changes. The registry defines:
//   - Which JSON path to extract from the raw API response
//   - How to convert the value to a UTF-8 string
//   - Which freeform atoms to write (namespace + name)
//
// ## Pattern
//
// Follows the `codec_registry.rs` pattern: `include_str!` → `toml::from_str()`
// → typed public API with sorted, deterministic output.
//
// @see tags.toml — The TOML config file defining all tag mappings
// @see services/metadata_tag_service.rs — Calls write_tags_from_registry()

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

/// Compiled-in tags.toml content. Changes require app rebuild.
const TAGS_TOML: &str = include_str!("../../tags.toml");

/// iTunes freeform atom namespace (player-compatible).
const ITUNES_NAMESPACE: &str = "com.apple.iTunes";

/// MeedyaDL-branded freeform atom namespace.
const MEEDYADL_NAMESPACE: &str = "MeedyaMeta";

/// Lazily-loaded global tag registry. Parsed once on first access.
pub static TAG_REGISTRY: LazyLock<TagRegistry> = LazyLock::new(load_tag_registry);

// ============================================================
// Public Types
// ============================================================

/// Complete tag registry containing album-scope and track-scope definitions.
#[derive(Debug, Clone)]
pub struct TagRegistry {
    /// Tags written to every track in the album (same value per file).
    pub album_tags: Vec<TagDefinition>,
    /// Tags written per-track (matched by track/disc number).
    pub track_tags: Vec<TagDefinition>,
}

/// A single tag definition from tags.toml.
#[derive(Debug, Clone)]
pub struct TagDefinition {
    /// Unique identifier (e.g., "content_rating", "isrc").
    pub id: String,
    /// Dot-separated path into the raw API JSON (e.g., "attributes.isrc").
    pub json_path: String,
    /// How to convert the JSON value to a UTF-8 string.
    pub value_type: TagValueType,
    /// Freeform atoms to write this value to.
    pub atoms: Vec<AtomTarget>,
}

/// Rules for converting a JSON value to a freeform atom UTF-8 string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagValueType {
    /// Direct string value.
    String,
    /// JSON bool → "true" or "false".
    Bool,
    /// JSON number → decimal string (u32 range).
    U32,
    /// JSON number → decimal string (u64 range).
    U64,
    /// JSON array of strings → comma-separated (e.g., "Pop, Music").
    Array,
    /// First element of a JSON string array (e.g., primary genre).
    FirstOfArray,
}

/// A target freeform atom (namespace + name).
#[derive(Debug, Clone)]
pub struct AtomTarget {
    /// Full namespace string (e.g., "com.apple.iTunes" or "MeedyaMeta").
    pub namespace: &'static str,
    /// Atom name (e.g., "ISRC", "AppleReleaseDate").
    pub name: String,
}

// ============================================================
// TOML Deserialization Types (private)
// ============================================================

#[derive(Deserialize)]
struct RawTagsToml {
    #[serde(default)]
    album: HashMap<String, RawTagDefinition>,
    #[serde(default)]
    track: HashMap<String, RawTagDefinition>,
}

#[derive(Deserialize)]
struct RawTagDefinition {
    json_path: String,
    value_type: String,
    atoms: Vec<RawAtom>,
}

#[derive(Deserialize)]
struct RawAtom {
    namespace: String,
    name: String,
}

// ============================================================
// Loading
// ============================================================

/// Parse the compiled-in tags.toml into a `TagRegistry`.
///
/// Prefer using the `TAG_REGISTRY` static for cached access.
pub fn load_tag_registry() -> TagRegistry {
    let raw: RawTagsToml =
        toml::from_str(TAGS_TOML).expect("Failed to parse compiled-in tags.toml");

    let mut album_tags: Vec<TagDefinition> = raw
        .album
        .into_iter()
        .map(|(id, raw_def)| convert_definition(id, raw_def))
        .collect();
    album_tags.sort_by(|a, b| a.id.cmp(&b.id));

    let mut track_tags: Vec<TagDefinition> = raw
        .track
        .into_iter()
        .map(|(id, raw_def)| convert_definition(id, raw_def))
        .collect();
    track_tags.sort_by(|a, b| a.id.cmp(&b.id));

    TagRegistry {
        album_tags,
        track_tags,
    }
}

/// Convert a raw TOML definition into a typed `TagDefinition`.
fn convert_definition(id: String, raw: RawTagDefinition) -> TagDefinition {
    let value_type = match raw.value_type.as_str() {
        "string" => TagValueType::String,
        "bool" => TagValueType::Bool,
        "u32" => TagValueType::U32,
        "u64" => TagValueType::U64,
        "array" => TagValueType::Array,
        "first_of_array" => TagValueType::FirstOfArray,
        other => panic!("Unknown value_type '{other}' in tags.toml for tag '{id}'"),
    };

    let atoms = raw
        .atoms
        .into_iter()
        .map(|raw_atom| AtomTarget {
            namespace: match raw_atom.namespace.as_str() {
                "itunes" => ITUNES_NAMESPACE,
                "meedya" => MEEDYADL_NAMESPACE,
                other => panic!(
                    "Unknown namespace '{other}' in tags.toml for tag '{id}' \
                     (expected 'itunes' or 'meedya')"
                ),
            },
            name: raw_atom.name,
        })
        .collect();

    TagDefinition {
        id,
        json_path: raw.json_path,
        value_type,
        atoms,
    }
}

// ============================================================
// JSON Path Extraction
// ============================================================

/// Extract a value from a `serde_json::Value` using a dotted path.
///
/// Supports:
/// - Simple paths: `"attributes.name"` → `json["attributes"]["name"]`
/// - Nested objects: `"attributes.editorialNotes.short"`
/// - Array indexing: `"attributes.previews[0].url"`
/// - Relationship paths: `"relationships.artists.data[0].id"`
///
/// Returns `None` if any segment of the path is missing or the index is
/// out of bounds.
pub fn extract_json_value(json: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let mut current = json;

    for segment in path.split('.') {
        // Check for array index notation: "data[0]", "previews[0]"
        if let Some(bracket_pos) = segment.find('[') {
            // Extract the key part before the bracket
            let key = &segment[..bracket_pos];
            // Extract the index from between [ and ]
            let index_str = segment[bracket_pos + 1..].strip_suffix(']')?;
            let index: usize = index_str.parse().ok()?;

            // Navigate: key → array → index
            if !key.is_empty() {
                current = current.get(key)?;
            }
            current = current.as_array()?.get(index)?;
        } else {
            // Simple object key navigation
            current = current.get(segment)?;
        }
    }

    // Clone the final value (we need an owned Value for the caller)
    Some(current.clone())
}

/// Convert a JSON value to a UTF-8 string according to the `TagValueType` rules.
///
/// Returns `None` if the value cannot be converted (e.g., null, wrong type,
/// or empty array).
pub fn value_to_string(value: &serde_json::Value, value_type: &TagValueType) -> Option<String> {
    match value_type {
        TagValueType::String => {
            // Prefer as_str() for JSON strings; fall back to to_string() for numbers etc.
            value
                .as_str()
                .map(std::string::ToString::to_string)
                .or_else(|| {
                    if value.is_null() {
                        None
                    } else {
                        Some(value.to_string())
                    }
                })
        }
        TagValueType::Bool => value.as_bool().map(|b| b.to_string()),
        TagValueType::U32 | TagValueType::U64 => value.as_u64().map(|n| n.to_string()),
        TagValueType::Array => {
            let arr = value.as_array()?;
            let items: Vec<&str> = arr.iter().filter_map(serde_json::Value::as_str).collect();
            if items.is_empty() {
                None
            } else {
                Some(items.join(", "))
            }
        }
        TagValueType::FirstOfArray => value
            .as_array()?
            .first()
            .and_then(serde_json::Value::as_str)
            .map(std::string::ToString::to_string),
    }
}

// ============================================================
// Query Functions
// ============================================================

/// Get all JSON paths defined in the tag registry (for audit diffing).
///
/// Returns a sorted list of `(scope, tag_id, json_path)` tuples.
pub fn all_known_paths(registry: &TagRegistry) -> Vec<(String, String, String)> {
    let mut paths: Vec<(String, String, String)> = Vec::new();

    for def in &registry.album_tags {
        paths.push(("album".to_string(), def.id.clone(), def.json_path.clone()));
    }
    for def in &registry.track_tags {
        paths.push(("track".to_string(), def.id.clone(), def.json_path.clone()));
    }

    paths.sort();
    paths
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // Registry loading tests
    // ----------------------------------------------------------

    #[test]
    fn load_tag_registry_parses_successfully() {
        let registry = load_tag_registry();
        assert!(
            !registry.album_tags.is_empty(),
            "Should have album tag definitions"
        );
        assert!(
            !registry.track_tags.is_empty(),
            "Should have track tag definitions"
        );
    }

    #[test]
    fn album_tags_count() {
        let registry = load_tag_registry();
        // 17 album tags defined in tags.toml (16 original + lastModifiedDate)
        assert_eq!(registry.album_tags.len(), 17);
    }

    #[test]
    fn track_tags_count() {
        let registry = load_tag_registry();
        // 14 track tags defined in tags.toml
        assert_eq!(registry.track_tags.len(), 14);
    }

    #[test]
    fn album_upc_has_three_atoms() {
        let registry = load_tag_registry();
        let upc = registry
            .album_tags
            .iter()
            .find(|t| t.id == "upc")
            .expect("upc tag should exist");
        assert_eq!(upc.atoms.len(), 3); // UPC, Barcode, AppleUPC
        assert_eq!(upc.value_type, TagValueType::String);
    }

    #[test]
    fn track_isrc_has_one_atom() {
        let registry = load_tag_registry();
        let isrc = registry
            .track_tags
            .iter()
            .find(|t| t.id == "isrc")
            .expect("isrc tag should exist");
        assert_eq!(isrc.atoms.len(), 1);
        assert_eq!(isrc.atoms[0].namespace, ITUNES_NAMESPACE);
        assert_eq!(isrc.atoms[0].name, "ISRC");
    }

    #[test]
    fn track_song_id_has_two_atoms() {
        let registry = load_tag_registry();
        let song_id = registry
            .track_tags
            .iter()
            .find(|t| t.id == "song_id")
            .expect("song_id tag should exist");
        assert_eq!(song_id.atoms.len(), 2); // iTunesCatalogID + StoreID/AppleMusic
    }

    #[test]
    fn album_track_count_has_industry_standard_alt() {
        let registry = load_tag_registry();
        let tc = registry
            .album_tags
            .iter()
            .find(|t| t.id == "track_count")
            .expect("track_count tag should exist");
        assert_eq!(tc.atoms.len(), 3); // AlbumTrackCount + TOTALTRACKS + AppleTrackCount
        assert!(tc.atoms.iter().any(|a| a.name == "TOTALTRACKS"));
    }

    // ----------------------------------------------------------
    // JSON path extraction tests
    // ----------------------------------------------------------

    #[test]
    fn extract_simple_path() {
        let json = serde_json::json!({
            "attributes": {
                "name": "Midnights"
            }
        });
        let result = extract_json_value(&json, "attributes.name");
        assert_eq!(result, Some(serde_json::json!("Midnights")));
    }

    #[test]
    fn extract_nested_path() {
        let json = serde_json::json!({
            "attributes": {
                "editorialNotes": {
                    "short": "A brilliant album."
                }
            }
        });
        let result = extract_json_value(&json, "attributes.editorialNotes.short");
        assert_eq!(result, Some(serde_json::json!("A brilliant album.")));
    }

    #[test]
    fn extract_array_index() {
        let json = serde_json::json!({
            "attributes": {
                "previews": [
                    { "url": "https://example.com/preview.m4a" }
                ]
            }
        });
        let result = extract_json_value(&json, "attributes.previews[0].url");
        assert_eq!(
            result,
            Some(serde_json::json!("https://example.com/preview.m4a"))
        );
    }

    #[test]
    fn extract_relationship_path() {
        let json = serde_json::json!({
            "relationships": {
                "artists": {
                    "data": [
                        { "id": "159260351", "type": "artists" }
                    ]
                }
            }
        });
        let result = extract_json_value(&json, "relationships.artists.data[0].id");
        assert_eq!(result, Some(serde_json::json!("159260351")));
    }

    #[test]
    fn extract_top_level_id() {
        let json = serde_json::json!({
            "id": "1649434005",
            "type": "songs"
        });
        let result = extract_json_value(&json, "id");
        assert_eq!(result, Some(serde_json::json!("1649434005")));
    }

    #[test]
    fn extract_missing_path_returns_none() {
        let json = serde_json::json!({
            "attributes": { "name": "Test" }
        });
        assert!(extract_json_value(&json, "attributes.nonexistent").is_none());
        assert!(extract_json_value(&json, "missing.path").is_none());
    }

    #[test]
    fn extract_array_out_of_bounds_returns_none() {
        let json = serde_json::json!({
            "data": []
        });
        assert!(extract_json_value(&json, "data[0].id").is_none());
    }

    // ----------------------------------------------------------
    // Value conversion tests
    // ----------------------------------------------------------

    #[test]
    fn value_to_string_string_type() {
        let val = serde_json::json!("hello");
        assert_eq!(
            value_to_string(&val, &TagValueType::String),
            Some("hello".to_string())
        );
    }

    #[test]
    fn value_to_string_null_returns_none() {
        let val = serde_json::json!(null);
        assert!(value_to_string(&val, &TagValueType::String).is_none());
    }

    #[test]
    fn value_to_string_bool_true() {
        let val = serde_json::json!(true);
        assert_eq!(
            value_to_string(&val, &TagValueType::Bool),
            Some("true".to_string())
        );
    }

    #[test]
    fn value_to_string_bool_false() {
        let val = serde_json::json!(false);
        assert_eq!(
            value_to_string(&val, &TagValueType::Bool),
            Some("false".to_string())
        );
    }

    #[test]
    fn value_to_string_u64() {
        let val = serde_json::json!(202395);
        assert_eq!(
            value_to_string(&val, &TagValueType::U64),
            Some("202395".to_string())
        );
    }

    #[test]
    fn value_to_string_u32() {
        let val = serde_json::json!(13);
        assert_eq!(
            value_to_string(&val, &TagValueType::U32),
            Some("13".to_string())
        );
    }

    #[test]
    fn value_to_string_array() {
        let val = serde_json::json!(["Pop", "Music"]);
        assert_eq!(
            value_to_string(&val, &TagValueType::Array),
            Some("Pop, Music".to_string())
        );
    }

    #[test]
    fn value_to_string_empty_array_returns_none() {
        let val = serde_json::json!([]);
        assert!(value_to_string(&val, &TagValueType::Array).is_none());
    }

    #[test]
    fn value_to_string_first_of_array() {
        let val = serde_json::json!(["Pop", "Music"]);
        assert_eq!(
            value_to_string(&val, &TagValueType::FirstOfArray),
            Some("Pop".to_string())
        );
    }

    #[test]
    fn value_to_string_first_of_empty_array_returns_none() {
        let val = serde_json::json!([]);
        assert!(value_to_string(&val, &TagValueType::FirstOfArray).is_none());
    }

    // ----------------------------------------------------------
    // Known paths query test
    // ----------------------------------------------------------

    #[test]
    fn all_known_paths_includes_both_scopes() {
        let registry = load_tag_registry();
        let paths = all_known_paths(&registry);
        assert!(paths.iter().any(|(scope, _, _)| scope == "album"));
        assert!(paths.iter().any(|(scope, _, _)| scope == "track"));
        // Total should be album_tags + track_tags
        assert_eq!(
            paths.len(),
            registry.album_tags.len() + registry.track_tags.len()
        );
    }
}
