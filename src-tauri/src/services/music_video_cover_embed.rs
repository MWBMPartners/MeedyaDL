// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Embed sidecar `.jpg` / `.png` cover thumbnails into the
//! corresponding music-video `.mp4` and delete the sidecar (#533 / #569).
//!
//! Pre-#533 GAMDL produced a `.mp4` plus a sibling `.jpg` thumbnail
//! every time `save_cover = true` (our default). For music videos this
//! is redundant — every modern player (VLC, mpv, QuickTime, Plex,
//! Jellyfin, IINA, macOS Finder, Windows Explorer) renders the
//! poster frame straight from the MP4 container without needing the
//! sidecar. The loose `.jpg` files just clutter the library, and on
//! the broken `[Unknown]/-.mp4` path (#527) they inherited the same
//! `-.jpg` collision.
//!
//! Option B from #533's decision matrix: embed the sidecar as a
//! `covr` atom in the MP4, then delete the sidecar after the embed is
//! verified by re-reading the atom back. The verify step protects
//! against the edge case where the embed silently fails — losing the
//! sidecar would be a data-loss bug, so the helper refuses to delete
//! unless the round-trip succeeded.
//!
//! Wired into the MV post-download enrichment loop in
//! `download_queue.rs::extract_music_video_subtitles_for_dl` next to
//! the existing subtitle-extract + lyrics-pair steps.

use std::path::{Path, PathBuf};

use mp4ameta::{Img, ImgFmt, Tag};

/// Outcome of an embed-and-delete attempt for one video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbedOutcome {
    /// No sidecar found next to the video — nothing to do.
    NoSidecar,
    /// Sidecar embedded into the MP4 and the sidecar file was
    /// deleted from disk. The user keeps the cover-art atom; the
    /// folder loses the redundant file.
    Embedded { sidecar_filename: String, bytes_embedded: usize },
    /// Sidecar exists but the embed failed for the reason in
    /// `reason`. The sidecar is **not** deleted — losing it on a
    /// failed embed would be a data-loss bug.
    Failed { sidecar_filename: String, reason: String },
}

/// Look for a sibling cover sidecar (`.jpg`, `.jpeg`, or `.png`) with
/// the same stem as `video_path`, embed it into the MP4 as a `covr`
/// atom, then delete the sidecar on success. Returns the outcome so
/// the caller (the MV post-download loop in `download_queue.rs`) can
/// surface an activity-log entry.
///
/// # Errors
///
/// Returns `Failed` rather than an `Err` for the common failure
/// modes (sidecar unreadable, MP4 write rejected, verify round-trip
/// mismatch). This keeps the call site one match arm per outcome
/// instead of nested Result/match handling.
pub fn embed_and_remove_sidecar(video_path: &Path) -> EmbedOutcome {
    let Some(sidecar_path) = find_sidecar(video_path) else {
        return EmbedOutcome::NoSidecar;
    };

    let sidecar_filename = sidecar_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("(unknown)")
        .to_string();

    // Read the sidecar bytes into memory. The thumbnails GAMDL
    // emits are small (typically 50–200 KB), so the read is
    // negligible. We don't stream-embed because mp4ameta's Img
    // takes a Vec<u8> by value anyway.
    let bytes = match std::fs::read(&sidecar_path) {
        Ok(b) => b,
        Err(e) => {
            return EmbedOutcome::Failed {
                sidecar_filename,
                reason: format!("could not read sidecar: {e}"),
            };
        }
    };

    let bytes_len = bytes.len();
    let fmt = detect_image_fmt(&sidecar_path, &bytes);
    let img = Img { fmt, data: bytes };

    // Open the MP4, set the artwork, write back. mp4ameta handles
    // the read-modify-write atomicity internally — it serialises
    // the whole atom tree on `write_to_path`.
    let mut tag = match Tag::read_from_path(video_path) {
        Ok(t) => t,
        Err(e) => {
            return EmbedOutcome::Failed {
                sidecar_filename,
                reason: format!("could not read MP4 tag atoms: {e}"),
            };
        }
    };
    tag.set_artwork(img);
    if let Err(e) = tag.write_to_path(video_path) {
        return EmbedOutcome::Failed {
            sidecar_filename,
            reason: format!("could not write covr atom to MP4: {e}"),
        };
    }

    // Verify: re-read the tag and confirm an artwork atom is now
    // present. Without this step a silent write failure would let
    // us delete the only copy of the cover — exactly the data-loss
    // bug we're trying to avoid.
    let verify = Tag::read_from_path(video_path);
    let embedded_ok = match verify {
        Ok(t) => t.artwork().is_some(),
        Err(_) => false,
    };
    if !embedded_ok {
        return EmbedOutcome::Failed {
            sidecar_filename,
            reason: "MP4 written but covr atom is absent on re-read (embed silently failed)".to_string(),
        };
    }

    // Embed verified — safe to delete the sidecar.
    if let Err(e) = std::fs::remove_file(&sidecar_path) {
        // Embed succeeded but delete failed. Don't claim "Failed" —
        // the embed worked, the user just has a stale sidecar.
        // Surface as a Failed-with-clear-reason so the caller can
        // emit a warn line, but don't roll back the embed.
        return EmbedOutcome::Failed {
            sidecar_filename,
            reason: format!(
                "MP4 covr atom embedded successfully, but the sidecar could not be deleted: {e}"
            ),
        };
    }

    EmbedOutcome::Embedded {
        sidecar_filename,
        bytes_embedded: bytes_len,
    }
}

/// Look for a sibling cover sidecar (`.jpg`, `.jpeg`, or `.png`) with
/// the same stem as `video_path`. Returns the first candidate that
/// exists on disk, checked in the order GAMDL writes them (jpg is
/// the default `save_cover` extension; png is the
/// `cover_format=Png` option's output).
///
/// # Bug fix (#980)
///
/// `Path::with_extension` replaces everything **after the last
/// dot** in the file name, not the trailing extension GAMDL wrote.
/// For a stem containing a dot — `Mr. Brightside`, any `feat.` /
/// `Vol. 2` / `Pt. 1` title — `parent.join(stem).with_extension(ext)`
/// mangles the candidate: `Mr. Brightside` + `.with_extension("jpg")`
/// produces `Mr.jpg` instead of `Mr. Brightside.jpg`. Building the
/// candidate via string concatenation (`format!("{stem_str}.{ext}")`)
/// preserves the full stem, dots included.
fn find_sidecar(video_path: &Path) -> Option<PathBuf> {
    // Candidate sidecar extensions, in the order GAMDL writes them.
    const EXTENSIONS: [&str; 3] = ["jpg", "jpeg", "png"];

    let stem_str = video_path.file_stem().and_then(|s| s.to_str())?;
    let parent = video_path.parent()?;

    for ext in EXTENSIONS {
        let candidate = parent.join(format!("{stem_str}.{ext}"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

/// Guess the image format from the file extension first, falling
/// back to the magic-byte prefix. mp4ameta only supports
/// `ImgFmt::Jpeg` and `ImgFmt::Png`; if neither matches we default
/// to Jpeg because GAMDL's default output is `.jpg` and that's
/// what 99% of sidecars are.
fn detect_image_fmt(path: &Path, bytes: &[u8]) -> ImgFmt {
    // PNG magic: `89 50 4E 47`
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return ImgFmt::Png;
    }
    // JPEG magic: `FF D8 FF`
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return ImgFmt::Jpeg;
    }
    // Fall back to extension.
    match path.extension().and_then(|e| e.to_str()) {
        Some(e) if e.eq_ignore_ascii_case("png") => ImgFmt::Png,
        _ => ImgFmt::Jpeg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// `embed_and_remove_sidecar` returns `NoSidecar` when there's
    /// no matching sidecar file — the most common no-op path.
    #[test]
    fn no_sidecar_returns_no_sidecar() {
        let tmp = TempDir::new().unwrap();
        // Create only the MP4 with no sibling thumbnail.
        let video = tmp.path().join("song.mp4");
        fs::write(&video, [0; 16]).unwrap();
        let outcome = embed_and_remove_sidecar(&video);
        assert_eq!(outcome, EmbedOutcome::NoSidecar);
    }

    /// `detect_image_fmt` prefers magic bytes over extension. A
    /// `.jpg` file with PNG magic should be detected as PNG so the
    /// embedded atom matches the actual encoding.
    #[test]
    fn detect_image_fmt_prefers_magic_bytes() {
        let path = Path::new("fake.jpg");
        let png_bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert!(matches!(detect_image_fmt(path, &png_bytes), ImgFmt::Png));

        let path = Path::new("fake.png");
        let jpeg_bytes = [0xFF, 0xD8, 0xFF, 0xE0];
        assert!(matches!(detect_image_fmt(path, &jpeg_bytes), ImgFmt::Jpeg));
    }

    /// `detect_image_fmt` falls back to extension when magic bytes
    /// are absent — the empty / unknown case.
    #[test]
    fn detect_image_fmt_extension_fallback() {
        assert!(matches!(detect_image_fmt(Path::new("x.png"), &[]), ImgFmt::Png));
        assert!(matches!(detect_image_fmt(Path::new("x.jpg"), &[]), ImgFmt::Jpeg));
        assert!(matches!(detect_image_fmt(Path::new("x.bin"), &[]), ImgFmt::Jpeg));
    }

    /// On a video that mp4ameta can't read (e.g. an empty file
    /// masquerading as MP4) the embed must fail rather than panic
    /// and must NOT delete the sidecar — losing the only cover
    /// copy on a write failure would be a data-loss bug.
    #[test]
    fn failed_embed_does_not_delete_sidecar() {
        let tmp = TempDir::new().unwrap();
        let video = tmp.path().join("broken.mp4");
        let sidecar = tmp.path().join("broken.jpg");
        // Both files are dummies — mp4ameta will reject `broken.mp4`
        // because it's not a real MP4 container.
        fs::write(&video, b"not an mp4").unwrap();
        fs::write(&sidecar, [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]).unwrap();

        let outcome = embed_and_remove_sidecar(&video);
        match outcome {
            EmbedOutcome::Failed { sidecar_filename, .. } => {
                assert_eq!(sidecar_filename, "broken.jpg");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        // Sidecar must still exist — Failed means we kept the file.
        assert!(sidecar.is_file(), "sidecar must NOT be deleted on a failed embed");
    }

    /// #980 regression test: a dotted video stem (`Mr. Brightside`)
    /// must resolve to the correctly-named sidecar (`Mr.
    /// Brightside.jpg`), not the truncated `Mr.jpg` that
    /// `Path::with_extension` would produce by replacing everything
    /// after the last dot in the stem.
    #[test]
    fn find_sidecar_handles_dotted_stem() {
        let tmp = TempDir::new().unwrap();
        let video = tmp.path().join("Mr. Brightside.mp4");
        let correct_sidecar = tmp.path().join("Mr. Brightside.jpg");
        // Decoy: what the buggy `with_extension` call used to produce.
        let decoy_sidecar = tmp.path().join("Mr.jpg");

        fs::write(&video, [0; 16]).unwrap();
        fs::write(&correct_sidecar, [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
        fs::write(&decoy_sidecar, [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();

        let found = find_sidecar(&video);
        assert_eq!(found, Some(correct_sidecar), "must select the full-stem sidecar, not the dot-truncated decoy");
    }

    /// Without a dotted stem, `find_sidecar` still finds the plain
    /// sidecar — sanity check that the string-concat rewrite didn't
    /// break the common case.
    #[test]
    fn find_sidecar_handles_plain_stem() {
        let tmp = TempDir::new().unwrap();
        let video = tmp.path().join("song.mp4");
        let sidecar = tmp.path().join("song.jpg");
        fs::write(&video, [0; 16]).unwrap();
        fs::write(&sidecar, [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();

        assert_eq!(find_sidecar(&video), Some(sidecar));
    }

    /// No matching sidecar at all → `None`.
    #[test]
    fn find_sidecar_returns_none_when_absent() {
        let tmp = TempDir::new().unwrap();
        let video = tmp.path().join("song.mp4");
        fs::write(&video, [0; 16]).unwrap();
        assert_eq!(find_sidecar(&video), None);
    }
}
