// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Universal codec registry.
// =========================
//
// Provides a service-agnostic codec identity layer for MeedyaDL's
// multi-service architecture (v2+). Each codec has a canonical MeedyaDL
// ID (e.g., "eac3-atmos", "alac", "aac-hq") and per-service CLI flag
// mappings (e.g., gamdl="atmos", votify="aac-high").
//
// ## Design
//
// The registry is loaded from `codecs.toml` at the project root, compiled
// into the binary via `include_str!`. This lets developers configure codec
// mappings in a plain-text TOML file without touching Rust code.
//
// Three codec types:
// - **Audio codecs** — concrete formats (ALAC, AAC, Atmos, Ogg Vorbis, etc.)
// - **Meta codecs** — abstract quality tiers that resolve per-service
//   (e.g., "lossless" → ALAC on Apple Music, FLAC on Spotify)
// - **Video codecs** — H.265, H.264, VP9, AV1, etc.
//
// ## Bridge to Existing Code
//
// The existing `SongCodec` enum (`gamdl_options.rs`) continues to work
// for GAMDL downloads. Bridge functions `song_codec_to_registry_id()` and
// `registry_id_to_song_codec()` convert between the two systems. This
// allows incremental adoption — v2 service routing will use the registry
// directly, while existing GAMDL code keeps using `SongCodec`.
//
// ## Adding a New Service
//
// 1. Add service mappings under `[audio.<id>.services]` in `codecs.toml`
// 2. Use the service engine ID as the key (e.g., "votify", "ytdlp")
// 3. The value is the exact CLI flag string the engine expects

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

use super::gamdl_options::SongCodec;

// ============================================================
// Constants
// ============================================================

/// The codec definitions TOML file, compiled into the binary at build time.
/// Located at `src-tauri/codecs.toml` relative to the project root.
const CODECS_TOML: &str = include_str!("../../codecs.toml");

/// Lazily-loaded codec registry singleton. Parsed once from the compiled-in
/// `codecs.toml` on first access; subsequent calls return a reference to the
/// cached instance. Thread-safe via `LazyLock`.
pub static CODEC_REGISTRY: LazyLock<CodecRegistry> = LazyLock::new(load_registry);

// ============================================================
// Types
// ============================================================

/// A concrete audio codec with per-service CLI flag mappings.
///
/// Each entry represents a specific audio format (e.g., ALAC, Dolby Atmos,
/// AAC 256kbps). The `services` map provides the exact CLI flag value for
/// each supported download engine. Missing entries mean the codec is not
/// available on that service.
#[derive(Debug, Clone)]
pub struct AudioCodecEntry {
    /// Canonical MeedyaDL codec ID (e.g., "eac3-atmos", "alac", "aac-hq").
    /// Kebab-case, unique across all audio codecs.
    pub id: String,
    /// Human-readable display name for the UI (e.g., "Dolby Atmos (E-AC-3)").
    pub name: String,
    /// UI grouping category: "spatial", "lossless", or "lossy".
    pub category: String,
    /// Whether this codec preserves original audio quality without data loss.
    pub lossless: bool,
    /// MIME type for this audio format (e.g., "audio/alac", "audio/aac").
    pub mimetype: Option<String>,
    /// Filename suffix for companion download disambiguation (e.g., "[Lossless]",
    /// "[Dolby Atmos]"). Empty string means the codec uses clean filenames (no suffix).
    /// `None` means no suffix was defined in the registry (treated as empty).
    pub suffix: Option<String>,
    /// Per-service CLI flag mappings. Key = service engine ID (e.g., "gamdl"),
    /// value = exact CLI flag string (e.g., "atmos").
    pub services: HashMap<String, String>,
}

/// A meta codec that resolves to a concrete codec ID per service.
///
/// Meta codecs represent abstract quality tiers (e.g., "lossless", "atmos")
/// that map to the best available concrete codec for each service. This lets
/// users select intent without needing to know which specific codec each
/// service uses for that quality tier.
#[derive(Debug, Clone)]
pub struct MetaCodecEntry {
    /// Meta codec ID (e.g., "lossless", "atmos", "best-lossy").
    pub id: String,
    /// Human-readable display name (e.g., "Lossless (Best Available)").
    pub name: String,
    /// Category hint matching the resolved codec's category.
    pub category: String,
    /// Per-service resolution to a concrete audio codec ID.
    /// Key = service engine ID, value = audio codec ID from the registry.
    pub resolves_to: HashMap<String, String>,
}

/// A video codec with per-service CLI flag mappings.
#[derive(Debug, Clone)]
pub struct VideoCodecEntry {
    /// Canonical video codec ID (e.g., "h265", "h264", "vp9").
    pub id: String,
    /// Human-readable display name (e.g., "H.265 (HEVC)").
    pub name: String,
    /// Category: "modern" or "compatible".
    pub category: String,
    /// MIME type for this video format (e.g., "video/hevc", "video/avc").
    pub mimetype: Option<String>,
    /// Per-service CLI flag mappings.
    pub services: HashMap<String, String>,
}

/// A lyrics/subtitle/caption format entry.
///
/// Represents text-based sidecar formats for synced lyrics and subtitles
/// (TTML, LRC, SRT, WebVTT). Each format has a file extension, MIME type,
/// and per-service support flags.
#[derive(Debug, Clone)]
pub struct LyricsFormatEntry {
    /// Format ID (e.g., "ttml", "lrc", "enhanced-lrc", "srt", "vtt").
    pub id: String,
    /// Human-readable display name (e.g., "TTML (Timed Text Markup Language)").
    pub name: String,
    /// Category: "xml" or "text".
    pub category: String,
    /// File extension without dot (e.g., "ttml", "lrc", "srt", "vtt").
    pub extension: String,
    /// MIME type (e.g., "application/ttml+xml", "text/x-lrc").
    pub mimetype: String,
    /// Whether this format supports word-level timing (inline timestamps).
    pub word_timing: bool,
    /// Per-service CLI flag mappings.
    pub services: HashMap<String, String>,
}

/// The full codec registry containing all audio, meta, video, and lyrics entries.
///
/// Loaded once from `codecs.toml` via `load_registry()` and then queried
/// by the download pipeline to resolve codec IDs to service-specific flags.
#[derive(Debug, Clone)]
pub struct CodecRegistry {
    /// All concrete audio codec entries.
    pub audio: Vec<AudioCodecEntry>,
    /// All meta (abstract) codec entries.
    pub meta: Vec<MetaCodecEntry>,
    /// All video codec entries.
    pub video: Vec<VideoCodecEntry>,
    /// All lyrics/subtitle format entries.
    pub lyrics: Vec<LyricsFormatEntry>,
}

// ============================================================
// TOML deserialization types (internal)
// ============================================================

/// Raw TOML structure for the top-level codecs.toml file.
/// Each section (audio, meta, video) is a map of codec ID → codec definition.
#[derive(Deserialize)]
struct RawCodecsToml {
    #[serde(default)]
    audio: HashMap<String, RawAudioCodec>,
    #[serde(default)]
    meta: HashMap<String, RawMetaCodec>,
    #[serde(default)]
    video: HashMap<String, RawVideoCodec>,
    #[serde(default)]
    lyrics: HashMap<String, RawLyricsFormat>,
}

/// Raw TOML structure for an audio codec entry.
#[derive(Deserialize)]
struct RawAudioCodec {
    name: String,
    category: String,
    lossless: bool,
    mimetype: Option<String>,
    /// Filename suffix for companion download disambiguation (e.g., "[Lossless]").
    /// Defaults to `None` for backwards compatibility with older codecs.toml files.
    #[serde(default)]
    suffix: Option<String>,
    #[serde(default)]
    services: HashMap<String, String>,
}

/// Raw TOML structure for a meta codec entry.
#[derive(Deserialize)]
struct RawMetaCodec {
    name: String,
    category: String,
    resolves_to: HashMap<String, String>,
}

/// Raw TOML structure for a video codec entry.
#[derive(Deserialize)]
struct RawVideoCodec {
    name: String,
    category: String,
    mimetype: Option<String>,
    #[serde(default)]
    services: HashMap<String, String>,
}

/// Raw TOML structure for a lyrics/subtitle format entry.
#[derive(Deserialize)]
struct RawLyricsFormat {
    name: String,
    category: String,
    extension: String,
    mimetype: String,
    #[serde(default)]
    word_timing: bool,
    #[serde(default)]
    services: HashMap<String, String>,
}

// ============================================================
// Registry Loading
// ============================================================

/// Load the codec registry from the compiled-in `codecs.toml`.
///
/// Parses the TOML at runtime and converts the raw deserialized structures
/// into the typed `CodecRegistry`. The TOML is embedded at compile time
/// via `include_str!`, so no file I/O happens at runtime.
///
/// # Panics
///
/// Panics if the compiled-in TOML is malformed (should never happen in
/// release builds since CI validates compilation).
#[must_use]
pub fn load_registry() -> CodecRegistry {
    let raw: RawCodecsToml =
        toml::from_str(CODECS_TOML).expect("Failed to parse compiled-in codecs.toml");

    // Convert raw audio codecs, sorted by ID for deterministic ordering
    let mut audio: Vec<AudioCodecEntry> = raw
        .audio
        .into_iter()
        .map(|(id, raw)| AudioCodecEntry {
            id,
            name: raw.name,
            category: raw.category,
            lossless: raw.lossless,
            mimetype: raw.mimetype,
            suffix: raw.suffix,
            services: raw.services,
        })
        .collect();
    audio.sort_by(|a, b| a.id.cmp(&b.id));

    // Convert raw meta codecs
    let mut meta: Vec<MetaCodecEntry> = raw
        .meta
        .into_iter()
        .map(|(id, raw)| MetaCodecEntry {
            id,
            name: raw.name,
            category: raw.category,
            resolves_to: raw.resolves_to,
        })
        .collect();
    meta.sort_by(|a, b| a.id.cmp(&b.id));

    // Convert raw video codecs
    let mut video: Vec<VideoCodecEntry> = raw
        .video
        .into_iter()
        .map(|(id, raw)| VideoCodecEntry {
            id,
            name: raw.name,
            category: raw.category,
            mimetype: raw.mimetype,
            services: raw.services,
        })
        .collect();
    video.sort_by(|a, b| a.id.cmp(&b.id));

    // Convert raw lyrics/subtitle formats
    let mut lyrics: Vec<LyricsFormatEntry> = raw
        .lyrics
        .into_iter()
        .map(|(id, raw)| LyricsFormatEntry {
            id,
            name: raw.name,
            category: raw.category,
            extension: raw.extension,
            mimetype: raw.mimetype,
            word_timing: raw.word_timing,
            services: raw.services,
        })
        .collect();
    lyrics.sort_by(|a, b| a.id.cmp(&b.id));

    CodecRegistry {
        audio,
        meta,
        video,
        lyrics,
    }
}

// ============================================================
// Query Functions
// ============================================================

/// Resolve a MeedyaDL audio codec ID to a service-specific CLI flag string.
///
/// # Arguments
/// * `registry` - The loaded codec registry
/// * `meedya_id` - Canonical MeedyaDL codec ID (e.g., "eac3-atmos")
/// * `service` - Service engine ID (e.g., "gamdl", "votify")
///
/// # Returns
/// The service's CLI flag string, or `None` if the codec doesn't exist or
/// isn't available on that service.
#[must_use]
pub fn resolve_audio(registry: &CodecRegistry, meedya_id: &str, service: &str) -> Option<String> {
    registry
        .audio
        .iter()
        .find(|c| c.id == meedya_id)
        .and_then(|c| c.services.get(service).cloned())
}

/// Resolve a meta codec to a concrete audio codec ID for a given service.
///
/// Meta codecs are abstract quality tiers (e.g., "lossless") that resolve
/// to the best concrete codec per service (e.g., "alac" for gamdl).
///
/// # Returns
/// The concrete audio codec ID (e.g., "alac"), or `None` if the meta codec
/// doesn't exist or has no mapping for that service.
#[must_use]
pub fn resolve_meta(registry: &CodecRegistry, meta_id: &str, service: &str) -> Option<String> {
    registry
        .meta
        .iter()
        .find(|m| m.id == meta_id)
        .and_then(|m| m.resolves_to.get(service).cloned())
}

/// Map a service-specific CLI flag string back to a MeedyaDL audio codec ID.
///
/// Performs a reverse lookup: given a service's CLI flag (e.g., "atmos" from
/// gamdl), finds the canonical MeedyaDL ID (e.g., "eac3-atmos").
///
/// # Returns
/// The MeedyaDL codec ID, or `None` if no audio codec has that service mapping.
#[must_use]
pub fn from_service_codec(
    registry: &CodecRegistry,
    service: &str,
    service_codec: &str,
) -> Option<String> {
    registry
        .audio
        .iter()
        .find(|c| c.services.get(service).is_some_and(|v| v == service_codec))
        .map(|c| c.id.clone())
}

/// Get all audio codecs available for a given service.
///
/// Returns only codecs that have a mapping for the specified service engine.
/// Useful for building service-specific UI dropdowns and fallback chains.
#[must_use]
pub fn codecs_for_service<'a>(
    registry: &'a CodecRegistry,
    service: &str,
) -> Vec<&'a AudioCodecEntry> {
    registry
        .audio
        .iter()
        .filter(|c| c.services.contains_key(service))
        .collect()
}

/// Resolve a video codec ID to a service-specific CLI flag string.
#[must_use]
pub fn resolve_video(registry: &CodecRegistry, video_id: &str, service: &str) -> Option<String> {
    registry
        .video
        .iter()
        .find(|c| c.id == video_id)
        .and_then(|c| c.services.get(service).cloned())
}

/// Look up the filename suffix for a `SongCodec` from the codec registry.
///
/// Bridges the existing `SongCodec` enum to the registry's `suffix` field by
/// converting the codec to its registry ID and looking up the suffix. Returns
/// `Some(suffix)` for codecs with a non-empty suffix, or `None` for codecs
/// that should use clean filenames (empty or missing suffix).
///
/// Uses the cached `CODEC_REGISTRY` singleton for zero-cost repeated lookups.
/// The returned `&str` has `'static` lifetime because it borrows from the
/// lazily-initialized `CODEC_REGISTRY` static, not from the input `codec`.
#[must_use]
pub fn codec_suffix_from_registry(codec: &SongCodec) -> Option<&'static str> {
    let registry_id = song_codec_to_registry_id(codec);
    CODEC_REGISTRY
        .audio
        .iter()
        .find(|c| c.id == registry_id)
        .and_then(|c| c.suffix.as_deref())
        .filter(|s| !s.is_empty())
}

// ============================================================
// Bridge: SongCodec ↔ Registry ID
// ============================================================

/// Convert an existing `SongCodec` enum variant to its canonical registry ID.
///
/// This bridge function allows existing GAMDL code (which uses `SongCodec`)
/// to interoperate with the universal registry. The mapping is based on the
/// GAMDL service flag values defined in `codecs.toml`.
#[must_use]
pub fn song_codec_to_registry_id(codec: &SongCodec) -> &'static str {
    match codec {
        SongCodec::Alac => "alac",
        SongCodec::Atmos => "eac3-atmos",
        SongCodec::Ac3 => "ac3",
        SongCodec::AacBinaural => "aac-binaural",
        SongCodec::Aac => "aac-hq",
        SongCodec::AacLegacy => "aac-mq",
        SongCodec::AacHeLegacy => "aac-he-mq",
        SongCodec::AacHe => "aac-he-hq",
        SongCodec::AacDownmix => "aac-downmix",
        SongCodec::AacHeBinaural => "aac-he-binaural",
        SongCodec::AacHeDownmix => "aac-he-downmix",
    }
}

/// Convert a canonical registry ID back to a `SongCodec` enum variant.
///
/// Returns `None` if the ID doesn't correspond to any GAMDL-supported codec
/// (e.g., "flac", "ogg-vorbis" — these exist in the registry but have no
/// `SongCodec` equivalent since they're for other services).
#[must_use]
pub fn registry_id_to_song_codec(id: &str) -> Option<SongCodec> {
    match id {
        "alac" => Some(SongCodec::Alac),
        "eac3-atmos" => Some(SongCodec::Atmos),
        "ac3" => Some(SongCodec::Ac3),
        "aac-binaural" => Some(SongCodec::AacBinaural),
        "aac-hq" => Some(SongCodec::Aac),
        "aac-mq" => Some(SongCodec::AacLegacy),
        "aac-he-mq" => Some(SongCodec::AacHeLegacy),
        "aac-he-hq" => Some(SongCodec::AacHe),
        "aac-downmix" => Some(SongCodec::AacDownmix),
        "aac-he-binaural" => Some(SongCodec::AacHeBinaural),
        "aac-he-downmix" => Some(SongCodec::AacHeDownmix),
        _ => None,
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // Registry loading
    // ----------------------------------------------------------

    #[test]
    fn load_registry_parses_successfully() {
        let registry = load_registry();
        // Should have at least the 16 audio codecs defined in codecs.toml
        assert!(
            registry.audio.len() >= 16,
            "Expected at least 16 audio codecs, got {}",
            registry.audio.len()
        );
        // Should have at least 3 meta codecs
        assert!(
            registry.meta.len() >= 3,
            "Expected at least 3 meta codecs, got {}",
            registry.meta.len()
        );
        // Should have at least 4 video codecs
        assert!(
            registry.video.len() >= 4,
            "Expected at least 4 video codecs, got {}",
            registry.video.len()
        );
    }

    #[test]
    fn audio_codec_has_expected_fields() {
        let registry = load_registry();
        let atmos = registry
            .audio
            .iter()
            .find(|c| c.id == "eac3-atmos")
            .unwrap();
        assert_eq!(atmos.name, "Dolby Atmos (E-AC-3)");
        assert_eq!(atmos.category, "spatial");
        assert!(!atmos.lossless);
        assert_eq!(atmos.services.get("gamdl"), Some(&"atmos".to_string()));
    }

    // ----------------------------------------------------------
    // Audio resolution
    // ----------------------------------------------------------

    #[test]
    fn resolve_audio_gamdl_atmos() {
        let registry = load_registry();
        assert_eq!(
            resolve_audio(&registry, "eac3-atmos", "gamdl"),
            Some("atmos".to_string())
        );
    }

    #[test]
    fn resolve_audio_gamdl_alac() {
        let registry = load_registry();
        assert_eq!(
            resolve_audio(&registry, "alac", "gamdl"),
            Some("alac".to_string())
        );
    }

    #[test]
    fn resolve_audio_votify_aac_hq() {
        let registry = load_registry();
        assert_eq!(
            resolve_audio(&registry, "aac-hq", "votify"),
            Some("aac-high".to_string())
        );
    }

    #[test]
    fn resolve_audio_unmapped_codec_returns_none() {
        let registry = load_registry();
        // ac4-atmos has no service mappings
        assert_eq!(resolve_audio(&registry, "ac4-atmos", "gamdl"), None);
    }

    #[test]
    fn resolve_audio_nonexistent_codec_returns_none() {
        let registry = load_registry();
        assert_eq!(resolve_audio(&registry, "does-not-exist", "gamdl"), None);
    }

    // ----------------------------------------------------------
    // Meta codec resolution
    // ----------------------------------------------------------

    #[test]
    fn resolve_meta_lossless_gamdl() {
        let registry = load_registry();
        assert_eq!(
            resolve_meta(&registry, "lossless", "gamdl"),
            Some("alac".to_string())
        );
    }

    #[test]
    fn resolve_meta_lossless_votify() {
        let registry = load_registry();
        assert_eq!(
            resolve_meta(&registry, "lossless", "votify"),
            Some("flac".to_string())
        );
    }

    #[test]
    fn resolve_meta_atmos_gamdl() {
        let registry = load_registry();
        assert_eq!(
            resolve_meta(&registry, "atmos", "gamdl"),
            Some("eac3-atmos".to_string())
        );
    }

    #[test]
    fn resolve_meta_best_lossy_votify() {
        let registry = load_registry();
        assert_eq!(
            resolve_meta(&registry, "best-lossy", "votify"),
            Some("ogg-vorbis".to_string())
        );
    }

    // ----------------------------------------------------------
    // Reverse lookup
    // ----------------------------------------------------------

    #[test]
    fn from_service_codec_gamdl_atmos() {
        let registry = load_registry();
        assert_eq!(
            from_service_codec(&registry, "gamdl", "atmos"),
            Some("eac3-atmos".to_string())
        );
    }

    #[test]
    fn from_service_codec_gamdl_aac() {
        let registry = load_registry();
        assert_eq!(
            from_service_codec(&registry, "gamdl", "aac"),
            Some("aac-hq".to_string())
        );
    }

    #[test]
    fn from_service_codec_votify_flac() {
        let registry = load_registry();
        assert_eq!(
            from_service_codec(&registry, "votify", "flac"),
            Some("flac".to_string())
        );
    }

    #[test]
    fn from_service_codec_unknown_returns_none() {
        let registry = load_registry();
        assert_eq!(from_service_codec(&registry, "gamdl", "mp3"), None);
    }

    // ----------------------------------------------------------
    // Service codec listing
    // ----------------------------------------------------------

    #[test]
    fn codecs_for_gamdl_returns_all_mapped() {
        let registry = load_registry();
        let gamdl_codecs = codecs_for_service(&registry, "gamdl");
        // Should include all 11 GAMDL-mapped audio codecs
        assert!(
            gamdl_codecs.len() >= 11,
            "Expected at least 11 GAMDL codecs, got {}",
            gamdl_codecs.len()
        );
        // Verify specific codecs are present
        assert!(gamdl_codecs.iter().any(|c| c.id == "alac"));
        assert!(gamdl_codecs.iter().any(|c| c.id == "eac3-atmos"));
        assert!(gamdl_codecs.iter().any(|c| c.id == "aac-hq"));
    }

    #[test]
    fn codecs_for_votify_returns_only_votify_mapped() {
        let registry = load_registry();
        let votify_codecs = codecs_for_service(&registry, "votify");
        // Votify has: flac, aac-hq, aac-mq, ogg-vorbis
        assert!(
            votify_codecs.len() >= 4,
            "Expected at least 4 Votify codecs, got {}",
            votify_codecs.len()
        );
        // ALAC should NOT be in votify codecs (it's Apple-only)
        assert!(!votify_codecs.iter().any(|c| c.id == "alac"));
    }

    // ----------------------------------------------------------
    // Video resolution
    // ----------------------------------------------------------

    #[test]
    fn resolve_video_h265_gamdl() {
        let registry = load_registry();
        assert_eq!(
            resolve_video(&registry, "h265", "gamdl"),
            Some("h265".to_string())
        );
    }

    #[test]
    fn resolve_video_h264_votify() {
        let registry = load_registry();
        assert_eq!(
            resolve_video(&registry, "h264", "votify"),
            Some("mp4".to_string())
        );
    }

    // ----------------------------------------------------------
    // SongCodec bridge
    // ----------------------------------------------------------

    #[test]
    fn song_codec_to_registry_roundtrip() {
        // Every SongCodec variant should roundtrip through the bridge
        let codecs = [
            SongCodec::Alac,
            SongCodec::Atmos,
            SongCodec::Ac3,
            SongCodec::AacBinaural,
            SongCodec::Aac,
            SongCodec::AacLegacy,
            SongCodec::AacHeLegacy,
            SongCodec::AacHe,
            SongCodec::AacDownmix,
            SongCodec::AacHeBinaural,
            SongCodec::AacHeDownmix,
        ];

        for codec in &codecs {
            let id = song_codec_to_registry_id(codec);
            let roundtrip = registry_id_to_song_codec(id);
            assert_eq!(
                roundtrip.as_ref(),
                Some(codec),
                "Roundtrip failed for {:?} → {} → {:?}",
                codec,
                id,
                roundtrip
            );
        }
    }

    #[test]
    fn registry_id_to_song_codec_non_gamdl_returns_none() {
        // Codecs from other services should return None
        assert_eq!(registry_id_to_song_codec("flac"), None);
        assert_eq!(registry_id_to_song_codec("ogg-vorbis"), None);
        assert_eq!(registry_id_to_song_codec("opus"), None);
    }

    #[test]
    fn song_codec_bridge_matches_gamdl_service_mapping() {
        // Verify the bridge produces IDs that resolve to the correct GAMDL flags
        let registry = load_registry();

        let codec = SongCodec::Atmos;
        let registry_id = song_codec_to_registry_id(&codec);
        let gamdl_flag = resolve_audio(&registry, registry_id, "gamdl");
        assert_eq!(gamdl_flag, Some("atmos".to_string()));

        let codec = SongCodec::AacLegacy;
        let registry_id = song_codec_to_registry_id(&codec);
        let gamdl_flag = resolve_audio(&registry, registry_id, "gamdl");
        assert_eq!(gamdl_flag, Some("aac-legacy".to_string()));
    }

    // ----------------------------------------------------------
    // Codec suffix from registry
    // ----------------------------------------------------------

    #[test]
    fn codec_suffix_alac_is_lossless() {
        assert_eq!(codec_suffix_from_registry(&SongCodec::Alac), Some("[Lossless]"));
    }

    #[test]
    fn codec_suffix_atmos_is_dolby_atmos() {
        assert_eq!(
            codec_suffix_from_registry(&SongCodec::Atmos),
            Some("[Dolby Atmos]")
        );
    }

    #[test]
    fn codec_suffix_ac3_is_dolby_digital() {
        assert_eq!(
            codec_suffix_from_registry(&SongCodec::Ac3),
            Some("[Dolby Digital]")
        );
    }

    #[test]
    fn codec_suffix_aac_hq_is_empty_none() {
        // AAC 256 (aac-hq) has suffix = "" in codecs.toml, which means
        // it gets a clean filename — codec_suffix_from_registry returns None.
        assert_eq!(codec_suffix_from_registry(&SongCodec::Aac), None);
    }

    #[test]
    fn codec_suffix_aac_binaural() {
        assert_eq!(
            codec_suffix_from_registry(&SongCodec::AacBinaural),
            Some("[Binaural]")
        );
    }

    #[test]
    fn codec_suffix_aac_downmix() {
        assert_eq!(
            codec_suffix_from_registry(&SongCodec::AacDownmix),
            Some("[Downmix]")
        );
    }

    #[test]
    fn codec_suffix_aac_legacy() {
        assert_eq!(
            codec_suffix_from_registry(&SongCodec::AacLegacy),
            Some("[AAC Legacy]")
        );
    }

    #[test]
    fn codec_suffix_aac_he() {
        assert_eq!(
            codec_suffix_from_registry(&SongCodec::AacHe),
            Some("[HE-AAC]")
        );
    }

    #[test]
    fn codec_suffix_aac_he_binaural() {
        assert_eq!(
            codec_suffix_from_registry(&SongCodec::AacHeBinaural),
            Some("[HE Binaural]")
        );
    }

    #[test]
    fn codec_suffix_aac_he_downmix() {
        assert_eq!(
            codec_suffix_from_registry(&SongCodec::AacHeDownmix),
            Some("[HE Downmix]")
        );
    }

    #[test]
    fn codec_suffix_aac_he_legacy() {
        assert_eq!(
            codec_suffix_from_registry(&SongCodec::AacHeLegacy),
            Some("[HE-AAC Legacy]")
        );
    }

    #[test]
    fn audio_codec_suffix_field_loaded() {
        let registry = load_registry();
        // ALAC should have a suffix defined
        let alac = registry.audio.iter().find(|c| c.id == "alac").unwrap();
        assert_eq!(alac.suffix.as_deref(), Some("[Lossless]"));
        // AAC HQ should have an empty suffix (clean filename)
        let aac = registry.audio.iter().find(|c| c.id == "aac-hq").unwrap();
        assert_eq!(aac.suffix.as_deref(), Some(""));
    }
}
