// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// MusicBrainz recording lookup service.
// ======================================
//
// Queries the MusicBrainz database to discover recording metadata,
// relationships, and cross-platform URLs using ISRC codes. This serves
// two purposes:
//
// 1. **Music video companion fallback** — find music videos via
//    MusicBrainz when MusicKit credentials aren't configured.
//    No Apple Developer account needed.
//
// 2. **Cross-platform discovery groundwork** — MusicBrainz links
//    recordings to Apple Music, YouTube, Spotify, Deezer, Tidal etc.
//    This enables future "if unavailable on one platform, try another"
//    functionality when additional service engines are added.
//
// ## API Details
//
// - Endpoint: `https://musicbrainz.org/ws/2/`
// - Rate limit: 1 request/second (enforced via sleep between requests)
// - Authentication: None needed for read-only lookups
// - User-Agent: Required by MusicBrainz ToS (identifies the application)
// - Format: JSON (`fmt=json`)
//
// ## Lookup Chain
//
// ISRC code (from Apple Music API metadata) → MusicBrainz recording →
// relationships → video recording → external URLs (Apple Music, YouTube)
//
// ## Integration
//
// Called from the enrichment pipeline (Step 6b) as a fallback when the
// MusicKit-based music video lookup (Step 6) finds no videos. Also
// stores discovered cross-platform URLs as metadata for future use.

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;

// ============================================================
// Constants
// ============================================================

/// Base URL for the MusicBrainz web service API (v2).
const MB_API_BASE: &str = "https://musicbrainz.org/ws/2";

/// Rate limit delay between MusicBrainz API requests.
/// MusicBrainz requires max 1 request/second for unauthenticated clients.
const RATE_LIMIT_DELAY: Duration = Duration::from_millis(1100);

/// HTTP request timeout for MusicBrainz API calls.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// User-Agent header required by MusicBrainz Terms of Service.
/// Must identify the application and provide a contact URL.
const USER_AGENT: &str = "MeedyaDL/0.6 (https://github.com/MWBMPartners/MeedyaDL)";

// ============================================================
// Public Types
// ============================================================

/// A MusicBrainz recording with its metadata and discovered URLs.
///
/// Represents a single track/song as identified by MusicBrainz,
/// including all external platform URLs found in its relationships.
#[derive(Debug, Clone, Serialize)]
pub struct MusicBrainzRecording {
    /// MusicBrainz recording UUID (e.g., "5aa053a9-5b84-418f-bb3c-d61df67b3880")
    pub recording_id: String,
    /// Track title from MusicBrainz
    pub title: String,
    /// Artist credit string (may differ from Apple Music's artist name)
    pub artist: Option<String>,
    /// External platform URLs found in recording relationships.
    /// Key = platform identifier, value = URL.
    /// Common keys: "apple_music", "youtube", "spotify", "deezer", "tidal"
    pub external_urls: HashMap<String, String>,
    /// Music video URLs discovered for this recording.
    pub video_urls: Vec<MusicVideoUrl>,
}

/// A discovered music video URL from MusicBrainz relationships.
///
/// Represents a video linked to a recording via MusicBrainz's
/// recording-recording "performance" relationships or URL relationships.
#[derive(Debug, Clone, Serialize)]
pub struct MusicVideoUrl {
    /// Platform identifier: "apple_music", "youtube", etc.
    pub platform: String,
    /// Full URL to the music video on the platform
    pub url: String,
    /// Title of the video (from MusicBrainz, may differ from audio track)
    pub title: Option<String>,
}

// ============================================================
// Public API
// ============================================================

/// Look up a MusicBrainz recording by ISRC code.
///
/// Queries the MusicBrainz API for recordings matching the given ISRC,
/// including URL relationships for discovering platform links and
/// music videos.
///
/// # Arguments
/// * `isrc` - International Standard Recording Code (e.g., "USUG12345678")
///
/// # Returns
/// The first matching recording with its relationships, or `None` if
/// no recording matches the ISRC.
pub async fn lookup_recording_by_isrc(
    isrc: &str,
) -> Result<Option<MusicBrainzRecording>, String> {
    if isrc.is_empty() {
        return Ok(None);
    }

    // Build the MusicBrainz API query URL.
    // The `isrc` query searches for recordings with a specific ISRC code.
    // `inc=url-rels` includes URL relationships (streaming links, video links).
    let url = format!(
        "{MB_API_BASE}/recording?query=isrc:{isrc}&fmt=json&limit=1"
    );

    log::debug!("MusicBrainz: looking up ISRC {isrc}");

    // Create HTTP client with timeout and required User-Agent header
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    // Make the API request
    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz API request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(format!("MusicBrainz API returned HTTP {status} for ISRC {isrc}"));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    // Parse the first recording from the search results
    let recording = json
        .get("recordings")
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first());

    let Some(rec) = recording else {
        log::debug!("MusicBrainz: no recording found for ISRC {isrc}");
        return Ok(None);
    };

    // Extract basic recording metadata
    let recording_id = rec
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let title = rec
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let artist = rec
        .get("artist-credit")
        .and_then(|ac| ac.as_array())
        .and_then(|arr| arr.first())
        .and_then(|a| a.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    log::debug!(
        "MusicBrainz: found recording {} — \"{}\" by {}",
        recording_id,
        title,
        artist.as_deref().unwrap_or("unknown")
    );

    // Now fetch the full recording with URL relationships
    // The search endpoint doesn't include relationships, so we need a
    // separate lookup by recording ID with inc=url-rels
    let detail_url = format!(
        "{MB_API_BASE}/recording/{recording_id}?inc=url-rels+recording-rels&fmt=json"
    );

    // Rate limit: wait before the detail request
    tokio::time::sleep(RATE_LIMIT_DELAY).await;

    let detail_response = client
        .get(&detail_url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz detail request failed: {e}"))?;

    if !detail_response.status().is_success() {
        // Return basic recording without relationships
        return Ok(Some(MusicBrainzRecording {
            recording_id,
            title,
            artist,
            external_urls: HashMap::new(),
            video_urls: Vec::new(),
        }));
    }

    let detail_json: serde_json::Value = detail_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz detail response: {e}"))?;

    // Extract URL relationships (streaming links, video links)
    let mut external_urls = HashMap::new();
    let mut video_urls = Vec::new();

    // Parse URL relationships (type: "url")
    if let Some(relations) = detail_json.get("relations").and_then(|r| r.as_array()) {
        for rel in relations {
            let rel_type = rel
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let target_type = rel
                .get("target-type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // URL relationships — streaming/download links to external platforms
            if target_type == "url" {
                if let Some(url_resource) = rel.get("url") {
                    let resource_url = url_resource
                        .get("resource")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    // Classify the URL by platform
                    if let Some((platform, clean_url)) = classify_url(resource_url) {
                        external_urls.insert(platform.to_string(), clean_url.to_string());

                        // Check if this is a music video URL
                        if resource_url.contains("music-video")
                            || resource_url.contains("/video/")
                        {
                            video_urls.push(MusicVideoUrl {
                                platform: platform.to_string(),
                                url: clean_url.to_string(),
                                title: Some(title.clone()),
                            });
                        }
                    }
                }
            }

            // Recording-recording relationships — linked performances (e.g., video versions)
            if target_type == "recording" && rel_type == "performance" {
                // This is a linked video recording
                if let Some(target_rec) = rel.get("recording") {
                    let video_title = target_rec
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let video_id = target_rec
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if !video_id.is_empty() {
                        log::debug!(
                            "MusicBrainz: found linked video recording {} — {:?}",
                            video_id,
                            video_title
                        );
                        // We'd need another API call to get this recording's URLs
                        // For now, store the recording ID for potential future lookup
                    }
                }
            }
        }
    }

    log::debug!(
        "MusicBrainz: recording {} has {} external URL(s), {} video URL(s)",
        recording_id,
        external_urls.len(),
        video_urls.len()
    );

    Ok(Some(MusicBrainzRecording {
        recording_id,
        title,
        artist,
        external_urls,
        video_urls,
    }))
}

/// Look up music video URLs for a batch of tracks via ISRC codes.
///
/// Processes each track's ISRC through the MusicBrainz API with
/// rate limiting (1 req/sec). Returns all discovered music video URLs
/// across all tracks.
///
/// # Arguments
/// * `tracks` - List of (song_id, optional ISRC) pairs
///
/// # Returns
/// All discovered music video URLs, deduplicated by URL.
pub async fn lookup_videos_for_tracks(
    tracks: &[(String, Option<String>)],
) -> Result<Vec<MusicVideoUrl>, String> {
    let mut all_videos = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for (song_id, isrc) in tracks {
        // Skip tracks without ISRC codes
        let Some(isrc) = isrc else {
            log::debug!("MusicBrainz: skipping song {song_id} (no ISRC)");
            continue;
        };

        // Rate limit between requests
        if !all_videos.is_empty() {
            tokio::time::sleep(RATE_LIMIT_DELAY).await;
        }

        // Look up the recording by ISRC
        match lookup_recording_by_isrc(isrc).await {
            Ok(Some(recording)) => {
                // Collect video URLs, deduplicating by URL
                for video in &recording.video_urls {
                    if seen_urls.insert(video.url.clone()) {
                        all_videos.push(video.clone());
                    }
                }
            }
            Ok(None) => {
                log::debug!("MusicBrainz: no recording found for song {song_id} (ISRC: {isrc})");
            }
            Err(e) => {
                log::debug!("MusicBrainz: lookup failed for song {song_id}: {e}");
            }
        }
    }

    Ok(all_videos)
}

/// Get all discovered external URLs for a batch of tracks.
///
/// Similar to `lookup_videos_for_tracks` but returns all platform URLs
/// (not just video URLs). Useful for cross-platform discovery.
///
/// # Returns
/// A map of song_id → HashMap<platform, url> for all tracks that had
/// MusicBrainz matches.
pub async fn lookup_external_urls_for_tracks(
    tracks: &[(String, Option<String>)],
) -> Result<HashMap<String, HashMap<String, String>>, String> {
    let mut results = HashMap::new();

    for (song_id, isrc) in tracks {
        let Some(isrc) = isrc else {
            continue;
        };

        // Rate limit between requests
        if !results.is_empty() {
            tokio::time::sleep(RATE_LIMIT_DELAY).await;
        }

        match lookup_recording_by_isrc(isrc).await {
            Ok(Some(recording)) if !recording.external_urls.is_empty() => {
                results.insert(song_id.clone(), recording.external_urls);
            }
            _ => {}
        }
    }

    Ok(results)
}

// ============================================================
// Internal: URL Classification
// ============================================================

/// Classify a URL by its platform based on the domain.
///
/// Returns `Some((platform_id, url))` for recognized platforms,
/// or `None` for unrecognized URLs.
fn classify_url(url: &str) -> Option<(&str, &str)> {
    if url.contains("music.apple.com") || url.contains("itunes.apple.com") {
        Some(("apple_music", url))
    } else if url.contains("youtube.com") || url.contains("youtu.be") {
        Some(("youtube", url))
    } else if url.contains("spotify.com") || url.contains("open.spotify.com") {
        Some(("spotify", url))
    } else if url.contains("deezer.com") {
        Some(("deezer", url))
    } else if url.contains("tidal.com") {
        Some(("tidal", url))
    } else if url.contains("soundcloud.com") {
        Some(("soundcloud", url))
    } else if url.contains("bandcamp.com") {
        Some(("bandcamp", url))
    } else {
        None
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // URL classification
    // ----------------------------------------------------------

    #[test]
    fn classify_apple_music_url() {
        let result = classify_url("https://music.apple.com/gb/music-video/291812351");
        assert_eq!(result, Some(("apple_music", "https://music.apple.com/gb/music-video/291812351")));
    }

    #[test]
    fn classify_youtube_url() {
        let result = classify_url("https://www.youtube.com/watch?v=Eo-KmOd3i7s");
        assert_eq!(result, Some(("youtube", "https://www.youtube.com/watch?v=Eo-KmOd3i7s")));
    }

    #[test]
    fn classify_youtube_short_url() {
        let result = classify_url("https://youtu.be/Eo-KmOd3i7s");
        assert_eq!(result, Some(("youtube", "https://youtu.be/Eo-KmOd3i7s")));
    }

    #[test]
    fn classify_spotify_url() {
        let result = classify_url("https://open.spotify.com/track/abc123");
        assert_eq!(result, Some(("spotify", "https://open.spotify.com/track/abc123")));
    }

    #[test]
    fn classify_deezer_url() {
        let result = classify_url("https://www.deezer.com/track/12345");
        assert_eq!(result, Some(("deezer", "https://www.deezer.com/track/12345")));
    }

    #[test]
    fn classify_tidal_url() {
        let result = classify_url("https://tidal.com/browse/track/12345");
        assert_eq!(result, Some(("tidal", "https://tidal.com/browse/track/12345")));
    }

    #[test]
    fn classify_unknown_url_returns_none() {
        let result = classify_url("https://example.com/some/path");
        assert_eq!(result, None);
    }

    // ----------------------------------------------------------
    // MusicVideoUrl struct
    // ----------------------------------------------------------

    #[test]
    fn music_video_url_serializes() {
        let mv = MusicVideoUrl {
            platform: "apple_music".to_string(),
            url: "https://music.apple.com/gb/music-video/291812351".to_string(),
            title: Some("Test Video".to_string()),
        };
        let json = serde_json::to_string(&mv).unwrap();
        assert!(json.contains("apple_music"));
        assert!(json.contains("291812351"));
    }

    // ----------------------------------------------------------
    // MusicBrainzRecording struct
    // ----------------------------------------------------------

    #[test]
    fn recording_with_empty_urls() {
        let rec = MusicBrainzRecording {
            recording_id: "test-id".to_string(),
            title: "Test Song".to_string(),
            artist: Some("Test Artist".to_string()),
            external_urls: HashMap::new(),
            video_urls: Vec::new(),
        };
        assert!(rec.external_urls.is_empty());
        assert!(rec.video_urls.is_empty());
    }

    #[test]
    fn recording_with_multiple_platforms() {
        let mut urls = HashMap::new();
        urls.insert("apple_music".to_string(), "https://music.apple.com/...".to_string());
        urls.insert("youtube".to_string(), "https://youtube.com/...".to_string());
        urls.insert("spotify".to_string(), "https://open.spotify.com/...".to_string());

        let rec = MusicBrainzRecording {
            recording_id: "test-id".to_string(),
            title: "Test Song".to_string(),
            artist: None,
            external_urls: urls,
            video_urls: Vec::new(),
        };
        assert_eq!(rec.external_urls.len(), 3);
        assert!(rec.external_urls.contains_key("apple_music"));
        assert!(rec.external_urls.contains_key("youtube"));
        assert!(rec.external_urls.contains_key("spotify"));
    }
}
