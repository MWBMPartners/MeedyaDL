// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Rich SRT subtitle generation and embedding service.
// =====================================================
//
// Generates format-rich SRT subtitle files from Apple Music TTML that
// preserve styling information (bold, italic, underline, colours) using
// HTML-like tags supported by most modern video players.
//
// Also provides subtitle embedding: reads .srt and .vtt sidecar files
// and embeds their content into MP4/M4A/M4V containers as freeform atoms
// (same pattern as Enhanced LRC's ©lyr atom).
//
// SRT styling tags supported:
//   <b>Bold</b>
//   <i>Italic</i>
//   <u>Underlined</u>
//   <font color="#RRGGBB">Coloured</font>
//
// TTML attributes mapped:
//   tts:fontWeight="bold"         → <b>
//   tts:fontStyle="italic"        → <i>
//   tts:textDecoration="underline"→ <u>
//   tts:color="#RRGGBB"           → <font color="...">
//
// Used by: `download_queue` (post-download enrichment Steps 2d and 2e)

use std::collections::HashMap;
use std::path::Path;

// ============================================================
// Constants
// ============================================================

/// TTML Styling namespace (W3C standard) for tts:* attributes.
const TTS_NS: &str = "http://www.w3.org/ns/ttml#styling";

/// Apple iTunes freeform atom namespace for subtitle embedding.
const ITUNES_NAMESPACE: &str = "com.apple.iTunes";

// ============================================================
// Style Types
// ============================================================

/// Resolved styling for a TTML element.
///
/// Tracks which style attributes are active so `wrap_with_style()`
/// can wrap text in the appropriate SRT HTML-like tags.
#[derive(Debug, Clone, Default)]
struct TtmlStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    /// Colour as "#RRGGBB" hex string, or None for default colour.
    color: Option<String>,
}

impl TtmlStyle {
    /// Returns true if any styling is active.
    fn has_styling(&self) -> bool {
        self.bold || self.italic || self.underline || self.color.is_some()
    }
}

// ============================================================
// Rich SRT Generation (Public API)
// ============================================================

/// Generate format-rich SRT files from TTML for all tracks in a directory.
///
/// Scans `album_dir` for `.ttml` files. For each TTML found, converts
/// to rich SRT with styling tags and writes/overwrites the corresponding
/// `.srt` file. TTML-derived SRT is strictly richer than plain SRT
/// downloaded by GAMDL, so overwriting is intentional.
///
/// If no TTML exists for a track, any existing plain `.srt` is kept.
///
/// # Returns
/// * `Ok(count)` — Number of rich SRT files generated.
/// * `Err(msg)` — Directory read failure.
pub fn generate_rich_srt_for_directory(album_dir: &str) -> Result<usize, String> {
    let dir = Path::new(album_dir);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {album_dir}"));
    }

    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {album_dir}: {e}"))?;

    let mut generated = 0;

    for entry in entries.flatten() {
        let path = entry.path();

        // Only process .ttml files
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext != "ttml" {
            continue;
        }

        // Get the file stem (e.g., "01 Song Title")
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        // Read the TTML content
        let ttml_content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::debug!("Failed to read TTML for {stem}: {e}");
                continue;
            }
        };

        // Convert TTML to rich SRT
        let srt_content = match ttml_to_rich_srt(&ttml_content) {
            Ok(srt) => srt,
            Err(e) => {
                log::debug!("Failed to convert TTML to rich SRT for {stem}: {e}");
                continue;
            }
        };

        // Write the .srt file (overwriting any existing plain SRT)
        let srt_path = dir.join(format!("{stem}.srt"));
        if let Err(e) = std::fs::write(&srt_path, &srt_content) {
            log::debug!("Failed to write rich SRT for {stem}: {e}");
            continue;
        }

        log::debug!("Generated rich SRT: {stem}.srt");
        generated += 1;
    }

    Ok(generated)
}

/// Convert TTML XML content to format-rich SRT.
///
/// Parses the TTML document, resolves named styles from `<head><styling>`,
/// extracts timed text from `<p>` elements, and wraps text in SRT
/// HTML-like styling tags based on resolved `tts:*` attributes.
///
/// If no styling attributes exist in the TTML, produces valid plain SRT.
pub fn ttml_to_rich_srt(ttml_content: &str) -> Result<String, String> {
    let doc = roxmltree::Document::parse(ttml_content)
        .map_err(|e| format!("Failed to parse TTML: {e}"))?;

    // Resolve named styles from <head><styling><style> definitions
    let named_styles = resolve_named_styles(&doc);

    let mut cues: Vec<(f64, f64, String)> = Vec::new();

    // Find all <p> elements (each represents one subtitle cue)
    for node in doc.descendants() {
        if node.tag_name().name() != "p" {
            continue;
        }

        // Extract begin and end timestamps
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

        // Resolve the <p> element's own styling
        let p_style = resolve_element_style(&node, &named_styles);

        // Collect styled text content from the <p> and its children
        let text = collect_styled_text(&node, &p_style, &named_styles);
        if text.trim().is_empty() {
            continue;
        }

        cues.push((begin, end, text.trim().to_string()));
    }

    if cues.is_empty() {
        return Err("No timed cues found in TTML".to_string());
    }

    // Build SRT output with sequential indices
    let mut srt = String::new();
    for (i, (begin, end, text)) in cues.iter().enumerate() {
        if i > 0 {
            srt.push('\n');
        }
        srt.push_str(&format!(
            "{}\n{} --> {}\n{}\n",
            i + 1,
            format_srt_time(*begin),
            format_srt_time(*end),
            text
        ));
    }

    Ok(srt)
}

// ============================================================
// Subtitle Embedding (Public API)
// ============================================================

/// Embed subtitle sidecar content into media files in a directory.
///
/// For each media file (`.m4a`, `.m4v`, `.mp4`), reads matching `.srt`
/// and `.vtt` sidecar files and embeds their content as freeform atoms
/// in the MP4 container. Uses the same `mp4ameta` pattern as Enhanced
/// LRC lyrics embedding.
///
/// # Returns
/// * `Ok(count)` — Number of media files with embedded subtitles.
/// * `Err(msg)` — Directory read failure.
pub fn embed_subtitles_for_directory(album_dir: &str) -> Result<usize, String> {
    let dir = Path::new(album_dir);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {album_dir}"));
    }

    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {album_dir}: {e}"))?;

    let mut embedded = 0;

    for entry in entries.flatten() {
        let path = entry.path();

        // Only process media files
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

        // Read matching sidecar files
        let srt_path = dir.join(format!("{stem}.srt"));
        let vtt_path = dir.join(format!("{stem}.vtt"));

        let srt_content = std::fs::read_to_string(&srt_path).ok();
        let vtt_content = std::fs::read_to_string(&vtt_path).ok();

        // Skip if neither sidecar exists
        if srt_content.is_none() && vtt_content.is_none() {
            continue;
        }

        match embed_subtitles_in_file(
            &path,
            srt_content.as_deref(),
            vtt_content.as_deref(),
        ) {
            Ok(()) => {
                log::debug!("Embedded subtitles in: {stem}.{ext}");
                embedded += 1;
            }
            Err(e) => {
                log::debug!("Failed to embed subtitles in {stem}.{ext}: {e}");
            }
        }
    }

    Ok(embedded)
}

/// Embed subtitle content as freeform atoms in a media file.
///
/// Uses the `mp4ameta` crate to write subtitle text as freeform atoms
/// in the `com.apple.iTunes` namespace. Preserves all existing metadata.
///
/// Atom names:
/// - `subtitles-srt` — SRT subtitle content
/// - `subtitles-vtt` — WebVTT subtitle content
pub fn embed_subtitles_in_file(
    file_path: &Path,
    srt_content: Option<&str>,
    vtt_content: Option<&str>,
) -> Result<(), String> {
    use mp4ameta::{Data, FreeformIdent, Tag};

    let mut tag = Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to read media file metadata: {e}"))?;

    if let Some(srt) = srt_content {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "subtitles-srt"),
            Data::Utf8(srt.to_string()),
        );
    }

    if let Some(vtt) = vtt_content {
        tag.set_data(
            FreeformIdent::new_static(ITUNES_NAMESPACE, "subtitles-vtt"),
            Data::Utf8(vtt.to_string()),
        );
    }

    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to write subtitle metadata: {e}"))?;

    Ok(())
}

// ============================================================
// Style Resolution Helpers
// ============================================================

/// Parse named style definitions from `<head><styling><style>` elements.
///
/// Returns a map from style ID (the `xml:id` attribute) to resolved
/// `TtmlStyle`. These are referenced by `style="..."` attributes on
/// `<p>` and `<span>` elements.
fn resolve_named_styles(doc: &roxmltree::Document) -> HashMap<String, TtmlStyle> {
    let mut styles = HashMap::new();

    for node in doc.descendants() {
        if node.tag_name().name() != "style" {
            continue;
        }

        // Get the style ID (xml:id or id attribute)
        let id = node
            .attribute((roxmltree::NS_XML_URI, "id"))
            .or_else(|| node.attribute("id"));

        let Some(id) = id else { continue };

        let style = extract_tts_style(&node);
        styles.insert(id.to_string(), style);
    }

    styles
}

/// Resolve the effective style for a TTML element.
///
/// Merges the named style (from `style="..."` reference) with any
/// inline `tts:*` attributes directly on the element. Inline attributes
/// override the named style.
fn resolve_element_style(
    node: &roxmltree::Node,
    named_styles: &HashMap<String, TtmlStyle>,
) -> TtmlStyle {
    // Start with the referenced named style (if any)
    let mut style = node
        .attribute("style")
        .and_then(|id| named_styles.get(id))
        .cloned()
        .unwrap_or_default();

    // Override with inline tts:* attributes
    let inline = extract_tts_style(node);
    if inline.bold {
        style.bold = true;
    }
    if inline.italic {
        style.italic = true;
    }
    if inline.underline {
        style.underline = true;
    }
    if inline.color.is_some() {
        style.color = inline.color;
    }

    style
}

/// Extract TTML styling attributes from a node's `tts:*` attributes.
///
/// Checks both namespaced (`(TTS_NS, "fontWeight")`) and unnamespaced
/// (`"fontWeight"`) lookups since Apple Music TTML may vary.
fn extract_tts_style(node: &roxmltree::Node) -> TtmlStyle {
    let mut style = TtmlStyle::default();

    // Bold: tts:fontWeight="bold"
    let font_weight = node
        .attribute((TTS_NS, "fontWeight"))
        .or_else(|| node.attribute("fontWeight"));
    if font_weight.is_some_and(|v| v.eq_ignore_ascii_case("bold")) {
        style.bold = true;
    }

    // Italic: tts:fontStyle="italic"
    let font_style = node
        .attribute((TTS_NS, "fontStyle"))
        .or_else(|| node.attribute("fontStyle"));
    if font_style.is_some_and(|v| v.eq_ignore_ascii_case("italic")) {
        style.italic = true;
    }

    // Underline: tts:textDecoration="underline"
    let text_decoration = node
        .attribute((TTS_NS, "textDecoration"))
        .or_else(|| node.attribute("textDecoration"));
    if text_decoration.is_some_and(|v| v.to_lowercase().contains("underline")) {
        style.underline = true;
    }

    // Colour: tts:color="#RRGGBB" or tts:color="white"
    let color = node
        .attribute((TTS_NS, "color"))
        .or_else(|| node.attribute("color"));
    if let Some(c) = color {
        style.color = normalize_color(c);
    }

    style
}

/// Normalize a TTML colour value to "#RRGGBB" hex format.
///
/// Handles both hex codes (with or without `#` prefix) and named colours.
/// Returns `None` for unrecognized values.
fn normalize_color(value: &str) -> Option<String> {
    let trimmed = value.trim();

    // Already a hex code
    if trimmed.starts_with('#') && (trimmed.len() == 7 || trimmed.len() == 9) {
        // Return the first 7 chars (#RRGGBB), strip alpha channel if present (#RRGGBBAA)
        return Some(trimmed[..7].to_uppercase());
    }

    // Hex without # prefix
    if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(format!("#{}", trimmed.to_uppercase()));
    }

    // Named colour
    named_color_to_hex(trimmed).map(std::string::ToString::to_string)
}

/// Map common CSS/TTML named colours to hex codes.
fn named_color_to_hex(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "white" => Some("#FFFFFF"),
        "black" => Some("#000000"),
        "red" => Some("#FF0000"),
        "green" | "lime" => Some("#00FF00"),
        "blue" => Some("#0000FF"),
        "yellow" => Some("#FFFF00"),
        "cyan" | "aqua" => Some("#00FFFF"),
        "magenta" | "fuchsia" => Some("#FF00FF"),
        "gray" | "grey" => Some("#808080"),
        "silver" => Some("#C0C0C0"),
        "orange" => Some("#FFA500"),
        "purple" => Some("#800080"),
        _ => None,
    }
}

// ============================================================
// Styled Text Collection
// ============================================================

/// Collect styled text content from a `<p>` element and its children.
///
/// For each text run, resolves the effective style (parent + child
/// overrides) and wraps the text in SRT HTML-like tags. Background
/// vocals (`ttm:role="x-bg"`) are wrapped in parentheses.
fn collect_styled_text(
    node: &roxmltree::Node,
    parent_style: &TtmlStyle,
    named_styles: &HashMap<String, TtmlStyle>,
) -> String {
    let mut text = String::new();

    // Check if this <p> has <span> children for whitespace handling
    let has_spans = node.children().any(|c| c.tag_name().name() == "span");

    for child in node.children() {
        if child.is_text() {
            let t = child.text().unwrap_or("");
            // Skip XML indentation whitespace between spans
            if has_spans && t.trim().is_empty() {
                continue;
            }
            // Direct text inherits parent style
            text.push_str(&wrap_with_style(t, parent_style));
        } else if child.tag_name().name() == "span" {
            // Resolve span-level style (inherits from parent, overrides with own)
            let span_style = resolve_span_style(&child, parent_style, named_styles);

            // Check for background vocal marker (ttm:role="x-bg")
            // Check both namespaced (http://www.w3.org/ns/ttml#metadata)
            // and unnamespaced lookups for compatibility.
            let is_bg = child
                .attribute(("http://www.w3.org/ns/ttml#metadata", "role"))
                .or_else(|| child.attribute("role"))
                .is_some_and(|r| r.contains("x-bg"));

            // Collect text from span children (may be nested)
            let span_text = collect_span_text(&child);

            if span_text.is_empty() {
                continue;
            }

            let styled = wrap_with_style(&span_text, &span_style);

            if is_bg && !styled.starts_with('(') {
                text.push_str(&format!("({styled})"));
            } else {
                text.push_str(&styled);
            }
        }
    }

    text
}

/// Resolve effective style for a `<span>` element.
///
/// Starts with the parent `<p>` style and overrides with the span's
/// own inline and named style attributes.
fn resolve_span_style(
    node: &roxmltree::Node,
    parent_style: &TtmlStyle,
    named_styles: &HashMap<String, TtmlStyle>,
) -> TtmlStyle {
    // Start with parent style as base
    let mut style = parent_style.clone();

    // Override with named style reference
    if let Some(ref_style) = node
        .attribute("style")
        .and_then(|id| named_styles.get(id))
    {
        if ref_style.bold {
            style.bold = true;
        }
        if ref_style.italic {
            style.italic = true;
        }
        if ref_style.underline {
            style.underline = true;
        }
        if ref_style.color.is_some() {
            style.color.clone_from(&ref_style.color);
        }
    }

    // Override with inline tts:* attributes
    let inline = extract_tts_style(node);
    if inline.bold {
        style.bold = true;
    }
    if inline.italic {
        style.italic = true;
    }
    if inline.underline {
        style.underline = true;
    }
    if inline.color.is_some() {
        style.color = inline.color;
    }

    style
}

/// Collect raw text from a `<span>` element and its children.
fn collect_span_text(node: &roxmltree::Node) -> String {
    node.children()
        .filter_map(|c| c.text())
        .collect::<Vec<_>>()
        .join("")
}

/// Wrap text in SRT HTML-like styling tags based on resolved style.
///
/// Tags are nested outside-in: `<font>` → `<b>` → `<i>` → `<u>` → text.
/// Only non-default attributes produce tags. If no styling is active,
/// returns the text unchanged.
fn wrap_with_style(text: &str, style: &TtmlStyle) -> String {
    if !style.has_styling() {
        return text.to_string();
    }

    let mut result = text.to_string();

    // Innermost: underline
    if style.underline {
        result = format!("<u>{result}</u>");
    }

    // Italic
    if style.italic {
        result = format!("<i>{result}</i>");
    }

    // Bold
    if style.bold {
        result = format!("<b>{result}</b>");
    }

    // Outermost: colour
    if let Some(ref color) = style.color {
        result = format!("<font color=\"{color}\">{result}</font>");
    }

    result
}

// ============================================================
// Time Parsing & Formatting
// ============================================================

/// Parse a TTML timestamp string to seconds.
///
/// Supports: `HH:MM:SS.fff`, `MM:SS.fff`, `SS.fff`, `NNs` (with 's' suffix).
fn parse_ttml_time(time_str: &str) -> f64 {
    if let Some(stripped) = time_str.strip_suffix('s') {
        return stripped.parse::<f64>().unwrap_or(0.0);
    }

    let parts: Vec<&str> = time_str.split(':').collect();
    match parts.len() {
        3 => {
            let hours = parts[0].parse::<f64>().unwrap_or(0.0);
            let minutes = parts[1].parse::<f64>().unwrap_or(0.0);
            let seconds = parts[2].parse::<f64>().unwrap_or(0.0);
            hours * 3600.0 + minutes * 60.0 + seconds
        }
        2 => {
            let minutes = parts[0].parse::<f64>().unwrap_or(0.0);
            let seconds = parts[1].parse::<f64>().unwrap_or(0.0);
            minutes * 60.0 + seconds
        }
        1 => parts[0].parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Format seconds as SRT timestamp: `HH:MM:SS,mmm`
///
/// SRT uses comma (`,`) as the millisecond separator, not dot (`.`).
fn format_srt_time(seconds: f64) -> String {
    let total_ms = (seconds * 1000.0).round() as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let secs = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{hours:02}:{minutes:02}:{secs:02},{ms:03}")
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // SRT time formatting
    // ----------------------------------------------------------

    #[test]
    fn format_srt_time_zero() {
        assert_eq!(format_srt_time(0.0), "00:00:00,000");
    }

    #[test]
    fn format_srt_time_with_hours() {
        assert_eq!(format_srt_time(3661.5), "01:01:01,500");
    }

    #[test]
    fn format_srt_time_uses_comma_separator() {
        let t = format_srt_time(12.456);
        assert!(t.contains(','), "SRT timestamp must use comma: {t}");
        assert!(!t.contains('.'), "SRT timestamp must not use dot: {t}");
    }

    // ----------------------------------------------------------
    // TTML time parsing
    // ----------------------------------------------------------

    #[test]
    fn parse_ttml_time_hms() {
        let t = parse_ttml_time("01:02:03.456");
        assert!((t - 3723.456).abs() < 0.001);
    }

    #[test]
    fn parse_ttml_time_ms() {
        let t = parse_ttml_time("02:30.000");
        assert!((t - 150.0).abs() < 0.001);
    }

    #[test]
    fn parse_ttml_time_seconds_suffix() {
        let t = parse_ttml_time("15.8s");
        assert!((t - 15.8).abs() < 0.001);
    }

    // ----------------------------------------------------------
    // Named colour mapping
    // ----------------------------------------------------------

    #[test]
    fn named_color_white() {
        assert_eq!(named_color_to_hex("white"), Some("#FFFFFF"));
    }

    #[test]
    fn named_color_case_insensitive() {
        assert_eq!(named_color_to_hex("Red"), Some("#FF0000"));
        assert_eq!(named_color_to_hex("BLUE"), Some("#0000FF"));
    }

    #[test]
    fn named_color_unknown() {
        assert_eq!(named_color_to_hex("chartreuse"), None);
    }

    // ----------------------------------------------------------
    // Color normalization
    // ----------------------------------------------------------

    #[test]
    fn normalize_hex_color() {
        assert_eq!(normalize_color("#FF0000"), Some("#FF0000".to_string()));
    }

    #[test]
    fn normalize_hex_with_alpha() {
        // #RRGGBBAA → strip alpha, keep #RRGGBB
        assert_eq!(normalize_color("#FF0000FF"), Some("#FF0000".to_string()));
    }

    #[test]
    fn normalize_hex_without_hash() {
        assert_eq!(normalize_color("ff0000"), Some("#FF0000".to_string()));
    }

    #[test]
    fn normalize_named_color() {
        assert_eq!(normalize_color("white"), Some("#FFFFFF".to_string()));
    }

    // ----------------------------------------------------------
    // Style wrapping
    // ----------------------------------------------------------

    #[test]
    fn wrap_no_style() {
        let style = TtmlStyle::default();
        assert_eq!(wrap_with_style("hello", &style), "hello");
    }

    #[test]
    fn wrap_bold() {
        let style = TtmlStyle {
            bold: true,
            ..Default::default()
        };
        assert_eq!(wrap_with_style("hello", &style), "<b>hello</b>");
    }

    #[test]
    fn wrap_italic() {
        let style = TtmlStyle {
            italic: true,
            ..Default::default()
        };
        assert_eq!(wrap_with_style("hello", &style), "<i>hello</i>");
    }

    #[test]
    fn wrap_underline() {
        let style = TtmlStyle {
            underline: true,
            ..Default::default()
        };
        assert_eq!(wrap_with_style("hello", &style), "<u>hello</u>");
    }

    #[test]
    fn wrap_color() {
        let style = TtmlStyle {
            color: Some("#FF0000".to_string()),
            ..Default::default()
        };
        assert_eq!(
            wrap_with_style("hello", &style),
            "<font color=\"#FF0000\">hello</font>"
        );
    }

    #[test]
    fn wrap_all_styles() {
        let style = TtmlStyle {
            bold: true,
            italic: true,
            underline: true,
            color: Some("#FF0000".to_string()),
        };
        assert_eq!(
            wrap_with_style("hello", &style),
            "<font color=\"#FF0000\"><b><i><u>hello</u></i></b></font>"
        );
    }

    // ----------------------------------------------------------
    // TTML to Rich SRT conversion
    // ----------------------------------------------------------

    #[test]
    fn ttml_to_rich_srt_basic() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:00:12.450" end="00:00:15.200">Hello world</p>
      <p begin="00:00:15.200" end="00:00:18.500">Second line</p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("1\n00:00:12,450 --> 00:00:15,200\nHello world\n"));
        assert!(srt.contains("2\n00:00:15,200 --> 00:00:18,500\nSecond line\n"));
    }

    #[test]
    fn ttml_to_rich_srt_with_bold() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:fontWeight="bold">Bold text</p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("<b>Bold text</b>"));
    }

    #[test]
    fn ttml_to_rich_srt_with_italic() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:fontStyle="italic">Italic text</p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("<i>Italic text</i>"));
    }

    #[test]
    fn ttml_to_rich_srt_with_underline() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:textDecoration="underline">Underlined</p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("<u>Underlined</u>"));
    }

    #[test]
    fn ttml_to_rich_srt_with_hex_color() {
        let ttml = r##"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:color="#FF0000">Red text</p>
    </div>
  </body>
</tt>"##;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("<font color=\"#FF0000\">Red text</font>"));
    }

    #[test]
    fn ttml_to_rich_srt_with_named_color() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:color="white">White text</p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("<font color=\"#FFFFFF\">White text</font>"));
    }

    #[test]
    fn ttml_to_rich_srt_no_styling() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000">Plain text</p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("Plain text"));
        // No HTML tags when no styling
        assert!(!srt.contains("<b>"));
        assert!(!srt.contains("<i>"));
        assert!(!srt.contains("<u>"));
        assert!(!srt.contains("<font"));
    }

    #[test]
    fn ttml_to_rich_srt_named_styles_in_head() {
        let ttml = r##"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <head>
    <styling>
      <style xml:id="s1" tts:fontWeight="bold" tts:color="#00FF00"/>
    </styling>
  </head>
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" style="s1">Styled text</p>
    </div>
  </body>
</tt>"##;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("<font color=\"#00FF00\"><b>Styled text</b></font>"));
    }

    #[test]
    fn ttml_to_rich_srt_span_with_styling() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000">Normal <span tts:fontWeight="bold">bold part</span></p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("Normal <b>bold part</b>"));
    }

    #[test]
    fn ttml_to_rich_srt_background_vocals() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000">Main <span ttm:role="x-bg">backing</span></p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("Main (backing)"));
    }

    #[test]
    fn ttml_to_rich_srt_empty_body() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div/>
  </body>
</tt>"#;

        let result = ttml_to_rich_srt(ttml);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No timed cues"));
    }

    #[test]
    fn ttml_to_rich_srt_sequential_indices() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:02.000">First</p>
      <p begin="00:00:03.000" end="00:00:04.000">Second</p>
      <p begin="00:00:05.000" end="00:00:06.000">Third</p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.starts_with("1\n"));
        assert!(srt.contains("\n2\n"));
        assert!(srt.contains("\n3\n"));
    }

    #[test]
    fn ttml_to_rich_srt_style_inheritance() {
        // Parent <p> has bold, child <span> should inherit it
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:fontWeight="bold"><span>inherited bold</span></p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("<b>inherited bold</b>"));
    }

    #[test]
    fn ttml_to_rich_srt_span_style_override() {
        // Parent is bold, span adds italic
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:tts="http://www.w3.org/ns/ttml#styling">
  <body>
    <div>
      <p begin="00:00:01.000" end="00:00:03.000" tts:fontWeight="bold"><span tts:fontStyle="italic">bold and italic</span></p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("<b><i>bold and italic</i></b>"));
    }

    #[test]
    fn ttml_to_rich_srt_duration_attribute() {
        // Test <p> with dur instead of end
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:00:10.000" dur="00:00:05.000">Duration based</p>
    </div>
  </body>
</tt>"#;

        let srt = ttml_to_rich_srt(ttml).unwrap();
        assert!(srt.contains("00:00:10,000 --> 00:00:15,000"));
    }
}
