// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// apple_music_api.rs -- Shared Apple Music API client and authentication
// ======================================================================
//
// Provides reusable Apple Music (MusicKit) API infrastructure used by
// multiple services:
//
//   - `animated_artwork_service` -- Fetches animated cover art HLS URLs
//   - `metadata_tag_service` -- Fetches track/album metadata (ISRC, UPC, etc.)
//
// ## Extracted Functions
//
// These were originally in `animated_artwork_service.rs` and have been
// extracted here to avoid duplication:
//
//   - `generate_musickit_jwt()` -- ES256-signed JWT for MusicKit API auth
//   - `parse_apple_music_url()` -- Regex URL parser for album/song URLs
//   - `get_private_key_from_keychain()` -- OS keychain access for private key
//
// ## New Functionality
//
//   - `fetch_album_metadata()` -- Enriched API call returning album + track
//     metadata (ISRC, UPC, content rating, genre, artist IDs, etc.) plus
//     animated artwork URLs, all in a single request.
//
// ## Authentication
//
// The Apple Music catalog API requires a MusicKit Developer Token (JWT)
// signed with an ES256 private key. Credentials:
//   - Team ID + Key ID: stored in AppSettings (non-sensitive)
//   - Private key (.p8 PEM): stored in OS keychain under "musickit_private_key"
//
// @see animated_artwork_service.rs -- Consumes artwork URLs from AlbumMetadata
// @see metadata_tag_service.rs -- Consumes track metadata from AlbumMetadata
// @see https://developer.apple.com/documentation/applemusicapi/

use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::{Deserialize, Serialize};

// ============================================================
// Public Types
// ============================================================

/// A parsed Apple Music URL, containing the storefront, content type,
/// and numeric IDs needed for API queries.
///
/// Supports album, song, music-video, playlist, and artist URLs.
/// For album URLs with `?i=` query parameter, both the album ID and
/// the individual song ID are extracted.
#[derive(Debug, Clone)]
pub struct ParsedAppleMusicUrl {
    /// Two-letter country code (e.g., "us", "gb", "jp")
    pub storefront: String,
    /// Content type from the URL path (e.g., "album", "song", "music-video")
    pub content_type: String,
    /// Numeric album identifier (e.g., "1649434004")
    pub album_id: String,
    /// Optional song ID from `?i=` query parameter (single-track URLs)
    pub song_id: Option<String>,
}

/// Complete metadata for an Apple Music album and its tracks.
///
/// Returned by `fetch_album_metadata()`, this struct contains all fields
/// needed by both the animated artwork service (artwork URLs) and the
/// metadata tag service (ISRC, UPC, genre, advisory, artist IDs, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumMetadata {
    /// Apple Music album ID
    pub album_id: String,
    /// UPC/EAN barcode (GTIN), if available
    pub upc: Option<String>,
    /// Album-level content rating ("explicit", "clean", or None)
    pub content_rating: Option<String>,
    /// Genre names for the album (first entry is primary genre)
    pub genre_names: Vec<String>,
    /// Apple Music artist ID for the album artist
    pub artist_id: Option<String>,
    /// Album artist display name
    pub artist_name: Option<String>,
    /// Per-track metadata for all tracks in the album
    pub tracks: Vec<TrackMetadata>,
    /// HLS M3U8 URL for square (1:1) animated artwork, if available
    pub artwork_square_url: Option<String>,
    /// HLS M3U8 URL for portrait (3:4) animated artwork, if available
    pub artwork_tall_url: Option<String>,
}

/// Metadata for a single track within an album.
///
/// Mapped from the Apple Music API `songs` resource type within the
/// album's `relationships.tracks` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackMetadata {
    /// Apple Music song ID (numeric string, matches `cnID` in M4A metadata)
    pub song_id: String,
    /// ISRC (International Standard Recording Code) for the track
    pub isrc: Option<String>,
    /// Track-level content rating ("explicit", "clean", or None)
    pub content_rating: Option<String>,
    /// Apple Music artist ID for the track's primary artist
    pub artist_id: Option<String>,
    /// Track artist display name
    pub artist_name: Option<String>,
    /// Track title
    pub name: String,
    /// Track number within the disc (1-based)
    pub track_number: u32,
    /// Disc number (1-based)
    pub disc_number: u32,
}

// ============================================================
// URL Parsing
// ============================================================

/// Parse an Apple Music URL to extract the storefront, content type, and IDs.
///
/// Supports these URL patterns (both `music.apple.com` and `classical.apple.com`):
/// - `https://music.apple.com/us/album/album-name/1234567890`
/// - `https://classical.apple.com/us/album/beethoven-symphony/1234567890`
/// - `https://music.apple.com/us/album/album-name/1234567890?i=9876543210`
/// - `https://music.apple.com/us/song/song-name/9876543210`
/// - `https://music.apple.com/us/music-video/video-name/1234567890`
///
/// Apple Music Classical URLs (`classical.apple.com`) share the same path
/// structure as standard Apple Music URLs and are treated identically.
///
/// For album URLs with `?i=` query parameter, both the album ID and the
/// individual song ID are extracted.
///
/// # Panics
///
/// Panics if the hardcoded regex patterns are invalid (should never happen).
///
/// # Returns
/// * `Some(ParsedAppleMusicUrl)` - URL matched an Apple Music pattern
/// * `None` - URL doesn't match any supported Apple Music pattern
#[must_use]
pub fn parse_apple_music_url(url: &str) -> Option<ParsedAppleMusicUrl> {
    // Match album URLs: /storefront/album/slug/album_id with optional ?i=song_id
    // Accepts both music.apple.com and classical.apple.com domains
    let album_re = Regex::new(
        r"https?://(?:classical|music)\.apple\.com/([a-z]{2})/album/[^/]+/(\d+)(?:\?i=(\d+))?",
    )
    .expect("Invalid regex");

    if let Some(caps) = album_re.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "album".to_string(),
            album_id: caps[2].to_string(),
            song_id: caps.get(3).map(|m| m.as_str().to_string()),
        });
    }

    // Match song URLs: /storefront/song/slug/song_id
    // Accepts both music.apple.com and classical.apple.com domains
    let song_re =
        Regex::new(r"https?://(?:classical|music)\.apple\.com/([a-z]{2})/song/[^/]+/(\d+)")
            .expect("Invalid regex");

    if let Some(caps) = song_re.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "song".to_string(),
            album_id: String::new(), // Songs don't have an album ID in the URL
            song_id: Some(caps[2].to_string()),
        });
    }

    // Match music-video URLs: /storefront/music-video/slug/video_id
    // Accepts both music.apple.com and classical.apple.com domains
    let mv_re =
        Regex::new(r"https?://(?:classical|music)\.apple\.com/([a-z]{2})/music-video/[^/]+/(\d+)")
            .expect("Invalid regex");

    if let Some(caps) = mv_re.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "music-video".to_string(),
            album_id: caps[2].to_string(),
            song_id: None,
        });
    }

    None
}

// ============================================================
// JWT Generation
// ============================================================

/// Generate a short-lived `MusicKit` Developer Token (ES256-signed JWT).
///
/// Apple's `MusicKit` API requires a JWT signed with the developer's private
/// key (P8 format, ECDSA P-256). The JWT contains:
/// - Header: `alg: ES256`, `kid: {key_id}`, `typ: JWT`
/// - Claims: `iss: {team_id}`, `iat: {now}`, `exp: {now + 1 hour}`
///
/// # Arguments
/// * `team_id` - Apple Developer Team ID (10-character alphanumeric)
/// * `key_id` - `MusicKit` Key ID (10-character alphanumeric)
/// * `private_key_pem` - Content of the `.p8` private key file (PEM format)
///
/// # Errors
///
/// Returns `Err(String)` if the private key is invalid or JWT signing fails.
///
/// # Returns
/// * `Ok(String)` - The signed JWT string
/// * `Err(String)` - If the key is invalid or signing fails
pub fn generate_musickit_jwt(
    team_id: &str,
    key_id: &str,
    private_key_pem: &str,
) -> Result<String, String> {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

    // Build the JWT header with the MusicKit key ID.
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(key_id.to_string());
    header.typ = Some("JWT".to_string());

    // Calculate timestamps for the JWT claims.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("System time error: {e}"))?
        .as_secs();

    // Build the JWT claims: issuer (team ID), issued-at, and expiry.
    let claims = serde_json::json!({
        "iss": team_id,
        "iat": now,
        "exp": now + 3600,  // 1 hour from now
    });

    // Parse the PEM private key and sign the JWT.
    let encoding_key = EncodingKey::from_ec_pem(private_key_pem.as_bytes())
        .map_err(|e| format!("Invalid MusicKit private key: {e}"))?;

    encode(&header, &claims, &encoding_key).map_err(|e| format!("Failed to sign MusicKit JWT: {e}"))
}

// ============================================================
// Keychain Integration
// ============================================================

/// Retrieve the `MusicKit` private key from the OS keychain.
///
/// Uses the `keyring` crate with the same service name as the rest of
/// `MeedyaDL`'s credential system. The key is stored under:
///   Service: "io.github.meedyadl"
///   Account: "`musickit_private_key`"
///
/// # Errors
///
/// Returns `Err(String)` if the OS keychain is inaccessible (locked, permission
/// denied, or backend unavailable).
///
/// # Returns
/// * `Ok(Some(String))` - Private key PEM content found
/// * `Ok(None)` - No key stored (user hasn't configured it yet)
/// * `Err(String)` - Keychain access error (locked, permission denied, etc.)
pub fn get_private_key_from_keychain() -> Result<Option<String>, String> {
    const SERVICE_NAME: &str = "io.github.meedyadl";
    const KEY_NAME: &str = "musickit_private_key";

    let entry = keyring::Entry::new(SERVICE_NAME, KEY_NAME)
        .map_err(|e| format!("Failed to create keyring entry: {e}"))?;

    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Failed to retrieve MusicKit private key: {e}")),
    }
}

// ============================================================
// Apple Music Catalog API
// ============================================================

/// Fetch comprehensive album metadata from the Apple Music catalog API.
///
/// Makes a single enriched API call that returns:
/// - Album attributes: UPC, content rating, genre, artist info
/// - Track attributes: ISRC, content rating, artist info, track/disc number
/// - Animated artwork URLs: motionDetailSquare and motionDetailTall HLS URLs
///
/// This consolidates what was previously two separate API calls (one for
/// animated artwork, one for metadata) into a single request.
///
/// # Arguments
/// * `jwt` - `MusicKit` Developer Token (signed JWT)
/// * `storefront` - Two-letter country code (e.g., "us")
/// * `album_id` - Numeric album identifier
///
/// # Errors
///
/// Returns `Err(String)` if the HTTP request fails or the API response
/// cannot be parsed.
///
/// # Returns
/// * `Ok(Some(AlbumMetadata))` - Album found with metadata
/// * `Ok(None)` - Album not found or API returned empty data
/// * `Err(String)` - API request or parsing failure
pub async fn fetch_album_metadata(
    jwt: &str,
    storefront: &str,
    album_id: &str,
) -> Result<Option<AlbumMetadata>, String> {
    // Enriched API call: include tracks and artists, extend with editorialVideo
    let url = format!(
        "https://amp-api.music.apple.com/v1/catalog/{storefront}/albums/{album_id}?include=tracks,artists&extend=editorialVideo"
    );

    log::debug!("Querying Apple Music API for album metadata: {url}");

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {jwt}"))
        .header("User-Agent", "meedyadl")
        .header("Origin", "https://music.apple.com")
        .send()
        .await
        .map_err(|e| format!("Apple Music API request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        return Err(format!(
            "Apple Music API returned HTTP {status} for album {album_id}"
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Apple Music API response: {e}"))?;

    // Navigate to data[0] — the album object
    let Some(album_data) = json.get("data").and_then(|d| d.get(0)) else {
        return Ok(None);
    };

    let Some(attributes) = album_data.get("attributes") else {
        return Ok(None);
    };

    // Extract album-level fields
    let upc = attributes
        .get("upc")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    let content_rating = attributes
        .get("contentRating")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    let genre_names = attributes
        .get("genreNames")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                .collect()
        })
        .unwrap_or_default();

    let album_artist_name = attributes
        .get("artistName")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    // Extract album artist ID from relationships.artists
    let album_artist_id = album_data
        .get("relationships")
        .and_then(|r| r.get("artists"))
        .and_then(|a| a.get("data"))
        .and_then(|d| d.get(0))
        .and_then(|artist| artist.get("id"))
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    // Extract animated artwork HLS URLs from editorialVideo
    let editorial_video = attributes.get("editorialVideo");

    let artwork_square_url = editorial_video
        .and_then(|ev| ev.get("motionDetailSquare"))
        .and_then(|m| m.get("video"))
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    let artwork_tall_url = editorial_video
        .and_then(|ev| ev.get("motionDetailTall"))
        .and_then(|m| m.get("video"))
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    // Extract track metadata from relationships.tracks
    let tracks = parse_tracks_from_response(album_data);

    Ok(Some(AlbumMetadata {
        album_id: album_id.to_string(),
        upc,
        content_rating,
        genre_names,
        artist_id: album_artist_id,
        artist_name: album_artist_name,
        tracks,
        artwork_square_url,
        artwork_tall_url,
    }))
}

/// Parse track metadata from the album API response's relationships.tracks field.
///
/// Path: data[0].relationships.tracks.data[*]
/// Each track has: id, attributes (name, isrc, contentRating, trackNumber,
/// discNumber, artistName), and relationships.artists.
fn parse_tracks_from_response(album_data: &serde_json::Value) -> Vec<TrackMetadata> {
    let Some(track_data) = album_data
        .get("relationships")
        .and_then(|r| r.get("tracks"))
        .and_then(|t| t.get("data"))
        .and_then(|d| d.as_array())
    else {
        return Vec::new();
    };

    track_data
        .iter()
        .filter_map(|track| {
            let song_id = track.get("id")?.as_str()?.to_string();
            let attrs = track.get("attributes")?;

            let name = attrs
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let isrc = attrs
                .get("isrc")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            let content_rating = attrs
                .get("contentRating")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            let artist_name = attrs
                .get("artistName")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            // Extract track artist ID from per-track relationships
            let artist_id = track
                .get("relationships")
                .and_then(|r| r.get("artists"))
                .and_then(|a| a.get("data"))
                .and_then(|d| d.get(0))
                .and_then(|artist| artist.get("id"))
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            let track_number = attrs
                .get("trackNumber")
                .and_then(serde_json::Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(0);

            let disc_number = attrs
                .get("discNumber")
                .and_then(serde_json::Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(1);

            Some(TrackMetadata {
                song_id,
                isrc,
                content_rating,
                artist_id,
                artist_name,
                name,
                track_number,
                disc_number,
            })
        })
        .collect()
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // URL parsing tests
    // ----------------------------------------------------------

    #[test]
    fn parse_standard_album_url() {
        let url = "https://music.apple.com/us/album/midnights/1649434004";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.content_type, "album");
        assert_eq!(parsed.album_id, "1649434004");
        assert!(parsed.song_id.is_none());
    }

    #[test]
    fn parse_album_url_with_track_id() {
        let url = "https://music.apple.com/gb/album/anti-hero/1649434004?i=1649434280";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "gb");
        assert_eq!(parsed.content_type, "album");
        assert_eq!(parsed.album_id, "1649434004");
        assert_eq!(parsed.song_id.as_deref(), Some("1649434280"));
    }

    #[test]
    fn parse_non_us_storefront() {
        let url = "https://music.apple.com/jp/album/some-album/9876543210";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "jp");
        assert_eq!(parsed.album_id, "9876543210");
    }

    #[test]
    fn parse_song_url() {
        let url = "https://music.apple.com/us/song/anti-hero/1649434280";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.content_type, "song");
        assert_eq!(parsed.song_id.as_deref(), Some("1649434280"));
    }

    #[test]
    fn parse_music_video_url() {
        let url = "https://music.apple.com/us/music-video/anti-hero/1649434280";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.content_type, "music-video");
        assert_eq!(parsed.album_id, "1649434280");
    }

    #[test]
    fn parse_playlist_url_returns_none() {
        let url =
            "https://music.apple.com/us/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb";
        let result = parse_apple_music_url(url);
        assert!(result.is_none());
    }

    #[test]
    fn parse_artist_url_returns_none() {
        let url = "https://music.apple.com/us/artist/taylor-swift/159260351";
        let result = parse_apple_music_url(url);
        assert!(result.is_none());
    }

    #[test]
    fn parse_non_apple_music_url_returns_none() {
        let url = "https://www.example.com/some/path";
        let result = parse_apple_music_url(url);
        assert!(result.is_none());
    }

    #[test]
    fn parse_empty_string_returns_none() {
        let result = parse_apple_music_url("");
        assert!(result.is_none());
    }

    #[test]
    fn parse_classical_album_url() {
        let url = "https://classical.apple.com/us/album/beethoven-symphony-no-9/1234567890";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.content_type, "album");
        assert_eq!(parsed.album_id, "1234567890");
        assert!(parsed.song_id.is_none());
    }

    #[test]
    fn parse_classical_album_url_with_track() {
        let url = "https://classical.apple.com/gb/album/beethoven-symphony/1234567890?i=9876543210";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "gb");
        assert_eq!(parsed.content_type, "album");
        assert_eq!(parsed.album_id, "1234567890");
        assert_eq!(parsed.song_id.unwrap(), "9876543210");
    }

    // ----------------------------------------------------------
    // JWT tests
    // ----------------------------------------------------------

    #[test]
    fn generate_jwt_produces_three_part_token() {
        let test_key = "-----BEGIN PRIVATE KEY-----\n\
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgeh6KDqvJ79pjAOBV\n\
aSqMvySOY7Z/xSeiIvUA6uSA0a2hRANCAATuO7iI++EWLlqR8bBjpW3tGnOQnNXi\n\
FJPkH0mNKDTBHi2UUm8qku8mDfB7vmFMjIbzhMqurhYu6/mjzGKIADEv\n\
-----END PRIVATE KEY-----";

        let result = generate_musickit_jwt("TEAM123456", "KEY1234567", test_key);
        assert!(result.is_ok(), "JWT generation failed: {:?}", result.err());

        let token = result.unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(!parts[0].is_empty());
        assert!(!parts[1].is_empty());
        assert!(!parts[2].is_empty());
    }

    #[test]
    fn generate_jwt_rejects_invalid_key() {
        let result = generate_musickit_jwt("TEAM123456", "KEY1234567", "not a valid PEM key");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid MusicKit private key"));
    }

    // ----------------------------------------------------------
    // AlbumMetadata serialization tests
    // ----------------------------------------------------------

    #[test]
    fn album_metadata_serializes_correctly() {
        let metadata = AlbumMetadata {
            album_id: "12345".to_string(),
            upc: Some("00602445790258".to_string()),
            content_rating: Some("explicit".to_string()),
            genre_names: vec!["Pop".to_string(), "Music".to_string()],
            artist_id: Some("159260351".to_string()),
            artist_name: Some("Taylor Swift".to_string()),
            tracks: vec![TrackMetadata {
                song_id: "1649434280".to_string(),
                isrc: Some("USUG12345678".to_string()),
                content_rating: Some("explicit".to_string()),
                artist_id: Some("159260351".to_string()),
                artist_name: Some("Taylor Swift".to_string()),
                name: "Anti-Hero".to_string(),
                track_number: 3,
                disc_number: 1,
            }],
            artwork_square_url: Some("https://example.com/square.m3u8".to_string()),
            artwork_tall_url: None,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"upc\":\"00602445790258\""));
        assert!(json.contains("\"isrc\":\"USUG12345678\""));
        assert!(json.contains("\"track_number\":3"));
    }

    // ----------------------------------------------------------
    // API response parsing tests
    // ----------------------------------------------------------

    #[test]
    fn parse_tracks_from_sample_response() {
        let sample = serde_json::json!({
            "id": "1649434004",
            "type": "albums",
            "attributes": {
                "name": "Midnights",
                "artistName": "Taylor Swift",
                "upc": "00602445790258",
                "contentRating": "explicit",
                "genreNames": ["Pop", "Music"]
            },
            "relationships": {
                "tracks": {
                    "data": [
                        {
                            "id": "1649434005",
                            "type": "songs",
                            "attributes": {
                                "name": "Lavender Haze",
                                "isrc": "USUG12300001",
                                "contentRating": "explicit",
                                "artistName": "Taylor Swift",
                                "trackNumber": 1,
                                "discNumber": 1
                            }
                        },
                        {
                            "id": "1649434006",
                            "type": "songs",
                            "attributes": {
                                "name": "Maroon",
                                "isrc": "USUG12300002",
                                "artistName": "Taylor Swift",
                                "trackNumber": 2,
                                "discNumber": 1
                            }
                        }
                    ]
                },
                "artists": {
                    "data": [
                        {
                            "id": "159260351",
                            "type": "artists"
                        }
                    ]
                }
            }
        });

        let tracks = parse_tracks_from_response(&sample);
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].song_id, "1649434005");
        assert_eq!(tracks[0].name, "Lavender Haze");
        assert_eq!(tracks[0].isrc.as_deref(), Some("USUG12300001"));
        assert_eq!(tracks[0].track_number, 1);
        assert_eq!(tracks[0].disc_number, 1);
        assert_eq!(tracks[1].song_id, "1649434006");
        assert_eq!(tracks[1].track_number, 2);
        assert!(tracks[1].content_rating.is_none()); // Maroon has no contentRating
    }

    #[test]
    fn parse_tracks_empty_when_no_relationships() {
        let sample = serde_json::json!({
            "id": "12345",
            "attributes": {
                "name": "Test Album"
            }
        });

        let tracks = parse_tracks_from_response(&sample);
        assert!(tracks.is_empty());
    }
}
