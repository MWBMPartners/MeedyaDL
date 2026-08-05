// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See the LICENSE file in the project
// root for full licence text.
//
//! Cover-art fallback chain (#756).
//!
//! GAMDL writes the static album cover as `Cover.<ext>` based on the
//! user's `cover_format` setting (`raw`, `png`, or `jpg`). When
//! `cover_format = raw`, GAMDL preserves whatever bytes Apple's CDN
//! serves (often JPEG, sometimes PNG or WebP). The RAW path is
//! historically the **least reliable** because the cover-bytes fetch
//! goes through `httpx` and a single timeout / 5xx leaves the album
//! folder with no static cover at all — even though the embedded
//! cover atom inside each M4A is still present.
//!
//! This module rescues those albums by walking a deterministic
//! fallback chain — `RAW` → `PNG` → `JPEG` — using the static artwork
//! URL template that we already extract from the Apple Music API
//! response (`AlbumMetadata::artwork_url_template`). The fetch is a
//! single HTTP GET per attempt; PNG and JPEG attempts cost one extra
//! request each in the worst case.
//!
//! # When the fallback runs
//!
//! - The cover is **missing** from the album directory, OR
//! - The cover **exists** but is suspiciously small (< 4 KB —
//!   GAMDL has been observed to leave behind a zero-byte
//!   `Cover.raw` when the upstream fetch raised mid-stream).
//!
//! Successful GAMDL writes (any format, sane file size) are skipped —
//! this module never makes an outbound request when GAMDL did its
//! job.
//!
//! # Why not re-run GAMDL
//!
//! Re-running GAMDL costs minutes (whole-album re-download). The
//! Apple Music artwork URL is already in our hands at this point in
//! the enrichment pipeline (we fetch the album metadata earlier), so
//! a single HTTP request + a single disk write is the cheapest path
//! to a working cover.

use std::path::{Path, PathBuf};

use crate::models::gamdl_options::CoverFormat;
use crate::services::apple_music_api::AlbumMetadata;

/// Minimum file size considered "successful" — GAMDL occasionally
/// leaves behind a 0-byte or near-empty file when the cover-bytes
/// fetch is interrupted. 4 KiB comfortably exceeds any reasonable
/// HTTP error response and any real cover image is at least 30 KB
/// at 1000×1000 JPEG.
const MIN_VALID_COVER_BYTES: u64 = 4 * 1024;

/// Maximum dimension we'll request from Apple's CDN. Apple typically
/// serves up to 3000×3000 — going beyond that risks 404 on older
/// catalogue entries. We pick 3000 as the upper bound and clamp
/// against `AlbumMetadata::artwork_width` if reported smaller.
const TARGET_DIMENSION: u32 = 3000;

/// Outcome of an `ensure_cover_present` call. Carries enough detail
/// for the activity log to describe what happened without the caller
/// needing to re-derive state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverFallbackOutcome {
    /// The cover was already present and validly sized — nothing to
    /// do. This is the expected fast path on healthy downloads.
    AlreadyPresent { existing_path: PathBuf },
    /// We fetched a fallback format successfully and wrote it. The
    /// `format` reports which fallback won (PNG or JPEG); RAW never
    /// appears here because we cannot fabricate a RAW response from
    /// a known PNG / JPEG endpoint.
    FetchedFallback {
        format: CoverFormat,
        written_path: PathBuf,
    },
    /// No artwork URL template available in the album metadata
    /// (extremely rare — Apple always populates it for catalogue
    /// items). The embedded cover atom inside each M4A still works.
    NoTemplate,
    /// Both PNG and JPEG fallback fetches failed (4xx / 5xx /
    /// network). The user's M4A files still have an embedded cover;
    /// only the static sidecar is missing. We log and move on.
    AllFallbacksFailed { last_error: String },
}

/// Determines whether a static cover file is already on disk in the
/// requested format and meets the minimum-size sanity check.
///
/// Inspects `<album_dir>/<stem>.<ext>` for every conventional
/// alias (the user's configured `cover_art_name` plus the GAMDL
/// hardcoded "Cover" stem) so a successful GAMDL write isn't
/// misidentified as missing during the post-rename window.
fn find_existing_cover(album_dir: &Path, stems: &[&str], requested_ext: &str) -> Option<PathBuf> {
    for stem in stems {
        let candidate = album_dir.join(format!("{stem}.{requested_ext}"));
        if let Ok(metadata) = std::fs::metadata(&candidate) {
            if metadata.is_file() && metadata.len() >= MIN_VALID_COVER_BYTES {
                return Some(candidate);
            }
        }
    }
    None
}

/// Substitutes Apple's `{w}`, `{h}`, `{c}`, `{f}` placeholders in a
/// templated artwork URL.
///
/// Apple Music returns templates like
/// `https://is1-ssl.mzstatic.com/image/thumb/Music122/v4/93/.../source/{w}x{h}{c}.{f}`.
/// We substitute width/height to the album's reported native size
/// (clamped to [`TARGET_DIMENSION`]), `{c}` to `bb` (default black-bar
/// crop, matches what GAMDL emits), and `{f}` to the requested
/// extension.
fn build_artwork_url(template: &str, max_width: u32, max_height: u32, ext: &str) -> String {
    let target_w = max_width.min(TARGET_DIMENSION);
    let target_h = max_height.min(TARGET_DIMENSION);
    template
        .replace("{w}", &target_w.to_string())
        .replace("{h}", &target_h.to_string())
        .replace("{c}", "bb")
        .replace("{f}", ext)
}

/// Fetches a single artwork URL with a 30-second timeout.
///
/// Returns the response bytes on a `2xx` status, or a descriptive
/// error string for any other outcome (network failure, non-2xx, body
/// read failure). The error is intentionally short — it surfaces in
/// the activity log without overwhelming the line.
async fn fetch_artwork_bytes(url: &str) -> Result<Vec<u8>, String> {
    let client = crate::utils::http_client::build_simple(30)?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("network error: {e}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("body read failed: {e}"))?;
    Ok(bytes.to_vec())
}

/// Writes cover-art bytes atomically (temp file + rename) so a
/// crash mid-write can't leave a partial file behind that the next
/// run would mistake for a successful GAMDL output.
fn write_cover_atomically(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = target.with_extension(format!(
        "{}.tmp",
        target.extension().and_then(|e| e.to_str()).unwrap_or("img")
    ));
    std::fs::write(&tmp, bytes).map_err(|e| format!("write temp file: {e}"))?;
    std::fs::rename(&tmp, target).map_err(|e| format!("rename temp file: {e}"))?;
    Ok(())
}

/// The fallback formats we'll attempt, in order. RAW is intentionally
/// excluded from the *fallback* chain — if the user requested RAW and
/// GAMDL didn't produce it, we cannot fabricate the same byte stream
/// from the API (which serves PNG or JPEG depending on `{f}`).
const FALLBACK_FORMATS: &[(CoverFormat, &str)] =
    &[(CoverFormat::Png, "png"), (CoverFormat::Jpg, "jpg")];

/// Ensure a static cover image exists in `album_dir` for the user's
/// configured cover name. Falls back through `RAW` → `PNG` → `JPEG`
/// when GAMDL didn't write one.
///
/// # Parameters
///
/// * `album_dir` — directory where GAMDL wrote the album.
/// * `metadata` — album metadata fetched earlier in the enrichment
///   pipeline. The artwork URL template lives at
///   `metadata.artwork_url_template`.
/// * `requested_format` — the user's configured `cover_format`. Used
///   to decide whether to skip the fast-path check (we look for a
///   `Cover.<requested_ext>` first, falling back to checks for the
///   other extensions only if the requested one is missing — same
///   stem could exist as `Cover.png` from a previous run with
///   different settings).
/// * `cover_stem` — the user's configured cover stem (e.g.
///   `"FrontCover"`, `"Folder"`, or `"Cover"`). The fallback writes
///   `<cover_stem>.<format>`.
///
/// # Returns
///
/// A [`CoverFallbackOutcome`] describing what happened. The caller
/// emits an appropriate activity-log line based on the variant.
pub async fn ensure_cover_present(
    album_dir: &Path,
    metadata: &AlbumMetadata,
    requested_format: &CoverFormat,
    cover_stem: &str,
) -> CoverFallbackOutcome {
    if !album_dir.is_dir() {
        // The directory doesn't exist — likely a download that
        // failed before GAMDL wrote anything. Nothing for us to do.
        return CoverFallbackOutcome::NoTemplate;
    }

    // Fast path: any existing cover file (in any format) under the
    // user's stem OR GAMDL's hardcoded "Cover" stem counts as a
    // success. Inspecting all three extensions catches the case
    // where GAMDL fell back internally to a different format than
    // the user requested (rare but possible).
    let stems = [cover_stem, "Cover"];
    let requested_ext = requested_format.to_cli_string();
    if let Some(existing) = find_existing_cover(album_dir, &stems, requested_ext) {
        return CoverFallbackOutcome::AlreadyPresent {
            existing_path: existing,
        };
    }
    // Also check the alternative extensions — a previously-completed
    // run with `cover_format = png` would leave `Cover.png`, and we
    // don't want to overwrite it just because the current run is on
    // RAW.
    for alt_ext in ["raw", "png", "jpg"] {
        if alt_ext == requested_ext {
            continue;
        }
        if let Some(existing) = find_existing_cover(album_dir, &stems, alt_ext) {
            return CoverFallbackOutcome::AlreadyPresent {
                existing_path: existing,
            };
        }
    }

    // Need to fetch. Bail if we don't have an artwork URL template —
    // happens for a small set of unusual catalogue items, and for
    // playlists / non-album content that we don't fall back on.
    let Some(template) = metadata.artwork_url_template.as_deref() else {
        return CoverFallbackOutcome::NoTemplate;
    };

    let max_w = metadata.artwork_width.unwrap_or(TARGET_DIMENSION);
    let max_h = metadata.artwork_height.unwrap_or(TARGET_DIMENSION);

    let mut last_error = "no fallback formats configured".to_string();
    for (format, ext) in FALLBACK_FORMATS {
        let url = build_artwork_url(template, max_w, max_h, ext);
        log::debug!("Cover-art fallback: trying {ext} from {url}");
        match fetch_artwork_bytes(&url).await {
            Ok(bytes) => {
                let target = album_dir.join(format!("{cover_stem}.{ext}"));
                if let Err(e) = write_cover_atomically(&target, &bytes) {
                    last_error = format!("{ext} fetched but write failed: {e}");
                    continue;
                }
                return CoverFallbackOutcome::FetchedFallback {
                    format: format.clone(),
                    written_path: target,
                };
            }
            Err(e) => {
                last_error = format!("{ext} fetch failed: {e}");
            }
        }
    }

    CoverFallbackOutcome::AllFallbacksFailed { last_error }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn metadata_with_template(template: Option<&str>) -> AlbumMetadata {
        AlbumMetadata {
            album_id: "1".to_string(),
            album_name: Some("Test".to_string()),
            upc: None,
            content_rating: None,
            genre_names: vec![],
            artist_id: None,
            artist_name: None,
            record_label: None,
            copyright: None,
            release_date: None,
            last_modified_date: None,
            is_compilation: None,
            is_single: None,
            is_complete: None,
            is_mastered_for_itunes: None,
            track_count: None,
            editorial_notes: None,
            tracks: vec![],
            artwork_square_url: None,
            artwork_tall_url: None,
            album_spotlight_url: None,
            fallback_storefront: None,
            artwork_url_template: template.map(str::to_string),
            artwork_width: Some(3000),
            artwork_height: Some(3000),
            raw_json: serde_json::Value::Null,
        }
    }

    #[test]
    fn build_artwork_url_substitutes_all_placeholders() {
        let template = "https://example.com/source/{w}x{h}{c}.{f}";
        let url = build_artwork_url(template, 3000, 3000, "png");
        assert_eq!(url, "https://example.com/source/3000x3000bb.png");
    }

    #[test]
    fn build_artwork_url_clamps_oversized_dimensions() {
        let template = "https://example.com/{w}x{h}.{f}";
        let url = build_artwork_url(template, 9999, 9999, "jpg");
        assert_eq!(url, "https://example.com/3000x3000.jpg");
    }

    #[test]
    fn find_existing_cover_finds_file_above_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Cover.jpg");
        // Write 5 KiB of dummy data — above MIN_VALID_COVER_BYTES.
        fs::write(&path, vec![0u8; 5 * 1024]).unwrap();
        let found = find_existing_cover(dir.path(), &["Cover"], "jpg");
        assert_eq!(found.as_deref(), Some(path.as_path()));
    }

    #[test]
    fn find_existing_cover_rejects_files_below_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Cover.jpg");
        // Write 100 bytes — below MIN_VALID_COVER_BYTES, so should
        // be treated as a failed GAMDL write that needs fallback.
        fs::write(&path, vec![0u8; 100]).unwrap();
        let found = find_existing_cover(dir.path(), &["Cover"], "jpg");
        assert!(found.is_none());
    }

    #[test]
    fn find_existing_cover_searches_multiple_stems() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("FrontCover.jpg");
        fs::write(&path, vec![0u8; 5 * 1024]).unwrap();
        let found = find_existing_cover(dir.path(), &["FrontCover", "Cover"], "jpg");
        assert_eq!(found.as_deref(), Some(path.as_path()));
    }

    #[tokio::test]
    async fn ensure_cover_present_returns_already_present_when_jpg_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Cover.jpg");
        fs::write(&path, vec![0u8; 5 * 1024]).unwrap();
        let metadata = metadata_with_template(Some("https://example.com/{w}x{h}.{f}"));
        let outcome =
            ensure_cover_present(dir.path(), &metadata, &CoverFormat::Jpg, "Cover").await;
        match outcome {
            CoverFallbackOutcome::AlreadyPresent { existing_path } => {
                assert_eq!(existing_path, path);
            }
            other => panic!("expected AlreadyPresent, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn ensure_cover_present_returns_already_present_when_alt_format_exists() {
        // User requested RAW but a previous run left Cover.png — we
        // should NOT re-download just because the requested ext is
        // different. Picking the existing file preserves user choice
        // (they may have manually swapped formats and want the
        // existing file kept).
        let dir = tempfile::tempdir().unwrap();
        let png_path = dir.path().join("Cover.png");
        fs::write(&png_path, vec![0u8; 5 * 1024]).unwrap();
        let metadata = metadata_with_template(Some("https://example.com/{w}x{h}.{f}"));
        let outcome =
            ensure_cover_present(dir.path(), &metadata, &CoverFormat::Raw, "Cover").await;
        match outcome {
            CoverFallbackOutcome::AlreadyPresent { existing_path } => {
                assert_eq!(existing_path, png_path);
            }
            other => panic!("expected AlreadyPresent, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn ensure_cover_present_returns_no_template_when_unset() {
        let dir = tempfile::tempdir().unwrap();
        // No file present, no template — nothing we can do.
        let metadata = metadata_with_template(None);
        let outcome =
            ensure_cover_present(dir.path(), &metadata, &CoverFormat::Raw, "Cover").await;
        assert_eq!(outcome, CoverFallbackOutcome::NoTemplate);
    }

    #[tokio::test]
    async fn ensure_cover_present_returns_no_template_when_dir_missing() {
        let metadata = metadata_with_template(Some("https://example.com/{w}x{h}.{f}"));
        let outcome = ensure_cover_present(
            Path::new("/nonexistent/path/that/does/not/exist"),
            &metadata,
            &CoverFormat::Raw,
            "Cover",
        )
        .await;
        assert_eq!(outcome, CoverFallbackOutcome::NoTemplate);
    }
}
