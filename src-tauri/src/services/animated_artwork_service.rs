// Copyright (c) 2026 MeedyaSuite
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

    // Geo-lock warning (#961): a fallback storefront served this metadata,
    // so its animated-artwork HLS URLs were minted for that region and may
    // 403 when fetched from elsewhere. No per-download activity-log/dl_id
    // context is available at this call site (unlike
    // `metadata_tag_service::try_fetch_metadata`'s `event_context`), so
    // this is tracing-only.
    if let Some(ref sf) = metadata.fallback_storefront {
        log::warn!(
            "Animated artwork: album {} metadata served by fallback storefront '{sf}' (requested '{}') — HLS URLs may be geo-locked",
            parsed.album_id,
            parsed.storefront,
        );
    }

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

    // #961: PORTRAIT is downloaded first (reordered from the original
    // square-then-portrait sequence) so the square-URL-absent fallback
    // below can reuse the already-downloaded portrait file via a plain
    // filesystem copy instead of a second HLS fetch when possible.
    let portrait_status = match metadata.artwork_tall_url.as_deref() {
        Some(tall_url) => {
            let dest = output_path.join("FrontCoverPortrait.mp4");
            attempt_artwork_variant(app, tall_url, &dest, "portrait").await
        }
        None => VariantStatus::NotOffered,
    };

    let square_status = match metadata.artwork_square_url.as_deref() {
        Some(square_url) => {
            let dest = output_path.join("FrontCover.mp4");
            attempt_artwork_variant(app, square_url, &dest, "square").await
        }
        None => {
            // Cross-variant fallback (#961): Apple didn't offer a
            // dedicated square (1:1) motion asset for this album, but a
            // portrait (3:4) one exists. Rather than leave FrontCover.mp4
            // entirely absent, reuse the portrait source -- most players
            // handle a non-square animated cover fine (cropped/letterboxed),
            // and it beats no animated cover at all.
            match (&portrait_status, metadata.artwork_tall_url.as_deref()) {
                // Portrait already landed on disk -- a plain file copy is
                // cheaper and more reliable than a second HLS fetch.
                (VariantStatus::Downloaded { path, .. }, Some(_)) => {
                    let portrait_path = PathBuf::from(path);
                    let square_dest = output_path.join("FrontCover.mp4");
                    match std::fs::copy(&portrait_path, &square_dest) {
                        Ok(size_bytes) => {
                            log::info!(
                                "Animated artwork: album {} has no square variant — copied portrait as square fallback ({size_bytes} bytes)",
                                metadata.album_id,
                            );
                            VariantStatus::Downloaded {
                                path: square_dest.to_string_lossy().to_string(),
                                size_bytes,
                            }
                        }
                        Err(e) => {
                            log::warn!(
                                "Animated artwork: failed to copy portrait as square fallback for album {}: {e}",
                                metadata.album_id,
                            );
                            VariantStatus::DownloadFailed {
                                url: portrait_path.to_string_lossy().to_string(),
                                reason: format!("portrait-source fallback copy failed: {e}"),
                            }
                        }
                    }
                }
                // Portrait itself failed (or is still NotOffered despite a
                // URL being present, which shouldn't happen but is handled
                // defensively) -- re-fetch the SAME tall_url directly into
                // FrontCover.mp4 rather than copy a file that doesn't exist.
                (_, Some(tall_url)) => {
                    let dest = output_path.join("FrontCover.mp4");
                    attempt_artwork_variant(
                        app,
                        tall_url,
                        &dest,
                        "square (portrait-source fallback)",
                    )
                    .await
                }
                (_, None) => VariantStatus::NotOffered,
            }
        }
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
    match download_hls_with_native_fallback(app, m3u8_url, dest).await {
        Ok(()) => {
            // FFmpeg (or the #974 native concat fallback) returned Ok —
            // verify the file is real (#529).
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

// ============================================================
// HLS Variant Selection (#972)
// ============================================================
//
// Apple's animated-artwork CDN serves motion art as an HLS master
// playlist with several ABR (adaptive bitrate) renditions — e.g. a
// 640x640, a 1080x1080, and a 2160x2160 rendition of the same square
// artwork, each on its own variant playlist. Without this module,
// FFmpeg's default HLS variant selection picks a rendition on its own
// (in practice, usually the highest available), which is needlessly
// large for how small this artwork is typically displayed. This module
// parses the master playlist ourselves and picks the rendition closest
// to the user's configured `animated_artwork_resolution` ceiling.

/// A single ABR rendition parsed from an HLS master playlist's
/// `#EXT-X-STREAM-INF` tags.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HlsVariant {
    /// Rendition width in pixels, from `RESOLUTION=WxH`. `0` when the
    /// attribute is missing or unparsable.
    width: u32,
    /// Rendition height in pixels, from `RESOLUTION=WxH`. `0` when the
    /// attribute is missing or unparsable.
    height: u32,
    /// Bitrate in bits/sec, from `BANDWIDTH=`. `0` when missing.
    bandwidth: u64,
    /// The variant playlist URI for this rendition — may be relative to
    /// the master playlist's own URL.
    uri: String,
}

/// Splits an `#EXT-X-STREAM-INF:` attribute list on commas that are
/// OUTSIDE double-quoted values.
///
/// HLS attribute lists routinely contain commas inside quoted values
/// (e.g. `CODECS="hvc1.2.4.L123,ec-3"`), so a naive `.split(',')` would
/// incorrectly split that single attribute into two fragments.
fn split_m3u8_attributes(attrs: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in attrs.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            ',' if !in_quotes => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    result.push(trimmed.to_string());
                }
                current = String::new();
            }
            _ => current.push(c),
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        result.push(trimmed.to_string());
    }

    result
}

/// Parses an HLS master playlist body into its list of ABR variants.
///
/// Each `#EXT-X-STREAM-INF:` tag is paired with the URI line
/// immediately following it (per HLS spec RFC 8216 §4.3.4.2 — the tag
/// applies to the next non-comment, non-blank line). Lines that aren't
/// part of a stream-inf/URI pair are ignored.
fn parse_master_playlist(body: &str) -> Vec<HlsVariant> {
    let mut variants = Vec::new();
    let mut lines = body.lines();

    while let Some(line) = lines.next() {
        let line = line.trim();
        let Some(attrs_str) = line.strip_prefix("#EXT-X-STREAM-INF:") else {
            continue;
        };

        let attrs = split_m3u8_attributes(attrs_str);
        let mut width = 0u32;
        let mut height = 0u32;
        let mut bandwidth = 0u64;

        for attr in &attrs {
            if let Some(value) = attr.strip_prefix("RESOLUTION=") {
                if let Some((w, h)) = value.split_once('x') {
                    width = w.trim().parse().unwrap_or(0);
                    height = h.trim().parse().unwrap_or(0);
                }
            } else if let Some(value) = attr.strip_prefix("BANDWIDTH=") {
                bandwidth = value.trim().parse().unwrap_or(0);
            }
        }

        // The variant URI is the next non-blank, non-comment line.
        for uri_line in lines.by_ref() {
            let uri_line = uri_line.trim();
            if uri_line.is_empty() || uri_line.starts_with('#') {
                continue;
            }
            variants.push(HlsVariant {
                width,
                height,
                bandwidth,
                uri: uri_line.to_string(),
            });
            break;
        }
    }

    variants
}

/// Picks the variant whose height is closest to `target_height`.
///
/// Ties are broken by (1) higher width, then (2) higher bandwidth —
/// preferring more detail per pixel when two renditions are equally
/// close in height. Variants with `height == 0` (unparsable
/// `RESOLUTION`) are excluded entirely — an unknown height can't be
/// judged "close" to anything.
fn pick_variant(variants: &[HlsVariant], target_height: u32) -> Option<&HlsVariant> {
    variants
        .iter()
        .filter(|v| v.height != 0)
        .min_by(|a, b| {
            let diff_a = a.height.abs_diff(target_height);
            let diff_b = b.height.abs_diff(target_height);
            diff_a
                .cmp(&diff_b)
                .then_with(|| b.width.cmp(&a.width))
                .then_with(|| b.bandwidth.cmp(&a.bandwidth))
        })
}

/// Resolves an HLS master playlist URL to a specific rendition's
/// variant playlist URL, honouring the user's configured
/// `animated_artwork_resolution` ceiling (#972).
///
/// `target_height` is `None` for `AnimatedArtworkResolution::Max` — no
/// selection is performed and the master URL is returned unchanged
/// (matches pre-#972 behaviour, where FFmpeg picks its own default
/// rendition from the master playlist). When `Some`, the master
/// playlist is fetched and parsed, and the variant closest to the
/// target height is selected.
///
/// Every failure path (client build error, fetch error, non-2xx
/// response, unreadable body, no parseable variants, relative-URI
/// resolution failure) falls back to returning the master URL
/// unchanged and logs a warning — animated artwork should never
/// hard-fail just because the resolution-selection step couldn't run.
async fn resolve_hls_variant_url(master_url: &str, target_height: Option<u32>) -> String {
    let Some(target_height) = target_height else {
        return master_url.to_string();
    };

    let client = match crate::utils::http_client::build_simple(15) {
        Ok(c) => c,
        Err(e) => {
            log::warn!(
                "Failed to build HTTP client for animated-artwork HLS variant selection: {e} — using master playlist URL"
            );
            return master_url.to_string();
        }
    };

    // Same browser-grade headers as the eventual segment/playlist fetch
    // in `download_hls_to_mp4` — Apple's motion-art CDN rejects requests
    // without them.
    let response = match client
        .get(master_url)
        .header("Origin", "https://music.apple.com")
        .header("Referer", "https://music.apple.com/")
        .header(
            "User-Agent",
            crate::utils::http_client::SAFARI_MACOS_USER_AGENT,
        )
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            log::warn!(
                "Failed to fetch HLS master playlist for variant selection: {e} — using master playlist URL"
            );
            return master_url.to_string();
        }
    };

    if !response.status().is_success() {
        log::warn!(
            "HLS master playlist fetch returned HTTP {} — using master playlist URL",
            response.status()
        );
        return master_url.to_string();
    }

    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => {
            log::warn!(
                "Failed to read HLS master playlist body: {e} — using master playlist URL"
            );
            return master_url.to_string();
        }
    };

    let variants = parse_master_playlist(&body);
    let Some(variant) = pick_variant(&variants, target_height) else {
        log::warn!(
            "No parseable ABR variants in HLS master playlist — using master playlist URL"
        );
        return master_url.to_string();
    };

    let master = match url::Url::parse(master_url) {
        Ok(u) => u,
        Err(e) => {
            log::warn!(
                "Failed to parse HLS master playlist URL {master_url}: {e} — using master playlist URL"
            );
            return master_url.to_string();
        }
    };

    match master.join(&variant.uri) {
        Ok(resolved) => resolved.to_string(),
        Err(e) => {
            log::warn!(
                "Failed to resolve HLS variant URI {} against master playlist URL: {e} — using master playlist URL",
                variant.uri
            );
            master_url.to_string()
        }
    }
}

// ============================================================
// Native fMP4 Init+Segment Concat Fallback (#974)
// ============================================================
//
// FFmpeg remains the PRIMARY download mechanism for animated-artwork
// HLS streams — nothing here changes behaviour when FFmpeg is
// installed and working. This section adds a pure-Rust fallback that
// only runs when FFmpeg errors, is missing, or produces a
// missing/empty output file, so animated artwork can still be
// downloaded (and PLAY correctly in players that understand a
// concatenated fMP4) on a machine without a working FFmpeg install.
//
// Apple's animated-artwork CDN serves fragmented MP4 (fMP4) media
// segments: one init segment (the `ftyp`+`moov` boxes, referenced by
// `#EXT-X-MAP:URI="..."`) followed by a sequence of `moof`+`mdat`
// media segments. Because every segment in this delivery shares a
// single init segment and container timeline, a byte-for-byte
// concatenation of init + segments (in playlist order) produces a
// single valid, playable MP4 — no transcoding or container
// remuxing required. This is NOT true of every HLS stream (MPEG-TS
// segments, or fMP4 with per-segment `moov` boxes, would need real
// remuxing), which is why this fallback is scoped narrowly to the
// animated-artwork HLS path and not applied to the GAMDL music-video
// pipeline.

/// One HLS fMP4 MEDIA playlist's init-segment URI (from
/// `#EXT-X-MAP:URI="..."`) plus its ordered list of media segment URIs.
/// Both fields hold URIs exactly as they appear in the playlist —
/// possibly relative to the playlist's own URL — resolution happens at
/// fetch time via `url::Url::join`.
struct Fmp4MediaPlaylist {
    /// The init segment URI, when the playlist declares one via
    /// `#EXT-X-MAP:`. `None` is technically permitted by the fMP4 HLS
    /// spec (a playlist can omit it if segments are self-initializing),
    /// though Apple's animated-artwork CDN always includes one in
    /// practice.
    init_uri: Option<String>,
    /// Ordered media segment URIs, in playlist order (concatenation
    /// order matters for a valid fMP4 timeline).
    segment_uris: Vec<String>,
}

/// Safety cap on the number of media segments the native concat
/// fallback (#974) will fetch. Animated artwork is typically 5-15
/// seconds of video (a handful of segments); this cap is a generous
/// ceiling that still bounds worst-case request count against a
/// pathological or hostile playlist.
const MAX_CONCAT_SEGMENTS: usize = 512;

/// Safety cap on the total bytes the native concat fallback (#974)
/// will write to disk. Animated artwork files are a few MB at most;
/// this is a generous ceiling that still bounds worst-case disk usage.
const MAX_CONCAT_TOTAL_BYTES: u64 = 512 * 1024 * 1024;

/// Returns `true` when `body` is an HLS MASTER playlist (lists
/// `#EXT-X-STREAM-INF:` variant renditions to choose between) rather
/// than a MEDIA playlist (lists segments directly).
fn is_master_playlist(body: &str) -> bool {
    body.lines()
        .any(|line| line.trim().starts_with("#EXT-X-STREAM-INF:"))
}

/// Parses an HLS fMP4 MEDIA playlist (NOT a master playlist — see
/// [`is_master_playlist`]) into its init-segment URI and ordered list
/// of media segment URIs.
///
/// The `#EXT-X-MAP:` tag's `URI="..."` attribute is parsed via the
/// same [`split_m3u8_attributes`] comma-splitter used for
/// `#EXT-X-STREAM-INF:`, so a URI containing a comma inside other
/// attributes (e.g. a trailing `BYTERANGE="720@0"`) doesn't confuse
/// parsing. Only the first `#EXT-X-MAP:` tag's URI is kept, matching
/// the single-init-segment shape the animated-artwork CDN uses.
///
/// Returns `None` when the playlist has zero segments, or when the
/// segment count exceeds [`MAX_CONCAT_SEGMENTS`] (defence against a
/// pathological/hostile playlist driving unbounded HTTP requests).
fn parse_fmp4_media_playlist(body: &str) -> Option<Fmp4MediaPlaylist> {
    let mut init_uri: Option<String> = None;
    let mut segment_uris: Vec<String> = Vec::new();

    for line in body.lines() {
        let line = line.trim();

        if let Some(attrs_str) = line.strip_prefix("#EXT-X-MAP:") {
            if init_uri.is_none() {
                for attr in split_m3u8_attributes(attrs_str) {
                    if let Some(raw) = attr.strip_prefix("URI=") {
                        init_uri = Some(raw.trim_matches('"').to_string());
                        break;
                    }
                }
            }
            continue;
        }

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        segment_uris.push(line.to_string());
        if segment_uris.len() > MAX_CONCAT_SEGMENTS {
            return None;
        }
    }

    if segment_uris.is_empty() {
        None
    } else {
        Some(Fmp4MediaPlaylist {
            init_uri,
            segment_uris,
        })
    }
}

/// Applies the same Apple CDN header set the master-playlist and
/// FFmpeg fetch paths use (Origin/Referer + the fixed macOS Safari
/// User-Agent) — Apple's motion-art CDN edges reject requests without
/// them regardless of which HTTP client makes the request.
fn apple_cdn_headers(req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    req.header("Origin", "https://music.apple.com")
        .header("Referer", "https://music.apple.com/")
        .header(
            "User-Agent",
            crate::utils::http_client::SAFARI_MACOS_USER_AGENT,
        )
}

/// Fetches `url`'s response body as raw bytes, applying the shared
/// Apple CDN headers. A non-2xx status (notably 403, the geo-lock
/// symptom -- see #961) is surfaced as an `Err` naming the status
/// rather than silently returning an empty body.
async fn fetch_cdn_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let response = apple_cdn_headers(client.get(url))
        .send()
        .await
        .map_err(|e| format!("Request failed for {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} for {url}", response.status()));
    }

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("Failed to read response body for {url}: {e}"))
}

/// Downloads an HLS animated-artwork stream to `dest` via a pure-Rust
/// fMP4 init+segment concatenation — no FFmpeg required (#974).
///
/// Handles both master and media playlists at `m3u8_url`: if it's a
/// master playlist, selects the variant closest to `target_height`
/// (mirroring [`resolve_hls_variant_url`]'s own selection, #972) and
/// re-fetches that variant's media playlist before concatenating.
///
/// Writes to a `.part` sibling of `dest` first and renames into place
/// only after every segment has landed successfully, so a
/// mid-download failure never leaves a corrupt/partial file at the
/// final destination.
async fn download_hls_native_concat(
    m3u8_url: &str,
    dest: &Path,
    target_height: Option<u32>,
) -> Result<(), String> {
    let client = crate::utils::http_client::build_simple(30)?;

    let root_body = fetch_cdn_bytes(&client, m3u8_url).await?;
    let root_text = String::from_utf8(root_body)
        .map_err(|e| format!("HLS playlist at {m3u8_url} is not valid UTF-8: {e}"))?;

    // Resolve a master playlist down to a media playlist, mirroring
    // `resolve_hls_variant_url`'s own selection logic (#972).
    let (media_url, media_body) = if is_master_playlist(&root_text) {
        let variants = parse_master_playlist(&root_text);
        let variant = pick_variant(&variants, target_height.unwrap_or(u32::MAX)).ok_or_else(|| {
            "No parseable ABR variants in HLS master playlist (native concat)".to_string()
        })?;

        let master = url::Url::parse(m3u8_url)
            .map_err(|e| format!("Failed to parse HLS master playlist URL {m3u8_url}: {e}"))?;
        let variant_url = master.join(&variant.uri).map_err(|e| {
            format!("Failed to resolve HLS variant URI {}: {e}", variant.uri)
        })?;

        let body = fetch_cdn_bytes(&client, variant_url.as_str()).await?;
        let text = String::from_utf8(body).map_err(|e| {
            format!("HLS media playlist at {variant_url} is not valid UTF-8: {e}")
        })?;
        (variant_url.to_string(), text)
    } else {
        (m3u8_url.to_string(), root_text)
    };

    let playlist = parse_fmp4_media_playlist(&media_body).ok_or_else(|| {
        "HLS media playlist has no parseable segments (native concat)".to_string()
    })?;

    let media_base = url::Url::parse(&media_url)
        .map_err(|e| format!("Failed to parse HLS media playlist URL {media_url}: {e}"))?;

    let part_path = dest.with_extension("mp4.part");
    let mut file = tokio::fs::File::create(&part_path)
        .await
        .map_err(|e| format!("Failed to create {}: {e}", part_path.display()))?;

    let mut total_bytes: u64 = 0;
    let mut segment_count: usize = 0;

    // Fetch + write one URI (init or media segment), enforcing the
    // total-bytes safety cap and cleaning up the partial file on any
    // failure so a half-written `.part` file is never left behind.
    async fn fetch_and_append(
        client: &reqwest::Client,
        base: &url::Url,
        uri: &str,
        file: &mut tokio::fs::File,
        part_path: &Path,
        total_bytes: &mut u64,
    ) -> Result<(), String> {
        let resolved = base
            .join(uri)
            .map_err(|e| format!("Failed to resolve segment URI {uri}: {e}"))?;
        let bytes = match fetch_cdn_bytes(client, resolved.as_str()).await {
            Ok(b) => b,
            Err(e) => {
                let _ = tokio::fs::remove_file(part_path).await;
                return Err(e);
            }
        };
        *total_bytes += bytes.len() as u64;
        if *total_bytes > MAX_CONCAT_TOTAL_BYTES {
            let _ = tokio::fs::remove_file(part_path).await;
            return Err("HLS native concat exceeded the total byte safety cap".to_string());
        }
        if let Err(e) = tokio::io::AsyncWriteExt::write_all(file, &bytes).await {
            let _ = tokio::fs::remove_file(part_path).await;
            return Err(format!("Failed to write {resolved}: {e}"));
        }
        Ok(())
    }

    if let Some(init_uri) = &playlist.init_uri {
        fetch_and_append(
            &client,
            &media_base,
            init_uri,
            &mut file,
            &part_path,
            &mut total_bytes,
        )
        .await?;
    }

    for segment_uri in &playlist.segment_uris {
        fetch_and_append(
            &client,
            &media_base,
            segment_uri,
            &mut file,
            &part_path,
            &mut total_bytes,
        )
        .await?;
        segment_count += 1;
    }

    drop(file);

    if total_bytes == 0 {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err("HLS native concat produced 0 bytes".to_string());
    }

    tokio::fs::rename(&part_path, dest).await.map_err(|e| {
        format!(
            "Failed to rename {} to {}: {e}",
            part_path.display(),
            dest.display()
        )
    })?;

    log::info!(
        "HLS native concat: wrote {segment_count} segment(s), {total_bytes} bytes, to {}",
        dest.display(),
    );

    Ok(())
}

/// Downloads an HLS stream to `dest`, preferring FFmpeg (the
/// long-established primary path, unchanged) and falling back to the
/// pure-Rust native fMP4 concat above (#974) only when FFmpeg errors,
/// is missing, or reports success but produces a missing/empty file.
///
/// On a fallback, both failure messages are combined so a genuine
/// underlying problem (e.g. a network outage that breaks both paths,
/// or a geo-locked #961 storefront mismatch surfacing as HTTP 403 on
/// both) is fully diagnosable from a single error string.
async fn download_hls_with_native_fallback(
    app: &AppHandle,
    m3u8_url: &str,
    dest: &Path,
) -> Result<(), String> {
    let ffmpeg_failure = match download_hls_to_mp4(app, m3u8_url, dest).await {
        Ok(()) => match std::fs::metadata(dest) {
            Ok(meta) if meta.len() > 0 => return Ok(()),
            Ok(_) => "FFmpeg reported success but produced a 0-byte file".to_string(),
            Err(e) => format!("FFmpeg reported success but the output file is missing: {e}"),
        },
        Err(e) => e,
    };

    log::warn!(
        "FFmpeg HLS download for {} failed ({ffmpeg_failure}) — trying native fMP4 concat fallback (#974)",
        dest.display()
    );

    let settings = config_service::load_settings(app).unwrap_or_default();
    download_hls_native_concat(
        m3u8_url,
        dest,
        settings.animated_artwork_resolution.target_height(),
    )
    .await
    .map_err(|native_err| {
        format!(
            "FFmpeg failed ({ffmpeg_failure}); native concat fallback also failed ({native_err})"
        )
    })
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

    // #972: resolve the user's configured resolution ceiling to a
    // specific HLS variant playlist URL before handing off to FFmpeg.
    // Falls back to the master playlist URL on any failure (see
    // `resolve_hls_variant_url` doc comment) — a resolution-selection
    // failure should never block the download outright.
    let settings = config_service::load_settings(app).unwrap_or_default();
    let input_url = resolve_hls_variant_url(
        m3u8_url,
        settings.animated_artwork_resolution.target_height(),
    )
    .await;

    log::debug!(
        "Downloading HLS stream to {}: {}",
        output_path.display(),
        input_url
    );

    // Run FFmpeg to download the HLS stream and remux to MP4.
    //
    // ### Browser-grade headers (added 2026-06-21 — reliability fix)
    //
    // Apple's motion-art CDN edges (`mvod-akamaized.itunes.apple.com`
    // and friends) have tightened over the past year and now reject
    // requests that lack browser-grade headers. FFmpeg's default
    // User-Agent (`Lavf/<version>`) gets a 403 on either the master
    // m3u8, a variant playlist, or a segment fetch — and from the
    // user's point of view this manifests as "animated artwork
    // doesn't work reliably" even when MeedyaDL successfully
    // discovered the HLS URL from the catalog API.
    //
    // We pass the same `Origin` / `Referer` / `User-Agent` triple
    // that the Apple Music web player sends. Mirrors the
    // syllable-lyrics fix (#935/#936) but applied at the HLS layer.
    //
    // FFmpeg's `-headers` flag takes a `\r\n`-delimited blob; the
    // trailing `\r\n` is mandatory or the last header line is
    // silently dropped. `-user_agent` is a dedicated flag because
    // FFmpeg would otherwise also append its default `Lavf` UA;
    // the dedicated flag cleanly overrides.
    //
    // Other flags:
    //   -i {url}             -- input HLS stream
    //   -c copy              -- copy streams without re-encoding (preserves HEVC quality)
    //   -movflags +faststart -- move moov atom to start for faster playback
    //   -y                   -- overwrite output file if it exists
    //   -loglevel warning    -- suppress verbose output, only show warnings/errors
    // Apple Music always gets the fixed macOS Safari UA regardless of host
    // OS (single source of truth in utils::http_client).
    let browser_user_agent = crate::utils::http_client::SAFARI_MACOS_USER_AGENT;
    let apple_music_headers = "Origin: https://music.apple.com\r\nReferer: https://music.apple.com/\r\n";

    let output = Command::new(&ffmpeg_bin)
        .args([
            "-user_agent",
            browser_user_agent,
            "-headers",
            apple_music_headers,
            "-i",
            input_url.as_str(),
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
        Some(&settings.language),
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
    download_hls_with_native_fallback(app, &promo.video_url, &dest).await?;

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

    // ----------------------------------------------------------
    // HLS variant selection tests (#972)
    // ----------------------------------------------------------

    /// A representative master playlist with three ABR renditions
    /// (640x640, 1080x1080, 2160x2160) and a quoted `CODECS` attribute
    /// containing a comma — the case `split_m3u8_attributes` exists to
    /// handle correctly.
    const MASTER_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x640,CODECS=\"hvc1.2.4.L123,ec-3\"\n\
640x640/playlist.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=2500000,RESOLUTION=1080x1080,CODECS=\"hvc1.2.4.L123,ec-3\"\n\
1080x1080/playlist.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=8000000,RESOLUTION=2160x2160,CODECS=\"hvc1.2.4.L123,ec-3\"\n\
2160x2160/playlist.m3u8\n";

    #[test]
    fn parses_all_stream_inf_renditions() {
        let variants = parse_master_playlist(MASTER_PLAYLIST);
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].width, 640);
        assert_eq!(variants[0].height, 640);
        assert_eq!(variants[0].bandwidth, 800_000);
        assert_eq!(variants[0].uri, "640x640/playlist.m3u8");
        assert_eq!(variants[1].height, 1080);
        assert_eq!(variants[2].height, 2160);
        assert_eq!(variants[2].bandwidth, 8_000_000);
    }

    #[test]
    fn quoted_codecs_commas_do_not_split_attributes() {
        // Every rendition in MASTER_PLAYLIST carries a quoted CODECS
        // attribute with an internal comma. If `split_m3u8_attributes`
        // naively split on every comma, RESOLUTION/BANDWIDTH parsing
        // would still happen to work (they come before CODECS), but the
        // attribute count would be wrong. Assert on the concrete
        // symptom instead: RESOLUTION and BANDWIDTH must both parse
        // correctly despite the quoted comma sitting between them and
        // the end of the line.
        let variants = parse_master_playlist(MASTER_PLAYLIST);
        assert_eq!(variants.len(), 3);
        for variant in &variants {
            assert_ne!(variant.width, 0);
            assert_ne!(variant.height, 0);
            assert_ne!(variant.bandwidth, 0);
        }

        // Direct unit check on the splitter itself.
        let attrs = split_m3u8_attributes(
            "BANDWIDTH=800000,RESOLUTION=640x640,CODECS=\"hvc1.2.4.L123,ec-3\"",
        );
        assert_eq!(attrs.len(), 3);
        assert_eq!(attrs[0], "BANDWIDTH=800000");
        assert_eq!(attrs[1], "RESOLUTION=640x640");
        assert_eq!(attrs[2], "CODECS=\"hvc1.2.4.L123,ec-3\"");
    }

    #[test]
    fn pick_variant_nearest_height() {
        let variants = parse_master_playlist(MASTER_PLAYLIST);

        // Exact match.
        let picked = pick_variant(&variants, 1080).unwrap();
        assert_eq!(picked.height, 1080);

        // Closer to 640 than to 1080.
        let picked = pick_variant(&variants, 700).unwrap();
        assert_eq!(picked.height, 640);

        // Closer to 2160 than to 1080.
        let picked = pick_variant(&variants, 1800).unwrap();
        assert_eq!(picked.height, 2160);

        // Above every rendition — nearest (highest) wins.
        let picked = pick_variant(&variants, 4000).unwrap();
        assert_eq!(picked.height, 2160);
    }

    #[test]
    fn pick_variant_tie_breaks_on_width() {
        // Two renditions equally close to the target height (900 is
        // 260 away from 640 and 180 away from 1080 — NOT actually a
        // tie; construct an explicit tie instead so the test doesn't
        // depend on arithmetic coincidence).
        let variants = vec![
            HlsVariant {
                width: 640,
                height: 640,
                bandwidth: 800_000,
                uri: "a.m3u8".to_string(),
            },
            HlsVariant {
                width: 1080,
                height: 1080,
                bandwidth: 2_500_000,
                uri: "b.m3u8".to_string(),
            },
        ];
        // 860 is equidistant from 640 (diff 220) and 1080 (diff 220).
        let picked = pick_variant(&variants, 860).unwrap();
        // Tie-break prefers the higher width/bandwidth rendition.
        assert_eq!(picked.uri, "b.m3u8");
    }

    #[test]
    fn pick_variant_empty_or_untagged_returns_none() {
        assert!(pick_variant(&[], 1080).is_none());

        // A variant with height == 0 (unparsable RESOLUTION) must be
        // excluded rather than ever being picked as "closest".
        let variants = vec![HlsVariant {
            width: 0,
            height: 0,
            bandwidth: 0,
            uri: "unknown.m3u8".to_string(),
        }];
        assert!(pick_variant(&variants, 1080).is_none());
    }

    // ----------------------------------------------------------
    // Native fMP4 concat fallback tests (#974)
    // ----------------------------------------------------------

    #[test]
    fn is_master_playlist_detects_stream_inf() {
        assert!(is_master_playlist(MASTER_PLAYLIST));
    }

    #[test]
    fn is_master_playlist_false_for_media_playlist() {
        let media = "#EXTM3U\n\
#EXT-X-VERSION:7\n\
#EXT-X-MAP:URI=\"init.mp4\"\n\
#EXTINF:2.0,\n\
seg0.m4s\n\
#EXTINF:2.0,\n\
seg1.m4s\n\
#EXT-X-ENDLIST\n";
        assert!(!is_master_playlist(media));
    }

    /// A representative fMP4 media playlist: one `#EXT-X-MAP:` init
    /// segment declaration followed by three ordered media segments.
    const MEDIA_PLAYLIST: &str = "#EXTM3U\n\
#EXT-X-VERSION:7\n\
#EXT-X-MAP:URI=\"init.mp4\"\n\
#EXTINF:2.0,\n\
seg0.m4s\n\
#EXTINF:2.0,\n\
seg1.m4s\n\
#EXTINF:2.0,\n\
seg2.m4s\n\
#EXT-X-ENDLIST\n";

    #[test]
    fn parse_fmp4_media_playlist_extracts_init_and_ordered_segments() {
        let playlist = parse_fmp4_media_playlist(MEDIA_PLAYLIST).unwrap();
        assert_eq!(playlist.init_uri.as_deref(), Some("init.mp4"));
        assert_eq!(playlist.segment_uris, vec!["seg0.m4s", "seg1.m4s", "seg2.m4s"]);
    }

    #[test]
    fn parse_fmp4_media_playlist_ignores_comments_and_blank_lines() {
        let playlist = parse_fmp4_media_playlist(MEDIA_PLAYLIST).unwrap();
        // Every non-segment, non-EXT-X-MAP line (#EXTM3U, #EXT-X-VERSION,
        // #EXTINF, #EXT-X-ENDLIST) must be excluded from segment_uris.
        assert!(!playlist.segment_uris.iter().any(|s| s.starts_with('#')));
        assert_eq!(playlist.segment_uris.len(), 3);
    }

    #[test]
    fn parse_fmp4_media_playlist_none_on_zero_segments() {
        let empty = "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXT-X-ENDLIST\n";
        assert!(parse_fmp4_media_playlist(empty).is_none());
    }

    #[test]
    fn parse_fmp4_media_playlist_caps_segment_count() {
        let mut body = String::from("#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n");
        for i in 0..=MAX_CONCAT_SEGMENTS {
            body.push_str(&format!("#EXTINF:2.0,\nseg{i}.m4s\n"));
        }
        body.push_str("#EXT-X-ENDLIST\n");
        assert!(
            parse_fmp4_media_playlist(&body).is_none(),
            "a playlist with MAX_CONCAT_SEGMENTS + 1 segments must be rejected"
        );
    }

    #[test]
    fn parse_fmp4_media_playlist_handles_extra_map_attributes() {
        // `#EXT-X-MAP:` can carry additional attributes (e.g. BYTERANGE)
        // after URI; the comma-aware splitter must still extract just
        // the URI value.
        let body = "#EXTM3U\n\
#EXT-X-MAP:URI=\"init.mp4\",BYTERANGE=\"720@0\"\n\
#EXTINF:2.0,\n\
seg0.m4s\n\
#EXT-X-ENDLIST\n";
        let playlist = parse_fmp4_media_playlist(body).unwrap();
        assert_eq!(playlist.init_uri.as_deref(), Some("init.mp4"));
    }

    #[test]
    fn parse_fmp4_media_playlist_none_init_uri_when_map_absent() {
        let body = "#EXTM3U\n#EXTINF:2.0,\nseg0.m4s\n#EXT-X-ENDLIST\n";
        let playlist = parse_fmp4_media_playlist(body).unwrap();
        assert!(playlist.init_uri.is_none());
        assert_eq!(playlist.segment_uris, vec!["seg0.m4s"]);
    }
}
