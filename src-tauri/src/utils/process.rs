// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Subprocess output parsing utilities.
// ======================================
//
// When GAMDL runs as a subprocess, it (and its internal yt-dlp dependency)
// writes progress and status information to stdout/stderr in various
// human-readable formats. This module parses those raw text lines into
// structured `GamdlOutputEvent` values that the React frontend can
// consume to render:
//   - A progress bar with percentage, speed, and ETA
//   - Track title/artist information
//   - Post-processing step names (Remuxing, Tagging, etc.)
//   - Error messages with classification (auth, network, codec, etc.)
//   - Download completion with the output file path
//
// The parsing is regex-based. Each regex is compiled **once** using
// `std::sync::LazyLock` (stabilised in Rust 1.80) and reused for every
// line, amortising the compilation cost across the application's lifetime.
//
// Data flow:
//   GAMDL subprocess stdout/stderr
//     -> `services::gamdl_service` reads each line
//     -> `parse_gamdl_output(line)` returns a `GamdlOutputEvent`
//     -> event is serialised as JSON and emitted to the frontend via
//        Tauri's event system (`window.emit("gamdl-output", event)`)
//     -> React `useEffect` listener updates the download queue UI
//
// Reference: https://docs.rs/regex/latest/regex/
// Reference: https://v2.tauri.app/develop/calling-rust/#events
// Reference: https://doc.rust-lang.org/std/sync/struct.LazyLock.html

use regex::Regex;
// `Serialize` is needed because `GamdlOutputEvent` is sent over Tauri's
// IPC as JSON. The `#[serde(tag = "type")]` attribute makes the JSON
// output an externally tagged enum: `{ "type": "download_progress", ... }`.
// Reference: https://serde.rs/enum-representations.html
use serde::Serialize;
// `LazyLock` is a thread-safe lazy initialisation primitive. The value
// is computed on first access and then cached for all subsequent accesses.
// Unlike `lazy_static!`, it is part of the standard library (since 1.80).
// Reference: https://doc.rust-lang.org/std/sync/struct.LazyLock.html
use std::sync::LazyLock;

// ============================================================
// Compiled regex patterns (initialised once via LazyLock, reused for
// every line of GAMDL output throughout the application's lifetime)
// ============================================================
//
// Each `static LazyLock<Regex>` compiles the regex on first access.
// Subsequent calls to `.captures()` or `.is_match()` use the compiled
// automaton directly, making per-line matching very fast.
//
// Reference: https://docs.rs/regex/latest/regex/struct.Regex.html

/// Matches yt-dlp-style download progress output.
///
/// Capture groups:
///   1. `percent`  -- e.g. "45.2"
///   2. `size`     -- e.g. "5.12MiB" (total or estimated with ~)
///   3. `speed`    -- e.g. "2.51MiB/s"
///   4. `eta`      -- e.g. "00:01"
///
/// Example input: `[download]  45.2% of ~  5.12MiB at  2.51MiB/s ETA 00:01`
///
/// The `~?` makes the tilde optional (yt-dlp uses `~` for estimated sizes).
/// `\S+` matches any non-whitespace sequence, which is flexible enough to
/// handle varying size/speed/time formats.
static PROGRESS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\[download\]\s+(\d+\.?\d*)%\s+of\s+~?\s*(\S+)\s+at\s+(\S+)\s+ETA\s+(\S+)",
    )
    .expect("Invalid progress regex")
});

/// Matches yt-dlp-style download completion output (100% reached).
///
/// Capture groups:
///   1. `size`     -- e.g. "5.12MiB" (final size)
///   2. `duration` -- e.g. "00:02" (total download time)
///
/// Example input: `[download] 100% of 5.12MiB in 00:02`
///
/// This is a separate pattern from `PROGRESS_REGEX` because the 100%
/// completion line uses "in" instead of "at ... ETA ..." syntax.
static PROGRESS_COMPLETE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[download\]\s+100%\s+of\s+(\S+)\s+in\s+(\S+)")
        .expect("Invalid progress complete regex")
});

/// Matches GAMDL track information lines.
///
/// Capture groups:
///   1. `type`  -- either "song" or "track N of M" (e.g. "track 3 of 12")
///   2. `info`  -- the rest of the line (title, possibly "Title by Artist")
///
/// Example inputs:
///   - `Getting song: Song Title by Artist Name`
///   - `Getting track 3 of 12: Song Title`
///
/// The alternation `(song|track\s+\d+\s+of\s+\d+)` handles both
/// single-track and album-track formats that GAMDL outputs.
static TRACK_INFO_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Getting\s+(song|track\s+\d+\s+of\s+\d+):\s+(.+)")
        .expect("Invalid track info regex")
});

/// Matches GAMDL "Saved to" completion lines.
///
/// Capture groups:
///   1. `path` -- the output file path (e.g. "/path/to/output/file.m4a")
///
/// Example input: `Saved to: /path/to/output/file.m4a`
///
/// The `(?i)` flag makes the match case-insensitive ("Saved", "saved",
/// "SAVED" all match). The `:?` makes the colon optional to handle
/// minor formatting variations across GAMDL versions.
static SAVED_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)saved\s+to:?\s+(.+)").expect("Invalid saved regex")
});

/// Matches lines containing explicit error indicators at the start.
///
/// Capture groups:
///   1. `message` -- the error message text after the prefix
///
/// Example inputs:
///   - `ERROR: Unable to download webpage`
///   - `Error: cookies file not found`
///   - `error: network timeout`
///
/// The `(?i)` flag makes the match case-insensitive. The `^` anchor
/// ensures this only matches errors at the start of a line (not
/// "no error occurred" mid-line). The `:?` makes the colon optional.
static ERROR_PREFIX_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:ERROR|error|Error):?\s+(.+)").expect("Invalid error regex")
});

// ============================================================
// Event types emitted to the frontend
// ============================================================
//
// These events cross the Rust -> TypeScript boundary via Tauri's event
// system. The `Serialize` derive generates JSON like:
//   { "type": "download_progress", "percent": 45.2, "speed": "2.51MiB/s", "eta": "00:01" }
//
// The `#[serde(tag = "type")]` attribute uses "internally tagged" enum
// representation: the discriminant becomes a `"type"` field in the JSON
// object, and the variant fields are flattened into the same object.
// The `rename_all = "snake_case"` converts PascalCase variant names to
// snake_case (e.g., `DownloadProgress` -> `"download_progress"`).
//
// Reference: https://serde.rs/enum-representations.html#internally-tagged

/// A structured event parsed from a single line of GAMDL's stdout/stderr
/// output. The frontend listens for these events (via `listen("gamdl-output")`)
/// to update the download progress UI in real time.
///
/// Each variant corresponds to a different kind of output line. The parser
/// ([`parse_gamdl_output`]) tries patterns in priority order and returns
/// the first match, or `Unknown` if no pattern matches.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GamdlOutputEvent {
    /// Information about the track currently being processed
    TrackInfo {
        /// Track title (or the full info string if title/artist can't be separated)
        title: String,
        /// Artist name (empty string if not parsed)
        artist: String,
        /// Album name (empty string if not parsed - album info often comes separately)
        album: String,
    },

    /// Download progress update from yt-dlp's output
    DownloadProgress {
        /// Progress percentage (0.0 to 100.0)
        percent: f64,
        /// Current download speed (e.g., "2.51MiB/s")
        speed: String,
        /// Estimated time remaining (e.g., "00:01")
        eta: String,
    },

    /// A post-download processing step (remuxing, tagging, etc.)
    ProcessingStep {
        /// Description of the current step (e.g., "Remuxing to M4A")
        step: String,
    },

    /// An error occurred during the download
    Error {
        /// Error message from GAMDL or its subprocesses
        message: String,
    },

    /// Download completed successfully for a track/file
    Complete {
        /// Path to the output file (if available)
        path: String,
    },

    /// Unrecognized output line (included for debugging/logging in the UI)
    Unknown {
        /// The raw output line that couldn't be categorized
        raw: String,
    },
}

/// Parses a single line of GAMDL output into a structured event.
///
/// GAMDL and its subprocesses (yt-dlp, FFmpeg) output progress and status
/// information in various formats. This parser applies regex patterns in
/// priority order to categorize each line:
///
/// 1. Download progress (yt-dlp format)
/// 2. Download completion (yt-dlp format)
/// 3. Track information (GAMDL "Getting song/track" lines)
/// 4. Explicit errors (ERROR/Error prefix)
/// 5. Post-processing steps (Remuxing/Tagging/Embedding)
/// 6. File save completion (Saved to ...)
/// 7. Common error patterns (case-insensitive "failed", "not found", etc.)
/// 8. Unknown (everything else)
///
/// # Arguments
/// * `line` - A single line from GAMDL's stdout or stderr
///
/// # Returns
/// A `GamdlOutputEvent` representing the parsed content of the line.
pub fn parse_gamdl_output(line: &str) -> GamdlOutputEvent {
    let trimmed = line.trim();

    // Skip empty lines
    if trimmed.is_empty() {
        return GamdlOutputEvent::Unknown {
            raw: String::new(),
        };
    }

    // Priority 1: yt-dlp download progress (most frequent during downloads).
    // Checked first because during an active download, the vast majority of
    // output lines are progress updates. Matching this first avoids running
    // all other regex patterns on every progress line.
    if let Some(captures) = PROGRESS_REGEX.captures(trimmed) {
        // Extract capture group 1 (percent) and parse as f64.
        // `.and_then()` chains the Option: if the group exists, try parsing.
        // Falls back to 0.0 if the group is missing or unparseable.
        let percent = captures
            .get(1)
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .unwrap_or(0.0);
        // Capture group 3 = download speed (e.g. "2.51MiB/s")
        let speed = captures
            .get(3)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        // Capture group 4 = estimated time remaining (e.g. "00:01")
        let eta = captures
            .get(4)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        return GamdlOutputEvent::DownloadProgress {
            percent,
            speed,
            eta,
        };
    }

    // Priority 2: yt-dlp download completion (100%)
    if PROGRESS_COMPLETE_REGEX.is_match(trimmed) {
        return GamdlOutputEvent::DownloadProgress {
            percent: 100.0,
            speed: String::new(),
            eta: "00:00".to_string(),
        };
    }

    // Priority 3: Track information from GAMDL.
    // When GAMDL starts processing a new track, it prints a line like
    // "Getting song: Title by Artist" or "Getting track 3 of 12: Title".
    if let Some(captures) = TRACK_INFO_REGEX.captures(trimmed) {
        // Capture group 2 contains the info string after the colon.
        let info = captures
            .get(2)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        // Attempt to split "Title by Artist" using the **last** occurrence
        // of " by " (via `rfind`). Using the last occurrence handles cases
        // where the title itself contains " by " (e.g. "Stand by Me by
        // Ben E. King"). If no " by " separator is found, the entire info
        // string is treated as the title with an empty artist.
        let (title, artist) = if let Some(idx) = info.rfind(" by ") {
            (info[..idx].to_string(), info[idx + 4..].to_string())
        } else {
            (info, String::new())
        };

        return GamdlOutputEvent::TrackInfo {
            title,
            artist,
            // Album info typically comes from a separate GAMDL output line
            // and is not available in the "Getting song/track" line.
            album: String::new(),
        };
    }

    // Priority 4: Explicit error messages with ERROR/Error prefix
    if let Some(captures) = ERROR_PREFIX_REGEX.captures(trimmed) {
        let message = captures
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| trimmed.to_string());
        return GamdlOutputEvent::Error { message };
    }

    // Priority 5: Post-processing steps (remuxing, tagging, embedding artwork).
    // After the raw download completes, GAMDL runs post-processing steps:
    //   - Remuxing:   converting container format (e.g. WebM -> M4A)
    //   - Tagging:    writing ID3/MP4 metadata tags
    //   - Embedding:  adding album artwork to the output file
    //   - Applying:   applying ReplayGain or other audio adjustments
    //   - Converting: converting between audio codecs
    //   - Decrypting: decrypting DRM-protected streams via mp4decrypt
    // These are matched by simple prefix checks (no regex needed) since
    // GAMDL always starts these lines with the step name.
    if trimmed.starts_with("Remuxing")
        || trimmed.starts_with("Tagging")
        || trimmed.starts_with("Embedding")
        || trimmed.starts_with("Applying")
        || trimmed.starts_with("Converting")
        || trimmed.starts_with("Decrypting")
    {
        return GamdlOutputEvent::ProcessingStep {
            step: trimmed.to_string(),
        };
    }

    // Priority 6: File save completion
    if let Some(captures) = SAVED_REGEX.captures(trimmed) {
        let path = captures
            .get(1)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        return GamdlOutputEvent::Complete { path };
    }

    // Priority 7: Common error patterns detected by keyword matching.
    // These catch errors that don't have an explicit "ERROR:" prefix but
    // contain well-known error indicators. The lowercase conversion ensures
    // case-insensitive matching without regex overhead.
    //
    // Keywords:
    //   - "failed"           -- generic failure messages from any tool
    //   - "not found"        -- missing files, URLs, or resources
    //   - "permission denied"-- filesystem permission errors
    //   - "codec not available" -- requested audio/video codec not offered
    //   - "no entry"         -- missing archive entries or config keys
    //   - "traceback"        -- Python stack traces from GAMDL/yt-dlp
    //   - "exception"        -- Python exception messages
    let lower = trimmed.to_lowercase();
    if lower.contains("failed")
        || lower.contains("not found")
        || lower.contains("permission denied")
        || lower.contains("codec not available")
        || lower.contains("no entry")
        || lower.contains("traceback")
        || lower.contains("exception")
    {
        return GamdlOutputEvent::Error {
            message: trimmed.to_string(),
        };
    }

    // Default: unrecognized output line
    GamdlOutputEvent::Unknown {
        raw: trimmed.to_string(),
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // parse_gamdl_output: Progress events
    // ----------------------------------------------------------

    #[test]
    fn parses_ytdlp_progress_line() {
        let line = "[download]  45.2% of ~  5.12MiB at  2.51MiB/s ETA 00:01";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::DownloadProgress {
                percent,
                speed,
                eta,
            } => {
                assert!((percent - 45.2).abs() < 0.01);
                assert_eq!(speed, "2.51MiB/s");
                assert_eq!(eta, "00:01");
            }
            other => panic!("Expected DownloadProgress, got {:?}", other),
        }
    }

    #[test]
    fn parses_ytdlp_progress_without_tilde() {
        let line = "[download]  78.0% of 12.34MiB at 5.00MiB/s ETA 00:03";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::DownloadProgress { percent, .. } => {
                assert!((percent - 78.0).abs() < 0.01);
            }
            other => panic!("Expected DownloadProgress, got {:?}", other),
        }
    }

    #[test]
    fn parses_ytdlp_100_percent_completion() {
        let line = "[download] 100% of 5.12MiB in 00:02";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::DownloadProgress { percent, eta, .. } => {
                assert!((percent - 100.0).abs() < 0.01);
                assert_eq!(eta, "00:00");
            }
            other => panic!("Expected DownloadProgress, got {:?}", other),
        }
    }

    // ----------------------------------------------------------
    // parse_gamdl_output: Track info
    // ----------------------------------------------------------

    #[test]
    fn parses_song_track_info_with_artist() {
        let line = "Getting song: Anti-Hero by Taylor Swift";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo { title, artist, .. } => {
                assert_eq!(title, "Anti-Hero");
                assert_eq!(artist, "Taylor Swift");
            }
            other => panic!("Expected TrackInfo, got {:?}", other),
        }
    }

    #[test]
    fn parses_track_info_without_artist() {
        let line = "Getting song: Bohemian Rhapsody";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo { title, artist, .. } => {
                assert_eq!(title, "Bohemian Rhapsody");
                assert_eq!(artist, "");
            }
            other => panic!("Expected TrackInfo, got {:?}", other),
        }
    }

    #[test]
    fn parses_numbered_track_info() {
        let line = "Getting track 3 of 12: Song Title by Artist";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo { title, artist, .. } => {
                assert_eq!(title, "Song Title");
                assert_eq!(artist, "Artist");
            }
            other => panic!("Expected TrackInfo, got {:?}", other),
        }
    }

    #[test]
    fn handles_title_containing_by() {
        // "Stand by Me by Ben E. King" -- the last "by" is the separator
        let line = "Getting song: Stand by Me by Ben E. King";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo { title, artist, .. } => {
                assert_eq!(title, "Stand by Me");
                assert_eq!(artist, "Ben E. King");
            }
            other => panic!("Expected TrackInfo, got {:?}", other),
        }
    }

    // ----------------------------------------------------------
    // parse_gamdl_output: Error detection
    // ----------------------------------------------------------

    #[test]
    fn parses_error_prefix() {
        let line = "ERROR: Unable to download webpage";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert_eq!(message, "Unable to download webpage");
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn parses_error_case_insensitive() {
        let line = "error: something went wrong";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert_eq!(message, "something went wrong");
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn parses_keyword_error_failed() {
        let line = "Download failed for track 5";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert_eq!(message, "Download failed for track 5");
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn parses_keyword_error_traceback() {
        let line = "Traceback (most recent call last):";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(message.contains("Traceback"));
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    // ----------------------------------------------------------
    // parse_gamdl_output: Processing steps
    // ----------------------------------------------------------

    #[test]
    fn parses_remuxing_step() {
        let line = "Remuxing to M4A";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::ProcessingStep { step } => {
                assert_eq!(step, "Remuxing to M4A");
            }
            other => panic!("Expected ProcessingStep, got {:?}", other),
        }
    }

    #[test]
    fn parses_tagging_step() {
        let line = "Tagging track 5 of 12";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::ProcessingStep { step } => {
                assert!(step.starts_with("Tagging"));
            }
            other => panic!("Expected ProcessingStep, got {:?}", other),
        }
    }

    #[test]
    fn parses_decrypting_step() {
        let line = "Decrypting with mp4decrypt";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::ProcessingStep { step } => {
                assert!(step.starts_with("Decrypting"));
            }
            other => panic!("Expected ProcessingStep, got {:?}", other),
        }
    }

    // ----------------------------------------------------------
    // parse_gamdl_output: Completion
    // ----------------------------------------------------------

    #[test]
    fn parses_saved_to_path() {
        let line = "Saved to: /path/to/output/song.m4a";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Complete { path } => {
                assert_eq!(path, "/path/to/output/song.m4a");
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    #[test]
    fn parses_saved_to_case_insensitive() {
        let line = "SAVED TO /another/path.m4a";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Complete { path } => {
                assert_eq!(path, "/another/path.m4a");
            }
            other => panic!("Expected Complete, got {:?}", other),
        }
    }

    // ----------------------------------------------------------
    // parse_gamdl_output: Unknown
    // ----------------------------------------------------------

    #[test]
    fn returns_unknown_for_unrecognized_line() {
        let line = "Some random log output that doesn't match anything";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Unknown { raw } => {
                assert_eq!(raw, line);
            }
            other => panic!("Expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn returns_unknown_for_empty_line() {
        match parse_gamdl_output("") {
            GamdlOutputEvent::Unknown { raw } => {
                assert_eq!(raw, "");
            }
            other => panic!("Expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn trims_whitespace_before_parsing() {
        let line = "  Remuxing to M4A  ";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::ProcessingStep { step } => {
                assert_eq!(step, "Remuxing to M4A");
            }
            other => panic!("Expected ProcessingStep, got {:?}", other),
        }
    }

    // ----------------------------------------------------------
    // is_codec_error
    // ----------------------------------------------------------

    #[test]
    fn detects_codec_not_available() {
        assert!(is_codec_error("Codec not available for this track"));
    }

    #[test]
    fn detects_no_matching_codec() {
        assert!(is_codec_error("No matching codec found"));
    }

    #[test]
    fn detects_format_not_available() {
        assert!(is_codec_error("Format not available: alac"));
    }

    #[test]
    fn detects_drm_error() {
        assert!(is_codec_error("DRM protected content cannot be processed"));
    }

    #[test]
    fn does_not_detect_network_error_as_codec() {
        assert!(!is_codec_error("Network timeout occurred"));
    }

    #[test]
    fn does_not_detect_auth_error_as_codec() {
        assert!(!is_codec_error("Cookie authentication failed"));
    }

    // ----------------------------------------------------------
    // classify_error
    // ----------------------------------------------------------

    #[test]
    fn classifies_auth_errors() {
        assert_eq!(classify_error("Cookie file expired"), "auth");
        assert_eq!(classify_error("Authentication failed"), "auth");
        assert_eq!(classify_error("Login required"), "auth");
    }

    #[test]
    fn classifies_network_errors() {
        assert_eq!(classify_error("Network timeout"), "network");
        assert_eq!(classify_error("Connection refused"), "network");
        assert_eq!(classify_error("DNS resolution failed"), "network");
    }

    #[test]
    fn classifies_codec_errors() {
        assert_eq!(classify_error("Codec not available"), "codec");
        assert_eq!(classify_error("No matching codec"), "codec");
    }

    #[test]
    fn classifies_not_found_errors() {
        assert_eq!(classify_error("Resource not found"), "not_found");
        assert_eq!(classify_error("HTTP 404 error"), "not_found");
    }

    #[test]
    fn classifies_rate_limit_errors() {
        assert_eq!(classify_error("Rate limit exceeded"), "rate_limit");
        assert_eq!(classify_error("HTTP 429 too many requests"), "rate_limit");
    }

    #[test]
    fn classifies_tool_errors() {
        assert_eq!(classify_error("FFmpeg process crashed"), "tool");
        assert_eq!(classify_error("mp4decrypt returned error"), "tool");
    }

    #[test]
    fn classifies_unknown_errors() {
        assert_eq!(classify_error("Something completely unexpected"), "unknown");
    }
}

/// Checks if a GAMDL error message indicates a codec-related failure.
///
/// This is used by the **fallback quality system** in `services::download_queue`
/// to decide whether to retry the download with a different audio codec or
/// video resolution. The quality fallback chain is:
///   AAC-HE -> AAC-LC -> (give up)   for audio
///   2160p  -> 1080p  -> 720p        for video (music videos)
///
/// Codec errors mean the content is not available in the requested format
/// on the server side, so retrying with the same format would fail again.
/// Other error types (network, auth, not-found) are transient or
/// configuration issues and should **not** trigger codec fallback.
///
/// # Arguments
/// * `error_message` - The error message string to classify.
///
/// # Returns
/// `true` if the error is codec-related and a quality fallback should be
/// attempted; `false` otherwise.
///
/// # Connection
/// Called by `services::download_queue` after a download fails, before
/// deciding whether to enqueue a retry with a lower-quality codec.
pub fn is_codec_error(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    lower.contains("codec not available")      // yt-dlp: requested codec not in manifest
        || lower.contains("no matching codec") // GAMDL: no codec matches quality preference
        || lower.contains("format not available") // yt-dlp: requested format ID not found
        || lower.contains("unable to find matching codec") // GAMDL variant
        || lower.contains("requested codec")   // GAMDL: "requested codec X not available"
        || lower.contains("drm")               // DRM-protected content (cannot be decoded)
}

/// Classifies an error message into a named category for the React UI.
///
/// Error categories serve two purposes:
///   1. **Visual feedback** -- the React download queue component uses the
///      category to select an icon, colour, and user-friendly description.
///   2. **Retry logic** -- the download queue manager checks the category
///      to decide whether automatic retry or quality fallback is appropriate
///      (e.g., "auth" errors should not be retried automatically, but
///      "network" errors might be).
///
/// Categories are returned as `&'static str` (compile-time string literals)
/// to avoid heap allocation. The frontend matches on these exact strings.
///
/// # Category mapping
/// | Category       | Keywords matched                          | Retry? |
/// |----------------|-------------------------------------------|--------|
/// | `"auth"`       | cookie, auth, login                       | No     |
/// | `"network"`    | network, timeout, connection, dns         | Yes    |
/// | `"codec"`      | (delegated to `is_codec_error`)           | Fallback|
/// | `"not_found"`  | not found, 404, no results                | No     |
/// | `"rate_limit"` | rate limit, 429, too many                 | Delayed|
/// | `"tool"`       | ffmpeg, mp4decrypt, mp4box, nm3u8dl       | No     |
/// | `"unknown"`    | (default)                                 | No     |
///
/// # Arguments
/// * `error_message` - The error message string to classify.
///
/// # Returns
/// A `&'static str` category identifier.
///
/// # Connection
/// Called by `services::download_queue` and `commands::gamdl` when reporting
/// errors to the frontend.
pub fn classify_error(error_message: &str) -> &'static str {
    let lower = error_message.to_lowercase();

    // Authentication / cookie errors: user needs to provide valid credentials.
    if lower.contains("cookie") || lower.contains("auth") || lower.contains("login") {
        "auth"
    // Network errors: transient, may resolve on retry.
    } else if lower.contains("network")
        || lower.contains("timeout")
        || lower.contains("connection")
        || lower.contains("dns")
    {
        "network"
    // Codec/format errors: the requested quality is not available; try fallback.
    } else if is_codec_error(error_message) {
        "codec"
    // Content not found: the URL is invalid or the content was removed.
    } else if lower.contains("not found") || lower.contains("404") || lower.contains("no results")
    {
        "not_found"
    // Rate limiting: the server is throttling requests; retry after delay.
    } else if lower.contains("rate limit") || lower.contains("429") || lower.contains("too many")
    {
        "rate_limit"
    // External tool errors: FFmpeg, mp4decrypt, etc. failed during post-processing.
    } else if lower.contains("ffmpeg")
        || lower.contains("mp4decrypt")
        || lower.contains("mp4box")
        || lower.contains("nm3u8dl")
    {
        "tool"
    // Default: unclassified error.
    } else {
        "unknown"
    }
}
