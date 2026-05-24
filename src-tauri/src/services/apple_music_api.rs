// Copyright (c) 2026 MeedyaSuite
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
    /// Optional playlist identifier for playlist URLs (e.g., "pl.u-GgA567VCXX").
    /// Catalog playlist IDs start with `pl.`; library playlists are not parsed
    /// here (they require separate Music-User-Token auth).
    pub playlist_id: Option<String>,
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
    /// HLS M3U8 URL for 16:9 album-spotlight editorial video, if
    /// available (#538). Sourced from `editorialVideo` on the
    /// **album** endpoint (priority:
    /// `motionArtistFullscreen16x9` → `motionArtistWide16x9`), and
    /// destined for `AlbumSpotlightCover.mp4` in the album folder.
    /// Distinct from the artist-page spotlight that's saved as
    /// `ArtistSpotlightCover.mp4` — that one comes from the
    /// `/artists/{id}` endpoint and is shared across the artist's
    /// whole catalogue; this one is specific to the album.
    pub album_spotlight_url: Option<String>,
    /// Static cover-art URL template from `attributes.artwork.url`. Apple
    /// returns a templated URL with `{w}`, `{h}`, and `{f}` placeholders
    /// (e.g., `https://is1-ssl.mzstatic.com/.../source/{w}x{h}{c}.{f}`).
    /// Used by [`super::cover_art_fallback`] when GAMDL fails to write
    /// the static `Cover.<ext>` file (#756).
    pub artwork_url_template: Option<String>,
    /// Maximum native width of the static cover image (Apple typically
    /// serves up to 3000×3000 for modern releases).
    pub artwork_width: Option<u32>,
    /// Maximum native height of the static cover image.
    pub artwork_height: Option<u32>,
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
// iTunes Search/Lookup API (public, no auth required) (#454)
// ============================================================

/// Metadata from the iTunes Search/Lookup API for a single track.
///
/// The iTunes API returns a flat JSON structure with different field names
/// than the Apple Music API. This struct captures the iTunes-exclusive fields
/// that supplement Apple Music metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItunesTrackResult {
    /// iTunes track ID (same as Apple Music song ID / cnID atom)
    #[serde(rename = "trackId")]
    pub track_id: Option<u64>,
    /// Track name
    #[serde(rename = "trackName")]
    pub track_name: Option<String>,
    /// Artist name
    #[serde(rename = "artistName")]
    pub artist_name: Option<String>,
    /// Album name
    #[serde(rename = "collectionName")]
    pub collection_name: Option<String>,
    /// Primary genre as a single string (vs Apple Music's array)
    #[serde(rename = "primaryGenreName")]
    pub primary_genre_name: Option<String>,
    /// Track price in local currency
    #[serde(rename = "trackPrice")]
    pub track_price: Option<f64>,
    /// Album price in local currency
    #[serde(rename = "collectionPrice")]
    pub collection_price: Option<f64>,
    /// Currency code (e.g., "GBP", "USD")
    pub currency: Option<String>,
    /// Release country (e.g., "GBR", "USA")
    pub country: Option<String>,
    /// Track duration in milliseconds
    #[serde(rename = "trackTimeMillis")]
    pub track_time_millis: Option<u64>,
    /// Track number on disc
    #[serde(rename = "trackNumber")]
    pub track_number: Option<u32>,
    /// Disc number
    #[serde(rename = "discNumber")]
    pub disc_number: Option<u32>,
    /// Total disc count
    #[serde(rename = "discCount")]
    pub disc_count: Option<u32>,
    /// Track count in album
    #[serde(rename = "trackCount")]
    pub track_count: Option<u32>,
    /// ISRC code
    pub isrc: Option<String>,
    /// Content advisory (e.g., "explicit", "cleaned", "notExplicit")
    #[serde(rename = "trackExplicitness")]
    pub track_explicitness: Option<String>,
    /// Album-level advisory
    #[serde(rename = "collectionExplicitness")]
    pub collection_explicitness: Option<String>,
    /// Preview URL (30-second clip)
    #[serde(rename = "previewUrl")]
    pub preview_url: Option<String>,
    /// Track Apple Music URL
    #[serde(rename = "trackViewUrl")]
    pub track_view_url: Option<String>,
    /// Album Apple Music URL
    #[serde(rename = "collectionViewUrl")]
    pub collection_view_url: Option<String>,
    /// Media kind (e.g., "song", "music-video")
    pub kind: Option<String>,
    /// Wrapper type ("track", "collection", "artist")
    #[serde(rename = "wrapperType")]
    pub wrapper_type: Option<String>,
}

/// Response from the iTunes Lookup API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItunesLookupResponse {
    /// Number of results returned
    #[serde(rename = "resultCount")]
    pub result_count: u32,
    /// Array of results (first is the collection, rest are tracks)
    pub results: Vec<serde_json::Value>,
}

/// Fetch album + track metadata from the public iTunes Lookup API.
///
/// This API requires NO authentication — it's Apple's public search/lookup
/// endpoint. Returns track-level metadata that supplements the Apple Music
/// API response with fields like price, currency, country, and disc count.
///
/// # Arguments
/// * `album_id` - Apple Music album ID (numeric string, same as plID atom)
///
/// # Returns
/// * `Ok(Some(Vec<ItunesTrackResult>))` — Tracks found
/// * `Ok(None)` — Album not found or no track results
/// * `Err(String)` — Network or parse error
pub async fn fetch_itunes_lookup(
    album_id: &str,
) -> Result<Option<Vec<ItunesTrackResult>>, String> {
    let url = format!(
        "https://itunes.apple.com/lookup?id={album_id}&entity=song&limit=200"
    );

    log::debug!("Querying iTunes Lookup API: {url}");

    let client = crate::utils::http_client::build_simple(15)?;

    let response = client
        .get(&url)
        .header("User-Agent", "meedyadl")
        .send()
        .await
        .map_err(|e| format!("iTunes Lookup API request failed: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("iTunes Lookup API returned HTTP {status}"));
    }

    let body: ItunesLookupResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse iTunes Lookup response: {e}"))?;

    if body.result_count == 0 || body.results.is_empty() {
        log::debug!("iTunes Lookup: no results for album {album_id}");
        return Ok(None);
    }

    // Parse track results (skip the first entry if it's a "collection" wrapper)
    let tracks: Vec<ItunesTrackResult> = body
        .results
        .into_iter()
        .filter_map(|val| {
            // Only include "track" entries, not the "collection" header
            let wrapper = val.get("wrapperType")?.as_str()?;
            if wrapper == "track" {
                serde_json::from_value(val).ok()
            } else {
                None
            }
        })
        .collect();

    if tracks.is_empty() {
        log::debug!("iTunes Lookup: no track results for album {album_id}");
        return Ok(None);
    }

    log::info!(
        "iTunes Lookup: found {} track(s) for album {album_id}",
        tracks.len()
    );
    Ok(Some(tracks))
}

// ============================================================
// URL Parsing
// ============================================================

/// Parse an Apple Music URL to extract the storefront, content type, and IDs.
///
/// Supports four domains (`music.apple.com`, `classical.apple.com`,
/// `classical.music.apple.com`, and legacy `itunes.apple.com`) and two
/// path styles:
///
/// - **Slugged** (classic): `{domain}/{sf}/{type}/{slug}/{id}` — the
///   original music.apple.com share-link format.
/// - **Slug-less** (new Apple Music Classical): `{domain}/{sf}/{type}/{id}` —
///   Apple's native Classical app dropped the human-readable slug in
///   2026 when Classical migrated to the `classical.music.apple.com`
///   subdomain. The new Share links ship without a slug segment.
///
/// Examples:
/// - `https://music.apple.com/us/album/album-name/1234567890`
/// - `https://classical.apple.com/us/album/beethoven-symphony/1234567890`
/// - `https://classical.music.apple.com/gb/album/1844602145`
///   (new domain, no slug, no track ID)
/// - `https://classical.music.apple.com/gb/album/1844602145?i=9876543210`
///   (new domain, no slug, with track ID)
/// - `https://itunes.apple.com/us/album/album-name/1234567890`
/// - `https://music.apple.com/us/album/album-name/1234567890?i=9876543210`
/// - `https://music.apple.com/us/song/song-name/9876543210`
/// - `https://music.apple.com/us/music-video/video-name/1234567890`
///
/// All four domains share the same path structure (up to slug
/// presence) and are parsed identically. The `itunes` alternation was
/// added in #548 to close the same gap; the `classical\.music` branch
/// added here closes the same gap for Apple's domain migration.
/// Trailing query parameters beyond `?i=` (e.g. `?l=en-GB` locale
/// hints) are ignored — the regexes stop capturing at the album /
/// song / artist ID.
///
/// For album URLs with `?i=` query parameter, both the album ID and the
/// individual song ID are extracted.
///
/// # Panics
///
/// Panics if the hardcoded regex patterns are invalid (should never happen).
///
/// Returns `true` if the URL is an Apple Music **personal library**
/// URL (`/library/...` path segment), not a public catalog URL.
///
/// Library URLs route through GAMDL's `/v1/me/library/...` endpoints
/// (Music-User-Token-bound) and refer to items in the signed-in user's
/// own library — including content that may not exist in the public
/// catalog at all (e.g., user-uploaded MP3s, region-restricted items).
///
/// MeedyaDL skips catalog API enrichment (iTunes Lookup + Apple Music
/// Catalog + syllable-lyrics + animated artwork + music-video relations)
/// for library items because the catalog APIs have nothing to return
/// for personal uploads — 404s pile up noisily in the activity log
/// without contributing useful metadata.
///
/// AcoustID fingerprinting + ReplayGain still run normally (they work
/// on any audio file regardless of provenance).
///
/// GAMDL v3.7 extended its library URL regex to recognise
/// `/library/{albums,playlists,songs,music-videos}/` with
/// `{p.,l.,i.}*` ID prefixes. This helper matches the broader
/// substring shape — any URL containing `/library/` is treated as
/// library — so it stays correct as GAMDL's regex evolves further.
///
/// # Examples
/// ```
/// # use meedyadl::services::apple_music_api::is_library_url;
/// // Personal library URLs:
/// assert!(is_library_url("https://music.apple.com/us/library/albums/l.foo123"));
/// assert!(is_library_url("https://music.apple.com/gb/library/songs/i.bar456"));
/// assert!(is_library_url("https://music.apple.com/library/music-videos/i.mv789"));
/// assert!(is_library_url("https://music.apple.com/us/library/playlists/p.qux"));
///
/// // Public catalog URLs — NOT library:
/// assert!(!is_library_url("https://music.apple.com/us/album/abbey-road/1441164426"));
/// assert!(!is_library_url("https://music.apple.com/gb/playlist/.../pl.abc"));
/// ```
///
/// (#871 — part of #867 GAMDL v3.7 EPIC)
#[must_use]
pub fn is_library_url(url: &str) -> bool {
    url.contains("/library/")
}

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

    // Domain alternation + optional slug segment.
    //
    // Domain alternation `(?:classical(?:\.music)?|music|itunes)\.apple\.com`
    // matches four hostnames: `music.apple.com`, `classical.apple.com`,
    // `classical.music.apple.com`, `itunes.apple.com`. The
    // `classical(?:\.music)?` construct tries `classical.music` first
    // (greedy), falls back to plain `classical` when the next chars
    // aren't `.music` — so both Classical hostnames parse cleanly.
    //
    // Optional slug segment `(?:[^/]+/)?` matches both the classic
    // `/album/slug/id` format and the new Apple Music Classical
    // slug-less `/album/id` format. See function doc for the URL-shape
    // migration that drove this change.
    //
    // The `itunes` alternation closes the gap where iTunes URLs passed
    // host validation but failed every parser branch (#548); the
    // `classical\.music` branch closes the equivalent gap for
    // Apple's Classical domain migration.

    // Match album URLs: /storefront/album/[slug/]album_id with optional ?i=song_id
    static ALBUM_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"https?://(?:classical(?:\.music)?|music|itunes)\.apple\.com/([a-z]{2})/album/(?:[^/]+/)?(\d+)(?:\?i=(\d+))?",
        )
        .expect("Invalid album regex")
    });

    // Match song URLs: /storefront/song/[slug/]song_id
    static SONG_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"https?://(?:classical(?:\.music)?|music|itunes)\.apple\.com/([a-z]{2})/song/(?:[^/]+/)?(\d+)",
        )
        .expect("Invalid song regex")
    });

    // Match music-video URLs: /storefront/music-video/[slug/]video_id
    static MV_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"https?://(?:classical(?:\.music)?|music|itunes)\.apple\.com/([a-z]{2})/music-video/(?:[^/]+/)?(\d+)",
        )
        .expect("Invalid music-video regex")
    });

    // Match artist URLs: /storefront/artist/[slug/]artist_id
    static ARTIST_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"https?://(?:classical(?:\.music)?|music|itunes)\.apple\.com/([a-z]{2})/artist/(?:[^/]+/)?(\d+)",
        )
        .expect("Invalid artist regex")
    });

    // Match catalog playlist URLs: /storefront/playlist/[slug/]pl.xxxxx
    // Library playlists (/library/playlist/...) are intentionally NOT
    // matched here — they need a different endpoint and Music-User-Token
    // auth. Callers that need to detect library playlists should do so
    // via a separate check before calling this parser.
    static PLAYLIST_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"https?://(?:classical(?:\.music)?|music|itunes)\.apple\.com/([a-z]{2})/playlist/(?:[^/]+/)?(pl\.[a-zA-Z0-9._-]+)",
        )
        .expect("Invalid playlist regex")
    });

    if let Some(caps) = ALBUM_RE.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "album".to_string(),
            album_id: caps[2].to_string(),
            song_id: caps.get(3).map(|m| m.as_str().to_string()),
            artist_id: None,
            playlist_id: None,
        });
    }

    if let Some(caps) = SONG_RE.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "song".to_string(),
            album_id: String::new(),
            song_id: Some(caps[2].to_string()),
            artist_id: None,
            playlist_id: None,
        });
    }

    if let Some(caps) = MV_RE.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "music-video".to_string(),
            album_id: caps[2].to_string(),
            song_id: None,
            artist_id: None,
            playlist_id: None,
        });
    }

    if let Some(caps) = ARTIST_RE.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "artist".to_string(),
            album_id: String::new(),
            song_id: None,
            artist_id: Some(caps[2].to_string()),
            playlist_id: None,
        });
    }

    if let Some(caps) = PLAYLIST_RE.captures(url) {
        return Some(ParsedAppleMusicUrl {
            storefront: caps[1].to_string(),
            content_type: "playlist".to_string(),
            album_id: String::new(),
            song_id: None,
            artist_id: None,
            playlist_id: Some(caps[2].to_string()),
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

    // Build the JWT claims: issuer (team ID), issued-at, expiry, and
    // audience. The `aud` claim is required by Apple's MusicKit API (#161).
    // Without it, the API may return 401 even with valid credentials.
    let claims = serde_json::json!({
        "iss": team_id,
        "iat": now,
        "exp": now + 3600,  // 1 hour from now
        "aud": "https://music.apple.com",
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

    let client = crate::utils::http_client::build_simple(30)?;
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

    // #538: Album-level 16:9 editorial spotlight video. The same
    // `editorialVideo` block can carry both portrait (`motionDetailTall`)
    // and wide (`motionArtistFullscreen16x9` / `motionArtistWide16x9`)
    // variants; the wide variants are album-cinematic teasers
    // distinct from the static album cover. Priority matches the
    // artist-spotlight code path (#455 / #538): prefer
    // `motionArtistFullscreen16x9`, fall back to
    // `motionArtistWide16x9`. Lower-tier `motionDetail*` keys are
    // already covered by `artwork_square_url`/`artwork_tall_url`
    // and intentionally excluded — they're tightly cropped around
    // the cover and would look wrong as a 16:9 spotlight.
    let album_spotlight_url = editorial_video
        .and_then(|ev| {
            ev.get("motionArtistFullscreen16x9")
                .or_else(|| ev.get("motionArtistWide16x9"))
        })
        .and_then(|m| m.get("video"))
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    // Static cover artwork (#756). The `artwork.url` is a template
    // with `{w}`, `{h}`, `{f}` placeholders we can substitute when
    // falling back from a failed RAW write to PNG/JPEG.
    let artwork_obj = attributes.get("artwork");
    let artwork_url_template = artwork_obj
        .and_then(|a| a.get("url"))
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);
    let artwork_width = artwork_obj
        .and_then(|a| a.get("width"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());
    let artwork_height = artwork_obj
        .and_then(|a| a.get("height"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|v| u32::try_from(v).ok());

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
        album_spotlight_url,
        artwork_url_template,
        artwork_width,
        artwork_height,
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

    let client = crate::utils::http_client::build_simple(30)?;
    let mut relations = Vec::new();

    // Batch song IDs into groups of 100 (Apple Music API limit per request).
    //
    // We pass BOTH `include=music-videos` AND `relate=music-videos`:
    // - `include` puts the related resources in a top-level `included[]`
    //   array with full `attributes` (this is where `name` lives).
    // - `relate` keeps the inline `relationships.music-videos.data[]`
    //   structure populated, which is what we use to associate each
    //   song_id with its MV ids. We use it as a defensive fallback for
    //   the (rare) case where Apple omits `included[]`.
    //
    // Without `include`, `relate` alone returns only `id` + `type` on
    // the inline references — so the per-MV `attributes.name` lookup
    // returns `None` for every entry and the activity log says
    // "Downloading music video: unknown" (#775).
    for chunk in song_ids.chunks(100) {
        let ids_param = chunk.join(",");
        let url = format!(
            "https://api.music.apple.com/v1/catalog/{storefront}/songs?ids={ids_param}&include=music-videos&relate=music-videos"
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

        // Build a side-table of MV ID → name from the top-level
        // `included[]` array. JSON:API style: each entry has its own
        // `id`, `type`, and full `attributes` block.
        let included_names = build_included_name_lookup(&json);

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

                        // Prefer the name from `included[]` (the only
                        // place full attributes are guaranteed to live
                        // when `include=music-videos` is set). Fall
                        // back to the inline `attributes.name` so we
                        // still recover something if Apple sends a
                        // sparse response.
                        let name = included_names
                            .get(music_video_id.as_str())
                            .cloned()
                            .or_else(|| {
                                mv.get("attributes")
                                    .and_then(|a| a.get("name"))
                                    .and_then(|v| v.as_str())
                                    .map(std::string::ToString::to_string)
                            });

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

/// Builds a `MV id → name` lookup table from the top-level `included[]`
/// array of an Apple Music JSON:API response.
///
/// Returns an empty map if `included[]` is absent or contains no
/// `music-videos` entries with a `name` attribute. Callers should treat
/// a missing entry as "fall back to inline relationship data."
fn build_included_name_lookup(
    json: &serde_json::Value,
) -> std::collections::HashMap<String, String> {
    let mut lookup = std::collections::HashMap::new();

    let Some(included) = json.get("included").and_then(|v| v.as_array()) else {
        return lookup;
    };

    for entry in included {
        // Restrict to music-videos so we don't accidentally pick up the
        // name of a different included resource type that happens to
        // share an ID space.
        if entry.get("type").and_then(|t| t.as_str()) != Some("music-videos") {
            continue;
        }

        let Some(id) = entry.get("id").and_then(|v| v.as_str()) else {
            continue;
        };

        let Some(name) = entry
            .get("attributes")
            .and_then(|a| a.get("name"))
            .and_then(|v| v.as_str())
        else {
            continue;
        };

        lookup.insert(id.to_string(), name.to_string());
    }

    lookup
}

/// Album linkage for a music video, returned by [`fetch_music_video_album_linkage`].
///
/// Carries just the fields needed to route the MV into the linked
/// album's existing folder on disk: artist name + album name +
/// release date. **Not** a full `AlbumMetadata` — we don't need the
/// editorial-video URLs, audio traits, or 20-odd other fields for the
/// folder-resolution use case, and fetching them would add latency
/// without UX benefit (Tier 2's whole point is the linked-folder
/// hint; if the user wants enrichment, the album download itself
/// handles that).
#[derive(Debug, Clone)]
pub struct MusicVideoAlbumLinkage {
    /// Canonical album ID from Apple Music.
    pub album_id: String,
    /// Album name as Apple lists it (used for `{album}` substitution).
    pub album_name: String,
    /// Artist name from the album record (used for `{album_artist}`).
    pub artist_name: String,
    /// Release date (ISO date) if available. Used by templates that
    /// substitute `{release_year}` etc.
    pub release_date: Option<String>,
}

/// **MV filename-resolution Tier 2** (#558): fetch the music video's
/// album linkage from the Apple Music Catalog API.
///
/// Endpoint: `GET .../music-videos/{id}?include=albums`. The response
/// embeds the linked album(s) under
/// `relationships.albums.data[]` (id only) and the full attributes
/// under top-level `included[]` (JSON:API style). Most MVs link to
/// exactly one album; the first entry is canonical for our purposes.
///
/// Fail-soft contract per the issue spec:
///   - 404 (MV has no album linkage) → `Ok(None)` so callers can
///     fall through to Tier 3 / Tier 4.
///   - 401 / 403 (auth) → `Ok(None)` with a warn, same fall-through.
///   - Network error → bubbled up as `Err` so the caller decides
///     whether to retry; current callers just log + fall through.
///
/// Token comes from [`resolve_premium_feature_token`] (same path as
/// animated artwork / syllable lyrics) so this gracefully degrades
/// when MusicKit credentials are missing.
pub async fn fetch_music_video_album_linkage(
    jwt: &str,
    storefront: &str,
    video_id: &str,
) -> Result<Option<MusicVideoAlbumLinkage>, String> {
    let url = format!(
        "https://api.music.apple.com/v1/catalog/{storefront}/music-videos/{video_id}?include=albums"
    );
    log::debug!("Fetching MV album linkage (Tier 2): {url}");

    let client = crate::utils::http_client::build_simple(15)?;
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {jwt}"))
        .header("User-Agent", "meedyadl")
        .send()
        .await
        .map_err(|e| format!("MV album linkage lookup failed: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        match status.as_u16() {
            404 => {
                log::debug!("MV {video_id} has no album linkage (404) — falling through to Tier 3/4");
                return Ok(None);
            }
            401 | 403 => {
                log::warn!(
                    "MV album linkage lookup auth failed ({status}) — falling through to Tier 3/4"
                );
                return Ok(None);
            }
            _ => {
                return Err(format!(
                    "Apple Music API returned HTTP {status} for MV album linkage"
                ));
            }
        }
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse MV album linkage response: {e}"))?;

    parse_mv_album_linkage(&json)
}

/// Extracted parser so unit tests can exercise the shape without
/// needing live API access. Returns the first linked album when
/// `relationships.albums.data[]` is non-empty AND the corresponding
/// entry in `included[]` is resolvable; otherwise `None`.
fn parse_mv_album_linkage(json: &serde_json::Value) -> Result<Option<MusicVideoAlbumLinkage>, String> {
    // Locate the MV record. `/music-videos/{id}` returns a single
    // `data` object (not an array). Defensive: handle both shapes.
    let mv = json
        .get("data")
        .and_then(|d| {
            if d.is_array() {
                d.as_array().and_then(|arr| arr.first())
            } else {
                Some(d)
            }
        });
    let Some(mv) = mv else { return Ok(None); };

    let album_data = mv
        .get("relationships")
        .and_then(|r| r.get("albums"))
        .and_then(|a| a.get("data"))
        .and_then(|d| d.as_array());

    let Some(album_data) = album_data else { return Ok(None); };
    let Some(first_album_ref) = album_data.first() else {
        return Ok(None);
    };
    let Some(album_id) = first_album_ref.get("id").and_then(|v| v.as_str()) else {
        return Ok(None);
    };

    // Look up the full album attributes from `included[]`.
    let included = json.get("included").and_then(|i| i.as_array());
    let Some(included) = included else { return Ok(None); };

    for entry in included {
        if entry.get("type").and_then(|t| t.as_str()) != Some("albums") {
            continue;
        }
        if entry.get("id").and_then(|v| v.as_str()) != Some(album_id) {
            continue;
        }
        let attrs = match entry.get("attributes") {
            Some(a) => a,
            None => continue,
        };
        let album_name = attrs
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let artist_name = attrs
            .get("artistName")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let release_date = attrs
            .get("releaseDate")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Defensive: a linkage with both name and artist empty is
        // useless for folder routing — treat as a miss.
        if album_name.is_empty() && artist_name.is_empty() {
            return Ok(None);
        }

        return Ok(Some(MusicVideoAlbumLinkage {
            album_id: album_id.to_string(),
            album_name,
            artist_name,
            release_date,
        }));
    }

    Ok(None)
}

#[cfg(test)]
mod mv_album_linkage_tests {
    use super::*;

    #[test]
    fn parses_canonical_response_shape() {
        let json = serde_json::json!({
            "data": {
                "id": "1639963816",
                "type": "music-videos",
                "relationships": {
                    "albums": {
                        "data": [
                            { "id": "1639963810", "type": "albums" }
                        ]
                    }
                }
            },
            "included": [
                {
                    "id": "1639963810",
                    "type": "albums",
                    "attributes": {
                        "name": "Psycho - Single",
                        "artistName": "Anne-Marie",
                        "releaseDate": "2022-08-26"
                    }
                }
            ]
        });
        let linkage = parse_mv_album_linkage(&json).unwrap().unwrap();
        assert_eq!(linkage.album_id, "1639963810");
        assert_eq!(linkage.album_name, "Psycho - Single");
        assert_eq!(linkage.artist_name, "Anne-Marie");
        assert_eq!(linkage.release_date.as_deref(), Some("2022-08-26"));
    }

    #[test]
    fn returns_none_when_no_album_relationship() {
        let json = serde_json::json!({
            "data": {
                "id": "999",
                "type": "music-videos",
                "relationships": {}
            }
        });
        assert!(parse_mv_album_linkage(&json).unwrap().is_none());
    }

    #[test]
    fn returns_none_when_album_data_array_empty() {
        let json = serde_json::json!({
            "data": {
                "id": "999",
                "type": "music-videos",
                "relationships": {
                    "albums": { "data": [] }
                }
            }
        });
        assert!(parse_mv_album_linkage(&json).unwrap().is_none());
    }

    #[test]
    fn returns_none_when_included_missing_album_attrs() {
        // relationships.albums.data references id "X" but included[]
        // doesn't carry the matching record. Treat as a miss rather
        // than reporting an empty linkage.
        let json = serde_json::json!({
            "data": {
                "id": "999",
                "type": "music-videos",
                "relationships": {
                    "albums": {
                        "data": [{ "id": "X", "type": "albums" }]
                    }
                }
            },
            "included": []
        });
        assert!(parse_mv_album_linkage(&json).unwrap().is_none());
    }

    #[test]
    fn returns_none_when_album_attrs_empty_strings() {
        // Defensive: an album record with both name and artist empty
        // is useless for folder routing — Tier 2 should miss so
        // Tier 3/4 can fire.
        let json = serde_json::json!({
            "data": {
                "id": "999",
                "relationships": {
                    "albums": {
                        "data": [{ "id": "X", "type": "albums" }]
                    }
                }
            },
            "included": [
                { "id": "X", "type": "albums", "attributes": { "name": "", "artistName": "" } }
            ]
        });
        assert!(parse_mv_album_linkage(&json).unwrap().is_none());
    }

    #[test]
    fn picks_first_album_when_multiple_linked() {
        // Edge case: an MV linked to multiple albums (e.g. compilation
        // + original single). The first entry is canonical per spec.
        let json = serde_json::json!({
            "data": {
                "id": "999",
                "relationships": {
                    "albums": {
                        "data": [
                            { "id": "FIRST", "type": "albums" },
                            { "id": "SECOND", "type": "albums" }
                        ]
                    }
                }
            },
            "included": [
                { "id": "FIRST", "type": "albums", "attributes": { "name": "Original", "artistName": "A" } },
                { "id": "SECOND", "type": "albums", "attributes": { "name": "Compilation", "artistName": "A" } }
            ]
        });
        let linkage = parse_mv_album_linkage(&json).unwrap().unwrap();
        assert_eq!(linkage.album_id, "FIRST");
        assert_eq!(linkage.album_name, "Original");
    }
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
/// Also supports `classical.apple.com`, `classical.music.apple.com`,
/// and legacy `itunes.apple.com` domains.
///
/// # Arguments
/// * `url` - The Apple Music URL to normalize
///
/// # Returns
/// The URL with a storefront code guaranteed to be present. Non-Apple-Music
/// URLs are returned unchanged.
#[must_use]
pub fn normalize_apple_music_url(url: &str) -> String {
    // First pass: rewrite legacy iTunes-domain URLs to music.apple.com (#568).
    //
    // GAMDL 2.9.3+ rejects `itunes.apple.com` URLs outright with a
    // "Could not parse … skipping" warning even though MeedyaDL's own
    // parser accepts them (#548 audit). Catalog IDs are shared across
    // both domains, so the rewrite is safe and silent.
    //
    // Two iTunes-specific quirks handled here:
    //   1. Hostname swap: `itunes.apple.com` → `music.apple.com`.
    //   2. `id`-prefix strip: iTunes URLs use `/album/id1567637891`
    //      where Apple Music expects `/album/1567637891` (digits only).
    //      Without the strip, the rewritten URL would still fail
    //      every parser branch downstream.
    //
    // The slug-less variant (`/album/id123` with no human-readable slug)
    // is already handled by parse_apple_music_url's `(?:[^/]+/)?`
    // optional-slug regex — no extra work needed here.
    //
    // Pass-through: iTunes URLs that don't match the recognised
    // `/<storefront>/<content-type>/<id-prefix?><digits>` shape (or the
    // non-geographic equivalent below) are NOT rewritten — they fall
    // through to #549's catch-all WARN ("Unrecognised Apple Music URL
    // shape"). Better than mangling a URL we don't understand.
    let url_owned = if let Some(rewritten) = rewrite_itunes_url(url) {
        log::info!("Rewrote legacy iTunes URL: {url} → {rewritten}");
        rewritten
    } else {
        url.to_string()
    };
    let url = url_owned.as_str();

    // Second pass: rewrite legacy `classical.apple.com` URLs to the
    // current `classical.music.apple.com` host (#880).
    //
    // GAMDL's URL parser (`gamdl/interface/constants.py`
    // `VALID_URL_PATTERN`) accepts `music.apple.com` with an OPTIONAL
    // `classical.` prefix — i.e., `classical.music.apple.com` ✓ but
    // **NOT** the bare legacy `classical.apple.com` host (which lacks
    // the `music.` segment). MeedyaDL's own parser is more permissive
    // and still accepts the legacy form, so users who paste a
    // `classical.apple.com` URL get past the frontend gate; the
    // download then fails inside GAMDL with "Could not parse URL".
    //
    // The catalog IDs are shared across the two hosts (Apple Music
    // Classical migrated the host but kept the IDs), so the rewrite
    // is silent and lossless. Identical pattern to the iTunes rewrite
    // above — see #568 / #880 for rationale.
    //
    // Gated behind `GamdlFeature::ClassicalMusicHostRequired` so we
    // never apply the rewrite on an unaudited GAMDL version. The gate
    // is `true` for the entire MeedyaDL support window (>= 2.9.1) but
    // `false` for unknown / out-of-window installs, which preserves
    // the pre-#880 pass-through behaviour in those edge cases.
    let url_owned = if super::gamdl_capabilities::supports(
        super::gamdl_capabilities::GamdlFeature::ClassicalMusicHostRequired,
    ) {
        if let Some(rewritten) = rewrite_classical_legacy_url(url) {
            log::info!(
                "Rewrote legacy classical.apple.com URL: {url} → {rewritten}"
            );
            rewritten
        } else {
            url.to_string()
        }
    } else {
        url.to_string()
    };
    let url = url_owned.as_str();

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

/// Reduce an Apple Music URL to its storefront-independent canonical
/// form (#807). Used for cross-source URL matching where the two
/// sides may carry different storefronts and/or different slugs but
/// refer to the same catalogue entity (most commonly when matching
/// MusicBrainz `external_urls.apple_music` against a downloaded
/// album's source URL).
///
/// ## Normalisation steps
///
/// 1. **Strip the storefront segment** (`/{2-letter-ISO}/`). Both
///    `https://music.apple.com/gb/album/.../123` and
///    `https://music.apple.com/us/album/123` reduce to a single
///    canonical shape with no storefront.
/// 2. **Strip the slug** between the type segment and the numeric ID.
///    `/album/super-bowl-lviii-megamix-dj-mix/1729264859` and
///    `/album/1729264859` both reduce to `/album/1729264859`.
///    Slugs are SEO furniture; the numeric ID is the canonical
///    identifier.
/// 3. **Strip the query string + fragment**. MusicBrainz never
///    carries `?l=en-GB` or `?i=…`; Apple Music sometimes does.
///
/// ## Return value
///
/// `Some(canonical)` when the URL parses as Apple Music with a
/// numeric ID we can extract; `None` for non-Apple-Music URLs,
/// library URLs (`l.XXXX` prefix), or shapes the parser doesn't
/// recognise. Returns the canonical form **without** scheme — pure
/// path semantics — so callers compare on the path itself rather
/// than recreating a `https://music.apple.com/…` envelope.
///
/// ## Example
///
/// ```ignore
/// canonicalise_apple_music_url(
///     "https://music.apple.com/gb/album/super-bowl-lviii-megamix-dj-mix/1729264859",
/// );
/// // → Some("music.apple.com/album/1729264859")
///
/// canonicalise_apple_music_url("https://music.apple.com/us/album/1729264859?i=171");
/// // → Some("music.apple.com/album/1729264859")
/// ```
#[must_use]
pub fn canonicalise_apple_music_url(url: &str) -> Option<String> {
    // Use the existing `parse_apple_music_url` to do the heavy
    // regex work — it already handles every URL shape we care about
    // (storefronted, non-storefronted, slug-or-no-slug, classical /
    // itunes / music subdomain), strips the query string, and
    // extracts the numeric ID + content type. Reusing it keeps the
    // canonicaliser aligned with the parser's evolving regex set —
    // when a new content-type lands the canonical form picks it up
    // for free.
    //
    // We deliberately first run `normalize_apple_music_url` to
    // collapse the iTunes-legacy + non-geographic variants into
    // their music.apple.com equivalents; that means canonicalisation
    // is idempotent on the result.
    let normalised = normalize_apple_music_url(url);
    let parsed = parse_apple_music_url(&normalised)?;

    // Library URLs (`l.XXXX`) carry a per-user identifier that's
    // not portable across users, so canonicalising them for cross-
    // source matching is meaningless — return None so the caller
    // falls through to the next tier (ISRC / AcoustID).
    if parsed.album_id.starts_with("l.") {
        return None;
    }

    // The canonical form omits the storefront and slug, keeps only
    // the content type + numeric ID. `parse_apple_music_url` has
    // already given us those two as parsed fields. We use
    // `album_id` as the canonical numeric ID even for non-album
    // shapes (song, music-video, artist) because the field name
    // is the historic shape — it carries the entity's numeric ID
    // regardless of content type.
    Some(format!(
        "music.apple.com/{}/{}",
        parsed.content_type, parsed.album_id
    ))
}

/// Rewrite an `itunes.apple.com` URL to its `music.apple.com` equivalent (#568).
///
/// Returns `Some(rewritten)` when:
///   - The hostname is `itunes.apple.com` (case-insensitive in scheme),
///     AND
///   - The path matches one of the known content-type shapes
///     (album, song, music-video, artist, playlist), with optional
///     storefront, optional slug, and an `id`-prefixed numeric ID
///     (or `pl.<token>` for playlists).
///
/// Returns `None` when the URL isn't on the iTunes domain, or when the
/// path shape doesn't match any recognised pattern (the catch-all WARN
/// in `start_download` is the right place to surface those).
///
/// Implementation: a single regex with anchored alternation captures
/// both the geographic (`/gb/album/…`) and non-geographic
/// (`/album/…`) variants in one pass; the non-geographic variant
/// is then re-routed through `detect_non_geographic_url` for
/// storefront injection.
#[must_use]
fn rewrite_itunes_url(url: &str) -> Option<String> {
    use std::sync::LazyLock;

    // Single regex captures: optional `/<storefront>` (group 1),
    // content-type (group 2), optional slug (group 3), and the
    // `id`-prefixed or playlist ID (group 4). We strip the leading
    // `id` from numeric IDs in the substitution; playlist IDs
    // (`pl.…`) pass through unchanged.
    static ITUNES_REWRITE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(https?://)itunes\.apple\.com(/[a-z]{2})?/(album|song|music-video|artist|playlist)/((?:[^/?]+/)?)((?:id)?\d+|pl\.[A-Za-z0-9]+)(\?[^#]*)?(#.*)?$",
        )
        .expect("Invalid iTunes-rewrite regex")
    });

    let caps = ITUNES_REWRITE_RE.captures(url)?;
    let scheme = caps.get(1).map_or("", |m| m.as_str());
    let storefront_segment = caps.get(2).map_or("", |m| m.as_str()); // includes leading /
    let content_type = caps.get(3).map_or("", |m| m.as_str());
    let slug_segment = caps.get(4).map_or("", |m| m.as_str()); // includes trailing /
    let id_token = caps.get(5).map_or("", |m| m.as_str());
    let query = caps.get(6).map_or("", |m| m.as_str());
    let fragment = caps.get(7).map_or("", |m| m.as_str());

    // Strip the legacy `id` prefix from numeric IDs. Playlist IDs
    // (`pl.…`) are left intact.
    let normalised_id = id_token
        .strip_prefix("id")
        .filter(|rest| rest.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(id_token);

    Some(format!(
        "{scheme}music.apple.com{storefront_segment}/{content_type}/{slug_segment}{normalised_id}{query}{fragment}"
    ))
}

/// Rewrite a legacy `classical.apple.com` URL to the current
/// `classical.music.apple.com` host (#880).
///
/// Apple Music Classical originally lived at `classical.apple.com` and
/// later moved to `classical.music.apple.com`. Both URL forms are
/// still in the wild (deep links from older builds, bookmarks, blog
/// posts) but GAMDL v3.7.1's `VALID_URL_PATTERN` only accepts the
/// new form. The legacy hostname → new hostname rewrite is silent
/// because the storefront, content type, slug, and numeric ID are all
/// preserved unchanged.
///
/// Returns `None` for URLs that don't carry the legacy `classical.apple.com`
/// hostname so the caller can use this helper as part of a chain of
/// normalisation passes without an explicit `is_legacy_classical` check
/// at the call site.
#[must_use]
fn rewrite_classical_legacy_url(url: &str) -> Option<String> {
    use std::sync::LazyLock;

    // Captures the optional scheme separator + the rest of the URL
    // after the legacy hostname. We don't dissect the path because
    // GAMDL parses the path itself once the hostname is correct;
    // anything that's syntactically valid on the legacy host is
    // syntactically valid on the new host.
    static CLASSICAL_LEGACY_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(https?://)classical\.apple\.com(/.*)?$")
            .expect("Invalid classical-legacy-rewrite regex")
    });

    let caps = CLASSICAL_LEGACY_RE.captures(url)?;
    let scheme = caps.get(1).map_or("https://", |m| m.as_str());
    let path = caps.get(2).map_or("", |m| m.as_str());
    Some(format!("{scheme}classical.music.apple.com{path}"))
}

/// Rewrite an Apple Music URL to use a different storefront code (#666).
///
/// Used by the storefront-fallback retry path: when a primary download fails
/// because the URL's storefront returned `Resource Not Found` from the AMP
/// API (or an authenticated licence acquisition refused), MeedyaDL retries
/// the same content via the user's account-region storefront.
///
/// Preserves: host (`music`/`classical(.music)?`/`itunes` variants), path
/// segments after the storefront (slug + numeric/`pl.` ID), and the `?i=…`
/// query that distinguishes a track inside an album.
///
/// Returns the input unchanged when the URL doesn't match the standard
/// `/<storefront>/<content-type>/…` shape (non-Apple-Music URLs,
/// `/library/…` URLs, novel paths) so this helper is always safe to call.
///
/// # Example
/// ```
/// # use meedyadl::services::apple_music_api::rewrite_url_storefront;
/// assert_eq!(
///     rewrite_url_storefront("https://music.apple.com/us/album/foo/123?i=456", "gb"),
///     "https://music.apple.com/gb/album/foo/123?i=456"
/// );
/// ```
#[must_use]
pub fn rewrite_url_storefront(url: &str, new_storefront: &str) -> String {
    use std::sync::LazyLock;

    // Match `(host)/(2-letter-storefront)/(content-type-keyword)` with the
    // storefront as a single capture so `replace` can substitute exactly the
    // 2-letter segment, leaving everything before and after intact (slug,
    // numeric ID, ?i=… query, anchor fragment, etc.).
    //
    // Hostnames mirror parse_apple_music_url's domain alternation:
    // `music.apple.com`, `classical.apple.com`, `classical.music.apple.com`,
    // `itunes.apple.com`. Content-type alternation keeps us scoped to known
    // shapes — we deliberately don't rewrite `/library/…` URLs because they
    // use a Music-User-Token-bound endpoint where storefront isn't a free
    // variable.
    static STOREFRONT_REWRITE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(https?://(?:classical(?:\.music)?|music|itunes)\.apple\.com)/[a-z]{2}/(album|song|music-video|artist|playlist)",
        )
        .expect("Invalid storefront-rewrite regex")
    });

    // Defensive: refuse to rewrite when the proposed storefront isn't a
    // valid 2-letter code. Returning the input unchanged keeps the caller's
    // happy path stable; the activity log + error category will surface the
    // original failure rather than an internally-mangled URL.
    let new_sf = new_storefront.to_ascii_lowercase();
    if new_sf.len() != 2 || !new_sf.chars().all(|c| c.is_ascii_alphabetic()) {
        return url.to_string();
    }

    if STOREFRONT_REWRITE_RE.is_match(url) {
        STOREFRONT_REWRITE_RE
            .replace(url, |caps: &regex::Captures| {
                format!("{}/{}/{}", &caps[1], new_sf, &caps[2])
            })
            .into_owned()
    } else {
        url.to_string()
    }
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
            r"^(https?://(?:classical(?:\.music)?|music|itunes)\.apple\.com)/(album|song|playlist|music-video|artist)(/.*)?$",
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
// Artist Albums (for pre-queue duplicate detection, #510)
// ============================================================

/// A lightweight summary of an album returned by the artist→albums endpoint.
///
/// Used by the duplicate-detection pipeline to enumerate every album that
/// belongs to an artist under a given auto-select mode (main-albums,
/// singles-eps, compilation-albums, live-albums). Tracks are fetched in a
/// second pass via [`fetch_album_metadata`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistAlbumRef {
    /// Numeric Apple Music album ID.
    pub album_id: String,
    /// Album title, if present in the catalog response.
    pub name: Option<String>,
    /// Whether the API marks this as a compilation.
    pub is_compilation: Option<bool>,
    /// Whether the API marks this as a single release.
    pub is_single: Option<bool>,
    /// `attributes.trackCount` — useful for heuristic EP vs album classification.
    pub track_count: Option<u32>,
    /// Release date (YYYY-MM-DD), used to order live-album detection heuristics.
    pub release_date: Option<String>,
}

/// Fetch every album for an artist from the Apple Music catalog API.
///
/// Paginates through `/v1/catalog/{storefront}/artists/{artist_id}/albums`
/// with `limit=100` (Apple's maximum) until the `next` cursor is absent.
/// Used by the duplicate-detection pipeline (#510) to enumerate albums
/// before fanning out one artist URL into per-mode queue items.
///
/// # Arguments
/// * `jwt` — MusicKit Developer Token (ES256-signed JWT)
/// * `storefront` — Two-letter storefront code (e.g., "us")
/// * `artist_id` — Numeric Apple Music artist ID
///
/// # Returns
/// * `Ok(Vec<ArtistAlbumRef>)` — Possibly empty list of albums
/// * `Err(String)` — HTTP / parse error from any page
pub async fn fetch_artist_albums(
    jwt: &str,
    storefront: &str,
    artist_id: &str,
) -> Result<Vec<ArtistAlbumRef>, String> {
    let client = crate::utils::http_client::build_simple(30)?;

    // Page cursor: Apple Music returns a relative `next` URL (e.g.,
    // "/v1/catalog/us/artists/123/albums?offset=100") that we append to the
    // host. Start with the initial page.
    let mut next_path: Option<String> = Some(format!(
        "/v1/catalog/{storefront}/artists/{artist_id}/albums?limit=100"
    ));
    let mut albums: Vec<ArtistAlbumRef> = Vec::new();

    // Safety cap: even hyper-prolific artists top out well below 50 pages (5,000
    // albums). Prevents a runaway loop if the API returns a circular `next`.
    const MAX_PAGES: u32 = 50;
    let mut page = 0u32;

    while let Some(path) = next_path.take() {
        if page >= MAX_PAGES {
            log::warn!(
                "fetch_artist_albums: hit max page cap ({MAX_PAGES}) for artist {artist_id} in storefront '{storefront}'"
            );
            break;
        }
        page += 1;

        let url = format!("https://api.music.apple.com{path}");
        log::debug!("Fetching artist albums page {page}: {url}");

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {jwt}"))
            .header("User-Agent", "meedyadl")
            .send()
            .await
            .map_err(|e| format!("Artist albums API request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(format!(
                "Apple Music API returned HTTP {status} for artist {artist_id} albums (page {page})"
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse artist albums response: {e}"))?;

        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            for item in data {
                let Some(album_id) = item.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                let attrs = item.get("attributes");
                let name = attrs
                    .and_then(|a| a.get("name"))
                    .and_then(|v| v.as_str())
                    .map(std::string::ToString::to_string);
                let is_compilation = attrs
                    .and_then(|a| a.get("isCompilation"))
                    .and_then(serde_json::Value::as_bool);
                let is_single = attrs
                    .and_then(|a| a.get("isSingle"))
                    .and_then(serde_json::Value::as_bool);
                let track_count = attrs
                    .and_then(|a| a.get("trackCount"))
                    .and_then(serde_json::Value::as_u64)
                    .and_then(|v| u32::try_from(v).ok());
                let release_date = attrs
                    .and_then(|a| a.get("releaseDate"))
                    .and_then(|v| v.as_str())
                    .map(std::string::ToString::to_string);

                albums.push(ArtistAlbumRef {
                    album_id: album_id.to_string(),
                    name,
                    is_compilation,
                    is_single,
                    track_count,
                    release_date,
                });
            }
        }

        // Continue if the API gave us a `next` cursor (relative path).
        next_path = json
            .get("next")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);
    }

    log::info!(
        "fetch_artist_albums: collected {} album(s) for artist {artist_id} in storefront '{storefront}' across {page} page(s)",
        albums.len()
    );

    Ok(albums)
}

/// Classify an album into the GAMDL artist-auto-select bucket that would
/// have downloaded it. Uses the same classification GAMDL itself does when
/// applying `--artist-auto-select` (best-effort — we don't have GAMDL's
/// exact source code reference here, but the API fields are the same
/// signals Apple Music's web client uses).
///
/// - `compilation-albums`: `isCompilation == Some(true)`
/// - `singles-eps`: `isSingle == Some(true)` OR `track_count <= 6`
/// - `live-albums`: album name heuristics — contains "(Live", "[Live",
///   ": Live", or ends with " (Live)" / " [Live]" (case-insensitive).
///   Apple doesn't expose a structured `isLive` flag.
/// - `main-albums`: everything else (the canonical full studio album).
///
/// Returns `None` for modes that this classifier can't resolve from album
/// metadata (e.g., top-songs — that's a catalog relation, not an album
/// flag).
#[must_use]
pub fn classify_album_bucket(
    album: &ArtistAlbumRef,
) -> crate::models::gamdl_options::ArtistAutoSelect {
    use crate::models::gamdl_options::ArtistAutoSelect;

    if album.is_compilation == Some(true) {
        return ArtistAutoSelect::CompilationAlbums;
    }

    if let Some(name) = album.name.as_deref() {
        let lower = name.to_lowercase();
        // Match "live" as a standalone token inside parens/brackets or
        // a trailing " - live" / ": live" suffix. Avoids false-positives
        // like "Olive" or "Alive" by requiring a word boundary.
        let live_markers = [" (live", " [live", ": live", " - live"];
        if live_markers.iter().any(|marker| lower.contains(marker))
            || lower.ends_with(" (live)")
            || lower.ends_with(" [live]")
        {
            return ArtistAutoSelect::LiveAlbums;
        }
    }

    if album.is_single == Some(true)
        || album.track_count.is_some_and(|tc| tc <= 6)
    {
        return ArtistAutoSelect::SinglesEps;
    }

    ArtistAutoSelect::MainAlbums
}

// ============================================================
// Playlist Tracks (for pre-queue duplicate detection, #512)
// ============================================================

/// A compact track record returned by the playlist-tracks fetch.
///
/// Unlike `TrackMetadata`, this carries only the fields the dedup pipeline
/// needs plus enough context to reconstruct a per-track download URL — song_id,
/// ISRC, title, and the album_id the track lives on (`catalogId` from
/// `playParams`). Tracks without a usable catalog album_id are filtered out
/// upstream (they can't be downloaded individually).
#[derive(Debug, Clone)]
pub struct PlaylistTrackRef {
    /// Apple Music catalog song ID.
    pub song_id: String,
    /// ISRC code, if the API provides one.
    pub isrc: Option<String>,
    /// Track title (diagnostic / activity-log only).
    pub name: String,
    /// Catalog album ID the track sits on (from `playParams.catalogId` on
    /// the `songs` resource). Required to build an `album/{id}?i={song_id}`
    /// URL for GAMDL.
    pub album_id: String,
}

/// Fetch every track on a catalog playlist.
///
/// Paginates `/v1/catalog/{storefront}/playlists/{playlist_id}/tracks`
/// with `limit=100` (Apple's maximum) until the `next` cursor is absent.
/// Used by the playlist dedup planner (#512) to enumerate a playlist's
/// contents before deciding which tracks to skip.
///
/// # Notes
///
/// - Catalog playlist IDs start with `pl.`. Library playlists (`p.`) are
///   out of scope here and need a different endpoint + Music-User-Token.
/// - Tracks that lack a resolvable catalog album_id (rare — e.g. radio
///   stations, cloud uploads) are silently dropped. The caller treats
///   those playlists as non-dedupable.
pub async fn fetch_playlist_tracks(
    jwt: &str,
    storefront: &str,
    playlist_id: &str,
) -> Result<Vec<PlaylistTrackRef>, String> {
    let client = crate::utils::http_client::build_simple(30)?;

    let mut next_path: Option<String> = Some(format!(
        "/v1/catalog/{storefront}/playlists/{playlist_id}/tracks?limit=100"
    ));
    let mut tracks: Vec<PlaylistTrackRef> = Vec::new();

    const MAX_PAGES: u32 = 50; // supports up to 5k tracks, well above real playlists
    let mut page = 0u32;

    while let Some(path) = next_path.take() {
        if page >= MAX_PAGES {
            log::warn!(
                "fetch_playlist_tracks: hit max page cap ({MAX_PAGES}) for playlist {playlist_id} in storefront '{storefront}'"
            );
            break;
        }
        page += 1;

        let url = format!("https://api.music.apple.com{path}");
        log::debug!("Fetching playlist tracks page {page}: {url}");

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {jwt}"))
            .header("User-Agent", "meedyadl")
            .send()
            .await
            .map_err(|e| format!("Playlist tracks API request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(format!(
                "Apple Music API returned HTTP {status} for playlist {playlist_id} tracks (page {page})"
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse playlist tracks response: {e}"))?;

        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            for item in data {
                // Only include songs — playlists can contain music-videos, which we don't dedupe.
                let kind = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if kind != "songs" {
                    continue;
                }
                let Some(song_id) = item.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                let attrs = item.get("attributes");
                let isrc = attrs
                    .and_then(|a| a.get("isrc"))
                    .and_then(|v| v.as_str())
                    .map(std::string::ToString::to_string);
                let name = attrs
                    .and_then(|a| a.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                // Prefer `playParams.catalogId` for album_id; fall back to
                // the `albums` relationship; fall back to the song id itself
                // (can't really happen — but keeps the flow defensive).
                let album_id = attrs
                    .and_then(|a| a.get("playParams"))
                    .and_then(|pp| pp.get("catalogId"))
                    .and_then(|v| v.as_str())
                    .map(std::string::ToString::to_string)
                    .or_else(|| {
                        item.get("relationships")
                            .and_then(|r| r.get("albums"))
                            .and_then(|a| a.get("data"))
                            .and_then(|d| d.get(0))
                            .and_then(|album| album.get("id"))
                            .and_then(|v| v.as_str())
                            .map(std::string::ToString::to_string)
                    });
                let Some(album_id) = album_id else {
                    // Without a catalog album id we can't build a downloadable
                    // per-track URL — skip. Worst case the overall planner
                    // falls through for the whole playlist if enough tracks
                    // are unusable.
                    continue;
                };

                tracks.push(PlaylistTrackRef {
                    song_id: song_id.to_string(),
                    isrc,
                    name,
                    album_id,
                });
            }
        }

        next_path = json
            .get("next")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);
    }

    log::info!(
        "fetch_playlist_tracks: collected {} track(s) for playlist {playlist_id} in storefront '{storefront}' across {page} page(s)",
        tracks.len()
    );

    Ok(tracks)
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

    let client = crate::utils::http_client::build_simple(30)?;

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

    let client = crate::utils::http_client::build_simple(15)?;
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
    //
    // ArtistSpotlightCover.mp4 is a 16:9 full-width panel designed to
    // play behind the artist's name on an artist page — the hero
    // motion background. We restrict the source to the two `motionArtist*`
    // keys because they are the only feeds shot/framed for that role.
    // The `motionDetailSquare` / `motionDetailTall` feeds that appear on
    // album detail pages have a different composition (they're tightly
    // cropped around album cover art) and look wrong used as an artist
    // spotlight — so we deliberately do NOT fall through to them here.
    //
    // Priority: `motionArtistFullscreen16x9` → `motionArtistWide16x9`.
    // Fullscreen is preferred when available because its source stream
    // is typically higher-resolution and has full-bleed framing; the
    // Wide variant is the common fallback. If neither key is present,
    // we return `None` and the caller skips the download rather than
    // substituting a visually mismatched fallback (#537).
    let editorial_video = match attributes.get("editorialVideo") {
        Some(ev) => ev,
        None => {
            log::debug!("No editorialVideo for artist {artist_name} ({artist_id})");
            return Ok(None);
        }
    };

    let video_keys = [
        "motionArtistFullscreen16x9",
        "motionArtistWide16x9",
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
    // Storefront-rewrite tests (#666)
    // ----------------------------------------------------------

    #[test]
    fn rewrite_storefront_swaps_us_to_gb_album() {
        assert_eq!(
            rewrite_url_storefront("https://music.apple.com/us/album/foo/123", "gb"),
            "https://music.apple.com/gb/album/foo/123",
        );
    }

    #[test]
    fn rewrite_storefront_preserves_track_query() {
        // ?i= query identifies a single track inside an album. Must survive
        // the rewrite or per-track retries lose their target.
        assert_eq!(
            rewrite_url_storefront(
                "https://music.apple.com/us/album/midnights/1649434004?i=1649434280",
                "gb",
            ),
            "https://music.apple.com/gb/album/midnights/1649434004?i=1649434280",
        );
    }

    #[test]
    fn rewrite_storefront_handles_classical_host() {
        assert_eq!(
            rewrite_url_storefront(
                "https://classical.music.apple.com/it/album/foo/789",
                "gb",
            ),
            "https://classical.music.apple.com/gb/album/foo/789",
        );
        assert_eq!(
            rewrite_url_storefront("https://classical.apple.com/it/album/789", "gb"),
            "https://classical.apple.com/gb/album/789",
        );
    }

    #[test]
    fn rewrite_storefront_handles_itunes_host() {
        assert_eq!(
            rewrite_url_storefront("https://itunes.apple.com/jp/album/foo/123", "gb"),
            "https://itunes.apple.com/gb/album/foo/123",
        );
    }

    #[test]
    fn rewrite_storefront_covers_all_content_types() {
        for content_type in ["album", "song", "music-video", "artist", "playlist"] {
            let input = format!("https://music.apple.com/us/{content_type}/foo/123");
            let expected = format!("https://music.apple.com/gb/{content_type}/foo/123");
            assert_eq!(
                rewrite_url_storefront(&input, "gb"),
                expected,
                "rewrite failed for content type {content_type}",
            );
        }
    }

    #[test]
    fn rewrite_storefront_uppercase_input_normalised() {
        // Settings.storefront is canonically lowercase but the helper must
        // not produce an invalid uppercase URL if it ever receives "GB".
        assert_eq!(
            rewrite_url_storefront("https://music.apple.com/us/album/foo/123", "GB"),
            "https://music.apple.com/gb/album/foo/123",
        );
    }

    #[test]
    fn rewrite_storefront_rejects_invalid_target() {
        // Non-2-letter / non-alphabetic targets must produce no change so
        // we never mangle a working URL.
        let url = "https://music.apple.com/us/album/foo/123";
        assert_eq!(rewrite_url_storefront(url, ""), url);
        assert_eq!(rewrite_url_storefront(url, "g"), url);
        assert_eq!(rewrite_url_storefront(url, "gbr"), url);
        assert_eq!(rewrite_url_storefront(url, "g1"), url);
    }

    #[test]
    fn rewrite_storefront_passes_through_library_url() {
        // /library/… URLs use a Music-User-Token endpoint where storefront
        // isn't a free variable. Helper must leave them untouched so the
        // retry path doesn't ship a malformed URL to GAMDL.
        let url = "https://music.apple.com/us/library/albums/l.foo123";
        assert_eq!(rewrite_url_storefront(url, "gb"), url);
    }

    #[test]
    fn rewrite_storefront_passes_through_non_apple_url() {
        let url = "https://example.com/some/path";
        assert_eq!(rewrite_url_storefront(url, "gb"), url);
    }

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

    // ----------------------------------------------------------
    // canonicalise_apple_music_url (#807)
    // ----------------------------------------------------------

    /// The literal repro from the user report — \`/gb/\` with a slug
    /// must canonicalise to the same form as MusicBrainz's
    /// \`/us/\` without a slug.
    #[test]
    fn canonicalise_matches_musicbrainz_super_bowl_pair() {
        let user_url = "https://music.apple.com/gb/album/super-bowl-lviii-megamix-dj-mix/1729264859";
        let mb_url = "https://music.apple.com/us/album/1729264859";
        assert_eq!(
            canonicalise_apple_music_url(user_url),
            canonicalise_apple_music_url(mb_url),
            "the canonicaliser must reduce both forms to the same string so MusicBrainz Tier 1 lookup matches across storefronts (#807)"
        );
    }

    /// Storefront strip: every common storefront produces the same canonical form.
    #[test]
    fn canonicalise_strips_storefront() {
        let canonical = canonicalise_apple_music_url(
            "https://music.apple.com/us/album/anti-hero/1649434004",
        );
        for storefront in &["gb", "de", "fr", "jp", "br", "au", "ca", "mx", "es", "it"] {
            let url = format!(
                "https://music.apple.com/{storefront}/album/anti-hero/1649434004",
            );
            assert_eq!(
                canonicalise_apple_music_url(&url),
                canonical,
                "storefront {storefront} must reduce to the same canonical form as us",
            );
        }
    }

    /// Slug strip: the canonical form drops the human-readable
    /// slug between the type and the numeric ID.
    #[test]
    fn canonicalise_strips_slug() {
        let with_slug = canonicalise_apple_music_url(
            "https://music.apple.com/us/album/super-bowl-lviii-megamix-dj-mix/1729264859",
        );
        let without_slug =
            canonicalise_apple_music_url("https://music.apple.com/us/album/1729264859");
        assert_eq!(with_slug, without_slug);
        // The canonical form itself shouldn't carry the slug.
        let canonical = with_slug.unwrap();
        assert!(
            !canonical.contains("super-bowl"),
            "canonical form must not carry the slug, got {canonical:?}",
        );
        assert!(canonical.contains("1729264859"));
    }

    /// Query-string strip: \`?i=…\` and \`?l=en-GB\` and similar
    /// are dropped — the canonical form is the pure path.
    #[test]
    fn canonicalise_strips_query_string() {
        let with_query = canonicalise_apple_music_url(
            "https://music.apple.com/us/album/1729264859?l=en-GB",
        );
        let without_query =
            canonicalise_apple_music_url("https://music.apple.com/us/album/1729264859");
        assert_eq!(with_query, without_query);
    }

    /// Library URLs (\`l.XXXX\` numeric prefix) return None — they're
    /// per-user identifiers and not portable across sources.
    #[test]
    fn canonicalise_library_url_returns_none() {
        let url = "https://music.apple.com/us/library/albums/l.GpB5n1h";
        assert!(canonicalise_apple_music_url(url).is_none());
    }

    /// Idempotency: canonicalising the canonical form returns the
    /// same string. (Required because the MusicBrainz lookup may
    /// call canonicalise on already-canonical MB-stored URLs.)
    #[test]
    fn canonicalise_is_idempotent() {
        let url = "https://music.apple.com/us/album/super-bowl-lviii-megamix-dj-mix/1729264859";
        let once = canonicalise_apple_music_url(url).unwrap();
        // Re-canonicalising the canonical form requires routing
        // through the parser, which expects a real URL — so we
        // can't strictly assert `canonicalise(canonical) == canonical`
        // without rebuilding a real URL envelope. What we CAN
        // assert is that the canonical form is stable across
        // every slug-and-storefront permutation of the same ID.
        let permutations = [
            "https://music.apple.com/gb/album/super-bowl/1729264859",
            "https://music.apple.com/de/album/super-bowl-lviii-megamix-dj-mix/1729264859",
            "https://music.apple.com/jp/album/1729264859?i=999",
        ];
        for p in permutations {
            let canon = canonicalise_apple_music_url(p).unwrap();
            assert_eq!(canon, once, "permutation {p:?} must produce the same canonical form");
        }
    }

    /// Non-Apple-Music URLs return None — the canonicaliser is
    /// scoped to Apple Music URLs only.
    #[test]
    fn canonicalise_non_apple_music_returns_none() {
        assert!(canonicalise_apple_music_url("https://open.spotify.com/album/foo").is_none());
        assert!(canonicalise_apple_music_url("https://www.youtube.com/watch?v=foo").is_none());
        assert!(canonicalise_apple_music_url("not a url at all").is_none());
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
    fn parse_playlist_url_extracts_id() {
        // Catalog playlist URLs (#512) parse into content_type="playlist"
        // with playlist_id populated from the pl.XXXX slug.
        let url =
            "https://music.apple.com/us/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb";
        let parsed = parse_apple_music_url(url).expect("catalog playlist URL should parse");
        assert_eq!(parsed.content_type, "playlist");
        assert_eq!(parsed.storefront, "us");
        assert_eq!(
            parsed.playlist_id.as_deref(),
            Some("pl.f4d106fed2bd41149aaacabb233eb5eb")
        );
        assert!(parsed.album_id.is_empty());
        assert!(parsed.song_id.is_none());
        assert!(parsed.artist_id.is_none());
    }

    #[test]
    fn parse_library_playlist_url_returns_none() {
        // Library playlists (/library/playlist/p.xxx) require different
        // auth (Music-User-Token) and must still be rejected by the
        // catalog-URL parser so the caller falls through to the generic
        // GAMDL path.
        let url = "https://music.apple.com/us/library/playlist/p.abc123";
        assert!(parse_apple_music_url(url).is_none());
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

    // Apple Music Classical domain migration (2026): Apple moved Classical
    // under the `music.apple.com` subdomain hierarchy and dropped the slug
    // segment from Share-link URLs. These tests lock in parser support for
    // the new `classical.music.apple.com` domain and the slug-less
    // `/album/{id}` / `/song/{id}` / etc. path style. Regression canary
    // against Apple's UI rolling the change back.

    #[test]
    fn parse_new_classical_album_url_without_slug() {
        // Real-world URL shape captured 2026-04-23 from Apple Music
        // Classical app's Share → Copy Link.
        let url = "https://classical.music.apple.com/gb/album/1844602145";
        let parsed = parse_apple_music_url(url).expect("new classical URL should parse");
        assert_eq!(parsed.storefront, "gb");
        assert_eq!(parsed.content_type, "album");
        assert_eq!(parsed.album_id, "1844602145");
        assert!(parsed.song_id.is_none());
    }

    #[test]
    fn parse_new_classical_album_url_with_locale_query() {
        // `?l=en-GB` locale hint appended by Apple's new Share UI.
        // Regex must not capture it as a song ID.
        let url = "https://classical.music.apple.com/gb/album/1844602145?l=en-GB";
        let parsed = parse_apple_music_url(url).expect("?l= query should not block parsing");
        assert_eq!(parsed.storefront, "gb");
        assert_eq!(parsed.album_id, "1844602145");
        assert!(parsed.song_id.is_none(), "song_id should be None; ?l= is a locale hint");
    }

    #[test]
    fn parse_new_classical_album_url_with_track_id() {
        let url = "https://classical.music.apple.com/gb/album/1844602145?i=1844602150";
        let parsed = parse_apple_music_url(url).expect("?i= form must still work");
        assert_eq!(parsed.album_id, "1844602145");
        assert_eq!(parsed.song_id.as_deref(), Some("1844602150"));
    }

    #[test]
    fn parse_new_classical_song_url_without_slug() {
        let url = "https://classical.music.apple.com/us/song/9876543210";
        let parsed = parse_apple_music_url(url).expect("new classical song URL should parse");
        assert_eq!(parsed.content_type, "song");
        assert_eq!(parsed.song_id.as_deref(), Some("9876543210"));
    }

    #[test]
    fn parse_new_classical_music_video_url_without_slug() {
        let url = "https://classical.music.apple.com/us/music-video/1234567890";
        let parsed = parse_apple_music_url(url).expect("new classical MV URL should parse");
        assert_eq!(parsed.content_type, "music-video");
        assert_eq!(parsed.album_id, "1234567890");
    }

    #[test]
    fn parse_new_classical_artist_url_without_slug() {
        let url = "https://classical.music.apple.com/us/artist/123456789";
        let parsed = parse_apple_music_url(url).expect("new classical artist URL should parse");
        assert_eq!(parsed.content_type, "artist");
        assert_eq!(parsed.artist_id.as_deref(), Some("123456789"));
    }

    #[test]
    fn parse_new_classical_playlist_url_without_slug() {
        let url = "https://classical.music.apple.com/us/playlist/pl.u-ABCDefgh123456";
        let parsed = parse_apple_music_url(url).expect("new classical playlist URL should parse");
        assert_eq!(parsed.content_type, "playlist");
        assert_eq!(parsed.playlist_id.as_deref(), Some("pl.u-ABCDefgh123456"));
    }

    #[test]
    fn parse_new_classical_album_url_with_slug_still_works() {
        // If Apple keeps emitting slugged URLs for backward compat,
        // the slug-optional regex must still accept them on the new
        // hostname.
        let url = "https://classical.music.apple.com/us/album/beethoven-9-symphonies/1844602145";
        let parsed = parse_apple_music_url(url).expect("slugged form on new domain must still parse");
        assert_eq!(parsed.album_id, "1844602145");
    }

    #[test]
    fn parse_classic_slugless_form_on_music_apple_com() {
        // If Apple rolls the slug-less format out to the main
        // music.apple.com domain, the parser should keep up.
        // Defensive coverage — may or may not exist in the wild yet.
        let url = "https://music.apple.com/us/album/1649434004";
        let parsed = parse_apple_music_url(url).expect("slugless form on music.apple.com should parse");
        assert_eq!(parsed.album_id, "1649434004");
    }

    // Legacy iTunes Store URLs (#548). Before the alternation was extended
    // to include `itunes`, these passed host validation but failed every
    // parser branch and reached GAMDL raw with no MeedyaDL metadata
    // prefetch or storefront normalisation. The regex gap was the fix;
    // these tests lock it in.

    #[test]
    fn parse_itunes_legacy_album_url() {
        let url = "https://itunes.apple.com/us/album/some-album/1234567890";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.content_type, "album");
        assert_eq!(parsed.album_id, "1234567890");
        assert!(parsed.song_id.is_none());
    }

    #[test]
    fn parse_itunes_legacy_album_url_with_track() {
        let url = "https://itunes.apple.com/gb/album/some-album/1234567890?i=9876543210";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "gb");
        assert_eq!(parsed.content_type, "album");
        assert_eq!(parsed.album_id, "1234567890");
        assert_eq!(parsed.song_id.unwrap(), "9876543210");
    }

    #[test]
    fn parse_itunes_legacy_song_url() {
        let url = "https://itunes.apple.com/us/song/some-song/9876543210";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.content_type, "song");
        assert_eq!(parsed.song_id.unwrap(), "9876543210");
    }

    #[test]
    fn parse_itunes_legacy_music_video_url() {
        let url = "https://itunes.apple.com/us/music-video/some-video/1234567890";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.content_type, "music-video");
        assert_eq!(parsed.album_id, "1234567890");
    }

    #[test]
    fn parse_itunes_legacy_artist_url() {
        let url = "https://itunes.apple.com/us/artist/some-artist/159260351";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.content_type, "artist");
        assert_eq!(parsed.artist_id, Some("159260351".to_string()));
    }

    #[test]
    fn parse_itunes_legacy_playlist_url() {
        let url = "https://itunes.apple.com/us/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.content_type, "playlist");
        assert_eq!(
            parsed.playlist_id,
            Some("pl.f4d106fed2bd41149aaacabb233eb5eb".to_string())
        );
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
            album_spotlight_url: None,
            artwork_url_template: Some(
                "https://is1-ssl.mzstatic.com/.../source/{w}x{h}{c}.{f}".to_string(),
            ),
            artwork_width: Some(3000),
            artwork_height: Some(3000),
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
    // build_included_name_lookup — JSON:API parser for MV names (#775)
    // ----------------------------------------------------------

    /// Happy path: `included[]` contains music-video entries with names;
    /// the lookup picks them up keyed by id.
    #[test]
    fn build_included_name_lookup_extracts_mv_names() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "data": [],
                "included": [
                    { "id": "1649500001", "type": "music-videos",
                      "attributes": { "name": "Lavender Haze" } },
                    { "id": "1649500002", "type": "music-videos",
                      "attributes": { "name": "Anti-Hero" } }
                ]
            }"#,
        )
        .expect("test JSON parses");

        let lookup = super::build_included_name_lookup(&json);
        assert_eq!(lookup.get("1649500001"), Some(&"Lavender Haze".to_string()));
        assert_eq!(lookup.get("1649500002"), Some(&"Anti-Hero".to_string()));
        assert_eq!(lookup.len(), 2);
    }

    /// Defensive: response without `included[]` returns an empty map
    /// (callers fall back to inline relationship data).
    #[test]
    fn build_included_name_lookup_handles_missing_included_array() {
        let json: serde_json::Value = serde_json::from_str(r#"{"data": []}"#).unwrap();
        let lookup = super::build_included_name_lookup(&json);
        assert!(lookup.is_empty());
    }

    /// Type discrimination: only entries with `type=="music-videos"`
    /// contribute to the lookup. Songs / albums / other resource types
    /// in `included[]` are ignored even if they share an ID space.
    #[test]
    fn build_included_name_lookup_ignores_non_mv_resource_types() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "data": [],
                "included": [
                    { "id": "999", "type": "songs",
                      "attributes": { "name": "Song Title" } },
                    { "id": "999", "type": "music-videos",
                      "attributes": { "name": "MV Title" } }
                ]
            }"#,
        )
        .unwrap();

        let lookup = super::build_included_name_lookup(&json);
        assert_eq!(lookup.get("999"), Some(&"MV Title".to_string()));
        assert_eq!(lookup.len(), 1);
    }

    /// Defensive: an MV entry without `attributes.name` is skipped
    /// rather than panicking. Callers fall back to inline data.
    #[test]
    fn build_included_name_lookup_skips_entries_without_name() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
                "data": [],
                "included": [
                    { "id": "100", "type": "music-videos",
                      "attributes": { "artistName": "Some Artist" } },
                    { "id": "101", "type": "music-videos",
                      "attributes": { "name": "Has Name" } }
                ]
            }"#,
        )
        .unwrap();

        let lookup = super::build_included_name_lookup(&json);
        assert!(!lookup.contains_key("100"), "no name → not in lookup");
        assert_eq!(lookup.get("101"), Some(&"Has Name".to_string()));
    }

    // ----------------------------------------------------------
    // iTunes legacy URL rewrite tests (#568)
    // ----------------------------------------------------------

    /// Acceptance criterion 1: geographic + id-prefix + slug-less.
    #[test]
    fn rewrite_itunes_album_with_storefront_and_id_prefix() {
        let url = "https://itunes.apple.com/gb/album/id1567637891";
        let result = normalize_apple_music_url(url);
        assert_eq!(result, "https://music.apple.com/gb/album/1567637891");
    }

    /// Acceptance criterion 2: geographic + slug + numeric (no `id` prefix).
    #[test]
    fn rewrite_itunes_album_with_slug_and_plain_id() {
        let url = "https://itunes.apple.com/gb/album/some-slug/1567637891";
        let result = normalize_apple_music_url(url);
        assert_eq!(result, "https://music.apple.com/gb/album/some-slug/1567637891");
    }

    /// Acceptance criterion 3: non-geographic + id-prefix + slug-less.
    /// Storefront injection happens AFTER the iTunes-host rewrite so the
    /// final URL has both fixes applied.
    #[test]
    fn rewrite_itunes_album_non_geographic_then_inject_storefront() {
        let url = "https://itunes.apple.com/album/id1567637891";
        let result = normalize_apple_music_url(url);
        // Host swapped + id stripped; storefront injected by
        // detect_non_geographic_url. Storefront resolution depends on
        // the OS locale at test time, so we only assert the structural
        // shape (host, content type, id) — not the specific code.
        assert!(
            result.starts_with("https://music.apple.com/"),
            "host must be rewritten: {result}"
        );
        assert!(
            result.ends_with("/album/1567637891"),
            "id must be stripped of `id` prefix: {result}"
        );
        // Path between host and content type should be a 2-letter
        // storefront.
        let after_host = result
            .strip_prefix("https://music.apple.com/")
            .expect("host prefix");
        let first_segment = after_host.split('/').next().unwrap_or("");
        assert_eq!(first_segment.len(), 2, "storefront injected: {result}");
    }

    /// All five content types iTunes URLs can carry must rewrite.
    #[test]
    fn rewrite_itunes_song_url() {
        assert_eq!(
            normalize_apple_music_url("https://itunes.apple.com/us/song/some-song/9876543210"),
            "https://music.apple.com/us/song/some-song/9876543210"
        );
    }

    #[test]
    fn rewrite_itunes_song_with_id_prefix() {
        assert_eq!(
            normalize_apple_music_url("https://itunes.apple.com/us/song/id9876543210"),
            "https://music.apple.com/us/song/9876543210"
        );
    }

    #[test]
    fn rewrite_itunes_music_video_url() {
        assert_eq!(
            normalize_apple_music_url("https://itunes.apple.com/us/music-video/some-mv/1234567890"),
            "https://music.apple.com/us/music-video/some-mv/1234567890"
        );
    }

    #[test]
    fn rewrite_itunes_artist_url() {
        assert_eq!(
            normalize_apple_music_url("https://itunes.apple.com/us/artist/some-artist/159260351"),
            "https://music.apple.com/us/artist/some-artist/159260351"
        );
    }

    /// Playlist IDs use a `pl.<token>` shape rather than digits — the
    /// `id` prefix strip must NOT touch them.
    #[test]
    fn rewrite_itunes_playlist_url_preserves_pl_token() {
        assert_eq!(
            normalize_apple_music_url(
                "https://itunes.apple.com/us/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb"
            ),
            "https://music.apple.com/us/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb"
        );
    }

    /// Query string (e.g. `?i=<song_id>` for in-album track) and
    /// fragment (`#anchor`) survive the rewrite verbatim.
    #[test]
    fn rewrite_itunes_album_preserves_track_query() {
        assert_eq!(
            normalize_apple_music_url(
                "https://itunes.apple.com/gb/album/some-slug/1567637891?i=9876543210"
            ),
            "https://music.apple.com/gb/album/some-slug/1567637891?i=9876543210"
        );
    }

    #[test]
    fn rewrite_itunes_album_preserves_fragment() {
        assert_eq!(
            normalize_apple_music_url(
                "https://itunes.apple.com/gb/album/some-slug/1567637891#section"
            ),
            "https://music.apple.com/gb/album/some-slug/1567637891#section"
        );
    }

    /// Defensive: iTunes URLs with shapes we don't recognise (uploaded
    /// videos, lookup endpoint, novel paths) are passed through
    /// unchanged so the #549 catch-all WARN can surface them as
    /// "unrecognised" instead of silently mangling.
    #[test]
    fn rewrite_itunes_unknown_path_passes_through_unchanged() {
        let url = "https://itunes.apple.com/lookup?id=1567637891&entity=song";
        assert_eq!(normalize_apple_music_url(url), url);
    }

    #[test]
    fn rewrite_itunes_uploaded_video_passes_through_unchanged() {
        // Hypothetical — `uploaded-video` isn't in the rewrite alternation
        // (parser handles it via #549 catch-all instead).
        let url = "https://itunes.apple.com/gb/uploaded-video/something/123";
        assert_eq!(normalize_apple_music_url(url), url);
    }

    /// Non-iTunes URLs (the regular music.apple.com path, classical, etc.)
    /// are not touched by the rewrite — they fall through to the existing
    /// normalize logic.
    #[test]
    fn rewrite_does_not_touch_music_apple_com() {
        let url = "https://music.apple.com/us/album/midnights/1649434004";
        assert_eq!(normalize_apple_music_url(url), url);
    }

    // #880: The classical-host rewrite is gated behind
    // `GamdlFeature::ClassicalMusicHostRequired`. Tests that exercise
    // the rewrite must set the detected GAMDL version first; tests
    // that exercise the unknown-version pass-through path clear the
    // cache first. A small RAII guard makes the
    // set-version-then-restore-on-drop pattern obvious at the call
    // site so parallel tests don't leak state into each other.
    struct VersionGuard {
        previous: Option<String>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl VersionGuard {
        fn new(version: Option<&str>) -> Self {
            static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let lock = LOCK.lock().unwrap_or_else(|p| p.into_inner());
            let previous = crate::services::gamdl_capabilities::detected_version();
            crate::services::gamdl_capabilities::set_detected_version(
                version.map(ToString::to_string),
            );
            Self {
                previous,
                _lock: lock,
            }
        }
    }

    impl Drop for VersionGuard {
        fn drop(&mut self) {
            crate::services::gamdl_capabilities::set_detected_version(self.previous.take());
        }
    }

    #[test]
    fn normalize_rewrites_classical_apple_com_on_supported_gamdl() {
        let _g = VersionGuard::new(Some("3.7.1"));
        // #880: GAMDL >= 2.9.1's URL regex requires `classical.music.apple.com`
        // (note the `music.` segment). The bare legacy host
        // `classical.apple.com` was accepted by MeedyaDL's own parser
        // but rejected by GAMDL. `normalize_apple_music_url` now rewrites
        // the legacy host silently so the downstream subprocess accepts
        // the URL.
        assert_eq!(
            normalize_apple_music_url(
                "https://classical.apple.com/us/album/some-classical/123456"
            ),
            "https://classical.music.apple.com/us/album/some-classical/123456",
        );
    }

    #[test]
    fn normalize_rewrites_classical_apple_com_preserving_track_query() {
        let _g = VersionGuard::new(Some("3.7.1"));
        // The `?i=` query identifies a single track inside an album.
        // Must survive the rewrite or per-track retries lose their target.
        assert_eq!(
            normalize_apple_music_url(
                "https://classical.apple.com/gb/album/beethoven-symphony-9/123456?i=789"
            ),
            "https://classical.music.apple.com/gb/album/beethoven-symphony-9/123456?i=789",
        );
    }

    #[test]
    fn normalize_does_not_touch_classical_music_apple_com() {
        let _g = VersionGuard::new(Some("3.7.1"));
        // The CURRENT classical host (with `.music.` segment) must NOT
        // be rewritten — it's already in the form GAMDL accepts.
        let url = "https://classical.music.apple.com/us/album/foo/123";
        assert_eq!(normalize_apple_music_url(url), url);
    }

    #[test]
    fn normalize_does_not_rewrite_classical_apple_com_when_gamdl_version_unknown() {
        // Pre-#880 pass-through behaviour: on an unaudited GAMDL (None
        // version cached) MeedyaDL must NOT rewrite, preserving exactly
        // the URL the user pasted. Per the user's note when filing #880:
        // "older versions should still be supported".
        let _g = VersionGuard::new(None);
        let url = "https://classical.apple.com/us/album/some-classical/123456";
        assert_eq!(normalize_apple_music_url(url), url);
    }

    #[test]
    fn rewrite_classical_legacy_url_returns_none_for_non_legacy_hosts() {
        // Direct helper-level test: the rewrite must be a strict no-op
        // for every non-legacy-classical URL form so it's safe to use
        // in the normalisation chain unconditionally. This test does
        // NOT need a VersionGuard because the helper is pure — the
        // gate is enforced one level up in `normalize_apple_music_url`.
        assert!(rewrite_classical_legacy_url("https://music.apple.com/us/album/foo/123").is_none());
        assert!(rewrite_classical_legacy_url(
            "https://classical.music.apple.com/us/album/foo/123"
        )
        .is_none());
        assert!(rewrite_classical_legacy_url("https://itunes.apple.com/us/album/foo/123").is_none());
        assert!(rewrite_classical_legacy_url("https://example.com").is_none());
    }

    /// Non-Apple URLs are completely untouched.
    #[test]
    fn rewrite_does_not_touch_non_apple_url() {
        let url = "https://example.com/some/path";
        assert_eq!(normalize_apple_music_url(url), url);
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
        // Updated for #880: when a known-supported GAMDL is detected the
        // legacy `classical.apple.com` host is also rewritten to
        // `classical.music.apple.com` before storefront injection. So
        // the result starts with the NEW host, not the legacy one. The
        // assertion still anchors on the slug+ID + the "different from
        // input" property; both still hold.
        let _g = VersionGuard::new(Some("3.7.1"));
        let url = "https://classical.apple.com/album/beethoven-symphony/1234567890";
        let result = normalize_apple_music_url(url);
        assert!(result.starts_with("https://classical.music.apple.com/"));
        assert!(result.contains("/album/beethoven-symphony/1234567890"));
        assert_ne!(result, url);
    }

    #[test]
    fn normalize_itunes_url_without_storefront() {
        let url = "https://itunes.apple.com/album/some-album/1234567890";
        let result = normalize_apple_music_url(url);
        // Updated for #568: iTunes-domain URLs are now rewritten to
        // music.apple.com (GAMDL doesn't accept itunes.apple.com URLs)
        // before storefront injection runs. Final shape is therefore
        // music.apple.com/{storefront}/album/some-album/1234567890.
        assert!(
            result.starts_with("https://music.apple.com/"),
            "iTunes domain must be rewritten to music.apple.com: {result}"
        );
        assert!(result.contains("/album/some-album/1234567890"));
        assert_ne!(result, url);
    }

    #[test]
    fn normalize_new_classical_url_without_storefront() {
        // The new Classical domain must also get storefront injection
        // when the URL arrives in the non-geographic shape.
        let url = "https://classical.music.apple.com/album/1844602145";
        let result = normalize_apple_music_url(url);
        assert!(result.starts_with("https://classical.music.apple.com/"));
        assert!(result.contains("/album/1844602145"));
        assert_ne!(result, url, "storefront should be injected");
    }

    #[test]
    fn normalize_new_classical_url_with_storefront_unchanged() {
        // Already has a storefront → returned unchanged (parser matches).
        let url = "https://classical.music.apple.com/gb/album/1844602145";
        assert_eq!(normalize_apple_music_url(url), url);
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

    // ----------------------------------------------------------
    // is_library_url tests (#871)
    // ----------------------------------------------------------

    #[test]
    fn is_library_url_matches_library_song() {
        assert!(is_library_url(
            "https://music.apple.com/us/library/songs/i.GxEKn7nhdVeR93o"
        ));
    }

    #[test]
    fn is_library_url_matches_library_album() {
        assert!(is_library_url(
            "https://music.apple.com/gb/library/albums/l.MGoVNk0"
        ));
    }

    #[test]
    fn is_library_url_matches_library_music_video() {
        assert!(is_library_url(
            "https://music.apple.com/us/library/music-videos/i.aB3yX9k"
        ));
    }

    #[test]
    fn is_library_url_rejects_catalog_album() {
        assert!(!is_library_url(
            "https://music.apple.com/us/album/abbey-road/1441164426"
        ));
    }

    #[test]
    fn is_library_url_rejects_catalog_playlist() {
        assert!(!is_library_url(
            "https://music.apple.com/gb/playlist/chill-mix/pl.u-XkD0NgYI2DRm5"
        ));
    }

    #[test]
    fn is_library_url_rejects_artist_url() {
        assert!(!is_library_url(
            "https://music.apple.com/us/artist/the-beatles/136975"
        ));
    }
}
