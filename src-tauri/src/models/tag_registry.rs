// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Metadata tag registry — now powered by MeedyaSuite-core
// ========================================================
//
// This module delegates to `meedya_core::metadata::tag_registry` for all tag
// definition loading, JSON path extraction, and value conversion. The local
// `tags.toml` is still used as the source of truth (compiled in via the
// meedya-metadata crate's default tags).
//
// ## Migration Notes
//
// Previously this module contained ~560 lines of custom TOML parsing, JSON
// path extraction, and value conversion. All of that logic now lives in
// `meedya-core` and is shared across MeedyaDL, MeedyaConverter, and
// MeedyaManager.
//
// @see meedya_core::metadata::tag_registry — Core implementation
// @see services/metadata_tag_service.rs — Calls write_tags_from_registry()

use std::sync::LazyLock;

// Re-export types from meedya-core for backward compatibility
pub use meedya_core::metadata::tag_registry::{
    extract_json_value, value_to_string, AtomTarget, TagDefinition as CoreTagDefinition,
    TagRegistry as CoreTagRegistry, TagScope, TagValueType,
};

/// iTunes freeform atom namespace (player-compatible).
pub const ITUNES_NAMESPACE: &str = "com.apple.iTunes";

/// MeedyaDL-branded freeform atom namespace.
pub const MEEDYADL_NAMESPACE: &str = "MeedyaMeta";

/// Tag definition with ID included for backward compatibility.
#[derive(Debug, Clone)]
pub struct TagDefinition {
    pub id: String,
    pub json_path: String,
    pub value_type: TagValueType,
    pub atoms: Vec<ResolvedAtomTarget>,
}

/// Atom target with resolved (full) namespace string.
#[derive(Debug, Clone)]
pub struct ResolvedAtomTarget {
    pub namespace: &'static str,
    pub name: String,
}

/// Complete tag registry containing album-scope and track-scope definitions.
#[derive(Debug, Clone)]
pub struct TagRegistry {
    pub album_tags: Vec<TagDefinition>,
    pub track_tags: Vec<TagDefinition>,
}

/// Lazily-loaded global tag registry. Parsed once on first access.
pub static TAG_REGISTRY: LazyLock<TagRegistry> = LazyLock::new(load_tag_registry);

/// Load the tag registry from meedya-core's built-in tags.toml.
pub fn load_tag_registry() -> TagRegistry {
    let core_registry = &*meedya_core::metadata::tag_registry::DEFAULT_REGISTRY;

    let album_tags = core_registry
        .album_tags
        .iter()
        .map(|(id, def)| convert_core_definition(id, def))
        .collect();

    let track_tags = core_registry
        .track_tags
        .iter()
        .map(|(id, def)| convert_core_definition(id, def))
        .collect();

    TagRegistry {
        album_tags,
        track_tags,
    }
}

fn convert_core_definition(id: &str, def: &CoreTagDefinition) -> TagDefinition {
    let atoms = def
        .atoms
        .iter()
        .map(|atom| ResolvedAtomTarget {
            namespace: match atom.resolved_namespace() {
                "com.apple.iTunes" => ITUNES_NAMESPACE,
                "MeedyaMeta" => MEEDYADL_NAMESPACE,
                _ => ITUNES_NAMESPACE, // fallback
            },
            name: atom.name.clone(),
        })
        .collect();

    TagDefinition {
        id: id.to_string(),
        json_path: def.json_path.clone(),
        value_type: def.value_type.clone(),
        atoms,
    }
}

/// Get all JSON paths defined in the tag registry (for audit diffing).
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn value_to_string_string_type() {
        let val = serde_json::json!("hello");
        assert_eq!(
            value_to_string(&val, &TagValueType::String),
            Some("hello".to_string())
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
    fn all_known_paths_includes_both_scopes() {
        let registry = load_tag_registry();
        let paths = all_known_paths(&registry);
        assert!(paths.iter().any(|(scope, _, _)| scope == "album"));
        assert!(paths.iter().any(|(scope, _, _)| scope == "track"));
    }
}
