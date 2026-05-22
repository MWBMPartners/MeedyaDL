// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Engine registry service — runtime query layer for engines.toml.
// ===============================================================
//
// Provides typed access to the engine and platform configuration defined
// in `engines.toml` (compiled into the binary via `include_str!`).
//
// This is the single source of truth for which download engine handles
// which media service. The download queue, dependency manager, and
// frontend all query this registry instead of hardcoding engine knowledge.
//
// ## Usage
//
//   let registry = EngineRegistry::load();
//   let engine = registry.resolve_engine("apple-music");
//   // → Some(Engine { id: "gamdl", name: "GAMDL", ... })

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Raw engines.toml content compiled into the binary.
const ENGINES_TOML: &str = include_str!("../../engines.toml");

/// A download engine (external tool) definition from engines.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Engine {
    /// Unique engine identifier (e.g., "gamdl", "votify", "ytdlp").
    pub id: String,
    /// Human-readable name (e.g., "GAMDL", "yt-dlp").
    pub name: String,
    /// Whether the engine is required for setup.
    pub required: bool,
    /// Whether the engine is currently enabled.
    pub enabled: bool,
    /// Whether the engine is bundled with MeedyaDL.
    pub bundled: bool,
    /// Installation method: "pip", "binary", or "system".
    pub install_method: String,
    /// PyPI package name (for pip-installed engines).
    pub pip_package: Option<String>,
    /// CLI invocation command.
    pub cli_command: String,
    /// Project homepage URL.
    pub homepage: String,
    /// Short description.
    pub description: String,
}

/// A platform (media service) definition from engines.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    /// Platform identifier (e.g., "apple-music", "spotify").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Whether the platform is enabled in the UI.
    pub enabled: bool,
    /// Optional icon path.
    pub icon: Option<String>,
    /// URL hostname patterns for auto-detection.
    pub url_patterns: Vec<String>,
    /// Ordered engine IDs (first = primary, rest = fallback).
    pub engines: Vec<String>,
    /// Content types this platform supports.
    pub content_types: Vec<String>,
}

/// The loaded engine registry — provides lookups by platform and engine ID.
#[derive(Debug, Clone)]
pub struct EngineRegistry {
    /// All engines keyed by ID.
    pub engines: HashMap<String, Engine>,
    /// All platforms keyed by ID.
    pub platforms: HashMap<String, Platform>,
}

impl EngineRegistry {
    /// Loads the engine registry from the compiled-in engines.toml.
    pub fn load() -> Self {
        let parsed: toml::Value = toml::from_str(ENGINES_TOML)
            .expect("engines.toml is compiled into the binary and must be valid TOML");

        let engines = Self::parse_engines(&parsed);
        let platforms = Self::parse_platforms(&parsed);

        Self { engines, platforms }
    }

    /// Resolves the primary engine for a platform ID (e.g., "apple-music" → GAMDL engine).
    pub fn resolve_engine(&self, platform_id: &str) -> Option<&Engine> {
        let platform = self.platforms.get(platform_id)?;
        let primary_engine_id = platform.engines.first()?;
        self.engines.get(primary_engine_id)
    }

    /// Returns the ordered list of engines for a platform (primary + fallbacks).
    pub fn resolve_engine_chain(&self, platform_id: &str) -> Vec<&Engine> {
        let Some(platform) = self.platforms.get(platform_id) else {
            return vec![];
        };
        platform
            .engines
            .iter()
            .filter_map(|id| self.engines.get(id))
            .collect()
    }

    /// Detects the platform from a URL by matching against url_patterns.
    pub fn detect_platform(&self, url: &str) -> Option<&Platform> {
        let lower = url.to_lowercase();
        self.platforms.values().find(|p| {
            p.enabled && p.url_patterns.iter().any(|pat| lower.contains(pat))
        })
    }

    fn parse_engines(parsed: &toml::Value) -> HashMap<String, Engine> {
        let Some(engines_table) = parsed.get("engines").and_then(|e| e.as_table()) else {
            return HashMap::new();
        };

        engines_table
            .iter()
            .filter_map(|(id, def)| {
                Some(Engine {
                    id: id.clone(),
                    name: def.get("name")?.as_str()?.to_string(),
                    required: def.get("required").and_then(|v| v.as_bool()).unwrap_or(false),
                    enabled: def.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                    bundled: def.get("bundled").and_then(|v| v.as_bool()).unwrap_or(false),
                    install_method: def.get("install_method")?.as_str()?.to_string(),
                    pip_package: def.get("pip_package").and_then(|v| v.as_str()).map(String::from),
                    cli_command: def.get("cli_command")?.as_str()?.to_string(),
                    homepage: def.get("homepage").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    description: def.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                })
            })
            .map(|e| (e.id.clone(), e))
            .collect()
    }

    fn parse_platforms(parsed: &toml::Value) -> HashMap<String, Platform> {
        let Some(platforms_table) = parsed.get("platforms").and_then(|p| p.as_table()) else {
            return HashMap::new();
        };

        platforms_table
            .iter()
            .filter_map(|(id, def)| {
                Some(Platform {
                    id: id.clone(),
                    name: def.get("name")?.as_str()?.to_string(),
                    enabled: def.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                    icon: def.get("icon").and_then(|v| v.as_str()).map(String::from),
                    url_patterns: def
                        .get("url_patterns")?
                        .as_array()?
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                    engines: def
                        .get("engines")?
                        .as_array()?
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                    content_types: def
                        .get("content_types")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                })
            })
            .map(|p| (p.id.clone(), p))
            .collect()
    }
}

// ============================================================
// IPC-friendly types
// ============================================================

/// Engine configuration returned to the frontend via IPC.
#[derive(Debug, Clone, Serialize)]
pub struct EngineConfig {
    pub engines: Vec<Engine>,
    pub platforms: Vec<Platform>,
}

impl From<&EngineRegistry> for EngineConfig {
    fn from(registry: &EngineRegistry) -> Self {
        Self {
            engines: registry.engines.values().cloned().collect(),
            platforms: registry.platforms.values().cloned().collect(),
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_registry() {
        let registry = EngineRegistry::load();
        assert!(registry.engines.contains_key("gamdl"));
        assert!(registry.platforms.contains_key("apple-music"));
    }

    #[test]
    fn test_resolve_engine() {
        let registry = EngineRegistry::load();
        let engine = registry.resolve_engine("apple-music");
        assert!(engine.is_some());
        assert_eq!(engine.unwrap().id, "gamdl");
    }

    #[test]
    fn test_resolve_engine_chain() {
        let registry = EngineRegistry::load();
        // BBC iPlayer has two engines: get_iplayer (primary) + ytdlp (fallback)
        let chain = registry.resolve_engine_chain("bbc-iplayer");
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].id, "get_iplayer");
        assert_eq!(chain[1].id, "ytdlp");
    }

    #[test]
    fn test_detect_platform() {
        let registry = EngineRegistry::load();
        let platform = registry.detect_platform("https://music.apple.com/us/album/test/123");
        assert!(platform.is_some());
        assert_eq!(platform.unwrap().id, "apple-music");
    }

    #[test]
    fn test_unknown_url() {
        let registry = EngineRegistry::load();
        assert!(registry.detect_platform("https://example.com/foo").is_none());
    }
}
