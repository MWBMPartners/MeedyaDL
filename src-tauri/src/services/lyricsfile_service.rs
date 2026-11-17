// Copyright (c) 2024-2026 MeedyaSuite
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

use meedya_lyrics::Lyricsfile;

// ============================================================
// Internal helpers
// ============================================================

/// Builds the text of one Lyricsfile (`.lyrics`) from a TTML document.
///
/// The download's per-file lyrics step (`download_queue::lyrics_steps`) and
/// the extra-format copies (`companion_lyrics.rs`) both use this, so both
/// produce byte-identical output for the same TTML and file name.
///
/// The title comes from the file name stem when one can be derived
/// ([`derive_title_from_stem`] drops the track number and any trailing
/// `[...]` labels, so a copy's name gives the same title as the main
/// download's); otherwise `default_title` is used. `default_artist` is the
/// artist written into the file.
///
/// The `.lyrics` file's language comes entirely from the shared
/// `meedya-lyrics` code: `from_ttml` copies the TTML's `xml:lang` into it
/// as written. MeedyaDL deliberately does not rewrite it here (#1250): the
/// fix to write the policy's standard tag belongs in MeedyaSuite-core.
///
/// Returns the file text, or a short reason when the TTML cannot be
/// converted or written out.
pub(crate) fn lyricsfile_from_ttml(
    ttml: &str,
    stem: &str,
    default_title: &str,
    default_artist: &str,
) -> Result<String, String> {
    let title = derive_title_from_stem(stem).unwrap_or_else(|| default_title.to_string());
    let lyricsfile = Lyricsfile::from_ttml(ttml, title, default_artist)
        .map_err(|e| format!("TTML conversion failed: {e}"))?;
    lyricsfile
        .to_yaml()
        .map_err(|e| format!("YAML serialise failed: {e}"))
}

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
        assert_eq!(derive_title_from_stem("01 Hello").as_deref(), Some("Hello"));
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

    /// The per-track helper (shared with the download's per-file step and
    /// the extra-format copies) writes a fixed text for a given TTML, and a
    /// copy with a different label gets the same title. (Before #1443 this
    /// was compared with a whole-folder function; its exact output is now
    /// written down instead.)
    #[test]
    fn the_shared_helper_gives_the_pinned_output() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
            <tt xmlns="http://www.w3.org/ns/ttml" xml:lang="en-US">
              <body><div>
                <p begin="00:12.450" end="00:15.800">Hello world today</p>
              </div></body>
            </tt>"#;
        let from_helper =
            lyricsfile_from_ttml(ttml, "01 Hello [Lossless]", "Untitled", "Some Artist").unwrap();
        assert_eq!(
            from_helper,
            "version: '1.0'\nmetadata:\n  title: Hello\n  artist: Some Artist\n  language: en-US\n  instrumental: false\nlines:\n- text: Hello world today\n  start_ms: 12450\n  end_ms: 15800\n"
        );
        // A copy with a different label gives the same title.
        let other = lyricsfile_from_ttml(ttml, "01 Hello [Dolby Atmos]", "Untitled", "Some Artist")
            .unwrap();
        assert_eq!(from_helper, other);
    }
}
