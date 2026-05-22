// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// WebVTT subtitle generation service.
// ====================================
//
// Generates WebVTT (`.vtt`) subtitle files from existing lyrics sidecars
// (TTML, SRT, or LRC). WebVTT is the standard format for web-based video
// players (HTML5 `<track>` element, YouTube, etc.).
//
// ## Source Priority
//
// When multiple source formats exist for the same track, TTML is preferred
// (richest timing data including word-level timestamps), followed by SRT
// (has explicit start+end times), then LRC (start times only; end times
// are estimated from the next cue).
//
// ## Integration
//
// Called from the enrichment pipeline (Step 2c) via `spawn_blocking()`.
// This function is intentionally synchronous (no `.await` calls) because
// it only does filesystem I/O — the async runtime wrapper prevents
// blocking the tokio event loop on slow FUSE mounts.
//
// ## Opt-In
//
// Controlled by the `generate_webvtt` setting (default: false). Toggle
// in Settings > Lyrics > "Generate WebVTT Subtitles".

use std::path::Path;

// ============================================================
// Public API
// ============================================================

/// Generate WebVTT subtitle files for all media files in a directory.
///
/// Scans the directory for media files (`.m4a`, `.m4v`, `.mp4`) and
/// attempts to create a `.vtt` sidecar for each one by converting the
/// best available lyrics source file.
///
/// Source priority: `.ttml` → `.srt` → `.lrc`
///
/// Skips tracks that already have a `.vtt` file.
///
/// # Arguments
/// * `album_dir` - Path to the album directory containing media and lyrics files
///
/// # Returns
/// The number of `.vtt` files successfully generated.
pub fn generate_webvtt_for_directory(album_dir: &str) -> Result<usize, String> {
    let dir = Path::new(album_dir);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {album_dir}"));
    }

    // Collect all media file stems (unique base names without extension)
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {e}"))?;

    let mut generated = 0;

    // Iterate over directory entries looking for media files
    for entry in entries.flatten() {
        let path = entry.path();

        // Skip macOS AppleDouble shadows and similar sidecars before
        // the extension check — `._Track.m4a` would otherwise pass
        // the allowlist below (#577).
        if crate::utils::fs_safe::is_filesystem_sidecar(&path) {
            continue;
        }

        // Only process media files (.m4a, .m4v, .mp4)
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext != "m4a" && ext != "m4v" && ext != "mp4" {
            continue;
        }

        // Get the file stem (e.g., "01 Song Title")
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        // Skip if .vtt already exists for this track
        let vtt_path = dir.join(format!("{stem}.vtt"));
        if vtt_path.exists() {
            continue;
        }

        // Try source formats in priority order: TTML → SRT → LRC
        let vtt_content = if let Some(content) = try_convert_ttml(dir, &stem) {
            content
        } else if let Some(content) = try_convert_srt(dir, &stem) {
            content
        } else if let Some(content) = try_convert_lrc(dir, &stem) {
            content
        } else {
            // No source lyrics found for this track
            continue;
        };

        // Write the .vtt file
        if let Err(e) = std::fs::write(&vtt_path, &vtt_content) {
            log::debug!("Failed to write VTT for {stem}: {e}");
            continue;
        }

        log::debug!("Generated WebVTT: {stem}.vtt");
        generated += 1;
    }

    Ok(generated)
}

// ============================================================
// TTML → WebVTT Conversion
// ============================================================

/// Try to convert a TTML file to WebVTT for the given track stem.
///
/// Returns `Some(vtt_content)` if a `.ttml` file exists and was
/// successfully parsed, or `None` if no TTML file exists or parsing failed.
fn try_convert_ttml(dir: &Path, stem: &str) -> Option<String> {
    let ttml_path = dir.join(format!("{stem}.ttml"));
    if !ttml_path.exists() {
        return None;
    }

    let ttml_content = std::fs::read_to_string(&ttml_path).ok()?;
    ttml_to_webvtt(&ttml_content).ok()
}

/// Convert TTML XML content to WebVTT format.
///
/// Parses the TTML document, extracts timed text from `<p>` elements,
/// and formats each as a WebVTT cue with start and end timestamps.
///
/// Handles both Apple Music TTML namespaces:
/// - `http://music.apple.com/lyric-ttml-internal`
/// - `http://itunes.apple.com/lyric-ttml-extensions`
pub fn ttml_to_webvtt(ttml_content: &str) -> Result<String, String> {
    let doc = roxmltree::Document::parse(ttml_content)
        .map_err(|e| format!("Failed to parse TTML: {e}"))?;

    let mut cues = Vec::new();

    // Find all <p> elements (each represents a lyrics line)
    for node in doc.descendants() {
        if node.tag_name().name() != "p" {
            continue;
        }

        // Extract begin and end attributes
        let begin = match node.attribute("begin") {
            Some(b) => parse_ttml_time(b),
            None => continue,
        };
        let end = match node.attribute("end") {
            Some(e) => parse_ttml_time(e),
            None => {
                // If no end time, try duration attribute
                if let Some(dur) = node.attribute("dur") {
                    begin + parse_ttml_time(dur)
                } else {
                    // Estimate: begin + 5 seconds (generous default)
                    begin + 5.0
                }
            }
        };

        // Collect text content from the <p> element and its children
        let text = collect_text_content(&node);
        if text.trim().is_empty() {
            continue;
        }

        cues.push((begin, end, text.trim().to_string()));
    }

    if cues.is_empty() {
        return Err("No timed cues found in TTML".to_string());
    }

    // Build WebVTT output
    let mut vtt = String::from("WEBVTT\n\n");
    for (begin, end, text) in &cues {
        vtt.push_str(&format!(
            "{} --> {}\n{}\n\n",
            format_webvtt_time(*begin),
            format_webvtt_time(*end),
            text
        ));
    }

    Ok(vtt)
}

/// Collect all text content from a TTML node and its children.
///
/// Concatenates text from `<span>` children (word-level timing) and
/// direct text nodes. Handles background vocals marked with
/// `ttm:role="x-bg"` by wrapping in parentheses.
fn collect_text_content(node: &roxmltree::Node) -> String {
    let mut text = String::new();
    // Check if this <p> has <span> children — if so, whitespace-only
    // text nodes between spans are just XML indentation, not lyrics.
    let has_spans = node.children().any(|c| c.tag_name().name() == "span");
    for child in node.children() {
        if child.is_text() {
            let t = child.text().unwrap_or("");
            if has_spans && t.trim().is_empty() {
                continue; // Skip indentation whitespace between spans
            }
            text.push_str(t);
        } else if child.tag_name().name() == "span" {
            // Check for background vocal marker
            let is_bg = child.attribute("role").is_some_and(|r| r.contains("x-bg"));

            let span_text: String = child
                .children()
                .filter_map(|c| c.text())
                .collect::<Vec<_>>()
                .join("");

            if is_bg && !span_text.starts_with('(') {
                text.push_str(&format!("({span_text})"));
            } else {
                text.push_str(&span_text);
            }
        }
    }
    text
}

// ============================================================
// SRT → WebVTT Conversion
// ============================================================

/// Try to convert an SRT file to WebVTT for the given track stem.
fn try_convert_srt(dir: &Path, stem: &str) -> Option<String> {
    let srt_path = dir.join(format!("{stem}.srt"));
    if !srt_path.exists() {
        return None;
    }

    let srt_content = std::fs::read_to_string(&srt_path).ok()?;
    Some(srt_to_webvtt(&srt_content))
}

/// Convert SRT content to WebVTT format.
///
/// SRT and WebVTT are nearly identical formats. The main differences:
/// - WebVTT requires a `WEBVTT` header line
/// - WebVTT uses `.` as the millisecond separator (SRT uses `,`)
/// - WebVTT supports additional styling (not used here)
pub fn srt_to_webvtt(srt_content: &str) -> String {
    let mut vtt = String::from("WEBVTT\n\n");

    for line in srt_content.lines() {
        // Replace SRT timestamp comma with WebVTT dot
        // SRT: 00:01:23,456 --> 00:01:25,789
        // VTT: 00:01:23.456 --> 00:01:25.789
        if line.contains("-->") {
            vtt.push_str(&line.replace(',', "."));
        } else {
            vtt.push_str(line);
        }
        vtt.push('\n');
    }

    vtt
}

// ============================================================
// LRC → WebVTT Conversion
// ============================================================

/// Try to convert an LRC file to WebVTT for the given track stem.
fn try_convert_lrc(dir: &Path, stem: &str) -> Option<String> {
    let lrc_path = dir.join(format!("{stem}.lrc"));
    if !lrc_path.exists() {
        return None;
    }

    let lrc_content = std::fs::read_to_string(&lrc_path).ok()?;
    let vtt = lrc_to_webvtt(&lrc_content);
    if vtt == "WEBVTT\n\n" {
        // Empty conversion — no valid cues found
        return None;
    }
    Some(vtt)
}

/// Convert LRC content to WebVTT format.
///
/// LRC format only has start times (`[mm:ss.xx]`), not end times.
/// End times are estimated as the start time of the next cue, or
/// start + 5 seconds for the last cue.
///
/// Handles both standard LRC and Enhanced LRC (strips inline
/// `<mm:ss.xx>` word timestamps for cleaner VTT output).
pub fn lrc_to_webvtt(lrc_content: &str) -> String {
    // Parse LRC lines into (time_seconds, text) pairs
    let mut cues: Vec<(f64, String)> = Vec::new();

    for line in lrc_content.lines() {
        let trimmed = line.trim();
        // Skip metadata tags like [ar:Artist] [ti:Title]
        if trimmed.is_empty() || !trimmed.starts_with('[') {
            continue;
        }

        // Parse [mm:ss.xx] timestamp
        if let Some(close_bracket) = trimmed.find(']') {
            let time_str = &trimmed[1..close_bracket];
            if let Some(time) = parse_lrc_time(time_str) {
                // Get the text after the timestamp, stripping any Enhanced LRC
                // inline word timestamps like <00:12.45>
                let mut text = trimmed[close_bracket + 1..].to_string();
                // Remove Enhanced LRC inline timestamps (<mm:ss.xx>)
                while let Some(start) = text.find('<') {
                    if let Some(end) = text[start..].find('>') {
                        text.replace_range(start..start + end + 1, "");
                    } else {
                        break;
                    }
                }
                let text = text.trim().to_string();
                if !text.is_empty() {
                    cues.push((time, text));
                }
            }
        }
    }

    if cues.is_empty() {
        return String::from("WEBVTT\n\n");
    }

    // Build WebVTT with estimated end times
    let mut vtt = String::from("WEBVTT\n\n");
    for (i, (begin, text)) in cues.iter().enumerate() {
        // End time = start of next cue, or begin + 5s for last cue
        let end = if i + 1 < cues.len() {
            cues[i + 1].0
        } else {
            begin + 5.0
        };

        vtt.push_str(&format!(
            "{} --> {}\n{}\n\n",
            format_webvtt_time(*begin),
            format_webvtt_time(end),
            text
        ));
    }

    vtt
}

// ============================================================
// Time Parsing & Formatting Helpers
// ============================================================

/// Parse a TTML timestamp string to seconds.
///
/// Supports these formats:
/// - `HH:MM:SS.fff` (hours:minutes:seconds.fractional)
/// - `MM:SS.fff` (minutes:seconds.fractional)
/// - `SS.fff` (seconds.fractional)
/// - `NNs` (seconds with 's' suffix)
fn parse_ttml_time(time_str: &str) -> f64 {
    // Handle 's' suffix (e.g., "15.8s")
    if let Some(stripped) = time_str.strip_suffix('s') {
        return stripped.parse::<f64>().unwrap_or(0.0);
    }

    let parts: Vec<&str> = time_str.split(':').collect();
    match parts.len() {
        // HH:MM:SS.fff
        3 => {
            let hours = parts[0].parse::<f64>().unwrap_or(0.0);
            let minutes = parts[1].parse::<f64>().unwrap_or(0.0);
            let seconds = parts[2].parse::<f64>().unwrap_or(0.0);
            hours * 3600.0 + minutes * 60.0 + seconds
        }
        // MM:SS.fff
        2 => {
            let minutes = parts[0].parse::<f64>().unwrap_or(0.0);
            let seconds = parts[1].parse::<f64>().unwrap_or(0.0);
            minutes * 60.0 + seconds
        }
        // SS.fff
        1 => parts[0].parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Parse an LRC timestamp string `mm:ss.xx` to seconds.
///
/// Returns `None` if the string doesn't match the expected format
/// (used to skip metadata tags like `[ar:Artist]`).
fn parse_lrc_time(time_str: &str) -> Option<f64> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let minutes = parts[0].parse::<f64>().ok()?;
    let seconds = parts[1].parse::<f64>().ok()?;
    Some(minutes * 60.0 + seconds)
}

/// Format a time in seconds as a WebVTT timestamp: `HH:MM:SS.mmm`
///
/// WebVTT requires hours (2 digits), minutes (2 digits), seconds (2 digits),
/// and milliseconds (3 digits) separated by colons and a dot.
fn format_webvtt_time(seconds: f64) -> String {
    let total_ms = (seconds * 1000.0).round() as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let secs = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{hours:02}:{minutes:02}:{secs:02}.{ms:03}")
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // WebVTT time formatting
    // ----------------------------------------------------------

    #[test]
    fn format_webvtt_time_zero() {
        assert_eq!(format_webvtt_time(0.0), "00:00:00.000");
    }

    #[test]
    fn format_webvtt_time_seconds_and_ms() {
        assert_eq!(format_webvtt_time(12.45), "00:00:12.450");
    }

    #[test]
    fn format_webvtt_time_minutes() {
        assert_eq!(format_webvtt_time(75.5), "00:01:15.500");
    }

    #[test]
    fn format_webvtt_time_hours() {
        assert_eq!(format_webvtt_time(3661.123), "01:01:01.123");
    }

    // ----------------------------------------------------------
    // TTML time parsing
    // ----------------------------------------------------------

    #[test]
    fn parse_ttml_time_hms() {
        let result = parse_ttml_time("01:02:03.456");
        assert!((result - 3723.456).abs() < 0.001);
    }

    #[test]
    fn parse_ttml_time_ms() {
        let result = parse_ttml_time("02:30.500");
        assert!((result - 150.5).abs() < 0.001);
    }

    #[test]
    fn parse_ttml_time_seconds_suffix() {
        let result = parse_ttml_time("15.8s");
        assert!((result - 15.8).abs() < 0.001);
    }

    // ----------------------------------------------------------
    // LRC time parsing
    // ----------------------------------------------------------

    #[test]
    fn parse_lrc_time_valid() {
        assert_eq!(parse_lrc_time("01:30.50"), Some(90.5));
    }

    #[test]
    fn parse_lrc_time_metadata_tag() {
        // Metadata tags like "ar:Artist" should return None
        assert_eq!(parse_lrc_time("ar:Taylor Swift"), None);
    }

    // ----------------------------------------------------------
    // SRT → WebVTT conversion
    // ----------------------------------------------------------

    #[test]
    fn srt_to_webvtt_basic() {
        let srt = "1\n00:00:12,450 --> 00:00:15,200\nHello world\n\n";
        let vtt = srt_to_webvtt(srt);
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("00:00:12.450 --> 00:00:15.200"));
        assert!(vtt.contains("Hello world"));
    }

    #[test]
    fn srt_to_webvtt_preserves_text() {
        let srt = "1\n00:01:00,000 --> 00:01:05,000\nLine one\nLine two\n\n";
        let vtt = srt_to_webvtt(srt);
        assert!(vtt.contains("Line one"));
        assert!(vtt.contains("Line two"));
    }

    // ----------------------------------------------------------
    // LRC → WebVTT conversion
    // ----------------------------------------------------------

    #[test]
    fn lrc_to_webvtt_basic() {
        let lrc = "[00:12.45]Hello world\n[00:15.20]Second line\n";
        let vtt = lrc_to_webvtt(lrc);
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("00:00:12.450 --> 00:00:15.200"));
        assert!(vtt.contains("Hello world"));
        assert!(vtt.contains("Second line"));
    }

    #[test]
    fn lrc_to_webvtt_strips_enhanced_timestamps() {
        let lrc = "[00:12.45]<00:12.45>Hello <00:13.20>world\n[00:15.20]Next\n";
        let vtt = lrc_to_webvtt(lrc);
        assert!(vtt.contains("Hello world"));
        // Should NOT contain Enhanced LRC inline timestamps
        assert!(!vtt.contains("<00:"));
    }

    #[test]
    fn lrc_to_webvtt_skips_metadata() {
        let lrc = "[ar:Taylor Swift]\n[ti:Song Title]\n[00:12.45]Lyrics here\n";
        let vtt = lrc_to_webvtt(lrc);
        assert!(!vtt.contains("Taylor Swift"));
        assert!(vtt.contains("Lyrics here"));
    }

    #[test]
    fn lrc_to_webvtt_empty_returns_header_only() {
        let lrc = "[ar:Artist]\n[ti:Title]\n";
        let vtt = lrc_to_webvtt(lrc);
        assert_eq!(vtt, "WEBVTT\n\n");
    }

    // ----------------------------------------------------------
    // TTML → WebVTT conversion
    // ----------------------------------------------------------

    #[test]
    fn ttml_to_webvtt_basic() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:12.450" end="00:15.200">Hello world</p>
      <p begin="00:15.200" end="00:18.000">Second line</p>
    </div>
  </body>
</tt>"#;

        let vtt = ttml_to_webvtt(ttml).unwrap();
        assert!(vtt.starts_with("WEBVTT\n\n"));
        assert!(vtt.contains("00:00:12.450 --> 00:00:15.200"));
        assert!(vtt.contains("Hello world"));
        assert!(vtt.contains("Second line"));
    }

    #[test]
    fn ttml_to_webvtt_with_spans() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:12.450" end="00:15.200">
        <span>Hello </span>
        <span>world</span>
      </p>
    </div>
  </body>
</tt>"#;

        let vtt = ttml_to_webvtt(ttml).unwrap();
        assert!(vtt.contains("Hello world"));
    }

    #[test]
    fn ttml_to_webvtt_empty_body() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body><div></div></body>
</tt>"#;

        let result = ttml_to_webvtt(ttml);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No timed cues"));
    }
}
