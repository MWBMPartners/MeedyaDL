// Copyright (c) 2026 MeedyaSuite
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
//      - Binaural (AAC/AAC-HE): `isBinaural = Y` (both namespaces)
//      - Downmix (AAC/AAC-HE): `isDownmix = Y` (both namespaces)
//
//   2. **Source & format tags** (always-on, no API needed):
//      - `SourceStore = Apple Music` (both iTunes and MeedyaMeta namespaces)
//      - `EncodeSource = Web`
//      - `iTunesMediaType = Music`
//      - `isMedley = Y` (when title contains "Medley")
//      - `ChannelConfig = 2.0 / 5.1 / 7.1 / etc.` (via ffprobe)
//
//   3. **Apple Music API metadata** (always-on when MusicKit credentials configured):
//      Tags are written in dual namespaces for maximum compatibility:
//      - `com.apple.iTunes` — player-compatible names (Album* prefix for album scope,
//        no prefix for track scope). Industry-standard alternatives where they exist
//        (LABEL, COPYRIGHT, COMPILATION, TOTALTRACKS).
//      - `MeedyaMeta` — MeedyaDL-branded Apple-sourced tags (Apple* prefix).
//
//      Per-track (iTunes): ISRC, iTunesAdvisory, iTunesArtistID, iTunesCatalogID,
//        StoreID/AppleMusic, AppleDigitalMaster, ReleaseDate, Composer, DurationMs,
//        HasLyrics, PlayParamsId, TrackUrl, PreviewUrl, Genre
//      Per-track (MeedyaMeta): AppleAudioTraits, AppleDigitalMaster, AppleReleaseDate,
//        AppleComposerName, AppleDurationMs, AppleHasLyrics, ApplePlayParamsId,
//        AppleTrackUrl, ApplePreviewUrl, AppleGenres
//      Per-album (iTunes): AlbumAdvisory, AlbumArtistID, AlbumArtistSort, AlbumGenre,
//        UPC, Barcode, LABEL, COPYRIGHT, AlbumReleaseDate, COMPILATION, AlbumIsSingle,
//        AlbumIsComplete, AlbumMasteredForItunes, AlbumTrackCount, TOTALTRACKS,
//        AlbumEditorialNote
//      Per-album (MeedyaMeta): AppleAlbumContentRating, AppleUPC, AppleRecordLabel,
//        AppleCopyright, AppleAlbumReleaseDate, AppleIsCompilation, AppleIsSingle,
//        AppleIsComplete, AppleMasteredForItunes, AppleTrackCount, AppleEditorialNote
//      Artwork URLs: MotionArtURL, MotionArtPortraitURL (both namespaces)
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

use crate::models::codec_registry::codec_suffix_from_registry;
use crate::models::gamdl_options::SongCodec;
use crate::models::tag_registry::{self, TagRegistry, TAG_REGISTRY};
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
///   - `SongCodec::Ac3` → `SpatialType = Dolby Digital` (both namespaces)
///   - `SongCodec::AacBinaural` / `AacHeBinaural` → `isBinaural = Y` (both namespaces)
///   - `SongCodec::AacDownmix` / `AacHeDownmix` → `isDownmix = Y` (both namespaces)
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
    // Codec-specific tags: ALAC, Atmos, AC3, Binaural, Downmix.
    // For non-binaural/downmix codecs (AAC Legacy, AAC, etc.), we still need
    // to clear any inherited isBinaural/isDownmix tags that GAMDL's
    // --fetch-extra-tags may have written from the Apple Music API's audioTraits.
    let tag_writer: Box<dyn Fn(&mut Tag)> = match codec {
        SongCodec::Alac => Box::new(|tag: &mut Tag| {
            write_lossless_tags(tag);
            clear_binaural_downmix_tags(tag);
        }),
        SongCodec::Atmos => Box::new(|tag: &mut Tag| {
            write_atmos_tags(tag);
            clear_binaural_downmix_tags(tag);
        }),
        SongCodec::Ac3 => Box::new(|tag: &mut Tag| {
            write_dolby_digital_tags(tag);
            clear_binaural_downmix_tags(tag);
        }),
        SongCodec::AacBinaural | SongCodec::AacHeBinaural => Box::new(write_binaural_tags),
        SongCodec::AacDownmix | SongCodec::AacHeDownmix => Box::new(write_downmix_tags),
        _ => Box::new(clear_binaural_downmix_tags),
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
///
/// Migrated to [`crate::utils::fs_walk::walk_dir_depth`] in v1.0.8
/// (#716/1) with `max_depth=3` — matches GAMDL's natural
/// Output/Artist/Album/file shape and covers disc-subfolder splits
/// (Album/Disc 1/track.m4a). Visitor returns `Some(())` per
/// successfully-tagged file; `result.len()` is the count. Filesystem
/// sidecars (._*, .DS_Store, Thumbs.db) skipped (#577).
fn tag_directory_recursive(dir: &Path, tag_writer: &dyn Fn(&mut Tag)) -> usize {
    crate::utils::fs_walk::walk_dir_depth(dir, 3, |path| {
        if crate::utils::fs_safe::is_filesystem_sidecar(path) {
            return None;
        }
        if !path.is_file() || !is_m4a(path) {
            return None;
        }
        match tag_single_file(path, tag_writer) {
            Ok(()) => Some(()),
            Err(e) => {
                log::debug!("Skipping {}: {}", path.display(), e);
                None
            }
        }
    })
    .len()
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

/// Writes Dolby Digital (AC-3) surround audio identification tags to an M4A
/// file's metadata. AC-3 is a legacy surround format that encodes 5.1 channel
/// audio — it is distinct from Dolby Atmos (E-AC-3 JOC) but still a spatial
/// format. Two tags are written in different namespaces.
///
/// Tags written:
///   - `----:com.apple.iTunes:SpatialType` → "Dolby Digital"
///   - `----:MeedyaMeta:SpatialType`       → "Dolby Digital"
fn write_dolby_digital_tags(tag: &mut Tag) {
    let itunes_ident = FreeformIdent::new_static(ITUNES_NAMESPACE, "SpatialType");
    tag.set_data(itunes_ident, Data::Utf8("Dolby Digital".to_owned()));

    let meedya_ident = FreeformIdent::new_static(MEEDYADL_NAMESPACE, "SpatialType");
    tag.set_data(meedya_ident, Data::Utf8("Dolby Digital".to_owned()));
}

/// Writes binaural audio identification tags to an M4A file's metadata.
///
/// Binaural codecs (AAC Binaural, AAC-HE Binaural) are stereo renderings of
/// spatial audio designed for headphone listening. They cannot be distinguished
/// from standard stereo AAC by analysing the audio stream — the codec identity
/// at download time is the only way to know.
///
/// Tags written:
///   - `----:com.apple.iTunes:isBinaural` → "Y"
///   - `----:MeedyaMeta:isBinaural`       → "Y"
fn write_binaural_tags(tag: &mut Tag) {
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "isBinaural"),
        Data::Utf8("Y".to_owned()),
    );
    tag.set_data(
        FreeformIdent::new_static(MEEDYADL_NAMESPACE, "isBinaural"),
        Data::Utf8("Y".to_owned()),
    );
}

/// Writes downmix audio identification tags to an M4A file's metadata.
///
/// Downmix codecs (AAC Downmix, AAC-HE Downmix) are stereo downmixes of
/// spatial/surround masters. Like binaural, they cannot be distinguished from
/// standard stereo AAC by audio analysis alone.
///
/// Tags written:
///   - `----:com.apple.iTunes:isDownmix` → "Y"
///   - `----:MeedyaMeta:isDownmix`       → "Y"
fn write_downmix_tags(tag: &mut Tag) {
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "isDownmix"),
        Data::Utf8("Y".to_owned()),
    );
    tag.set_data(
        FreeformIdent::new_static(MEEDYADL_NAMESPACE, "isDownmix"),
        Data::Utf8("Y".to_owned()),
    );
}

/// Removes isBinaural and isDownmix tags from an M4A file's metadata.
///
/// Apple Music's servers or GAMDL's `--fetch-extra-tags` may embed these
/// delivery-mode indicators into files regardless of which codec was actually
/// downloaded. This function strips them when the effective codec is not
/// binaural or downmix, preventing incorrect tagging on AAC Legacy, standard
/// AAC, ALAC, or other non-spatial formats.
fn clear_binaural_downmix_tags(tag: &mut Tag) {
    let binaural_itunes = FreeformIdent::new_static(ITUNES_NAMESPACE, "isBinaural");
    let binaural_meedya = FreeformIdent::new_static(MEEDYADL_NAMESPACE, "isBinaural");
    let downmix_itunes = FreeformIdent::new_static(ITUNES_NAMESPACE, "isDownmix");
    let downmix_meedya = FreeformIdent::new_static(MEEDYADL_NAMESPACE, "isDownmix");

    // Only log if we actually removed something (avoid noise)
    let had_binaural = tag.strings_of(&binaural_itunes).next().is_some()
        || tag.strings_of(&binaural_meedya).next().is_some();
    let had_downmix = tag.strings_of(&downmix_itunes).next().is_some()
        || tag.strings_of(&downmix_meedya).next().is_some();

    tag.remove_data_of(&binaural_itunes);
    tag.remove_data_of(&binaural_meedya);
    tag.remove_data_of(&downmix_itunes);
    tag.remove_data_of(&downmix_meedya);

    if had_binaural || had_downmix {
        log::debug!(
            "Cleared inherited binaural/downmix tags (binaural={had_binaural}, downmix={had_downmix})"
        );
    }
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
#[allow(clippy::too_many_arguments)]
pub async fn apply_enriched_metadata_tags(
    app: &AppHandle,
    output_path: &str,
    codec: &SongCodec,
    urls: &[String],
    pre_fetched_metadata: Option<&AlbumMetadata>,
    event_context: Option<(&tauri::AppHandle, &str)>,
    uses_native_priority: bool,
    content_advisory_in_filenames: bool,
    is_library: bool,
) -> Result<(usize, Option<AlbumMetadata>, Option<String>), String> {
    // Collect all M4A files from the output path
    let m4a_files = collect_m4a_files(output_path);
    if m4a_files.is_empty() {
        return Ok((0, pre_fetched_metadata.cloned(), None));
    }

    // Get ffprobe path (optional — channel detection and codec detection
    // are best-effort; enrichment proceeds without ffprobe)
    let ffprobe_path = match get_ffprobe_path(app) {
        Ok(path) => Some(path),
        Err(e) => {
            // Warn in the activity log (non-verbose) when native priority
            // is active, since codec detection accuracy is degraded
            if uses_native_priority {
                if let Some((app_handle, dl_id)) = event_context {
                    crate::utils::activity_log::emit_download_log(
                        app_handle,
                        dl_id,
                        &format!(
                            "ffprobe unavailable ({e}) — codec tags may be inaccurate with native priority. \
                             Falling back to requested codec."
                        ),
                    );
                }
            }
            log::warn!("ffprobe unavailable: {e}");
            None
        }
    };

    // Get MediaInfo path (optional — more accurate codec detection for Atmos/AC3)
    let mediainfo_path = super::mediainfo_service::get_mediainfo_path(app);
    if mediainfo_path.is_some() {
        log::debug!("MediaInfo available for codec detection");
    }

    // Resolve album metadata: reuse pre-fetched or try API fetch.
    //
    // #871: Library URLs (`/library/songs/`, `/library/albums/`, …) point at
    // a user's personal Apple Music library — those IDs are NOT valid against
    // the catalog endpoints (the API returns 404). Skip the fetch entirely
    // so we don't waste an HTTP round-trip and don't log a misleading
    // "Album not found" warning for a working library download. AcoustID,
    // ReplayGain, channel detection, and codec tag writes still run below.
    let album_metadata: Option<AlbumMetadata> = match pre_fetched_metadata {
        Some(m) => Some(m.clone()),
        None if is_library => None,
        None => try_fetch_metadata(app, urls, event_context).await,
    };

    // Apply content advisory suffixes ([Explicit] / [Clean]) to filenames
    // and album folder before tag writing. Must happen AFTER metadata fetch
    // (we need content_rating) and BEFORE enrichment (paths must be stable).
    let (_effective_output_path, files_to_enrich, renamed_output) = if content_advisory_in_filenames
    {
        if let Some(ref metadata) = album_metadata {
            let (new_path, new_files) = apply_advisory_suffixes(output_path, metadata);
            let changed = if new_path != output_path {
                Some(new_path.clone())
            } else {
                None
            };
            (new_path, new_files, changed)
        } else {
            (output_path.to_string(), m4a_files, None)
        }
    } else {
        (output_path.to_string(), m4a_files, None)
    };

    // Process each M4A file with all enrichment layers.
    // Collect the ffprobe-detected codec per file so we can apply the
    // correct suffix based on the ACTUAL codec, not the requested one.
    let mut tagged_count = 0;
    let mut file_codecs: Vec<(PathBuf, SongCodec)> = Vec::new();
    for file_path in &files_to_enrich {
        match enrich_single_file(
            file_path,
            codec,
            uses_native_priority,
            ffprobe_path.as_ref(),
            mediainfo_path.as_ref(),
            album_metadata.as_ref(),
            event_context,
        )
        .await
        {
            Ok(detected_codec) => {
                tagged_count += 1;
                file_codecs.push((file_path.clone(), detected_codec));
            }
            Err(e) => {
                log::debug!("Failed to enrich {}: {}", file_path.display(), e);
            }
        }
    }

    // --- Post-enrichment: apply codec suffix to filenames ---
    // Renames files like "01 Title.m4a" → "01 Title [Dolby Atmos].m4a"
    // based on the ffprobe-detected codec per file. This ensures the suffix
    // reflects the ACTUAL content (e.g., ALAC when native priority fell back
    // from Atmos), not just what was requested.
    for (file_path, detected_codec) in &file_codecs {
        if let Some(suffix) = codec_suffix_from_registry(detected_codec) {
            if !suffix.is_empty() {
                apply_codec_rename_suffix(file_path, suffix);
            }
        }
    }

    log::info!(
        "Enriched {} of {} M4A file(s) with metadata tags",
        tagged_count,
        files_to_enrich.len()
    );

    Ok((tagged_count, album_metadata, renamed_output))
}

// ============================================================
// Internal: Per-File Enrichment
// ============================================================

/// Apply all enrichment layers to a single M4A file.
///
/// Returns the effective codec detected for the file (may differ from the
/// requested codec when native priority is active and ffprobe detects
/// the actual codec from the file). The caller uses this to apply the
/// correct codec suffix to the filename.
///
/// Runs ffprobe (async) for channel and codec detection, then offloads all
/// `mp4ameta` Tag read/write operations to `spawn_blocking` to prevent
/// starving the tokio async runtime on slow filesystems (FUSE mounts, NFS,
/// cloud storage).
///
/// When `uses_native_priority` is `true`, the actual audio codec is detected
/// from the file via ffprobe (since GAMDL doesn't report which codec it
/// selected from the priority chain). This prevents incorrect codec-specific
/// tags (e.g., tagging AAC files as Dolby Atmos).
async fn enrich_single_file(
    file_path: &Path,
    requested_codec: &SongCodec,
    uses_native_priority: bool,
    ffprobe_path: Option<&PathBuf>,
    mediainfo_path: Option<&PathBuf>,
    album_metadata: Option<&AlbumMetadata>,
    event_context: Option<(&tauri::AppHandle, &str)>,
) -> Result<SongCodec, String> {
    // Try MediaInfo first (more accurate for Atmos/AC3 detection), then ffprobe as fallback.
    let mediainfo_result = if let Some(mi_path) = mediainfo_path {
        super::mediainfo_service::detect_codec(mi_path, file_path).await
    } else {
        None
    };

    // Run ffprobe as fallback (async I/O — subprocess with .await).
    let audio_info = if mediainfo_result.is_none() {
        match ffprobe_path {
            Some(ffprobe) => detect_audio_info(ffprobe, file_path).await,
            None => None,
        }
    } else {
        None // Skip ffprobe when MediaInfo succeeded
    };

    // Determine the effective codec for tag writing.
    // Priority: MediaInfo > ffprobe > requested codec.
    // When native priority was used, the actual codec is detected from the
    // file. When single-codec mode was used, the requested codec is always
    // correct. Falls back to requested codec if neither tool is available.
    let effective_codec = if uses_native_priority {
        if let Some(ref mi) = mediainfo_result {
            // MediaInfo detection — definitive for Atmos/AC3/ALAC/AAC
            if let Some((app_handle, dl_id)) = event_context {
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
                crate::utils::activity_log::emit_verbose_download_log(
                    app_handle,
                    dl_id,
                    &format!(
                        "Codec detection (MediaInfo): format={}, features={}, resolved={} [{}]",
                        mi.format,
                        mi.additional_features.as_deref().unwrap_or("none"),
                        mi.codec.to_cli_string(),
                        filename,
                    ),
                );
            }
            mi.codec.clone()
        } else if let Some(ref info) = audio_info {
            let resolved = resolve_codec_from_ffprobe(info, requested_codec);
            // Log codec detection result in verbose mode
            if let Some((app_handle, dl_id)) = event_context {
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
                crate::utils::activity_log::emit_verbose_download_log(
                    app_handle,
                    dl_id,
                    &format!(
                        "Codec detection: ffprobe={}{}, requested={}, resolved={} [{}]",
                        info.codec_name,
                        info.profile
                            .as_deref()
                            .map_or(String::new(), |p| format!("/{p}")),
                        requested_codec.to_cli_string(),
                        resolved.to_cli_string(),
                        filename,
                    ),
                );
            }
            resolved
        } else {
            // ffprobe unavailable or failed for this file — fall back to requested codec.
            // Log at warn level (non-verbose) since this affects tagging accuracy.
            let filename = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            log::warn!(
                "ffprobe unavailable for {}, using requested codec {:?}",
                filename,
                requested_codec
            );
            if let Some((app_handle, dl_id)) = event_context {
                if ffprobe_path.is_some() {
                    // ffprobe exists but failed on this specific file
                    crate::utils::activity_log::emit_download_log(
                        app_handle,
                        dl_id,
                        &format!(
                            "ffprobe failed for {filename} — codec tags may be inaccurate. Using requested codec."
                        ),
                    );
                }
            }
            requested_codec.clone()
        }
    } else {
        // Single-codec mode: the requested codec is exactly what GAMDL used
        requested_codec.clone()
    };

    // Extract channel config from audio info (separate from codec detection)
    let channel_config = audio_info.as_ref().map(|info| info.channel_config.clone());

    // Clone data needed for the blocking closure (all are Send + 'static).
    // effective_codec is cloned so we can return it after the closure consumes the original.
    let return_codec = effective_codec.clone();
    let tag_path = file_path.to_path_buf();
    let metadata_owned = album_metadata.cloned();

    // Offload all Tag I/O to a blocking thread. Tag::read_from_path and
    // Tag::write_to_path are synchronous file I/O that can block for
    // seconds on slow FUSE mounts (CloudMounter, NFS, etc.).
    tokio::task::spawn_blocking(move || {
        // Open the M4A file for reading/writing
        let mut tag = Tag::read_from_path(&tag_path).map_err(|e| {
            format!(
                "Failed to read M4A metadata from {}: {}",
                tag_path.display(),
                e
            )
        })?;

        // --- Layer 1: Codec-specific tags (ALAC/Atmos/AC3/Binaural/Downmix) ---
        // Uses the effective codec (ffprobe-detected when native priority,
        // or requested codec when single-codec mode).
        match effective_codec {
            SongCodec::Alac => write_lossless_tags(&mut tag),
            SongCodec::Atmos => write_atmos_tags(&mut tag),
            SongCodec::Ac3 => write_dolby_digital_tags(&mut tag),
            SongCodec::AacBinaural | SongCodec::AacHeBinaural => {
                write_binaural_tags(&mut tag);
            }
            SongCodec::AacDownmix | SongCodec::AacHeDownmix => {
                write_downmix_tags(&mut tag);
            }
            _ => {
                // Explicitly remove binaural/downmix tags that may have been
                // inherited from Apple's servers or GAMDL's --fetch-extra-tags.
                // These are delivery-mode indicators that only apply to specific
                // codec variants, not to AAC Legacy/standard AAC/ALAC etc.
                clear_binaural_downmix_tags(&mut tag);
            }
        }

        // --- Layer 2: Source & format tags (always-on, no API needed) ---
        write_local_tags(&mut tag);

        // --- Layer 3: Channel configuration (via ffprobe) ---
        if let Some(ref config) = channel_config {
            let ident = FreeformIdent::new_static(ITUNES_NAMESPACE, "ChannelConfig");
            tag.set_data(ident, Data::Utf8(config.clone()));
        }

        // --- Layer 4: Apple Music API metadata (if available) ---
        // Uses config-driven tag definitions from tags.toml via the tag registry.
        // The registry defines JSON paths, value types, and atom names — adding
        // new tags requires only editing tags.toml, zero Rust code changes.
        if let Some(ref metadata) = metadata_owned {
            // Safety guard (#452): verify the file belongs to this album before
            // writing API metadata. If the file's embedded album name doesn't
            // match the API response, skip API metadata to prevent cross-
            // contamination between concurrent downloads.
            let file_album = tag.album().map(|s| s.to_string());
            let file_artist = tag.album_artist().or_else(|| tag.artist()).map(|s| s.to_string());
            let api_album = metadata.album_name.as_deref();
            let api_artist = metadata.artist_name.as_deref();
            let album_matches = match (&file_album, api_album) {
                (Some(file_name), Some(api_name)) => {
                    // Case-insensitive comparison — Apple Music sometimes
                    // normalises casing differently than GAMDL
                    file_name.to_lowercase() == api_name.to_lowercase()
                }
                (None, Some(_)) => {
                    // File has no album tag — try artist name as fallback guard.
                    // If artist names also differ, this file likely belongs to
                    // a different download (#452).
                    match (&file_artist, api_artist) {
                        (Some(fa), Some(aa)) => fa.to_lowercase() == aa.to_lowercase(),
                        // If both album AND artist are missing from file, allow
                        // enrichment (fresh file with minimal tags)
                        _ => true,
                    }
                }
                // API has no album name — allow enrichment
                (_, None) => true,
            };

            if !album_matches {
                log::warn!(
                    "Skipping API metadata for {} — album mismatch: file='{}', API='{}'",
                    tag_path.display(),
                    file_album.as_deref().unwrap_or("?"),
                    api_album.unwrap_or("?"),
                );
            } else {
                // Match this file to a track by track/disc number
                let track_num = tag.track_number();
                let disc_num = tag.disc_number().unwrap_or(1);
                let matched_track = match_track_to_metadata(track_num, disc_num, &metadata.tracks);

                write_tags_from_registry(
                    &mut tag,
                    &TAG_REGISTRY,
                    &metadata.raw_json,
                    matched_track.map(|t| &t.raw_json),
                );
            }
        }

        // --- Layer 5: ISRC extraction from Vendor tag (fallback) ---
        // GAMDL preserves the Apple Music Vendor string, which contains the
        // ISRC in the format "Label:isrc:ISRCCODE". Extract and write to the
        // standardised ISRC atom if not already set by the API (Layer 4).
        extract_isrc_from_vendor(&mut tag);

        // --- Layer 5b: Spatial ISRC annotation ---
        // When the detected codec is Atmos or AC3 (spatial), tag the file
        // with MeedyaMeta:SpatialAudioCodec so downstream tools know this
        // ISRC is from the spatial version of the track. The Apple Music API
        // returns a single ISRC per track — this marker helps identify
        // spatial variants for future cross-platform ISRC matching.
        let spatial_codecs = [
            SongCodec::Atmos,
            SongCodec::Ac3,
            SongCodec::AacBinaural,
        ];
        if spatial_codecs.contains(&effective_codec) {
            let spatial_ident = FreeformIdent::new_static(MEEDYADL_NAMESPACE, "SpatialAudioCodec");
            tag.set_data(
                spatial_ident,
                Data::Utf8(effective_codec.to_cli_string().to_string()),
            );
        }

        // Write all changes back to the file in a single operation
        tag.write_to_path(&tag_path).map_err(|e| {
            format!(
                "Failed to write M4A metadata to {}: {}",
                tag_path.display(),
                e
            )
        })?;

        log::debug!("Enriched: {}", tag_path.display());
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("Metadata enrichment task panicked: {e}"))??;

    Ok(return_codec)
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

/// Reconcile ISRC between the standardised ISRC atom and the Apple Vendor tag.
///
/// Apple Music M4A files contain a freeform atom under `com.apple.iTunes`
/// with the name `Vendor`, whose value follows the pattern
/// `Label:isrc:ISRCCODE` (e.g., `Warner:isrc:USAT22504136`).
///
/// Three cases:
///   1. ISRC blank, Vendor has ISRC → copy Vendor ISRC to standardised tag
///   2. ISRC set, Vendor has ISRC → if different, append Vendor ISRC as
///      additional value; if identical, do nothing
///   3. ISRC set, no Vendor ISRC → do nothing
fn extract_isrc_from_vendor(tag: &mut Tag) {
    // Try to read the Vendor freeform atom under the iTunes namespace
    let vendor_ident = FreeformIdent::new_static(ITUNES_NAMESPACE, "Vendor");
    let vendor_value = match tag.strings_of(&vendor_ident).next() {
        Some(v) => v.to_owned(),
        None => return, // Case 3 (no Vendor tag at all)
    };

    // Parse the ISRC from the Vendor string.
    // Format: "Label:isrc:ISRCCODE" — find ":isrc:" (case-insensitive)
    let lower = vendor_value.to_ascii_lowercase();
    let vendor_isrc = match lower.find(":isrc:") {
        Some(pos) => {
            let isrc_start = pos + ":isrc:".len();
            let extracted = vendor_value[isrc_start..].trim();
            if extracted.is_empty() {
                return; // Vendor has `:isrc:` but no actual code
            }
            extracted.to_string()
        }
        None => return, // Case 3 (Vendor exists but no ISRC in it)
    };

    let isrc_ident = FreeformIdent::new_static(ITUNES_NAMESPACE, "ISRC");
    let existing_isrc: Option<String> = tag.strings_of(&isrc_ident).next().map(|s| s.to_owned());

    match existing_isrc {
        None => {
            // Case 1: ISRC blank → set from Vendor
            log::debug!("ISRC empty — setting from Vendor tag: {vendor_isrc}");
            tag.set_data(isrc_ident, Data::Utf8(vendor_isrc));
        }
        Some(ref current) if current == &vendor_isrc => {
            // Case 2a: identical — do nothing
            log::debug!("ISRC matches Vendor tag: {vendor_isrc}");
        }
        Some(ref current) => {
            // Case 2b: different — append Vendor ISRC as second value
            // Write both ISRCs separated by " / " (common multi-value convention)
            let combined = format!("{current} / {vendor_isrc}");
            log::debug!(
                "ISRC mismatch — API={current}, Vendor={vendor_isrc} — storing both: {combined}"
            );
            tag.set_data(isrc_ident, Data::Utf8(combined));
        }
    }
}

/// Write per-album tags from the Apple Music API response.
///
/// Tags are written in up to three layers per field:
///   1. `com.apple.iTunes` — player-compatible names (Album* prefix for disambiguation)
///   2. `com.apple.iTunes` — industry-standard alternative names (LABEL, COPYRIGHT, etc.)
///   3. `MeedyaMeta` — MeedyaDL-branded Apple-sourced tags (Apple* prefix)
///
/// Industry-standard names recognised by MusicBrainz Picard, Mp3tag, foobar2000, beets:
///   LABEL, COPYRIGHT, COMPILATION, TOTALTRACKS
#[allow(dead_code)]
fn write_api_album_tags(tag: &mut Tag, metadata: &AlbumMetadata) {
    // --- Album content rating ---
    if let Some(ref rating) = metadata.content_rating {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumAdvisory"),
            Data::Utf8(rating.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleAlbumContentRating"),
            Data::Utf8(rating.clone()),
        );
    }

    // --- Album artist ID ---
    if let Some(ref artist_id) = metadata.artist_id {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumArtistID"),
            Data::Utf8(artist_id.clone()),
        );
    }

    // --- Album artist name ---
    if let Some(ref artist_name) = metadata.artist_name {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumArtistSort"),
            Data::Utf8(artist_name.clone()),
        );
    }

    // --- Album genre ---
    if let Some(genre) = metadata.genre_names.first() {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumGenre"),
            Data::Utf8(genre.clone()),
        );
    }

    // --- UPC / Barcode ---
    if let Some(ref upc) = metadata.upc {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "UPC"),
            Data::Utf8(upc.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "Barcode"),
            Data::Utf8(upc.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleUPC"),
            Data::Utf8(upc.clone()),
        );
    }

    // --- Record label ---
    // Industry standard: "LABEL" (recognised by Picard, Mp3tag, foobar2000, beets)
    if let Some(ref label) = metadata.record_label {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "LABEL"),
            Data::Utf8(label.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleRecordLabel"),
            Data::Utf8(label.clone()),
        );
    }

    // --- Copyright ---
    // Industry standard: "COPYRIGHT" (recognised by Mp3tag, foobar2000)
    if let Some(ref copyright) = metadata.copyright {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "COPYRIGHT"),
            Data::Utf8(copyright.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleCopyright"),
            Data::Utf8(copyright.clone()),
        );
    }

    // --- Album release date ---
    if let Some(ref date) = metadata.release_date {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumReleaseDate"),
            Data::Utf8(date.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleAlbumReleaseDate"),
            Data::Utf8(date.clone()),
        );
    }

    // --- Compilation flag ---
    // Industry standard: "COMPILATION" (recognised by Picard, Mp3tag, beets)
    // Note: native `cpil` atom may already exist from GAMDL — this is supplementary.
    if let Some(val) = metadata.is_compilation {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "COMPILATION"),
            Data::Utf8(val.to_string()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleIsCompilation"),
            Data::Utf8(val.to_string()),
        );
    }

    // --- Single flag ---
    if let Some(val) = metadata.is_single {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumIsSingle"),
            Data::Utf8(val.to_string()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleIsSingle"),
            Data::Utf8(val.to_string()),
        );
    }

    // --- Complete flag (all tracks available at download time) ---
    if let Some(val) = metadata.is_complete {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumIsComplete"),
            Data::Utf8(val.to_string()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleIsComplete"),
            Data::Utf8(val.to_string()),
        );
    }

    // --- Mastered for iTunes / Apple Digital Master (album-level) ---
    if let Some(val) = metadata.is_mastered_for_itunes {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumMasteredForItunes"),
            Data::Utf8(val.to_string()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleMasteredForItunes"),
            Data::Utf8(val.to_string()),
        );
    }

    // --- Track count ---
    // Industry standard: "TOTALTRACKS" (recognised by Picard, foobar2000)
    // Note: native `trkn` atom pair may already contain total — this is supplementary.
    if let Some(count) = metadata.track_count {
        let count_str = count.to_string();
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumTrackCount"),
            Data::Utf8(count_str.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "TOTALTRACKS"),
            Data::Utf8(count_str.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleTrackCount"),
            Data::Utf8(count_str),
        );
    }

    // --- Editorial notes ---
    if let Some(ref notes) = metadata.editorial_notes {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AlbumEditorialNote"),
            Data::Utf8(notes.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleEditorialNote"),
            Data::Utf8(notes.clone()),
        );
    }
}

/// Write per-track tags from the matched Apple Music API track metadata.
///
/// Tags are written in up to three layers per field:
///   1. `com.apple.iTunes` — player-compatible names (no prefix = track scope)
///   2. `com.apple.iTunes` — industry-standard alternative names where they exist
///   3. `MeedyaMeta` — MeedyaDL-branded Apple-sourced tags (Apple* prefix)
///
/// Track-level tags use no scope prefix (track is the default/assumed scope
/// since each M4A file IS a track). Album-level uses `Album*` prefix for
/// disambiguation.
#[allow(dead_code)]
fn write_api_track_tags(tag: &mut Tag, track: &apple_music_api::TrackMetadata) {
    // --- ISRC ---
    if let Some(ref isrc) = track.isrc {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "ISRC"),
            Data::Utf8(isrc.clone()),
        );
    }

    // --- Track content rating ---
    if let Some(ref rating) = track.content_rating {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "iTunesAdvisory"),
            Data::Utf8(rating.clone()),
        );
    }

    // --- Track artist ID ---
    if let Some(ref artist_id) = track.artist_id {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "iTunesArtistID"),
            Data::Utf8(artist_id.clone()),
        );
    }

    // --- Song / catalog IDs ---
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "iTunesCatalogID"),
        Data::Utf8(track.song_id.clone()),
    );
    tag.set_data(
        FreeformIdent::new_static(ITUNES_NAMESPACE, "StoreID/AppleMusic"),
        Data::Utf8(track.song_id.clone()),
    );

    // --- Audio traits (Apple-specific, MeedyaMeta only) ---
    if !track.audio_traits.is_empty() {
        let traits_str = track.audio_traits.join(", ");
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleAudioTraits"),
            Data::Utf8(traits_str),
        );
    }

    // --- Apple Digital Master certification (track-level) ---
    if let Some(val) = track.is_apple_digital_master {
        let val_str = val.to_string();
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "AppleDigitalMaster"),
            Data::Utf8(val_str.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleDigitalMaster"),
            Data::Utf8(val_str),
        );
    }

    // --- Track release date ---
    // Note: native `©day` atom may already exist from GAMDL — this is supplementary.
    if let Some(ref date) = track.release_date {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "ReleaseDate"),
            Data::Utf8(date.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleReleaseDate"),
            Data::Utf8(date.clone()),
        );
    }

    // --- Composer ---
    // Note: native `©wrt` atom may already exist from GAMDL — this is supplementary
    // with the exact catalog value.
    if let Some(ref composer) = track.composer_name {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "Composer"),
            Data::Utf8(composer.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleComposerName"),
            Data::Utf8(composer.clone()),
        );
    }

    // --- Duration in milliseconds ---
    if let Some(duration) = track.duration_in_millis {
        let dur_str = duration.to_string();
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "DurationMs"),
            Data::Utf8(dur_str.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleDurationMs"),
            Data::Utf8(dur_str),
        );
    }

    // --- Has lyrics flag ---
    if let Some(val) = track.has_lyrics {
        let val_str = val.to_string();
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "HasLyrics"),
            Data::Utf8(val_str.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleHasLyrics"),
            Data::Utf8(val_str),
        );
    }

    // --- Play params ID (Apple-specific catalog identifier) ---
    if let Some(ref id) = track.play_params_id {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "PlayParamsId"),
            Data::Utf8(id.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "ApplePlayParamsId"),
            Data::Utf8(id.clone()),
        );
    }

    // --- Canonical Apple Music URL ---
    if let Some(ref url) = track.url {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "TrackUrl"),
            Data::Utf8(url.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleTrackUrl"),
            Data::Utf8(url.clone()),
        );
    }

    // --- Preview URL ---
    if let Some(ref url) = track.preview_url {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "PreviewUrl"),
            Data::Utf8(url.clone()),
        );
        tag.set_data(
            FreeformIdent::new_static(MEEDYADL_NAMESPACE, "ApplePreviewUrl"),
            Data::Utf8(url.clone()),
        );
    }

    // --- Track genres (deduplicated, merged with existing) ---
    // Merge API genre names with any existing genre from the standard ©gen atom
    // (set by GAMDL). Deduplicate entries case-insensitively (#452).
    if !track.genre_names.is_empty() {
        // Collect existing genres from the standard atom
        let existing_genre = tag.genre().map(|s| s.to_string());
        let mut all_genres: Vec<String> = Vec::new();
        let mut seen_lower: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Add existing genre entries first (from GAMDL's ©gen atom)
        if let Some(ref existing) = existing_genre {
            for g in existing.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                let lower = g.to_lowercase();
                if seen_lower.insert(lower) {
                    all_genres.push(g.to_string());
                }
            }
        }

        // Add API genres, skipping duplicates and the generic "Music" entry
        for g in &track.genre_names {
            let trimmed = g.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("Music") {
                continue;
            }
            let lower = trimmed.to_lowercase();
            if seen_lower.insert(lower) {
                all_genres.push(trimmed.to_string());
            }
        }

        if !all_genres.is_empty() {
            let genres_str = all_genres.join(", ");
            // Update standard ©gen atom with deduplicated genre list
            tag.set_genre(&genres_str);
            // Also write to freeform atoms for compatibility
            tag.set_data(
                FreeformIdent::new_static(ITUNES_NAMESPACE, "Genre"),
                Data::Utf8(genres_str.clone()),
            );
            tag.set_data(
                FreeformIdent::new_static(MEEDYADL_NAMESPACE, "AppleGenres"),
                Data::Utf8(genres_str),
            );
        }
    }
}

/// Write animated artwork HLS M3U8 URLs as metadata tags.
///
/// These allow downstream tools to discover and download the animated
/// cover art without re-querying the Apple Music API.
#[allow(dead_code)]
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
// Internal: Config-Driven Tag Writing (tags.toml)
// ============================================================

/// Write API-sourced metadata tags using definitions from the tag registry.
///
/// Iterates all tag definitions in `tags.toml`, extracts values from the raw
/// API JSON using dotted paths, converts them to strings, and writes freeform
/// atoms to the M4A file. This replaces the old hardcoded `write_api_album_tags`,
/// `write_api_track_tags`, and `write_artwork_url_tags` functions.
///
/// Album-scope tags are written from `album_json` (same value on every track).
/// Track-scope tags are written from `track_json` (matched by track/disc number).
fn write_tags_from_registry(
    tag: &mut Tag,
    registry: &TagRegistry,
    album_json: &serde_json::Value,
    track_json: Option<&serde_json::Value>,
) {
    // Skip if raw JSON is null (e.g., pre-fetched metadata from older queue format)
    if album_json.is_null() {
        return;
    }

    // Write album-scope tags (same value written to every track in the album)
    for def in &registry.album_tags {
        if let Some(raw_value) = tag_registry::extract_json_value(album_json, &def.json_path) {
            if let Some(string_value) = tag_registry::value_to_string(&raw_value, &def.value_type) {
                for atom in &def.atoms {
                    tag.set_data(
                        FreeformIdent::new_borrowed(atom.namespace, &atom.name),
                        Data::Utf8(string_value.clone()),
                    );
                }
            }
        }
    }

    // Write track-scope tags (value specific to the matched track)
    if let Some(track_json) = track_json {
        if !track_json.is_null() {
            for def in &registry.track_tags {
                if let Some(raw_value) =
                    tag_registry::extract_json_value(track_json, &def.json_path)
                {
                    if let Some(string_value) =
                        tag_registry::value_to_string(&raw_value, &def.value_type)
                    {
                        for atom in &def.atoms {
                            tag.set_data(
                                FreeformIdent::new_borrowed(atom.namespace, &atom.name),
                                Data::Utf8(string_value.clone()),
                            );
                        }
                    }
                }
            }
        }
    }
}

// ============================================================
// Internal: Content Advisory Filename Suffixes
// ============================================================

/// Returns the advisory suffix string for a content rating value.
///
/// Apple Music normally returns lowercase `"explicit"` / `"clean"`, but we
/// match case-insensitively to be defensive against capitalisation drift
/// across API responses and locale variants.
///
/// Made `pub` in #528 so the companion-spawn site in `download_queue.rs`
/// can predict the suffix the primary's enrichment will apply, and inject
/// it into the companion's GAMDL folder template — preventing the
/// sibling-folder bug where the companion writes to `Album/` while the
/// renamed primary lives at `Album [Explicit]/`.
#[must_use]
pub fn advisory_suffix(content_rating: &str) -> Option<&'static str> {
    match content_rating.trim().to_ascii_lowercase().as_str() {
        "explicit" => Some("[Explicit]"),
        "clean" | "cleaned" | "explicitcleaned" => Some("[Clean]"),
        _ => None,
    }
}

/// Apply content advisory suffixes (`[Explicit]` / `[Clean]`) to track
/// filenames and the album folder name based on Apple Music content ratings.
///
/// Called during enrichment after metadata is fetched. Renames files in-place
/// so all subsequent enrichment steps operate on the correctly named paths.
///
/// Returns the (possibly renamed) output directory path and a list of the
/// new file paths for downstream processing.
pub fn apply_advisory_suffixes(
    output_path: &str,
    album_metadata: &apple_music_api::AlbumMetadata,
) -> (String, Vec<PathBuf>) {
    let m4a_files = collect_m4a_files(output_path);
    let mut renamed_files = Vec::with_capacity(m4a_files.len());

    // --- Step 1: Rename individual track files ---
    for file_path in &m4a_files {
        let new_path = rename_track_with_advisory(file_path, &album_metadata.tracks);
        renamed_files.push(new_path);
    }

    // --- Step 2: Rename album folder ---
    let new_output_path = rename_folder_with_advisory(output_path, album_metadata);

    // Update file paths if the folder was renamed
    if new_output_path != output_path {
        let old_prefix = Path::new(output_path);
        let new_prefix = Path::new(&new_output_path);
        for path in &mut renamed_files {
            if let Ok(relative) = path.strip_prefix(old_prefix) {
                *path = new_prefix.join(relative);
            }
        }
    }

    (new_output_path, renamed_files)
}

/// Apply content advisory suffixes (`[Explicit]` / `[Clean]`) to any M4A
/// files in `output_path` that don't yet carry them, reading the advisory
/// directly from each file's `rtng` atom.
///
/// Used as a post-companion pass so companion downloads (ALAC, AAC, etc.)
/// — which land after the primary enrichment already ran — still pick up
/// the advisory suffix. Idempotent: files already carrying the suffix are
/// left alone. Because `rtng` is written by GAMDL itself (the primary
/// downloader), this works without needing access to the original
/// `AlbumMetadata` struct.
pub fn apply_advisory_suffixes_from_tags(output_path: &str) {
    let m4a_files = collect_m4a_files(output_path);
    for file_path in &m4a_files {
        let Ok(tag) = mp4ameta::Tag::read_from_path(file_path) else {
            continue;
        };
        let suffix = match tag.advisory_rating() {
            Some(mp4ameta::AdvisoryRating::Explicit) => "[Explicit]",
            Some(mp4ameta::AdvisoryRating::Clean) => "[Clean]",
            // Inoffensive / None → no advisory suffix
            _ => continue,
        };
        drop(tag);

        let stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        // Case-insensitive idempotency guard — avoids double-suffixing.
        if stem
            .to_ascii_lowercase()
            .contains(&suffix.to_ascii_lowercase())
        {
            continue;
        }

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("m4a");
        let new_stem = insert_advisory_before_codec_suffix(stem, suffix);
        let new_name = format!("{new_stem}.{ext}");
        let new_path = file_path.with_file_name(&new_name);

        // Never clobber — if an unrelated file already sits at the
        // target path (e.g. a previous session's differently-tagged
        // track), `safe_rename` auto-disambiguates with `.1`, `.2`
        // suffix so nothing is overwritten.
        match crate::utils::fs_safe::safe_rename(file_path, &new_path) {
            Ok(final_path) => log::debug!(
                "Post-companion advisory suffix: {} → {}",
                file_path.display(),
                final_path.display()
            ),
            Err(e) => {
                log::warn!(
                    "Post-companion advisory rename failed for {}: {e}",
                    file_path.display()
                );
                // Audio rename failed → skip the sidecar pass for
                // this file. Without the audio rename the sidecars
                // are already correctly paired with the un-renamed
                // audio stem; renaming them now would create the
                // orphan situation #788 was opened to prevent.
                continue;
            }
        }

        // Sidecar rename in lockstep with the audio (#788). Pre-fix,
        // `apply_advisory_suffixes_from_tags` only renamed audio
        // files, so companion sidecars (`01 Title [Lossless].lrc`)
        // were left orphaned next to the now-renamed audio
        // (`01 Title [Explicit] [Lossless].m4a`) — lyrics
        // conversion + subtitle embedding couldn't pair them.
        // Mirrors the pattern from `apply_codec_rename_suffix` /
        // `rename_matching_sidecars` (#535) but uses the
        // advisory-insertion stem transform instead of plain
        // suffix-append.
        rename_matching_advisory_sidecars(file_path, stem, &new_stem);
    }
}

/// Rename lyrics / subtitle sidecars next to `audio_path` whose stem
/// matches `old_stem` so they keep pace with an audio file that was
/// just renamed by the advisory pass (#788).
///
/// Mirror of `rename_matching_sidecars` (the codec-suffix sibling
/// from #535), but takes the explicit new stem rather than a suffix
/// to append — because the advisory rename uses
/// `insert_advisory_before_codec_suffix` (advisory lands BEFORE
/// any codec suffix, not at the end), the caller already has the
/// new stem from that helper and passes it in.
///
/// Silently skips sidecars that don't exist or whose target is
/// already occupied. Errors are logged but don't propagate — sidecar
/// rename is best-effort; the audio rename already landed.
fn rename_matching_advisory_sidecars(audio_path: &Path, old_stem: &str, new_stem: &str) {
    let parent = match audio_path.parent() {
        Some(p) => p,
        None => return,
    };
    for sidecar_ext in CODEC_RENAME_SIDECAR_EXTENSIONS {
        let sidecar = parent.join(format!("{old_stem}.{sidecar_ext}"));
        if !sidecar.exists() {
            continue;
        }
        let new_sidecar = parent.join(format!("{new_stem}.{sidecar_ext}"));
        // Idempotency: if a prior run already produced the renamed
        // sidecar AND the old-stem sidecar still exists (e.g. a
        // partial run earlier created the suffixed version but
        // didn't remove the source), leave both alone — the user
        // can decide. Same conservative behaviour as the codec
        // sidecar helper.
        if new_sidecar.exists() {
            log::debug!(
                "Advisory sidecar rename: target {} already exists — leaving {} in place",
                new_sidecar.display(),
                sidecar.display()
            );
            continue;
        }
        match crate::utils::fs_safe::safe_rename(&sidecar, &new_sidecar) {
            Ok(final_path) => log::debug!(
                "Advisory sidecar: {} → {}",
                sidecar.display(),
                final_path.display()
            ),
            Err(e) => log::warn!(
                "Advisory sidecar rename failed for {}: {e}",
                sidecar.display()
            ),
        }
    }
}

/// Lyrics / subtitle sidecar extensions that travel with an audio file.
/// When we rename the audio to add a codec suffix, these sidecars must be
/// renamed in lockstep so downstream lyrics conversion and embedding can
/// still pair them by stem (see #535).
const CODEC_RENAME_SIDECAR_EXTENSIONS: &[&str] =
    &["ttml", "lrc", "srt", "vtt", "ass"];

/// Rename a single M4A file to include its codec suffix, plus any
/// lyrics/subtitle sidecars that share its stem.
///
/// Appends the suffix (e.g., " [Dolby Atmos]", " [Lossless]") before
/// the file extension. Idempotent: skips files that already contain the
/// suffix in their stem.
///
/// Sidecar handling (#535): when native `--song-codec-priority` is active,
/// GAMDL writes both audio and sidecars with a clean stem because the
/// actual codec is unknown until the download completes. Renaming only the
/// audio here would leave sidecars orphaned — a following companion run
/// with a clean-filename tier would overwrite them, and the (now suffixed)
/// primary audio would end up with no lyrics sidecars and no embedded
/// lyrics. So we also rename any matching `.ttml` / `.lrc` / `.srt` /
/// `.vtt` / `.ass` next to the audio file.
fn apply_codec_rename_suffix(file_path: &Path, suffix: &str) {
    if suffix.is_empty() {
        return;
    }

    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    // Idempotency: skip if suffix already present
    if stem.contains(suffix) {
        return;
    }

    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("m4a");
    let new_name = format!("{stem} {suffix}.{ext}");
    let new_path = file_path.with_file_name(&new_name);

    // `safe_rename` auto-disambiguates if `new_path` is already
    // occupied by an unrelated file (e.g. a previous session left the
    // suffixed version on disk with different content).
    match crate::utils::fs_safe::safe_rename(file_path, &new_path) {
        Ok(final_path) => {
            log::debug!(
                "Codec suffix: {} → {}",
                file_path.display(),
                final_path.display()
            );
        }
        Err(e) => {
            log::warn!(
                "Failed to rename {} with codec suffix: {e}",
                file_path.display()
            );
        }
    }

    rename_matching_sidecars(file_path, stem, suffix);
}

/// Rename any lyrics/subtitle sidecars next to `audio_path` that share the
/// audio's stem, applying the same codec suffix. Silently skips sidecars
/// that are missing or already suffixed. Errors are logged but don't
/// propagate — sidecar rename is best-effort (audio was already renamed).
fn rename_matching_sidecars(audio_path: &Path, stem: &str, suffix: &str) {
    let parent = match audio_path.parent() {
        Some(p) => p,
        None => return,
    };
    for sidecar_ext in CODEC_RENAME_SIDECAR_EXTENSIONS {
        let sidecar = parent.join(format!("{stem}.{sidecar_ext}"));
        if !sidecar.exists() {
            continue;
        }
        // Idempotency: if a prior run already produced the suffixed
        // sidecar and something else created the clean-stem one in the
        // meantime (e.g. an overlapping companion run), leave both alone.
        let new_sidecar =
            parent.join(format!("{stem} {suffix}.{sidecar_ext}"));
        if new_sidecar.exists() {
            log::debug!(
                "Codec sidecar rename: target {} already exists — leaving {} in place",
                new_sidecar.display(),
                sidecar.display()
            );
            continue;
        }
        match crate::utils::fs_safe::safe_rename(&sidecar, &new_sidecar) {
            Ok(final_path) => log::debug!(
                "Codec sidecar: {} → {}",
                sidecar.display(),
                final_path.display()
            ),
            Err(e) => log::warn!(
                "Failed to rename sidecar {} with codec suffix: {e}",
                sidecar.display()
            ),
        }
    }
}

/// Rename a single track file to include its content advisory suffix.
/// Returns the new path (or the original if no rename was needed).
fn rename_track_with_advisory(
    file_path: &Path,
    tracks: &[apple_music_api::TrackMetadata],
) -> PathBuf {
    // Read track/disc number from M4A tags to match against API metadata
    let Ok(tag) = mp4ameta::Tag::read_from_path(file_path) else {
        return file_path.to_path_buf();
    };
    let track_num = tag.track_number();
    let disc_num = tag.disc_number().unwrap_or(1);
    let matched = match_track_to_metadata(track_num, disc_num, tracks);

    let Some(track_meta) = matched else {
        return file_path.to_path_buf();
    };

    let Some(ref rating) = track_meta.content_rating else {
        return file_path.to_path_buf();
    };

    let Some(suffix) = advisory_suffix(rating) else {
        return file_path.to_path_buf();
    };

    // Check if the file stem already contains this suffix (idempotency).
    // Matches case-insensitively so re-runs don't double-suffix a file
    // that was renamed during a previous attempt with different casing.
    let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem.to_ascii_lowercase().contains(&suffix.to_ascii_lowercase()) {
        return file_path.to_path_buf();
    }

    // Build new filename: insert advisory suffix BEFORE any codec suffix
    // like [Lossless] or [Dolby Atmos]. If no codec suffix, append at end.
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("m4a");
    let new_stem = insert_advisory_before_codec_suffix(stem, suffix);
    let new_name = format!("{new_stem}.{ext}");
    let new_path = file_path.with_file_name(&new_name);

    // `safe_rename` returns the actual landing path — important here
    // because collision disambiguation changes it, and subsequent
    // enrichment steps must operate on the real path, not the ideal
    // one we asked for.
    match crate::utils::fs_safe::safe_rename(file_path, &new_path) {
        Ok(final_path) => {
            log::debug!(
                "Advisory suffix: {} → {}",
                file_path.display(),
                final_path.display()
            );
            final_path
        }
        Err(e) => {
            log::warn!(
                "Failed to rename {} with advisory suffix: {e}",
                file_path.display()
            );
            file_path.to_path_buf()
        }
    }
}

/// Collect every non-empty codec suffix defined in the codec registry.
///
/// Used to reorder filenames so the advisory suffix (`[Explicit]` /
/// `[Clean]`) always precedes the codec suffix, regardless of which codec
/// suffix happens to be in use. Without this, suffixes outside a small
/// hardcoded list (`[Binaural]`, `[Downmix]`, `[AAC Legacy]`, `[HE-AAC]`,
/// etc.) would end up after the advisory instead of before it.
fn registry_codec_suffixes() -> Vec<&'static str> {
    use crate::models::codec_registry::CODEC_REGISTRY;
    CODEC_REGISTRY
        .audio
        .iter()
        .filter_map(|c| c.suffix.as_deref())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Insert the advisory suffix before any existing codec suffix (e.g.
/// `[Lossless]`, `[Dolby Atmos]`, `[Dolby Digital]`, `[Binaural]`,
/// `[Downmix]`, `[AAC Legacy]`, `[HE-AAC]`).
/// If no codec suffix is found, appends at the end.
fn insert_advisory_before_codec_suffix(stem: &str, advisory: &str) -> String {
    // Pull the full codec-suffix set from the registry so every codec
    // recognised by MeedyaDL ends up with the correct `[Explicit]/[Clean]`
    // ordering — not just the three originally hardcoded here.
    for cs in registry_codec_suffixes() {
        if let Some(pos) = stem.find(cs) {
            // Insert advisory before the codec suffix
            let before = stem[..pos].trim_end();
            let after = &stem[pos..];
            return format!("{before} {advisory} {after}");
        }
    }

    // No codec suffix found — append advisory at end
    format!("{stem} {advisory}")
}

/// Rename the album folder to include the album-level content advisory suffix.
/// Returns the new path (or the original if no rename was needed).
fn rename_folder_with_advisory(
    output_path: &str,
    album_metadata: &apple_music_api::AlbumMetadata,
) -> String {
    let path = Path::new(output_path);

    // Only rename directories, not single files
    if !path.is_dir() {
        return output_path.to_string();
    }

    let Some(ref rating) = album_metadata.content_rating else {
        return output_path.to_string();
    };

    let Some(suffix) = advisory_suffix(rating) else {
        return output_path.to_string();
    };

    // Check if folder name already contains the suffix (idempotency,
    // case-insensitive to tolerate mixed-case renames from previous runs).
    let folder_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if folder_name
        .to_ascii_lowercase()
        .contains(&suffix.to_ascii_lowercase())
    {
        return output_path.to_string();
    }

    let new_folder_name = format!("{folder_name} {suffix}");
    let new_path = path.with_file_name(&new_folder_name);

    // Use `rename_if_dest_free` rather than `safe_rename` — merging
    // two logically-distinct album folders under a numerically
    // disambiguated name (`Album [Explicit].1`) would be worse than
    // leaving the folder un-suffixed. If the suffixed name is taken,
    // skip the rename, log a warning, and let the caller keep using
    // the original path.
    match crate::utils::fs_safe::rename_if_dest_free(path, &new_path) {
        Ok(true) => {
            log::info!(
                "Advisory suffix: folder {} → {}",
                folder_name,
                new_folder_name
            );
            new_path.to_string_lossy().to_string()
        }
        Ok(false) => {
            log::warn!(
                "Advisory folder rename skipped — {} already exists \
                 (keeping files in {})",
                new_path.display(),
                folder_name
            );
            output_path.to_string()
        }
        Err(e) => {
            log::warn!(
                "Failed to rename folder {} with advisory suffix: {e}",
                folder_name
            );
            output_path.to_string()
        }
    }
}

// ============================================================
// Internal: File Collection
// ============================================================

// ============================================================
// iTunes API Supplementary Tags (#454)
// ============================================================

/// Apply supplementary metadata from the iTunes Lookup API.
///
/// Writes iTunes-exclusive fields (price, currency, country, disc count)
/// to M4A files as freeform atoms. Only writes fields not already present
/// from the Apple Music API enrichment. Matches tracks by track number
/// and disc number.
///
/// Returns the number of files enriched.
pub fn apply_itunes_supplementary_tags(
    output_path: &str,
    itunes_tracks: &[apple_music_api::ItunesTrackResult],
) -> usize {
    let files = collect_m4a_files(output_path);
    let mut count = 0;

    for file_path in &files {
        let tag = match mp4ameta::Tag::read_from_path(file_path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let track_num = tag.track_number();
        let disc_num = tag.disc_number().unwrap_or(1);
        drop(tag);

        // Match to iTunes track by track/disc number
        let matched = itunes_tracks.iter().find(|t| {
            t.track_number.map(|n| n as u16) == track_num
                && t.disc_number.unwrap_or(1) as u16 == disc_num
        });

        let Some(itunes) = matched else {
            continue;
        };

        // Re-open for writing
        let mut tag = match mp4ameta::Tag::read_from_path(file_path) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let mut wrote_any = false;

        // Write iTunes-exclusive fields as supplementary atoms
        // Note: Currency excluded (locale-dependent, changes with storefront)
        if let Some(ref country) = itunes.country {
            tag.set_data(
                mp4ameta::FreeformIdent::new_static(ITUNES_NAMESPACE, "Country"),
                mp4ameta::Data::Utf8(country.clone()),
            );
            wrote_any = true;
        }
        // Note: TrackPrice/CollectionPrice/Currency excluded from file metadata
        // because they are locale-dependent and change over time. Available in
        // the ItunesTrackResult struct for future use if needed.
        if let Some(disc_count) = itunes.disc_count {
            tag.set_data(
                mp4ameta::FreeformIdent::new_static(ITUNES_NAMESPACE, "DiscCount"),
                mp4ameta::Data::Utf8(disc_count.to_string()),
            );
            wrote_any = true;
        }
        if let Some(ref view_url) = itunes.track_view_url {
            tag.set_data(
                mp4ameta::FreeformIdent::new_static(ITUNES_NAMESPACE, "iTunesTrackURL"),
                mp4ameta::Data::Utf8(view_url.clone()),
            );
            wrote_any = true;
        }

        if wrote_any {
            if let Err(e) = tag.write_to_path(file_path) {
                log::debug!("Failed to write iTunes tags to {}: {e}", file_path.display());
            } else {
                count += 1;
            }
        }
    }

    count
}

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
        // Collect M4A files from the album directory with limited depth.
        // Depth 0 = album dir itself, depth 1 = disc subdirectories.
        // Do NOT recurse deeper to prevent cross-contamination when
        // the directory is an artist dir instead of album dir (#452).
        // Migrated to the shared `utils::fs_walk::walk_dir_depth` helper
        // (#716 finding #1, v1.0.3 prep). Depth limit and sidecar-skip
        // semantics preserved exactly.
        files.extend(crate::utils::fs_walk::walk_dir_depth(
            path,
            1,
            |p| {
                if crate::utils::fs_safe::is_filesystem_sidecar(p) {
                    return None;
                }
                if is_m4a(p) {
                    Some(p.to_path_buf())
                } else {
                    None
                }
            },
        ));
    }

    files
}

// ============================================================
// Internal: Channel Detection (via ffprobe)
// ============================================================

/// Resolve the ffprobe binary path (sibling to the managed `FFmpeg` binary).
///
/// ffprobe ships alongside `FFmpeg` in the same download archive. Its path
/// is derived by replacing the `FFmpeg` binary name with "ffprobe".
pub fn get_ffprobe_path(app: &AppHandle) -> Result<PathBuf, String> {
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

/// Audio stream information detected by ffprobe.
///
/// Contains the channel configuration, codec name, and profile extracted
/// from the first audio stream of an M4A file. Used for both channel
/// tagging and codec detection (to determine actual codec when GAMDL
/// uses native priority and doesn't report which codec was selected).
pub struct FfprobeAudioInfo {
    /// Channel configuration string (e.g., "2.0", "5.1", "7.1").
    channel_config: String,
    /// FFmpeg codec name (e.g., "alac", "aac", "eac3", "ac3").
    codec_name: String,
    /// FFmpeg profile string (e.g., "LC", "HE-AAC", "HE-AACv2").
    /// Not all codecs report a profile — ALAC and AC3 typically don't.
    profile: Option<String>,
}

/// Detect audio stream information from an M4A file using ffprobe.
///
/// Runs ffprobe to inspect the first audio stream and extracts:
/// - Channel count → mapped to configuration string ("2.0", "5.1", etc.)
/// - Codec name (e.g., "alac", "aac", "eac3") for actual codec detection
/// - Codec profile (e.g., "LC", "HE-AAC") for AAC variant identification
///
/// Returns `None` if ffprobe fails or the file has no audio stream.
pub async fn detect_audio_info(ffprobe_path: &Path, file_path: &Path) -> Option<FfprobeAudioInfo> {
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
    let stream = json.get("streams")?.as_array()?.first()?;

    let channels = stream.get("channels")?.as_u64()?;
    let codec_name = stream.get("codec_name")?.as_str()?.to_string();
    let profile = stream
        .get("profile")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string);

    Some(FfprobeAudioInfo {
        channel_config: channels_to_config(channels),
        codec_name,
        profile,
    })
}

/// Determine the actual `SongCodec` from ffprobe analysis of a downloaded file.
///
/// For unambiguous codecs (ALAC, E-AC3/Atmos, AC3), ffprobe is definitive.
/// For AAC variants, ffprobe cannot distinguish binaural/downmix from standard
/// AAC — these are semantically different delivery modes of the same codec.
/// In those cases, the requested codec is trusted IF the ffprobe codec family
/// matches (i.e., ffprobe says "aac" and the requested codec is an AAC variant).
///
/// # Arguments
/// * `info` - Audio stream information from ffprobe
/// * `requested_codec` - The codec the user requested at download time
///
/// # Returns
/// The most accurate `SongCodec` determination, combining ffprobe analysis
/// with the requested codec as a hint for ambiguous cases.
pub fn resolve_codec_from_ffprobe(info: &FfprobeAudioInfo, requested_codec: &SongCodec) -> SongCodec {
    match info.codec_name.as_str() {
        // Unambiguous: ALAC is always lossless
        "alac" => SongCodec::Alac,
        // Unambiguous: E-AC3 (Enhanced AC-3) with JOC = Dolby Atmos
        "eac3" => SongCodec::Atmos,
        // Unambiguous: AC-3 = Dolby Digital
        "ac3" => SongCodec::Ac3,
        // Ambiguous: AAC family — binaural/downmix are indistinguishable
        // from standard AAC by audio analysis alone. Trust the requested
        // codec if it's an AAC variant; otherwise default to standard AAC.
        "aac" => match requested_codec {
            SongCodec::AacBinaural
            | SongCodec::AacHeBinaural
            | SongCodec::AacDownmix
            | SongCodec::AacHeDownmix
            | SongCodec::Aac
            | SongCodec::AacLegacy
            | SongCodec::AacHe
            | SongCodec::AacHeLegacy => requested_codec.clone(),
            // Requested non-AAC (e.g., Atmos) but got AAC — GAMDL fell back
            _ => SongCodec::Aac,
        },
        // Unknown codec — trust the request as a fallback
        _ => requested_codec.clone(),
    }
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
///
/// Also called by `download_queue.rs` for early metadata fetch at download
/// start, so artist_name and album_name are available in the progress bar
/// and activity log from the very first track.
pub async fn try_fetch_metadata(
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

    let team_id = settings.musickit_team_id.as_deref();
    let key_id = settings.musickit_key_id.as_deref();

    // Private key is stored in the OS keychain (sensitive credential).
    // Missing keychain credentials may still be okay when a build-time
    // embedded token is available.
    let private_key = match apple_music_api::get_private_key_from_keychain() {
        Ok(Some(key)) => Some(key),
        Ok(None) => None,
        Err(e) => {
            log::warn!("Failed to read MusicKit private key: {e}");
            None
        }
    };

    log_event("Apple Music API: resolving MusicKit developer token...");

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

    // Web-dev-key path: use the 3-tier premium-feature resolver (user
    // MusicKit JWT -> embedded build token -> web-player token from the OS
    // keychain) rather than the 2-tier basic-catalog resolver. Without this,
    // a user who authenticated only via the Apple Music web player (no
    // MusicKit developer credentials and no embedded build token) received
    // `None` album metadata during enrichment, which silently disabled the
    // Step 1b syllable / word-by-word lyrics fetch (and the richer Apple
    // Music API tags) -- even though Step 1b itself already resolves the
    // web-player token. The animated-artwork path already fetches catalog
    // metadata with this resolver, so this aligns enrichment with the
    // established pattern. Tier 1 (user JWT) still wins whenever present, so
    // the MusicKit-credentials path is unchanged.
    let jwt = match apple_music_api::resolve_premium_feature_token(
        team_id,
        key_id,
        private_key.as_deref(),
    ) {
        Ok(pair) => {
            let Some((jwt, token_source)) = pair else {
                log_event(
                    "Apple Music API: no MusicKit credentials, embedded token, or web-player token available, skipping API metadata",
                );
                return None;
            };
            log_event("Apple Music API: token ready, fetching album metadata...");
            // Verbose: show the resolved token source (user credentials /
            // embedded build token / web player) for debugging auth issues.
            if let Some((app_handle, dl_id)) = event_context {
                crate::utils::activity_log::emit_verbose_download_log(
                    app_handle,
                    dl_id,
                    &format!("MusicKit token source: {token_source}"),
                );
            }
            jwt
        }
        Err(e) => {
            log::warn!("Failed to resolve MusicKit token for enrichment: {e}");
            log_event(&format!("Apple Music API: token resolution failed: {e}"));
            return None;
        }
    };

    match apple_music_api::fetch_album_metadata_with_fallback(
        &jwt,
        &parsed.storefront,
        &parsed.album_id,
    )
    .await
    {
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
                audio_traits: vec!["lossy-stereo".to_string(), "lossless".to_string()],
                is_apple_digital_master: Some(true),
                release_date: Some("2023-01-01".to_string()),
                composer_name: Some("Composer One".to_string()),
                duration_in_millis: Some(240_000),
                has_lyrics: Some(true),
                play_params_id: Some("100".to_string()),
                url: Some("https://music.apple.com/us/album/track-one/1?i=100".to_string()),
                preview_url: Some("https://audio-ssl.itunes.apple.com/100.m4a".to_string()),
                genre_names: vec!["Pop".to_string(), "Music".to_string()],
                raw_json: serde_json::Value::Null,
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
                audio_traits: vec!["lossy-stereo".to_string(), "dolby-atmos".to_string()],
                is_apple_digital_master: None,
                release_date: None,
                composer_name: None,
                duration_in_millis: Some(180_000),
                has_lyrics: Some(false),
                play_params_id: Some("200".to_string()),
                url: None,
                preview_url: None,
                genre_names: vec![],
                raw_json: serde_json::Value::Null,
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
                audio_traits: vec![],
                is_apple_digital_master: None,
                release_date: None,
                composer_name: None,
                duration_in_millis: None,
                has_lyrics: None,
                play_params_id: None,
                url: None,
                preview_url: None,
                genre_names: vec![],
                raw_json: serde_json::Value::Null,
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

    // ----------------------------------------------------------
    // Codec detection from ffprobe tests
    // ----------------------------------------------------------

    fn make_audio_info(codec_name: &str, profile: Option<&str>) -> FfprobeAudioInfo {
        FfprobeAudioInfo {
            channel_config: "2.0".to_string(),
            codec_name: codec_name.to_string(),
            profile: profile.map(std::string::ToString::to_string),
        }
    }

    #[test]
    fn resolve_alac_is_definitive() {
        // ffprobe says ALAC — always ALAC regardless of requested codec
        let info = make_audio_info("alac", None);
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Atmos),
            SongCodec::Alac
        ));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Aac),
            SongCodec::Alac
        ));
    }

    #[test]
    fn resolve_eac3_is_atmos() {
        // ffprobe says E-AC3 (Enhanced AC-3 with JOC = Dolby Atmos)
        let info = make_audio_info("eac3", None);
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Atmos),
            SongCodec::Atmos
        ));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Aac),
            SongCodec::Atmos
        ));
    }

    #[test]
    fn resolve_ac3_is_dolby_digital() {
        // ffprobe says AC-3 = Dolby Digital
        let info = make_audio_info("ac3", None);
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Atmos),
            SongCodec::Ac3
        ));
    }

    #[test]
    fn resolve_aac_trusts_binaural_request() {
        // ffprobe says AAC, requested binaural — trust the request
        let info = make_audio_info("aac", Some("LC"));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::AacBinaural),
            SongCodec::AacBinaural
        ));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::AacHeBinaural),
            SongCodec::AacHeBinaural
        ));
    }

    #[test]
    fn resolve_aac_trusts_downmix_request() {
        // ffprobe says AAC, requested downmix — trust the request
        let info = make_audio_info("aac", Some("LC"));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::AacDownmix),
            SongCodec::AacDownmix
        ));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::AacHeDownmix),
            SongCodec::AacHeDownmix
        ));
    }

    #[test]
    fn resolve_aac_trusts_standard_aac_variants() {
        // ffprobe says AAC, requested standard AAC — trust the request
        let info = make_audio_info("aac", Some("LC"));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Aac),
            SongCodec::Aac
        ));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::AacLegacy),
            SongCodec::AacLegacy
        ));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::AacHe),
            SongCodec::AacHe
        ));
    }

    #[test]
    fn resolve_aac_overrides_non_aac_request() {
        // ffprobe says AAC but user requested Atmos — GAMDL fell back to AAC
        let info = make_audio_info("aac", Some("LC"));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Atmos),
            SongCodec::Aac
        ));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Alac),
            SongCodec::Aac
        ));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Ac3),
            SongCodec::Aac
        ));
    }

    #[test]
    fn resolve_unknown_codec_trusts_request() {
        // Unknown codec from ffprobe — fall back to requested
        let info = make_audio_info("opus", None);
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Atmos),
            SongCodec::Atmos
        ));
        assert!(matches!(
            resolve_codec_from_ffprobe(&info, &SongCodec::Alac),
            SongCodec::Alac
        ));
    }

    // ----------------------------------------------------------
    // Content advisory suffix tests
    // ----------------------------------------------------------

    #[test]
    fn advisory_suffix_explicit() {
        assert_eq!(advisory_suffix("explicit"), Some("[Explicit]"));
    }

    #[test]
    fn advisory_suffix_clean() {
        assert_eq!(advisory_suffix("clean"), Some("[Clean]"));
    }

    #[test]
    fn advisory_suffix_none_for_unknown() {
        assert_eq!(advisory_suffix("notRated"), None);
        assert_eq!(advisory_suffix(""), None);
    }

    #[test]
    fn advisory_suffix_case_insensitive() {
        assert_eq!(advisory_suffix("Explicit"), Some("[Explicit]"));
        assert_eq!(advisory_suffix("EXPLICIT"), Some("[Explicit]"));
        assert_eq!(advisory_suffix("CLEAN"), Some("[Clean]"));
        assert_eq!(advisory_suffix("  explicit  "), Some("[Explicit]"));
    }

    #[test]
    fn insert_advisory_before_binaural_suffix() {
        // Regression for #482 — registry-driven suffix list covers every
        // codec, not just the three originally hardcoded ones.
        assert_eq!(
            insert_advisory_before_codec_suffix("01 Title [Binaural]", "[Explicit]"),
            "01 Title [Explicit] [Binaural]"
        );
    }

    #[test]
    fn insert_advisory_before_aac_legacy_suffix() {
        assert_eq!(
            insert_advisory_before_codec_suffix("01 Title [AAC Legacy]", "[Clean]"),
            "01 Title [Clean] [AAC Legacy]"
        );
    }

    #[test]
    fn insert_advisory_before_he_aac_suffix() {
        assert_eq!(
            insert_advisory_before_codec_suffix("01 Title [HE-AAC]", "[Explicit]"),
            "01 Title [Explicit] [HE-AAC]"
        );
    }

    #[test]
    fn insert_advisory_before_downmix_suffix() {
        assert_eq!(
            insert_advisory_before_codec_suffix("01 Title [Downmix]", "[Explicit]"),
            "01 Title [Explicit] [Downmix]"
        );
    }

    #[test]
    fn insert_advisory_no_codec_suffix() {
        // No codec suffix → advisory appended at end
        assert_eq!(
            insert_advisory_before_codec_suffix("01 Title", "[Explicit]"),
            "01 Title [Explicit]"
        );
    }

    #[test]
    fn insert_advisory_before_lossless_suffix() {
        // Advisory should come before [Lossless]
        assert_eq!(
            insert_advisory_before_codec_suffix("01 Title [Lossless]", "[Explicit]"),
            "01 Title [Explicit] [Lossless]"
        );
    }

    #[test]
    fn insert_advisory_before_dolby_atmos_suffix() {
        assert_eq!(
            insert_advisory_before_codec_suffix("01 Title [Dolby Atmos]", "[Clean]"),
            "01 Title [Clean] [Dolby Atmos]"
        );
    }

    #[test]
    fn insert_advisory_before_dolby_digital_suffix() {
        assert_eq!(
            insert_advisory_before_codec_suffix("01 Title [Dolby Digital]", "[Explicit]"),
            "01 Title [Explicit] [Dolby Digital]"
        );
    }

    #[test]
    fn insert_advisory_idempotent_check() {
        // The stem already contains the advisory — this tests the caller's
        // idempotency check (rename_track_with_advisory does `stem.contains(suffix)`)
        // but insert_advisory_before_codec_suffix itself doesn't guard against this
        let stem = "01 Title [Explicit]";
        // Caller should NOT call this if stem already contains the suffix
        assert!(stem.contains("[Explicit]"));
    }

    // ----------------------------------------------------------
    // Codec sidecar rename tests (#535)
    // ----------------------------------------------------------

    #[test]
    fn rename_matching_sidecars_renames_all_formats() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("01 Track [Dolby Atmos].m4a");
        std::fs::write(&audio, b"audio").unwrap();
        for ext in ["ttml", "lrc", "srt", "vtt", "ass"] {
            std::fs::write(
                dir.path().join(format!("01 Track.{ext}")),
                b"sidecar",
            )
            .unwrap();
        }

        // Audio is already suffix-renamed; sidecars still on clean stem.
        rename_matching_sidecars(&audio, "01 Track", "[Dolby Atmos]");

        for ext in ["ttml", "lrc", "srt", "vtt", "ass"] {
            let renamed =
                dir.path().join(format!("01 Track [Dolby Atmos].{ext}"));
            assert!(
                renamed.exists(),
                "expected sidecar {renamed:?} to be renamed with codec suffix"
            );
            let orig = dir.path().join(format!("01 Track.{ext}"));
            assert!(
                !orig.exists(),
                "expected original sidecar {orig:?} to be gone after rename"
            );
        }
    }

    #[test]
    fn rename_matching_sidecars_skips_missing() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("01 Track [Lossless].m4a");
        std::fs::write(&audio, b"audio").unwrap();
        // Only .ttml present — others missing.
        std::fs::write(dir.path().join("01 Track.ttml"), b"tt").unwrap();

        rename_matching_sidecars(&audio, "01 Track", "[Lossless]");

        assert!(dir.path().join("01 Track [Lossless].ttml").exists());
        assert!(!dir.path().join("01 Track.ttml").exists());
        // Missing sidecars don't fail the call.
        assert!(!dir.path().join("01 Track [Lossless].lrc").exists());
    }

    #[test]
    fn rename_matching_sidecars_preserves_existing_suffixed_target() {
        // If a suffixed sidecar already exists (e.g. from an overlapping
        // companion run that wrote it with the suffix baked in) and a
        // stale clean-stem sidecar is also present, neither should be
        // touched — we don't want safe_rename's auto-disambiguation
        // creating "01 Track [Dolby Atmos] (1).ttml" noise files.
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("01 Track [Dolby Atmos].m4a");
        std::fs::write(&audio, b"audio").unwrap();
        std::fs::write(dir.path().join("01 Track.ttml"), b"old").unwrap();
        std::fs::write(
            dir.path().join("01 Track [Dolby Atmos].ttml"),
            b"new",
        )
        .unwrap();

        rename_matching_sidecars(&audio, "01 Track", "[Dolby Atmos]");

        // Both files still exist untouched.
        assert!(dir.path().join("01 Track.ttml").exists());
        let suffixed = dir.path().join("01 Track [Dolby Atmos].ttml");
        assert!(suffixed.exists());
        assert_eq!(std::fs::read(&suffixed).unwrap(), b"new");
    }

    #[test]
    fn apply_codec_rename_suffix_renames_audio_and_sidecars() {
        // Integration-style test for the full public behaviour: the audio
        // M4A and every matching sidecar should all land on the same
        // suffixed stem.
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("05 Song.m4a");
        std::fs::write(&audio, b"audio").unwrap();
        for ext in ["ttml", "lrc", "srt", "vtt", "ass"] {
            std::fs::write(
                dir.path().join(format!("05 Song.{ext}")),
                b"sidecar",
            )
            .unwrap();
        }

        apply_codec_rename_suffix(&audio, "[Lossless]");

        assert!(dir.path().join("05 Song [Lossless].m4a").exists());
        assert!(!dir.path().join("05 Song.m4a").exists());
        for ext in ["ttml", "lrc", "srt", "vtt", "ass"] {
            let renamed =
                dir.path().join(format!("05 Song [Lossless].{ext}"));
            assert!(
                renamed.exists(),
                "sidecar .{ext} should follow audio rename"
            );
        }
    }

    #[test]
    fn apply_codec_rename_suffix_idempotent_on_already_suffixed_stem() {
        // Companion tier that wrote suffix-in-template already has the
        // suffix baked in — re-running rename should be a no-op and
        // must not touch adjacent sidecars either.
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("05 Song [Lossless].m4a");
        std::fs::write(&audio, b"audio").unwrap();
        std::fs::write(
            dir.path().join("05 Song [Lossless].ttml"),
            b"side",
        )
        .unwrap();

        apply_codec_rename_suffix(&audio, "[Lossless]");

        assert!(dir.path().join("05 Song [Lossless].m4a").exists());
        assert!(dir.path().join("05 Song [Lossless].ttml").exists());
        // No double-suffixed variants produced.
        assert!(
            !dir.path()
                .join("05 Song [Lossless] [Lossless].m4a")
                .exists()
        );
        assert!(
            !dir.path()
                .join("05 Song [Lossless] [Lossless].ttml")
                .exists()
        );
    }

    // ----------------------------------------------------------
    // Advisory sidecar rename tests (#788)
    // ----------------------------------------------------------

    /// All five sidecar formats follow the audio when the post-companion
    /// advisory pass renames `01 Track [Lossless].m4a` →
    /// `01 Track [Explicit] [Lossless].m4a`.
    #[test]
    fn rename_matching_advisory_sidecars_renames_all_formats() {
        let dir = tempfile::tempdir().unwrap();
        let new_audio = dir
            .path()
            .join("01 Track [Explicit] [Lossless].m4a");
        // Audio already renamed in place by the caller; sidecars
        // still on the old (pre-advisory) stem.
        std::fs::write(&new_audio, b"audio").unwrap();
        for ext in ["ttml", "lrc", "srt", "vtt", "ass"] {
            std::fs::write(
                dir.path().join(format!("01 Track [Lossless].{ext}")),
                b"sidecar",
            )
            .unwrap();
        }

        rename_matching_advisory_sidecars(
            &new_audio,
            "01 Track [Lossless]",
            "01 Track [Explicit] [Lossless]",
        );

        for ext in ["ttml", "lrc", "srt", "vtt", "ass"] {
            let renamed = dir
                .path()
                .join(format!("01 Track [Explicit] [Lossless].{ext}"));
            assert!(
                renamed.exists(),
                "expected sidecar .{ext} to be renamed with advisory suffix"
            );
            let orig =
                dir.path().join(format!("01 Track [Lossless].{ext}"));
            assert!(
                !orig.exists(),
                "expected original sidecar .{ext} to be gone after rename"
            );
        }
    }

    /// Missing sidecars don't cause errors — the helper is best-effort.
    #[test]
    fn rename_matching_advisory_sidecars_skips_missing() {
        let dir = tempfile::tempdir().unwrap();
        let new_audio = dir
            .path()
            .join("01 Track [Explicit] [Lossless].m4a");
        std::fs::write(&new_audio, b"audio").unwrap();
        // Only .lrc present — others missing.
        std::fs::write(
            dir.path().join("01 Track [Lossless].lrc"),
            b"side",
        )
        .unwrap();

        rename_matching_advisory_sidecars(
            &new_audio,
            "01 Track [Lossless]",
            "01 Track [Explicit] [Lossless]",
        );

        assert!(dir
            .path()
            .join("01 Track [Explicit] [Lossless].lrc")
            .exists());
        assert!(!dir.path().join("01 Track [Lossless].lrc").exists());
        // Missing .ttml/.srt/etc. don't get spurious targets.
        assert!(!dir
            .path()
            .join("01 Track [Explicit] [Lossless].ttml")
            .exists());
    }

    /// Idempotency: if a prior partial run created the advisory-renamed
    /// sidecar AND the old-stem sidecar still exists (e.g. a crash
    /// between audio and sidecar rename), don't blindly overwrite — leave
    /// both files alone for the user to reconcile.
    #[test]
    fn rename_matching_advisory_sidecars_preserves_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let new_audio = dir
            .path()
            .join("01 Track [Explicit] [Lossless].m4a");
        std::fs::write(&new_audio, b"audio").unwrap();
        std::fs::write(
            dir.path().join("01 Track [Lossless].ttml"),
            b"old",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("01 Track [Explicit] [Lossless].ttml"),
            b"new",
        )
        .unwrap();

        rename_matching_advisory_sidecars(
            &new_audio,
            "01 Track [Lossless]",
            "01 Track [Explicit] [Lossless]",
        );

        // Both files still exist, untouched.
        assert!(dir.path().join("01 Track [Lossless].ttml").exists());
        let suffixed = dir
            .path()
            .join("01 Track [Explicit] [Lossless].ttml");
        assert!(suffixed.exists());
        assert_eq!(std::fs::read(&suffixed).unwrap(), b"new");
        assert_eq!(
            std::fs::read(dir.path().join("01 Track [Lossless].ttml"))
                .unwrap(),
            b"old"
        );
    }

    /// Track with no codec suffix at all (single-codec mode where the
    /// primary doesn't need a suffix) → sidecars rename from `01 Track.lrc`
    /// to `01 Track [Explicit].lrc`. Confirms the helper handles the
    /// suffix-less case.
    #[test]
    fn rename_matching_advisory_sidecars_handles_no_codec_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let new_audio = dir.path().join("01 Track [Explicit].m4a");
        std::fs::write(&new_audio, b"audio").unwrap();
        std::fs::write(dir.path().join("01 Track.lrc"), b"side").unwrap();

        rename_matching_advisory_sidecars(
            &new_audio,
            "01 Track",
            "01 Track [Explicit]",
        );

        assert!(dir.path().join("01 Track [Explicit].lrc").exists());
        assert!(!dir.path().join("01 Track.lrc").exists());
    }
}
