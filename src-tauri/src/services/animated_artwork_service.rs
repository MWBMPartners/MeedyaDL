// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Animated artwork (motion cover art) download service.
// ======================================================
//
// Downloads animated cover art from Apple Music's catalog API and saves
// them as sidecar MP4 files alongside downloaded album audio files.
//
// ## How it works
//
// Apple Music provides animated (motion) artwork for many albums, delivered
// as HEVC H.265 video via HLS (HTTP Live Streaming) playlists. This service:
//
// 1. Parses the Apple Music URL to extract the storefront (country code)
//    and album ID.
// 2. Generates a short-lived MusicKit Developer Token (ES256-signed JWT)
//    using the user's Apple Developer credentials.
// 3. Queries the Apple Music catalog API with `extend=editorialVideo` to
//    check for animated artwork availability.
// 4. If available, uses FFmpeg to download the HLS streams directly to MP4:
//    - `FrontCover.mp4`    -- square (1:1), from `motionDetailSquare`
//    - `PortraitCover.mp4` -- portrait (3:4), from `motionDetailTall`
//
// ## Authentication
//
// The Apple Music API requires a MusicKit Developer Token (JWT) for
// catalog queries. The user provides:
// - **Team ID** and **Key ID**: stored in `AppSettings` (non-sensitive)
// - **Private key** (`.p8` file content): stored in the OS keychain
//   under the key `"musickit_private_key"` (never in config files)
//
// The JWT is generated fresh for each request with a 1-hour expiry,
// so there's no need to cache or refresh tokens.
//
// ## Output files
//
// | Artwork Type | Filename           | Aspect Ratio | Max Resolution |
// |--------------|--------------------|--------------|----------------|
// | Square       | `FrontCover.mp4`   | 1:1          | 3840x3840      |
// | Portrait     | `PortraitCover.mp4`| 3:4          | 2048x2732      |
//
// ## Error handling
//
// This service is designed to fail gracefully. If animated artwork is
// disabled, credentials are missing, the album has no motion artwork, or
// FFmpeg is not installed, the service returns early without errors
// propagating to the user. Only genuine unexpected failures are logged.
//
// ## References
//
// - Apple MusicKit Developer Tokens:
//   https://developer.apple.com/documentation/applemusicapi/generating_developer_tokens
// - Apple Music API `editorialVideo` extension:
//   Undocumented; returns M3U8 HLS URLs for `motionDetailSquare` and
//   `motionDetailTall` within album attributes.
// - FFmpeg HLS input:
//   https://ffmpeg.org/ffmpeg-protocols.html#hls

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::process::Command;

use crate::services::{config_service, dependency_manager};

// ============================================================
// Public Types
// ============================================================

/// Result of an animated artwork download attempt.
///
/// Serialized to JSON and returned to the frontend via the
/// `download_animated_artwork` Tauri command. The frontend can use
/// these flags to display success/skip indicators in the queue UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtworkResult {
    /// Whether the square (1:1) animated cover was downloaded as FrontCover.mp4
    pub square_downloaded: bool,
    /// Whether the portrait (3:4) animated cover was downloaded as PortraitCover.mp4
    pub portrait_downloaded: bool,
}

// ============================================================
// Internal Types
// ============================================================

/// HLS stream URLs for animated artwork, parsed from the Apple Music API
/// response. Each field is `None` if the album doesn't have that artwork type.
struct ArtworkUrls {
    /// M3U8 HLS URL for the square (1:1) animated cover (`motionDetailSquare`)
    square: Option<String>,
    /// M3U8 HLS URL for the portrait (3:4) animated cover (`motionDetailTall`)
    tall: Option<String>,
}

/// A parsed Apple Music album URL, containing the storefront (country code)
/// and numeric album ID needed for API queries.
struct ParsedAlbumUrl {
    /// Two-letter country code (e.g., "us", "gb", "jp")
    storefront: String,
    /// Numeric album identifier (e.g., "1234567890")
    album_id: String,
}

// ============================================================
// Public API
// ============================================================

/// Orchestrator: check for and download animated artwork for a completed album.
///
/// This is the main entry point called after a download completes. It handles
/// the entire flow: credential loading, URL parsing, API query, HLS download.
///
/// # Arguments
/// * `app` - Tauri AppHandle for accessing settings, keychain, and tool paths
/// * `urls` - The Apple Music URL(s) from the download request
/// * `output_dir` - The album output directory where audio files were saved
///
/// # Returns
/// * `Ok(ArtworkResult)` - Which artwork types were downloaded (may be both false)
/// * `Err(String)` - Only for unexpected failures (not "no artwork available")
///
/// # Graceful exits (returns Ok with both false):
/// * Feature disabled in settings
/// * MusicKit credentials not configured
/// * URL is not an album URL (single track, playlist, music video)
/// * Album has no animated artwork
/// * FFmpeg not installed
pub async fn process_album_artwork(
    app: &AppHandle,
    urls: &[String],
    output_dir: &str,
) -> Result<ArtworkResult, String> {
    // --- Step 1: Check if feature is enabled and credentials are configured ---
    let settings = config_service::load_settings(app).unwrap_or_default();

    if !settings.animated_artwork_enabled {
        log::debug!("Animated artwork disabled in settings");
        return Ok(ArtworkResult {
            square_downloaded: false,
            portrait_downloaded: false,
        });
    }

    // Team ID and Key ID are stored in settings (non-sensitive).
    let team_id = match &settings.musickit_team_id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => {
            log::debug!("MusicKit Team ID not configured, skipping animated artwork");
            return Ok(ArtworkResult {
                square_downloaded: false,
                portrait_downloaded: false,
            });
        }
    };

    let key_id = match &settings.musickit_key_id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => {
            log::debug!("MusicKit Key ID not configured, skipping animated artwork");
            return Ok(ArtworkResult {
                square_downloaded: false,
                portrait_downloaded: false,
            });
        }
    };

    // Private key is stored in the OS keychain (sensitive).
    let private_key = match get_private_key_from_keychain() {
        Ok(Some(key)) => key,
        Ok(None) => {
            log::debug!("MusicKit private key not stored in keychain, skipping animated artwork");
            return Ok(ArtworkResult {
                square_downloaded: false,
                portrait_downloaded: false,
            });
        }
        Err(e) => {
            log::warn!("Failed to read MusicKit private key from keychain: {}", e);
            return Ok(ArtworkResult {
                square_downloaded: false,
                portrait_downloaded: false,
            });
        }
    };

    // --- Step 2: Parse the Apple Music URL to extract storefront and album ID ---
    let parsed = urls
        .iter()
        .find_map(|url| parse_apple_music_url(url));

    let parsed = match parsed {
        Some(p) => p,
        None => {
            log::debug!("No album URL found in download URLs, skipping animated artwork");
            return Ok(ArtworkResult {
                square_downloaded: false,
                portrait_downloaded: false,
            });
        }
    };

    // --- Step 3: Generate MusicKit JWT ---
    let jwt = generate_musickit_jwt(&team_id, &key_id, &private_key)?;

    // --- Step 4: Query Apple Music API for animated artwork URLs ---
    let artwork_urls = fetch_animated_artwork_urls(&jwt, &parsed.storefront, &parsed.album_id).await?;

    let artwork_urls = match artwork_urls {
        Some(urls) => urls,
        None => {
            log::debug!(
                "No animated artwork available for album {} (storefront: {})",
                parsed.album_id,
                parsed.storefront
            );
            return Ok(ArtworkResult {
                square_downloaded: false,
                portrait_downloaded: false,
            });
        }
    };

    // --- Step 5: Download HLS streams via FFmpeg ---
    let output_path = Path::new(output_dir);
    let mut result = ArtworkResult {
        square_downloaded: false,
        portrait_downloaded: false,
    };

    // Download square artwork (FrontCover.mp4)
    if let Some(ref square_url) = artwork_urls.square {
        let dest = output_path.join("FrontCover.mp4");
        match download_hls_to_mp4(app, square_url, &dest).await {
            Ok(()) => {
                log::info!("Downloaded square animated artwork to {}", dest.display());
                result.square_downloaded = true;
            }
            Err(e) => {
                log::warn!("Failed to download square animated artwork: {}", e);
            }
        }
    }

    // Download portrait artwork (PortraitCover.mp4)
    if let Some(ref tall_url) = artwork_urls.tall {
        let dest = output_path.join("PortraitCover.mp4");
        match download_hls_to_mp4(app, tall_url, &dest).await {
            Ok(()) => {
                log::info!("Downloaded portrait animated artwork to {}", dest.display());
                result.portrait_downloaded = true;
            }
            Err(e) => {
                log::warn!("Failed to download portrait animated artwork: {}", e);
            }
        }
    }

    Ok(result)
}

// ============================================================
// JWT Generation
// ============================================================

/// Generate a short-lived MusicKit Developer Token (ES256-signed JWT).
///
/// Apple's MusicKit API requires a JWT signed with the developer's private
/// key (P8 format, ECDSA P-256). The JWT contains:
/// - Header: `alg: ES256`, `kid: {key_id}`, `typ: JWT`
/// - Claims: `iss: {team_id}`, `iat: {now}`, `exp: {now + 1 hour}`
///
/// The 1-hour expiry is well within Apple's 6-month maximum. We use short
/// expiry because tokens are generated fresh per-request and never cached.
///
/// # Arguments
/// * `team_id` - Apple Developer Team ID (10-character alphanumeric)
/// * `key_id` - MusicKit Key ID (10-character alphanumeric)
/// * `private_key_pem` - Content of the `.p8` private key file (PEM format)
///
/// # Returns
/// * `Ok(String)` - The signed JWT string
/// * `Err(String)` - If the key is invalid or signing fails
///
/// # Reference
/// https://developer.apple.com/documentation/applemusicapi/generating_developer_tokens
fn generate_musickit_jwt(
    team_id: &str,
    key_id: &str,
    private_key_pem: &str,
) -> Result<String, String> {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

    // Build the JWT header with the MusicKit key ID.
    // Apple requires `alg: ES256` and `kid: {key_id}` in the header.
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(key_id.to_string());
    header.typ = Some("JWT".to_string());

    // Calculate timestamps for the JWT claims.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("System time error: {}", e))?
        .as_secs();

    // Build the JWT claims: issuer (team ID), issued-at, and expiry.
    let claims = serde_json::json!({
        "iss": team_id,
        "iat": now,
        "exp": now + 3600,  // 1 hour from now
    });

    // Parse the PEM private key and sign the JWT.
    // Apple's `.p8` files are PKCS#8 PEM-encoded EC private keys.
    let encoding_key = EncodingKey::from_ec_pem(private_key_pem.as_bytes())
        .map_err(|e| format!("Invalid MusicKit private key: {}", e))?;

    encode(&header, &claims, &encoding_key)
        .map_err(|e| format!("Failed to sign MusicKit JWT: {}", e))
}

// ============================================================
// Apple Music API
// ============================================================

/// Query the Apple Music catalog API for animated artwork HLS URLs.
///
/// Makes a GET request to the Apple Music API with `extend=editorialVideo`
/// to retrieve motion artwork URLs for the specified album.
///
/// # Arguments
/// * `jwt` - MusicKit Developer Token (signed JWT)
/// * `storefront` - Two-letter country code (e.g., "us")
/// * `album_id` - Numeric album identifier
///
/// # Returns
/// * `Ok(Some(ArtworkUrls))` - Album has animated artwork (one or both types)
/// * `Ok(None)` - Album has no animated artwork (normal; many albums don't)
/// * `Err(String)` - API request or parsing failure
async fn fetch_animated_artwork_urls(
    jwt: &str,
    storefront: &str,
    album_id: &str,
) -> Result<Option<ArtworkUrls>, String> {
    let url = format!(
        "https://amp-api.music.apple.com/v1/catalog/{}/albums/{}?extend=editorialVideo",
        storefront, album_id
    );

    log::debug!("Querying Apple Music API for animated artwork: {}", url);

    // Make the authenticated API request.
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", jwt))
        .header("User-Agent", "meedyadl")
        .header("Origin", "https://music.apple.com")
        .send()
        .await
        .map_err(|e| format!("Apple Music API request failed: {}", e))?;

    // Handle HTTP error responses.
    if !response.status().is_success() {
        let status = response.status().as_u16();
        // 404 = album not found; 401/403 = invalid/expired token
        return Err(format!(
            "Apple Music API returned HTTP {} for album {}",
            status, album_id
        ));
    }

    // Parse the JSON response.
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Apple Music API response: {}", e))?;

    // Navigate to the editorialVideo object within the first album's attributes.
    // Path: data[0].attributes.editorialVideo
    let editorial_video = json
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("attributes"))
        .and_then(|a| a.get("editorialVideo"));

    // If there's no editorialVideo field, this album has no animated artwork.
    let editorial_video = match editorial_video {
        Some(ev) => ev,
        None => return Ok(None),
    };

    // Extract the M3U8 HLS URLs for square and tall formats.
    let square = editorial_video
        .get("motionDetailSquare")
        .and_then(|m| m.get("video"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let tall = editorial_video
        .get("motionDetailTall")
        .and_then(|m| m.get("video"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // If neither format is available, return None.
    if square.is_none() && tall.is_none() {
        return Ok(None);
    }

    Ok(Some(ArtworkUrls { square, tall }))
}

// ============================================================
// HLS Download via FFmpeg
// ============================================================

/// Download an HLS stream to an MP4 file using FFmpeg.
///
/// Uses FFmpeg's native HLS protocol support to download the M3U8 playlist
/// and all its segments, then remuxes them into a single MP4 file without
/// re-encoding (`-c copy`).
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving the managed FFmpeg binary path
/// * `m3u8_url` - The HLS playlist URL (from Apple Music API response)
/// * `output_path` - Destination file path (e.g., `.../FrontCover.mp4`)
///
/// # Returns
/// * `Ok(())` - FFmpeg completed successfully
/// * `Err(String)` - FFmpeg not installed, failed to spawn, or exited with error
fn get_ffmpeg_path(app: &AppHandle) -> Result<PathBuf, String> {
    let ffmpeg_bin = dependency_manager::get_tool_binary_path(app, "ffmpeg");
    if !ffmpeg_bin.exists() {
        return Err("FFmpeg not installed — required for animated artwork download".to_string());
    }
    Ok(ffmpeg_bin)
}

async fn download_hls_to_mp4(
    app: &AppHandle,
    m3u8_url: &str,
    output_path: &Path,
) -> Result<(), String> {
    let ffmpeg_bin = get_ffmpeg_path(app)?;

    log::debug!(
        "Downloading HLS stream to {}: {}",
        output_path.display(),
        m3u8_url
    );

    // Run FFmpeg to download the HLS stream and remux to MP4.
    // Flags:
    //   -i {url}          -- input HLS stream
    //   -c copy           -- copy streams without re-encoding (preserves HEVC quality)
    //   -movflags +faststart -- move moov atom to start for faster playback
    //   -y                -- overwrite output file if it exists
    //   -loglevel warning -- suppress verbose output, only show warnings/errors
    let output = Command::new(&ffmpeg_bin)
        .args([
            "-i",
            m3u8_url,
            "-c",
            "copy",
            "-movflags",
            "+faststart",
            "-y",
            "-loglevel",
            "warning",
        ])
        .arg(output_path)
        .output()
        .await
        .map_err(|e| format!("Failed to spawn FFmpeg: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Clean up partial file on failure
        let _ = std::fs::remove_file(output_path);
        return Err(format!("FFmpeg failed: {}", stderr.trim()));
    }

    Ok(())
}

// ============================================================
// URL Parsing
// ============================================================

/// Parse an Apple Music URL to extract the storefront and album ID.
///
/// Supports these URL patterns:
/// - `https://music.apple.com/us/album/album-name/1234567890`
/// - `https://music.apple.com/us/album/album-name/1234567890?i=9876543210`
///   (single track URL — we extract the album ID, not the track ID)
///
/// # Arguments
/// * `url` - An Apple Music URL string
///
/// # Returns
/// * `Some(ParsedAlbumUrl)` - Successfully extracted storefront and album ID
/// * `None` - URL doesn't match the Apple Music album pattern
fn parse_apple_music_url(url: &str) -> Option<ParsedAlbumUrl> {
    // Regex matches: //{storefront}/album/{slug}/{album_id}
    // The storefront is a 2-letter country code.
    // The album_id is a numeric string.
    // Optional ?i=trackId query parameter is ignored (we want the album ID).
    let re = Regex::new(r"https?://music\.apple\.com/([a-z]{2})/album/[^/]+/(\d+)")
        .expect("Invalid regex");

    re.captures(url).map(|caps| ParsedAlbumUrl {
        storefront: caps[1].to_string(),
        album_id: caps[2].to_string(),
    })
}

// ============================================================
// Keychain Integration
// ============================================================

/// Retrieve the MusicKit private key from the OS keychain.
///
/// Uses the same `keyring` crate and service name as the rest of MeedyaDL's
/// credential system. The key is stored under:
///   Service: "io.github.meedyadl"
///   Account: "musickit_private_key"
///
/// # Returns
/// * `Ok(Some(String))` - Private key PEM content found
/// * `Ok(None)` - No key stored (user hasn't configured it yet)
/// * `Err(String)` - Keychain access error (locked, permission denied, etc.)
fn get_private_key_from_keychain() -> Result<Option<String>, String> {
    const SERVICE_NAME: &str = "io.github.meedyadl";
    const KEY_NAME: &str = "musickit_private_key";

    let entry = keyring::Entry::new(SERVICE_NAME, KEY_NAME)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;

    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Failed to retrieve MusicKit private key: {}", e)),
    }
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

    /// Verifies that a standard Apple Music album URL is parsed correctly,
    /// extracting the storefront and album ID.
    #[test]
    fn parse_standard_album_url() {
        let url = "https://music.apple.com/us/album/midnights/1649434004";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.album_id, "1649434004");
    }

    /// Verifies that a single-track URL (with ?i= parameter) extracts the
    /// album ID, not the track ID. Animated artwork is album-level.
    #[test]
    fn parse_single_track_url_extracts_album_id() {
        let url = "https://music.apple.com/gb/album/anti-hero/1649434004?i=1649434280";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "gb");
        assert_eq!(parsed.album_id, "1649434004");
    }

    /// Verifies that a Japanese storefront URL is parsed correctly.
    #[test]
    fn parse_non_us_storefront() {
        let url = "https://music.apple.com/jp/album/some-album/9876543210";
        let result = parse_apple_music_url(url);
        assert!(result.is_some());
        let parsed = result.unwrap();
        assert_eq!(parsed.storefront, "jp");
        assert_eq!(parsed.album_id, "9876543210");
    }

    /// Verifies that a playlist URL returns None (not an album URL).
    #[test]
    fn parse_playlist_url_returns_none() {
        let url = "https://music.apple.com/us/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb";
        let result = parse_apple_music_url(url);
        assert!(result.is_none());
    }

    /// Verifies that a music video URL returns None (not an album URL).
    #[test]
    fn parse_music_video_url_returns_none() {
        let url = "https://music.apple.com/us/music-video/anti-hero/1649434280";
        let result = parse_apple_music_url(url);
        assert!(result.is_none());
    }

    /// Verifies that an artist URL returns None (not an album URL).
    #[test]
    fn parse_artist_url_returns_none() {
        let url = "https://music.apple.com/us/artist/taylor-swift/159260351";
        let result = parse_apple_music_url(url);
        assert!(result.is_none());
    }

    /// Verifies that a completely unrelated URL returns None.
    #[test]
    fn parse_non_apple_music_url_returns_none() {
        let url = "https://www.example.com/some/path";
        let result = parse_apple_music_url(url);
        assert!(result.is_none());
    }

    /// Verifies that an empty string returns None.
    #[test]
    fn parse_empty_string_returns_none() {
        let result = parse_apple_music_url("");
        assert!(result.is_none());
    }

    // ----------------------------------------------------------
    // JWT structure tests
    // ----------------------------------------------------------

    /// Verifies that the JWT generation function produces a string with
    /// three dot-separated parts (header.payload.signature), which is the
    /// standard JWT format.
    ///
    /// Note: This test uses a generated EC P-256 key in PEM format. It does
    /// NOT test against Apple's actual API (that would be an integration test).
    #[test]
    fn generate_jwt_produces_three_part_token() {
        // A test EC P-256 private key in PEM format (NOT a real MusicKit key).
        // Generated for testing purposes only -- never used in production.
        let test_key = "-----BEGIN PRIVATE KEY-----\n\
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgeh6KDqvJ79pjAOBV\n\
aSqMvySOY7Z/xSeiIvUA6uSA0a2hRANCAATuO7iI++EWLlqR8bBjpW3tGnOQnNXi\n\
FJPkH0mNKDTBHi2UUm8qku8mDfB7vmFMjIbzhMqurhYu6/mjzGKIADEv\n\
-----END PRIVATE KEY-----";

        let result = generate_musickit_jwt("TEAM123456", "KEY1234567", test_key);
        assert!(result.is_ok(), "JWT generation failed: {:?}", result.err());

        let token = result.unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3, "JWT should have 3 parts (header.payload.signature)");

        // Verify each part is non-empty
        assert!(!parts[0].is_empty(), "JWT header should not be empty");
        assert!(!parts[1].is_empty(), "JWT payload should not be empty");
        assert!(!parts[2].is_empty(), "JWT signature should not be empty");
    }

    /// Verifies that an invalid PEM key produces a descriptive error.
    #[test]
    fn generate_jwt_rejects_invalid_key() {
        let result = generate_musickit_jwt("TEAM123456", "KEY1234567", "not a valid PEM key");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("Invalid MusicKit private key"),
            "Error should mention invalid key: {}",
            err
        );
    }

    // ----------------------------------------------------------
    // ArtworkResult serialization tests
    // ----------------------------------------------------------

    /// Verifies that ArtworkResult serializes to the expected JSON format
    /// for the frontend to consume.
    #[test]
    fn artwork_result_serializes_correctly() {
        let result = ArtworkResult {
            square_downloaded: true,
            portrait_downloaded: false,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"square_downloaded\":true"));
        assert!(json.contains("\"portrait_downloaded\":false"));
    }
}
