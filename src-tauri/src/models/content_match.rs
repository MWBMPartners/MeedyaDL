// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Content matching models for cross-platform Smart Download.
// ===========================================================
//
// These types support the Smart Download feature, which searches multiple
// enabled media services for the same content and downloads the best
// available quality. Content matching uses industry-standard identifiers
// (ISRC for tracks, UPC/EAN for albums) with fuzzy title/artist matching
// as a fallback.
//
// ## Architecture
//
// ```
// User pastes URL
//   |
//   +-- URL parser detects service + content type
//   +-- smart_download service extracts ContentIdentifiers
//   +-- Searches other enabled services by ISRC/UPC or title+artist
//   +-- Returns SmartDownloadResult with ranked CrossPlatformMatch entries
//   +-- Frontend shows matches ranked by QualityTier
//   +-- User picks "Download best" or original
// ```
//
// ## Status
//
// Phase 1: Only Apple Music metadata extraction is implemented. As each
// service gains full implementation, cross-platform search functions will
// be added to `services/smart_download.rs`.

use serde::{Deserialize, Serialize};

use super::media_service::MediaServiceId;

/// Identifiers for cross-platform content matching.
///
/// Extracted from the source service's metadata. ISRC (International Standard
/// Recording Code) uniquely identifies a specific recording across all
/// platforms. UPC/EAN uniquely identifies an album/product. When these
/// identifiers are available, matching is exact; otherwise the system
/// falls back to fuzzy title+artist matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentIdentifiers {
    /// ISRC for individual tracks (e.g., "USRC11700001").
    /// Most reliable cross-platform identifier for audio recordings.
    pub isrc: Option<String>,

    /// UPC/EAN barcode for albums/products (e.g., "00602557000000").
    /// Used for matching entire albums across services.
    pub upc: Option<String>,

    /// Track or album title (always available from metadata).
    pub title: String,

    /// Primary artist name (always available from metadata).
    pub artist: String,

    /// Album name, if applicable (None for standalone singles).
    pub album: Option<String>,

    /// Track duration in milliseconds. Used as a validation heuristic
    /// for fuzzy matches — if durations differ by more than 5 seconds,
    /// the match confidence is reduced.
    pub duration_ms: Option<u64>,
}

/// Normalised quality tier for cross-platform comparison.
///
/// Provides a unified ranking system for comparing audio quality across
/// services that use different codec names and bitrate conventions.
/// Ordered from lowest to highest quality (the enum's derived `Ord`
/// implementation follows variant declaration order).
///
/// ## Tier mapping by service
///
/// | Tier            | Apple Music        | Spotify            | YouTube       |
/// |-----------------|--------------------|--------------------|---------------|
/// | Lossy128        | —                  | Ogg Vorbis 128     | —             |
/// | Lossy256        | AAC 256            | Ogg Vorbis 320     | best audio    |
/// | Lossless        | ALAC 16-bit        | —                  | —             |
/// | HiResLossless   | ALAC 24-bit        | —                  | —             |
/// | SurroundSound   | AC3 (Dolby Digital)| —                  | —             |
/// | DolbyAtmos      | Dolby Atmos        | —                  | —             |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum QualityTier {
    /// ~128kbps lossy (Ogg Vorbis, AAC-HE).
    Lossy128,
    /// ~256-320kbps lossy (AAC, Ogg Vorbis High).
    Lossy256,
    /// CD-quality lossless (16-bit ALAC/FLAC, ~1411kbps).
    Lossless,
    /// Hi-Res lossless (24-bit ALAC/FLAC, up to 192kHz).
    HiResLossless,
    /// 5.1 surround sound (AC3 / Dolby Digital).
    SurroundSound,
    /// Dolby Atmos / spatial audio (object-based).
    DolbyAtmos,
}

impl QualityTier {
    /// Returns a short human-readable label for the quality tier,
    /// suitable for display in the Smart Download panel.
    pub fn display_name(&self) -> &'static str {
        match self {
            QualityTier::Lossy128 => "Lossy (128kbps)",
            QualityTier::Lossy256 => "Lossy (256kbps)",
            QualityTier::Lossless => "Lossless",
            QualityTier::HiResLossless => "Hi-Res Lossless",
            QualityTier::SurroundSound => "5.1 Surround",
            QualityTier::DolbyAtmos => "Dolby Atmos",
        }
    }
}

/// Result of searching a single service for a cross-platform match.
///
/// Represents one candidate download from a specific service, including
/// the URL to use, the best quality available, and how confident the
/// match is (exact ISRC vs fuzzy title search).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossPlatformMatch {
    /// Which service this match was found on.
    pub service_id: MediaServiceId,

    /// Direct URL for downloading this content from the matched service.
    pub url: String,

    /// The highest quality tier available on this service for the content.
    pub best_quality: QualityTier,

    /// Match confidence between 0.0 and 1.0:
    /// - 1.0 = exact ISRC/UPC match
    /// - 0.8-0.99 = fuzzy title+artist match with duration validation
    /// - < 0.8 = fuzzy match without duration validation
    pub match_confidence: f64,

    /// How the match was made: "isrc", "upc", or "fuzzy".
    pub match_method: String,
}

/// Complete result of a Smart Download cross-platform search.
///
/// Returned to the frontend for display in the Smart Download panel.
/// Contains the original request context and all matches found across
/// enabled services, with a recommendation for the best quality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartDownloadResult {
    /// The service the user's original URL belongs to.
    pub original_service: MediaServiceId,

    /// The user's original URL.
    pub original_url: String,

    /// All matches found across enabled services, ranked by quality tier.
    pub matches: Vec<CrossPlatformMatch>,

    /// The recommended match — the highest quality across all platforms.
    /// `None` if no cross-platform matches were found.
    pub recommended: Option<CrossPlatformMatch>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_tier_ordering() {
        assert!(QualityTier::Lossy128 < QualityTier::Lossy256);
        assert!(QualityTier::Lossy256 < QualityTier::Lossless);
        assert!(QualityTier::Lossless < QualityTier::HiResLossless);
        assert!(QualityTier::HiResLossless < QualityTier::SurroundSound);
        assert!(QualityTier::SurroundSound < QualityTier::DolbyAtmos);
    }

    #[test]
    fn quality_tier_display_names() {
        assert_eq!(QualityTier::Lossy128.display_name(), "Lossy (128kbps)");
        assert_eq!(QualityTier::DolbyAtmos.display_name(), "Dolby Atmos");
    }

    #[test]
    fn smart_download_result_serde_roundtrip() {
        let result = SmartDownloadResult {
            original_service: MediaServiceId::AppleMusic,
            original_url: "https://music.apple.com/us/album/test/123".to_string(),
            matches: vec![CrossPlatformMatch {
                service_id: MediaServiceId::Spotify,
                url: "https://open.spotify.com/track/abc".to_string(),
                best_quality: QualityTier::Lossy256,
                match_confidence: 1.0,
                match_method: "isrc".to_string(),
            }],
            recommended: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SmartDownloadResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.original_url, result.original_url);
        assert_eq!(deserialized.matches.len(), 1);
        assert_eq!(deserialized.matches[0].best_quality, QualityTier::Lossy256);
        assert_eq!(deserialized.matches[0].match_confidence, 1.0);
    }

    #[test]
    fn content_identifiers_serde_roundtrip() {
        let ids = ContentIdentifiers {
            isrc: Some("USRC11700001".to_string()),
            upc: None,
            title: "Test Song".to_string(),
            artist: "Test Artist".to_string(),
            album: Some("Test Album".to_string()),
            duration_ms: Some(240000),
        };

        let json = serde_json::to_string(&ids).unwrap();
        let deserialized: ContentIdentifiers = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.isrc, Some("USRC11700001".to_string()));
        assert_eq!(deserialized.title, "Test Song");
        assert_eq!(deserialized.duration_ms, Some(240000));
    }
}
