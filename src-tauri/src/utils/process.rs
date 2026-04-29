// Copyright (c) 2026 MeedyaDL
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
    Regex::new(r"\[download\]\s+(\d+\.?\d*)%\s+of\s+~?\s*(\S+)\s+at\s+(\S+)\s+ETA\s+(\S+)")
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

/// Matches GAMDL 2.9.x track information lines.
///
/// Capture groups:
///   1. `current` -- current track number (e.g. "1")
///   2. `total`   -- total track count (e.g. "15")
///   3. `title`   -- track title in quotes (e.g. "F1")
///
/// Example input: `[Track 1/15] Downloading "F1"`
///
/// GAMDL 2.9.x changed its output format from "Getting track N of M: Title"
/// to "[Track N/M] Downloading "Title"". This regex handles that format.
/// The `(?i)` flag handles case variations. The title quotes are optional
/// to handle edge cases.
///
/// GAMDL v3.0 began wrapping the bracket in padded structlog context:
/// `[Track   1/15 ]` — `action=f"Track {index:>3}/{total:<3}"` pads both
/// sides to width 3, so a trailing space can appear between the total
/// and the closing `]`. The `\s*` tolerances around the slash and
/// before the close bracket handle that (and any future upstream
/// spacing change) without breaking the numeric-only total contract.
static TRACK_INFO_V2_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\[Track\s+(\d+)\s*/\s*(\d+)\s*\]\s+Downloading\s+"?([^"]+)"?"#)
        .expect("Invalid track info v2 regex")
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
static SAVED_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)saved\s+to:?\s+(.+)").expect("Invalid saved regex"));

/// Matches lines containing explicit error indicators at the start.
///
/// Capture groups:
///   1. `message` -- the error message text after the prefix
///
/// Example inputs:
///   - `ERROR: Unable to download webpage`
///   - `Error: cookies file not found`
///   - `error: network timeout`
///   - `[ERROR    12:34:56] Error processing "https://...": 404 Not Found`
///     (GAMDL >= v3.0 structlog format — see `cli/utils.py`
///     `custom_structlog_formatter`)
///   - `[ERROR    23:02:03] [Track   1/14 ] Error downloading "Lavender Haze"`
///     (GAMDL >= v3.0 with per-track/URL bracketed infix — observed
///     in real v3.0 captures, see #521)
///
/// The `(?i)` flag makes the match case-insensitive. The `:?` makes the
/// trailing colon optional.
///
/// The optional `\[[A-Z]+\s+[\d:]+\]\s*` prefix accepts GAMDL v3.0's
/// structlog timestamp banner so we still catch real errors even when
/// they arrive wrapped as `[ERROR    HH:MM:SS] Error ...`. Without this,
/// the `^` anchor rejected the whole line, silently downgrading every
/// GAMDL error to Priority 7 keyword matching (which does not include
/// the word "error" on its own) and in the worst case to `Unknown`.
///
/// The optional `(?:\[[^\]]+\]\s*)*` allows zero or more bracketed
/// infixes between the structlog banner and the error keyword — real
/// v3.0 emits context prefixes like `[Track   1/14 ]` and `[URL   1/1 ]`
/// after the banner (#521 capture data, 2026-04-23).
static ERROR_PREFIX_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:\[[A-Z]+\s+[\d:]+\]\s*)?(?:\[[^\]]+\]\s*)*(?:ERROR|error|Error):?\s+(.+)",
    )
    .expect("Invalid error regex")
});

/// Matches Python exception lines that appear as the final line of a traceback.
///
/// Python tracebacks end with a line like:
///   `TypeError: 'NoneType' object has no attribute 'foo'`
///   `ValueError: invalid literal for int() with base 10: 'abc'`
///   `KeyError: 'missing_key'`
///   `requests.exceptions.HTTPError: 403 Client Error`
///   `httpx.ConnectTimeout: Connection timed out`
///   `httpx.ReadTimeout: timed out`
///
/// The pattern matches lines that start with an optional dotted module path
/// followed by a CamelCase word ending in "Error", "Exception", or
/// "Timeout", then an optional colon with message. The `^` anchor prevents
/// false positives from mid-line occurrences.
///
/// `Timeout` was added alongside `Error`/`Exception` so httpx's typed
/// timeout hierarchy (`ConnectTimeout`, `ReadTimeout`, `WriteTimeout`,
/// `PoolTimeout`) is still captured. Without it these lines fell through
/// to `GamdlOutputEvent::Unknown` — a silent regression for every
/// network timeout raised by GAMDL's HTTP stack.
static PYTHON_EXCEPTION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:[a-zA-Z_][a-zA-Z0-9_.]*\.)?[A-Z][a-zA-Z]*(?:Error|Exception|Timeout)(?::\s*.*)?$",
    )
    .expect("Invalid Python exception regex")
});

/// Matches ANSI escape sequences (color codes, cursor movement, etc.).
///
/// GAMDL and its Python dependencies (e.g., yt-dlp) may output ANSI
/// colour codes like `\x1b[32m` (green text) or `\x1b[0m` (reset).
/// These codes are intended for terminal rendering but display as raw
/// escape characters in the Activity Log's HTML-based view.
///
/// Pattern matches:
///   - `\x1b[32m`   -- SGR color (green)
///   - `\x1b[0m`    -- SGR reset
///   - `\x1b[2K`    -- erase entire line
///   - `\x1b[?25l`  -- hide cursor
///
/// Reference: <https://en.wikipedia.org/wiki/ANSI_escape_code>
static ANSI_ESCAPE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\].*?\x07").expect("Invalid ANSI escape regex")
});

/// Strips ANSI escape sequences from a string.
///
/// Used to clean subprocess output before emitting it to the Activity Log
/// frontend, where HTML rendering cannot interpret terminal colour codes.
pub fn strip_ansi_codes(input: &str) -> String {
    ANSI_ESCAPE_REGEX.replace_all(input, "").to_string()
}

/// Returns `true` when `line` is part of a Python traceback that the
/// activity-log feed should hide in non-verbose mode (#660).
///
/// This is the cheap (string-ops-only) twin of the [`parse_gamdl_output`]
/// Priority 3c branch: it lets the stdout/stderr readers gate the
/// per-line `activity-log` Tauri event without paying the full parser
/// cost twice. The on-disk activity-log writer still records the line
/// regardless of verbose mode, so the forensic record stays complete.
///
/// Detected forms:
///   - the bare `Traceback (most recent call last):` header,
///   - a `File "<path>", line N, in <fn>` stack-frame line,
///   - a caret highlight line (`^^^^^^^^^^`).
///
/// The actual exception summary line (`TypeError: ...`) is *not* matched
/// here — that one is meaningful and must remain visible.
pub fn is_python_traceback_noise(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed == "Traceback (most recent call last):" {
        return true;
    }
    if trimmed.starts_with("File \"")
        && trimmed.contains("\", line ")
        && trimmed.contains(", in ")
    {
        return true;
    }
    trimmed.chars().all(|c| c == '^')
}

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
        /// Current track number (1-based), if available from "[Track N/M]" format
        #[serde(skip_serializing_if = "Option::is_none")]
        track_number: Option<u32>,
        /// Total track count, if available from "[Track N/M]" format
        #[serde(skip_serializing_if = "Option::is_none")]
        track_total: Option<u32>,
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

    /// A line that is part of a Python traceback emitted by GAMDL or one of
    /// its dependencies — the `Traceback (most recent call last):` header,
    /// a `File "..."` stack frame, or a caret highlight line (`^^^^^^^^`).
    ///
    /// Tracebacks originate inside upstream Python code (gamdl, httpx,
    /// async_lru, etc.) and MeedyaDL cannot prevent them being printed.
    /// What MeedyaDL **can** do is stop misclassifying the header as an
    /// `Error` event (Priority 7's `traceback` keyword used to do that)
    /// and instead route this noise to a dedicated variant that the
    /// activity-log consumer only forwards to the user-visible feed when
    /// `verbose_activity_log` is enabled (#660).
    ///
    /// The actual exception summary (e.g. `TypeError: ...`) is still
    /// captured by [`PYTHON_EXCEPTION_REGEX`] and emitted as `Error`, so
    /// the user always sees the meaningful one-line error even when the
    /// surrounding frames are suppressed.
    TracebackFrame {
        /// The raw frame / header line as printed by Python.
        raw: String,
    },
}

/// Parses a single line of GAMDL output into a structured event.
///
/// GAMDL and its subprocesses (yt-dlp, `FFmpeg`) output progress and status
/// information in various formats. This parser applies regex patterns in
/// priority order to categorize each line:
///
/// 1. Download progress (yt-dlp format)
/// 2. Download completion (yt-dlp format)
/// 3. Track information (GAMDL "Getting song/track" lines)
/// 3c. Python traceback frames (header / `File "..."` / caret lines) — #660
/// 4. Explicit errors (ERROR/Error prefix)
/// 4b. Python exception summary line (`TypeError: ...`)
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
        return GamdlOutputEvent::Unknown { raw: String::new() };
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
            track_number: None,
            track_total: None,
        };
    }

    // Priority 3b: GAMDL 2.9.x track information format.
    // "[Track 1/15] Downloading "F1"" — new format introduced in GAMDL 2.9.x.
    // The line often arrives wrapped in an [INFO timestamp] prefix from GAMDL's
    // logging, so we match on the [Track N/M] portion within the line.
    if let Some(captures) = TRACK_INFO_V2_REGEX.captures(trimmed) {
        let title = captures
            .get(3)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        let track_number = captures
            .get(1)
            .and_then(|m| m.as_str().parse::<u32>().ok());
        let track_total = captures
            .get(2)
            .and_then(|m| m.as_str().parse::<u32>().ok());

        return GamdlOutputEvent::TrackInfo {
            title,
            artist: String::new(),
            album: String::new(),
            track_number,
            track_total,
        };
    }

    // Priority 3c: Python traceback noise (#660).
    // Detect three benign forms that should never be rendered as an Error:
    //   - the `Traceback (most recent call last):` header,
    //   - a `File "<path>", line N, in <fn>` stack-frame line,
    //   - a caret highlight line (`^^^^^^^^^^^^`).
    // The actual exception summary (`TypeError: ...`) is handled below by
    // PYTHON_EXCEPTION_REGEX and still surfaces as a real Error event.
    if trimmed == "Traceback (most recent call last):"
        || (trimmed.starts_with("File \"")
            && trimmed.contains("\", line ")
            && trimmed.contains(", in "))
        || (!trimmed.is_empty() && trimmed.chars().all(|c| c == '^'))
    {
        return GamdlOutputEvent::TracebackFrame {
            raw: trimmed.to_string(),
        };
    }

    // Priority 4: Explicit error messages with ERROR/Error prefix
    if let Some(captures) = ERROR_PREFIX_REGEX.captures(trimmed) {
        let message = captures
            .get(1)
            .map_or_else(|| trimmed.to_string(), |m| m.as_str().to_string());
        return GamdlOutputEvent::Error { message };
    }

    // Priority 4b: Python exception lines (final line of a traceback).
    // Catches lines like "TypeError: ...", "ValueError: ...", "KeyError: ...",
    // "requests.exceptions.HTTPError: 403 Client Error", etc.
    // These are the actual error descriptions that follow the "Traceback
    // (most recent call last):" header and stack frame lines.
    if PYTHON_EXCEPTION_REGEX.is_match(trimmed) {
        return GamdlOutputEvent::Error {
            message: trimmed.to_string(),
        };
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
    //   - "format is not available" -- GAMDL 2.8.x: "Requested format is not available"
    //   - "skipping"         -- GAMDL: track skipped due to format/codec issues
    //   - "no entry"         -- missing archive entries or config keys
    //   - "exception"        -- Python exception messages
    //
    // The `traceback` keyword used to live here too, but the bare
    // `Traceback (most recent call last):` header is just the start of a
    // multi-line Python trace, not an error itself — the actual exception
    // is caught by Priority 4b (PYTHON_EXCEPTION_REGEX) and the header is
    // now classified as `TracebackFrame` by Priority 3c (#660). Keeping
    // `traceback` here would re-emit the header as a duplicate Error event
    // alongside the legitimate exception line.
    let lower = trimmed.to_lowercase();
    if lower.contains("failed")
        || lower.contains("not found")
        || lower.contains("permission denied")
        || lower.contains("codec not available")
        || lower.contains("format is not available")
        || lower.contains("skipping")
        || lower.contains("no entry")
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
// Error Classification
// ============================================================

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
#[must_use]
pub fn is_codec_error(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    lower.contains("codec not available")      // yt-dlp: requested codec not in manifest
        || lower.contains("no matching codec") // GAMDL: no codec matches quality preference
        || lower.contains("format not available") // yt-dlp/GAMDL: requested format ID not found
        || lower.contains("format is not available") // GAMDL 2.8.x: "Requested format is not available"
        || lower.contains("unable to find matching codec") // GAMDL variant
        || lower.contains("requested codec")   // GAMDL: "requested codec X not available"
        || lower.contains("requested format")  // GAMDL 2.8.x: "Requested format is not available"
        || lower.contains("drm") // DRM-protected content (cannot be decoded)
}

/// Checks if a GAMDL error message indicates a filesystem I/O error.
///
/// Used to distinguish filesystem timeouts (e.g., cloud-mounted drives
/// timing out when writing cover art or output files) from network timeouts
/// or codec errors. Cloud storage paths like `CloudMounter`, Google Drive
/// File Stream, `OneDrive`, or iCloud Drive may experience I/O timeouts when
/// the remote service is slow or unreachable, while the actual audio
/// download (to a local temp directory) may have succeeded.
///
/// # Arguments
/// * `error_message` - The error message string to check.
///
/// # Returns
/// `true` if the error indicates a filesystem I/O issue; `false` otherwise.
///
/// # Connection
/// Called by `services::download_queue` in the success path to determine
/// whether "no output" is due to a filesystem issue (recoverable — files
/// may exist) vs. a codec issue (needs fallback) vs. a genuine failure.
#[must_use]
pub fn is_io_error(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    lower.contains("operation timed out")       // macOS ETIMEDOUT (errno 60)
        || lower.contains("[errno 60]")         // macOS ETIMEDOUT errno
        || lower.contains("no space left")      // Disk full (ENOSPC)
        || lower.contains("read-only file system") // Read-only mount (EROFS)
        || lower.contains("input/output error") // Generic I/O error (EIO)
        || lower.contains("[errno 5]")          // Linux EIO
        || lower.contains("stale file handle")  // NFS stale handle (ESTALE)
        || lower.contains("[errno 116]") // ESTALE on Linux
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
/// | `"network"`    | network, timeout, timed out, connection, connecterror, dns, httpx, httpcore | Yes |
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
#[must_use]
pub fn classify_error(error_message: &str) -> &'static str {
    let lower = error_message.to_lowercase();

    // Authentication / cookie errors: user needs to provide valid credentials.
    if lower.contains("cookie") || lower.contains("auth") || lower.contains("login") {
        "auth"
    // Filesystem I/O errors: cloud storage timeout, disk full, etc.
    // Checked before network because "operation timed out" contains "timed"
    // which would otherwise match the network check's "timeout" pattern.
    // These indicate the output path is unreachable or slow (e.g., a
    // CloudMounter-mounted MEGA drive timing out on cover art writes).
    } else if is_io_error(error_message) {
        "io"
    // Network errors: transient, may resolve on retry.
    // Includes Python httpx/httpcore exceptions (ConnectError, ReadTimeout, etc.),
    // common socket-level messages (connection refused, timed out, etc.), and
    // the httpx/httpcore library names themselves (any error from these HTTP
    // transport libraries indicates a network/connectivity issue).
    } else if lower.contains("network")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection")
        || lower.contains("connecterror")
        || lower.contains("dns")
        || lower.contains("httpx")
        || lower.contains("httpcore")
    {
        "network"
    // Codec/format errors: the requested quality is not available; try fallback.
    } else if is_codec_error(error_message) {
        "codec"
    // GAMDL playlist template KeyError (#588). Specific to classical /
    // cross-work Apple Music Classical playlists where some tracks
    // lack a `title` attribute in the catalog response — GAMDL's
    // `get_playlist_file_path` template renderer unconditionally
    // dereferences `kwargs["title"]` and raises `KeyError: 'title'`.
    // Matched before the generic "unknown" bucket so users get a
    // specific "this is a known upstream limitation" message rather
    // than a raw Python traceback excerpt.
    } else if is_playlist_title_keyerror(error_message) {
        "playlist_title_keyerror"
    // Content not found: the URL is invalid or the content was removed.
    } else if lower.contains("not found") || lower.contains("404") || lower.contains("no results") {
        "not_found"
    // Rate limiting: the server is throttling requests; retry after delay.
    } else if lower.contains("rate limit") || lower.contains("429") || lower.contains("too many") {
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

/// Returns a user-friendly recovery suggestion for the given error category.
///
/// Used by the activity log and error display to help users understand
/// what to do when a download fails, rather than just showing the raw error.
pub fn error_guidance(category: &str) -> &'static str {
    match category {
        "auth" => "Try refreshing your cookies (Settings > Authentication) or check your wrapper connection.",
        "network" => "Check your internet connection. The download will auto-retry when connectivity is restored.",
        "io" => "Check that your output directory is accessible and has sufficient disk space.",
        "codec" => "This format may not be available for this content. Try a different quality setting.",
        "not_found" => "This content may have been removed from Apple Music or the URL may be incorrect.",
        "rate_limit" => "Apple Music is rate-limiting requests. Wait a few minutes and retry.",
        "tool" => "A required tool (FFmpeg, mp4decrypt, etc.) may be missing or outdated. Check Settings > Tools.",
        "playlist_title_keyerror" => "Some tracks in this playlist are missing required metadata — this is a known upstream GAMDL limitation with certain Apple Music Classical playlists (see issue #588). Try downloading the individual albums instead, or report it upstream at https://github.com/glomatico/gamdl/issues.",
        _ => "Check the Activity Log for more details. If this persists, report it via Settings > Advanced > Error Reporting.",
    }
}

/// Detect GAMDL's playlist-template `KeyError: 'title'` traceback (#588).
///
/// Pattern captured 2026-04-23 during #547 scenario 4 repro on an Apple
/// Music Classical cross-work playlist. GAMDL's `get_playlist_file_path`
/// unconditionally dereferences `kwargs["title"]` even when the track's
/// catalog entry lacks a `name` attribute, raising `KeyError: 'title'`
/// on every affected track.
///
/// Matches the exact traceback signature to avoid false positives on
/// other `KeyError: 'title'` causes (e.g. an unrelated metadata-parser
/// failure).
#[must_use]
pub fn is_playlist_title_keyerror(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    lower.contains("keyerror: 'title'")
        && (lower.contains("get_playlist_file_path")
            || lower.contains("downloader_base")
            || lower.contains("playlist_file_path"))
}

// ============================================================
// GAMDL output classification helpers (companion download safety)
// ============================================================
//
// GAMDL exits with code 0 even when individual tracks fail — the per-track
// exception is caught by its main loop, the traceback is printed to stderr,
// and the process keeps running. The summary line `Finished with N error(s)`
// is the authoritative signal that something went wrong inside an apparently
// "successful" run.
//
// These helpers let the companion download supervisor convert that "soft
// error" into a real failure (so the next codec / tier is tried) and turn
// known Python tracebacks into a single user-facing line, instead of dumping
// the raw traceback into the activity log.

/// Compiled regex for GAMDL's per-run summary line.
///
/// Matches `Finished with N error(s)` for any non-negative N. The summary
/// is emitted on stdout (not stderr) so the parser must scan both.
static GAMDL_FINISHED_SUMMARY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Finished with (\d+) error\(s\)")
        .expect("GAMDL_FINISHED_SUMMARY regex must compile")
});

/// Parses GAMDL's `Finished with N error(s)` summary line and returns N.
///
/// Returns `None` when the summary is absent (e.g., GAMDL crashed early
/// without printing the summary). Returns `Some(0)` when the summary is
/// present and reports zero errors. Callers should treat `None` as
/// "couldn't tell" and rely on the exit code in that case.
pub fn parse_gamdl_error_count(output: &str) -> Option<u32> {
    GAMDL_FINISHED_SUMMARY
        .captures(output)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
}

/// Translates a known GAMDL Python traceback into a single user-friendly
/// activity-log line. Returns `None` for tracebacks we don't recognise so
/// the caller can fall back to a generic "GAMDL reported an error" message.
///
/// Currently handled:
///   - `AttributeError: 'NoneType' object has no attribute 'audio_track'`
///     and the related `'NoneType' has no attribute 'stream_info'` paths,
///     both of which mean Apple Music's manifest didn't return any stream
///     for the requested codec on this track. Surfaces as a "codec not
///     available for this track" message rather than a Python crash.
pub fn classify_gamdl_traceback(output: &str) -> Option<&'static str> {
    let lower = output.to_ascii_lowercase();
    if lower.contains("'nonetype' object has no attribute 'audio_track'")
        || lower.contains("'nonetype' object has no attribute 'stream_info'")
        || lower.contains("'nonetype' object has no attribute 'audio'")
    {
        return Some(
            "this codec is not available for this track on Apple Music — skipping",
        );
    }
    None
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
    fn parses_traceback_header_as_traceback_frame() {
        // The bare `Traceback (most recent call last):` header used to be
        // promoted to an Error event by Priority 7's `traceback` keyword,
        // which produced a duplicate red entry in the Activity Log next
        // to the genuine exception line (PYTHON_EXCEPTION_REGEX). It now
        // reaches the dedicated TracebackFrame variant so the consumer
        // can suppress it from the user-facing feed in non-verbose mode
        // while still mirroring the line to the on-disk log (#660).
        let line = "Traceback (most recent call last):";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TracebackFrame { raw } => {
                assert_eq!(raw, line);
            }
            other => panic!("Expected TracebackFrame, got {:?}", other),
        }
    }

    #[test]
    fn parses_traceback_file_frame_as_traceback_frame() {
        // Stack-frame lines from upstream Python (gamdl, httpx, async_lru,
        // etc.) used to fall through to `Unknown` and were silently
        // dropped — a quiet win — but the explicit classification makes
        // the contract testable and documents the suppression intent.
        let line = r#"File "/path/to/gamdl/cli/cli.py", line 272, in main"#;
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TracebackFrame { raw } => {
                assert_eq!(raw, line);
            }
            other => panic!("Expected TracebackFrame, got {:?}", other),
        }
    }

    #[test]
    fn parses_traceback_caret_line_as_traceback_frame() {
        let line = "^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TracebackFrame { raw } => {
                assert_eq!(raw, line);
            }
            other => panic!("Expected TracebackFrame, got {:?}", other),
        }
    }

    #[test]
    fn is_python_traceback_noise_recognises_three_forms() {
        assert!(is_python_traceback_noise("Traceback (most recent call last):"));
        assert!(is_python_traceback_noise(
            r#"  File "/foo/bar.py", line 10, in baz"#
        ));
        assert!(is_python_traceback_noise("^^^^"));
        assert!(is_python_traceback_noise("    ^^^^^^^^^^^^^^^^"));
    }

    #[test]
    fn is_python_traceback_noise_rejects_meaningful_lines() {
        // These three are precisely the lines we *do* want to keep visible.
        assert!(!is_python_traceback_noise(
            "TypeError: 'NoneType' object has no attribute 'foo'"
        ));
        assert!(!is_python_traceback_noise("Downloading album..."));
        assert!(!is_python_traceback_noise(
            "[INFO     12:34:56] [Track 1/12] Downloading \"Hello\""
        ));
    }

    #[test]
    fn parses_structlog_wrapped_error_line() {
        // GAMDL v3.0 migrated to structlog: every user-facing log line
        // is rendered via `cli/utils.py::custom_structlog_formatter` as
        // `[LEVEL    HH:MM:SS] message`. The `[ERROR ...]` wrapper used
        // to cause Priority 4 to miss real errors, downgrading them to
        // `Unknown` in the activity log (Priority 7 only matches the
        // word "error" when combined with "failed"/"exception"/etc.).
        //
        // The regex captures everything after the `Error` prefix, so
        // the captured message is `processing "...": 404 Not Found`.
        // The URL and the reason ("404 Not Found") must survive — they
        // are what `classify_error` and the activity log display keys on.
        let line = r#"[ERROR    12:34:56] Error processing "https://music.apple.com/us/album/...": 404 Not Found"#;
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(
                    message.contains("music.apple.com")
                        && message.contains("404 Not Found"),
                    "Expected the structlog prefix + `Error` keyword to be \
                     stripped while URL and reason are preserved. \
                     Got: {message}"
                );
            }
            other => panic!("Expected Error, got {other:?}"),
        }
    }

    #[test]
    fn parses_structlog_wrapped_error_variable_spacing() {
        // Structlog's level field is left-padded to 8 chars via
        // `f"[{level:<8} {timestamp}]"`. "ERROR" is 5 chars and gets
        // three trailing spaces, "INFO" is 4 and gets four — the regex
        // must tolerate the variable width.
        let line = "[ERROR    09:01:02] ERROR: cookies file not found";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(
                    message.contains("cookies file not found"),
                    "Expected structlog + embedded ERROR: prefix to be \
                     stripped. Got: {message}"
                );
            }
            other => panic!("Expected Error, got {other:?}"),
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
    // is_io_error
    // ----------------------------------------------------------

    #[test]
    fn detects_macos_timeout_error() {
        assert!(is_io_error(
            "TimeoutError: [Errno 60] Operation timed out: '/path/to/Cover.jpg'"
        ));
    }

    #[test]
    fn detects_errno_60() {
        assert!(is_io_error("[Errno 60] Operation timed out"));
    }

    #[test]
    fn detects_disk_full() {
        assert!(is_io_error("No space left on device"));
    }

    #[test]
    fn detects_read_only_fs() {
        assert!(is_io_error("Read-only file system: '/mnt/external'"));
    }

    #[test]
    fn detects_generic_io_error() {
        assert!(is_io_error("Input/output error writing file"));
    }

    #[test]
    fn does_not_detect_network_as_io() {
        assert!(!is_io_error("Network timeout"));
    }

    #[test]
    fn does_not_detect_codec_as_io() {
        assert!(!is_io_error("Codec not available"));
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
    fn classifies_gamdl_playlist_title_keyerror() {
        // The exact traceback signature captured 2026-04-23 during
        // #547 scenario 4 repro (#588). Must route to the dedicated
        // category, not fall through to "unknown".
        let stderr = "Traceback (most recent call last):\n\
            File \".../gamdl/downloader/downloader_base.py\", line 377, in get_playlist_file_path\n\
            formatted_part = CustomStringFormatter().format(\n\
            KeyError: 'title'";
        assert_eq!(classify_error(stderr), "playlist_title_keyerror");
    }

    #[test]
    fn classifies_unrelated_keyerror_title_as_unknown() {
        // Regression canary: a `KeyError: 'title'` without the
        // playlist-renderer frame must NOT be mis-classified as a
        // playlist bug. Some unrelated upstream code path could
        // legitimately raise the same error with a different cause.
        let stderr = "KeyError: 'title' in unrelated module";
        assert_eq!(classify_error(stderr), "unknown");
    }

    #[test]
    fn playlist_keyerror_guidance_points_users_upstream() {
        // Users hitting this should be told (a) it's a known limitation
        // and (b) where to escalate — not just "check the log".
        let guidance = error_guidance("playlist_title_keyerror");
        assert!(guidance.contains("Apple Music Classical"));
        assert!(guidance.contains("glomatico/gamdl"));
    }

    #[test]
    fn classifies_io_errors() {
        // macOS CloudMounter timeout (the exact error from the user's test)
        assert_eq!(
            classify_error("TimeoutError: [Errno 60] Operation timed out: '/path/Cover.jpg'"),
            "io"
        );
        // Disk full
        assert_eq!(classify_error("No space left on device"), "io");
        // Read-only filesystem
        assert_eq!(classify_error("Read-only file system"), "io");
        // Generic I/O error
        assert_eq!(classify_error("Input/output error"), "io");
        // NFS stale handle
        assert_eq!(classify_error("Stale file handle"), "io");
    }

    #[test]
    fn classifies_network_errors() {
        assert_eq!(classify_error("Network timeout"), "network");
        assert_eq!(classify_error("Connection refused"), "network");
        assert_eq!(classify_error("DNS resolution failed"), "network");
        // Python httpx/httpcore exceptions from wrapper connectivity issues
        assert_eq!(
            classify_error("httpx.ConnectError: [Errno 61] Connection refused"),
            "network"
        );
        assert_eq!(
            classify_error("httpcore.ConnectError: All connection attempts failed"),
            "network"
        );
        assert_eq!(classify_error("httpx.ConnectTimeout: timed out"), "network");
        // Bare ConnectError without message context
        assert_eq!(classify_error("httpcore.ConnectError:"), "network");
        // "timed out" without "operation" prefix (network, not IO)
        assert_eq!(classify_error("Read timed out"), "network");
        // Raw traceback frame line containing httpx library path — the exact
        // error from the user's screenshot when wrapper is unreachable
        assert_eq!(
            classify_error(
                r#"File "/Users/user/Library/Application Support/io.github.meedyadl/python/lib/python3.12/site-packages/httpx/_transports/default.py", line 118, in map_httpcore_exceptions"#
            ),
            "network"
        );
        // httpcore frame line
        assert_eq!(
            classify_error(
                r#"File "/path/to/httpcore/_exceptions.py", line 10, in map_exceptions"#
            ),
            "network"
        );
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

    // ---- Python exception line detection (Priority 4b) ----

    #[test]
    fn parses_python_type_error() {
        let line = "TypeError: 'NoneType' object has no attribute 'foo'";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(message.contains("TypeError"));
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn parses_python_value_error() {
        let line = "ValueError: invalid literal for int() with base 10: 'abc'";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(message.contains("ValueError"));
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn parses_python_key_error() {
        let line = "KeyError: 'missing_key'";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(message.contains("KeyError"));
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn parses_dotted_python_exception() {
        let line = "requests.exceptions.HTTPError: 403 Client Error";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(message.contains("HTTPError"));
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn parses_bare_exception_without_message() {
        // A bare exception class name (no colon/message) should still match
        // if it ends with "Error" or "Exception"
        let line = "RuntimeError";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert_eq!(message, "RuntimeError");
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn does_not_match_non_exception_line() {
        // A regular lowercase word should not match the Python exception regex
        let line = "downloading file from server";
        if let GamdlOutputEvent::Error { .. } = parse_gamdl_output(line) {
            panic!("Should not match as Python exception");
        }
    }

    #[test]
    fn traceback_frame_not_captured_as_error() {
        // Python traceback frame lines start with `File "` and should NOT be
        // captured as Error events, even when they contain "exception" in a
        // function name (e.g., `map_httpcore_exceptions`). These are stack
        // frames, not error messages.
        let line =
            r#"File "/path/to/httpx/_transports/default.py", line 118, in map_httpcore_exceptions"#;
        if let GamdlOutputEvent::Error { .. } = parse_gamdl_output(line) {
            panic!("Traceback frame should not be captured as Error");
        }
    }

    #[test]
    fn traceback_frame_with_indentation_not_captured() {
        // Same as above but with leading whitespace (how it appears in actual tracebacks)
        let line = r#"  File "/path/to/httpcore/_exceptions.py", line 10, in map_exceptions"#;
        if let GamdlOutputEvent::Error { .. } = parse_gamdl_output(line) {
            panic!("Indented traceback frame should not be captured as Error");
        }
    }

    // ----------------------------------------------------------
    // GAMDL companion-soft-error helpers
    // ----------------------------------------------------------

    #[test]
    fn parse_gamdl_error_count_finds_summary() {
        let output = "[INFO  21:57:04] Starting Gamdl 2.9.3\n\
                      [ERROR 21:57:04] [Track 1/1] Error downloading \"PSYCHO\"\n\
                      [INFO  21:57:04] Finished with 1 error(s)\n";
        assert_eq!(parse_gamdl_error_count(output), Some(1));
    }

    #[test]
    fn parse_gamdl_error_count_zero() {
        let output = "[INFO 12:00:00] Finished with 0 error(s)\n";
        assert_eq!(parse_gamdl_error_count(output), Some(0));
    }

    #[test]
    fn parse_gamdl_error_count_missing_returns_none() {
        let output = "[INFO 12:00:00] Starting Gamdl 2.9.3\n";
        assert_eq!(parse_gamdl_error_count(output), None);
    }

    #[test]
    fn classify_gamdl_traceback_recognises_audio_track_none() {
        let trace = "Traceback (most recent call last):\n  \
                     File \"downloader_song.py\", line 90, in get_download_item\n    \
                     if download_item.stream_info.audio_track.legacy:\n\
                     AttributeError: 'NoneType' object has no attribute 'audio_track'";
        let msg = classify_gamdl_traceback(trace).expect("should be classified");
        assert!(msg.contains("not available"));
    }

    #[test]
    fn classify_gamdl_traceback_unknown_returns_none() {
        let trace = "Traceback (most recent call last):\n\
                     KeyError: 'foo'";
        assert!(classify_gamdl_traceback(trace).is_none());
    }

    // ======================================================================
    // GAMDL v3.0 output fixtures — derived from real live-fire capture data
    // ======================================================================
    //
    // The happy-path, bracketed-track-error, and double-traceback fixtures
    // are verbatim transcriptions of stderr from a v3.0 run on macOS
    // (2026-04-23, see #521 capture comment). The codec-skip fixture is
    // still synthetic — no capture exercised a real codec-unavailable
    // scenario (all four captures errored pre-download on cover-fetch or
    // catalog 404). The wording for `Skipping "{title}": {e}` comes from
    // upstream source at `cli/cli.py`.
    //
    // Key v3.0 formatting patterns verified against real output:
    //
    //   * `cli/utils.py::custom_structlog_formatter` pads the level name
    //     to 8 characters: `[INFO     HH:MM:SS]`, `[WARNING  HH:MM:SS]`,
    //     `[ERROR    HH:MM:SS]`.
    //   * Per-URL/per-track context prefixes have an internal-padded form:
    //     `[URL   1/1  ]`, `[Track   1/14 ]`, `[Track   2/4  ]`.
    //   * Track-scoped errors stack TWO bracket groups:
    //     `[ERROR    HH:MM:SS] [Track   N/M ] Error downloading "Title"`.
    //   * When a previous exception is re-raised, Python emits the marker:
    //     `During handling of the above exception, another exception occurred:`
    //     followed by a second traceback.
    //   * The final line is always `[INFO     HH:MM:SS] Finished with N error(s)`.
    //   * Startup line is `[INFO     HH:MM:SS] Starting Gamdl 3.0`
    //     (just major.minor, no patch).

    /// Happy-path album download — structlog INFO lines only.
    ///
    /// Uses the real v3.0 formatting verified from capture data:
    /// `Starting Gamdl 3.0`, `[URL   1/1  ]` prefix, `[Track   N/M ]` prefix.
    const FIXTURE_V3_SUCCESSFUL_ALBUM: &str =
        "[INFO     12:00:00] Starting Gamdl 3.0\n\
         [INFO     12:00:01] [URL   1/1  ] Processing \"https://music.apple.com/us/album/example/1234567890\"\n\
         [INFO     12:00:02] [Track   1/2  ] Downloading \"Track One\"\n\
         [INFO     12:00:05] [Track   2/2  ] Downloading \"Track Two\"\n\
         [INFO     12:00:08] Finished with 0 error(s)\n";

    /// Codec-skip scenario — triggers gap-fill retry in
    /// `download_queue::count_codec_skip_warnings`. Structlog prefixes
    /// every warning. Exact wording inherited from the exception raised
    /// by the downloader when a requested codec isn't offered.
    ///
    /// NOTE: still synthetic as of 2026-04-23 (#521). The live-fire
    /// captures all errored on cover-fetch or catalog 404 before
    /// reaching a track-download stage, so we haven't verified the exact
    /// `Skipping "..."` wording against a real v3.0 run.
    const FIXTURE_V3_CODEC_SKIPS: &str =
        "[INFO     12:10:00] [URL   1/1  ] Processing \"https://music.apple.com/us/album/example/1234567890\"\n\
         [WARNING  12:10:01] You have chosen an experimental song codec without enabling wrapper. They're not guaranteed to work due to API limitations.\n\
         [INFO     12:10:02] [Track   1/4  ] Downloading \"Track One\"\n\
         [WARNING  12:10:05] [Track   2/4  ] Skipping \"Track Two\": Requested format is not available\n\
         [INFO     12:10:06] [Track   3/4  ] Downloading \"Track Three\"\n\
         [WARNING  12:10:09] [Track   4/4  ] Skipping \"Track Four\": Requested format is not available\n\
         [INFO     12:10:10] Finished with 2 error(s)\n";

    /// Auth / URL error — 404 from the Apple Music catalog, as a
    /// nested-exception traceback. Real v3.0 capture data (#521,
    /// Capture D, 2026-04-23).
    ///
    /// Two distinctive features only present in real v3.0:
    ///   1. The error line carries a `[Track   1/1  ]` bracketed infix
    ///      between the structlog banner and `Error downloading`.
    ///   2. `During handling of the above exception, another exception
    ///      occurred:` boundary between an httpx 404 and the wrapping
    ///      `GamdlApiResponseError`.
    const FIXTURE_V3_AUTH_ERROR: &str = concat!(
        "[INFO     12:20:00] Starting Gamdl 3.0\n",
        "[WARNING  12:20:01] You have chosen an experimental song codec without enabling wrapper. They're not guaranteed to work due to API limitations.\n",
        "[INFO     12:20:01] [URL   1/1  ] Processing \"https://music.apple.com/gb/album/fake-album-test/999999999999\"\n",
        "[INFO     12:20:01] [Track   1/1  ] Downloading \"Unknown Title\"\n",
        "[ERROR    12:20:01] [Track   1/1  ] Error downloading \"Unknown Title\"\n",
        "Traceback (most recent call last):\n",
        "  File \"/site-packages/gamdl/api/apple_music.py\", line 274, in _amp_request\n",
        "    response.raise_for_status()\n",
        "  File \"/site-packages/httpx/_models.py\", line 829, in raise_for_status\n",
        "    raise HTTPStatusError(message, request=request, response=self)\n",
        "httpx.HTTPStatusError: Client error '404 Not Found' for url 'https://amp-api.music.apple.com/v1/catalog/gb/albums/999999999999?extend=extendedAssetUrls'\n",
        "For more information check: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/404\n",
        "During handling of the above exception, another exception occurred:\n",
        "Traceback (most recent call last):\n",
        "  File \"/site-packages/gamdl/cli/cli.py\", line 279, in main\n",
        "    await downloader.download(download_item)\n",
        "  File \"/site-packages/gamdl/api/apple_music.py\", line 277, in _amp_request\n",
        "    raise GamdlApiResponseError(\n",
        "gamdl.api.exceptions.GamdlApiResponseError: Error fetching from AMP API (Status code: 404): {\"errors\":[{\"id\":\"NJWPW6PVGQY53KULKAYYOVHBMI\",\"title\":\"Resource Not Found\",\"detail\":\"Resource with requested id was not found\",\"status\":\"404\",\"code\":\"40400\"}]}\n",
        "[INFO     12:20:01] Finished with 1 error(s)\n",
    );

    /// Network failure with a verbose traceback (happens when the user
    /// has flipped `verbose_gamdl_exceptions` on or MeedyaDL is running
    /// against an older GAMDL that didn't set `--no-exceptions`).
    const FIXTURE_V3_NETWORK_TRACEBACK: &str =
        "[INFO     12:30:00] Processing \"https://music.apple.com/us/album/example/1234567890\"\n\
         [ERROR    12:30:05] Error processing \"https://music.apple.com/us/album/example/1234567890\": \
         Connection timed out\n\
         Traceback (most recent call last):\n  \
         File \"gamdl/cli/cli.py\", line 142, in main\n    \
         downloader.download(url)\n  \
         File \"httpx/_transports/default.py\", line 118, in map_httpcore_exceptions\n    \
         raise mapped_exc(message) from exc\n\
         httpx.ConnectTimeout: Connection timed out\n\
         [INFO     12:30:05] Finished with 1 error(s)\n";

    /// Helper: walk every non-empty line in `output` through
    /// `parse_gamdl_output` and return the classifications. Keeps the
    /// per-fixture assertions focused on the interesting events.
    fn classify_lines(output: &str) -> Vec<GamdlOutputEvent> {
        output
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(parse_gamdl_output)
            .collect()
    }

    #[test]
    fn v3_successful_album_classifies_cleanly() {
        // Every line in the happy-path fixture is either a TrackInfo
        // (from "Downloading \"...\"" — actually no, that's an INFO
        // string which doesn't match our regex, so Unknown is fine) or
        // a plain INFO status line. What matters is that NONE of them
        // get misclassified as Error.
        let events = classify_lines(FIXTURE_V3_SUCCESSFUL_ALBUM);
        let errors: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, GamdlOutputEvent::Error { .. }))
            .collect();
        assert!(
            errors.is_empty(),
            "Happy-path fixture must not produce Error events. Got: {errors:?}"
        );
    }

    #[test]
    fn v3_successful_album_finished_summary() {
        // `parse_gamdl_error_count` reads the "Finished with N error(s)"
        // line — confirm it survives the structlog prefix (the regex is
        // a substring match, so it should).
        assert_eq!(parse_gamdl_error_count(FIXTURE_V3_SUCCESSFUL_ALBUM), Some(0));
    }

    #[test]
    fn v3_codec_skips_are_detected_as_errors() {
        // The two "Skipping ... Requested format is not available" lines
        // are WARNING level from GAMDL's perspective. For MeedyaDL's
        // parser they should register as Error events so
        // `count_codec_skip_warnings` / `is_codec_error` can pick them
        // up — we need that to kick off gap-fill retry.
        let events = classify_lines(FIXTURE_V3_CODEC_SKIPS);
        let errors: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                GamdlOutputEvent::Error { message } => Some(message.clone()),
                _ => None,
            })
            .collect();
        assert!(
            errors.iter().any(|m| m.to_lowercase().contains("format is not available")),
            "Expected at least one 'format is not available' Error, got: {errors:?}"
        );
    }

    #[test]
    fn v3_codec_skips_trigger_codec_error_classification() {
        // `is_codec_error` is the signal the fallback chain consumes —
        // it must return true for the skip warnings, otherwise we stay
        // on the failed codec instead of trying the next one.
        let any_codec = FIXTURE_V3_CODEC_SKIPS
            .lines()
            .any(is_codec_error);
        assert!(
            any_codec,
            "v3.0 codec-skip fixture must contain at least one line that \
             `is_codec_error` recognises"
        );
    }

    #[test]
    fn v3_codec_skips_finished_summary_counts_errors() {
        assert_eq!(parse_gamdl_error_count(FIXTURE_V3_CODEC_SKIPS), Some(2));
    }

    #[test]
    fn v3_auth_error_classifies_as_error_with_url_and_reason() {
        // The `ERROR_PREFIX_REGEX` update in #517 should let this line
        // through without needing Priority-7 keyword matching. Verify
        // the URL and the reason both survive.
        let events = classify_lines(FIXTURE_V3_AUTH_ERROR);
        let error_messages: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                GamdlOutputEvent::Error { message } => Some(message.clone()),
                _ => None,
            })
            .collect();
        assert!(
            error_messages.iter().any(|m| {
                m.contains("music.apple.com") && m.contains("404 Not Found")
            }),
            "Expected URL + reason to be preserved in the Error message, \
             got: {error_messages:?}"
        );
    }

    #[test]
    fn v3_auth_error_classifies_as_not_found() {
        // `classify_error` is what drives the UI category badge. The
        // "404 Not Found" string should land in `not_found`, not
        // `network` or `unknown`.
        let reason = "404 Not Found";
        assert_eq!(classify_error(reason), "not_found");
    }

    #[test]
    fn v3_network_traceback_stays_classified_as_network() {
        // Even with the traceback interleaved, the wrapping error line
        // "Error processing ...: Connection timed out" plus the
        // `httpx.ConnectTimeout` exception line both carry network
        // keywords. `classify_error` must see the whole message as
        // network-category so our retry logic fires.
        //
        // Walk every line of the fixture and confirm at least one of
        // them classifies as `network`. Using the fixture (not
        // hard-coded snippets) keeps this test honest when we update
        // the fixture later with real v3.0 output.
        let any_network = FIXTURE_V3_NETWORK_TRACEBACK
            .lines()
            .filter(|line| !line.trim().is_empty())
            .any(|line| classify_error(line) == "network");
        assert!(
            any_network,
            "Expected at least one line in the v3 network fixture to \
             classify as `network`; otherwise retry logic will not fire"
        );

        // Final Python exception line on its own should also classify
        // as network via the httpx keyword branch.
        let exc_line = "httpx.ConnectTimeout: Connection timed out";
        assert_eq!(classify_error(exc_line), "network");
    }

    #[test]
    fn v3_network_traceback_frame_is_not_misclassified_as_error() {
        // `map_httpcore_exceptions` contains the substring "exception",
        // which WOULD be picked up by Priority-7 keyword matching — but
        // `File "..."` traceback frames short-circuit that branch.
        // Confirm we still skip the frame line.
        let frame = r#"  File "httpx/_transports/default.py", line 118, in map_httpcore_exceptions"#;
        if let GamdlOutputEvent::Error { .. } = parse_gamdl_output(frame) {
            panic!("Traceback frame must not be captured as an Error");
        }
    }

    #[test]
    fn v3_network_traceback_exception_line_is_captured() {
        // The real exception line (`httpx.ConnectTimeout: ...`) MUST be
        // captured as an Error, so the activity log at least shows the
        // final cause even when frames are hidden.
        let exc = "httpx.ConnectTimeout: Connection timed out";
        match parse_gamdl_output(exc) {
            GamdlOutputEvent::Error { message } => {
                assert!(message.contains("ConnectTimeout"));
            }
            other => panic!("Expected Error, got {other:?}"),
        }
    }

    // ===================================================================
    // Real v3.0 capture regression tests (#521)
    // ===================================================================
    //
    // These tests encode invariants derived directly from the 2026-04-23
    // capture data. They're separate from the fixture-based tests above
    // so a future fixture refactor cannot accidentally relax them.

    #[test]
    fn v3_real_bracketed_track_error_is_captured_as_error() {
        // Regression for #521: the real v3.0 per-track error line carries
        // a `[Track   N/M ]` infix between the structlog banner and the
        // `Error downloading` keyword. Before the regex fix this fell
        // through to `Unknown` because neither `ERROR_PREFIX_REGEX` nor
        // Priority-7 keyword matching ("error" alone is not a keyword)
        // would pick it up. After the fix the infix is allowed.
        let line = r#"[ERROR    23:02:03] [Track   1/14 ] Error downloading "Lavender Haze""#;
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(
                    message.contains("Lavender Haze"),
                    "Track title must survive in Error message, got: {message}"
                );
            }
            other => panic!(
                "Real v3.0 bracketed Track-error line must classify as Error, \
                 not {other:?}"
            ),
        }
    }

    #[test]
    fn v3_real_bracketed_url_error_is_captured_as_error() {
        // Sibling regression: GAMDL v3.0 also emits URL-level errors with
        // the `[URL   1/1  ]` infix.
        let line = r#"[ERROR    23:02:03] [URL   1/1  ] Error processing "https://music.apple.com/gb/album/example/1649434004""#;
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(message.contains("music.apple.com"));
            }
            other => panic!("Expected Error, got {other:?}"),
        }
    }

    #[test]
    fn v3_real_nested_exception_marker_captured_by_keyword_match() {
        // `During handling of the above exception, another exception
        // occurred:` is Python's marker for a re-raised exception chain.
        // Priority-7 keyword matching picks up the "exception" substring
        // and surfaces it as an Error event — which is the right
        // behaviour (it signals "another error is coming" in the log).
        let line = "During handling of the above exception, another exception occurred:";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { .. } => {}
            other => panic!(
                "Nested-exception marker must be captured as Error so the \
                 activity log preserves the chain, got: {other:?}"
            ),
        }
    }

    #[test]
    fn v3_real_gamdl_api_response_error_captured_by_python_regex() {
        // `gamdl.api.exceptions.GamdlApiResponseError` has a three-dot
        // module path. The Python-exception regex uses `[a-zA-Z0-9_.]*`
        // in the optional module-path prefix, so multi-dot paths must
        // work. This is the exception class GAMDL v3.0 raises for
        // every AMP API failure (404s, 403s, etc.).
        let line = r#"gamdl.api.exceptions.GamdlApiResponseError: Error fetching from AMP API (Status code: 404): {"errors":[{"title":"Resource Not Found"}]}"#;
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(message.contains("GamdlApiResponseError"));
                assert!(message.contains("404"));
            }
            other => panic!(
                "GamdlApiResponseError line must be captured by PYTHON_EXCEPTION_REGEX, \
                 got: {other:?}"
            ),
        }
    }

    #[test]
    fn v3_real_auth_fixture_produces_full_error_chain() {
        // End-to-end: the real double-traceback fixture should yield an
        // Error-event chain that contains BOTH the bracketed track error
        // AND the final GamdlApiResponseError. These are the two pieces
        // the activity log surfaces to the user.
        let events = classify_lines(FIXTURE_V3_AUTH_ERROR);
        let messages: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                GamdlOutputEvent::Error { message } => Some(message.clone()),
                _ => None,
            })
            .collect();

        // `ERROR_PREFIX_REGEX` consumes the `Error ` prefix word itself
        // so the captured message starts at `downloading "Title"` — check
        // for the tail, not the prefix.
        assert!(
            messages.iter().any(|m| m.contains("downloading") && m.contains("Unknown Title")),
            "Expected bracketed Track-error line in Error chain, got: {messages:?}"
        );
        assert!(
            messages.iter().any(|m| m.contains("GamdlApiResponseError")),
            "Expected GamdlApiResponseError in Error chain, got: {messages:?}"
        );
    }

    #[test]
    fn v3_real_finished_summary_survives_nested_traceback() {
        // After a double-traceback, the `Finished with N error(s)` line
        // must still be discoverable — `parse_gamdl_error_count` scans
        // for the "Finished with N error(s)" substring anywhere in the
        // output, so interleaved tracebacks before it shouldn't block it.
        assert_eq!(parse_gamdl_error_count(FIXTURE_V3_AUTH_ERROR), Some(1));
    }

    #[test]
    fn v3_real_experimental_codec_warning_is_not_misclassified_as_error() {
        // Every v3.0 cookie-auth download emits this WARNING on startup
        // when the codec priority includes experimental codecs. It must
        // NOT be classified as an Error, otherwise every successful
        // ALAC+Atmos+... run would have a spurious error in the log.
        let line = "[WARNING  23:02:02] You have chosen an experimental song codec without enabling wrapper. They're not guaranteed to work due to API limitations.";
        if let GamdlOutputEvent::Error { .. } = parse_gamdl_output(line) {
            panic!("Experimental-codec warning must not be captured as Error");
        }
    }

    // ================================================================
    // GAMDL v3.1 regression tests (#608)
    // ================================================================
    //
    // What changed upstream between v3.0 and v3.1:
    //   * Track progress format is still `action=f"Track {index:>3}/{total:<3}"`,
    //     but a new fallback `media_total or "-"` means the emitted line
    //     reads `Track   1/-  ` when `total == 0`. Every call site in
    //     v3.1 passes an explicit non-zero total, so `-` shouldn't
    //     occur in practice — but the parser must degrade gracefully
    //     if it ever does.
    //   * URL parse errors upgraded from `url_log.warning` to
    //     `url_log.error` (commit `fd3b621`). The line now arrives as
    //     `[ERROR    HH:MM:SS] [URL   1/1  ] …`, which must trigger
    //     `ERROR_PREFIX_REGEX` and flow into `classify_error()`.
    //   * `AppleMusicMedia.index/total` are now populated for every
    //     download type (single songs → `total=1`, artist buckets →
    //     `total=len(selected_items)`, playlist/album → `trackCount`),
    //     so the `[Track   1/1  ]` line appears for single-song URLs
    //     where v3.0 stayed silent.

    #[test]
    fn v31_track_regex_matches_padded_format() {
        // `action=f"Track {index:>3}/{total:<3}"` produces
        // `Track   1/15 ` (right-padded index, left-padded total).
        let line = "[INFO     12:00:02] [Track   1/15 ] Downloading \"F1\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo {
                title,
                track_number,
                track_total,
                ..
            } => {
                assert_eq!(title, "F1");
                assert_eq!(track_number, Some(1));
                assert_eq!(track_total, Some(15));
            }
            other => panic!("Expected TrackInfo, got {other:?}"),
        }
    }

    #[test]
    fn v31_track_regex_matches_single_song_1_of_1() {
        // v3.1 emits `Track   1/1  ` for single-song URLs because
        // `_get_song_media` is now called with `total=1` (previously
        // no track line appeared for songs).
        let line = "[INFO     12:00:02] [Track   1/1  ] Downloading \"Flowers\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo {
                track_number,
                track_total,
                ..
            } => {
                assert_eq!(track_number, Some(1));
                assert_eq!(track_total, Some(1));
            }
            other => panic!("Expected TrackInfo, got {other:?}"),
        }
    }

    #[test]
    fn v31_track_regex_does_not_match_dash_total_fallback() {
        // Defensive: `media_total or "-"` fallback. The `\d+/\d+`
        // pattern in `TRACK_INFO_V2_REGEX` is numeric-only, so this
        // line must NOT parse as `TrackInfo` (would give a bogus
        // `track_total`); it should surface as `Unknown` so the
        // activity log still displays the raw line.
        let line = "[INFO     12:00:02] [Track   1/-  ] Downloading \"Flowers\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo { .. } => {
                panic!("Track N/- must not parse as TrackInfo with numeric total");
            }
            // Any other event variant is acceptable — the contract is
            // "don't silently record a wrong track_total".
            _ => {}
        }
    }

    #[test]
    fn v31_url_parse_error_is_captured_as_error_not_warning() {
        // Upstream `fd3b621` upgraded URL parse errors to ERROR level.
        // The line must trigger `ERROR_PREFIX_REGEX` and produce an
        // `Error` event so `classify_error()` can route it. On v3.0
        // this line would have been WARNING and fallen through to
        // `Unknown`.
        let line = "[ERROR    12:00:01] [URL   1/1  ] Failed to parse URL: invalid scheme";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(
                    message.contains("Failed to parse URL"),
                    "message should carry the URL parse reason, got {message:?}"
                );
            }
            other => panic!("Expected Error, got {other:?}"),
        }
    }

    #[test]
    fn v31_padded_url_bracket_does_not_break_error_capture() {
        // The `[URL   1/1  ]` infix between the log level and the
        // message must not prevent the `ERROR_PREFIX_REGEX` capture.
        // This is the same guard as v3.0's #599 fix but on the
        // URL-context variant.
        let line = "[ERROR    17:09:24] [URL   1/1  ] Error processing \"https://example.com/bad\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { .. } => {}
            other => panic!("Expected Error for v3.1 [URL...] error line, got {other:?}"),
        }
    }

    #[test]
    fn v31_padded_track_bracket_error_still_captured() {
        // Exact shape per the upstream formatter. Kept separate from
        // the v3.0 test above because v3.1 is where we first exercise
        // single-track downloads with a `[Track   1/1  ]` prefix.
        let line = "[ERROR    17:09:23] [Track   1/1  ] Error downloading \"Flowers\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::Error { message } => {
                assert!(
                    message.contains("Error downloading") || message.contains("Flowers"),
                    "message should carry the track error, got {message:?}"
                );
            }
            other => panic!("Expected Error, got {other:?}"),
        }
    }

    #[test]
    fn v31_url_parse_error_classifies_as_not_found() {
        // Full pipeline check: the URL parse error message (once
        // stripped of the level prefix by `ERROR_PREFIX_REGEX`) must
        // land in a sensible bucket. "Failed to parse URL" contains
        // no "auth" / "network" / "codec" keywords but we accept
        // either `not_found` or `unknown` here — the exact mapping
        // is less important than "doesn't falsely match network".
        // Regression guard: #521's "httpx/httpcore" network rule
        // should NOT fire.
        let cat = classify_error("Failed to parse URL: invalid scheme");
        assert_ne!(cat, "network", "URL parse error must not be classified as network");
        assert_ne!(cat, "auth", "URL parse error must not be classified as auth");
    }

    // ========================================================================
    // GAMDL v3.2 regression tests (#615)
    //
    // v3.2 made two parser-adjacent changes:
    //
    //   1. `track_log.info(f'Downloading "{media_title}"')` in `cli.py` is now
    //      conditional on `download_item.media.partial` AND
    //      `media_type in {None, songs, library-songs, music-videos,
    //      library-music-videos, uploaded-videos}`. Wrapper media types
    //      (albums, playlists, artists) no longer emit the line.
    //   2. The exception class `GamdlDownloaderFlatFilterExcludedError` was
    //      renamed to `GamdlInterfaceFlatFilterExcludedError`.
    //
    // MeedyaDL matches neither by class name — the regex path targets the
    // bracketed `[Track N/M]` shape and the classifier targets substring
    // heuristics — so both changes should be transparent. These tests lock
    // that invariant in.
    //
    // Fixtures live in `.github/audits/fixtures/gamdl-3.2/`. Each `.log`
    // file captures a representative scenario with shape derived from the
    // v3.2 upstream source (`cli.py` + `cli/utils.py` — `custom_structlog_formatter`).
    // When real-sample captures from a live v3.2 run become available, they
    // should drop in as replacements for the synthesised fixtures without
    // any test rewrites; the assertions target structural properties
    // (counter values, event types), not exact whitespace.
    // ========================================================================

    /// Loads a fixture file from `.github/audits/fixtures/gamdl-3.2/` and
    /// returns its content as a `String`. Panics with a clear error if
    /// the fixture is missing — the tests that call this are specifically
    /// there to catch parser regressions, so a missing fixture is always
    /// a setup bug worth surfacing loudly.
    fn load_v32_fixture(name: &str) -> String {
        // `CARGO_MANIFEST_DIR` points at `src-tauri/`, so the repo root
        // is one level up. The fixtures directory is at
        // `<repo_root>/.github/audits/fixtures/gamdl-3.2/<name>`.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(".github")
            .join("audits")
            .join("fixtures")
            .join("gamdl-3.2")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("failed to load v3.2 fixture {name} at {path:?}: {e}")
        })
    }

    /// Iterates over lines in a fixture, parses each through
    /// `parse_gamdl_output`, and returns only the matching variant. Useful
    /// for extracting just the `TrackInfo` events from a long run-log.
    fn fixture_track_events(fixture: &str) -> Vec<(Option<u32>, Option<u32>, String)> {
        load_v32_fixture(fixture)
            .lines()
            .filter_map(|line| match parse_gamdl_output(line) {
                GamdlOutputEvent::TrackInfo {
                    track_number,
                    track_total,
                    title,
                    ..
                } => Some((track_number, track_total, title)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn v32_song_track_info_still_captured() {
        // Positive: a song-track `[Track N/M] Downloading "…"` line still
        // parses as TrackInfo. The media_type filter is applied upstream
        // of the log emission, so once MeedyaDL sees the line the parse is
        // identical to v3.1.
        let line = "[INFO     12:00:02] [Track   3/15 ] Downloading \"Song Three\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo {
                track_number,
                track_total,
                title,
                ..
            } => {
                assert_eq!(track_number, Some(3));
                assert_eq!(track_total, Some(15));
                assert_eq!(title, "Song Three");
            }
            other => panic!("Expected TrackInfo for v3.2 song line, got {other:?}"),
        }
    }

    #[test]
    fn v32_music_video_track_info_still_captured() {
        // Positive: music-video media-type is in the `partial` allowlist —
        // the line still fires and our regex still matches.
        let line = "[INFO     12:00:02] [Track   1/1  ] Downloading \"Vevo Live Cut\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo { title, .. } => {
                assert_eq!(title, "Vevo Live Cut");
            }
            other => panic!("Expected TrackInfo for v3.2 MV line, got {other:?}"),
        }
    }

    #[test]
    fn v32_album_wrapper_line_not_misclassified_as_track() {
        // Negative: a plain album-name banner line (no `[Track N/M]`) must
        // NOT match TRACK_INFO_V2_REGEX. v3.2 no longer emits this for
        // wrapper entities, but defensive testing ensures that if
        // something similar ever does land in stdout it doesn't silently
        // populate a bogus track counter.
        let line = "[INFO     12:00:01] Processing album \"Midnights (3am Edition)\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo { .. } => {
                panic!("Album-wrapper line must not parse as TrackInfo");
            }
            _ => {}
        }
    }

    #[test]
    fn v32_playlist_wrapper_line_not_misclassified_as_track() {
        // Same guard as the album case, for playlists. The `[URL N/M]`
        // prefix is for URL-level progress, not per-track.
        let line = "[INFO     12:00:01] [URL   1/1  ] Processing \"https://music.apple.com/.../pl.abc\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo { .. } => {
                panic!("URL-wrapper line must not parse as TrackInfo");
            }
            _ => {}
        }
    }

    #[test]
    fn v32_flat_filter_excluded_rename_invisible_to_classifier() {
        // v3.2 renamed the class. Neither the old nor the new name is
        // mentioned anywhere in the classifier, so both should fall
        // through to `unknown`. Locking this in here means a future
        // parser change that accidentally keys on the old name will fail
        // loudly instead of silently regressing.
        let old_name = "GamdlDownloaderFlatFilterExcludedError: 123456 already in DB";
        let new_name = "GamdlInterfaceFlatFilterExcludedError: 123456 already in DB";
        // Both should classify identically — whatever that is today.
        let cat_old = classify_error(old_name);
        let cat_new = classify_error(new_name);
        assert_eq!(
            cat_old, cat_new,
            "rename must not change classification bucket (old={cat_old}, new={cat_new})",
        );
        // And it must not be misclassified as a real error class.
        assert_ne!(cat_new, "auth", "flat-filter excluded is not an auth error");
        assert_ne!(cat_new, "network", "flat-filter excluded is not a network error");
        assert_ne!(cat_new, "io", "flat-filter excluded is not an I/O error");
    }

    #[test]
    fn v32_multi_track_album_counter_populates_for_every_track() {
        // End-to-end: synthesised v3.2 album output with a 3-track
        // album. Each `[Track N/M] Downloading "…"` line must produce a
        // TrackInfo with the correct counter.
        let lines = [
            "[INFO     12:00:02] [Track   1/3  ] Downloading \"Track One\"",
            "[INFO     12:00:03] [Track   2/3  ] Downloading \"Track Two\"",
            "[INFO     12:00:04] [Track   3/3  ] Downloading \"Track Three\"",
        ];
        let mut observed = Vec::new();
        for line in lines {
            if let GamdlOutputEvent::TrackInfo {
                track_number,
                track_total,
                title,
                ..
            } = parse_gamdl_output(line)
            {
                observed.push((track_number, track_total, title));
            }
        }
        assert_eq!(
            observed,
            vec![
                (Some(1), Some(3), "Track One".to_string()),
                (Some(2), Some(3), "Track Two".to_string()),
                (Some(3), Some(3), "Track Three".to_string()),
            ],
            "all three v3.2 album tracks must emit correctly-numbered TrackInfo events",
        );
    }

    // ----------------------------------------------------------------
    // Fixture-driven v3.2 tests (#615)
    //
    // The above inline-string tests pin the exact whitespace. The
    // fixture-driven tests below assert structural properties over
    // realistic multi-line captures, so they continue passing even if a
    // future real-sample capture tweaks alignment details.
    // ----------------------------------------------------------------

    #[test]
    fn v32_fixture_album_multi_track_counter_correct() {
        let events = fixture_track_events("album-multi-track-stderr.log");
        assert_eq!(
            events,
            vec![
                (Some(1), Some(3), "Opening Track".to_string()),
                (Some(2), Some(3), "Second Song".to_string()),
                (Some(3), Some(3), "Closing Track".to_string()),
            ],
            "3-track album fixture must produce exactly 3 TrackInfo events in order"
        );
    }

    #[test]
    fn v32_fixture_single_song_emits_one_track_event() {
        let events = fixture_track_events("single-song-stderr.log");
        assert_eq!(events.len(), 1, "single-song fixture must emit exactly one TrackInfo event");
        let (num, total, title) = &events[0];
        assert_eq!(*num, Some(1));
        assert_eq!(*total, Some(1));
        assert_eq!(title, "Standalone Single");
    }

    #[test]
    fn v32_fixture_music_video_emits_one_track_event() {
        let events = fixture_track_events("music-video-stderr.log");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, Some(1));
        assert_eq!(events[0].1, Some(1));
        assert!(events[0].2.contains("Live From London"));
    }

    #[test]
    fn v32_fixture_playlist_emits_four_track_events() {
        let events = fixture_track_events("playlist-stderr.log");
        assert_eq!(events.len(), 4, "4-track playlist fixture must emit exactly 4 TrackInfo events");
        for (i, (num, total, _)) in events.iter().enumerate() {
            assert_eq!(*num, Some((i + 1) as u32), "track {} number mismatch", i + 1);
            assert_eq!(*total, Some(4), "track {} total mismatch", i + 1);
        }
    }

    #[test]
    fn v32_fixture_flat_filter_excluded_does_not_break_track_counter() {
        // The `GamdlInterfaceFlatFilterExcludedError` warning line uses
        // the same `[Track N/M]` bracket shape as the Downloading line
        // (both pass through `custom_structlog_formatter`). The Track
        // event must not fire for the warning line (it lacks the
        // `Downloading "..."` suffix the regex requires).
        let events = fixture_track_events("flat-filter-excluded-stderr.log");
        assert!(
            events.is_empty(),
            "flat-filter fixture contains no `Downloading \"...\"` lines; no TrackInfo should fire — got {events:?}"
        );
    }

    #[test]
    fn v32_fixture_url_context_line_not_misclassified_as_track() {
        // Every fixture starts with `[URL   1/1  ] Processing "..."`.
        // Across all fixtures combined, none of these lines should
        // produce a TrackInfo — they are URL-level progress markers.
        for name in [
            "album-multi-track-stderr.log",
            "single-song-stderr.log",
            "music-video-stderr.log",
            "playlist-stderr.log",
            "flat-filter-excluded-stderr.log",
        ] {
            let content = load_v32_fixture(name);
            for line in content.lines() {
                if line.contains("[URL") && line.contains("Processing") {
                    match parse_gamdl_output(line) {
                        GamdlOutputEvent::TrackInfo { .. } => {
                            panic!("URL-context line from {name} must not parse as TrackInfo: {line}");
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
