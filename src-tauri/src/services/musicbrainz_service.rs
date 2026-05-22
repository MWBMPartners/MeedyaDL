// Copyright (c) 2026 MeedyaSuite
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
pub async fn lookup_recording_by_isrc(isrc: &str) -> Result<Option<MusicBrainzRecording>, String> {
    if isrc.is_empty() {
        return Ok(None);
    }

    // Build the MusicBrainz API query URL.
    // The `isrc` query searches for recordings with a specific ISRC code.
    // `inc=url-rels` includes URL relationships (streaming links, video links).
    let url = format!("{MB_API_BASE}/recording?query=isrc:{isrc}&fmt=json&limit=1");

    log::debug!("MusicBrainz: looking up ISRC {isrc}");

    // Create HTTP client with timeout and required User-Agent header
    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    // Make the API request
    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz API request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(format!(
            "MusicBrainz API returned HTTP {status} for ISRC {isrc}"
        ));
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
    let detail_url =
        format!("{MB_API_BASE}/recording/{recording_id}?inc=url-rels+recording-rels&fmt=json");

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

    // Parse relationships using the shared helper function
    let (external_urls, video_urls) = parse_recording_relations(&detail_json);

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

/// Information for looking up a track on MusicBrainz.
///
/// Carries all available identifiers for a track, used by the 3-tier
/// discovery priority chain: URL → ISRC → AcoustID recording ID.
#[derive(Debug, Clone)]
pub struct TrackLookupInfo {
    /// Internal song/download ID (for logging)
    pub song_id: String,
    /// Apple Music song URL (primary discovery path — search MB external links)
    pub apple_music_url: Option<String>,
    /// ISRC code from Apple Music API metadata (secondary path)
    pub isrc: Option<String>,
    /// MusicBrainz recording ID from AcoustID fingerprinting (tertiary path)
    pub musicbrainz_recording_id: Option<String>,
}

/// Look up music video URLs for a batch of tracks.
///
/// Uses a 3-tier discovery priority for each track:
/// 1. **Apple Music URL** — search MusicBrainz for recordings linked to
///    this URL via external links (most direct match)
/// 2. **ISRC code** — search by International Standard Recording Code
///    (reliable, standard identifier)
/// 3. **MusicBrainz recording ID** — direct lookup by ID if available
///    from AcoustID fingerprinting (skips search entirely)
///
/// Rate-limited to 1 request/second per MusicBrainz ToS.
///
/// # Returns
/// All discovered music video URLs, deduplicated by URL.
pub async fn lookup_videos_for_tracks(
    tracks: &[(String, Option<String>)],
) -> Result<Vec<MusicVideoUrl>, String> {
    // Convert legacy (song_id, isrc) pairs to TrackLookupInfo
    let infos: Vec<TrackLookupInfo> = tracks
        .iter()
        .map(|(song_id, isrc)| TrackLookupInfo {
            song_id: song_id.clone(),
            apple_music_url: None,
            isrc: isrc.clone(),
            musicbrainz_recording_id: None,
        })
        .collect();

    lookup_videos_for_tracks_enhanced(&infos).await
}

/// Enhanced track video lookup with 3-tier discovery priority.
///
/// Priority chain per track:
/// 1. Apple Music URL → MusicBrainz external link search
/// 2. ISRC → MusicBrainz recording search
/// 3. MusicBrainz recording ID → direct lookup (from AcoustID)
pub async fn lookup_videos_for_tracks_enhanced(
    tracks: &[TrackLookupInfo],
) -> Result<Vec<MusicVideoUrl>, String> {
    let mut all_videos = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();
    let mut request_count = 0;

    for track in tracks {
        let mut found = false;

        // Tier 1: Try Apple Music URL lookup on MusicBrainz
        // (search for recordings that have this URL as an external link)
        if let Some(ref am_url) = track.apple_music_url {
            if request_count > 0 {
                tokio::time::sleep(RATE_LIMIT_DELAY).await;
            }
            request_count += 1;

            log::debug!(
                "MusicBrainz: Tier 1 — looking up song {} via Apple Music URL",
                track.song_id
            );

            // Search MusicBrainz for recordings linked to this Apple Music URL
            match lookup_recording_by_url(am_url).await {
                Ok(Some(recording)) => {
                    for video in &recording.video_urls {
                        if seen_urls.insert(video.url.clone()) {
                            all_videos.push(video.clone());
                        }
                    }
                    found = true;
                }
                Ok(None) => {
                    log::debug!("MusicBrainz: Tier 1 — no match for URL {am_url}");
                }
                Err(e) => {
                    log::debug!("MusicBrainz: Tier 1 — URL lookup failed: {e}");
                }
            }
        }

        // Tier 2: Try ISRC lookup (if Tier 1 didn't find anything)
        if !found {
            if let Some(ref isrc) = track.isrc {
                if request_count > 0 {
                    tokio::time::sleep(RATE_LIMIT_DELAY).await;
                }
                request_count += 1;

                log::debug!(
                    "MusicBrainz: Tier 2 — looking up song {} via ISRC {isrc}",
                    track.song_id
                );

                match lookup_recording_by_isrc(isrc).await {
                    Ok(Some(recording)) => {
                        for video in &recording.video_urls {
                            if seen_urls.insert(video.url.clone()) {
                                all_videos.push(video.clone());
                            }
                        }
                        found = true;
                    }
                    Ok(None) => {
                        log::debug!("MusicBrainz: Tier 2 — no match for ISRC {isrc}");
                    }
                    Err(e) => {
                        log::debug!("MusicBrainz: Tier 2 — ISRC lookup failed: {e}");
                    }
                }
            }
        }

        // Tier 3: Try direct MusicBrainz recording ID lookup (from AcoustID)
        if !found {
            if let Some(ref mb_id) = track.musicbrainz_recording_id {
                if request_count > 0 {
                    tokio::time::sleep(RATE_LIMIT_DELAY).await;
                }
                request_count += 1;

                log::debug!(
                    "MusicBrainz: Tier 3 — looking up song {} via recording ID {mb_id}",
                    track.song_id
                );

                match lookup_recording_by_id(mb_id).await {
                    Ok(Some(recording)) => {
                        for video in &recording.video_urls {
                            if seen_urls.insert(video.url.clone()) {
                                all_videos.push(video.clone());
                            }
                        }
                    }
                    Ok(None) => {
                        log::debug!("MusicBrainz: Tier 3 — no match for recording ID {mb_id}");
                    }
                    Err(e) => {
                        log::debug!("MusicBrainz: Tier 3 — ID lookup failed: {e}");
                    }
                }
            }
        }
    }

    Ok(all_videos)
}

/// Look up a MusicBrainz recording by searching for an external URL.
///
/// Searches MusicBrainz for recordings that have the given URL as an
/// external link (e.g., an Apple Music song URL). This is the most
/// direct discovery path — if a MusicBrainz record has this exact URL
/// as an external link, the match is highly reliable.
///
/// # Arguments
/// * `external_url` - The URL to search for (e.g., Apple Music song URL)
///
/// # Returns
/// The matching recording with relationships, or `None` if not found.
pub async fn lookup_recording_by_url(
    external_url: &str,
) -> Result<Option<MusicBrainzRecording>, String> {
    if external_url.is_empty() {
        return Ok(None);
    }

    // #807: try the user's exact URL first, then fall back to the
    // storefront-independent canonical form. MusicBrainz indexes
    // URLs as-stored, so an MB record with `/us/album/123` won't
    // match a Lucene query for `/gb/album/super-slug/123`. The
    // canonical fallback issues a second query with a wildcard
    // glob (`*album/123`) that matches every storefront-and-slug
    // permutation MB might have indexed.
    let exact_hit = try_lookup_recording_by_url_exact(external_url).await?;
    if exact_hit.is_some() {
        return Ok(exact_hit);
    }

    if let Some(canonical) =
        super::apple_music_api::canonicalise_apple_music_url(external_url)
    {
        log::debug!(
            "MusicBrainz: exact-URL lookup missed, falling back to canonical-form glob (#807)"
        );
        // The canonical form is `music.apple.com/{type}/{id}`. MB's
        // Lucene query syntax supports wildcards on URL fields, so
        // we glob the storefront + slug segments by searching for
        // the tail. Search shape: `url:*{type}/{id}` — matches
        // every MB external_url that ends in `{type}/{id}`
        // regardless of storefront / slug.
        // Strip `music.apple.com/` prefix to get just the
        // type+ID portion that we want to anchor.
        if let Some(tail) = canonical.strip_prefix("music.apple.com/") {
            return try_lookup_recording_by_url_glob(tail).await;
        }
    }

    Ok(None)
}

/// Issue the exact-string MB Lucene query for the given URL.
/// Internal helper for `lookup_recording_by_url`; the canonical
/// fallback path lives in `try_lookup_recording_by_url_glob`.
async fn try_lookup_recording_by_url_exact(
    external_url: &str,
) -> Result<Option<MusicBrainzRecording>, String> {
    // URL-encode the search URL for the MusicBrainz query
    let encoded_url = external_url.replace(':', "%3A").replace('/', "%2F");
    let url = format!("{MB_API_BASE}/recording?query=url:%22{encoded_url}%22&fmt=json&limit=1");

    log::debug!("MusicBrainz: searching for recording by exact URL");

    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz URL search failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "MusicBrainz API returned HTTP {}",
            response.status().as_u16()
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    extract_first_recording_from_search(&json).await
}

/// Wildcard-glob fallback (#807) — issues an MB Lucene query that
/// matches the canonical `type/id` tail regardless of storefront
/// or slug. Used when the exact-URL lookup misses because the
/// user's URL carries `/gb/.../super-slug/` and MB's stored URL
/// carries `/us/.../`.
async fn try_lookup_recording_by_url_glob(
    canonical_tail: &str,
) -> Result<Option<MusicBrainzRecording>, String> {
    // Encode the tail (path-separator + colon-safe) for inclusion
    // in MB's Lucene query string. We deliberately do NOT wrap the
    // value in quotes — quoted searches are exact match in Lucene.
    // Wildcards anchor the suffix.
    let encoded_tail = canonical_tail.replace('/', "%2F");
    let url = format!(
        "{MB_API_BASE}/recording?query=url:*{encoded_tail}&fmt=json&limit=1"
    );

    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz URL glob search failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "MusicBrainz API returned HTTP {} on glob fallback",
            response.status().as_u16()
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    extract_first_recording_from_search(&json).await
}

/// Shared post-search step: pull the first recording's ID from
/// the search JSON and fetch the full recording with relationships.
async fn extract_first_recording_from_search(
    json: &serde_json::Value,
) -> Result<Option<MusicBrainzRecording>, String> {
    // Get the first matching recording
    let recording = json
        .get("recordings")
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first());

    let Some(rec) = recording else {
        return Ok(None);
    };

    let recording_id = rec
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if recording_id.is_empty() {
        return Ok(None);
    }

    // Fetch full recording with relationships
    tokio::time::sleep(RATE_LIMIT_DELAY).await;
    lookup_recording_by_id(&recording_id).await
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

/// Look up a MusicBrainz recording directly by its recording ID.
///
/// Used when the recording ID is already known (e.g., from AcoustID
/// fingerprint results). Skips the ISRC search step entirely.
///
/// # Arguments
/// * `recording_id` - MusicBrainz recording UUID
///
/// # Returns
/// Recording with relationships and external URLs.
pub async fn lookup_recording_by_id(
    recording_id: &str,
) -> Result<Option<MusicBrainzRecording>, String> {
    if recording_id.is_empty() {
        return Ok(None);
    }

    log::debug!("MusicBrainz: looking up recording by ID {recording_id}");

    let client = crate::utils::http_client::build_client(
        crate::utils::http_client::ClientConfig::with_timeout(REQUEST_TIMEOUT.as_secs())
            .user_agent(USER_AGENT),
    )?;

    // Fetch recording with URL and recording relationships
    let url = format!(
        "{MB_API_BASE}/recording/{recording_id}?inc=url-rels+recording-rels+artist-credits&fmt=json"
    );

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("MusicBrainz API request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(format!(
            "MusicBrainz API returned HTTP {status} for recording {recording_id}"
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MusicBrainz response: {e}"))?;

    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let artist = json
        .get("artist-credit")
        .and_then(|ac| ac.as_array())
        .and_then(|arr| arr.first())
        .and_then(|a| a.get("name"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Parse relationships (same logic as in lookup_recording_by_isrc)
    let (external_urls, video_urls) = parse_recording_relations(&json);

    Ok(Some(MusicBrainzRecording {
        recording_id: recording_id.to_string(),
        title,
        artist,
        external_urls,
        video_urls,
    }))
}

/// Rewrite an Apple Music URL's storefront code.
///
/// If the URL contains a different storefront than the user's preferred
/// one, returns a new URL with the storefront replaced. Useful when
/// MusicBrainz returns an Apple Music URL for a different region.
///
/// # Examples
/// ```ignore
/// rewrite_apple_music_storefront(
///     "https://music.apple.com/de/album/550152190",
///     "gb"
/// ) // → "https://music.apple.com/gb/album/550152190"
/// ```
#[must_use]
pub fn rewrite_apple_music_storefront(url: &str, target_storefront: &str) -> String {
    // Apple Music URL pattern: https://music.apple.com/{storefront}/{type}/{name}/{id}
    // The storefront is a 2-letter country code after the domain.
    if let Some(domain_end) = url.find("music.apple.com/") {
        let after_domain = &url[domain_end + "music.apple.com/".len()..];
        // Check if the next segment is a 2-3 letter storefront code
        if let Some(slash_pos) = after_domain.find('/') {
            let current_sf = &after_domain[..slash_pos];
            // Only rewrite if it looks like a storefront (2-3 lowercase letters)
            if current_sf.len() >= 2
                && current_sf.len() <= 3
                && current_sf.chars().all(|c| c.is_ascii_lowercase())
                && current_sf != target_storefront
            {
                let prefix = &url[..domain_end + "music.apple.com/".len()];
                let rest = &after_domain[slash_pos..]; // includes the leading /
                return format!("{prefix}{target_storefront}{rest}");
            }
        }
    }
    // Return original if no rewrite needed or URL doesn't match pattern
    url.to_string()
}

// ============================================================
// Internal: Relationship Parsing
// ============================================================

/// Parse URL and recording relationships from a MusicBrainz recording JSON.
///
/// Extracts external platform URLs and music video URLs from the
/// `relations` array. Shared between ISRC lookup and direct ID lookup.
fn parse_recording_relations(
    json: &serde_json::Value,
) -> (HashMap<String, String>, Vec<MusicVideoUrl>) {
    let mut external_urls = HashMap::new();
    let mut video_urls = Vec::new();

    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if let Some(relations) = json.get("relations").and_then(|r| r.as_array()) {
        for rel in relations {
            let rel_type = rel.get("type").and_then(|v| v.as_str()).unwrap_or("");

            let target_type = rel
                .get("target-type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // URL relationships — streaming/download links
            if target_type == "url" {
                if let Some(url_resource) = rel.get("url") {
                    let resource_url = url_resource
                        .get("resource")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if let Some((platform, clean_url)) = classify_url(resource_url) {
                        external_urls.insert(platform.to_string(), clean_url.to_string());

                        if resource_url.contains("music-video") || resource_url.contains("/video/")
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

            // Recording-recording relationships — linked performances
            if target_type == "recording" && rel_type == "performance" {
                if let Some(target_rec) = rel.get("recording") {
                    let video_title = target_rec
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let _video_id = target_rec.get("id").and_then(|v| v.as_str()).unwrap_or("");

                    if let Some(ref vt) = video_title {
                        log::debug!("MusicBrainz: found linked video recording — {vt}");
                    }
                }
            }
        }
    }

    (external_urls, video_urls)
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
        assert_eq!(
            result,
            Some((
                "apple_music",
                "https://music.apple.com/gb/music-video/291812351"
            ))
        );
    }

    #[test]
    fn classify_youtube_url() {
        let result = classify_url("https://www.youtube.com/watch?v=Eo-KmOd3i7s");
        assert_eq!(
            result,
            Some(("youtube", "https://www.youtube.com/watch?v=Eo-KmOd3i7s"))
        );
    }

    #[test]
    fn classify_youtube_short_url() {
        let result = classify_url("https://youtu.be/Eo-KmOd3i7s");
        assert_eq!(result, Some(("youtube", "https://youtu.be/Eo-KmOd3i7s")));
    }

    #[test]
    fn classify_spotify_url() {
        let result = classify_url("https://open.spotify.com/track/abc123");
        assert_eq!(
            result,
            Some(("spotify", "https://open.spotify.com/track/abc123"))
        );
    }

    #[test]
    fn classify_deezer_url() {
        let result = classify_url("https://www.deezer.com/track/12345");
        assert_eq!(
            result,
            Some(("deezer", "https://www.deezer.com/track/12345"))
        );
    }

    #[test]
    fn classify_tidal_url() {
        let result = classify_url("https://tidal.com/browse/track/12345");
        assert_eq!(
            result,
            Some(("tidal", "https://tidal.com/browse/track/12345"))
        );
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
        urls.insert(
            "apple_music".to_string(),
            "https://music.apple.com/...".to_string(),
        );
        urls.insert("youtube".to_string(), "https://youtube.com/...".to_string());
        urls.insert(
            "spotify".to_string(),
            "https://open.spotify.com/...".to_string(),
        );

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

    // ----------------------------------------------------------
    // Storefront rewriting
    // ----------------------------------------------------------

    #[test]
    fn rewrite_storefront_de_to_gb() {
        let result =
            rewrite_apple_music_storefront("https://music.apple.com/de/album/test/550152190", "gb");
        assert_eq!(result, "https://music.apple.com/gb/album/test/550152190");
    }

    #[test]
    fn rewrite_storefront_us_to_gb() {
        let result = rewrite_apple_music_storefront(
            "https://music.apple.com/us/music-video/test/291812351",
            "gb",
        );
        assert_eq!(
            result,
            "https://music.apple.com/gb/music-video/test/291812351"
        );
    }

    #[test]
    fn rewrite_storefront_same_no_change() {
        let url = "https://music.apple.com/gb/album/test/12345";
        let result = rewrite_apple_music_storefront(url, "gb");
        assert_eq!(result, url);
    }

    #[test]
    fn rewrite_storefront_non_apple_url_unchanged() {
        let url = "https://youtube.com/watch?v=abc";
        let result = rewrite_apple_music_storefront(url, "gb");
        assert_eq!(result, url);
    }

    #[test]
    fn rewrite_storefront_no_storefront_segment() {
        // URL without storefront (geo-non-specific)
        let url = "https://music.apple.com/album/test/12345";
        let result = rewrite_apple_music_storefront(url, "gb");
        // "album" is not a 2-3 char lowercase code, so no rewrite
        assert_eq!(result, url);
    }

    // ----------------------------------------------------------
    // Relationship parsing
    // ----------------------------------------------------------

    #[test]
    fn parse_relations_with_url_relationships() {
        let json = serde_json::json!({
            "title": "Test Song",
            "relations": [
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": {
                        "resource": "https://music.apple.com/gb/album/test/12345"
                    }
                },
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": {
                        "resource": "https://www.youtube.com/watch?v=abc123"
                    }
                }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 2);
        assert!(urls.contains_key("apple_music"));
        assert!(urls.contains_key("youtube"));
        assert!(videos.is_empty()); // No music-video URLs in this test
    }

    #[test]
    fn parse_relations_with_video_url() {
        let json = serde_json::json!({
            "title": "Test Video",
            "relations": [
                {
                    "type": "streaming",
                    "target-type": "url",
                    "url": {
                        "resource": "https://music.apple.com/gb/music-video/test/12345"
                    }
                }
            ]
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert_eq!(urls.len(), 1);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].platform, "apple_music");
    }

    #[test]
    fn parse_relations_empty() {
        let json = serde_json::json!({
            "title": "No Relations"
        });

        let (urls, videos) = parse_recording_relations(&json);
        assert!(urls.is_empty());
        assert!(videos.is_empty());
    }
}
