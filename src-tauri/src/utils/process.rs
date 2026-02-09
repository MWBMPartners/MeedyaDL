// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Subprocess output parsing utilities.
// Parses GAMDL's stdout/stderr output into structured events that the
// frontend can display as progress bars, track info, and status messages.
// Uses regex patterns to match yt-dlp download progress, GAMDL track
// information, post-processing steps, and error messages.

use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

// ============================================================
// Compiled regex patterns (initialized once, reused for every line)
// ============================================================

/// Matches yt-dlp-style download progress output.
/// Example: "[download]  45.2% of ~  5.12MiB at  2.51MiB/s ETA 00:01"
static PROGRESS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\[download\]\s+(\d+\.?\d*)%\s+of\s+~?\s*(\S+)\s+at\s+(\S+)\s+ETA\s+(\S+)",
    )
    .expect("Invalid progress regex")
});

/// Matches yt-dlp-style download completion output.
/// Example: "[download] 100% of 5.12MiB in 00:02"
static PROGRESS_COMPLETE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[download\]\s+100%\s+of\s+(\S+)\s+in\s+(\S+)")
        .expect("Invalid progress complete regex")
});

/// Matches GAMDL track information lines.
/// Example: "Getting song: Song Title by Artist Name"
/// Example: "Getting track 3 of 12: Song Title"
static TRACK_INFO_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Getting\s+(song|track\s+\d+\s+of\s+\d+):\s+(.+)")
        .expect("Invalid track info regex")
});

/// Matches GAMDL "Saved to" completion lines.
/// Example: "Saved to: /path/to/output/file.m4a"
static SAVED_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)saved\s+to:?\s+(.+)").expect("Invalid saved regex")
});

/// Matches lines containing explicit error indicators.
/// Looks for "ERROR:", "Error:", or "error:" prefixes
static ERROR_PREFIX_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:ERROR|error|Error):?\s+(.+)").expect("Invalid error regex")
});

// ============================================================
// Event types emitted to the frontend
// ============================================================

/// A structured event parsed from a line of GAMDL's stdout/stderr output.
/// The frontend listens for these events to update the download progress UI.
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

    // Priority 1: yt-dlp download progress (most frequent during downloads)
    if let Some(captures) = PROGRESS_REGEX.captures(trimmed) {
        let percent = captures
            .get(1)
            .and_then(|m| m.as_str().parse::<f64>().ok())
            .unwrap_or(0.0);
        let speed = captures
            .get(3)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
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

    // Priority 3: Track information from GAMDL
    if let Some(captures) = TRACK_INFO_REGEX.captures(trimmed) {
        let info = captures
            .get(2)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        // Try to split "Title by Artist" format
        let (title, artist) = if let Some(idx) = info.rfind(" by ") {
            (info[..idx].to_string(), info[idx + 4..].to_string())
        } else {
            (info, String::new())
        };

        return GamdlOutputEvent::TrackInfo {
            title,
            artist,
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

    // Priority 5: Post-processing steps (remuxing, tagging, embedding artwork)
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

    // Priority 7: Common error patterns (case-insensitive)
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

/// Checks if a GAMDL error message indicates a codec-related failure.
///
/// This is used by the fallback quality system to decide whether to retry
/// with a different audio codec or video resolution. Codec errors mean the
/// content is not available in the requested format and a fallback should
/// be attempted. Other errors (network, auth, not-found) should not trigger
/// codec fallback.
///
/// # Arguments
/// * `error_message` - The error message to classify
///
/// # Returns
/// `true` if the error is codec-related and fallback should be attempted
pub fn is_codec_error(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    lower.contains("codec not available")
        || lower.contains("no matching codec")
        || lower.contains("format not available")
        || lower.contains("unable to find matching codec")
        || lower.contains("requested codec")
        || lower.contains("drm")
}

/// Classifies an error message into a category for the UI.
///
/// Error categories determine which icon/color to show in the queue UI
/// and whether automatic retry/fallback is appropriate.
///
/// # Arguments
/// * `error_message` - The error message to classify
///
/// # Returns
/// A string identifier for the error category
pub fn classify_error(error_message: &str) -> &'static str {
    let lower = error_message.to_lowercase();

    if lower.contains("cookie") || lower.contains("auth") || lower.contains("login") {
        "auth" // Authentication/cookie error
    } else if lower.contains("network")
        || lower.contains("timeout")
        || lower.contains("connection")
        || lower.contains("dns")
    {
        "network" // Network connectivity error
    } else if is_codec_error(error_message) {
        "codec" // Codec/format availability error
    } else if lower.contains("not found") || lower.contains("404") || lower.contains("no results")
    {
        "not_found" // Content not found
    } else if lower.contains("rate limit") || lower.contains("429") || lower.contains("too many")
    {
        "rate_limit" // Rate limiting
    } else if lower.contains("ffmpeg")
        || lower.contains("mp4decrypt")
        || lower.contains("mp4box")
        || lower.contains("nm3u8dl")
    {
        "tool" // External tool error
    } else {
        "unknown" // Unclassified error
    }
}
