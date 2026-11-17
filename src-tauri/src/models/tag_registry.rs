// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Metadata tag registry — now powered by MeedyaSuite-core
// ========================================================
//
// This module delegates to `meedya_core::metadata::tag_registry` for all tag
// definition loading, JSON path extraction, and value conversion. The tag
// list used is the meedya-metadata crate's own compiled-in default list
// (`DEFAULT_REGISTRY`), NOT this project's `src-tauri/tags.toml`, which no
// code reads and whose names differ (see #1444).
//
// One entry of that shared list is set aside on purpose: `last_modified_date`.
// The pinned version of the shared list reads Apple's list of an album's old
// IDs (`meta.formerIds`) where the date belongs. MeedyaDL writes the real date
// itself, as `MeedyaMeta:LastModified`, and only into files the
// current download wrote (see `NOT_USED_FROM_SHARED_REGISTRY`, #1444).
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

/// The name (inside the `MeedyaMeta` group) of the tag that holds the date the
/// service the file came from last updated the album (#1444). One name, used
/// everywhere, and service-neutral: other services and other MeedyaSuite apps
/// use the same tag.
pub const SOURCE_LAST_MODIFIED_TAG: &str = "LastModified";

/// The name (inside the `com.apple.iTunes` group) of the second tag that
/// holds the same date, so apps that read the iTunes-style album tags find it
/// too. It always carries exactly the same value as `MeedyaMeta:LastModified`
/// (maintainer's decision, 17 Nov 2026; MWBMPartners/MeedyaSuite-core#115).
pub const ITUNES_LAST_MODIFIED_TAG: &str = "AlbumLastModified";

/// Entries of the shared MeedyaSuite-core list that MeedyaDL does not use,
/// because the pinned version reads the wrong field. `last_modified_date`
/// there reads `meta.formerIds` (Apple's list of an album's old IDs), not the
/// date. MeedyaDL writes the real date itself as
/// `MeedyaMeta:LastModified` (#1444), and only into files the
/// current download wrote, which the shared list cannot do (it would stamp
/// every file in the folder whose album matches). Even after the shared list
/// is fixed, keep this entry set aside unless that freshness rule is dropped.
const NOT_USED_FROM_SHARED_REGISTRY: &[&str] = &["last_modified_date"];

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
        .filter(|(id, _)| !NOT_USED_FROM_SHARED_REGISTRY.contains(&id.as_str()))
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

/// Converts one tag definition from the shared MeedyaSuite-core registry into
/// MeedyaDL's own type. Each atom's namespace is mapped to this app's
/// constants; an unknown namespace falls back to the iTunes one.
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

    #[test]
    fn the_wrong_last_modified_entry_is_not_loaded() {
        let registry = load_tag_registry();
        assert!(
            !registry
                .album_tags
                .iter()
                .any(|d| d.id == "last_modified_date"),
            "the shared last_modified_date entry must be set aside (#1444)"
        );
        assert!(
            !registry
                .album_tags
                .iter()
                .chain(registry.track_tags.iter())
                .any(|d| d.atoms.iter().any(|a| a.name == "AppleLastModified")),
            "no tag named AppleLastModified may be loaded (#1444)"
        );
        // Nor may any other shared entry write either last-updated tag: the
        // registry walk would stamp it on every album-matched file in the
        // folder, older ones included. MeedyaDL writes both itself.
        assert!(
            !registry
                .album_tags
                .iter()
                .chain(registry.track_tags.iter())
                .any(|d| d.atoms.iter().any(|a| {
                    (a.namespace == ITUNES_NAMESPACE && a.name == ITUNES_LAST_MODIFIED_TAG)
                        || (a.namespace == MEEDYADL_NAMESPACE && a.name == SOURCE_LAST_MODIFIED_TAG)
                })),
            "no shared entry may write the last-updated tags (#1444)"
        );
    }

    /// Fails when MeedyaSuite-core changes the entry, so the set-aside gets
    /// looked at again instead of staying for ever.
    #[test]
    fn tripwire_shared_registry_still_reads_the_wrong_field() {
        let core = &*meedya_core::metadata::tag_registry::DEFAULT_REGISTRY;
        let entry = core
            .album_tags
            .iter()
            .find(|(id, _)| id == "last_modified_date")
            .map(|(_, def)| def)
            .expect("MeedyaSuite-core no longer has last_modified_date: see #1444");
        assert_eq!(
            entry.json_path, "meta.formerIds",
            "MeedyaSuite-core has changed this entry: see #1444 and revisit \
             NOT_USED_FROM_SHARED_REGISTRY"
        );
    }
}
