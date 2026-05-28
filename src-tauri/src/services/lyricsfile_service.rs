// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Lyricsfile (.lyrics) sidecar generation service.
// =================================================
//
// Generates Lyricsfile YAML sidecars from TTML lyric documents during
// enrichment Step 2g. Lyricsfile is the open, extensible lyrics format
// jointly endorsed by LRCGET v2.0.0 and LRCLIB — see
// MeedyaSuite-core#34 + MWBMPartners/MeedyaDL#596 for full context.
//
// ## Source priority
//
// Step 2g consumes the TTML sidecar that GAMDL emits during download
// (or that the syllable-lyrics API fallback in Step 1b upgrades to
// word-level timing). LRC sources are deliberately NOT used here —
// they lack word-level fidelity, and exposing the user to a v1.0
// Lyricsfile based on coarser timing data would be a regression
// compared to keeping their existing `.lrc` sidecar.
//
// ## Output layout
//
// Sidecar placement matches every other enrichment-generated lyric
// file: same directory as the audio file, same filename stem,
// `.lyrics` extension. `01 Track.m4a` → `01 Track.lyrics`.
//
// ## Idempotency
//
// If a `.lyrics` file already exists for a track, the generation is
// skipped. This preserves user edits — if the user opened the
// `.lyrics` in LRCGET, tweaked timing, and re-downloaded the album
// for a quality upgrade, the manual edits survive.
//
// ## What the upstream shared crate does
//
// `meedya_lyrics::Lyricsfile::from_ttml(ttml, title, artist)` parses
// the TTML, extracts line-level + word-level timing, then
// `to_yaml()` serialises to the canonical YAML format LRCGET reads.
// Everything format-related (struct shape, YAML emission rules,
// schema versioning, forward-compat policy) lives upstream — this
// service is just the filesystem orchestration layer.

use std::path::Path;

use meedya_lyrics::Lyricsfile;

// ============================================================
// Public API
// ============================================================

/// Generate Lyricsfile (`.lyrics`) sidecars for every media file in
/// `album_dir` that has a companion `.ttml` sidecar and doesn't yet
/// have a `.lyrics`. Returns the number of sidecars written.
///
/// `title` and `artist` are passed through to the shared converter as
/// the document's metadata header. They come from the Apple Music
/// API metadata fetched during enrichment (NOT from any embedded
/// TTML `<metadata>` block — Apple's is unreliable and often empty).
///
/// # Arguments
/// * `album_dir` - Path to the album directory containing media + TTML files
/// * `default_title` - Fallback track title when the audio filename can't
///   resolve one (rare; mostly defensive)
/// * `default_artist` - Fallback artist (typically the album artist)
///
/// # Returns
/// * `Ok(count)` - Number of `.lyrics` sidecars written
/// * `Err(message)` - Filesystem or shared-crate error
pub fn generate_lyricsfile_for_directory(
    album_dir: &str,
    default_title: &str,
    default_artist: &str,
) -> Result<usize, String> {
    let dir = Path::new(album_dir);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {album_dir}"));
    }

    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {e}"))?;

    let mut generated = 0usize;

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip macOS AppleDouble shadows + filesystem cruft before
        // the extension check (same defence as the WebVTT/ASS services).
        if crate::utils::fs_safe::is_filesystem_sidecar(&path) {
            continue;
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "m4a" && ext != "m4v" && ext != "mp4" {
            continue;
        }

        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        let lyricsfile_path = dir.join(format!("{stem}.lyrics"));
        if lyricsfile_path.exists() {
            // Idempotency: don't clobber existing files (may carry
            // user edits from LRCGET).
            continue;
        }

        let ttml_path = dir.join(format!("{stem}.ttml"));
        let ttml = match std::fs::read_to_string(&ttml_path) {
            Ok(content) => content,
            Err(_) => {
                // No TTML for this track — silently skip. Tracks
                // without TTML (e.g., instrumental tracks that
                // GAMDL didn't get lyrics for) don't get a
                // Lyricsfile either. A separate "mark instrumental
                // from manifest" pass could decide otherwise; out
                // of scope for v1 of #596.
                continue;
            }
        };

        // Title comes from the filename stem when we can derive it,
        // since the audio file's stem is what GAMDL applied the user's
        // template against. Falls back to the caller-supplied default
        // (album track-1 title is a sensible last resort).
        let title = derive_title_from_stem(&stem).unwrap_or_else(|| default_title.to_string());

        let lyricsfile = match Lyricsfile::from_ttml(&ttml, title, default_artist) {
            Ok(lf) => lf,
            Err(e) => {
                log::debug!(
                    "Lyricsfile: TTML conversion failed for {stem}: {e} — skipping"
                );
                continue;
            }
        };

        let yaml = match lyricsfile.to_yaml() {
            Ok(s) => s,
            Err(e) => {
                log::debug!(
                    "Lyricsfile: YAML serialise failed for {stem}: {e} — skipping"
                );
                continue;
            }
        };

        if let Err(e) = std::fs::write(&lyricsfile_path, &yaml) {
            log::debug!("Failed to write Lyricsfile for {stem}: {e}");
            continue;
        }

        log::debug!("Generated Lyricsfile: {stem}.lyrics");
        generated += 1;
    }

    Ok(generated)
}

// ============================================================
// Internal helpers
// ============================================================

/// Best-effort track title extraction from a GAMDL-style filename
/// stem like `"01 Track Title"` or `"1 - 05 Track Title [Lossless]"`.
///
/// Strips leading disc/track numbers and trailing codec suffixes in
/// square brackets (e.g., `[Lossless]`, `[Dolby Atmos]`). Returns
/// `None` if the result is empty (caller falls back to its default).
fn derive_title_from_stem(stem: &str) -> Option<String> {
    let mut t = stem.trim().to_string();

    // Strip codec suffix like `" [Lossless]"`, `" [Dolby Atmos]"`,
    // `" [Explicit]"`. Loop in case multiple suffixes are present
    // (advisory + codec, see `apply_codec_suffix` in download_queue).
    loop {
        let prev_len = t.len();
        if let Some(open) = t.rfind(" [") {
            if t.ends_with(']') {
                t.truncate(open);
            }
        }
        if t.len() == prev_len {
            break;
        }
    }

    // Strip leading `"1 - 05 "` or `"1-05 "` or `"05 "` (track number)
    // GAMDL's default `track_filename_template` produces these shapes.
    let mut chars = t.chars().peekable();
    let mut prefix_len = 0;
    let mut digits_seen = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            digits_seen = true;
            prefix_len += c.len_utf8();
            chars.next();
        } else if (c == ' ' || c == '-' || c == '.') && digits_seen {
            prefix_len += c.len_utf8();
            chars.next();
        } else {
            break;
        }
    }
    if digits_seen && prefix_len < t.len() {
        t = t[prefix_len..].trim().to_string();
    }

    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_title_from_simple_track_number() {
        assert_eq!(
            derive_title_from_stem("01 Hello").as_deref(),
            Some("Hello")
        );
    }

    #[test]
    fn derives_title_from_disc_and_track_number() {
        assert_eq!(
            derive_title_from_stem("1 - 05 In My Dreams").as_deref(),
            Some("In My Dreams")
        );
    }

    #[test]
    fn strips_codec_suffix() {
        assert_eq!(
            derive_title_from_stem("01 Hello [Lossless]").as_deref(),
            Some("Hello")
        );
    }

    #[test]
    fn strips_multiple_bracketed_suffixes() {
        assert_eq!(
            derive_title_from_stem("01 Hello [Explicit] [Lossless]").as_deref(),
            Some("Hello")
        );
    }

    #[test]
    fn returns_none_for_empty_stem() {
        assert!(derive_title_from_stem("").is_none());
        assert!(derive_title_from_stem("   ").is_none());
    }

    #[test]
    fn preserves_titles_with_numbers_in_middle() {
        // "Stronger than 4 You" — the "4" is part of the title, not a
        // track number. Track-number prefix only strips leading
        // contiguous digit+separator runs.
        assert_eq!(
            derive_title_from_stem("01 Stronger than 4 You").as_deref(),
            Some("Stronger than 4 You")
        );
    }

    #[test]
    fn non_directory_path_returns_err() {
        let err = generate_lyricsfile_for_directory("/nonexistent/path", "t", "a")
            .unwrap_err();
        assert!(err.contains("Not a directory"));
    }
}
