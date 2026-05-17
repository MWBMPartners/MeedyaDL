// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// End-to-end integration tests for the enrichment pipeline services.
// ===================================================================
//
// Tests the subtitle/lyrics services with actual files on disk to verify
// the full pipeline: read source → convert → write output. Each test
// creates a temporary directory with sample files, calls the public
// directory-level API, and verifies the output files exist and contain
// the expected content.
//
// These tests exercise the file I/O paths that the unit tests in each
// service module do not cover (unit tests only test pure conversion
// functions with in-memory strings). The integration tests ensure that
// the directory scanning, file extension matching, source priority, and
// output writing all work together correctly.
//
// All tests are self-contained: no external files, no network calls,
// no AppHandle. They use `tempfile::tempdir()` for isolated directories
// that are automatically cleaned up when the test completes.
//
// Services under test:
//   - `rich_srt_service`       — TTML → Rich SRT with styling tags
//   - `webvtt_service`         — TTML/SRT/LRC → WebVTT conversion
//   - `enhanced_lyrics_service` — TTML → Enhanced LRC with word timestamps
//   - `ass_subtitle_service`   — TTML/WebVTT → ASS with override tags
//   - Unicode filename handling across all services

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::tempdir;

    use crate::services::ass_subtitle_service;
    use crate::services::enhanced_lyrics_service;
    use crate::services::rich_srt_service;
    use crate::services::webvtt_service;

    // ============================================================
    // Sample TTML content constants
    // ============================================================

    /// Minimal valid TTML with two cues and no styling.
    /// Used to verify basic parsing and SRT/VTT output format.
    const TTML_BASIC: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:00:05.200" end="00:00:08.100">Hello world</p>
      <p begin="00:00:08.100" end="00:00:12.500">Second line here</p>
    </div>
  </body>
</tt>"#;

    /// TTML with rich styling: bold, italic, underline, colour, named styles.
    /// Used to verify that Rich SRT and ASS preserve styling information.
    const TTML_STYLED: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml"
    xmlns:tts="http://www.w3.org/ns/ttml#styling"
    xmlns:ttm="http://www.w3.org/ns/ttml#metadata">
  <head>
    <styling>
      <style xml:id="chorus" tts:fontWeight="bold" tts:color="#FFD700"/>
    </styling>
  </head>
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:fontWeight="bold">Bold line</p>
      <p begin="00:00:03.000" end="00:00:06.000" tts:fontStyle="italic">Italic line</p>
      <p begin="00:00:06.000" end="00:00:09.000" style="chorus">Golden chorus</p>
      <p begin="00:00:09.000" end="00:00:12.000">Normal with <span tts:fontWeight="bold">bold</span> word</p>
      <p begin="00:00:12.000" end="00:00:15.000">Main vocal <span ttm:role="x-bg">ooh</span></p>
    </div>
  </body>
</tt>"##;

    /// TTML with word-level timing for Enhanced LRC generation.
    /// Uses the Apple Music `itunes:timing="Word"` attribute.
    const TTML_WORD_TIMING: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml"
    xmlns:ttm="http://www.w3.org/ns/ttml#metadata"
    xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
    itunes:timing="Word" xml:lang="en-US">
  <head>
    <metadata>
      <ttm:title>Integration Test Song</ttm:title>
      <ttm:agent xml:id="v1" type="person">
        <ttm:name type="full">Test Artist</ttm:name>
      </ttm:agent>
    </metadata>
  </head>
  <body>
    <div>
      <p begin="00:12.450" end="00:15.800">
        <span begin="00:12.450" end="00:13.200">Hello </span>
        <span begin="00:13.200" end="00:14.100">world </span>
        <span begin="00:14.100" end="00:15.800">today</span>
      </p>
      <p begin="00:16.000" end="00:20.000">
        <span begin="00:16.000" end="00:17.500">Another </span>
        <span begin="00:17.500" end="00:19.000">line </span>
        <span ttm:role="x-bg" begin="00:19.000" end="00:20.000">yeah</span>
      </p>
    </div>
  </body>
</tt>"#;

    /// Minimal SRT content for SRT → WebVTT conversion tests.
    const SRT_BASIC: &str = "1\n00:00:05,200 --> 00:00:08,100\nHello from SRT\n\n\
                             2\n00:00:08,100 --> 00:00:12,500\nSecond SRT line\n\n";

    /// Minimal LRC content for LRC → WebVTT conversion tests.
    const LRC_BASIC: &str = "[00:05.20]Hello from LRC\n\
                              [00:08.10]Second LRC line\n\
                              [00:12.50]Third LRC line\n";

    /// WebVTT content for WebVTT → ASS conversion tests.
    const WEBVTT_STYLED: &str = "WEBVTT\n\n\
                                  00:00:01.000 --> 00:00:03.000\n\
                                  <b>Bold</b> and <i>italic</i>\n\n\
                                  00:00:04.000 --> 00:00:06.000\n\
                                  Plain text line\n";

    // ============================================================
    // Helper: create a dummy .m4a file (empty, but with correct extension)
    // ============================================================

    /// Write a zero-byte .m4a placeholder in the temp directory.
    ///
    /// The directory-level functions scan for media files (.m4a, .m4v, .mp4)
    /// to determine which tracks need subtitle generation. The actual media
    /// content is never read by the subtitle services — they only use the
    /// file stem to find matching source files (e.g., "01 Song.ttml" for
    /// "01 Song.m4a").
    fn create_dummy_media(dir: &std::path::Path, stem: &str) {
        let media_path = dir.join(format!("{stem}.m4a"));
        fs::write(&media_path, b"").expect("Failed to create dummy media file");
    }

    // ============================================================
    // Rich SRT Service — Integration Tests
    // ============================================================

    #[test]
    /// Test that generate_rich_srt_for_directory() converts a TTML file to
    /// a .srt file with correct SRT formatting (sequential indices, comma-
    /// separated timestamps, newline-separated blocks).
    fn rich_srt_from_ttml_basic() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        // Create a dummy media file and its matching TTML source
        create_dummy_media(dir, "01 Test Song");
        fs::write(dir.join("01 Test Song.ttml"), TTML_BASIC)
            .expect("Failed to write TTML");

        // Run the directory-level conversion
        let count = rich_srt_service::generate_rich_srt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_rich_srt_for_directory failed");

        assert_eq!(count, 1, "Expected 1 rich SRT file generated");

        // Verify the .srt output file exists
        let srt_path = dir.join("01 Test Song.srt");
        assert!(srt_path.exists(), "SRT file was not created");

        // Verify SRT content
        let srt_content = fs::read_to_string(&srt_path).expect("Failed to read SRT");
        assert!(srt_content.contains("1\n"), "Missing cue index 1");
        assert!(srt_content.contains("2\n"), "Missing cue index 2");
        assert!(
            srt_content.contains("00:00:05,200 --> 00:00:08,100"),
            "Missing first timestamp"
        );
        assert!(srt_content.contains("Hello world"), "Missing first cue text");
        assert!(srt_content.contains("Second line here"), "Missing second cue text");
    }

    #[test]
    /// Test that Rich SRT preserves styling tags (bold, italic, colour)
    /// from TTML source files with tts:* attributes and named styles.
    fn rich_srt_from_ttml_with_styling() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "02 Styled Song");
        fs::write(dir.join("02 Styled Song.ttml"), TTML_STYLED)
            .expect("Failed to write TTML");

        let count = rich_srt_service::generate_rich_srt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_rich_srt_for_directory failed");

        assert_eq!(count, 1);

        let srt_content = fs::read_to_string(dir.join("02 Styled Song.srt"))
            .expect("Failed to read SRT");

        // Bold line should have <b> tags
        assert!(srt_content.contains("<b>Bold line</b>"), "Missing bold styling");

        // Italic line should have <i> tags
        assert!(srt_content.contains("<i>Italic line</i>"), "Missing italic styling");

        // Named style (chorus) should have <font color> and <b>
        assert!(
            srt_content.contains("<font color=\"#FFD700\"><b>Golden chorus</b></font>"),
            "Missing named style (chorus) styling"
        );

        // Inline span with bold
        assert!(
            srt_content.contains("Normal with <b>bold</b> word"),
            "Missing inline span bold styling"
        );

        // Background vocals should be parenthesized
        assert!(
            srt_content.contains("Main vocal (ooh)"),
            "Missing background vocal parentheses"
        );
    }

    #[test]
    /// Test that generate_rich_srt_for_directory() handles multiple tracks
    /// in one directory, converting each TTML to its own .srt file.
    fn rich_srt_multiple_tracks() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        // Create two tracks with TTML sources
        create_dummy_media(dir, "01 Track One");
        create_dummy_media(dir, "02 Track Two");
        fs::write(dir.join("01 Track One.ttml"), TTML_BASIC)
            .expect("Failed to write TTML 1");
        fs::write(dir.join("02 Track Two.ttml"), TTML_STYLED)
            .expect("Failed to write TTML 2");

        let count = rich_srt_service::generate_rich_srt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_rich_srt_for_directory failed");

        assert_eq!(count, 2, "Expected 2 rich SRT files generated");
        assert!(dir.join("01 Track One.srt").exists(), "Missing SRT for track 1");
        assert!(dir.join("02 Track Two.srt").exists(), "Missing SRT for track 2");
    }

    #[test]
    /// Test that tracks without a TTML or VTT source are skipped — existing
    /// plain .srt files are not overwritten and no error is returned.
    fn rich_srt_skips_tracks_without_source() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 No Source");
        // No .ttml or .vtt file — should be skipped

        let count = rich_srt_service::generate_rich_srt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_rich_srt_for_directory failed");

        assert_eq!(count, 0, "Expected 0 files generated when no source exists");
    }

    #[test]
    /// Test that an invalid directory path returns an error, not a panic.
    fn rich_srt_invalid_directory() {
        let result = rich_srt_service::generate_rich_srt_for_directory(
            "/nonexistent/path/that/does/not/exist",
        );
        assert!(result.is_err(), "Expected error for invalid directory");
    }

    // ============================================================
    // WebVTT Service — Integration Tests
    // ============================================================

    #[test]
    /// Test that generate_webvtt_for_directory() converts a TTML file to
    /// a .vtt file with the WEBVTT header and correct timestamp format.
    fn webvtt_from_ttml() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 Test Song");
        fs::write(dir.join("01 Test Song.ttml"), TTML_BASIC)
            .expect("Failed to write TTML");

        let count = webvtt_service::generate_webvtt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_webvtt_for_directory failed");

        assert_eq!(count, 1, "Expected 1 VTT file generated");

        let vtt_path = dir.join("01 Test Song.vtt");
        assert!(vtt_path.exists(), "VTT file was not created");

        let vtt_content = fs::read_to_string(&vtt_path).expect("Failed to read VTT");
        assert!(vtt_content.starts_with("WEBVTT"), "Missing WEBVTT header");
        // WebVTT uses dot separator (not comma like SRT)
        assert!(
            vtt_content.contains("00:00:05.200 --> 00:00:08.100"),
            "Missing TTML-derived timestamp with dot separator"
        );
        assert!(vtt_content.contains("Hello world"), "Missing first cue text");
        assert!(vtt_content.contains("Second line here"), "Missing second cue text");
    }

    #[test]
    /// Test that generate_webvtt_for_directory() converts an SRT source to
    /// WebVTT when no TTML is available (SRT is second priority).
    fn webvtt_from_srt() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 Song");
        // No .ttml — provide only .srt
        fs::write(dir.join("01 Song.srt"), SRT_BASIC).expect("Failed to write SRT");

        let count = webvtt_service::generate_webvtt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_webvtt_for_directory failed");

        assert_eq!(count, 1, "Expected 1 VTT file generated from SRT source");

        let vtt_content = fs::read_to_string(dir.join("01 Song.vtt"))
            .expect("Failed to read VTT");
        assert!(vtt_content.starts_with("WEBVTT"), "Missing WEBVTT header");
        // Comma from SRT should be converted to dot in WebVTT
        assert!(
            vtt_content.contains("00:00:05.200 --> 00:00:08.100"),
            "SRT comma timestamps not converted to dot"
        );
        assert!(vtt_content.contains("Hello from SRT"), "Missing SRT text");
    }

    #[test]
    /// Test that generate_webvtt_for_directory() converts an LRC source to
    /// WebVTT when no TTML or SRT is available (LRC is third priority).
    fn webvtt_from_lrc() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 Song");
        // No .ttml, no .srt — provide only .lrc
        fs::write(dir.join("01 Song.lrc"), LRC_BASIC).expect("Failed to write LRC");

        let count = webvtt_service::generate_webvtt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_webvtt_for_directory failed");

        assert_eq!(count, 1, "Expected 1 VTT file generated from LRC source");

        let vtt_content = fs::read_to_string(dir.join("01 Song.vtt"))
            .expect("Failed to read VTT");
        assert!(vtt_content.starts_with("WEBVTT"), "Missing WEBVTT header");
        assert!(vtt_content.contains("Hello from LRC"), "Missing LRC text");
        assert!(vtt_content.contains("Second LRC line"), "Missing second LRC line");
    }

    #[test]
    /// Test source priority: when TTML exists alongside SRT and LRC,
    /// the TTML source should be preferred for WebVTT generation.
    fn webvtt_source_priority_ttml_over_srt() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 Song");
        // Provide all three sources — TTML should win
        fs::write(dir.join("01 Song.ttml"), TTML_BASIC).expect("Failed to write TTML");
        fs::write(dir.join("01 Song.srt"), SRT_BASIC).expect("Failed to write SRT");
        fs::write(dir.join("01 Song.lrc"), LRC_BASIC).expect("Failed to write LRC");

        let count = webvtt_service::generate_webvtt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_webvtt_for_directory failed");

        assert_eq!(count, 1);

        let vtt_content = fs::read_to_string(dir.join("01 Song.vtt"))
            .expect("Failed to read VTT");
        // TTML has "Hello world", SRT has "Hello from SRT", LRC has "Hello from LRC"
        assert!(
            vtt_content.contains("Hello world"),
            "WebVTT should use TTML source (has 'Hello world'), not SRT/LRC"
        );
    }

    #[test]
    /// Test that existing .vtt files are not overwritten — the service
    /// skips tracks that already have a WebVTT sidecar.
    fn webvtt_skips_existing_vtt() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 Song");
        fs::write(dir.join("01 Song.ttml"), TTML_BASIC).expect("Failed to write TTML");
        // Pre-create a .vtt file that should NOT be overwritten
        let existing_content = "WEBVTT\n\nExisting content, do not overwrite\n";
        fs::write(dir.join("01 Song.vtt"), existing_content)
            .expect("Failed to write existing VTT");

        let count = webvtt_service::generate_webvtt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_webvtt_for_directory failed");

        assert_eq!(count, 0, "Expected 0 — existing VTT should be skipped");

        // Verify original content is preserved
        let vtt_content = fs::read_to_string(dir.join("01 Song.vtt"))
            .expect("Failed to read VTT");
        assert!(
            vtt_content.contains("Existing content, do not overwrite"),
            "Existing VTT was overwritten"
        );
    }

    #[test]
    /// Test WebVTT generation for an invalid directory.
    fn webvtt_invalid_directory() {
        let result = webvtt_service::generate_webvtt_for_directory(
            "/nonexistent/path/that/does/not/exist",
        );
        assert!(result.is_err(), "Expected error for invalid directory");
    }

    // ============================================================
    // Enhanced LRC Service — Integration Tests
    // ============================================================

    #[test]
    /// Test that process_enhanced_lyrics_for_directory() converts a TTML
    /// file with word-level timing to an Enhanced LRC sidecar file.
    fn enhanced_lrc_word_level_timing() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        // Write the TTML with word-level timing — no media file needed
        // for the LRC sidecar write (only needed for embedding, which
        // we skip by not providing a valid .m4a)
        fs::write(dir.join("01 Test Song.ttml"), TTML_WORD_TIMING)
            .expect("Failed to write TTML");

        let count = enhanced_lyrics_service::process_enhanced_lyrics_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("process_enhanced_lyrics_for_directory failed");

        assert_eq!(count, 1, "Expected 1 Enhanced LRC file generated");

        // Verify the .lrc sidecar file exists
        let lrc_path = dir.join("01 Test Song.lrc");
        assert!(lrc_path.exists(), "LRC file was not created");

        let lrc_content = fs::read_to_string(&lrc_path).expect("Failed to read LRC");

        // Verify LRC metadata header
        assert!(
            lrc_content.contains("[ti:Integration Test Song]"),
            "Missing title in LRC header"
        );
        assert!(
            lrc_content.contains("[ar:Test Artist]"),
            "Missing artist in LRC header"
        );
        assert!(
            lrc_content.contains("[la:en-US]"),
            "Missing language in LRC header"
        );
        assert!(
            lrc_content.contains("[by:MeedyaDL]"),
            "Missing tool attribution in LRC header"
        );
        assert!(
            lrc_content.contains("[re:MeedyaDL Enhanced LRC]"),
            "Missing format tag in LRC header"
        );

        // Verify Enhanced LRC word-level timestamps
        // Line 1: [00:12.45]<00:12.45>Hello <00:13.20>world <00:14.10>today
        assert!(
            lrc_content.contains("[00:12.45]<00:12.45>Hello <00:13.20>world <00:14.10>today"),
            "Missing Enhanced LRC word timestamps for line 1"
        );

        // Line 2: background vocals wrapped in parentheses
        assert!(
            lrc_content.contains("<00:19.00>(yeah)"),
            "Missing background vocals in parentheses"
        );
    }

    #[test]
    /// Test that a TTML without word-level timing (line-level only) produces
    /// standard LRC with line timestamps but no inline word timestamps.
    fn enhanced_lrc_line_level_fallback() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        let ttml_line_level = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml"
    xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
    itunes:timing="Line">
  <body>
    <div>
      <p begin="00:12.450" end="00:15.800">Hello world today</p>
      <p begin="00:16.000" end="00:20.000">Another line</p>
    </div>
  </body>
</tt>"#;

        fs::write(dir.join("01 Line Level.ttml"), ttml_line_level)
            .expect("Failed to write TTML");

        let count = enhanced_lyrics_service::process_enhanced_lyrics_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("process_enhanced_lyrics_for_directory failed");

        assert_eq!(count, 1);

        let lrc_content = fs::read_to_string(dir.join("01 Line Level.lrc"))
            .expect("Failed to read LRC");

        // Standard LRC format: [mm:ss.xx]text (no inline <> word timestamps)
        assert!(
            lrc_content.contains("[00:12.45]Hello world today"),
            "Missing line-level LRC timestamp"
        );
        assert!(
            lrc_content.contains("[00:16.00]Another line"),
            "Missing second line LRC"
        );

        // Should NOT contain Enhanced LRC inline timestamps
        assert!(
            !lrc_content.contains("<00:"),
            "Line-level LRC should not have inline word timestamps"
        );
    }

    #[test]
    /// Test that multiple TTML files in one directory are each processed
    /// into their own .lrc sidecar file.
    fn enhanced_lrc_multiple_tracks() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        fs::write(dir.join("01 Song One.ttml"), TTML_WORD_TIMING)
            .expect("Failed to write TTML 1");
        fs::write(dir.join("02 Song Two.ttml"), TTML_BASIC)
            .expect("Failed to write TTML 2");

        let count = enhanced_lyrics_service::process_enhanced_lyrics_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("process_enhanced_lyrics_for_directory failed");

        assert_eq!(count, 2, "Expected 2 LRC files generated");
        assert!(dir.join("01 Song One.lrc").exists(), "Missing LRC for track 1");
        assert!(dir.join("02 Song Two.lrc").exists(), "Missing LRC for track 2");
    }

    #[test]
    /// Test Enhanced LRC with an invalid directory path.
    fn enhanced_lrc_invalid_directory() {
        let result = enhanced_lyrics_service::process_enhanced_lyrics_for_directory(
            "/nonexistent/path/that/does/not/exist",
        );
        assert!(result.is_err(), "Expected error for invalid directory");
    }

    // ============================================================
    // ASS Subtitle Service — Integration Tests
    // ============================================================

    #[test]
    /// Test that generate_ass_for_directory() converts a TTML file to an
    /// ASS file with correct section headers and Dialogue events.
    fn ass_from_ttml_basic() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 Test Song");
        fs::write(dir.join("01 Test Song.ttml"), TTML_BASIC)
            .expect("Failed to write TTML");

        let count = ass_subtitle_service::generate_ass_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_ass_for_directory failed");

        assert_eq!(count, 1, "Expected 1 ASS file generated");

        let ass_path = dir.join("01 Test Song.ass");
        assert!(ass_path.exists(), "ASS file was not created");

        let ass_content = fs::read_to_string(&ass_path).expect("Failed to read ASS");

        // Verify mandatory ASS sections
        assert!(ass_content.contains("[Script Info]"), "Missing [Script Info]");
        assert!(ass_content.contains("[V4+ Styles]"), "Missing [V4+ Styles]");
        assert!(ass_content.contains("[Events]"), "Missing [Events]");

        // Verify Dialogue lines with ASS timestamp format (H:MM:SS.cc)
        assert!(
            ass_content.contains("Dialogue: 0,0:00:05.20,0:00:08.10,Default,,0,0,0,,Hello world"),
            "Missing Dialogue event for first cue"
        );
        assert!(
            ass_content.contains("Second line here"),
            "Missing second cue text in Dialogue"
        );
    }

    #[test]
    /// Test that ASS conversion preserves styling via override tags
    /// (bold → {\b1}...{\b0}, colour → {\c&HBBGGRR&}).
    fn ass_from_ttml_with_styling() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "02 Styled");
        fs::write(dir.join("02 Styled.ttml"), TTML_STYLED)
            .expect("Failed to write TTML");

        let count = ass_subtitle_service::generate_ass_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_ass_for_directory failed");

        assert_eq!(count, 1);

        let ass_content = fs::read_to_string(dir.join("02 Styled.ass"))
            .expect("Failed to read ASS");

        // Bold → ASS override tags
        assert!(
            ass_content.contains("{\\b1}Bold line{\\b0}"),
            "Missing bold ASS override"
        );

        // Italic → ASS override tags
        assert!(
            ass_content.contains("{\\i1}Italic line{\\i0}"),
            "Missing italic ASS override"
        );

        // Background vocals → BgVocals style and parenthesized text
        assert!(
            ass_content.contains("BgVocals"),
            "Missing BgVocals style for background vocals"
        );
        assert!(
            ass_content.contains("(ooh)"),
            "Missing parenthesized background vocals text"
        );
    }

    #[test]
    /// Test that generate_ass_for_directory() can convert a WebVTT source
    /// to ASS when no TTML is available.
    fn ass_from_webvtt() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 Song");
        // No TTML — provide only .vtt
        fs::write(dir.join("01 Song.vtt"), WEBVTT_STYLED)
            .expect("Failed to write VTT");

        let count = ass_subtitle_service::generate_ass_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_ass_for_directory failed");

        assert_eq!(count, 1, "Expected 1 ASS file generated from VTT source");

        let ass_content = fs::read_to_string(dir.join("01 Song.ass"))
            .expect("Failed to read ASS");

        assert!(ass_content.contains("[Script Info]"), "Missing [Script Info]");
        assert!(ass_content.contains("[Events]"), "Missing [Events]");

        // WebVTT bold/italic should be converted to ASS overrides
        assert!(
            ass_content.contains("{\\b1}Bold{\\b0}"),
            "Missing bold ASS override from VTT"
        );
        assert!(
            ass_content.contains("{\\i1}italic{\\i0}"),
            "Missing italic ASS override from VTT"
        );
    }

    #[test]
    /// Test that existing .ass files are not overwritten.
    fn ass_skips_existing() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 Song");
        fs::write(dir.join("01 Song.ttml"), TTML_BASIC).expect("Failed to write TTML");
        // Pre-create .ass
        let existing = "Existing ASS content\n";
        fs::write(dir.join("01 Song.ass"), existing)
            .expect("Failed to write existing ASS");

        let count = ass_subtitle_service::generate_ass_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_ass_for_directory failed");

        assert_eq!(count, 0, "Expected 0 — existing ASS should be skipped");

        let ass_content = fs::read_to_string(dir.join("01 Song.ass"))
            .expect("Failed to read ASS");
        assert_eq!(
            ass_content, existing,
            "Existing ASS file was overwritten"
        );
    }

    #[test]
    /// Test ASS generation with an invalid directory path.
    fn ass_invalid_directory() {
        let result = ass_subtitle_service::generate_ass_for_directory(
            "/nonexistent/path/that/does/not/exist",
        );
        assert!(result.is_err(), "Expected error for invalid directory");
    }

    // ============================================================
    // Unicode Filename Tests
    // ============================================================

    #[test]
    /// Test that Rich SRT generation works with Japanese filenames.
    /// Verifies the entire pipeline handles non-ASCII characters in
    /// file stems without corruption or failure.
    fn rich_srt_unicode_japanese() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        let stem = "\u{6771}\u{4EAC}\u{30BF}\u{30EF}\u{30FC}"; // 東京タワー (Tokyo Tower)
        create_dummy_media(dir, stem);
        fs::write(dir.join(format!("{stem}.ttml")), TTML_BASIC)
            .expect("Failed to write TTML with Japanese name");

        let count = rich_srt_service::generate_rich_srt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_rich_srt_for_directory failed with Japanese filename");

        assert_eq!(count, 1, "Expected 1 rich SRT file for Japanese filename");

        let srt_path = dir.join(format!("{stem}.srt"));
        assert!(srt_path.exists(), "SRT file not created for Japanese filename");

        let srt_content = fs::read_to_string(&srt_path).expect("Failed to read SRT");
        assert!(srt_content.contains("Hello world"), "Missing cue text");
    }

    #[test]
    /// Test that WebVTT generation works with Korean filenames.
    fn webvtt_unicode_korean() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        let stem = "\u{C11C}\u{C6B8}\u{C758} \u{BD04}"; // 서울의 봄 (Spring of Seoul)
        create_dummy_media(dir, stem);
        fs::write(dir.join(format!("{stem}.ttml")), TTML_BASIC)
            .expect("Failed to write TTML with Korean name");

        let count = webvtt_service::generate_webvtt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_webvtt_for_directory failed with Korean filename");

        assert_eq!(count, 1, "Expected 1 VTT file for Korean filename");

        let vtt_content = fs::read_to_string(dir.join(format!("{stem}.vtt")))
            .expect("Failed to read VTT");
        assert!(vtt_content.starts_with("WEBVTT"), "Missing WEBVTT header");
        assert!(vtt_content.contains("Hello world"), "Missing cue text");
    }

    #[test]
    /// Test that ASS generation works with emoji filenames.
    fn ass_unicode_emoji() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        let stem = "01 \u{1F3B5}\u{1F3B6} Music"; // 01 🎵🎶 Music
        create_dummy_media(dir, stem);
        fs::write(dir.join(format!("{stem}.ttml")), TTML_BASIC)
            .expect("Failed to write TTML with emoji name");

        let count = ass_subtitle_service::generate_ass_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_ass_for_directory failed with emoji filename");

        assert_eq!(count, 1, "Expected 1 ASS file for emoji filename");

        let ass_content = fs::read_to_string(dir.join(format!("{stem}.ass")))
            .expect("Failed to read ASS");
        assert!(ass_content.contains("[Script Info]"), "Missing [Script Info]");
        assert!(ass_content.contains("Hello world"), "Missing cue text");
    }

    #[test]
    /// Test that Enhanced LRC generation works with mixed CJK and Latin
    /// characters in filenames.
    fn enhanced_lrc_unicode_mixed_cjk() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        let stem = "01 \u{4E2D}\u{6587}Chinese-\u{82F1}\u{6587}English"; // 01 中文Chinese-英文English
        fs::write(dir.join(format!("{stem}.ttml")), TTML_WORD_TIMING)
            .expect("Failed to write TTML with mixed CJK name");

        let count = enhanced_lyrics_service::process_enhanced_lyrics_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("process_enhanced_lyrics_for_directory failed with mixed CJK filename");

        assert_eq!(count, 1, "Expected 1 LRC file for mixed CJK filename");

        let lrc_content = fs::read_to_string(dir.join(format!("{stem}.lrc")))
            .expect("Failed to read LRC");
        assert!(
            lrc_content.contains("[ti:Integration Test Song]"),
            "Missing title in LRC header"
        );
        assert!(
            lrc_content.contains("<00:12.45>Hello"),
            "Missing word timestamp in Enhanced LRC"
        );
    }

    // ============================================================
    // Cross-Service Integration Tests
    // ============================================================

    #[test]
    /// Test that all four subtitle services can process the same directory
    /// concurrently (simulating the enrichment pipeline running steps in
    /// sequence). Verifies no file conflicts between .srt, .vtt, .ass, .lrc.
    fn all_services_same_directory() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        // Set up a track with TTML source (used by all services)
        create_dummy_media(dir, "01 Full Pipeline");
        fs::write(dir.join("01 Full Pipeline.ttml"), TTML_STYLED)
            .expect("Failed to write TTML");

        // Step 2d: Rich SRT
        let srt_count = rich_srt_service::generate_rich_srt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("Rich SRT failed");
        assert_eq!(srt_count, 1, "Rich SRT should produce 1 file");

        // Step 2c: WebVTT (should not regenerate from the .srt we just created,
        // because TTML is still the preferred source)
        let vtt_count = webvtt_service::generate_webvtt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("WebVTT failed");
        assert_eq!(vtt_count, 1, "WebVTT should produce 1 file");

        // Step 2f: ASS (should skip since WebVTT is not the trigger — wait,
        // ASS also takes TTML as source, so it should produce 1 file)
        // Note: ASS skips if .ass already exists, but we haven't created one.
        let ass_count = ass_subtitle_service::generate_ass_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("ASS failed");
        assert_eq!(ass_count, 1, "ASS should produce 1 file");

        // Verify all output files coexist
        assert!(dir.join("01 Full Pipeline.srt").exists(), "SRT missing");
        assert!(dir.join("01 Full Pipeline.vtt").exists(), "VTT missing");
        assert!(dir.join("01 Full Pipeline.ass").exists(), "ASS missing");

        // Verify SRT has styling
        let srt = fs::read_to_string(dir.join("01 Full Pipeline.srt"))
            .expect("Failed to read SRT");
        assert!(srt.contains("<b>"), "SRT should have bold tags");

        // Verify VTT has WEBVTT header
        let vtt = fs::read_to_string(dir.join("01 Full Pipeline.vtt"))
            .expect("Failed to read VTT");
        assert!(vtt.starts_with("WEBVTT"), "VTT should start with WEBVTT header");

        // Verify ASS has ASS sections
        let ass = fs::read_to_string(dir.join("01 Full Pipeline.ass"))
            .expect("Failed to read ASS");
        assert!(ass.contains("[Events]"), "ASS should have [Events] section");
    }

    #[test]
    /// Test the full Enhanced LRC → WebVTT pipeline: Enhanced LRC is
    /// generated from TTML, then WebVTT is generated from the .lrc output.
    fn enhanced_lrc_then_webvtt_from_lrc() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 Chain Test");
        fs::write(dir.join("01 Chain Test.ttml"), TTML_WORD_TIMING)
            .expect("Failed to write TTML");

        // Step 2: Enhanced LRC from TTML
        let lrc_count = enhanced_lyrics_service::process_enhanced_lyrics_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("Enhanced LRC failed");
        assert_eq!(lrc_count, 1);
        assert!(dir.join("01 Chain Test.lrc").exists(), "LRC should exist");

        // Step 2c: WebVTT — should use TTML source (priority over LRC)
        let vtt_count = webvtt_service::generate_webvtt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("WebVTT failed");
        assert_eq!(vtt_count, 1);

        let vtt = fs::read_to_string(dir.join("01 Chain Test.vtt"))
            .expect("Failed to read VTT");
        assert!(vtt.starts_with("WEBVTT"), "VTT header missing");
        // TTML source text should appear (not Enhanced LRC inline timestamps)
        assert!(vtt.contains("Hello"), "VTT should contain lyrics text");
    }

    #[test]
    /// Test WebVTT from LRC when TTML has been removed (simulates a scenario
    /// where only the .lrc sidecar remains).
    fn webvtt_falls_back_to_lrc_when_no_ttml() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        create_dummy_media(dir, "01 Fallback");
        // Only .lrc available — no .ttml, no .srt
        fs::write(dir.join("01 Fallback.lrc"), LRC_BASIC)
            .expect("Failed to write LRC");

        let vtt_count = webvtt_service::generate_webvtt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("WebVTT failed");
        assert_eq!(vtt_count, 1);

        let vtt = fs::read_to_string(dir.join("01 Fallback.vtt"))
            .expect("Failed to read VTT");
        assert!(vtt.starts_with("WEBVTT"), "VTT header missing");
        assert!(
            vtt.contains("Hello from LRC"),
            "VTT should contain LRC-sourced text"
        );
    }

    // ============================================================
    // Edge Case Tests
    // ============================================================

    #[test]
    /// Test that an empty directory (no media files) produces zero output
    /// for all services without errors.
    fn empty_directory_all_services() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        let srt_count = rich_srt_service::generate_rich_srt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("Rich SRT should not error on empty dir");
        assert_eq!(srt_count, 0);

        let vtt_count = webvtt_service::generate_webvtt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("WebVTT should not error on empty dir");
        assert_eq!(vtt_count, 0);

        let ass_count = ass_subtitle_service::generate_ass_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("ASS should not error on empty dir");
        assert_eq!(ass_count, 0);
    }

    #[test]
    /// Test that media files with different extensions (.m4a, .m4v, .mp4)
    /// are all recognized and processed.
    fn all_media_extensions_recognized() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        // Create three media files with different extensions
        fs::write(dir.join("01 Audio.m4a"), b"").expect("Failed to create m4a");
        fs::write(dir.join("02 Video.m4v"), b"").expect("Failed to create m4v");
        fs::write(dir.join("03 Movie.mp4"), b"").expect("Failed to create mp4");

        // Create matching TTML sources
        fs::write(dir.join("01 Audio.ttml"), TTML_BASIC).expect("Failed to write TTML");
        fs::write(dir.join("02 Video.ttml"), TTML_BASIC).expect("Failed to write TTML");
        fs::write(dir.join("03 Movie.ttml"), TTML_BASIC).expect("Failed to write TTML");

        let count = rich_srt_service::generate_rich_srt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_rich_srt_for_directory failed");

        assert_eq!(count, 3, "All three media extensions should be processed");
        assert!(dir.join("01 Audio.srt").exists(), "SRT for .m4a missing");
        assert!(dir.join("02 Video.srt").exists(), "SRT for .m4v missing");
        assert!(dir.join("03 Movie.srt").exists(), "SRT for .mp4 missing");
    }

    #[test]
    /// Test that filenames with special characters (spaces, hyphens,
    /// parentheses, apostrophes) are handled correctly.
    fn special_characters_in_filenames() {
        let tmp = tempdir().expect("Failed to create temp dir");
        let dir = tmp.path();

        let stem = "01 Don't Stop (Remix) - Radio Edit [Explicit]";
        create_dummy_media(dir, stem);
        fs::write(dir.join(format!("{stem}.ttml")), TTML_BASIC)
            .expect("Failed to write TTML");

        let count = rich_srt_service::generate_rich_srt_for_directory(
            dir.to_str().unwrap(),
        )
        .expect("generate_rich_srt_for_directory failed");

        assert_eq!(count, 1, "Should handle special characters in filenames");
        assert!(
            dir.join(format!("{stem}.srt")).exists(),
            "SRT file with special characters not created"
        );
    }

    // ================================================================
    // GAMDL Smoke Tests
    // ================================================================
    //
    // These tests verify that the GAMDL download pipeline works end-to-end.
    // They require:
    //   - Python installed at the managed path
    //   - GAMDL installed via pip
    //   - Valid Apple Music cookies or wrapper URL
    //
    // They are IGNORED by default (`#[ignore]`) so `cargo test` runs without
    // them. Run them explicitly with: `cargo test -- --ignored`
    // Or in CI: `cargo test -- --ignored` with secrets configured.

    /// Smoke test: verify GAMDL is installed and can show its version.
    /// This doesn't download anything — just confirms the subprocess works.
    #[test]
    #[ignore] // Requires Python + GAMDL installed
    fn gamdl_version_smoke_test() {
        use std::process::Command;

        // Try to run `python -m gamdl --version`
        let output = Command::new("python3")
            .args(["-m", "gamdl", "--version"])
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let combined = format!("{stdout}{stderr}");
                assert!(
                    combined.contains("gamdl") || out.status.success(),
                    "GAMDL should respond to --version. Output: {combined}"
                );
                println!("GAMDL version: {}", combined.trim());
            }
            Err(e) => {
                panic!("Failed to run GAMDL: {e}. Is Python 3 installed?");
            }
        }
    }

    /// Smoke test: verify URL parsing works for a known Apple Music URL.
    #[test]
    fn url_parsing_smoke_test() {
        use crate::services::apple_music_api;

        // A well-known public album URL
        let url = "https://music.apple.com/us/album/abbey-road/401186199";
        let parsed = apple_music_api::parse_apple_music_url(url);
        assert!(parsed.is_some(), "Should parse a valid Apple Music album URL");
        let parsed = parsed.unwrap();
        assert_eq!(parsed.storefront, "us");
        assert_eq!(parsed.album_id, "401186199");
    }
}
