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
// catalog queries. Shared authentication logic (JWT generation, keychain
// access, URL parsing) is provided by `apple_music_api.rs`.
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
//
// @see apple_music_api.rs -- Shared MusicKit auth and API client

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::process::Command;

use crate::services::{apple_music_api, config_service, dependency_manager};
use crate::services::apple_music_api::AlbumMetadata;

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

/// Default result with both artwork types not downloaded.
fn empty_result() -> ArtworkResult {
    ArtworkResult {
        square_downloaded: false,
        portrait_downloaded: false,
    }
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
        return Ok(empty_result());
    }

    // Team ID and Key ID are stored in settings (non-sensitive).
    let team_id = match &settings.musickit_team_id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => {
            log::debug!("MusicKit Team ID not configured, skipping animated artwork");
            return Ok(empty_result());
        }
    };

    let key_id = match &settings.musickit_key_id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => {
            log::debug!("MusicKit Key ID not configured, skipping animated artwork");
            return Ok(empty_result());
        }
    };

    // Private key is stored in the OS keychain (sensitive).
    let private_key = match apple_music_api::get_private_key_from_keychain() {
        Ok(Some(key)) => key,
        Ok(None) => {
            log::debug!("MusicKit private key not stored in keychain, skipping animated artwork");
            return Ok(empty_result());
        }
        Err(e) => {
            log::warn!("Failed to read MusicKit private key from keychain: {}", e);
            return Ok(empty_result());
        }
    };

    // --- Step 2: Parse the Apple Music URL to extract storefront and album ID ---
    let parsed = urls
        .iter()
        .find_map(|url| apple_music_api::parse_apple_music_url(url));

    let parsed = match parsed {
        Some(p) if p.content_type == "album" => p,
        _ => {
            log::debug!("No album URL found in download URLs, skipping animated artwork");
            return Ok(empty_result());
        }
    };

    // --- Step 3: Generate MusicKit JWT ---
    let jwt = apple_music_api::generate_musickit_jwt(&team_id, &key_id, &private_key)?;

    // --- Step 4: Query Apple Music API for album metadata (includes artwork URLs) ---
    let metadata = apple_music_api::fetch_album_metadata(
        &jwt,
        &parsed.storefront,
        &parsed.album_id,
    ).await?;

    let metadata = match metadata {
        Some(m) => m,
        None => {
            log::debug!(
                "No metadata returned for album {} (storefront: {})",
                parsed.album_id,
                parsed.storefront
            );
            return Ok(empty_result());
        }
    };

    // --- Step 5: Download artwork using the fetched metadata ---
    download_artwork_from_metadata(app, &metadata, output_dir).await
}

/// Download animated artwork using pre-fetched album metadata.
///
/// This alternative entry point skips the API call when album metadata
/// has already been fetched by the metadata enrichment service. Avoids
/// making duplicate API requests when both metadata enrichment and
/// animated artwork are enabled.
///
/// # Arguments
/// * `app` - Tauri AppHandle for accessing settings and tool paths
/// * `metadata` - Pre-fetched album metadata from `apple_music_api::fetch_album_metadata()`
/// * `output_dir` - The album output directory where audio files were saved
///
/// # Returns
/// * `Ok(ArtworkResult)` - Which artwork types were downloaded
/// * `Err(String)` - Only for unexpected failures
pub async fn process_album_artwork_from_metadata(
    app: &AppHandle,
    metadata: &AlbumMetadata,
    output_dir: &str,
) -> Result<ArtworkResult, String> {
    // Check if feature is enabled
    let settings = config_service::load_settings(app).unwrap_or_default();
    if !settings.animated_artwork_enabled {
        log::debug!("Animated artwork disabled in settings");
        return Ok(empty_result());
    }

    download_artwork_from_metadata(app, metadata, output_dir).await
}

// ============================================================
// Internal: Artwork Download Logic
// ============================================================

/// Download artwork files from the HLS URLs in the album metadata.
///
/// Shared implementation used by both `process_album_artwork()` and
/// `process_album_artwork_from_metadata()`.
async fn download_artwork_from_metadata(
    app: &AppHandle,
    metadata: &AlbumMetadata,
    output_dir: &str,
) -> Result<ArtworkResult, String> {
    // Check if any artwork URLs are available
    if metadata.artwork_square_url.is_none() && metadata.artwork_tall_url.is_none() {
        log::debug!(
            "No animated artwork available for album {}",
            metadata.album_id
        );
        return Ok(empty_result());
    }

    let output_path = Path::new(output_dir);
    let mut result = empty_result();

    // Download square artwork (FrontCover.mp4)
    if let Some(ref square_url) = metadata.artwork_square_url {
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
    if let Some(ref tall_url) = metadata.artwork_tall_url {
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
// HLS Download via FFmpeg
// ============================================================

/// Resolve the managed FFmpeg binary path.
fn get_ffmpeg_path(app: &AppHandle) -> Result<PathBuf, String> {
    let ffmpeg_bin = dependency_manager::get_tool_binary_path(app, "ffmpeg");
    if !ffmpeg_bin.exists() {
        return Err("FFmpeg not installed — required for animated artwork download".to_string());
    }
    Ok(ffmpeg_bin)
}

/// Download an HLS stream to an MP4 file using FFmpeg.
///
/// Uses FFmpeg's native HLS protocol support to download the M3U8 playlist
/// and all its segments, then remuxes them into a single MP4 file without
/// re-encoding (`-c copy`).
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
// File Hiding (Platform-Specific)
// ============================================================

/// Set the OS "hidden" attribute on an animated artwork file.
///
/// This keeps album folders clean by hiding companion video files from
/// default file browser views, while preserving the original filenames
/// on macOS and Windows so media players can still find them by name.
///
/// # Platform behavior
///
/// - **macOS**: Uses `chflags hidden` which sets the `UF_HIDDEN` flag.
///   Files are hidden in Finder but visible with `ls -la` and retain
///   their original filename.
/// - **Windows**: Uses `attrib +H` which sets the Win32 hidden attribute.
///   Files are hidden in Explorer but visible with `dir /a:h` and retain
///   their original filename.
/// - **Linux**: Renames the file with a `.` prefix (e.g., `FrontCover.mp4`
///   → `.FrontCover.mp4`). This is the only standard mechanism on Linux
///   but it changes the filename, so software looking for `FrontCover.mp4`
///   by name will not find it.
pub async fn hide_file(file_path: &Path) -> Result<(), String> {
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", file_path.display()));
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: use chflags hidden to set UF_HIDDEN without renaming
        let output = Command::new("chflags")
            .arg("hidden")
            .arg(file_path)
            .output()
            .await
            .map_err(|e| format!("Failed to run chflags: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("chflags hidden failed: {}", stderr.trim()));
        }
        log::debug!("Set hidden flag on {}", file_path.display());
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: use attrib +H to set the hidden attribute without renaming
        let output = Command::new("attrib")
            .arg("+H")
            .arg(file_path)
            .output()
            .await
            .map_err(|e| format!("Failed to run attrib: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("attrib +H failed: {}", stderr.trim()));
        }
        log::debug!("Set hidden attribute on {}", file_path.display());
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: rename with dot prefix (only standard hiding mechanism)
        if let Some(filename) = file_path.file_name().and_then(|f| f.to_str()) {
            if !filename.starts_with('.') {
                let hidden_name = format!(".{}", filename);
                let hidden_path = file_path.with_file_name(&hidden_name);
                std::fs::rename(file_path, &hidden_path)
                    .map_err(|e| format!("Failed to rename to {}: {}", hidden_name, e))?;
                log::debug!(
                    "Renamed {} to {} (Linux hidden)",
                    file_path.display(),
                    hidden_path.display()
                );
            }
        }
    }

    Ok(())
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Verifies the empty_result helper returns both false.
    #[test]
    fn empty_result_has_both_false() {
        let result = empty_result();
        assert!(!result.square_downloaded);
        assert!(!result.portrait_downloaded);
    }
}
