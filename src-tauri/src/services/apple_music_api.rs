// Copyright (c) 2026 MeedyaDL
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
// The Apple Music catalog API (api.music.apple.com) requires a MusicKit
// Developer Token (JWT) signed with an ES256 private key. Credentials:
//   - Team ID + Key ID: stored in AppSettings (non-sensitive)
//   - Private key (.p8 PEM): stored in OS keychain under "musickit_private_key"
//
// @see animated_artwork_service.rs -- Consumes artwork URLs from AlbumMetadata
// @see metadata_tag_service.rs -- Consumes track metadata from AlbumMetadata
// @see https://developer.apple.com/documentation/applemusicapi/

use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Compile-time embedded MusicKit developer token.
///
/// Set via the `MUSICKIT_DEVELOPER_TOKEN` environment variable at build time
/// (for example from a GitHub Actions secret). This enables release builds to
/// ship with API access for end users who do not have their own Apple
/// Developer credentials.
///
/// Security note: this is a bearer token (not a private key), but it can still
/// be extracted from binaries and should be scoped/rotated operationally.
const EMBEDDED_MUSICKIT_DEVELOPER_TOKEN: Option<&str> = option_env!("MUSICKIT_DEVELOPER_TOKEN");

/// The cookie name used by Apple Music's web client to store the subscriber
/// authentication token. Shared across modules that need to detect or extract
/// this cookie (login window webview, Netscape cookie file parsing).
pub const MEDIA_USER_TOKEN_COOKIE_NAME: &str = "media-user-token";

/// Keychain account name for the web player developer token extracted from
/// the Apple Music login window WebView.
const WEBPLAYER_TOKEN_KEYCHAIN_KEY: &str = "webplayer_developer_token";

/// Keychain service name (shared with `credentials.rs`).
const SERVICE_NAME: &str = "io.github.meedyadl";

/// Identifies which mechanism provided the MusicKit developer token.
///
/// Used by callers of `resolve_premium_feature_token()` for diagnostic
/// logging. The variant names intentionally avoid revealing implementation
/// details in public-facing output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    /// User-provided Team ID + Key ID + .p8 private key → self-generated JWT.
    UserCredentials,
    /// Compile-time embedded developer token from CI secret.
    EmbeddedBuildToken,
    /// Token extracted from the Apple Music web player login window.
    WebPlayerExtracted,
}

impl std::fmt::Display for TokenSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserCredentials => write!(f, "user credentials"),
            Self::EmbeddedBuildToken => write!(f, "embedded token"),
            Self::WebPlayerExtracted => write!(f, "web session"),
        }
    }
}

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
    /// Content type from the URL path (e.g., "album", "song", "music-video", "artist")
    pub content_type: String,
    /// Numeric album identifier (e.g., "1649434004")
    pub album_id: String,
    /// Optional song ID from `?i=` query parameter (single-track URLs)
    pub song_id: Option<String>,
    /// Optional artist ID for artist URLs (e.g., "368433979")
    pub artist_id: Option<String>,
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
    /// Album title
    pub album_name: Option<String>,
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
    /// Record label name
    pub record_label: Option<String>,
    /// Full copyright notice
    pub copyright: Option<String>,
    /// Album release date (YYYY-MM-DD)
    pub release_date: Option<String>,
    /// Whether this is a compilation album
    pub is_compilation: Option<bool>,
    /// Whether this is a single release
    pub is_single: Option<bool>,
    /// Whether all tracks in the album are available
    pub is_complete: Option<bool>,
    /// Whether the album is Mastered for iTunes / Apple Digital Master
    pub is_mastered_for_itunes: Option<bool>,
    /// Total number of tracks in the album
    pub track_count: Option<u32>,
    /// Apple's editorial summary (short description)
    pub editorial_notes: Option<String>,
    /// ISO 8601 timestamp of the last metadata modification by Apple.
    /// Used for smart re-download detection — comparing this against the
    /// value stored in the .meedyadl manifest reveals whether the album
    /// has changed since the user's last download.
    pub last_modified_date: Option<String>,
    /// Per-track metadata for all tracks in the album
    pub tracks: Vec<TrackMetadata>,
    /// HLS M3U8 URL for square (1:1) animated artwork, if available
    pub artwork_square_url: Option<String>,
    /// HLS M3U8 URL for portrait (3:4) animated artwork, if available
    pub artwork_tall_url: Option<String>,
    /// Raw API JSON object (data[0]) for config-driven tag extraction via tags.toml.
    /// Contains all album attributes and relationships as returned by the API.
    #[serde(default = "default_json_null")]
    pub raw_json: serde_json::Value,
}

/// Default for raw_json field — null when not populated (backwards compat).
fn default_json_null() -> serde_json::Value {
    serde_json::Value::Null
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
    /// Available audio formats for this track (from `audioTraits` API field).
    /// Values: "lossy-stereo", "lossless", "hi-res-lossless", "dolby-atmos", "spatial".
    /// Empty if the API didn't return audioTraits for this track.
    pub audio_traits: Vec<String>,
    /// Whether this track has Apple Digital Master certification
    pub is_apple_digital_master: Option<bool>,
    /// Track-level release date (YYYY-MM-DD), may differ from album release date
    pub release_date: Option<String>,
    /// Songwriter / composer credits
    pub composer_name: Option<String>,
    /// Track duration in milliseconds (from catalog, precise)
    pub duration_in_millis: Option<u64>,
    /// Whether Apple Music has lyrics for this track
    pub has_lyrics: Option<bool>,
    /// Apple Music catalog play params ID
    pub play_params_id: Option<String>,
    /// Canonical Apple Music URL for this track
    pub url: Option<String>,
    /// 30-second preview URL
    pub preview_url: Option<String>,
    /// Genre names for this track (first entry is primary genre)
    pub genre_names: Vec<String>,
    /// Raw API JSON object for this track, for config-driven tag extraction via tags.toml.
    #[serde(default = "default_json_null")]
    pub raw_json: serde_json::Value,
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
    use std::sync::LazyLock;

    // Static regex instances compiled once at first use. Avoids recompiling
    // on every call (this function is called on every URL entered and every
    // queue restore). The .expect() on a LazyLock only runs once — a regex
    // compilation failure here indicates a code defect, not a runtime error.

    // Match album URLs: /storefront/album/slug/album_id with optional ?i=song_id
    // Accepts both music.apple.com and classical.apple.com domains
    static ALBUM_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"https?://(?:classical|music)\.apple\.com/([a-z]{2})/album/[^/]+/(\d+)(?:\?i=(\d+))?",
        )
        .expect("Invalid album regex")
    });

    // Match song URLs: /storefront/song/slug/song_id
    static SONG_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"https?://(?:classical|music)\.apple\.com/([a-z]{2})/song/[^/]+/(\d+)")
            .expect("Invalid song regex")
    });

    // Match music-video URLs: /storefront/music-video/slug/video_id
    static MV_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"https?://(?:classical|music)\.apple\.com/([a-z]{2})/music-video/[^/]+/(\d+)")
            .expect("Invalid music-video regex")
    });

    // Match artist URLs: /storefront/artist/slug/artist_id
    static ARTIST_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"https?://(?:classical|music)\.apple\.com/([a-z]{2})/artist/[^/]+/(\d+)")
            .expect("Invalid artist regex")
    });

    if let Some(caps) = ALBUM_RE.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "album".to_string(),
            album_id: caps[2].to_string(),
            song_id: caps.get(3).map(|m| m.as_str().to_string()),
            artist_id: None,
        });
    }

    if let Some(caps) = SONG_RE.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "song".to_string(),
            album_id: String::new(),
            song_id: Some(caps[2].to_string()),
            artist_id: None,
        });
    }

    if let Some(caps) = MV_RE.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "music-video".to_string(),
            album_id: caps[2].to_string(),
            song_id: None,
            artist_id: None,
        });
    }

    if let Some(caps) = ARTIST_RE.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "artist".to_string(),
            album_id: String::new(),
            song_id: None,
            artist_id: Some(caps[2].to_string()),
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

/// Resolve the effective MusicKit developer token for API requests.
///
/// Priority:
/// 1. User-provided Team ID + Key ID + private key (generate fresh JWT)
/// 2. Compile-time embedded `MUSICKIT_DEVELOPER_TOKEN`
/// 3. `None` (MusicKit API unavailable)
///
/// Returns an error only if user credentials are present but invalid.
pub fn resolve_musickit_developer_token(
    team_id: Option<&str>,
    key_id: Option<&str>,
    private_key_pem: Option<&str>,
) -> Result<Option<String>, String> {
    let team = team_id.map(str::trim).filter(|s| !s.is_empty());
    let key = key_id.map(str::trim).filter(|s| !s.is_empty());
    let private_key = private_key_pem.map(str::trim).filter(|s| !s.is_empty());

    if let (Some(team), Some(key), Some(private_key)) = (team, key, private_key) {
        return generate_musickit_jwt(team, key, private_key).map(Some);
    }

    Ok(EMBEDDED_MUSICKIT_DEVELOPER_TOKEN
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string))
}

/// Returns true when a build-time MusicKit developer token is embedded.
#[must_use]
pub fn has_embedded_musickit_developer_token() -> bool {
    EMBEDDED_MUSICKIT_DEVELOPER_TOKEN
        .map(str::trim)
        .is_some_and(|s| !s.is_empty())
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
    const KEY_NAME: &str = "musickit_private_key";

    let entry = keyring::Entry::new(SERVICE_NAME, KEY_NAME)
        .map_err(|e| format!("Failed to create keyring entry: {e}"))?;

    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Failed to retrieve MusicKit private key: {e}")),
    }
}

/// Retrieve the web player developer token from the OS keychain.
///
/// This token is extracted opportunistically from the Apple Music login
/// window WebView during cookie import. It serves as a last-resort fallback
/// for premium API features (syllable-lyrics, animated artwork, music video
/// relations) when the user has not configured their own MusicKit credentials.
///
/// # Returns
/// * `Ok(Some(String))` - Web player token found in keychain
/// * `Ok(None)` - No token stored (user hasn't logged in via the login window,
///   or the token was cleared)
/// * `Err(String)` - Keychain access error
pub fn get_webplayer_token_from_keychain() -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(SERVICE_NAME, WEBPLAYER_TOKEN_KEYCHAIN_KEY)
        .map_err(|e| format!("Failed to create keyring entry: {e}"))?;

    match entry.get_password() {
        Ok(token) if token.trim().is_empty() => Ok(None),
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Failed to retrieve web player token: {e}")),
    }
}

/// Store a web player developer token in the OS keychain.
///
/// Called by the login window service after successfully extracting the token
/// from the Apple Music web player. Overwrites any previously stored token.
///
/// # Security Note
/// The token value is never logged — only the key name for auditability.
pub fn store_webplayer_token_in_keychain(token: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, WEBPLAYER_TOKEN_KEYCHAIN_KEY)
        .map_err(|e| format!("Failed to create keyring entry: {e}"))?;

    entry
        .set_password(token)
        .map_err(|e| format!("Failed to store web player token: {e}"))?;

    log::info!("Web player developer token stored securely");
    Ok(())
}

/// Delete the web player developer token from the OS keychain.
///
/// Idempotent — returns `Ok(())` even if no token was stored.
pub fn clear_webplayer_token_from_keychain() -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE_NAME, WEBPLAYER_TOKEN_KEYCHAIN_KEY)
        .map_err(|e| format!("Failed to create keyring entry: {e}"))?;

    match entry.delete_credential() {
        Ok(()) => {
            log::info!("Web player developer token cleared");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Failed to delete web player token: {e}")),
    }
}

/// Returns true when a web player developer token is stored in the keychain.
#[must_use]
pub fn has_webplayer_token() -> bool {
    get_webplayer_token_from_keychain()
        .ok()
        .flatten()
        .is_some()
}

/// Resolve a MusicKit developer token for premium API features only.
///
/// This is the **extended** resolver used exclusively for:
/// - Syllable-lyrics API (`/syllable-lyrics`)
/// - Animated album artwork download
/// - Music video relation lookups
///
/// Unlike `resolve_musickit_developer_token()`, this adds a third fallback
/// tier: the web player token extracted from the login window. General
/// catalog API calls (`fetch_album_metadata`, etc.) should continue to use
/// `resolve_musickit_developer_token()` — they must NOT use the web player
/// token.
///
/// # Priority
/// 1. User-provided Team ID + Key ID + private key → self-generated JWT
/// 2. Compile-time embedded `MUSICKIT_DEVELOPER_TOKEN`
/// 3. Web player token from OS keychain (last resort)
/// 4. `None` (premium features unavailable)
///
/// Returns an error only if user credentials are present but invalid.
pub fn resolve_premium_feature_token(
    team_id: Option<&str>,
    key_id: Option<&str>,
    private_key_pem: Option<&str>,
) -> Result<Option<(String, TokenSource)>, String> {
    let team = team_id.map(str::trim).filter(|s| !s.is_empty());
    let key = key_id.map(str::trim).filter(|s| !s.is_empty());
    let private_key = private_key_pem.map(str::trim).filter(|s| !s.is_empty());

    // Priority 1: User-provided MusicKit credentials
    if let (Some(team), Some(key), Some(pk)) = (team, key, private_key) {
        let jwt = generate_musickit_jwt(team, key, pk)?;
        return Ok(Some((jwt, TokenSource::UserCredentials)));
    }

    // Priority 2: Compile-time embedded developer token
    if let Some(token) = EMBEDDED_MUSICKIT_DEVELOPER_TOKEN
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Ok(Some((token.to_string(), TokenSource::EmbeddedBuildToken)));
    }

    // Priority 3: Web player token from keychain (last resort)
    match get_webplayer_token_from_keychain() {
        Ok(Some(token)) => Ok(Some((token, TokenSource::WebPlayerExtracted))),
        Ok(None) => Ok(None),
        Err(e) => {
            // Keychain errors for the fallback path are non-fatal — log and
            // continue as if no token is available.
            log::warn!("Web player token keychain error (non-fatal): {e}");
            Ok(None)
        }
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
        "https://api.music.apple.com/v1/catalog/{storefront}/albums/{album_id}?include=tracks,artists&extend=editorialVideo"
    );

    log::debug!("Querying Apple Music API for album metadata: {url}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {jwt}"))
        .header("User-Agent", "meedyadl")
        .send()
        .await
        .map_err(|e| format!("Apple Music API request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        // Provide actionable guidance for common authentication errors
        let detail = match status {
            401 => " — check MusicKit credentials (Team ID, Key ID, or private key may be expired/revoked on developer.apple.com)",
            403 => " — MusicKit key may lack required permissions (MusicKit service must be enabled for the key)",
            429 => " — rate limited by Apple Music API, try again later",
            _ => "",
        };
        return Err(format!(
            "Apple Music API returned HTTP {status} for album {album_id}{detail}"
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
    let album_name = attributes
        .get("name")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

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

    let record_label = attributes
        .get("recordLabel")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    let copyright = attributes
        .get("copyright")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    let album_release_date = attributes
        .get("releaseDate")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    // ISO 8601 timestamp of when Apple last modified this resource's metadata.
    // Used for smart re-download detection (#263).
    let last_modified_date = attributes
        .get("lastModifiedDate")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    let is_compilation = attributes.get("isCompilation").and_then(|v| v.as_bool());

    let is_single = attributes.get("isSingle").and_then(|v| v.as_bool());

    let is_complete = attributes.get("isComplete").and_then(|v| v.as_bool());

    let is_mastered_for_itunes = attributes
        .get("isMasteredForItunes")
        .and_then(|v| v.as_bool());

    let track_count = attributes
        .get("trackCount")
        .and_then(serde_json::Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());

    let editorial_notes = attributes
        .get("editorialNotes")
        .and_then(|en| en.get("short"))
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

    log::debug!(
        "API parsed: album={}, tracks={}, has_artwork_square={}, has_artwork_tall={}, upc={}",
        album_name.as_deref().unwrap_or("?"),
        tracks.len(),
        artwork_square_url.is_some(),
        artwork_tall_url.is_some(),
        upc.as_deref().unwrap_or("N/A"),
    );

    Ok(Some(AlbumMetadata {
        album_id: album_id.to_string(),
        album_name,
        upc,
        content_rating,
        genre_names,
        artist_id: album_artist_id,
        artist_name: album_artist_name,
        record_label,
        copyright,
        release_date: album_release_date,
        last_modified_date,
        is_compilation,
        is_single,
        is_complete,
        is_mastered_for_itunes,
        track_count,
        editorial_notes,
        tracks,
        artwork_square_url,
        artwork_tall_url,
        raw_json: album_data.clone(),
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

            // Extract audioTraits array (e.g., ["lossy-stereo", "lossless", "dolby-atmos"])
            // This is returned by default in the catalog API response — no extend needed.
            let audio_traits = attrs
                .get("audioTraits")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                        .collect()
                })
                .unwrap_or_default();

            // Apple Digital Master / Mastered for iTunes certification (track-level)
            let is_apple_digital_master =
                attrs.get("isAppleDigitalMaster").and_then(|v| v.as_bool());

            // Track-level release date (may differ from album release date)
            let release_date = attrs
                .get("releaseDate")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            // Songwriter / composer credits
            let composer_name = attrs
                .get("composerName")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            // Precise duration from catalog (milliseconds)
            let duration_in_millis = attrs
                .get("durationInMillis")
                .and_then(serde_json::Value::as_u64);

            // Whether Apple Music has lyrics for this track
            let has_lyrics = attrs.get("hasLyrics").and_then(|v| v.as_bool());

            // Unique play identifier (catalog ID) from playParams
            let play_params_id = attrs
                .get("playParams")
                .and_then(|pp| pp.get("id"))
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            // Canonical Apple Music URL for the track
            let url = attrs
                .get("url")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            // 30-second preview URL (first preview entry)
            let preview_url = attrs
                .get("previews")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|p| p.get("url"))
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);

            // Per-track genre names
            let genre_names = attrs
                .get("genreNames")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                        .collect()
                })
                .unwrap_or_default();

            Some(TrackMetadata {
                song_id,
                isrc,
                content_rating,
                artist_id,
                artist_name,
                name,
                track_number,
                disc_number,
                audio_traits,
                is_apple_digital_master,
                release_date,
                composer_name,
                duration_in_millis,
                has_lyrics,
                play_params_id,
                url,
                preview_url,
                genre_names,
                raw_json: track.clone(),
            })
        })
        .collect()
}

// ============================================================
// Music Video Relationship Lookup
// ============================================================

/// Metadata for a music video related to a song.
///
/// Returned by `fetch_music_video_relations()` for songs that have
/// a corresponding music video on Apple Music.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicVideoRelation {
    /// Apple Music song ID that this music video is related to
    pub song_id: String,
    /// Apple Music music video ID
    pub music_video_id: String,
    /// Music video title (for logging)
    pub name: Option<String>,
}

/// Construct an Apple Music music video URL from a storefront and video ID.
///
/// The URL format matches the standard Apple Music music-video URL pattern
/// that GAMDL accepts for downloading.
///
/// # Arguments
/// * `storefront` - Two-letter country code (e.g., "us", "gb")
/// * `music_video_id` - Apple Music music video numeric ID
///
/// # Returns
/// A fully-formed Apple Music music video URL
#[must_use]
pub fn build_music_video_url(storefront: &str, music_video_id: &str) -> String {
    format!("https://music.apple.com/{storefront}/music-video/mv/{music_video_id}")
}

/// Look up music video relationships for a batch of song IDs.
///
/// Queries the Apple Music catalog songs endpoint with `relate=music-videos`
/// to find which songs have corresponding music videos. Songs without music
/// videos are omitted from the result.
///
/// Song IDs are batched into groups of up to 100 per API request to stay
/// within URL length limits. Each batch is a single HTTP GET.
///
/// # Arguments
/// * `jwt` - MusicKit Developer Token (signed JWT)
/// * `storefront` - Two-letter country code (e.g., "us")
/// * `song_ids` - List of Apple Music song IDs to look up
///
/// # Returns
/// A vector of `MusicVideoRelation` structs for songs that have music videos.
pub async fn fetch_music_video_relations(
    jwt: &str,
    storefront: &str,
    song_ids: &[String],
) -> Result<Vec<MusicVideoRelation>, String> {
    if song_ids.is_empty() {
        return Ok(Vec::new());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;
    let mut relations = Vec::new();

    // Batch song IDs into groups of 100 (Apple Music API limit per request)
    for chunk in song_ids.chunks(100) {
        let ids_param = chunk.join(",");
        let url = format!(
            "https://api.music.apple.com/v1/catalog/{storefront}/songs?ids={ids_param}&relate=music-videos"
        );

        log::debug!(
            "Querying Apple Music API for music video relations: {} song(s)",
            chunk.len()
        );

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {jwt}"))
            .header("User-Agent", "meedyadl")
            .send()
            .await
            .map_err(|e| format!("Music video relation lookup failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(format!(
                "Apple Music API returned HTTP {status} for music video relation lookup"
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse music video relations response: {e}"))?;

        // Parse each song in the response
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            for song in data {
                let song_id = match song.get("id").and_then(|v| v.as_str()) {
                    Some(id) => id.to_string(),
                    None => continue,
                };

                // Check for music-videos relationship
                let mv_data = song
                    .get("relationships")
                    .and_then(|r| r.get("music-videos"))
                    .and_then(|mv| mv.get("data"))
                    .and_then(|d| d.as_array());

                if let Some(music_videos) = mv_data {
                    for mv in music_videos {
                        let music_video_id = match mv.get("id").and_then(|v| v.as_str()) {
                            Some(id) => id.to_string(),
                            None => continue,
                        };

                        let name = mv
                            .get("attributes")
                            .and_then(|a| a.get("name"))
                            .and_then(|v| v.as_str())
                            .map(std::string::ToString::to_string);

                        relations.push(MusicVideoRelation {
                            song_id: song_id.clone(),
                            music_video_id,
                            name,
                        });
                    }
                }
            }
        }
    }

    Ok(relations)
}

// ============================================================
// Unit Tests
// ============================================================

// ============================================================
// Non-Geographic URL Normalization
// ============================================================

/// Normalize an Apple Music URL by injecting a storefront code if missing.
///
/// GAMDL requires a 2-letter storefront code in the URL path (e.g., `/us/`).
/// This function detects non-geographic URLs (e.g., `music.apple.com/album/...`)
/// and injects a storefront code to produce `music.apple.com/{sf}/album/...`.
///
/// Storefront resolution priority:
/// 1. URL-embedded — if already present, returns the URL unchanged
/// 2. OS locale — fast, no network, uses `detect_storefront()` from login_window_service
/// 3. Fallback — defaults to "us" (Apple's own redirect default)
///
/// Also supports `classical.apple.com` and `itunes.apple.com` domains.
///
/// # Arguments
/// * `url` - The Apple Music URL to normalize
///
/// # Returns
/// The URL with a storefront code guaranteed to be present. Non-Apple-Music
/// URLs are returned unchanged.
#[must_use]
pub fn normalize_apple_music_url(url: &str) -> String {
    // If the URL already has a storefront (existing regex matches), return as-is.
    if parse_apple_music_url(url).is_some() {
        return url.to_string();
    }

    // Check if it's a non-geographic Apple Music URL (content type keyword
    // immediately after the domain, with no storefront segment).
    if let Some((base, rest)) = detect_non_geographic_url(url) {
        let storefront = resolve_storefront_sync();
        log::info!("Non-geographic Apple Music URL detected — injecting storefront '{storefront}'");
        return format!("{base}/{storefront}{rest}");
    }

    // Not an Apple Music URL or doesn't match non-geographic pattern — return unchanged.
    url.to_string()
}

/// Detect whether an Apple Music URL is missing a storefront code.
///
/// Returns `Some((base, rest))` where `base` is the domain prefix
/// (e.g., `https://music.apple.com`) and `rest` is the remaining path
/// (e.g., `/album/midnights/1649434004`).
///
/// Returns `None` if the URL already has a storefront or is not Apple Music.
///
/// Safe because all content type keywords (`album`, `song`, `playlist`,
/// `music-video`, `artist`) are longer than 3 characters, so they can never
/// be confused with a 2-letter storefront code.
fn detect_non_geographic_url(url: &str) -> Option<(&str, &str)> {
    use std::sync::LazyLock;

    // Matches Apple Music URLs where the first path segment after the domain
    // is a content type keyword rather than a 2-letter storefront code.
    // Group 1: domain prefix (e.g., "https://music.apple.com")
    // The rest of the URL after group 1 starts with /{content_type}/...
    static NON_GEO_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(https?://(?:classical|music|itunes)\.apple\.com)/(album|song|playlist|music-video|artist)(/.*)?$",
        )
        .expect("Invalid non-geographic URL regex")
    });

    let caps = NON_GEO_RE.captures(url)?;
    let base_end = caps.get(1)?.end();
    Some((&url[..base_end], &url[base_end..]))
}

/// Resolve the best available storefront code without network I/O.
///
/// Priority:
/// 1. OS locale via `detect_storefront()` from login_window_service
/// 2. Fallback: `"us"` (Apple's own default redirect target)
fn resolve_storefront_sync() -> String {
    super::login_window_service::detect_storefront().unwrap_or_else(|| "us".to_string())
}

// ============================================================
// Storefront Fallback for Enrichment API
// ============================================================

/// Fetch album metadata with automatic storefront fallback.
///
/// Tries the primary storefront first. If the Apple Music API returns
/// HTTP 404 or no data, retries with alternative storefronts. This handles
/// cases where a user shares a URL with a storefront that doesn't match
/// the album's catalog region.
///
/// Fallback chain:
/// 1. Primary storefront (from the URL)
/// 2. OS locale-derived storefront (if different from primary)
/// 3. `"us"` (Apple's largest catalog, if different from both)
///
/// Non-404 errors (auth failure, network timeout) are NOT retried — they
/// indicate infrastructure issues, not region mismatches.
///
/// # Returns
/// * `Ok(Some(AlbumMetadata))` - Album found (possibly via a fallback storefront)
/// * `Ok(None)` - Album not found in any storefront
/// * `Err(String)` - Non-recoverable API error
pub async fn fetch_album_metadata_with_fallback(
    jwt: &str,
    primary_storefront: &str,
    album_id: &str,
) -> Result<Option<AlbumMetadata>, String> {
    // Try primary storefront first.
    match fetch_album_metadata(jwt, primary_storefront, album_id).await {
        Ok(Some(metadata)) => return Ok(Some(metadata)),
        Ok(None) => {
            log::debug!(
                "Album {album_id} returned empty data in storefront '{primary_storefront}', trying fallbacks"
            );
        }
        Err(ref e) if e.contains("HTTP 404") || e.contains("HTTP 400") => {
            log::debug!(
                "Album {album_id} returned error in storefront '{primary_storefront}': {e}, trying fallbacks"
            );
        }
        Err(e) => return Err(e), // Non-region errors (auth, network) — propagate immediately
    }

    // Build deduplicated fallback list (excluding primary).
    let mut fallbacks: Vec<String> = Vec::new();
    if let Some(locale_sf) = super::login_window_service::detect_storefront() {
        if locale_sf != primary_storefront && !fallbacks.contains(&locale_sf) {
            fallbacks.push(locale_sf);
        }
    }
    if primary_storefront != "us" && !fallbacks.iter().any(|s| s == "us") {
        fallbacks.push("us".to_string());
    }

    for sf in &fallbacks {
        log::debug!("Trying fallback storefront '{sf}' for album {album_id}");
        match fetch_album_metadata(jwt, sf, album_id).await {
            Ok(Some(metadata)) => {
                log::info!(
                    "Album {album_id} found via fallback storefront '{sf}' (primary was '{primary_storefront}')"
                );
                return Ok(Some(metadata));
            }
            Ok(None) | Err(_) => continue,
        }
    }

    // All storefronts exhausted.
    log::debug!(
        "Album {album_id} not found in any storefront (tried: '{primary_storefront}', {:?})",
        fallbacks
    );
    Ok(None)
}

// ============================================================
// Syllable-Level Lyrics (Word-by-Word TTML)
// ============================================================

/// Extract the `media-user-token` value from an Apple Music Netscape cookies file.
///
/// The `/syllable-lyrics` endpoint requires subscriber authentication via a
/// `Music-User-Token` header in addition to the MusicKit Developer Token JWT.
/// This token is stored as the `media-user-token` cookie by Apple Music's web
/// client after the user signs in.
///
/// # Arguments
/// * `cookies_path` - Path to the Netscape-format cookies.txt file
///
/// # Returns
/// * `Ok(Some(String))` - Token value found
/// * `Ok(None)` - Cookies file exists but no `media-user-token` cookie present
/// * `Err(String)` - File read error
pub fn extract_media_user_token(cookies_path: &str) -> Result<Option<String>, String> {
    let content = std::fs::read_to_string(cookies_path)
        .map_err(|e| format!("Failed to read cookies file: {e}"))?;

    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Netscape cookie format: domain \t flag \t path \t secure \t expires \t name \t value
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() >= 7 && fields[5] == MEDIA_USER_TOKEN_COOKIE_NAME {
            let value = fields[6].trim();
            if value.is_empty() {
                continue;
            }
            // Check cookie expiry (field 4). A value of 0 means session cookie (no expiry).
            if let Ok(expires) = fields[4].parse::<u64>() {
                if expires > 0 && expires < now_epoch {
                    log::warn!(
                        "media-user-token cookie has expired (expired {} seconds ago). Re-import cookies from your browser.",
                        now_epoch - expires
                    );
                    return Ok(None);
                }
            }
            return Ok(Some(value.to_string()));
        }
    }

    Ok(None)
}

/// Fetch word-level (syllable) TTML lyrics for a song from Apple Music.
///
/// Calls the `/syllable-lyrics` endpoint which returns TTML with
/// `itunes:timing="Word"` and per-word `<span begin="" end="">` elements,
/// providing word-by-word timing data for Enhanced LRC generation.
///
/// This endpoint requires **both** a MusicKit Developer Token (JWT) and a
/// `Music-User-Token` from an authenticated Apple Music subscriber session.
///
/// # Arguments
/// * `jwt` - MusicKit Developer Token (ES256-signed JWT)
/// * `storefront` - Two-letter country code (e.g., "us", "gb")
/// * `song_id` - Apple Music numeric song ID
/// * `music_user_token` - Subscriber token from `media-user-token` cookie
///
/// # Returns
/// * `Ok(Some(String))` - Raw TTML XML with word-level timing
/// * `Ok(None)` - No syllable-lyrics available for this track
/// * `Err(String)` - API or network error
pub async fn fetch_syllable_lyrics(
    jwt: &str,
    storefront: &str,
    song_id: &str,
    music_user_token: &str,
) -> Result<Option<String>, String> {
    let url = format!(
        "https://api.music.apple.com/v1/catalog/{storefront}/songs/{song_id}/syllable-lyrics"
    );

    log::debug!("Fetching syllable-lyrics for song {song_id} (storefront: {storefront})");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {jwt}"))
        .header("Music-User-Token", music_user_token)
        .header("User-Agent", "meedyadl")
        .send()
        .await
        .map_err(|e| format!("Syllable-lyrics request failed for song {song_id}: {e}"))?;

    match response.status().as_u16() {
        200 => {}
        404 => {
            log::debug!("No syllable-lyrics available for song {song_id}");
            return Ok(None);
        }
        401 => {
            return Err(
                "Syllable-lyrics auth failed (HTTP 401) — Music-User-Token may be expired. Re-import cookies from your browser.".to_string(),
            );
        }
        403 => {
            return Err(
                "Syllable-lyrics forbidden (HTTP 403) — an active Apple Music subscription is required.".to_string(),
            );
        }
        status => {
            return Err(format!(
                "Syllable-lyrics API returned HTTP {status} for song {song_id}"
            ));
        }
    }

    // The response is a JSON envelope containing TTML content.
    // Extract the TTML string from data[0].attributes.ttml
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse syllable-lyrics response: {e}"))?;

    let ttml = json
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("attributes"))
        .and_then(|a| a.get("ttml"))
        .and_then(|t| t.as_str())
        .map(String::from);

    if ttml.is_some() {
        log::debug!("Syllable-lyrics TTML fetched for song {song_id}");
    } else {
        log::debug!("Syllable-lyrics response for song {song_id} contained no TTML data");
    }

    Ok(ttml)
}

// ============================================================
// Artist Promo Video
// ============================================================

/// Metadata for an Apple Music artist's promotional video.
///
/// These are the animated backgrounds displayed on Apple Music artist pages,
/// served as HLS streams. Not all artists have promo videos.
#[derive(Debug, Clone)]
pub struct ArtistPromoVideo {
    /// Artist display name
    pub artist_name: String,
    /// HLS M3U8 URL for the promo video
    pub video_url: String,
}

/// Fetch the promotional video URL for an Apple Music artist.
///
/// Queries the Apple Music API for the artist's `editorialVideo` field,
/// which contains the animated background shown on the artist's page.
///
/// # Arguments
/// * `jwt` - MusicKit Developer Token (signed JWT)
/// * `storefront` - Two-letter country code (e.g., "us", "gb")
/// * `artist_id` - Numeric artist identifier (e.g., "368433979")
///
/// # Returns
/// * `Ok(Some(ArtistPromoVideo))` - Artist has a promo video
/// * `Ok(None)` - Artist found but has no promo video
/// * `Err(String)` - API request or parsing failure
pub async fn fetch_artist_promo_video(
    jwt: &str,
    storefront: &str,
    artist_id: &str,
) -> Result<Option<ArtistPromoVideo>, String> {
    let url = format!(
        "https://api.music.apple.com/v1/catalog/{storefront}/artists/{artist_id}?extend=editorialVideo"
    );

    log::debug!("Querying Apple Music API for artist promo video: {url}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {jwt}"))
        .header("User-Agent", "meedyadl")
        .send()
        .await
        .map_err(|e| format!("Apple Music API request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        if status == 404 {
            log::debug!("Artist {artist_id} not found (storefront: {storefront})");
            return Ok(None);
        }
        return Err(format!(
            "Apple Music API returned HTTP {status} for artist {artist_id}"
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse artist API response: {e}"))?;

    // Extract the first artist from the response: data[0].attributes
    let artist_data = match json.get("data").and_then(|d| d.get(0)) {
        Some(data) => data,
        None => {
            log::debug!("No artist data in API response for {artist_id}");
            return Ok(None);
        }
    };

    let attributes = match artist_data.get("attributes") {
        Some(attrs) => attrs,
        None => return Ok(None),
    };

    let artist_name = attributes
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Artist")
        .to_string();

    // Extract promo video URL from editorialVideo.
    // The API may use different keys for artist videos:
    // - motionArtistWide16x9 — wide landscape format (most common for artist pages)
    // - motionArtistFullscreen16x9 — fullscreen variant
    // - motionDetailSquare — square format (same as album artwork)
    // - motionDetailTall — tall/portrait format
    // Try each in priority order.
    let editorial_video = match attributes.get("editorialVideo") {
        Some(ev) => ev,
        None => {
            log::debug!("No editorialVideo for artist {artist_name} ({artist_id})");
            return Ok(None);
        }
    };

    // Priority order: wide 16:9 → fullscreen 16:9 → square → tall
    let video_keys = [
        "motionArtistWide16x9",
        "motionArtistFullscreen16x9",
        "motionDetailSquare",
        "motionDetailTall",
    ];

    let video_url = video_keys.iter().find_map(|key| {
        editorial_video
            .get(*key)
            .and_then(|m| m.get("video"))
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string)
    });

    match video_url {
        Some(url) => {
            log::info!(
                "Found promo video for artist {artist_name} ({artist_id})"
            );
            Ok(Some(ArtistPromoVideo {
                artist_name,
                video_url: url,
            }))
        }
        None => {
            log::debug!(
                "editorialVideo present but no video URL found for artist {artist_name} ({artist_id})"
            );
            Ok(None)
        }
    }
}

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
    fn parse_artist_url() {
        let url = "https://music.apple.com/us/artist/taylor-swift/159260351";
        let result = parse_apple_music_url(url).unwrap();
        assert_eq!(result.content_type, "artist");
        assert_eq!(result.storefront, "us");
        assert_eq!(result.artist_id, Some("159260351".to_string()));
        assert!(result.album_id.is_empty());
    }

    #[test]
    fn parse_artist_url_gb_storefront() {
        let url = "https://music.apple.com/gb/artist/zedd/368433979";
        let result = parse_apple_music_url(url).unwrap();
        assert_eq!(result.content_type, "artist");
        assert_eq!(result.storefront, "gb");
        assert_eq!(result.artist_id, Some("368433979".to_string()));
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
            album_name: Some("Midnights".to_string()),
            upc: Some("00602445790258".to_string()),
            content_rating: Some("explicit".to_string()),
            genre_names: vec!["Pop".to_string(), "Music".to_string()],
            artist_id: Some("159260351".to_string()),
            artist_name: Some("Taylor Swift".to_string()),
            record_label: Some("Republic Records".to_string()),
            copyright: Some("℗ 2022 Republic Records".to_string()),
            release_date: Some("2022-10-21".to_string()),
            last_modified_date: None, // Test fixture — not present in mock API response
            is_compilation: Some(false),
            is_single: Some(false),
            is_complete: Some(true),
            is_mastered_for_itunes: Some(true),
            track_count: Some(13),
            editorial_notes: Some("Taylor Swift's tenth studio album.".to_string()),
            tracks: vec![TrackMetadata {
                song_id: "1649434280".to_string(),
                isrc: Some("USUG12345678".to_string()),
                content_rating: Some("explicit".to_string()),
                artist_id: Some("159260351".to_string()),
                artist_name: Some("Taylor Swift".to_string()),
                name: "Anti-Hero".to_string(),
                track_number: 3,
                disc_number: 1,
                audio_traits: vec![
                    "lossy-stereo".to_string(),
                    "lossless".to_string(),
                    "dolby-atmos".to_string(),
                ],
                is_apple_digital_master: Some(true),
                release_date: Some("2022-10-21".to_string()),
                composer_name: Some("Taylor Swift & Jack Antonoff".to_string()),
                duration_in_millis: Some(200_690),
                has_lyrics: Some(true),
                play_params_id: Some("1649434280".to_string()),
                url: Some(
                    "https://music.apple.com/us/album/anti-hero/1649434004?i=1649434280"
                        .to_string(),
                ),
                preview_url: Some("https://audio-ssl.itunes.apple.com/preview.m4a".to_string()),
                genre_names: vec!["Pop".to_string(), "Music".to_string()],
                raw_json: serde_json::Value::Null,
            }],
            artwork_square_url: Some("https://example.com/square.m3u8".to_string()),
            artwork_tall_url: None,
            raw_json: serde_json::Value::Null,
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
                                "discNumber": 1,
                                "isAppleDigitalMaster": true,
                                "releaseDate": "2022-10-21",
                                "composerName": "Taylor Swift & Jack Antonoff",
                                "durationInMillis": 202395,
                                "hasLyrics": true,
                                "playParams": { "id": "1649434005" },
                                "url": "https://music.apple.com/us/album/lavender-haze/1649434004?i=1649434005",
                                "previews": [{ "url": "https://audio-ssl.itunes.apple.com/lavender.m4a" }],
                                "genreNames": ["Pop", "Music"]
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

        // Track 1: has all fields populated
        assert_eq!(tracks[0].song_id, "1649434005");
        assert_eq!(tracks[0].name, "Lavender Haze");
        assert_eq!(tracks[0].isrc.as_deref(), Some("USUG12300001"));
        assert_eq!(tracks[0].track_number, 1);
        assert_eq!(tracks[0].disc_number, 1);
        assert_eq!(tracks[0].is_apple_digital_master, Some(true));
        assert_eq!(tracks[0].release_date.as_deref(), Some("2022-10-21"));
        assert_eq!(
            tracks[0].composer_name.as_deref(),
            Some("Taylor Swift & Jack Antonoff")
        );
        assert_eq!(tracks[0].duration_in_millis, Some(202_395));
        assert_eq!(tracks[0].has_lyrics, Some(true));
        assert_eq!(tracks[0].play_params_id.as_deref(), Some("1649434005"));
        assert!(tracks[0].url.is_some());
        assert!(tracks[0].preview_url.is_some());
        assert_eq!(tracks[0].genre_names, vec!["Pop", "Music"]);

        // Track 2: minimal fields (no new fields present in API response)
        assert_eq!(tracks[1].song_id, "1649434006");
        assert_eq!(tracks[1].track_number, 2);
        assert!(tracks[1].content_rating.is_none());
        assert!(tracks[1].is_apple_digital_master.is_none());
        assert!(tracks[1].release_date.is_none());
        assert!(tracks[1].composer_name.is_none());
        assert!(tracks[1].duration_in_millis.is_none());
        assert!(tracks[1].has_lyrics.is_none());
        assert!(tracks[1].play_params_id.is_none());
        assert!(tracks[1].url.is_none());
        assert!(tracks[1].preview_url.is_none());
        assert!(tracks[1].genre_names.is_empty());
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

    // ----------------------------------------------------------
    // Music video URL construction tests
    // ----------------------------------------------------------

    #[test]
    fn build_music_video_url_us_storefront() {
        let url = build_music_video_url("us", "1649434280");
        assert_eq!(url, "https://music.apple.com/us/music-video/mv/1649434280");
    }

    #[test]
    fn build_music_video_url_gb_storefront() {
        let url = build_music_video_url("gb", "9876543210");
        assert_eq!(url, "https://music.apple.com/gb/music-video/mv/9876543210");
    }

    // ----------------------------------------------------------
    // Music video relation serialization tests
    // ----------------------------------------------------------

    #[test]
    fn music_video_relation_serializes_correctly() {
        let relation = MusicVideoRelation {
            song_id: "1649434005".to_string(),
            music_video_id: "1649500001".to_string(),
            name: Some("Lavender Haze".to_string()),
        };

        let json = serde_json::to_string(&relation).unwrap();
        assert!(json.contains("\"song_id\":\"1649434005\""));
        assert!(json.contains("\"music_video_id\":\"1649500001\""));
        assert!(json.contains("\"name\":\"Lavender Haze\""));
    }

    #[test]
    fn music_video_relation_with_none_name() {
        let relation = MusicVideoRelation {
            song_id: "12345".to_string(),
            music_video_id: "67890".to_string(),
            name: None,
        };

        let json = serde_json::to_string(&relation).unwrap();
        assert!(json.contains("\"name\":null"));
    }

    // ----------------------------------------------------------
    // Non-geographic URL normalization tests
    // ----------------------------------------------------------

    #[test]
    fn normalize_url_with_storefront_unchanged() {
        let url = "https://music.apple.com/us/album/midnights/1649434004";
        assert_eq!(normalize_apple_music_url(url), url);
    }

    #[test]
    fn normalize_url_with_non_us_storefront_unchanged() {
        let url = "https://music.apple.com/gb/album/anti-hero/1649434004?i=1649434280";
        assert_eq!(normalize_apple_music_url(url), url);
    }

    #[test]
    fn normalize_album_url_without_storefront() {
        let url = "https://music.apple.com/album/midnights/1649434004";
        let result = normalize_apple_music_url(url);
        // Should have a 2-letter storefront injected between domain and /album/
        assert!(result.contains("/album/midnights/1649434004"));
        assert_ne!(result, url); // Should have changed
                                 // Verify structural correctness: domain/{2-letter-code}/album/...
        let after_domain = result
            .strip_prefix("https://music.apple.com/")
            .expect("should start with domain");
        let first_segment: &str = after_domain.split('/').next().unwrap();
        assert_eq!(first_segment.len(), 2, "storefront should be 2 chars");
        assert!(
            first_segment.chars().all(|c| c.is_ascii_lowercase()),
            "storefront should be lowercase ascii"
        );
    }

    #[test]
    fn normalize_song_url_without_storefront() {
        let url = "https://music.apple.com/song/anti-hero/1649434280";
        let result = normalize_apple_music_url(url);
        assert!(result.contains("/song/anti-hero/1649434280"));
        assert_ne!(result, url);
    }

    #[test]
    fn normalize_album_with_track_without_storefront() {
        let url = "https://music.apple.com/album/midnights/1649434004?i=1649434280";
        let result = normalize_apple_music_url(url);
        assert!(result.contains("/album/midnights/1649434004?i=1649434280"));
        assert_ne!(result, url);
    }

    #[test]
    fn normalize_playlist_url_without_storefront() {
        let url =
            "https://music.apple.com/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb";
        let result = normalize_apple_music_url(url);
        assert!(result.contains("/playlist/todays-hits/"));
        assert_ne!(result, url);
    }

    #[test]
    fn normalize_music_video_url_without_storefront() {
        let url = "https://music.apple.com/music-video/some-video/1234567890";
        let result = normalize_apple_music_url(url);
        assert!(result.contains("/music-video/some-video/1234567890"));
        assert_ne!(result, url);
    }

    #[test]
    fn normalize_artist_url_without_storefront() {
        let url = "https://music.apple.com/artist/taylor-swift/159260351";
        let result = normalize_apple_music_url(url);
        assert!(result.contains("/artist/taylor-swift/159260351"));
        assert_ne!(result, url);
    }

    #[test]
    fn normalize_classical_url_without_storefront() {
        let url = "https://classical.apple.com/album/beethoven-symphony/1234567890";
        let result = normalize_apple_music_url(url);
        assert!(result.starts_with("https://classical.apple.com/"));
        assert!(result.contains("/album/beethoven-symphony/1234567890"));
        assert_ne!(result, url);
    }

    #[test]
    fn normalize_itunes_url_without_storefront() {
        let url = "https://itunes.apple.com/album/some-album/1234567890";
        let result = normalize_apple_music_url(url);
        assert!(result.starts_with("https://itunes.apple.com/"));
        assert!(result.contains("/album/some-album/1234567890"));
        assert_ne!(result, url);
    }

    #[test]
    fn normalize_non_apple_music_url_unchanged() {
        let url = "https://www.example.com/some/path";
        assert_eq!(normalize_apple_music_url(url), url);
    }

    #[test]
    fn normalize_empty_string_unchanged() {
        assert_eq!(normalize_apple_music_url(""), "");
    }

    #[test]
    fn normalize_youtube_url_unchanged() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        assert_eq!(normalize_apple_music_url(url), url);
    }

    #[test]
    fn normalized_album_url_is_parseable() {
        // After normalization, the URL should be parseable by parse_apple_music_url
        let url = "https://music.apple.com/album/midnights/1649434004";
        let normalized = normalize_apple_music_url(url);
        let parsed = parse_apple_music_url(&normalized);
        assert!(parsed.is_some(), "Normalized URL should be parseable");
        let parsed = parsed.unwrap();
        assert_eq!(parsed.content_type, "album");
        assert_eq!(parsed.album_id, "1649434004");
        assert_eq!(parsed.storefront.len(), 2);
    }

    #[test]
    fn normalized_song_url_is_parseable() {
        let url = "https://music.apple.com/song/anti-hero/1649434280";
        let normalized = normalize_apple_music_url(url);
        let parsed = parse_apple_music_url(&normalized);
        assert!(parsed.is_some(), "Normalized song URL should be parseable");
        let parsed = parsed.unwrap();
        assert_eq!(parsed.content_type, "song");
        assert_eq!(parsed.song_id.as_deref(), Some("1649434280"));
    }

    #[test]
    fn normalized_music_video_url_is_parseable() {
        let url = "https://music.apple.com/music-video/some-video/1234567890";
        let normalized = normalize_apple_music_url(url);
        let parsed = parse_apple_music_url(&normalized);
        assert!(
            parsed.is_some(),
            "Normalized music-video URL should be parseable"
        );
        let parsed = parsed.unwrap();
        assert_eq!(parsed.content_type, "music-video");
        assert_eq!(parsed.album_id, "1234567890");
    }

    #[test]
    fn detect_non_geographic_returns_none_for_geographic_url() {
        let url = "https://music.apple.com/us/album/midnights/1649434004";
        // This URL has a storefront, but since "us" is only 2 chars and the
        // regex looks for content-type keywords, it won't match as non-geographic.
        // The normalize function handles this by checking parse_apple_music_url first.
        // detect_non_geographic_url sees "us" as a 2-letter segment, not a keyword.
        assert!(detect_non_geographic_url(url).is_none());
    }

    #[test]
    fn detect_non_geographic_returns_some_for_album_url() {
        let url = "https://music.apple.com/album/midnights/1649434004";
        let result = detect_non_geographic_url(url);
        assert!(result.is_some());
        let (base, rest) = result.unwrap();
        assert_eq!(base, "https://music.apple.com");
        assert_eq!(rest, "/album/midnights/1649434004");
    }

    #[test]
    fn resolve_storefront_sync_returns_valid_code() {
        let sf = resolve_storefront_sync();
        assert_eq!(sf.len(), 2);
        assert!(sf.chars().all(|c| c.is_ascii_lowercase()));
    }

    // ----------------------------------------------------------
    // extract_media_user_token tests
    // ----------------------------------------------------------

    #[test]
    fn extract_token_from_valid_cookies_file() {
        let dir = std::env::temp_dir().join("meedyadl_test_cookies_valid");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cookies.txt");
        std::fs::write(
            &path,
            "# Netscape HTTP Cookie File\n\
             .apple.com\tTRUE\t/\tTRUE\t0\tmedia-user-token\tABCDEF123456\n",
        )
        .unwrap();
        let result = extract_media_user_token(path.to_str().unwrap());
        assert_eq!(result.unwrap(), Some("ABCDEF123456".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extract_token_returns_none_when_missing() {
        let dir = std::env::temp_dir().join("meedyadl_test_cookies_missing");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cookies.txt");
        std::fs::write(
            &path,
            "# Netscape HTTP Cookie File\n\
             .apple.com\tTRUE\t/\tTRUE\t0\tother-cookie\tvalue123\n",
        )
        .unwrap();
        let result = extract_media_user_token(path.to_str().unwrap());
        assert_eq!(result.unwrap(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extract_token_returns_none_for_expired_cookie() {
        let dir = std::env::temp_dir().join("meedyadl_test_cookies_expired");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cookies.txt");
        // Expiry in the past (epoch 1000)
        std::fs::write(
            &path,
            "# Netscape HTTP Cookie File\n\
             .apple.com\tTRUE\t/\tTRUE\t1000\tmedia-user-token\tEXPIRED_TOKEN\n",
        )
        .unwrap();
        let result = extract_media_user_token(path.to_str().unwrap());
        assert_eq!(result.unwrap(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extract_token_accepts_session_cookie_zero_expiry() {
        let dir = std::env::temp_dir().join("meedyadl_test_cookies_session");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cookies.txt");
        // Expiry of 0 = session cookie, should be accepted
        std::fs::write(
            &path,
            "# Netscape HTTP Cookie File\n\
             .apple.com\tTRUE\t/\tTRUE\t0\tmedia-user-token\tSESSION_TOKEN\n",
        )
        .unwrap();
        let result = extract_media_user_token(path.to_str().unwrap());
        assert_eq!(result.unwrap(), Some("SESSION_TOKEN".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extract_token_skips_empty_value() {
        let dir = std::env::temp_dir().join("meedyadl_test_cookies_empty");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cookies.txt");
        std::fs::write(
            &path,
            "# Netscape HTTP Cookie File\n\
             .apple.com\tTRUE\t/\tTRUE\t0\tmedia-user-token\t\n",
        )
        .unwrap();
        let result = extract_media_user_token(path.to_str().unwrap());
        assert_eq!(result.unwrap(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extract_token_errors_on_missing_file() {
        let result = extract_media_user_token("/nonexistent/path/cookies.txt");
        assert!(result.is_err());
    }

    #[test]
    fn extract_token_skips_comments_and_blank_lines() {
        let dir = std::env::temp_dir().join("meedyadl_test_cookies_comments");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cookies.txt");
        std::fs::write(
            &path,
            "# Netscape HTTP Cookie File\n\
             # This is a comment\n\
             \n\
             .apple.com\tTRUE\t/\tTRUE\t0\tmedia-user-token\tTOKEN_AFTER_COMMENTS\n",
        )
        .unwrap();
        let result = extract_media_user_token(path.to_str().unwrap());
        assert_eq!(result.unwrap(), Some("TOKEN_AFTER_COMMENTS".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }
}
