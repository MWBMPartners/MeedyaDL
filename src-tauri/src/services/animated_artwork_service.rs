// Copyright (c) 2026 MeedyaDL
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
//    - `FrontCover.mp4`         -- square (1:1),  from `motionDetailSquare`
//    - `FrontCoverPortrait.mp4` -- portrait (3:4), from `motionDetailTall`
//
//    The naming pair (`FrontCover` + `FrontCoverPortrait`) keeps both
//    variants adjacent in an alphabetical listing and makes the
//    portrait file self-describing about its source (the same album
//    cover, rotated/reframed for vertical layout).
//
// ## Authentication
//
// The Apple Music API requires a MusicKit Developer Token (JWT) for
// catalog queries. Shared authentication logic (JWT generation, keychain
// access, URL parsing) is provided by `apple_music_api.rs`.
//
// ## Output files
//
// | Artwork Type | Filename                  | Aspect Ratio | Max Resolution |
// |--------------|---------------------------|--------------|----------------|
// | Square       | `FrontCover.mp4`          | 1:1          | 3840x3840      |
// | Portrait     | `FrontCoverPortrait.mp4`  | 3:4          | 2048x2732      |
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

use crate::services::apple_music_api::AlbumMetadata;
use crate::services::{apple_music_api, config_service, dependency_manager};

// ============================================================
// Public Types
// ============================================================

/// Per-variant outcome of an animated-artwork download attempt
/// (#529). Replaces the pre-#529 `bool` flags which collapsed
/// "API didn't offer this variant" and "API offered it but
/// download failed" into the same `false` value, causing the
/// activity log to lie ("No animated artwork available") when the
/// downloads had actually failed.
///
/// The frontend uses the discriminant to render success/skip/error
/// indicators in the queue UI; the embedded fields drive the
/// activity-log emissions in `download_queue.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VariantStatus {
    /// Apple Music did not offer this variant for this album.
    /// Distinct from `DownloadFailed` because no fix is possible.
    NotOffered,
    /// File landed on disk and passed the post-download verify
    /// (exists + size > 0). `path` is absolute; `size_bytes` is
    /// the on-disk file size at verification time.
    Downloaded { path: String, size_bytes: u64 },
    /// API offered the variant but the download itself failed —
    /// either FFmpeg returned an error, FFmpeg returned success but
    /// produced a missing / zero-byte file, or a post-rename step
    /// (hide-file) failed.
    DownloadFailed { url: String, reason: String },
}

impl VariantStatus {
    /// Convenience: whether the variant landed on disk successfully.
    /// Preserves the pre-#529 semantics of the `*_downloaded` bool
    /// flag for callers that just want a yes/no.
    #[must_use]
    pub fn is_downloaded(&self) -> bool {
        matches!(self, Self::Downloaded { .. })
    }
}

/// Result of an animated artwork download attempt.
///
/// Serialized to JSON and returned to the frontend via the
/// `download_animated_artwork` Tauri command. The frontend can
/// use the per-variant `VariantStatus` to display the right
/// indicator (success / not-offered / failed) in the queue UI;
/// the activity-log emitter in `download_queue.rs` uses it to
/// emit one tailored line per variant (#529).
///
/// Backwards compatibility: the pre-#529 `square_downloaded` /
/// `portrait_downloaded` bool fields are preserved as serde-
/// flattened getters so JSON consumers that only know the old
/// shape (e.g. older `latest.json`-style frontend builds) keep
/// working. New code should read `square` / `portrait` directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtworkResult {
    /// Square (1:1) animated cover — destined for FrontCover.mp4
    pub square: VariantStatus,
    /// Portrait (3:4) animated cover — destined for FrontCoverPortrait.mp4
    pub portrait: VariantStatus,
    /// Album-level 16:9 spotlight video (#538) — destined for
    /// AlbumSpotlightCover.mp4 in the album folder. Distinct from
    /// the artist-page spotlight at `ArtistSpotlightCover.mp4`
    /// which lives in the parent artist folder and is shared
    /// across the artist's whole catalogue.
    #[serde(default = "default_not_offered")]
    pub spotlight: VariantStatus,
    /// LEGACY mirror of `square.is_downloaded()` for back-compat
    /// with pre-#529 JSON consumers. Always serialised; ignored
    /// on deserialise (derived from `square`).
    #[serde(default)]
    pub square_downloaded: bool,
    /// LEGACY mirror of `portrait.is_downloaded()` for back-compat
    /// with pre-#529 JSON consumers.
    #[serde(default)]
    pub portrait_downloaded: bool,
}

/// Serde default helper for the new `spotlight` field (#538) so
/// older JSON payloads written by pre-#538 builds load cleanly.
fn default_not_offered() -> VariantStatus {
    VariantStatus::NotOffered
}

impl ArtworkResult {
    /// Builder helper that fills in the legacy `*_downloaded`
    /// mirror fields from the per-variant statuses, so call sites
    /// only need to set `square` / `portrait` / `spotlight`.
    pub(crate) fn with_variants(
        square: VariantStatus,
        portrait: VariantStatus,
        spotlight: VariantStatus,
    ) -> Self {
        let square_downloaded = square.is_downloaded();
        let portrait_downloaded = portrait.is_downloaded();
        Self {
            square,
            portrait,
            spotlight,
            square_downloaded,
            portrait_downloaded,
        }
    }
}

/// Default result with all variants marked `NotOffered` — used
/// by the early-return graceful-exit paths (feature disabled,
/// credentials missing, non-album URL, etc.).
fn empty_result() -> ArtworkResult {
    ArtworkResult::with_variants(
        VariantStatus::NotOffered,
        VariantStatus::NotOffered,
        VariantStatus::NotOffered,
    )
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
/// * `app` - Tauri `AppHandle` for accessing settings, keychain, and tool paths
/// * `urls` - The Apple Music URL(s) from the download request
/// * `output_dir` - The album output directory where audio files were saved
///
/// # Returns
/// * `Ok(ArtworkResult)` - Which artwork types were downloaded (may be both false)
/// * `Err(String)` - Only for unexpected failures (not "no artwork available")
///
/// # Errors
///
/// Returns `Err(String)` if settings cannot be loaded, the Apple Music API
/// request fails, or `FFmpeg` download of the artwork fails.
///
/// # Graceful exits (returns Ok with both false):
/// * Feature disabled in settings
/// * `MusicKit` credentials not configured
/// * URL is not an album URL (single track, playlist, music video)
/// * Album has no animated artwork
/// * `FFmpeg` not installed
pub async fn process_album_artwork(
    app: &AppHandle,
    urls: &[String],
    output_dir: &str,
) -> Result<ArtworkResult, String> {
    // --- Step 1: Check if feature is enabled and credentials are configured ---
    let settings = config_service::load_settings(app).unwrap_or_default();

    if !settings.animated_artwork_enabled {
        log::info!("Animated artwork disabled in settings");
        return Ok(empty_result());
    }

    // Team ID / Key ID are non-sensitive settings fields. Private key is
    // sensitive and read from OS keychain. If these are incomplete, we may
    // still continue if a build-time embedded MusicKit token is available.
    let team_id = settings.musickit_team_id.as_deref();
    let key_id = settings.musickit_key_id.as_deref();
    let private_key = match apple_music_api::get_private_key_from_keychain() {
        Ok(Some(key)) => Some(key),
        Ok(None) => None,
        Err(e) => {
            log::warn!("Failed to read MusicKit private key from keychain: {e}");
            None
        }
    };

    // --- Step 2: Parse the Apple Music URL to extract storefront and album ID ---
    let parsed = urls
        .iter()
        .find_map(|url| apple_music_api::parse_apple_music_url(url));

    let parsed = match parsed {
        Some(p) if p.content_type == "album" => p,
        _ => {
            log::info!("No album URL found in download URLs, skipping animated artwork");
            return Ok(empty_result());
        }
    };

    // --- Step 3: Resolve MusicKit developer token (premium feature resolver with web player fallback) ---
    let (jwt, token_source) = match apple_music_api::resolve_premium_feature_token(
        team_id,
        key_id,
        private_key.as_deref(),
    )? {
        Some(pair) => pair,
        None => {
            log::info!(
                "No MusicKit token available (user creds / embedded / web player), skipping animated artwork"
            );
            return Ok(empty_result());
        }
    };

    log::debug!("Animated artwork: using MusicKit token from {token_source}");

    // --- Step 4: Query Apple Music API for album metadata (includes artwork URLs) ---
    let metadata = apple_music_api::fetch_album_metadata_with_fallback(
        &jwt,
        &parsed.storefront,
        &parsed.album_id,
    )
    .await?;

    let Some(metadata) = metadata else {
        log::debug!(
            "No metadata returned for album {} (storefront: {})",
            parsed.album_id,
            parsed.storefront
        );
        return Ok(empty_result());
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
/// * `app` - Tauri `AppHandle` for accessing settings and tool paths
/// * `metadata` - Pre-fetched album metadata from `apple_music_api::fetch_album_metadata()`
/// * `output_dir` - The album output directory where audio files were saved
///
/// # Errors
///
/// Returns `Err(String)` if settings cannot be loaded, the `FFmpeg` binary
/// is missing, or the HLS download fails.
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
        log::info!("Animated artwork disabled in settings");
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
    // Both variants start as `NotOffered`; the URL-present arms
    // below flip them to `Downloaded` or `DownloadFailed` based on
    // the actual outcome. This is the #529 fix — the pre-#529 code
    // collapsed "API didn't offer" and "API offered but download
    // failed" into the same `false` flag, producing the lying
    // "No animated artwork available" activity-log line.
    if metadata.artwork_square_url.is_none()
        && metadata.artwork_tall_url.is_none()
        && metadata.album_spotlight_url.is_none()
    {
        log::info!(
            "No animated artwork available for album {}",
            metadata.album_id
        );
        return Ok(empty_result());
    }

    let output_path = Path::new(output_dir);

    let square_status = match metadata.artwork_square_url.as_deref() {
        Some(square_url) => {
            let dest = output_path.join("FrontCover.mp4");
            attempt_artwork_variant(app, square_url, &dest, "square").await
        }
        None => VariantStatus::NotOffered,
    };

    let portrait_status = match metadata.artwork_tall_url.as_deref() {
        Some(tall_url) => {
            let dest = output_path.join("FrontCoverPortrait.mp4");
            attempt_artwork_variant(app, tall_url, &dest, "portrait").await
        }
        None => VariantStatus::NotOffered,
    };

    // #538: Album-level 16:9 spotlight video. Saved to the album
    // folder as AlbumSpotlightCover.mp4 — distinct from the
    // artist-page ArtistSpotlightCover.mp4 (which lives in the
    // artist folder and is the same across the artist's whole
    // catalogue). Same FFmpeg HLS pipeline as the cover variants.
    let spotlight_status = match metadata.album_spotlight_url.as_deref() {
        Some(spotlight_url) => {
            let dest = output_path.join("AlbumSpotlightCover.mp4");
            attempt_artwork_variant(app, spotlight_url, &dest, "album spotlight").await
        }
        None => VariantStatus::NotOffered,
    };

    Ok(ArtworkResult::with_variants(
        square_status,
        portrait_status,
        spotlight_status,
    ))
}

/// Downloads one HLS animated-artwork variant and runs the
/// post-download verification that #529 adds. Returns:
///
/// - `Downloaded { path, size_bytes }` when FFmpeg succeeded
///   AND the destination file exists with size > 0.
/// - `DownloadFailed { url, reason }` in three cases:
///     * FFmpeg returned an error.
///     * FFmpeg returned success but the destination is missing.
///     * FFmpeg returned success but the destination is zero bytes
///       (FFmpeg occasionally produces empty outputs on broken
///       upstream HLS playlists without flagging an error).
///
/// `variant_label` is just for log readability — `"square"` or
/// `"portrait"`. The activity-log emission lives in
/// `download_queue.rs` (so the writer has access to the
/// download-id); this helper only writes to `log::info!` /
/// `log::warn!` for the tracing log.
async fn attempt_artwork_variant(
    app: &AppHandle,
    m3u8_url: &str,
    dest: &Path,
    variant_label: &'static str,
) -> VariantStatus {
    match download_hls_to_mp4(app, m3u8_url, dest).await {
        Ok(()) => {
            // FFmpeg returned 0 — verify the file is real (#529).
            match std::fs::metadata(dest) {
                Ok(meta) if meta.len() > 0 => {
                    log::info!(
                        "Downloaded {variant_label} animated artwork to {} ({} bytes)",
                        dest.display(),
                        meta.len(),
                    );
                    VariantStatus::Downloaded {
                        path: dest.display().to_string(),
                        size_bytes: meta.len(),
                    }
                }
                Ok(meta) => {
                    // size == 0 — FFmpeg lied.
                    log::warn!(
                        "{variant_label} animated artwork: FFmpeg reported success but file is empty (0 bytes) at {}",
                        dest.display(),
                    );
                    VariantStatus::DownloadFailed {
                        url: m3u8_url.to_string(),
                        reason: format!(
                            "FFmpeg reported success but the output file is empty ({} bytes)",
                            meta.len()
                        ),
                    }
                }
                Err(e) => {
                    // File missing after a "successful" FFmpeg run.
                    log::warn!(
                        "{variant_label} animated artwork: FFmpeg reported success but file is missing at {}: {e}",
                        dest.display(),
                    );
                    VariantStatus::DownloadFailed {
                        url: m3u8_url.to_string(),
                        reason: format!(
                            "FFmpeg reported success but the output file is missing: {e}"
                        ),
                    }
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to download {variant_label} animated artwork: {e}");
            VariantStatus::DownloadFailed {
                url: m3u8_url.to_string(),
                reason: e,
            }
        }
    }
}

// ============================================================
// HLS Download via FFmpeg
// ============================================================

/// Resolve the managed `FFmpeg` binary path.
fn get_ffmpeg_path(app: &AppHandle) -> Result<PathBuf, String> {
    let ffmpeg_bin = dependency_manager::get_tool_binary_path(app, "ffmpeg");
    if !ffmpeg_bin.exists() {
        return Err("FFmpeg not installed — required for animated artwork download".to_string());
    }
    Ok(ffmpeg_bin)
}

/// Download an HLS stream to an MP4 file using `FFmpeg`.
///
/// Uses `FFmpeg`'s native HLS protocol support to download the M3U8 playlist
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
        .map_err(|e| format!("Failed to spawn FFmpeg: {e}"))?;

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
///
/// # Errors
///
/// Returns `Err(String)` if the file does not exist or the OS-specific hide
/// operation fails (e.g., `chflags` on macOS, `attrib` on Windows, rename on Linux).
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
            .map_err(|e| format!("Failed to run chflags: {e}"))?;

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
        // Linux: rename with dot prefix (only standard hiding mechanism).
        // Collision-proof: if `.FrontCover.mp4` already exists from a
        // previous session, `safe_rename` lands the new file on a
        // disambiguated sibling rather than silently overwriting the
        // earlier hidden artwork.
        if let Some(filename) = file_path.file_name().and_then(|f| f.to_str()) {
            if !filename.starts_with('.') {
                let hidden_name = format!(".{}", filename);
                let hidden_path = file_path.with_file_name(&hidden_name);
                match crate::utils::fs_safe::safe_rename(file_path, &hidden_path) {
                    Ok(final_path) => log::debug!(
                        "Renamed {} to {} (Linux hidden)",
                        file_path.display(),
                        final_path.display()
                    ),
                    Err(e) => {
                        return Err(format!("Failed to rename to {}: {}", hidden_name, e));
                    }
                }
            }
        }
    }

    Ok(())
}

// ============================================================
// Artist Promo Video Download
// ============================================================

/// Download an artist's promotional video to the artist folder.
///
/// Queries the Apple Music API for the artist's `editorialVideo` and
/// downloads the HLS stream as `ArtistSpotlightCover.mp4` to the artist directory
/// (the parent of the album directory).
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for tool paths and settings
/// * `artist_id` - Apple Music artist ID (e.g., "368433979")
/// * `storefront` - Two-letter country code (e.g., "gb")
/// * `album_dir` - Path to the album output directory. The artist folder
///   is derived as its parent (e.g., `/Music/Zedd/Album Name` → `/Music/Zedd/`).
///
/// # Returns
/// * `Ok(true)` - Promo video downloaded successfully
/// * `Ok(false)` - No promo video available, or already exists, or skipped
/// * `Err(String)` - API or download failure
pub async fn download_artist_promo_video(
    app: &AppHandle,
    artist_id: &str,
    storefront: &str,
    album_dir: &str,
) -> Result<bool, String> {
    // Derive the artist directory (parent of the album directory).
    // GAMDL's default template is `{album_artist}/{album}`, so the parent
    // of the album dir is the artist folder.
    let album_path = Path::new(album_dir);
    let artist_dir = match album_path.parent() {
        Some(p) if p.exists() => p,
        _ => {
            log::debug!(
                "Cannot derive artist directory from album dir: {album_dir}"
            );
            return Ok(false);
        }
    };

    let dest = artist_dir.join("ArtistSpotlightCover.mp4");

    // Skip if the promo video already exists (idempotent — don't re-download
    // on every album download from the same artist).
    if dest.exists() {
        log::info!(
            "Artist promo video already exists at {}, skipping",
            dest.display()
        );
        return Ok(false);
    }

    // Resolve MusicKit developer token (premium feature resolver with web player fallback)
    let settings = config_service::load_settings(app).unwrap_or_default();
    let team_id = settings.musickit_team_id.as_deref();
    let key_id = settings.musickit_key_id.as_deref();
    let private_key = match apple_music_api::get_private_key_from_keychain() {
        Ok(Some(key)) => Some(key),
        Ok(None) => None,
        Err(e) => {
            log::warn!("Failed to read MusicKit private key from keychain: {e}");
            None
        }
    };

    let (jwt, token_source) = match apple_music_api::resolve_premium_feature_token(
        team_id,
        key_id,
        private_key.as_deref(),
    )? {
        Some(pair) => pair,
        None => {
            log::info!("No MusicKit token available, skipping artist promo video");
            return Ok(false);
        }
    };

    log::debug!("Artist promo video: using MusicKit token from {token_source}");

    // Fetch artist promo video metadata from the API
    let promo = match apple_music_api::fetch_artist_promo_video(
        &jwt,
        storefront,
        artist_id,
    )
    .await?
    {
        Some(p) => p,
        None => return Ok(false),
    };

    // Download the HLS stream to ArtistSpotlightCover.mp4
    log::info!(
        "Downloading promo video for {} to {}",
        promo.artist_name,
        dest.display()
    );
    download_hls_to_mp4(app, &promo.video_url, &dest).await?;

    log::info!(
        "Artist promo video downloaded: {} → {}",
        promo.artist_name,
        dest.display()
    );

    // Hide the file if the user wants animated artwork hidden
    if settings.hide_animated_artwork {
        if let Err(e) = hide_file(&dest).await {
            log::debug!("Failed to hide ArtistSpotlightCover.mp4: {e}");
        }
    }

    Ok(true)
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
    /// for the frontend to consume — both the new per-variant `square`/
    /// `portrait` enum payloads (#529) and the legacy `*_downloaded`
    /// bool mirrors (back-compat with pre-#529 JSON consumers).
    #[test]
    fn artwork_result_serializes_correctly() {
        let result = ArtworkResult::with_variants(
            VariantStatus::Downloaded {
                path: "/Music/Album/FrontCover.mp4".to_string(),
                size_bytes: 1_234_567,
            },
            VariantStatus::NotOffered,
            VariantStatus::NotOffered,
        );
        let json = serde_json::to_string(&result).unwrap();
        // New per-variant shape — primary source of truth post-#529.
        assert!(json.contains("\"square\""));
        assert!(json.contains("\"kind\":\"downloaded\""));
        assert!(json.contains("\"path\":\"/Music/Album/FrontCover.mp4\""));
        assert!(json.contains("\"size_bytes\":1234567"));
        assert!(json.contains("\"portrait\""));
        assert!(json.contains("\"kind\":\"not_offered\""));
        // #538: spotlight variant carried alongside square/portrait.
        assert!(json.contains("\"spotlight\""));
        // Legacy mirror fields — preserved so older JSON consumers
        // still see the bool flags they expect.
        assert!(json.contains("\"square_downloaded\":true"));
        assert!(json.contains("\"portrait_downloaded\":false"));
    }

    /// #538 — Verify that the spotlight variant serialises as a
    /// real `VariantStatus` enum payload, not a placeholder
    /// `NotOffered` only. Round-trips a `Downloaded` spotlight
    /// alongside `NotOffered` square + portrait.
    #[test]
    fn artwork_result_serialises_spotlight_variant() {
        let result = ArtworkResult::with_variants(
            VariantStatus::NotOffered,
            VariantStatus::NotOffered,
            VariantStatus::Downloaded {
                path: "/Music/Album/AlbumSpotlightCover.mp4".to_string(),
                size_bytes: 5_000_000,
            },
        );
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"spotlight\""));
        // The path lives inside a quoted JSON string, so look for
        // the bare filename substring rather than a quote-wrapped
        // shape (the wrapping quotes are at the start/end of the
        // full path, not around the filename).
        assert!(json.contains("AlbumSpotlightCover.mp4"));
        assert!(json.contains("\"size_bytes\":5000000"));
    }

    /// Verifies the `DownloadFailed` variant round-trips through serde
    /// with both its `url` and `reason` payload intact — critical for
    /// the activity-log emitter in `download_queue.rs` which composes
    /// the user-facing failure message from `reason`.
    #[test]
    fn artwork_result_serialises_download_failed_variant() {
        let result = ArtworkResult::with_variants(
            VariantStatus::DownloadFailed {
                url: "https://example.com/playlist.m3u8".to_string(),
                reason: "FFmpeg exited with code 1".to_string(),
            },
            VariantStatus::NotOffered,
            VariantStatus::NotOffered,
        );
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"kind\":\"download_failed\""));
        assert!(json.contains("\"url\":\"https://example.com/playlist.m3u8\""));
        assert!(json.contains("\"reason\":\"FFmpeg exited with code 1\""));
        // The legacy mirror correctly reports `false` because a
        // DownloadFailed variant did NOT land on disk.
        assert!(json.contains("\"square_downloaded\":false"));
    }

    /// Verifies the empty_result helper returns all three variants
    /// as `NotOffered` and the legacy mirrors as `false`.
    #[test]
    fn empty_result_has_all_not_offered() {
        let result = empty_result();
        assert!(matches!(result.square, VariantStatus::NotOffered));
        assert!(matches!(result.portrait, VariantStatus::NotOffered));
        assert!(matches!(result.spotlight, VariantStatus::NotOffered));
        assert!(!result.square_downloaded);
        assert!(!result.portrait_downloaded);
    }

    /// `is_downloaded()` should distinguish all three VariantStatus
    /// shapes for downstream consumers that just want a yes/no
    /// (matches the pre-#529 bool semantics).
    #[test]
    fn variant_status_is_downloaded_distinguishes_shapes() {
        assert!(VariantStatus::Downloaded {
            path: "/x".to_string(),
            size_bytes: 1,
        }
        .is_downloaded());
        assert!(!VariantStatus::NotOffered.is_downloaded());
        assert!(!VariantStatus::DownloadFailed {
            url: "u".to_string(),
            reason: "r".to_string(),
        }
        .is_downloaded());
    }
}
