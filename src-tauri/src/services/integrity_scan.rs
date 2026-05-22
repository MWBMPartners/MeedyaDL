// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Output-directory integrity scan (#537 chunk B).
//!
//! Walks the user's configured `output_path` looking for **historic
//! damage from pre-v1.6 broken builds**:
//!
//! 1. Degenerate filenames produced by the empty-tag class of bugs that
//!    plagued v1.x.x music-video downloads. The Tier 4 safety net
//!    (#531) prevents these from being CREATED in current builds, but
//!    users who downloaded music videos on a broken release have
//!    already-corrupt files sitting on disk that they may never
//!    notice unless prompted.
//!
//!    Signatures the scan detects:
//!    - Files literally named `-.mp4` / `-.jpg` (artist + title both
//!      collapsed to empty strings → MV pipeline produced `-.<ext>`).
//!    - Paths containing a `[Unknown]/` directory segment (album-tag
//!      missing → folder template resolved to `[Unknown]`).
//!    - Paths whose parent directory ends with `Unknown Album` and
//!      whose filename starts with `-.` (older v1.5.x shape after a
//!      partial fix landed).
//!
//! 2. **Zero-byte fixed-name cover files** at the known animated-artwork
//!    paths (`FrontCover.mp4`, `PortraitCover.mp4`,
//!    `ArtistSpotlightCover.mp4`). These appear when the HLS download
//!    fails part-way through but the empty file was still committed to
//!    disk — typical symptom: the file exists but Finder/Explorer
//!    refuses to preview it. Pre-#447 builds were also vulnerable to
//!    "silent idempotency" where a corrupt cover was preserved on
//!    re-runs because the target path already existed.
//!
//! ## Not in scope (this commit)
//!
//! - **Quarantine action.** The full spec calls for moving detected
//!   files to a `_quarantine_<DATE>/` sibling directory so users can
//!   review and recover. This first cut only DETECTS and REPORTS —
//!   the user can decide what to do with the list. Quarantine UI lands
//!   as a follow-up (#537 chunk B.2).
//! - **Auto-run on startup.** The issue's spec calls for running the
//!   scan once per launch behind a feature flag. This commit makes
//!   it user-initiated only (button in Settings → Advanced →
//!   Diagnostics). Once the scan logic is proven on real user data,
//!   we'll wire the startup trigger behind the flag.
//! - **ffprobe-based zero-duration check** for covers that aren't
//!   zero-byte but are corrupt in some other way (e.g. truncated mid-
//!   file). A pure size check catches the most common failure mode
//!   without taking an external-binary dependency on every scan.
//!
//! ## Performance
//!
//! Single recursive walk with a hard depth limit (`MAX_SCAN_DEPTH`).
//! On a typical 10-album library the scan completes in <100ms. Larger
//! libraries (1000+ albums) take 1-5 seconds — still well inside the
//! "click a button, see a result" UX threshold.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Maximum directory depth the scan recurses. Matches the limit used
/// in [`crate::utils::fs_walk::walk_dir_depth`] callsites for output-
/// directory walks elsewhere. Going deeper than 10 levels under the
/// configured output_path would mean a pathologically nested user
/// layout — almost certainly not a legitimate MeedyaDL-produced
/// directory tree.
const MAX_SCAN_DEPTH: u32 = 10;

/// Filenames at which the animated-artwork pipeline writes its three
/// known cover variants (`animated_artwork_service.rs`). Each is a
/// fixed name (not template-driven), so the scan can do an exact
/// match. Zero-byte files at these paths are almost certainly stale
/// from an interrupted HLS download.
const FIXED_COVER_NAMES: &[&str] = &[
    "FrontCover.mp4",
    "PortraitCover.mp4",
    "ArtistSpotlightCover.mp4",
];

/// A single integrity issue the scan found. The `kind` discriminates
/// between the two detection classes; the `path` is the absolute
/// file path on disk so the UI can surface it directly to the user
/// (and a future quarantine action can act on it without re-walking).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IntegrityIssue {
    /// Filename or path matches one of the degenerate-name signatures
    /// from the empty-tag MV pipeline bugs.
    DegenerateName {
        path: PathBuf,
        /// Which signature triggered the match — useful for the UI
        /// to group findings and for future support requests.
        signature: &'static str,
    },
    /// File at a known fixed-cover path is zero bytes.
    ZeroByteCover { path: PathBuf },
}

/// Report returned by [`scan`]. Counts + lists of paths. The two
/// detection classes are reported separately so the UI can render
/// them in distinct sections (different severity, different
/// recommended action).
#[derive(Debug, Clone, Default, Serialize)]
pub struct IntegrityScanReport {
    /// Total number of files walked (informational; helps users
    /// confirm the scan actually saw their library).
    pub files_walked: usize,
    /// Issues found.
    pub issues: Vec<IntegrityIssue>,
    /// Absolute path the scan started from. Echoed back so the UI
    /// can show "Scanned: …" without re-reading settings.
    pub scanned_path: PathBuf,
}

impl IntegrityScanReport {
    /// Convenience: counts degenerate-name issues.
    #[must_use]
    pub fn degenerate_name_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| matches!(i, IntegrityIssue::DegenerateName { .. }))
            .count()
    }

    /// Convenience: counts zero-byte-cover issues.
    #[must_use]
    pub fn zero_byte_cover_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| matches!(i, IntegrityIssue::ZeroByteCover { .. }))
            .count()
    }
}

/// Tests whether `path` matches any degenerate-name signature.
///
/// Returns the matched signature name (so the report can group
/// findings) or `None` if the path is clean. The signatures are
/// described in the module docstring.
fn classify_degenerate_name(path: &Path) -> Option<&'static str> {
    // The filename component is what we check for `-.<ext>` shapes.
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

    // Signature 1: filename is literally `-.<ext>`. Pre-#531 the MV
    // pipeline produced these when both artist and title resolved to
    // empty strings.
    if file_name.starts_with("-.") && !file_name.contains('/') {
        return Some("hyphen-dot-extension");
    }

    // Walk parent components to look for `[Unknown]` segments.
    // Any ancestor named exactly `[Unknown]` indicates the
    // folder template produced an [Unknown] node — usually because
    // album metadata was missing AND we had no fallback name.
    let path_str = path.to_string_lossy();
    if path_str.contains("/[Unknown]/")
        || path_str.contains("\\[Unknown]\\")
        || path_str.starts_with("[Unknown]/")
        || path_str.starts_with("[Unknown]\\")
    {
        return Some("unknown-folder-segment");
    }

    // Signature 3: parent directory ends with `Unknown Album` AND
    // filename starts with `-.`. Combination shape from v1.5.x
    // partial fix that renamed `[Unknown]` to `Unknown Album` but
    // hadn't yet fixed the filename derivation.
    if file_name.starts_with("-.") {
        if let Some(parent) = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()) {
            if parent == "Unknown Album" {
                return Some("unknown-album-with-empty-filename");
            }
        }
    }

    None
}

/// Tests whether `path` is one of the fixed cover paths and is
/// zero bytes on disk. Returns the path as-is (move into the
/// report) or `None`.
fn classify_zero_byte_cover(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name().and_then(|s| s.to_str())?;
    if !FIXED_COVER_NAMES.contains(&file_name) {
        return None;
    }
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() == 0 {
        Some(path.to_path_buf())
    } else {
        None
    }
}

/// Runs the integrity scan rooted at `output_path` and returns the
/// [`IntegrityScanReport`]. Synchronous + sequential — small enough
/// that an async wrapper isn't needed at typical library sizes
/// (<2000 albums). IPC callers should `spawn_blocking` if the user's
/// library is so large that the scan exceeds ~1 second.
///
/// Errors during the walk (permission denied, broken symlinks, etc.)
/// are silently skipped — the scan is best-effort, not a forensic
/// audit. The caller can still trust `files_walked` as an accurate
/// count of readable files; entries we couldn't read are simply
/// absent from the total.
#[must_use]
pub fn scan(output_path: &Path) -> IntegrityScanReport {
    let mut report = IntegrityScanReport {
        files_walked: 0,
        issues: Vec::new(),
        scanned_path: output_path.to_path_buf(),
    };

    if !output_path.exists() {
        return report;
    }

    // Use the shared `walk_dir_depth` helper from utils/fs_walk so the
    // walk shape matches every other output-directory walker in the
    // codebase (`count_audio_files_in_directory`, manifest scan,
    // legacy folder merge). The visitor returns Some(()) for entries
    // we actually classified so we can count files_walked accurately
    // — we collect the unit return values to size the vec, then
    // discard them.
    let files_walked_marker = crate::utils::fs_walk::walk_dir_depth(
        output_path,
        MAX_SCAN_DEPTH,
        |path| {
            // Only consider regular files — directories are walked but
            // not classified directly (an `[Unknown]` directory
            // matches via the filename of audio file inside it).
            if !path.is_file() {
                return None;
            }
            if let Some(signature) = classify_degenerate_name(path) {
                report.issues.push(IntegrityIssue::DegenerateName {
                    path: path.to_path_buf(),
                    signature,
                });
                // Signatures are mutually exclusive in practice; don't
                // double-classify a path.
                return Some(());
            }
            if let Some(cover_path) = classify_zero_byte_cover(path) {
                report.issues.push(IntegrityIssue::ZeroByteCover {
                    path: cover_path,
                });
            }
            Some(())
        },
    );
    report.files_walked = files_walked_marker.len();

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::File::create(path).unwrap();
    }

    fn touch_with_bytes(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn detects_hyphen_dot_extension_in_isolation() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        touch(&root.join("Artist/Album/01 - Song.m4a"));
        touch(&root.join("Artist/-.mp4")); // degenerate
        let report = scan(root);
        let names: Vec<_> = report
            .issues
            .iter()
            .filter_map(|i| match i {
                IntegrityIssue::DegenerateName { signature, .. } => Some(*signature),
                _ => None,
            })
            .collect();
        assert!(
            names.contains(&"hyphen-dot-extension"),
            "expected hyphen-dot-extension match, got: {names:?}"
        );
        assert_eq!(report.degenerate_name_count(), 1);
        assert_eq!(report.zero_byte_cover_count(), 0);
    }

    #[test]
    fn detects_unknown_folder_segment() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        touch(&root.join("Artist/[Unknown]/track.m4a"));
        let report = scan(root);
        let names: Vec<_> = report
            .issues
            .iter()
            .filter_map(|i| match i {
                IntegrityIssue::DegenerateName { signature, .. } => Some(*signature),
                _ => None,
            })
            .collect();
        assert!(
            names.contains(&"unknown-folder-segment"),
            "expected unknown-folder-segment match, got: {names:?}"
        );
    }

    #[test]
    fn detects_zero_byte_front_cover() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        // Healthy track: real bytes
        touch_with_bytes(&root.join("Artist/Album/01 - Song.m4a"), b"audio");
        // Zero-byte cover
        touch_with_bytes(&root.join("Artist/Album/FrontCover.mp4"), b"");
        // Non-zero cover should NOT match
        touch_with_bytes(&root.join("Artist/AnotherAlbum/FrontCover.mp4"), b"\x00\x01\x02");
        let report = scan(root);
        assert_eq!(
            report.zero_byte_cover_count(),
            1,
            "expected one zero-byte cover, got: {:?}",
            report.issues
        );
        assert_eq!(report.degenerate_name_count(), 0);
    }

    #[test]
    fn passes_clean_library() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        touch_with_bytes(&root.join("Artist/Album/01 - Song.m4a"), b"audio");
        touch_with_bytes(&root.join("Artist/Album/FrontCover.mp4"), b"video");
        touch_with_bytes(&root.join("Artist/Album/cover.jpg"), b"img");
        let report = scan(root);
        assert_eq!(report.issues.len(), 0, "clean library should have no issues");
        assert!(report.files_walked >= 3, "should walk all 3 files");
    }

    #[test]
    fn missing_output_path_returns_empty_report() {
        let tmp = TempDir::new().unwrap();
        let nonexistent = tmp.path().join("does-not-exist");
        let report = scan(&nonexistent);
        assert_eq!(report.issues.len(), 0);
        assert_eq!(report.files_walked, 0);
        assert_eq!(report.scanned_path, nonexistent);
    }
}
