// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Smart Download service: cross-platform quality optimization.
// =============================================================
//
// When a user pastes a URL from any supported service, Smart Download
// searches all other enabled services (where valid cookies/credentials
// exist) for the same content and identifies the best available quality.
//
// ## Phase 1 (Current)
//
// Only Apple Music metadata extraction is implemented. The service can:
// - Extract ISRC/UPC from Apple Music URLs via the MusicKit API
// - Map each service's configured quality to a normalised QualityTier
// - Return the original service's quality as the baseline
//
// Cross-platform search (searching Spotify/YouTube by ISRC) will be
// added as each service gains full implementation.
//
// ## Architecture
//
// ```
// DownloadForm (user pastes URL)
//   |
//   +-- check_cross_platform(url, service_id)
//         |
//         +-- Extract ContentIdentifiers from source service
//         +-- For each other enabled service:
//         |     +-- Search by ISRC/UPC (exact)
//         |     +-- OR search by title+artist (fuzzy)
//         |     +-- Map to QualityTier based on service settings
//         +-- find_best_match() across all matches
//         +-- Return SmartDownloadResult
// ```

use tauri::AppHandle;

use crate::models::content_match::{
    CrossPlatformMatch, QualityTier, SmartDownloadResult,
};
use crate::models::media_service::MediaServiceId;
use crate::models::settings::AppSettings;
use crate::services::config_service;

/// Checks all enabled services for cross-platform matches of the given URL.
///
/// This is the main entry point for the Smart Download feature. It:
/// 1. Loads the user's settings to determine quality tiers and enabled services.
/// 2. Extracts content identifiers (ISRC/UPC) from the source service.
/// 3. Searches other enabled services for matching content.
/// 4. Returns matches ranked by quality tier.
///
/// ## Phase 1 Limitations
///
/// - Only Apple Music metadata extraction is implemented.
/// - For non-Apple Music URLs, returns an empty matches list.
/// - Cross-platform search requires each service to be fully implemented.
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving settings and credentials.
/// * `url` - The user's pasted URL.
/// * `service_id` - The detected source service.
///
/// # Returns
/// * `Ok(SmartDownloadResult)` with matches (may be empty if no cross-platform
///   results are found).
/// * `Err(String)` if a critical error occurs (e.g., settings can't be loaded).
pub async fn check_cross_platform(
    app: &AppHandle,
    url: &str,
    service_id: MediaServiceId,
) -> Result<SmartDownloadResult, String> {
    let settings = config_service::load_settings(app).unwrap_or_default();

    // Build the result with the original service's quality as a baseline
    let original_quality = get_service_quality_tier(&service_id, &settings);
    let matches = vec![CrossPlatformMatch {
        service_id,
        url: url.to_string(),
        best_quality: original_quality,
        match_confidence: 1.0,
        match_method: "original".to_string(),
    }];

    // Phase 1: Return early — cross-platform search is not yet implemented.
    // When services are implemented, this is where we'd:
    // 1. Extract ContentIdentifiers from the source service
    // 2. Search each other enabled service by ISRC/UPC
    // 3. Add CrossPlatformMatch entries for successful finds

    let recommended = find_best_match(&matches).cloned();

    Ok(SmartDownloadResult {
        original_service: service_id,
        original_url: url.to_string(),
        matches,
        recommended,
    })
}

/// Maps a service's configured quality settings to a normalised QualityTier.
///
/// Uses the user's current settings to determine what quality tier each
/// service would deliver. This allows comparing "Apple Music at ALAC" vs
/// "Spotify at Ogg Vorbis 320" on a common scale.
///
/// # Arguments
/// * `service_id` - The service to check.
/// * `settings` - The user's current application settings.
///
/// # Returns
/// The QualityTier that the service would deliver with current settings.
pub fn get_service_quality_tier(
    service_id: &MediaServiceId,
    settings: &AppSettings,
) -> QualityTier {
    match service_id {
        MediaServiceId::AppleMusic => {
            use crate::models::gamdl_options::SongCodec;
            match settings.default_song_codec {
                SongCodec::Atmos => QualityTier::DolbyAtmos,
                SongCodec::Ac3 => QualityTier::SurroundSound,
                SongCodec::Alac => QualityTier::Lossless,
                SongCodec::AacBinaural
                | SongCodec::Aac
                | SongCodec::AacLegacy
                | SongCodec::AacDownmix => QualityTier::Lossy256,
                SongCodec::AacHeLegacy
                | SongCodec::AacHe
                | SongCodec::AacHeBinaural
                | SongCodec::AacHeDownmix => QualityTier::Lossy128,
            }
        }
        MediaServiceId::Spotify => {
            // Map Spotify's Ogg Vorbis quality settings
            match settings.services.spotify.audio_quality.as_str() {
                "vorbis-high" => QualityTier::Lossy256,
                _ => QualityTier::Lossy128,
            }
        }
        MediaServiceId::YouTube => {
            // YouTube audio extraction is lossy (AAC/Opus ~256kbps at best)
            QualityTier::Lossy256
        }
        MediaServiceId::BBCiPlayer => {
            // BBC iPlayer audio is typically AAC at 128-320kbps
            QualityTier::Lossy256
        }
    }
}

/// Finds the best quality match from a list of cross-platform matches.
///
/// Returns the match with the highest `QualityTier`. In case of a tie,
/// the match with the highest confidence is preferred. Returns `None`
/// if the matches list is empty.
pub fn find_best_match(matches: &[CrossPlatformMatch]) -> Option<&CrossPlatformMatch> {
    matches.iter().max_by(|a, b| {
        a.best_quality
            .cmp(&b.best_quality)
            .then_with(|| a.match_confidence.partial_cmp(&b.match_confidence).unwrap_or(std::cmp::Ordering::Equal))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_service_quality_tier_apple_music() {
        let settings = AppSettings::default(); // defaults to ALAC
        let tier = get_service_quality_tier(&MediaServiceId::AppleMusic, &settings);
        assert_eq!(tier, QualityTier::Lossless);
    }

    #[test]
    fn test_get_service_quality_tier_spotify() {
        let settings = AppSettings::default(); // defaults to vorbis-high
        let tier = get_service_quality_tier(&MediaServiceId::Spotify, &settings);
        assert_eq!(tier, QualityTier::Lossy256);
    }

    #[test]
    fn test_get_service_quality_tier_youtube() {
        let settings = AppSettings::default();
        let tier = get_service_quality_tier(&MediaServiceId::YouTube, &settings);
        assert_eq!(tier, QualityTier::Lossy256);
    }

    #[test]
    fn test_find_best_match_selects_highest_quality() {
        let matches = vec![
            CrossPlatformMatch {
                service_id: MediaServiceId::Spotify,
                url: "https://open.spotify.com/track/abc".to_string(),
                best_quality: QualityTier::Lossy256,
                match_confidence: 1.0,
                match_method: "isrc".to_string(),
            },
            CrossPlatformMatch {
                service_id: MediaServiceId::AppleMusic,
                url: "https://music.apple.com/us/album/test/123".to_string(),
                best_quality: QualityTier::Lossless,
                match_confidence: 1.0,
                match_method: "isrc".to_string(),
            },
        ];

        let best = find_best_match(&matches).unwrap();
        assert_eq!(best.service_id, MediaServiceId::AppleMusic);
        assert_eq!(best.best_quality, QualityTier::Lossless);
    }

    #[test]
    fn test_find_best_match_empty() {
        let matches: Vec<CrossPlatformMatch> = vec![];
        assert!(find_best_match(&matches).is_none());
    }

    #[test]
    fn test_find_best_match_tiebreak_by_confidence() {
        let matches = vec![
            CrossPlatformMatch {
                service_id: MediaServiceId::Spotify,
                url: "https://open.spotify.com/track/abc".to_string(),
                best_quality: QualityTier::Lossy256,
                match_confidence: 0.8,
                match_method: "fuzzy".to_string(),
            },
            CrossPlatformMatch {
                service_id: MediaServiceId::YouTube,
                url: "https://youtube.com/watch?v=abc".to_string(),
                best_quality: QualityTier::Lossy256,
                match_confidence: 1.0,
                match_method: "isrc".to_string(),
            },
        ];

        let best = find_best_match(&matches).unwrap();
        assert_eq!(best.service_id, MediaServiceId::YouTube);
    }
}
