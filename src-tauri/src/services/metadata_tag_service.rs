// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// metadata_tag_service.rs -- Post-download metadata enrichment service
// ========================================================================
//
// After GAMDL finishes writing its standard Apple Music metadata (title,
// artist, album art, etc.), this service enriches downloaded M4A files with
// additional metadata tags at multiple levels:
//
//   1. **Codec identification** (always-on):
//      - ALAC: `isLossless = Y`
//      - Dolby Atmos: `SpatialType = Dolby Atmos` (both namespaces)
//
//   2. **Source & format tags** (always-on, no API needed):
//      - `SourceStore = Apple Music` (both iTunes and MeedyaMeta namespaces)
//      - `EncodeSource = Web`
//      - `iTunesMediaType = Music`
//      - `isMedley = Y` (when title contains "Medley")
//      - `ChannelConfig = 2.0 / 5.1 / 7.1 / etc.` (via ffprobe)
//
//   3. **Apple Music API metadata** (always-on when MusicKit credentials configured):
//      - Per-track: ISRC, iTunesAdvisory, iTunesArtistID, iTunesCatalogID,
//        StoreID/AppleMusic
//      - Per-album: AlbumAdvisory, AlbumArtistID, AlbumArtistSort,
//        AlbumGenre, UPC, Barcode
//      - Artwork URLs: MotionArtURL, MotionArtPortraitURL (both namespaces)
//
// All tags are stored as MP4 "freeform" atoms (the `----` box type), which
// is the standard mechanism for custom metadata in the iTunes/M4A ecosystem.
// The `mp4ameta` crate's `set_data()` + `write_to_path()` only modifies the
// MP4 metadata container atoms without re-encoding or touching the audio
// stream data. Existing tags written by GAMDL are fully preserved.
//
// ## Integration
//
// Called from `download_queue.rs` as a background fire-and-forget task after
// each download completes. The enriched function also returns `AlbumMetadata`
// to be reused by the animated artwork service (avoiding duplicate API calls).
//
// @see download_queue.rs -- Calls apply_enriched_metadata_tags() after download
// @see apple_music_api.rs -- Shared MusicKit auth and API client
// @see mp4ameta docs: https://docs.rs/mp4ameta/

use std::path::{Path, PathBuf};

use mp4ameta::{Data, FreeformIdent, Tag};
use tauri::AppHandle;
use tokio::process::Command;

use crate::models::gamdl_options::SongCodec;
use crate::services::apple_music_api::AlbumMetadata;
use crate::services::{apple_music_api, config_service, dependency_manager};

/// Apple iTunes freeform atom namespace. This is the standard "mean" value
/// used by iTunes, Apple Music, and third-party tagging tools for custom
/// metadata in M4A/MP4 files.
const ITUNES_NAMESPACE: &str = "com.apple.iTunes";

/// MeedyaDL-branded freeform atom namespace. Provides a dedicated namespace
/// for MeedyaDL-specific tags, preventing collisions with any current or
/// future Apple-defined atoms.
const MEEDYADL_NAMESPACE: &str = "MeedyaMeta";

/// Applies codec-specific custom metadata tags to all M4A files in the
/// given output directory.
///
/// Walks the directory (non-recursively for single-track downloads,
/// recursively for album downloads) and tags every `.m4a` file found.
///
/// # Arguments
///
/// * `output_path` -- The download output path. May be a file path (single
///   track) or a directory path (album). If it's a file, only that file
///   is tagged. If it's a directory, all `.m4a` files within it (including
///   subdirectories) are tagged.
/// * `codec` -- The audio codec used for the download. Determines which
///   tags are written:
///   - `SongCodec::Alac` → `isLossless = Y`
///   - `SongCodec::Atmos` → `SpatialType = Dolby Atmos` (both namespaces)
///   - All other codecs → no tags written (returns Ok immediately)
///
/// # Errors
///
/// Returns `Err(String)` if the output path does not exist or cannot be read.
///
/// # Returns
///
/// * `Ok(count)` -- The number of files successfully tagged.
/// * `Err(message)` -- A human-readable error if the operation fails.
///   Individual file failures are logged at debug level but do not stop
///   processing of remaining files.
pub fn apply_codec_metadata_tags(output_path: &str, codec: &SongCodec) -> Result<usize, String> {
    // Only ALAC and Atmos get custom tags; all other codecs return early.
    let tag_writer: Box<dyn Fn(&mut Tag)> = match codec {
        SongCodec::Alac => Box::new(write_lossless_tags),
        SongCodec::Atmos => Box::new(write_atmos_tags),
        _ => return Ok(0), // No custom tags for lossy codecs
    };

    let path = Path::new(output_path);
    let mut tagged_count = 0;

    if path.is_file() {
        // Single file: tag it directly if it's an M4A
        if is_m4a(path) {
            match tag_single_file(path, &tag_writer) {
                Ok(()) => tagged_count += 1,
                Err(e) => {
                    log::debug!("Failed to tag {}: {}", path.display(), e);
                }
            }
        }
    } else if path.is_dir() {
        // Directory: walk and tag all M4A files recursively
        tagged_count += tag_directory_recursive(path, &tag_writer);
    } else {
        return Err(format!("Output path does not exist: {output_path}"));
    }

    Ok(tagged_count)
}

/// Tags a single M4A file by opening it, applying the tag writer function,
/// and saving the modified metadata back to disk.
fn tag_single_file(path: &Path, tag_writer: &dyn Fn(&mut Tag)) -> Result<(), String> {
    // Open the M4A file and read its existing metadata
    let mut tag = Tag::read_from_path(path)
        .map_err(|e| format!("Failed to read M4A metadata from {}: {}", path.display(), e))?;

    // Apply the codec-specific custom tags
    tag_writer(&mut tag);

    // Write the modified metadata back to the file
    tag.write_to_path(path)
        .map_err(|e| format!("Failed to write M4A metadata to {}: {}", path.display(), e))?;

    log::debug!("Tagged: {}", path.display());
    Ok(())
}

/// Recursively walks a directory tree and tags all M4A files found.
/// Returns the count of successfully tagged files.
fn tag_directory_recursive(dir: &Path, tag_writer: &dyn Fn(&mut Tag)) -> usize {
    let mut count = 0;

    // Read the directory entries; log and skip on permission errors
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            log::debug!("Cannot read directory {}: {}", dir.display(), e);
            return 0;
        }
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();

        if entry_path.is_dir() {
            // Recurse into subdirectories (album folders may contain disc subfolders)
            count += tag_directory_recursive(&entry_path, tag_writer);
        } else if is_m4a(&entry_path) {
            match tag_single_file(&entry_path, tag_writer) {
                Ok(()) => count += 1,
                Err(e) => {
                    log::debug!("Skipping {}: {}", entry_path.display(), e);
                }
            }
        }
    }

    count
}

/// Writes lossless (ALAC) identification tags to an M4A file's metadata.
///
/// Tags written:
///   - `----:com.apple.iTunes:isLossless` → "Y"
fn write_lossless_tags(tag: &mut Tag) {
    // isLossless = Y under the Apple iTunes namespace
    let ident = FreeformIdent::new_static(ITUNES_NAMESPACE, "isLossless");
    tag.set_data(ident, Data::Utf8("Y".to_owned()));
}

/// Writes Dolby Atmos (spatial audio) identification tags to an M4A file's
/// metadata. Two tags are written in different namespaces for maximum
/// discoverability by different tools.
///
/// Tags written:
///   - `----:com.apple.iTunes:SpatialType` → "Dolby Atmos"
///   - `----:MeedyaMeta:SpatialType`       → "Dolby Atmos"
fn write_atmos_tags(tag: &mut Tag) {
    // SpatialType under the Apple iTunes namespace (standard discovery)
    let itunes_ident = FreeformIdent::new_static(ITUNES_NAMESPACE, "SpatialType");
    tag.set_data(itunes_ident, Data::Utf8("Dolby Atmos".to_owned()));

    // SpatialType under the MeedyaMeta namespace (MeedyaDL-branded)
    let meedya_ident = FreeformIdent::new_static(MEEDYADL_NAMESPACE, "SpatialType");
    tag.set_data(meedya_ident, Data::Utf8("Dolby Atmos".to_owned()));
}

/// Checks whether a file path has an `.m4a` extension (case-insensitive).
fn is_m4a(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("m4a"))
}

// ============================================================
// Public API: Enriched Metadata Tagging
// ============================================================

/// Applies comprehensive metadata enrichment to all M4A files after download.
///
/// This is the primary entry point for post-download metadata tagging,
/// replacing the codec-only `apply_codec_metadata_tags()` for primary
/// downloads. Handles codec tags, source/format tags, channel detection
/// via ffprobe, and Apple Music API metadata — all in a single pass per file.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for settings, keychain, and tool paths
/// * `output_path` - Download output path (file or album directory)
/// * `codec` - Audio codec used for this download
/// * `urls` - Apple Music URL(s) from the download request
/// * `pre_fetched_metadata` - Pre-fetched API metadata to reuse (avoids
///   duplicate API calls when called from companion downloads)
///
/// # Errors
///
/// Returns `Err(String)` if the output path does not exist or cannot be read.
///
/// # Returns
/// * `Ok((count, Some(metadata)))` - Files tagged; API metadata for reuse
/// * `Ok((count, None))` - Files tagged; no API metadata available
/// * `Err(message)` - Output path doesn't exist
pub async fn apply_enriched_metadata_tags(
    app: &AppHandle,
    output_path: &str,
    codec: &SongCodec,
    urls: &[String],
    pre_fetched_metadata: Option<&AlbumMetadata>,
    event_context: Option<(&tauri::AppHandle, &str)>,
) -> Result<(usize, Option<AlbumMetadata>), String> {
    // Collect all M4A files from the output path
    let m4a_files = collect_m4a_files(output_path);
    if m4a_files.is_empty() {
        return Ok((0, pre_fetched_metadata.cloned()));
    }

    // Get ffprobe path (optional — channel detection is best-effort)
    let ffprobe_path = get_ffprobe_path(app).ok();

    // Resolve album metadata: reuse pre-fetched or try API fetch
    let album_metadata: Option<AlbumMetadata> = match pre_fetched_metadata {
        Some(m) => Some(m.clone()),
        None => try_fetch_metadata(app, urls, event_context).await,
    };

    // Process each M4A file with all enrichment layers
    let mut tagged_count = 0;
    for file_path in &m4a_files {
        match enrich_single_file(
            file_path,
            codec,
            ffprobe_path.as_ref(),
            album_metadata.as_ref(),
        )
        .await
        {
            Ok(()) => tagged_count += 1,
            Err(e) => {
                log::debug!("Failed to enrich {}: {}", file_path.display(), e);
            }
        }
    }

    log::info!(
        "Enriched {} of {} M4A file(s) with metadata tags",
        tagged_count,
        m4a_files.len()
    );

    Ok((tagged_count, album_metadata))
}

// ============================================================
// Internal: Per-File Enrichment
// ============================================================

/// Apply all enrichment layers to a single M4A file.
///
/// Runs ffprobe (async) for channel detection, then opens the file with
/// mp4ameta and writes all applicable tags in a single pass.
async fn enrich_single_file(
    file_path: &Path,
    codec: &SongCodec,
    ffprobe_path: Option<&PathBuf>,
    album_metadata: Option<&AlbumMetadata>,
) -> Result<(), String> {
    // Run ffprobe first (async I/O) before opening the tag for sync writes
    let channel_config = match ffprobe_path {
        Some(ffprobe) => detect_channel_config(ffprobe, file_path).await,
        None => None,
    };

    // Open the M4A file for reading/writing
    let mut tag = Tag::read_from_path(file_path).map_err(|e| {
        format!(
            "Failed to read M4A metadata from {}: {}",
            file_path.display(),
            e
        )
    })?;

    // --- Layer 1: Codec-specific tags (ALAC/Atmos) ---
    match codec {
        SongCodec::Alac => write_lossless_tags(&mut tag),
        SongCodec::Atmos => write_atmos_tags(&mut tag),
        _ => {} // No codec tags for lossy formats
    }

    // --- Layer 2: Source & format tags (always-on, no API needed) ---
    write_local_tags(&mut tag);

    // --- Layer 3: Channel configuration (via ffprobe) ---
    if let Some(ref config) = channel_config {
        let ident = FreeformIdent::new_static(ITUNES_NAMESPACE, "ChannelConfig");
        tag.set_data(ident, Data::Utf8(config.clone()));
    }

    // --- Layer 4: Apple Music API metadata (if available) ---
    if let Some(metadata) = album_metadata {
        // Per-album tags (same on all files in the album)
        write_api_album_tags(&mut tag, metadata);

        // Animated artwork URL tags (same on all files)
        write_artwork_url_tags(&mut tag, metadata);

        // Per-track tags: match this file to a track by track/disc number
        let track_num = tag.track_number();
        let disc_num = tag.disc_number().unwrap_or(1);
        if let Some(track) = match_track_to_metadata(track_num, disc_num, &metadata.tracks) {
            write_api_track_tags(&mut tag, track);
        }
    }

    // Write all changes back to the file in a single operation
    tag.write_to_path(file_path).map_err(|e| {
        format!(
            "Failed to write M4A metadata to {}: {}",
            file_path.display(),
            e
        )
    })?;

    log::debug!("Enriched: {}", file_path.display());
    Ok(())
}

// ============================================================
// Internal: Tag Writing Helpers
// ============================================================

/// Write always-on local tags that don't require any API calls.
///
/// Tags written:
///   - `SourceStore = Apple Music` (iTunes + `MeedyaMeta` namespaces)
///   - `EncodeSource = Web`
///   - `iTunesMediaType = Music`
///   - `isMedley = Y` (only when title contains "Medley", case-insensitive)
fn write_local_tags(tag: &mut Tag) {
    // Extract isMedley flag before any mutable operations (avoids borrow conflict)
    let is_medley = tag
        .title()
        .is_some_and(|t| t.to_ascii_lowercase().contains("medley"));

    // SourceStore in both namespaces
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "SourceStore"),
        Data::Utf8("Apple Music".to_owned()),
    );
    tag.set_data(
        FreeformIdent::new_static(MEEDYADL_NAMESPACE, "SourceStore"),
        Data::Utf8("Apple Music".to_owned()),
    );

    // EncodeSource — all Apple Music downloads come via the web API
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "EncodeSource"),
        Data::Utf8("Web".to_owned()),
    );

    // iTunesMediaType — M4A files are always "Music" (music videos are M4V)
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "iTunesMediaType"),
        Data::Utf8("Music".to_owned()),
    );

    // isMedley — flag tracks whose title contains "Medley"
    if is_medley {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "isMedley"),
            Data::Utf8("Y".to_owned()),
        );
    }
}

/// Write per-album tags from the Apple Music API response.
///
/// These tags have the same value for every file in the album:
///   - `AlbumAdvisory`, `AlbumArtistID`, `AlbumArtistSort`, `AlbumGenre`, UPC, Barcode
fn write_api_album_tags(tag: &mut Tag, metadata: &AlbumMetadata) {
    if let Some(ref rating) = metadata.content_rating {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumAdvisory"),
            Data::Utf8(rating.clone()),
        );
    }

    if let Some(ref artist_id) = metadata.artist_id {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumArtistID"),
            Data::Utf8(artist_id.clone()),
        );
    }

    if let Some(ref artist_name) = metadata.artist_name {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumArtistSort"),
            Data::Utf8(artist_name.clone()),
        );
    }

    if let Some(genre) = metadata.genre_names.first() {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumGenre"),
            Data::Utf8(genre.clone()),
        );
    }

    if let Some(ref upc) = metadata.upc {
        // UPC and Barcode contain the same GTIN value
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "UPC"),
            Data::Utf8(upc.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "Barcode"),
            Data::Utf8(upc.clone()),
        );
    }
}

/// Write per-track tags from the matched Apple Music API track metadata.
///
/// Tags written:
///   - ISRC, iTunesAdvisory, iTunesArtistID, iTunesCatalogID, StoreID/AppleMusic
fn write_api_track_tags(tag: &mut Tag, track: &apple_music_api::TrackMetadata) {
    if let Some(ref isrc) = track.isrc {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "ISRC"),
            Data::Utf8(isrc.clone()),
        );
    }

    if let Some(ref rating) = track.content_rating {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "iTunesAdvisory"),
            Data::Utf8(rating.clone()),
        );
    }

    if let Some(ref artist_id) = track.artist_id {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "iTunesArtistID"),
            Data::Utf8(artist_id.clone()),
        );
    }

    // iTunesCatalogID and StoreID/AppleMusic both store the Apple Music song ID
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "iTunesCatalogID"),
        Data::Utf8(track.song_id.clone()),
    );
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "StoreID/AppleMusic"),
        Data::Utf8(track.song_id.clone()),
    );
}

/// Write animated artwork HLS M3U8 URLs as metadata tags.
///
/// These allow downstream tools to discover and download the animated
/// cover art without re-querying the Apple Music API.
fn write_artwork_url_tags(tag: &mut Tag, metadata: &AlbumMetadata) {
    if let Some(ref url) = metadata.artwork_square_url {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "MotionArtURL"),
            Data::Utf8(url.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "MotionArtURL"),
            Data::Utf8(url.clone()),
        );
    }

    if let Some(ref url) = metadata.artwork_tall_url {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "MotionArtPortraitURL"),
            Data::Utf8(url.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "MotionArtPortraitURL"),
            Data::Utf8(url.clone()),
        );
    }
}

// ============================================================
// Internal: File Collection
// ============================================================

/// Collect all M4A file paths from the output path.
///
/// Handles both single-file (single-track download) and directory
/// (album download) cases. For directories, walks recursively to
/// find M4A files in disc subfolders.
fn collect_m4a_files(output_path: &str) -> Vec<PathBuf> {
    let path = Path::new(output_path);
    let mut files = Vec::new();

    if path.is_file() {
        if is_m4a(path) {
            files.push(path.to_path_buf());
        }
    } else if path.is_dir() {
        collect_m4a_recursive(path, &mut files);
    }

    files
}

/// Recursively collect M4A file paths from a directory tree.
fn collect_m4a_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_m4a_recursive(&path, files);
        } else if is_m4a(&path) {
            files.push(path);
        }
    }
}

// ============================================================
// Internal: Channel Detection (via ffprobe)
// ============================================================

/// Resolve the ffprobe binary path (sibling to the managed `FFmpeg` binary).
///
/// ffprobe ships alongside `FFmpeg` in the same download archive. Its path
/// is derived by replacing the `FFmpeg` binary name with "ffprobe".
fn get_ffprobe_path(app: &AppHandle) -> Result<PathBuf, String> {
    let ffmpeg_bin = dependency_manager::get_tool_binary_path(app, "ffmpeg");
    let ffprobe_name = if cfg!(target_os = "windows") {
        "ffprobe.exe"
    } else {
        "ffprobe"
    };
    let ffprobe_bin = ffmpeg_bin
        .parent()
        .map(|p| p.join(ffprobe_name))
        .ok_or_else(|| "Cannot determine ffprobe directory".to_string())?;

    if !ffprobe_bin.exists() {
        return Err("ffprobe not found alongside FFmpeg".to_string());
    }
    Ok(ffprobe_bin)
}

/// Detect the audio channel configuration of an M4A file using ffprobe.
///
/// Runs ffprobe to inspect the first audio stream and maps the channel
/// count to a standard configuration string (e.g., "2.0", "5.1", "7.1").
///
/// Returns `None` if ffprobe fails or the file has no audio stream.
async fn detect_channel_config(ffprobe_path: &Path, file_path: &Path) -> Option<String> {
    let output = Command::new(ffprobe_path)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-select_streams",
            "a:0",
        ])
        .arg(file_path)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let channels = json
        .get("streams")?
        .as_array()?
        .first()?
        .get("channels")?
        .as_u64()?;

    Some(channels_to_config(channels))
}

/// Map an audio channel count to a standard configuration string.
///
/// Common mappings: 1→"1.0" (mono), 2→"2.0" (stereo), 6→"5.1", 8→"7.1".
/// Unusual counts use a fallback format (e.g., 4→"4.0").
fn channels_to_config(channels: u64) -> String {
    match channels {
        1 => "1.0".to_string(),
        2 => "2.0".to_string(),
        3 => "2.1".to_string(),
        6 => "5.1".to_string(),
        8 => "7.1".to_string(),
        n => format!("{n}.0"),
    }
}

// ============================================================
// Internal: Track-to-File Matching
// ============================================================

/// Match a file to its corresponding Apple Music API track metadata.
///
/// Uses the track number and disc number stored in the M4A file's standard
/// `trkn` and `disk` atoms to find the matching track in the API response.
/// Returns `None` if no track number is available or no match is found.
fn match_track_to_metadata(
    track_num: Option<u16>,
    disc_num: u16,
    tracks: &[apple_music_api::TrackMetadata],
) -> Option<&apple_music_api::TrackMetadata> {
    let track_num = u32::from(track_num?);
    let disc_num = u32::from(disc_num);
    tracks
        .iter()
        .find(|t| t.track_number == track_num && t.disc_number == disc_num)
}

// ============================================================
// Internal: API Metadata Fetching
// ============================================================

/// Try to fetch album metadata from the Apple Music API.
///
/// This is a best-effort fetch: returns `None` if `MusicKit` credentials are
/// not configured, the URL isn't an album URL, or the API call fails.
/// Failures are logged at warn level but do not propagate as errors.
async fn try_fetch_metadata(
    app: &AppHandle,
    urls: &[String],
    event_context: Option<(&tauri::AppHandle, &str)>,
) -> Option<AlbumMetadata> {
    // Helper to emit to Activity Log if context is available.
    // Uses the shared emit_download_log helper from utils::activity_log.
    let log_event = |msg: &str| {
        if let Some((app_handle, dl_id)) = event_context {
            crate::utils::activity_log::emit_download_log(app_handle, dl_id, msg);
        }
    };

    // Load settings for MusicKit credentials
    let settings = config_service::load_settings(app).unwrap_or_default();

    let team_id = match settings.musickit_team_id.as_ref().filter(|s| !s.is_empty()) {
        Some(id) => id,
        None => {
            log_event("Apple Music API: MusicKit Team ID not configured, skipping API metadata");
            return None;
        }
    };
    let key_id = match settings.musickit_key_id.as_ref().filter(|s| !s.is_empty()) {
        Some(id) => id,
        None => {
            log_event("Apple Music API: MusicKit Key ID not configured, skipping API metadata");
            return None;
        }
    };

    // Private key is stored in the OS keychain (sensitive credential)
    let private_key = match apple_music_api::get_private_key_from_keychain() {
        Ok(Some(key)) => key,
        Ok(None) => {
            log::debug!("MusicKit private key not in keychain, skipping API enrichment");
            log_event("Apple Music API: MusicKit private key not found in OS keychain");
            return None;
        }
        Err(e) => {
            log::warn!("Failed to read MusicKit private key: {e}");
            log_event(&format!(
                "Apple Music API: failed to read private key from keychain: {e}"
            ));
            return None;
        }
    };

    log_event("Apple Music API: MusicKit credentials found, generating JWT token...");

    // Parse URL to find an album URL (API enrichment only works for albums)
    let parsed = match urls
        .iter()
        .find_map(|url| apple_music_api::parse_apple_music_url(url))
        .filter(|p| p.content_type == "album")
    {
        Some(p) => p,
        None => {
            log_event("Apple Music API: URL is not an album, skipping API metadata");
            return None;
        }
    };

    let jwt = match apple_music_api::generate_musickit_jwt(team_id, key_id, &private_key) {
        Ok(jwt) => {
            log_event("Apple Music API: JWT generated, fetching album metadata...");
            jwt
        }
        Err(e) => {
            log::warn!("Failed to generate MusicKit JWT for enrichment: {e}");
            log_event(&format!("Apple Music API: JWT generation failed: {e}"));
            return None;
        }
    };

    match apple_music_api::fetch_album_metadata(&jwt, &parsed.storefront, &parsed.album_id).await {
        Ok(Some(metadata)) => {
            log_event(&format!(
                "Apple Music API: fetched metadata ({} track(s), artist: {}, UPC: {})",
                metadata.tracks.len(),
                metadata.artist_name.as_deref().unwrap_or("unknown"),
                metadata.upc.as_deref().unwrap_or("N/A"),
            ));
            Some(metadata)
        }
        Ok(None) => {
            log_event("Apple Music API: album not found in catalog");
            None
        }
        Err(e) => {
            log::warn!("Failed to fetch album metadata for enrichment: {e}");
            log_event(&format!("Apple Music API: fetch failed: {e}"));
            None
        }
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // Channel configuration mapping tests
    // ----------------------------------------------------------

    #[test]
    fn channels_to_config_mono() {
        assert_eq!(channels_to_config(1), "1.0");
    }

    #[test]
    fn channels_to_config_stereo() {
        assert_eq!(channels_to_config(2), "2.0");
    }

    #[test]
    fn channels_to_config_surround_51() {
        assert_eq!(channels_to_config(6), "5.1");
    }

    #[test]
    fn channels_to_config_surround_71() {
        assert_eq!(channels_to_config(8), "7.1");
    }

    #[test]
    fn channels_to_config_21() {
        assert_eq!(channels_to_config(3), "2.1");
    }

    #[test]
    fn channels_to_config_fallback_for_unusual_counts() {
        assert_eq!(channels_to_config(4), "4.0");
        assert_eq!(channels_to_config(10), "10.0");
        assert_eq!(channels_to_config(16), "16.0");
    }

    // ----------------------------------------------------------
    // Track-to-file matching tests
    // ----------------------------------------------------------

    fn sample_tracks() -> Vec<apple_music_api::TrackMetadata> {
        vec![
            apple_music_api::TrackMetadata {
                song_id: "100".to_string(),
                isrc: Some("USUG12300001".to_string()),
                content_rating: Some("explicit".to_string()),
                artist_id: Some("159260351".to_string()),
                artist_name: Some("Artist One".to_string()),
                name: "Track One".to_string(),
                track_number: 1,
                disc_number: 1,
            },
            apple_music_api::TrackMetadata {
                song_id: "200".to_string(),
                isrc: Some("USUG12300002".to_string()),
                content_rating: None,
                artist_id: None,
                artist_name: Some("Artist Two".to_string()),
                name: "Track Two".to_string(),
                track_number: 2,
                disc_number: 1,
            },
            apple_music_api::TrackMetadata {
                song_id: "300".to_string(),
                isrc: Some("USUG12300003".to_string()),
                content_rating: None,
                artist_id: None,
                artist_name: None,
                name: "Disc Two Track One".to_string(),
                track_number: 1,
                disc_number: 2,
            },
        ]
    }

    #[test]
    fn match_track_by_number_and_disc() {
        let tracks = sample_tracks();
        let matched = match_track_to_metadata(Some(1), 1, &tracks);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().song_id, "100");
    }

    #[test]
    fn match_track_second_track() {
        let tracks = sample_tracks();
        let matched = match_track_to_metadata(Some(2), 1, &tracks);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().song_id, "200");
    }

    #[test]
    fn match_track_multi_disc() {
        let tracks = sample_tracks();
        let matched = match_track_to_metadata(Some(1), 2, &tracks);
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().song_id, "300");
    }

    #[test]
    fn match_track_returns_none_for_missing_track_number() {
        let tracks = sample_tracks();
        assert!(match_track_to_metadata(None, 1, &tracks).is_none());
    }

    #[test]
    fn match_track_returns_none_for_nonexistent_track() {
        let tracks = sample_tracks();
        assert!(match_track_to_metadata(Some(99), 1, &tracks).is_none());
    }

    #[test]
    fn match_track_returns_none_for_wrong_disc() {
        let tracks = sample_tracks();
        assert!(match_track_to_metadata(Some(2), 2, &tracks).is_none());
    }

    #[test]
    fn match_track_empty_list_returns_none() {
        assert!(match_track_to_metadata(Some(1), 1, &[]).is_none());
    }

    // ----------------------------------------------------------
    // File extension detection tests
    // ----------------------------------------------------------

    #[test]
    fn is_m4a_detects_lowercase() {
        assert!(is_m4a(Path::new("/tmp/song.m4a")));
    }

    #[test]
    fn is_m4a_detects_uppercase() {
        assert!(is_m4a(Path::new("/tmp/song.M4A")));
    }

    #[test]
    fn is_m4a_rejects_other_extensions() {
        assert!(!is_m4a(Path::new("/tmp/song.mp3")));
        assert!(!is_m4a(Path::new("/tmp/song.m4v")));
        assert!(!is_m4a(Path::new("/tmp/song.flac")));
    }

    #[test]
    fn is_m4a_rejects_no_extension() {
        assert!(!is_m4a(Path::new("/tmp/song")));
    }
}
