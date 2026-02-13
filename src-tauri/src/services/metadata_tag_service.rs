// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// metadata_tag_service.rs -- Post-download custom metadata tagging service
// ========================================================================
//
// After GAMDL finishes writing its standard Apple Music metadata (title,
// artist, album art, etc.), this service injects additional MeedyaDL-specific
// custom metadata tags into the downloaded M4A files. These tags allow
// downstream tools, scripts, and media library managers to programmatically
// identify the codec quality tier of each file:
//
//   - **Lossless (ALAC)**: `isLossless = Y`
//   - **Dolby Atmos**: `SpatialType = Dolby Atmos` (Apple iTunes namespace)
//                       `SpatialType = Dolby Atmos` (MeedyaMeta namespace)
//
// Tags are stored as MP4 "freeform" atoms (the `----` box type), which is
// the standard mechanism for custom metadata in the iTunes/M4A ecosystem.
// Each freeform atom has a "mean" (namespace/domain) and a "name" (key):
//
//   ----:com.apple.iTunes:isLossless     → "Y"
//   ----:com.apple.iTunes:SpatialType    → "Dolby Atmos"
//   ----:MeedyaMeta:SpatialType          → "Dolby Atmos"
//
// The MeedyaMeta namespace provides a MeedyaDL-branded namespace so these
// tags don't collide with any future Apple-defined atoms under the iTunes
// namespace.
//
// ## Safety
//
// The `mp4ameta` crate modifies only the MP4 metadata container atoms
// without re-encoding or touching the audio stream data. This is safe
// for both ALAC (lossless) and EC-3 (Dolby Atmos) audio payloads.
//
// ## Integration
//
// Called from `download_queue.rs` in the download success path, immediately
// after the GAMDL process exits and before the companion download starts.
// Runs synchronously within the download task's async context (file I/O
// is fast relative to network downloads).
//
// @see download_queue.rs -- Calls apply_codec_metadata_tags() after download
// @see mp4ameta docs: https://docs.rs/mp4ameta/

use std::path::Path;

use mp4ameta::{Data, FreeformIdent, Tag};

use crate::models::gamdl_options::SongCodec;

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
/// # Returns
///
/// * `Ok(count)` -- The number of files successfully tagged.
/// * `Err(message)` -- A human-readable error if the operation fails.
///   Individual file failures are logged at debug level but do not stop
///   processing of remaining files.
pub fn apply_codec_metadata_tags(
    output_path: &str,
    codec: &SongCodec,
) -> Result<usize, String> {
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
                    log::debug!(
                        "Failed to tag {}: {}",
                        path.display(),
                        e
                    );
                }
            }
        }
    } else if path.is_dir() {
        // Directory: walk and tag all M4A files recursively
        tagged_count += tag_directory_recursive(path, &tag_writer);
    } else {
        return Err(format!(
            "Output path does not exist: {}",
            output_path
        ));
    }

    Ok(tagged_count)
}

/// Tags a single M4A file by opening it, applying the tag writer function,
/// and saving the modified metadata back to disk.
fn tag_single_file(
    path: &Path,
    tag_writer: &dyn Fn(&mut Tag),
) -> Result<(), String> {
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
fn tag_directory_recursive(
    dir: &Path,
    tag_writer: &dyn Fn(&mut Tag),
) -> usize {
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
        .map(|ext| ext.eq_ignore_ascii_case("m4a"))
        .unwrap_or(false)
}
