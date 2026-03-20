// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// ASS (Advanced SubStation Alpha) subtitle generation service.
// =============================================================
//
// Generates ASS subtitle files from TTML or WebVTT sources that
// preserve rich formatting: colours, bold, italic, underline, and
// dynamic positioning. ASS is the most capable text subtitle format,
// supporting per-character styling, positioning, fonts, and effects.
//
// ASS colour format: \c&HBBGGRR& (Blue-Green-Red, opposite of RGB).
// ASS override tags: {\b1}, {\i1}, {\u1}, {\c&H0000FF&} (inline).
//
// Source priority: TTML first (richest, has tts:* attributes and
// tts:origin/tts:extent positioning), then WebVTT.
//
// Used by: `download_queue` (post-download enrichment Step 2f,
//          when `generate_ass` enabled)

use std::collections::HashMap;
use std::path::Path;

use super::rich_srt_service::{
    collect_span_text, parse_ttml_time, resolve_element_style, resolve_named_styles,
    resolve_span_style, TtmlStyle, TTS_NS,
};

// ============================================================
// Constants
// ============================================================

/// Default ASS script resolution (width x height). Standard for
/// subtitle rendering in most players (VLC, mpv, MPC-HC).
const PLAY_RES_X: u32 = 384;
const PLAY_RES_Y: u32 = 288;

/// Default font size for ASS subtitles.
const DEFAULT_FONT_SIZE: u32 = 20;

/// Default font name. Arial is available on all platforms.
const DEFAULT_FONT: &str = "Arial";

// ============================================================
// ASS Generation (Public API)
// ============================================================

/// Generate ASS subtitle files from TTML or WebVTT sources for all
/// tracks in a directory.
///
/// Scans `album_dir` for media files (`.m4a`, `.m4v`, `.mp4`). For each
/// media file, looks for a rich source in priority order:
///   1. `.ttml` — richest source (Apple Music, has `tts:*` styling + positioning)
///   2. `.vtt`  — also supports styling (`<b>`, `<i>`, `<u>`)
///
/// Skips if `.ass` already exists for a track.
///
/// # Returns
/// * `Ok(count)` — Number of ASS files generated.
/// * `Err(msg)` — Directory read failure.
pub fn generate_ass_for_directory(album_dir: &str) -> Result<usize, String> {
    let dir = Path::new(album_dir);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {album_dir}"));
    }

    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory {album_dir}: {e}"))?;

    let mut generated = 0;

    for entry in entries.flatten() {
        let path = entry.path();

        // Only process media files (.m4a, .m4v, .mp4)
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

        // Skip if .ass already exists
        let ass_path = dir.join(format!("{stem}.ass"));
        if ass_path.exists() {
            continue;
        }

        // Try rich sources in priority order: TTML → WebVTT
        let ass_content = if let Some(ass) = try_ass_from_ttml(dir, &stem) {
            ass
        } else if let Some(ass) = try_ass_from_webvtt(dir, &stem) {
            ass
        } else {
            continue;
        };

        if let Err(e) = std::fs::write(&ass_path, &ass_content) {
            log::debug!("Failed to write ASS for {stem}: {e}");
            continue;
        }

        log::debug!("Generated ASS: {stem}.ass");
        generated += 1;
    }

    Ok(generated)
}

/// Try to generate ASS from a TTML source file.
fn try_ass_from_ttml(dir: &Path, stem: &str) -> Option<String> {
    let ttml_path = dir.join(format!("{stem}.ttml"));
    if !ttml_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&ttml_path).ok()?;
    match ttml_to_ass(&content, stem) {
        Ok(ass) => Some(ass),
        Err(e) => {
            log::debug!("Failed to convert TTML to ASS for {stem}: {e}");
            None
        }
    }
}

/// Try to generate ASS from a WebVTT source file.
fn try_ass_from_webvtt(dir: &Path, stem: &str) -> Option<String> {
    let vtt_path = dir.join(format!("{stem}.vtt"));
    if !vtt_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&vtt_path).ok()?;
    match webvtt_to_ass(&content, stem) {
        Ok(ass) => Some(ass),
        Err(e) => {
            log::debug!("Failed to convert WebVTT to ASS for {stem}: {e}");
            None
        }
    }
}

// ============================================================
// TTML → ASS Conversion
// ============================================================

/// Convert TTML XML content to ASS format.
///
/// Parses the TTML document, resolves named styles, extracts timed text
/// from `<p>` elements, and generates ASS Dialogue events with inline
/// override tags for styling and positioning.
pub fn ttml_to_ass(ttml_content: &str, title: &str) -> Result<String, String> {
    let doc = roxmltree::Document::parse(ttml_content)
        .map_err(|e| format!("Failed to parse TTML: {e}"))?;

    let named_styles = resolve_named_styles(&doc);
    let mut events: Vec<String> = Vec::new();
    let mut has_bg_vocals = false;

    for node in doc.descendants() {
        if node.tag_name().name() != "p" {
            continue;
        }

        let begin = match node.attribute("begin") {
            Some(b) => parse_ttml_time(b),
            None => continue,
        };
        let end = match node.attribute("end") {
            Some(e) => parse_ttml_time(e),
            None => {
                if let Some(dur) = node.attribute("dur") {
                    begin + parse_ttml_time(dur)
                } else {
                    begin + 5.0
                }
            }
        };

        let p_style = resolve_element_style(&node, &named_styles);

        // Extract positioning from tts:origin (if present)
        let position = extract_position(&node);

        // Collect ASS-formatted text with inline overrides
        let (text, uses_bg) = collect_ass_text(&node, &p_style, &named_styles);
        if text.trim().is_empty() {
            continue;
        }
        if uses_bg {
            has_bg_vocals = true;
        }

        // Build positioning override tags
        let mut overrides = String::new();
        if let Some((x, y)) = position {
            overrides.push_str(&format!("{{\\pos({x},{y})}}"));
        }

        let style_name = if uses_bg { "BgVocals" } else { "Default" };

        events.push(format!(
            "Dialogue: 0,{},{},{style_name},,0,0,0,,{overrides}{text}",
            format_ass_time(begin),
            format_ass_time(end),
        ));
    }

    if events.is_empty() {
        return Err("No timed cues found in TTML".to_string());
    }

    // Build the full ASS file
    let mut ass = build_script_info(title);
    ass.push_str(&build_styles(has_bg_vocals));
    ass.push_str("\n[Events]\n");
    ass.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );
    for event in &events {
        ass.push_str(event);
        ass.push('\n');
    }

    Ok(ass)
}

/// Extract positioning from TTML `tts:origin` attribute.
///
/// TTML positioning: `tts:origin="10% 80%"` or `tts:origin="50px 200px"`.
/// Returns `(x, y)` in ASS pixel coordinates relative to `PlayResX/PlayResY`.
fn extract_position(node: &roxmltree::Node) -> Option<(u32, u32)> {
    let origin = node
        .attribute((TTS_NS, "origin"))
        .or_else(|| node.attribute("origin"))?;

    let parts: Vec<&str> = origin.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let x = parse_position_value(parts[0], PLAY_RES_X)?;
    let y = parse_position_value(parts[1], PLAY_RES_Y)?;

    Some((x, y))
}

/// Parse a single position value (percentage or pixel) to ASS coordinates.
fn parse_position_value(value: &str, resolution: u32) -> Option<u32> {
    if let Some(pct) = value.strip_suffix('%') {
        let pct_val: f64 = pct.parse().ok()?;
        Some(((pct_val / 100.0) * f64::from(resolution)).round() as u32)
    } else if let Some(px) = value.strip_suffix("px") {
        Some(px.parse().ok()?)
    } else {
        value.parse().ok()
    }
}

// ============================================================
// WebVTT → ASS Conversion
// ============================================================

/// Convert WebVTT content to ASS format.
///
/// Parses WebVTT cues, preserves styling tags by converting them to
/// ASS inline override tags, and generates a complete ASS file.
pub fn webvtt_to_ass(vtt_content: &str, title: &str) -> Result<String, String> {
    let mut events: Vec<String> = Vec::new();
    let mut lines = vtt_content.lines().peekable();

    // Skip WEBVTT header
    let mut past_header = false;
    while let Some(line) = lines.peek() {
        let trimmed = line.trim();
        if trimmed.is_empty() && past_header {
            lines.next();
            break;
        }
        if trimmed.starts_with("WEBVTT") {
            past_header = true;
        }
        lines.next();
    }

    // Parse cues
    while lines.peek().is_some() {
        // Skip blank lines and cue identifiers
        while let Some(line) = lines.peek() {
            let trimmed = line.trim();
            if trimmed.contains("-->") {
                break;
            }
            lines.next();
        }

        let Some(timestamp_line) = lines.next() else {
            break;
        };
        let trimmed_ts = timestamp_line.trim();
        if !trimmed_ts.contains("-->") {
            continue;
        }

        let parts: Vec<&str> = trimmed_ts.splitn(2, "-->").collect();
        if parts.len() != 2 {
            continue;
        }

        let start = parse_vtt_time_to_seconds(parts[0].trim());
        let end_str = parts[1].split_whitespace().next().unwrap_or("");
        let end = parse_vtt_time_to_seconds(end_str);

        // Collect cue text
        let mut cue_text = String::new();
        while let Some(line) = lines.peek() {
            if line.trim().is_empty() {
                lines.next();
                break;
            }
            if !cue_text.is_empty() {
                cue_text.push_str("\\N");
            }
            cue_text.push_str(&vtt_tags_to_ass(line.trim()));
            lines.next();
        }

        if !cue_text.is_empty() {
            events.push(format!(
                "Dialogue: 0,{},{},Default,,0,0,0,,{cue_text}",
                format_ass_time(start),
                format_ass_time(end),
            ));
        }
    }

    if events.is_empty() {
        return Err("No cues found in WebVTT".to_string());
    }

    let mut ass = build_script_info(title);
    ass.push_str(&build_styles(false));
    ass.push_str("\n[Events]\n");
    ass.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );
    for event in &events {
        ass.push_str(event);
        ass.push('\n');
    }

    Ok(ass)
}

/// Convert WebVTT inline tags to ASS override tags.
///
/// `<b>text</b>` → `{\b1}text{\b0}`
/// `<i>text</i>` → `{\i1}text{\i0}`
/// `<u>text</u>` → `{\u1}text{\u0}`
/// Other VTT tags (`<c>`, `<v>`, timestamps) are stripped.
fn vtt_tags_to_ass(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut tag = String::from('<');
            for c in chars.by_ref() {
                tag.push(c);
                if c == '>' {
                    break;
                }
            }

            let tag_inner = tag.trim_start_matches('<').trim_end_matches('>').trim();
            let tag_lower = tag_inner.to_lowercase();

            match tag_lower.as_str() {
                "b" => result.push_str("{\\b1}"),
                "/b" => result.push_str("{\\b0}"),
                "i" => result.push_str("{\\i1}"),
                "/i" => result.push_str("{\\i0}"),
                "u" => result.push_str("{\\u1}"),
                "/u" => result.push_str("{\\u0}"),
                _ => {} // Strip VTT-only tags
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Parse a WebVTT timestamp (HH:MM:SS.mmm or MM:SS.mmm) to seconds.
fn parse_vtt_time_to_seconds(time_str: &str) -> f64 {
    let parts: Vec<&str> = time_str.split(':').collect();
    match parts.len() {
        3 => {
            let h: f64 = parts[0].parse().unwrap_or(0.0);
            let m: f64 = parts[1].parse().unwrap_or(0.0);
            let s: f64 = parts[2].parse().unwrap_or(0.0);
            h * 3600.0 + m * 60.0 + s
        }
        2 => {
            let m: f64 = parts[0].parse().unwrap_or(0.0);
            let s: f64 = parts[1].parse().unwrap_or(0.0);
            m * 60.0 + s
        }
        _ => 0.0,
    }
}

// ============================================================
// ASS Text Collection (TTML-specific)
// ============================================================

/// Collect ASS-formatted text from a `<p>` element with inline override tags.
///
/// Returns `(text, uses_bg_vocals)`. Background vocals get the "BgVocals"
/// style applied at the Dialogue level.
fn collect_ass_text(
    node: &roxmltree::Node,
    parent_style: &TtmlStyle,
    named_styles: &HashMap<String, TtmlStyle>,
) -> (String, bool) {
    let mut text = String::new();
    let mut uses_bg = false;
    let has_spans = node.children().any(|c| c.tag_name().name() == "span");

    for child in node.children() {
        if child.is_text() {
            let t = child.text().unwrap_or("");
            if has_spans && t.trim().is_empty() {
                continue;
            }
            text.push_str(&apply_ass_overrides(t, parent_style));
        } else if child.tag_name().name() == "span" {
            let span_style = resolve_span_style(&child, parent_style, named_styles);

            let is_bg = child
                .attribute(("http://www.w3.org/ns/ttml#metadata", "role"))
                .or_else(|| child.attribute("role"))
                .is_some_and(|r| r.contains("x-bg"));

            let span_text = collect_span_text(&child);
            if span_text.is_empty() {
                continue;
            }

            if is_bg {
                uses_bg = true;
                text.push_str(&format!("({span_text})"));
            } else {
                text.push_str(&apply_ass_overrides(&span_text, &span_style));
            }
        }
    }

    (text, uses_bg)
}

/// Apply ASS inline override tags based on resolved style.
///
/// Only emits override tags for non-default attributes. Uses paired
/// enable/disable tags: `{\b1}text{\b0}`.
fn apply_ass_overrides(text: &str, style: &TtmlStyle) -> String {
    if !style.has_styling() {
        return text.to_string();
    }

    let mut prefix = String::from('{');
    let mut suffix = String::from('{');
    let mut has_prefix = false;

    if style.bold {
        prefix.push_str("\\b1");
        suffix.push_str("\\b0");
        has_prefix = true;
    }
    if style.italic {
        prefix.push_str("\\i1");
        suffix.push_str("\\i0");
        has_prefix = true;
    }
    if style.underline {
        prefix.push_str("\\u1");
        suffix.push_str("\\u0");
        has_prefix = true;
    }
    if let Some(ref color) = style.color {
        if let Some(ass_color) = rgb_hex_to_ass_color(color) {
            prefix.push_str(&format!("\\c{ass_color}"));
            suffix.push_str("\\c");
            has_prefix = true;
        }
    }

    if !has_prefix {
        return text.to_string();
    }

    prefix.push('}');
    suffix.push('}');
    format!("{prefix}{text}{suffix}")
}

/// Convert RGB hex colour "#RRGGBB" to ASS BGR colour "&HBBGGRR&".
///
/// ASS uses Blue-Green-Red byte order (opposite of standard RGB).
/// Example: "#FF0000" (red) → "&H0000FF&"
fn rgb_hex_to_ass_color(hex: &str) -> Option<String> {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return None;
    }

    let r = &hex[0..2];
    let g = &hex[2..4];
    let b = &hex[4..6];

    Some(format!("&H00{b}{g}{r}&"))
}

// ============================================================
// ASS File Structure Builders
// ============================================================

/// Build the [Script Info] section of the ASS file.
fn build_script_info(title: &str) -> String {
    format!(
        "[Script Info]\n\
         Title: {title}\n\
         ScriptType: v4.00+\n\
         WrapStyle: 0\n\
         ScaledBorderAndShadow: yes\n\
         YCbCr Matrix: TV.601\n\
         PlayResX: {PLAY_RES_X}\n\
         PlayResY: {PLAY_RES_Y}\n\
         ; Generated by MeedyaDL\n\n"
    )
}

/// Build the [V4+ Styles] section with Default and optional BgVocals styles.
fn build_styles(include_bg_vocals: bool) -> String {
    let mut styles = String::from("[V4+ Styles]\n");
    styles.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");

    // Default style: white text, black outline, bottom-centre aligned
    styles.push_str(&format!(
        "Style: Default,{DEFAULT_FONT},{DEFAULT_FONT_SIZE},&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,1,2,10,10,10,1\n"
    ));

    if include_bg_vocals {
        // Background vocals: slightly smaller, semi-transparent, italic
        let bg_size = DEFAULT_FONT_SIZE - 2;
        styles.push_str(&format!(
            "Style: BgVocals,{DEFAULT_FONT},{bg_size},&H40FFFFFF,&H000000FF,&H00000000,&H80000000,0,1,0,0,100,100,0,0,1,2,1,2,10,10,10,1\n"
        ));
    }

    styles
}

// ============================================================
// Time Formatting
// ============================================================

/// Format seconds as ASS timestamp: `H:MM:SS.cc` (centiseconds, single-digit hour).
///
/// ASS uses centiseconds (1/100th second), not milliseconds.
fn format_ass_time(seconds: f64) -> String {
    let total_cs = (seconds * 100.0).round() as u64;
    let hours = total_cs / 360_000;
    let minutes = (total_cs % 360_000) / 6000;
    let secs = (total_cs % 6000) / 100;
    let cs = total_cs % 100;
    format!("{hours}:{minutes:02}:{secs:02}.{cs:02}")
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // ASS time formatting
    // ----------------------------------------------------------

    #[test]
    fn format_ass_time_zero() {
        assert_eq!(format_ass_time(0.0), "0:00:00.00");
    }

    #[test]
    fn format_ass_time_with_hours() {
        assert_eq!(format_ass_time(3661.5), "1:01:01.50");
    }

    #[test]
    fn format_ass_time_centiseconds() {
        assert_eq!(format_ass_time(12.456), "0:00:12.46"); // Rounded to centiseconds
    }

    // ----------------------------------------------------------
    // RGB to ASS BGR conversion
    // ----------------------------------------------------------

    #[test]
    fn rgb_to_ass_red() {
        assert_eq!(
            rgb_hex_to_ass_color("#FF0000"),
            Some("&H000000FF&".to_string())
        );
    }

    #[test]
    fn rgb_to_ass_blue() {
        assert_eq!(
            rgb_hex_to_ass_color("#0000FF"),
            Some("&H00FF0000&".to_string())
        );
    }

    #[test]
    fn rgb_to_ass_white() {
        assert_eq!(
            rgb_hex_to_ass_color("#FFFFFF"),
            Some("&H00FFFFFF&".to_string())
        );
    }

    #[test]
    fn rgb_to_ass_green() {
        assert_eq!(
            rgb_hex_to_ass_color("#00FF00"),
            Some("&H0000FF00&".to_string())
        );
    }

    #[test]
    fn rgb_to_ass_invalid() {
        assert_eq!(rgb_hex_to_ass_color("#FFF"), None);
    }

    // ----------------------------------------------------------
    // ASS override tags
    // ----------------------------------------------------------

    #[test]
    fn ass_overrides_no_style() {
        let style = TtmlStyle::default();
        assert_eq!(apply_ass_overrides("hello", &style), "hello");
    }

    #[test]
    fn ass_overrides_bold() {
        let style = TtmlStyle {
            bold: true,
            ..Default::default()
        };
        assert_eq!(apply_ass_overrides("hello", &style), "{\\b1}hello{\\b0}");
    }

    #[test]
    fn ass_overrides_italic() {
        let style = TtmlStyle {
            italic: true,
            ..Default::default()
        };
        assert_eq!(apply_ass_overrides("hello", &style), "{\\i1}hello{\\i0}");
    }

    #[test]
    fn ass_overrides_underline() {
        let style = TtmlStyle {
            underline: true,
            ..Default::default()
        };
        assert_eq!(apply_ass_overrides("hello", &style), "{\\u1}hello{\\u0}");
    }

    #[test]
    fn ass_overrides_color() {
        let style = TtmlStyle {
            color: Some("#FF0000".to_string()),
            ..Default::default()
        };
        assert_eq!(
            apply_ass_overrides("hello", &style),
            "{\\c&H000000FF&}hello{\\c}"
        );
    }

    #[test]
    fn ass_overrides_combined() {
        let style = TtmlStyle {
            bold: true,
            italic: true,
            color: Some("#00FF00".to_string()),
            ..Default::default()
        };
        let result = apply_ass_overrides("hello", &style);
        assert!(result.contains("\\b1"));
        assert!(result.contains("\\i1"));
        assert!(result.contains("\\c&H0000FF00&"));
    }

    // ----------------------------------------------------------
    // VTT tags to ASS conversion
    // ----------------------------------------------------------

    #[test]
    fn vtt_bold_to_ass() {
        assert_eq!(vtt_tags_to_ass("<b>Bold</b>"), "{\\b1}Bold{\\b0}");
    }

    #[test]
    fn vtt_italic_to_ass() {
        assert_eq!(vtt_tags_to_ass("<i>Italic</i>"), "{\\i1}Italic{\\i0}");
    }

    #[test]
    fn vtt_underline_to_ass() {
        assert_eq!(vtt_tags_to_ass("<u>Under</u>"), "{\\u1}Under{\\u0}");
    }

    #[test]
    fn vtt_class_stripped() {
        assert_eq!(vtt_tags_to_ass("<c.red>text</c>"), "text");
    }

    #[test]
    fn vtt_plain_text() {
        assert_eq!(vtt_tags_to_ass("no tags here"), "no tags here");
    }

    // ----------------------------------------------------------
    // Position parsing
    // ----------------------------------------------------------

    #[test]
    fn parse_position_percentage() {
        assert_eq!(parse_position_value("50%", 384), Some(192));
    }

    #[test]
    fn parse_position_pixels() {
        assert_eq!(parse_position_value("100px", 384), Some(100));
    }

    #[test]
    fn parse_position_raw_number() {
        assert_eq!(parse_position_value("200", 384), Some(200));
    }

    // ----------------------------------------------------------
    // Script info and styles
    // ----------------------------------------------------------

    #[test]
    fn script_info_contains_title() {
        let info = build_script_info("Test Song");
        assert!(info.contains("Title: Test Song"));
        assert!(info.contains("ScriptType: v4.00+"));
        assert!(info.contains(&format!("PlayResX: {PLAY_RES_X}")));
    }

    #[test]
    fn styles_default_only() {
        let styles = build_styles(false);
        assert!(styles.contains("Style: Default,"));
        assert!(!styles.contains("Style: BgVocals,"));
    }

    #[test]
    fn styles_with_bg_vocals() {
        let styles = build_styles(true);
        assert!(styles.contains("Style: Default,"));
        assert!(styles.contains("Style: BgVocals,"));
    }

    // ----------------------------------------------------------
    // TTML → ASS conversion
    // ----------------------------------------------------------

    #[test]
    fn ttml_to_ass_basic() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000">Hello world</p>
    </div>
  </body>
</tt>"#;

        let ass = ttml_to_ass(ttml, "Test").unwrap();
        assert!(ass.contains("[Script Info]"));
        assert!(ass.contains("[V4+ Styles]"));
        assert!(ass.contains("[Events]"));
        assert!(ass.contains("Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello world"));
    }

    #[test]
    fn ttml_to_ass_with_bold() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:fontWeight="bold">Bold text</p>
    </div>
  </body>
</tt>"#;

        let ass = ttml_to_ass(ttml, "Test").unwrap();
        assert!(ass.contains("{\\b1}Bold text{\\b0}"));
    }

    #[test]
    fn ttml_to_ass_with_color() {
        let ttml = r##"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:color="#FF0000">Red text</p>
    </div>
  </body>
</tt>"##;

        let ass = ttml_to_ass(ttml, "Test").unwrap();
        // RGB #FF0000 → ASS BGR &H000000FF&
        assert!(ass.contains("\\c&H000000FF&"));
        assert!(ass.contains("Red text"));
    }

    #[test]
    fn ttml_to_ass_background_vocals() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000">Main <span ttm:role="x-bg">backing</span></p>
    </div>
  </body>
</tt>"#;

        let ass = ttml_to_ass(ttml, "Test").unwrap();
        assert!(ass.contains("BgVocals"));
        assert!(ass.contains("(backing)"));
    }

    #[test]
    fn ttml_to_ass_empty_body() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body><div/></body>
</tt>"#;

        let result = ttml_to_ass(ttml, "Test");
        assert!(result.is_err());
    }

    #[test]
    fn ttml_to_ass_with_positioning() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:origin="50% 80%">Positioned</p>
    </div>
  </body>
</tt>"#;

        let ass = ttml_to_ass(ttml, "Test").unwrap();
        // 50% of 384 = 192, 80% of 288 = 230
        assert!(ass.contains("{\\pos(192,230)}"));
    }

    // ----------------------------------------------------------
    // WebVTT → ASS conversion
    // ----------------------------------------------------------

    #[test]
    fn webvtt_to_ass_basic() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nHello world\n";
        let ass = webvtt_to_ass(vtt, "Test").unwrap();
        assert!(ass.contains("[Script Info]"));
        assert!(ass.contains("Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello world"));
    }

    #[test]
    fn webvtt_to_ass_with_bold() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\n<b>Bold</b> text\n";
        let ass = webvtt_to_ass(vtt, "Test").unwrap();
        assert!(ass.contains("{\\b1}Bold{\\b0} text"));
    }

    #[test]
    fn webvtt_to_ass_multiline() {
        let vtt = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nLine one\nLine two\n";
        let ass = webvtt_to_ass(vtt, "Test").unwrap();
        assert!(ass.contains("Line one\\NLine two"));
    }

    #[test]
    fn webvtt_to_ass_empty() {
        let result = webvtt_to_ass("WEBVTT\n\n", "Test");
        assert!(result.is_err());
    }

    // ----------------------------------------------------------
    // VTT time parsing
    // ----------------------------------------------------------

    #[test]
    fn parse_vtt_time_hms() {
        let t = parse_vtt_time_to_seconds("01:02:03.456");
        assert!((t - 3723.456).abs() < 0.001);
    }

    #[test]
    fn parse_vtt_time_ms() {
        let t = parse_vtt_time_to_seconds("02:30.000");
        assert!((t - 150.0).abs() < 0.001);
    }
}
