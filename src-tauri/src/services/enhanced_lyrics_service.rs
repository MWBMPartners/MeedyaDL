// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Enhanced LRC lyrics conversion service.
// ========================================
//
// Post-processes Apple Music TTML lyrics files to produce Enhanced LRC
// with word-by-word synchronized timestamps. Apple Music delivers lyrics
// in TTML (Timed Text Markup Language) format, which can include word-level
// timing via `<span begin="" end="">` elements when `itunes:timing="Word"`
// is set on the root `<tt>` element.
//
// GAMDL (the download engine) preserves this word-level data when saving
// raw TTML (`--synced-lyrics-format ttml`), but discards it when converting
// to LRC or SRT. This service bridges that gap by:
//
//   1. Parsing the TTML XML using `roxmltree`
//   2. Detecting whether word-level timing is available
//   3. Converting to Enhanced LRC format with `<mm:ss.xx>` word timestamps
//   4. Writing the result as a `.lrc` sidecar file
//   5. Embedding the Enhanced LRC in the audio file's `©lyr` metadata atom
//
// Enhanced LRC is a backward-compatible extension of standard LRC that adds
// inline word timestamps using angle brackets:
//
//   Standard:  [00:12.45]Midnight rain falls on the window
//   Enhanced:  [00:12.45]<00:12.45>Midnight <00:13.20>rain <00:14.10>falls
//
// Players that support Enhanced LRC (foobar2000, Poweramp, AIMP, Symfonium)
// highlight words one at a time. Standard LRC players simply ignore the
// angle-bracketed timestamps and display lines normally.
//
// ## TTML Format Reference
//
// Apple Music TTML uses two iTunes namespace URIs (both must be accepted):
//   - `http://music.apple.com/lyric-ttml-internal` (API responses)
//   - `http://itunes.apple.com/lyric-ttml-extensions` (provider submissions)
//
// The `itunes:timing` attribute on `<tt>` indicates timing granularity:
//   - `"Word"` -- word-by-word timing in `<span>` elements
//   - `"Line"` -- line-level timing only (on `<p>` elements)
//   - absent/`"None"` -- unsynced lyrics
//
// Background vocals are marked with `ttm:role="x-bg"` on `<span>` elements
// and are conventionally wrapped in parentheses.
//
// ## References
//
// - AMLL TTML Specification: https://github.com/Steve-xmh/amll-ttml-db
// - Enhanced LRC Format: https://en.wikipedia.org/wiki/LRC_(file_format)
// - roxmltree: https://docs.rs/roxmltree/
// - mp4ameta: https://docs.rs/mp4ameta/

use std::path::Path;

use mp4ameta::{Data, Fourcc, Tag};

// ============================================================
// Constants
// ============================================================

/// The two iTunes namespace URIs used in Apple Music TTML files.
/// Both must be checked when looking up the `itunes:timing` attribute.
const ITUNES_NS_INTERNAL: &str = "http://music.apple.com/lyric-ttml-internal";
const ITUNES_NS_EXTENSIONS: &str = "http://itunes.apple.com/lyric-ttml-extensions";

/// TTML namespace (W3C standard).
const TTML_NS: &str = "http://www.w3.org/ns/ttml";

/// TTML metadata namespace (for agent/role attributes).
const TTML_METADATA_NS: &str = "http://www.w3.org/ns/ttml#metadata";

/// The MP4 fourcc for the standard lyrics atom (`©lyr`).
const LYRICS_FOURCC: Fourcc = Fourcc(*b"\xa9lyr");

// ============================================================
// Public types
// ============================================================

/// Result of a TTML → Enhanced LRC conversion.
#[derive(Debug)]
pub struct EnhancedLrcResult {
    /// The Enhanced LRC content string (header + timestamped lyrics).
    pub lrc_content: String,
    /// Whether word-level timing was found and used. When `false`, the
    /// output is standard line-level LRC (still valid, just not "enhanced").
    pub has_word_timing: bool,
}

/// Timing mode detected from the TTML document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimingMode {
    /// Word-by-word timing (`itunes:timing="Word"`)
    Word,
    /// Line-level timing only (`itunes:timing="Line"` or default)
    Line,
    /// No timing information (unsynced lyrics)
    None,
}

/// Metadata extracted from the TTML `<head>` section.
#[derive(Debug, Default)]
struct TtmlMetadata {
    /// Song title from `<ttm:title>`
    title: Option<String>,
    /// Artist name from `<ttm:agent>` with `type="person"`
    artist: Option<String>,
    /// Language code from `xml:lang` on `<tt>`
    language: Option<String>,
    /// Songwriter names from `<iTunesMetadata>/<songwriters>`
    songwriters: Vec<String>,
}

// ============================================================
// Public API
// ============================================================

/// Processes all TTML files in a directory, converting each to Enhanced LRC.
///
/// For each `.ttml` file found:
/// 1. Reads and parses the TTML content
/// 2. Converts to Enhanced LRC (word-level if available, line-level fallback)
/// 3. Writes a `.lrc` sidecar file (same name, different extension)
/// 4. Embeds the Enhanced LRC in the corresponding `.m4a` or `.m4v` file
///
/// Returns the number of files successfully processed.
///
/// **Note:** This function is intentionally `fn` (not `async fn`) because it
/// performs only synchronous file I/O (std::fs, mp4ameta). Callers in the
/// enrichment pipeline wrap it in `tokio::task::spawn_blocking()` to prevent
/// blocking the async runtime on slow filesystems (FUSE mounts, NFS, etc.).
pub fn process_enhanced_lyrics_for_directory(album_dir: &str) -> Result<usize, String> {
    let dir = Path::new(album_dir);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {album_dir}"));
    }

    let mut processed = 0;

    // Collect .ttml files in the directory. Skip filesystem sidecars
    // (macOS AppleDouble `._*.ttml`, etc.) so we don't try to parse
    // resource-fork blobs as TTML XML (#577).
    let entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory: {e}"))?
        .filter_map(Result::ok)
        .filter(|entry| {
            let path = entry.path();
            if crate::utils::fs_safe::is_filesystem_sidecar(&path) {
                return false;
            }
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ttml"))
        })
        .collect();

    for entry in entries {
        let ttml_path = entry.path();
        let stem = match ttml_path.file_stem() {
            Some(s) => s.to_string_lossy().to_string(),
            None => continue,
        };

        // Read the TTML content
        let ttml_content = match std::fs::read_to_string(&ttml_path) {
            Ok(c) => c,
            Err(e) => {
                log::debug!("Failed to read {}: {e}", ttml_path.display());
                continue;
            }
        };

        // Convert TTML → Enhanced LRC
        let result = match ttml_to_enhanced_lrc(&ttml_content) {
            Ok(r) => r,
            Err(e) => {
                log::debug!("Failed to convert {}: {e}", ttml_path.display());
                continue;
            }
        };

        if result.has_word_timing {
            log::info!(
                "Enhanced LRC (word-level) generated for {}",
                ttml_path.display()
            );
        } else {
            log::debug!(
                "Standard LRC (line-level) generated for {} (no word timing in TTML)",
                ttml_path.display()
            );
        }

        // Write the .lrc sidecar file
        let lrc_path = dir.join(format!("{stem}.lrc"));
        if let Err(e) = std::fs::write(&lrc_path, &result.lrc_content) {
            log::debug!("Failed to write {}: {e}", lrc_path.display());
            continue;
        }

        // Embed in corresponding audio/video file (.m4a or .m4v)
        for ext in &["m4a", "m4v", "mp4"] {
            let media_path = dir.join(format!("{stem}.{ext}"));
            if media_path.exists() {
                if let Err(e) = embed_enhanced_lrc_in_file(&media_path, &result.lrc_content) {
                    log::debug!(
                        "Failed to embed Enhanced LRC in {}: {e}",
                        media_path.display()
                    );
                } else {
                    log::debug!("Embedded Enhanced LRC in {}", media_path.display());
                }
                break; // Only embed in the first matching media file
            }
        }

        processed += 1;
    }

    Ok(processed)
}

/// Converts a TTML string to Enhanced LRC format.
///
/// Parses the TTML XML, extracts metadata and timing information, and
/// produces an Enhanced LRC string. If word-level timing is available
/// (`itunes:timing="Word"` and `<span>` elements with `begin` attributes),
/// the output includes inline `<mm:ss.xx>` word timestamps. Otherwise,
/// standard line-level `[mm:ss.xx]` timestamps are used.
pub fn ttml_to_enhanced_lrc(ttml_content: &str) -> Result<EnhancedLrcResult, String> {
    let doc = roxmltree::Document::parse(ttml_content)
        .map_err(|e| format!("Failed to parse TTML XML: {e}"))?;

    let root = doc.root_element();

    // Detect timing mode from itunes:timing attribute
    let timing = detect_timing_mode(&root);

    // Extract metadata from <head> section
    let metadata = extract_ttml_metadata(&doc, &root);

    // Build the LRC output
    let mut lrc = build_lrc_header(&metadata);
    let mut has_word_timing = false;

    // Process all <p> elements (lyric lines) in document order
    for node in doc.descendants() {
        if !is_ttml_element(&node, "p") {
            continue;
        }

        let begin = match node.attribute("begin") {
            Some(b) => b,
            None => continue, // Skip <p> elements without timing
        };

        let line_seconds = match parse_ttml_time(begin) {
            Some(s) => s,
            None => continue,
        };
        let line_ts = format_lrc_time(line_seconds);

        if timing == TimingMode::Word {
            // Try to build an Enhanced LRC line with word-level timestamps
            let spans: Vec<_> = node
                .children()
                .filter(|c| is_ttml_element(c, "span") && c.attribute("begin").is_some())
                .collect();

            if !spans.is_empty() {
                has_word_timing = true;
                let mut line = format!("[{line_ts}]");

                for span in &spans {
                    let word_begin = span.attribute("begin").unwrap_or(begin);
                    let word_seconds = parse_ttml_time(word_begin).unwrap_or(line_seconds);
                    let word_ts = format_lrc_time(word_seconds);

                    let text = collect_node_text(span);
                    if text.is_empty() {
                        continue;
                    }

                    // Check for background vocals (ttm:role="x-bg")
                    let is_bg = span.attribute((TTML_METADATA_NS, "role")) == Some("x-bg");

                    if is_bg && !text.starts_with('(') {
                        line.push_str(&format!("<{word_ts}>({text})"));
                    } else {
                        line.push_str(&format!("<{word_ts}>{text}"));
                    }
                }

                lrc.push_str(&line);
                lrc.push('\n');
                continue;
            }
        }

        // Fallback: line-level timing only
        let text = collect_p_text(&node);
        if !text.is_empty() {
            lrc.push_str(&format!("[{line_ts}]{text}\n"));
        } else {
            // Empty line (instrumental break) — include timestamp only
            lrc.push_str(&format!("[{line_ts}]\n"));
        }
    }

    Ok(EnhancedLrcResult {
        lrc_content: lrc,
        has_word_timing,
    })
}

/// Embeds Enhanced LRC content into an M4A/M4V file's `©lyr` metadata atom.
///
/// Uses the `mp4ameta` crate to read the existing tag, set the lyrics data,
/// and write back. This preserves all other existing metadata (artwork, track
/// info, etc.) while updating only the lyrics atom.
pub fn embed_enhanced_lrc_in_file(file_path: &Path, lrc_content: &str) -> Result<(), String> {
    let mut tag = Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to read M4A tags from {}: {e}", file_path.display()))?;

    tag.set_data(LYRICS_FOURCC, Data::Utf8(lrc_content.to_string()));

    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to write M4A tags to {}: {e}", file_path.display()))?;

    Ok(())
}

// ============================================================
// Internal helpers
// ============================================================

/// Checks whether a node is a TTML element with the given local name.
/// Handles both namespaced (`http://www.w3.org/ns/ttml`) and unnamespaced elements.
fn is_ttml_element(node: &roxmltree::Node, local_name: &str) -> bool {
    if !node.is_element() {
        return false;
    }
    node.tag_name().name() == local_name
        && (node.tag_name().namespace().is_none() || node.tag_name().namespace() == Some(TTML_NS))
}

/// Detects the timing mode from the `itunes:timing` attribute on `<tt>`.
///
/// Checks both known iTunes namespace URIs. Falls back to `Line` if the
/// attribute is missing (most TTML files still have line-level timing).
fn detect_timing_mode(root: &roxmltree::Node) -> TimingMode {
    let timing_value = root
        .attribute((ITUNES_NS_INTERNAL, "timing"))
        .or_else(|| root.attribute((ITUNES_NS_EXTENSIONS, "timing")));

    match timing_value {
        Some(v) if v.eq_ignore_ascii_case("word") => TimingMode::Word,
        Some(v) if v.eq_ignore_ascii_case("none") => TimingMode::None,
        // "Line" is the default when the attribute is present but not "Word"/"None",
        // or when the attribute is missing entirely (most TTML files have line timing).
        _ => TimingMode::Line,
    }
}

/// Extracts metadata from the TTML `<head>` section for the LRC header.
fn extract_ttml_metadata(doc: &roxmltree::Document, root: &roxmltree::Node) -> TtmlMetadata {
    let mut metadata = TtmlMetadata {
        // Language from xml:lang on <tt>
        language: root
            .attribute(("http://www.w3.org/XML/1998/namespace", "lang"))
            .map(|s| s.to_string()),
        ..Default::default()
    };

    // Traverse descendants for metadata elements
    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }

        let local = node.tag_name().name();

        // <ttm:title> — Song title
        if local == "title" && node.tag_name().namespace() == Some(TTML_METADATA_NS) {
            if let Some(text) = node.text() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    metadata.title = Some(trimmed.to_string());
                }
            }
        }

        // <ttm:agent type="person"> → <ttm:name> — Artist name
        if local == "agent"
            && node.tag_name().namespace() == Some(TTML_METADATA_NS)
            && node.attribute("type") == Some("person")
        {
            for child in node.children() {
                if child.is_element()
                    && child.tag_name().name() == "name"
                    && child.tag_name().namespace() == Some(TTML_METADATA_NS)
                {
                    if let Some(name) = child.text() {
                        let trimmed = name.trim();
                        if !trimmed.is_empty() {
                            metadata.artist = Some(trimmed.to_string());
                        }
                    }
                }
            }
        }

        // <songwriter> inside <songwriters> inside <iTunesMetadata>
        if local == "songwriter" {
            if let Some(text) = node.text() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    metadata.songwriters.push(trimmed.to_string());
                }
            }
        }
    }

    metadata
}

/// Builds the LRC metadata header from TTML metadata.
///
/// Produces lines like:
///   [ti:Song Title]
///   [ar:Artist Name]
///   [la:en-US]
///   [au:Songwriter Name]
///   [by:MeedyaDL]
///   [re:MeedyaDL Enhanced LRC]
fn build_lrc_header(metadata: &TtmlMetadata) -> String {
    let mut header = String::new();

    if let Some(ref title) = metadata.title {
        header.push_str(&format!("[ti:{title}]\n"));
    }
    if let Some(ref artist) = metadata.artist {
        header.push_str(&format!("[ar:{artist}]\n"));
    }
    if let Some(ref lang) = metadata.language {
        header.push_str(&format!("[la:{lang}]\n"));
    }
    if !metadata.songwriters.is_empty() {
        // Join multiple songwriters with " / " separator
        let writers = metadata.songwriters.join(" / ");
        header.push_str(&format!("[au:{writers}]\n"));
    }

    header.push_str("[by:MeedyaDL]\n");
    header.push_str("[re:MeedyaDL Enhanced LRC]\n");
    header.push('\n');

    header
}

/// Parses a TTML timestamp string to seconds as `f64`.
///
/// Supported formats:
/// - `HH:MM:SS.fff` (hours, minutes, seconds, fractional)
/// - `MM:SS.fff` (minutes, seconds, fractional)
/// - `SS.fff` (seconds, fractional)
/// - `NNs` (seconds with `s` suffix, e.g., `15.8s`)
///
/// Returns `None` if the timestamp cannot be parsed.
fn parse_ttml_time(time_str: &str) -> Option<f64> {
    let s = time_str.trim();

    // Handle "NNs" format (e.g., "15.8s")
    if let Some(stripped) = s.strip_suffix('s') {
        return stripped.parse::<f64>().ok();
    }

    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        // SS.fff
        1 => parts[0].parse::<f64>().ok(),
        // MM:SS.fff
        2 => {
            let mins: f64 = parts[0].parse().ok()?;
            let secs: f64 = parts[1].parse().ok()?;
            Some(mins * 60.0 + secs)
        }
        // HH:MM:SS.fff
        3 => {
            let hrs: f64 = parts[0].parse().ok()?;
            let mins: f64 = parts[1].parse().ok()?;
            let secs: f64 = parts[2].parse().ok()?;
            Some(hrs * 3600.0 + mins * 60.0 + secs)
        }
        _ => None,
    }
}

/// Formats seconds to LRC timestamp format `mm:ss.xx` (centiseconds).
///
/// LRC minutes can exceed 59 (hours are rolled into minutes).
/// For example, 1 hour 5 minutes 30.25 seconds → `65:30.25`.
fn format_lrc_time(seconds: f64) -> String {
    let total_centiseconds = (seconds * 100.0).round() as u64;
    let minutes = total_centiseconds / 6000;
    let remaining = total_centiseconds % 6000;
    let secs = remaining / 100;
    let centis = remaining % 100;
    format!("{minutes:02}:{secs:02}.{centis:02}")
}

/// Collects all text content from a node and its children (recursively).
///
/// Used for `<span>` elements that may contain nested text nodes or
/// child elements. Preserves whitespace as-is from the TTML source.
fn collect_node_text(node: &roxmltree::Node) -> String {
    let mut text = String::new();
    for child in node.children() {
        if child.is_text() {
            text.push_str(child.text().unwrap_or(""));
        } else if child.is_element() {
            // Recurse into nested elements (e.g., nested <span>)
            text.push_str(&collect_node_text(&child));
        }
    }
    text
}

/// Collects the full text content of a `<p>` element.
///
/// Gathers text from direct text nodes and all `<span>` children,
/// producing the plain-text lyric line (without timing).
fn collect_p_text(node: &roxmltree::Node) -> String {
    let mut text = String::new();
    for child in node.children() {
        if child.is_text() {
            text.push_str(child.text().unwrap_or(""));
        } else if child.is_element() {
            text.push_str(&collect_node_text(&child));
        }
    }
    text.trim().to_string()
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // parse_ttml_time
    // ----------------------------------------------------------

    #[test]
    fn parse_time_hh_mm_ss_fff() {
        let result = parse_ttml_time("01:02:03.456");
        assert!((result.unwrap() - 3723.456).abs() < 0.001);
    }

    #[test]
    fn parse_time_mm_ss_fff() {
        let result = parse_ttml_time("02:30.500");
        assert!((result.unwrap() - 150.5).abs() < 0.001);
    }

    #[test]
    fn parse_time_ss_fff() {
        let result = parse_ttml_time("15.800");
        assert!((result.unwrap() - 15.8).abs() < 0.001);
    }

    #[test]
    fn parse_time_with_s_suffix() {
        let result = parse_ttml_time("15.8s");
        assert!((result.unwrap() - 15.8).abs() < 0.001);
    }

    #[test]
    fn parse_time_zero() {
        let result = parse_ttml_time("00:00.000");
        assert!((result.unwrap()).abs() < 0.001);
    }

    #[test]
    fn parse_time_invalid() {
        assert!(parse_ttml_time("invalid").is_none());
    }

    // ----------------------------------------------------------
    // format_lrc_time
    // ----------------------------------------------------------

    #[test]
    fn format_time_standard() {
        assert_eq!(format_lrc_time(150.5), "02:30.50");
    }

    #[test]
    fn format_time_zero() {
        assert_eq!(format_lrc_time(0.0), "00:00.00");
    }

    #[test]
    fn format_time_over_one_hour() {
        // 1 hour 5 minutes 30.25 seconds = 65 minutes 30.25 seconds
        assert_eq!(format_lrc_time(3930.25), "65:30.25");
    }

    #[test]
    fn format_time_centiseconds() {
        assert_eq!(format_lrc_time(12.45), "00:12.45");
    }

    // ----------------------------------------------------------
    // ttml_to_enhanced_lrc — word-level timing
    // ----------------------------------------------------------

    #[test]
    fn convert_word_level_ttml() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:ttm="http://www.w3.org/ns/ttml#metadata"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Word" xml:lang="en-US">
          <head>
            <metadata>
              <ttm:title>Test Song</ttm:title>
            </metadata>
          </head>
          <body>
            <div>
              <p begin="00:12.450" end="00:15.800">
                <span begin="00:12.450" end="00:13.200">Hello </span>
                <span begin="00:13.200" end="00:14.100">world </span>
                <span begin="00:14.100" end="00:15.800">today</span>
              </p>
            </div>
          </body>
        </tt>"#;

        let result = ttml_to_enhanced_lrc(ttml).unwrap();
        assert!(result.has_word_timing);
        assert!(result.lrc_content.contains("[ti:Test Song]"));
        assert!(result.lrc_content.contains("[la:en-US]"));
        assert!(result.lrc_content.contains("[by:MeedyaDL]"));
        // Check Enhanced LRC line format: [line_ts]<word_ts>word <word_ts>word
        assert!(result
            .lrc_content
            .contains("[00:12.45]<00:12.45>Hello <00:13.20>world <00:14.10>today"));
    }

    #[test]
    fn convert_line_level_ttml() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Line">
          <body>
            <div>
              <p begin="00:12.450" end="00:15.800">Hello world today</p>
            </div>
          </body>
        </tt>"#;

        let result = ttml_to_enhanced_lrc(ttml).unwrap();
        assert!(!result.has_word_timing);
        assert!(result.lrc_content.contains("[00:12.45]Hello world today"));
    }

    #[test]
    fn convert_background_vocals() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:ttm="http://www.w3.org/ns/ttml#metadata"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Word">
          <body>
            <div>
              <p begin="00:10.000" end="00:15.000">
                <span begin="00:10.000" end="00:11.000">Main </span>
                <span ttm:role="x-bg" begin="00:11.000" end="00:12.000">oh oh</span>
              </p>
            </div>
          </body>
        </tt>"#;

        let result = ttml_to_enhanced_lrc(ttml).unwrap();
        assert!(result.has_word_timing);
        // Background vocals should be wrapped in parentheses
        assert!(result.lrc_content.contains("<00:11.00>(oh oh)"));
    }

    #[test]
    fn convert_background_vocals_already_parenthesized() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:ttm="http://www.w3.org/ns/ttml#metadata"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Word">
          <body>
            <div>
              <p begin="00:10.000" end="00:15.000">
                <span begin="00:10.000" end="00:11.000">Main </span>
                <span ttm:role="x-bg" begin="00:11.000" end="00:12.000">(oh oh)</span>
              </p>
            </div>
          </body>
        </tt>"#;

        let result = ttml_to_enhanced_lrc(ttml).unwrap();
        // Should NOT double-parenthesize
        assert!(result.lrc_content.contains("<00:11.00>(oh oh)"));
        assert!(!result.lrc_content.contains("((oh oh))"));
    }

    #[test]
    fn convert_mixed_timing() {
        // Some <p> elements with <span> timing, some without
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Word">
          <body>
            <div>
              <p begin="00:05.000" end="00:08.000">Line without spans</p>
              <p begin="00:10.000" end="00:13.000">
                <span begin="00:10.000" end="00:11.500">Word </span>
                <span begin="00:11.500" end="00:13.000">level</span>
              </p>
            </div>
          </body>
        </tt>"#;

        let result = ttml_to_enhanced_lrc(ttml).unwrap();
        assert!(result.has_word_timing);
        // Line without spans should be standard LRC
        assert!(result.lrc_content.contains("[00:05.00]Line without spans"));
        // Line with spans should be Enhanced LRC
        assert!(result
            .lrc_content
            .contains("[00:10.00]<00:10.00>Word <00:11.50>level"));
    }

    #[test]
    fn convert_alternate_itunes_namespace() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:itunes="http://itunes.apple.com/lyric-ttml-extensions"
            itunes:timing="Word">
          <body>
            <div>
              <p begin="00:01.000" end="00:03.000">
                <span begin="00:01.000" end="00:02.000">Test </span>
                <span begin="00:02.000" end="00:03.000">text</span>
              </p>
            </div>
          </body>
        </tt>"#;

        let result = ttml_to_enhanced_lrc(ttml).unwrap();
        assert!(result.has_word_timing);
    }

    #[test]
    fn convert_empty_p_element() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Line">
          <body>
            <div>
              <p begin="00:30.000" end="00:35.000"></p>
            </div>
          </body>
        </tt>"#;

        let result = ttml_to_enhanced_lrc(ttml).unwrap();
        // Empty <p> should produce an empty timestamped line
        assert!(result.lrc_content.contains("[00:30.00]\n"));
    }

    #[test]
    fn header_includes_songwriter() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:ttm="http://www.w3.org/ns/ttml#metadata"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Line">
          <head>
            <metadata>
              <ttm:title>My Song</ttm:title>
              <iTunesMetadata xmlns="http://music.apple.com/lyric-ttml-internal">
                <songwriters>
                  <songwriter>Writer One</songwriter>
                  <songwriter>Writer Two</songwriter>
                </songwriters>
              </iTunesMetadata>
            </metadata>
          </head>
          <body>
            <div>
              <p begin="00:01.000" end="00:03.000">Lyrics here</p>
            </div>
          </body>
        </tt>"#;

        let result = ttml_to_enhanced_lrc(ttml).unwrap();
        assert!(result.lrc_content.contains("[ti:My Song]"));
        assert!(result.lrc_content.contains("[au:Writer One / Writer Two]"));
    }

    #[test]
    fn invalid_ttml_returns_error() {
        let result = ttml_to_enhanced_lrc("not valid xml <><>");
        assert!(result.is_err());
    }

    // ----------------------------------------------------------
    // Artist extraction from <ttm:agent>
    // ----------------------------------------------------------

    #[test]
    fn extract_artist_from_agent() {
        let ttml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <tt xmlns="http://www.w3.org/ns/ttml"
            xmlns:ttm="http://www.w3.org/ns/ttml#metadata"
            xmlns:itunes="http://music.apple.com/lyric-ttml-internal"
            itunes:timing="Line">
          <head>
            <metadata>
              <ttm:agent xml:id="v1" type="person">
                <ttm:name type="full">Taylor Swift</ttm:name>
              </ttm:agent>
            </metadata>
          </head>
          <body>
            <div>
              <p begin="00:01.000" end="00:03.000">Lyrics</p>
            </div>
          </body>
        </tt>"#;

        let result = ttml_to_enhanced_lrc(ttml).unwrap();
        assert!(result.lrc_content.contains("[ar:Taylor Swift]"));
    }
}
