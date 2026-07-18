// Copyright (c) 2026 MeedyaSuite
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
///
/// GAMDL v3.7.1 (upstream commit `1d00e74`+) introduced a `-` placeholder
/// for the total when `download_item.media.total` is `None` (per
/// `gamdl/cli/cli.py:240` — `media_total = download_item.media.total or
/// "-"`). This fires on single-track URLs and any media-fetch path that
/// hasn't enumerated the total upfront. The regex now accepts `\d+` OR
/// a literal `-` in the total slot; the v2-event parser maps `-` to
/// `None` so downstream code (progress bar, queue label) can degrade
/// gracefully rather than the line being silently ignored.
static TRACK_INFO_V2_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\[Track\s+(\d+)\s*/\s*(\d+|-)\s*\]\s+Downloading\s+"?([^"]+)"?"#)
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

/// Reports whether `line` is a recurring ffprobe demuxing-error noise
/// line that should be suppressed from the user-facing activity log
/// when verbose mode is off (#847).
///
/// During enrichment, MeedyaDL runs ffprobe several times per track
/// (codec detection, ReplayGain, mediainfo, …). Each invocation against
/// a freshly-written M4A occasionally trips an
/// "Invalid data found when processing input" warning that ffmpeg's
/// stderr emits for the partial-moov-atom case at byte 0. Downloads
/// complete fine; ffprobe falls through to a valid result on retry.
/// But the noise produces ~20 entries per album in the activity log —
/// reported in #847 as the most-aggravating recurring log noise.
///
/// Modelled exactly on [`is_python_traceback_noise`] (#660): the
/// on-disk activity-log writer still records the line regardless so
/// the forensic record stays complete; this helper just gates the
/// per-line `activity-log` Tauri event when verbose is off.
///
/// The match is intentionally tight on the recognisable prefix —
/// `[in#0/<demuxer-list> @ 0x…] Error during demuxing: ` — to avoid
/// suppressing genuine ffmpeg errors that happen to share the
/// "demuxing" / "invalid data" wording. The hex pointer in the
/// bracket varies per invocation so we match by structure
/// (`[in#0/…@ 0x…]` substring) rather than literal.
#[must_use]
pub fn is_ffprobe_demux_noise(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Required prefix shape — `[in#<digit>/<demuxer-list> @ 0x<hex>]`.
    // `in#0/mov,mp4,m4a,3gp,3g2,mj2` is the typical demuxer-list ffmpeg
    // selects for Apple-Music-shipped M4A files, but we keep this
    // generic so future demuxer-list shifts don't slip past the gate.
    let Some(after_in_marker) = trimmed.strip_prefix("[in#") else {
        return false;
    };
    let Some(after_at_sign) = after_in_marker.split_once(" @ 0x").map(|(_, r)| r) else {
        return false;
    };
    let Some(after_close_bracket) = after_at_sign.split_once("] ").map(|(_, r)| r) else {
        return false;
    };
    // Required tail — both substrings present (order-flexible). Apple's
    // ffmpeg has been stable on this exact wording since 5.x; if it
    // changes upstream this gate fails open and noise resumes — which
    // is the safe default vs accidentally suppressing genuine errors.
    after_close_bracket.starts_with("Error during demuxing: ")
        && after_close_bracket.contains("Invalid data found when processing input")
}

/// Reports whether `line` matches the Python exception summary pattern
/// (e.g. `TypeError: …`, `httpx.ConnectError: Connection refused`).
///
/// Wraps [`PYTHON_EXCEPTION_REGEX`] for callers outside this module — the
/// traceback diagnostic capture (#758) needs to recognise the closing
/// line of a traceback group so it can package the header + frames + tail
/// into a single forensic record.
#[must_use]
pub fn is_python_exception_summary(line: &str) -> bool {
    let trimmed = line.trim();
    PYTHON_EXCEPTION_REGEX.is_match(trimmed)
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

    /// A per-track codec-availability skip emitted by GAMDL when Apple
    /// Music's catalog does not offer the requested format(s) for a
    /// specific track (#698). Canonical shape:
    ///
    /// ```text
    /// [WARNING 22:32:23] [Track 23/24] Skipping "Die Young (Deconstructed Mix)":
    /// Requested format is not available (media ID: 592365442):
    /// [<SongCodec.ATMOS: 'atmos'>, <SongCodec.ALAC: 'alac'>, ...]
    /// ```
    ///
    /// This is **not** a download failure — it's normal catalog behaviour
    /// (live mixes, deconstructed mixes, anniversary editions etc. often
    /// don't have ATMOS / Lossless variants). Routing this to a dedicated
    /// variant lets the queue distinguish "Apple does not offer this in
    /// the requested format" from "the download infrastructure failed",
    /// which materially changes the user-facing terminal-state message.
    ///
    /// Previously these lines were caught by Priority 7's `"skipping"`
    /// keyword and emitted as `Error`, polluting the queue item's error
    /// field with the misleading text "Download completed but no output
    /// files were produced: [WARNING] ...". With this variant, the
    /// downstream classifier can produce a meaningful "No audio available
    /// in your requested formats" message instead.
    CodecSkip {
        /// The raw warning line, with the `[WARNING ...]` banner
        /// stripped so the message reads cleanly in the activity log.
        message: String,
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
///    - 3c. Python traceback frames (header / `File "..."` / caret lines) — #660
/// 4. Explicit errors (ERROR/Error prefix)
///    - 4b. Python exception summary line (`TypeError: ...`)
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

    // Priority 6b: Per-track codec-availability skip (#698).
    //
    // GAMDL emits lines like:
    //   `[WARNING 22:32:23] [Track 23/24] Skipping "Die Young (...)":
    //   Requested format is not available (media ID: ...): [...codecs...]`
    //
    // when Apple Music's catalog does not offer the requested format(s)
    // for a specific track. This is normal catalog behaviour, not a
    // download failure — many tracks (live mixes, deconstructed mixes,
    // anniversary editions) don't have ATMOS / Lossless variants.
    //
    // Catching this before Priority 7's keyword match prevents the line
    // from being emitted as `Error`, which would otherwise pollute the
    // queue's error field with the misleading text "Download completed
    // but no output files were produced: [WARNING] ...".
    if is_codec_skip_line(trimmed) {
        return GamdlOutputEvent::CodecSkip {
            message: trimmed.to_string(),
        };
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
    //   - "no entry"         -- missing archive entries or config keys
    //   - "exception"        -- Python exception messages
    //
    // The `"skipping"` keyword used to live here too, but it was too
    // broad — every per-track codec-availability warning matched and
    // bubbled up as `Error`. Those warnings are now classified by
    // Priority 6b above as `CodecSkip` and routed through a dedicated
    // path that does not pollute the queue's error field (#698). Other
    // GAMDL "Skipping" emissions (rate-limit retries, pre-existing
    // file detection, etc.) do not contain the canonical "format is
    // not available" / "requested format" phrase and so still fall
    // through to the keyword match below if they're truly errors.
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
/// Detects per-track codec-availability skip lines emitted by GAMDL (#698).
///
/// Canonical shape:
/// ```text
/// [WARNING 22:32:23] [Track 23/24] Skipping "Die Young (Deconstructed Mix)":
///     Requested format is not available (media ID: 592365442):
///     [<SongCodec.ATMOS: 'atmos'>, <SongCodec.ALAC: 'alac'>, ...]
/// ```
///
/// This is **not** a download failure — it's normal Apple Music catalog
/// behaviour. Detecting these lines upstream of Priority 7's generic error
/// keyword match lets us route them to a dedicated `CodecSkip` event so
/// the queue's terminal-state classifier can produce a meaningful "no
/// audio available in your requested formats" message instead of the
/// misleading "Download completed but no output files were produced".
///
/// The match conditions are deliberately narrow: the line must contain
/// **both** a `Skipping`-style verb AND a phrase indicating format
/// unavailability. Any other "Skipping" line (rate-limit retries,
/// pre-existing file detection, etc.) falls through to other parser
/// branches and is classified normally.
#[must_use]
pub fn is_codec_skip_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    let has_skip_verb = lower.contains("skipping") || lower.contains("skipped");
    let has_format_unavailable = lower.contains("requested format is not available")
        || lower.contains("format is not available")
        || lower.contains("format not available")
        || lower.contains("requested format");
    has_skip_verb && has_format_unavailable
}

/// Detects whether an already-collected error/warning message is a codec
/// skip — used by the queue's terminal-state classifier to decide whether
/// every recorded "error" is actually just a normal Apple Music catalog
/// limitation. Same predicate as [`is_codec_skip_line`]; aliased here so
/// the queue's intent reads clearly at the call site.
#[must_use]
pub fn is_codec_skip_message(message: &str) -> bool {
    is_codec_skip_line(message)
}

/// Humanises a GAMDL "codec skip" line for the activity log UI.
///
/// GAMDL emits these as:
///
/// ```text
/// [WARNING 13:21:56] [Track 1/1] Skipping "Pickle (3ballMTY Remix)":
///     Requested format is not available (media ID: 1578734917):
///     [<SongCodec.AC3: 'ac3'>]
/// ```
///
/// The track title is already in quotes earlier on the line, so the
/// `(media ID: <numeric_id>)` portion is informational noise — it
/// gives a downstream debugger a way to look up the song in Apple's
/// catalog but provides no signal to a regular user reading the log.
/// The `[<SongCodec.AC3: 'ac3'>]` list is also Python's repr format
/// rather than a friendly codec name.
///
/// Transformation (Phase 3.5h, 2026-05-08 user request):
/// ```text
/// [WARNING 13:21:56] [Track 1/1] Skipping "Pickle (3ballMTY Remix)":
///     ac3 not available
/// ```
///
/// Idempotent: running this on an already-humanised line is a no-op.
/// Returns the unchanged input when the line doesn't match the codec-
/// skip shape, so callers can apply it unconditionally.
/// Maps a GAMDL codec CLI identifier (lowercase `atmos`, `aac-legacy`, etc.)
/// to a user-facing display label (`Atmos`, `AAC Legacy`, etc.) for use in
/// the activity log.
///
/// Mirrors the labels in [`crate::models::gamdl_options::SongCodec::display_name`]
/// but without the trailing `(Experimental)` annotation that GAMDL adds
/// internally and that's redundant noise in the activity log (the line is
/// ALREADY telling the user the codec isn't available, so the
/// "Experimental" tag adds zero information). Codecs not in the
/// lookup table fall back to a defensive title-cased form — keeps the
/// function future-proof if upstream adds a new codec we haven't
/// registered yet.
///
/// Used by [`humanise_codec_skip_line`] (#832) to replace lowercase
/// enum identifiers with the proper display labels users see elsewhere
/// in the UI.
#[must_use]
fn pretty_codec_label(cli_id: &str) -> String {
    match cli_id {
        "atmos" => "Atmos".to_string(),
        "alac" => "ALAC".to_string(),
        "ac3" => "AC3".to_string(),
        "aac" => "AAC".to_string(),
        "aac-legacy" => "AAC Legacy".to_string(),
        "aac-he" => "HE-AAC".to_string(),
        "aac-binaural" => "AAC Binaural".to_string(),
        "aac-downmix" => "AAC Downmix".to_string(),
        // Defensive fallback: title-case and replace `-` with space.
        // Means a new codec upstream produces `Some-New-Codec` rather
        // than the raw `some-new-codec` if it sneaks through before
        // we add a mapping.
        other => other
            .split('-')
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

#[must_use]
pub fn humanise_codec_skip_line(line: &str) -> String {
    if !is_codec_skip_line(line) {
        return line.to_string();
    }

    // Strip "(media ID: <digits>)" — the parens and any internal
    // whitespace. Tolerate variations in spacing.
    let media_id_re = match regex::Regex::new(r"\s*\(media ID:\s*\d+\)") {
        Ok(re) => re,
        Err(_) => return line.to_string(), // shouldn't happen; defensive
    };
    let mut out = media_id_re.replace_all(line, "").to_string();

    // #832: also strip GAMDL 3.x's verbose
    // `(Unavailable requested format candidates: Dolby Atmos
    // (Experimental) [atmos] -> Lossless (ALAC) (Experimental)
    // [alac] -> …)` parenthetical. Pre-fix this line was 200+
    // characters wide in the activity log and contained the exact
    // same codec list a second time, just with the verbose
    // "(Experimental)" annotations. The shorter
    // `atmos, alac, ac3, aac, aac-legacy not available` summary at
    // the start of the line already communicates everything.
    // Greedy `.*\)$` rather than `[^)]*\)` because GAMDL 3.x nests
    // parens inside the parenthetical itself (e.g. "Dolby Atmos
    // (Experimental)"). The greedy match anchored at end-of-line
    // consumes the whole verbose block in one go. The activity-log
    // is line-delimited so there's no risk of swallowing content
    // from a following line.
    let unavailable_re =
        match regex::Regex::new(r"\s*\(Unavailable requested format candidates:.*\)$") {
            Ok(re) => re,
            Err(_) => return out,
        };
    out = unavailable_re.replace_all(&out, "").to_string();

    // Replace the Python-repr codec list with friendly display labels.
    // `[<SongCodec.AC3: 'ac3'>, <SongCodec.ATMOS: 'atmos'>]`
    // → `AC3, Atmos`. Single-quoted name inside each entry is the
    // GAMDL CLI identifier; pass each through `pretty_codec_label` so
    // users see "Atmos" / "AAC Legacy" / etc. rather than the raw
    // enum forms.
    let codec_re = match regex::Regex::new(r"\[\s*(?:<SongCodec\.[A-Z_0-9]+:\s*'([a-z0-9_-]+)'>\s*,?\s*)+\]") {
        Ok(re) => re,
        Err(_) => return out,
    };
    if let Some(captures_iter) = codec_re.captures_iter(&out.clone()).next() {
        // Walk all captures inside the matched bracket region by
        // re-running a simpler per-codec regex.
        let inner_re = regex::Regex::new(r"<SongCodec\.[A-Z_0-9]+:\s*'([a-z0-9_-]+)'>")
            .ok();
        if let Some(inner) = inner_re {
            let codecs: Vec<String> = inner
                .captures_iter(captures_iter.get(0).map_or("", |m| m.as_str()))
                .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                .collect();
            if !codecs.is_empty() {
                let pretty: Vec<String> = codecs.iter().map(|c| pretty_codec_label(c)).collect();
                let friendly = format!("{} not available", pretty.join(", "));
                out = codec_re.replace(&out, friendly.as_str()).to_string();
            }
        }
    }

    // GAMDL 3.x multi-codec line shape (companion mode "all formats
    // failed"): the codec list appears as a lowercase comma-separated
    // run BEFORE the parenthetical we already stripped — e.g.
    // `Requested format is not available: atmos, alac, ac3, aac,
    // aac-legacy not available`. Match the exact "<list> not available"
    // suffix and rewrite the list with pretty labels. Pinning to the
    // "not available" tail keeps this from accidentally rewriting other
    // codec-shaped substrings elsewhere in the line.
    let lc_list_re =
        match regex::Regex::new(r"([a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:,\s*[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+)\s+not available") {
            Ok(re) => re,
            Err(_) => return out,
        };
    if let Some(caps) = lc_list_re.captures(&out.clone()) {
        if let Some(list_match) = caps.get(1) {
            let list_text = list_match.as_str();
            let codec_tokens: Vec<&str> =
                list_text.split(',').map(str::trim).filter(|t| !t.is_empty()).collect();
            // Only rewrite when every token is a known codec — avoids
            // accidentally rewriting e.g. "errno 1, errno 2" or any other
            // comma-separated lowercase run that happens to share the
            // pattern.
            let all_known_codecs = !codec_tokens.is_empty()
                && codec_tokens.iter().all(|t| {
                    matches!(
                        *t,
                        "atmos"
                            | "alac"
                            | "ac3"
                            | "aac"
                            | "aac-legacy"
                            | "aac-he"
                            | "aac-binaural"
                            | "aac-downmix"
                    )
                });
            if all_known_codecs {
                let pretty: Vec<String> = codec_tokens
                    .iter()
                    .map(|t| pretty_codec_label(t))
                    .collect();
                let replacement = format!("{} not available", pretty.join(", "));
                out = lc_list_re.replace(&out, replacement.as_str()).to_string();
            }
        }
    }

    // Tidy up the leftover ": :" pattern that results from stripping
    // "(media ID: …)" between two colons. (Don't try to collapse runs
    // of whitespace here — `str::replace("  ", " ")` is non-idempotent
    // for runs of 3+ spaces, which would break the
    // `humanise_is_idempotent` invariant. GAMDL's original spacing is
    // good enough for the activity log.)
    out = out.replace(": :", ":");

    // Strip trailing colon when nothing follows the explanation
    // (cosmetic; e.g. when the bracket part wasn't matched and got
    // stripped some other way).
    out.trim_end_matches([':', ' ']).to_string()
}

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
    //
    // GAMDL v3.7.1 (upstream commit `1d00e74`) refactored its yt-dlp call
    // path to use `HlsFD` / `HttpFD` directly and raise bare RuntimeError
    // strings on failure:
    //   * "yt-dlp HLS download failed"
    //   * "yt-dlp HTTP download failed"
    // These represent transport-level failures (the underlying cause —
    // 403, connection refused, DNS, etc. — appears in the traceback but
    // the surface message is just the RuntimeError text). Classify as
    // network so the existing retry-on-network logic kicks in and the
    // user sees the right "Check your connection" guidance.
    } else if lower.contains("network")
        || lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection")
        || lower.contains("connecterror")
        || lower.contains("dns")
        || lower.contains("httpx")
        || lower.contains("httpcore")
        || lower.contains("yt-dlp hls download failed")
        || lower.contains("yt-dlp http download failed")
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
    // Library webplayback KeyError (#570). Apple Music's library
    // webplayback endpoint returns a different response shape than
    // the catalog endpoint, and GAMDL's `interface_song.py:179`
    // unconditionally dereferences `webplayback["songList"][0]…` —
    // raises `KeyError: 'songList'` on every library track. Matched
    // before the generic "unknown" bucket so users get an
    // actionable message rather than a raw Python traceback excerpt.
    } else if is_library_webplayback_keyerror(error_message) {
        "library_webplayback_keyerror"
    // Media unstreamable (#898): GAMDL 3.7.2 (songs) and 3.7.3 (music-videos)
    // added defensive `.get("playParams", {})` access so the existing
    // `GamdlInterfaceMediaNotStreamableError` now surfaces reliably for
    // content that's been removed, region-locked, or is library-only.
    // Matched before the generic `not_found` substring fallback so the
    // user sees the specific "not streamable" guidance instead of the
    // broader "content may be removed" message.
    } else if is_media_not_streamable_error(error_message) {
        "media_not_streamable"
    // Wrapper-v2 / GAMDL version skew: GAMDL 3.8.2 hard-requires wrapper-v2
    // 0.0.2 (and moved decrypt off HTTP `POST /decrypt`), while MeedyaDL's
    // current ceiling (GAMDL <= 3.8.1) needs the older wrapper-v2 0.0.1.
    // Matched before the generic `not_found` fallback (same ordering
    // discipline as `media_not_streamable`) so a reverse-skew "404" on the
    // removed `/decrypt` endpoint gets the specific wrapper-version guidance
    // instead of the generic "content not found" message.
    } else if is_wrapper_version_mismatch_error(error_message) {
        "wrapper_version_mismatch"
    // Wrapper-v2 decrypt not ready (#319): the daemon is up + authenticated
    // but FairPlay playback isn't initialised (`runtime.playback_ready` is
    // false), so GAMDL 3.8.2+ raises `wrapper-v2: decrypt unavailable (503)`
    // mid-download for non-web codecs. Distinct from a version mismatch — the
    // daemon version is fine, it just isn't ready. Matched before the generic
    // fallback so the user gets "restart the daemon" guidance, not a raw error.
    } else if lower.contains("decrypt unavailable") {
        "wrapper_decrypt_unavailable"
    // Content not found: the URL is invalid or the content was removed.
    } else if lower.contains("not found") || lower.contains("404") || lower.contains("no results") {
        "not_found"
    // Rate limiting: the server is throttling requests; retry after delay.
    } else if lower.contains("rate limit") || lower.contains("429") || lower.contains("too many") {
        "rate_limit"
    // License declined (#307): Apple's license-exchange endpoint refused a
    // playback license for a specific track with a NON-429 status (e.g.
    // `Status code: 200` carrying `status:-1002`). The 429 rate-limit case is
    // already caught above; this catches the "Apple just won't license this
    // one track" case so the user gets codec/storefront guidance instead of a
    // raw traceback. Keyed on the endpoint phrase GAMDL surfaces.
    } else if lower.contains("license exchange") {
        "license_declined"
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
        "rate_limit" => "Apple Music is rate-limiting license requests (HTTP 429) — a per-account, server-side throttle that usually clears after 1–2+ hours, not minutes. MeedyaDL has paused the queue so it stops adding to the throttle; resume it from the Queue page once the cooldown lifts. Smaller batches help avoid it, and already-downloaded files are preserved (so a later run only re-fetches what's missing).",
        "license_declined" => "Apple declined a playback license for this track — this is NOT a rate limit. It's usually codec- or region-specific: try a different codec (e.g. AAC), a different storefront, or skip this track.",
        "tool" => "A required tool (FFmpeg, mp4decrypt, etc.) may be missing or outdated. Check Settings > Tools.",
        "playlist_title_keyerror" => "Some tracks in this playlist are missing required metadata — this is a known upstream GAMDL limitation with certain Apple Music Classical playlists (see issue #588). Try downloading the individual albums instead, or report it upstream at https://github.com/glomatico/gamdl/issues.",
        "library_webplayback_keyerror" => "Library URLs (music.apple.com/.../library/albums/l.XXXX) use a different Apple Music API endpoint than catalog URLs and aren't fully supported by GAMDL yet — it expects a 'songList' field that the library endpoint doesn't return (issue #570). Download the catalog version of the album instead by searching for it on music.apple.com, or report the gap upstream at https://github.com/glomatico/gamdl/issues.",
        "media_not_streamable" => "Apple Music says this content isn't streamable — it may have been removed, isn't licensed in your storefront, or is a personal-library upload that catalog tooling can't fetch. Try the catalog URL for the same album in a different storefront, or pick a release that's still available.",
        "wrapper_version_mismatch" => "GAMDL and the wrapper-v2 daemon must be upgraded together. GAMDL 3.8.2+ requires wrapper-v2 0.0.2 (native TCP decrypt, on its own host/port); GAMDL 3.6–3.8.1 require wrapper-v2 0.0.1 (HTTP decrypt). Check your GAMDL version and rebuild your wrapper-v2 container to the matching daemon version.",
        "wrapper_decrypt_unavailable" => "The wrapper-v2 daemon is signed in but its FairPlay decryptor isn't ready, so ALAC/Atmos/AC3 (non-web codecs) can't be decrypted. Restart the wrapper-v2 daemon and check its logs for Apple-library initialisation, then retry. AAC (aac-web) downloads without the decryptor and is unaffected.",
        _ => "Check the Activity Log for more details. If this persists, report it via Settings > Advanced > Error Reporting.",
    }
}

/// Detect a "wrong storefront" failure shape from GAMDL (#666).
///
/// Returns `true` when the error message looks like the AMP API responded
/// with `Resource Not Found` to a catalog query keyed by storefront — i.e.
/// the album / song / video isn't published in the URL's storefront
/// (typically because the user pasted a `/us/` link while their account
/// is in another region).
///
/// Captured shapes this matches (real user history, 2026-04-30):
/// * `gamdl.api.exceptions.GamdlApiResponseError: Error fetching from AMP API (Status code: 404): {"errors":[{"id":"…","title":"Resource Not Found","detail":"Resource with requested key …"}]}`
/// * Plain stderr `404 Resource Not Found` lines from the catalog probe.
///
/// Deliberately narrower than [`classify_error`]'s `not_found` bucket —
/// that bucket also covers user-typed bad URLs, deleted content, and
/// stale shared links where a storefront retry won't help. The retry
/// path uses *this* helper to gate the rewrite, so we only retry when
/// the evidence specifically suggests "the URL is fine, the storefront
/// is wrong."
///
/// We require BOTH `404` AND `Resource Not Found` (case-insensitive) so
/// that a generic "404 page not found" from an unrelated subprocess
/// (e.g. an updater asset fetch) doesn't trigger the rewrite.
#[must_use]
pub fn is_storefront_mismatch_error(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    (lower.contains("404") || lower.contains("status code: 404"))
        && lower.contains("resource not found")
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

/// Detect GAMDL's library-URL `KeyError: 'songList'` traceback (#570).
///
/// Captured 2026-04-23 during #546 scenario 1 repro on a personal-
/// library album URL (`music.apple.com/.../library/albums/l.XXXX`).
/// Apple Music's library webplayback endpoint returns a different
/// response shape than the catalog endpoint, but GAMDL's
/// `interface_song.py:179` unconditionally dereferences
/// `webplayback["songList"][0]["assets"][0]["metadata"]` regardless
/// of which endpoint produced the response — raises
/// `KeyError: 'songList'` on every library track.
///
/// Matches the exact traceback signature to avoid false positives on
/// other `KeyError: 'songList'` causes — we require BOTH the key
/// name and the GAMDL-side filepath token so unrelated key-error
/// shapes from a downstream tool aren't misclassified.
#[must_use]
pub fn is_library_webplayback_keyerror(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    lower.contains("keyerror: 'songlist'")
        && (lower.contains("interface_song")
            || lower.contains("get_tags")
            || lower.contains("webplayback"))
}

/// Detect GAMDL's `GamdlInterfaceMediaNotStreamableError` (#898).
///
/// `GamdlInterfaceMediaNotStreamableError("Media is not streamable: <id>")`
/// fires from `interface/song.py` and `interface/music_video.py` whenever
/// `is_media_streamable(media_metadata)` returns false — typically because
/// the song / video has been removed from Apple Music, is region-locked
/// out of the user's storefront, or is a library-only upload (for
/// music-videos, the `is_library` branch also raises this same error).
///
/// The error string has existed since at least GAMDL 3.5.2, but on
/// 3.7.1 and earlier it was masked for songs / library-only MVs by an
/// upstream `KeyError: 'playParams'` traceback. GAMDL 3.7.2 (songs) and
/// 3.7.3 (music-videos) added defensive `.get("playParams", {})` access,
/// so this user-facing message now surfaces reliably for affected items.
///
/// Without this matcher the error falls through `classify_error` to the
/// generic `unknown` bucket, leaving the user with no actionable guidance.
/// We classify it as `media_not_streamable` so the activity log can
/// surface a specific recovery suggestion (try a different URL or
/// storefront; the content may be removed / region-locked / library-only).
///
/// Matches the case-insensitive literal `media is not streamable` so it
/// catches the raw exception message regardless of whether it appears
/// inside a Python traceback (`GamdlInterfaceMediaNotStreamableError:
/// Media is not streamable: 1234567890`) or a bare stderr line from
/// `extract_python_exception`.
#[must_use]
pub fn is_media_not_streamable_error(error_message: &str) -> bool {
    error_message
        .to_lowercase()
        .contains("media is not streamable")
}

/// Detect a wrapper-v2 / GAMDL version-skew failure (2026-07).
///
/// GAMDL 3.8.2 hard-requires wrapper-v2 0.0.2 — it exact-matches the
/// `version` field of `GET /me`'s response at CLI startup and exits
/// immediately on a mismatch. The same release also moved decryption
/// from the HTTP `POST /decrypt` endpoint to a native TCP protocol.
/// Because GAMDL and wrapper-v2 are independent projects with no shared
/// release cadence, users can easily end up with a mismatched pair:
///
/// * **Forward skew** — GAMDL 3.8.2 against wrapper-v2 <= 0.0.1: GAMDL
///   exits at startup with `Unsupported wrapper-v2 API version. gamdl
///   requires wrapper-v2 0.0.2`.
/// * **Reverse skew** — GAMDL <= 3.8.1 (MeedyaDL's current ceiling)
///   against wrapper-v2 0.0.2: the removed HTTP endpoint yields
///   something like `wrapper-v2: POST /decrypt failed HTTP 404` at
///   decrypt time.
///
/// Without this matcher both shapes fall through `classify_error` to
/// the generic `unknown` bucket, leaving the user with no signal that
/// the fix is to align GAMDL and wrapper-v2 versions rather than retry
/// or swap codecs. We classify both as `wrapper_version_mismatch` so
/// the activity log can surface the specific upgrade-together guidance.
///
/// Matches either the literal forward-skew message, or the combination
/// of `/decrypt` + `404` for the reverse-skew shape — requiring both
/// substrings avoids false positives on unrelated 404s (e.g. a wrong
/// storefront `Resource Not Found`) that happen to also mention
/// `/decrypt` in a stack frame path.
#[must_use]
pub fn is_wrapper_version_mismatch_error(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    lower.contains("unsupported wrapper-v2 api version")
        || (lower.contains("/decrypt") && lower.contains("404"))
}

/// Detect GAMDL's music-video cover-art URL templating bug.
///
/// GAMDL fetches per-track cover art for music videos from
/// `https://a1.mzstatic.com/Video.../<id>.jpg/{w}x{h}mv.jpg`, where
/// `{w}` and `{h}` are placeholder tokens GAMDL is supposed to
/// substitute with concrete pixel dimensions before issuing the HTTP
/// request. On music-video albums (or any album whose tracks include
/// a (Visualizer) entry) GAMDL skips that substitution and sends the
/// literal `{w}x{h}` to Apple's CDN, which responds with
/// `400 Bad Request`. Every track that hits this code path fails
/// without ever attempting the audio download, so the user's run
/// produces 0 output files and a `GAMDL reported N per-track error(s)`
/// soft-error count where N == the music-video track count.
///
/// Captured shape (from a real user run, 2026-05-02):
/// ```text
/// httpx.HTTPStatusError: Client error '400 Bad Request' for url
/// 'https://a1.mzstatic.com/Video221/v4/.../1968719474350101.jpg/%7Bw%7Dx%7Bh%7Dmv.jpg'
/// ```
/// Note `%7B` / `%7D` are the URL-encoded `{` / `}`. We match BOTH
/// the encoded and the raw forms since either could appear depending
/// on whether httpx percent-encoded before raising.
///
/// Returns `true` when the buffer contains the bug's signature so the
/// caller can surface a focused message ("known GAMDL bug — music-
/// video cover URL not templated; report to glomatico/gamdl") instead
/// of the generic per-track-error count.
#[must_use]
pub fn is_gamdl_mv_cover_template_bug(error_message: &str) -> bool {
    let lower = error_message.to_lowercase();
    let has_400 = lower.contains("400 bad request") || lower.contains("status code: 400");
    let has_mv_url = lower.contains("mzstatic.com/video");
    let has_unsubstituted_template = lower.contains("%7bw%7dx%7bh%7d")
        || lower.contains("{w}x{h}");
    has_400 && has_mv_url && has_unsubstituted_template
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
    fn parses_v2_track_info_with_dash_total_from_gamdl_v3_7_1() {
        // GAMDL v3.7.1 renders `download_item.media.total or "-"` so a
        // single-track URL produces `[Track 1/-]` instead of `[Track 1/12]`.
        // Pre-fix TRACK_INFO_V2_REGEX rejected this line outright; the
        // queue label + progress-bar caption stayed blank for the whole
        // download. Now the regex matches and `track_total` parses to
        // None, which downstream consumers already handle.
        let line = "[INFO     12:34:56] [Track   1/-  ] Downloading \"Anti-Hero\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo {
                title,
                track_number,
                track_total,
                ..
            } => {
                assert_eq!(title, "Anti-Hero");
                assert_eq!(track_number, Some(1));
                assert_eq!(track_total, None, "`-` total must parse to None");
            }
            other => panic!("Expected TrackInfo, got {:?}", other),
        }
    }

    #[test]
    fn parses_v2_track_info_with_numeric_total_still_works() {
        // Regression-guard: the v3.7.1 fix must not break the
        // pre-existing numeric-total path.
        let line = "[INFO     12:34:56] [Track   2/12 ] Downloading \"Some Track\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo {
                title,
                track_number,
                track_total,
                ..
            } => {
                assert_eq!(title, "Some Track");
                assert_eq!(track_number, Some(2));
                assert_eq!(track_total, Some(12));
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

    // ----------------------------------------------------------
    // ffprobe demuxing-noise detector tests (#847)
    // ----------------------------------------------------------

    #[test]
    fn is_ffprobe_demux_noise_recognises_canonical_apple_music_shape() {
        // Verbatim from the #847 issue report — fires every track during
        // enrichment.
        assert!(is_ffprobe_demux_noise(
            "[in#0/mov,mp4,m4a,3gp,3g2,mj2 @ 0x954c14000] Error during demuxing: Invalid data found when processing input"
        ));
    }

    #[test]
    fn is_ffprobe_demux_noise_tolerates_leading_whitespace_and_alt_hex_widths() {
        // Some ffmpeg builds prefix the line with a single space; some
        // hex pointers are wider on 64-bit platforms.
        assert!(is_ffprobe_demux_noise(
            "  [in#0/mov,mp4,m4a,3gp,3g2,mj2 @ 0x7b8c1c000] Error during demuxing: Invalid data found when processing input"
        ));
        assert!(is_ffprobe_demux_noise(
            "[in#0/mov,mp4,m4a,3gp,3g2,mj2 @ 0xff00ff00ff00ff00] Error during demuxing: Invalid data found when processing input"
        ));
    }

    #[test]
    fn is_ffprobe_demux_noise_recognises_alternate_demuxer_lists() {
        // Future-proofing — ffmpeg's auto-selected demuxer list may
        // change as more formats are added. The match keys on the
        // structural `[in#<digit>/<list> @ 0x<hex>]` shape, not the
        // exact comma-separated demuxer names.
        assert!(is_ffprobe_demux_noise(
            "[in#0/aac @ 0xabc123] Error during demuxing: Invalid data found when processing input"
        ));
        assert!(is_ffprobe_demux_noise(
            "[in#1/wav,mp3 @ 0x1234] Error during demuxing: Invalid data found when processing input"
        ));
    }

    #[test]
    fn is_ffprobe_demux_noise_rejects_genuine_ffmpeg_errors() {
        // Other ffmpeg errors that happen to share words must NOT be
        // suppressed — they're rare but actionable when they fire.
        // Pattern requires BOTH "Error during demuxing:" prefix AND
        // "Invalid data found when processing input" — so e.g. a
        // muxer error or a generic "Invalid data" without the demuxing
        // prefix passes through.
        assert!(!is_ffprobe_demux_noise(
            "[in#0/mov,mp4,m4a,3gp,3g2,mj2 @ 0x954c14000] Error muxing packet (track 2)"
        ));
        assert!(!is_ffprobe_demux_noise(
            "[in#0/mov @ 0x1234] Could not find codec parameters for stream 0"
        ));
        assert!(!is_ffprobe_demux_noise(
            "Invalid data found when processing input"
        ));
        // No `[in#…]` prefix → not the noise we're after.
        assert!(!is_ffprobe_demux_noise(
            "Error during demuxing: Invalid data found when processing input"
        ));
    }

    #[test]
    fn is_ffprobe_demux_noise_rejects_unrelated_lines() {
        assert!(!is_ffprobe_demux_noise(""));
        assert!(!is_ffprobe_demux_noise("  "));
        assert!(!is_ffprobe_demux_noise(
            "[INFO     12:34:56] [Track 1/12] Downloading \"Hello\""
        ));
        assert!(!is_ffprobe_demux_noise(
            "Traceback (most recent call last):"
        ));
        // Line that LOOKS like a bracketed prefix but uses a different
        // shape (e.g. structlog's `[level    HH:MM:SS]` prefix on
        // GAMDL v3.0+ wrapped lines).
        assert!(!is_ffprobe_demux_noise(
            "[INFO     12:34:56] [in#0/mov,mp4 @ 0x1234] Error during demuxing: Invalid data found when processing input"
        ));
    }

    #[test]
    fn is_ffprobe_demux_noise_requires_zero_x_hex_pointer_form() {
        // ffmpeg always prints the pointer as `0x<hex>`; if upstream
        // ever switches format the gate fails open (noise resumes)
        // rather than over-suppressing.
        assert!(!is_ffprobe_demux_noise(
            "[in#0/mov,mp4,m4a,3gp,3g2,mj2 @ 12345] Error during demuxing: Invalid data found when processing input"
        ));
    }

    // ----------------------------------------------------------
    // Storefront-mismatch detector tests (#666)
    // ----------------------------------------------------------

    #[test]
    fn is_storefront_mismatch_recognises_amp_404_shape() {
        // Captured user evidence (2026-04-30, history.json line 23 Apr 23:17):
        let msg = r#"Download completed but no output files were produced: gamdl.api.exceptions.GamdlApiResponseError: Error fetching from AMP API (Status code: 404): {"errors":[{"id":"NJWPW6PVGQY53KULKAYYOVHBMI","title":"Resource Not Found","detail":"Resource with requeste..."#;
        assert!(is_storefront_mismatch_error(msg));
    }

    #[test]
    fn is_storefront_mismatch_recognises_plain_404_resource_not_found() {
        assert!(is_storefront_mismatch_error("404 Resource Not Found"));
        assert!(is_storefront_mismatch_error("status code: 404 — Resource Not Found"));
    }

    #[test]
    fn is_storefront_mismatch_requires_both_signals() {
        // Plain 404 without "Resource Not Found" must NOT trigger the rewrite —
        // could be an updater asset 404, a help-server 404, or any unrelated
        // HTTP failure. We don't want to burn the budget on those.
        assert!(!is_storefront_mismatch_error("HTTP 404 Page Not Found"));
        // "Resource Not Found" alone (without 404) shouldn't trigger either —
        // it's a strong AMP signal but the status code is the canonical proof.
        assert!(!is_storefront_mismatch_error("Resource Not Found"));
    }

    #[test]
    fn is_storefront_mismatch_rejects_unrelated_errors() {
        assert!(!is_storefront_mismatch_error("network timeout"));
        assert!(!is_storefront_mismatch_error("codec atmos not available"));
        assert!(!is_storefront_mismatch_error(
            "GAMDL reported 1 per-track error(s) even though the process exited 0"
        ));
    }

    // ----------------------------------------------------------
    // GAMDL music-video cover-template bug detector tests
    // ----------------------------------------------------------

    #[test]
    fn is_gamdl_mv_cover_template_bug_recognises_url_encoded_placeholders() {
        // Captured user evidence (2026-05-02) — httpx percent-encoded the URL:
        let msg = "httpx.HTTPStatusError: Client error '400 Bad Request' for url 'https://a1.mzstatic.com/Video221/v4/78/43/07/78430707-4e2a-0d51-0fb0-c49607dfe652/1968719474350101.jpg/%7Bw%7Dx%7Bh%7Dmv.jpg'";
        assert!(is_gamdl_mv_cover_template_bug(msg));
    }

    #[test]
    fn is_gamdl_mv_cover_template_bug_recognises_raw_placeholders() {
        // Defensive: same shape with raw `{w}x{h}` (e.g. if a future
        // GAMDL version skips the percent-encoding pass).
        let msg = "Client error '400 Bad Request' for url 'https://a1.mzstatic.com/Video112/v4/abc/{w}x{h}mv.jpg'";
        assert!(is_gamdl_mv_cover_template_bug(msg));
    }

    #[test]
    fn is_gamdl_mv_cover_template_bug_requires_all_three_signals() {
        // 400 alone is not enough — many things can 400.
        assert!(!is_gamdl_mv_cover_template_bug("400 Bad Request"));
        // Video URL alone with no 400 isn't the bug shape.
        assert!(!is_gamdl_mv_cover_template_bug(
            "200 OK from mzstatic.com/Video221"
        ));
        // 400 + Video URL but with substituted dimensions = a different bug.
        assert!(!is_gamdl_mv_cover_template_bug(
            "400 Bad Request for url https://a1.mzstatic.com/Video221/v4/abc/1920x1080mv.jpg"
        ));
    }

    #[test]
    fn is_gamdl_mv_cover_template_bug_rejects_storefront_404_shape() {
        // Storefront mismatch is a different bug — must not double-classify.
        let msg = r#"gamdl.api.exceptions.GamdlApiResponseError: Error fetching from AMP API (Status code: 404): {"errors":[{"id":"X","title":"Resource Not Found"}]}"#;
        assert!(!is_gamdl_mv_cover_template_bug(msg));
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
    fn classifies_gamdl_v3_7_1_ytdlp_runtime_errors_as_network() {
        // GAMDL v3.7.1 (upstream commit `1d00e74`) refactored the yt-dlp
        // call path to use HlsFD / HttpFD directly and raise bare
        // RuntimeError strings on failure. Both error shapes must
        // classify as network so the retry-on-network loop engages
        // and the user sees "Check your connection" guidance, not the
        // generic "unknown" fallback.
        assert_eq!(
            classify_error("RuntimeError: yt-dlp HLS download failed"),
            "network"
        );
        assert_eq!(
            classify_error("yt-dlp HTTP download failed"),
            "network"
        );
        // Surface-form variations (different case + extra surrounding context).
        assert_eq!(
            classify_error("error: yt-dlp HLS download failed during music-video fetch"),
            "network"
        );
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
    fn classifies_gamdl_library_webplayback_keyerror() {
        // The traceback signature captured 2026-04-23 during #546
        // scenario 1 repro on a personal-library album URL (#570).
        // GAMDL's `interface_song.py:179` unconditionally dereferences
        // `webplayback["songList"][0]…`, but the library endpoint
        // returns a different response shape — raises
        // `KeyError: 'songList'` on every library track. Must route
        // to the dedicated category so users get an actionable error.
        let stderr = "Traceback (most recent call last):\n\
            File \".../gamdl/interface/interface_song.py\", line 179, in get_tags\n\
            webplayback_metadata = webplayback[\"songList\"][0][\"assets\"][0][\"metadata\"]\n\
            KeyError: 'songList'";
        assert_eq!(classify_error(stderr), "library_webplayback_keyerror");
    }

    #[test]
    fn classifies_unrelated_keyerror_songlist_as_unknown() {
        // Regression canary: a `KeyError: 'songList'` without the
        // `interface_song` / `webplayback` frame must NOT be
        // misclassified as the library-URL bug. Some unrelated
        // upstream code could raise the same key error.
        let stderr = "KeyError: 'songList' in unrelated module";
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
    fn detects_media_not_streamable_error() {
        // The bare exception message from `GamdlInterfaceMediaNotStreamableError`,
        // as it appears in stderr after the GAMDL 3.7.2 (songs) / 3.7.3
        // (music-videos) defensive `.get("playParams", {})` fix routes
        // unstreamable items through the proper streamable check.
        assert!(is_media_not_streamable_error(
            "GamdlInterfaceMediaNotStreamableError: Media is not streamable: 1234567890"
        ));
        // Also matches the bare message extracted by `extract_python_exception`.
        assert!(is_media_not_streamable_error("Media is not streamable: 1568443843"));
        // Case-insensitive — should match even when nested in a traceback
        // with mixed casing or wrapped log decoration.
        assert!(is_media_not_streamable_error(
            "ERROR    12:34:56 Media is not streamable: 999"
        ));
    }

    #[test]
    fn does_not_misclassify_streamable_phrasing() {
        // Defensive guard: matcher must not collide with messages that
        // talk about streamability in a different sense (e.g. health
        // checks reporting on stream availability or future GAMDL log
        // lines that mention "streamable" as a noun-adjective).
        assert!(!is_media_not_streamable_error("Media is streamable"));
        assert!(!is_media_not_streamable_error(
            "Checking whether the media stream is available"
        ));
    }

    #[test]
    fn classifies_media_not_streamable_error() {
        // GAMDL 3.7.2 (songs) — defensive playParams access lets the
        // existing streamable check fire reliably; the surfaced error
        // must route to its own bucket so the user sees the dedicated
        // "removed / region-locked / library-only" guidance instead of
        // the generic `unknown` fallback.
        let stderr = "GamdlInterfaceMediaNotStreamableError: Media is not streamable: 1568443843";
        assert_eq!(classify_error(stderr), "media_not_streamable");
    }

    #[test]
    fn classifies_music_video_not_streamable_error() {
        // Same shape for music-videos after the 3.7.3 defensive fix.
        // `media.is_library = true` branch also raises this exception
        // class (`music_video.py:456`).
        let stderr = "gamdl.interface.exceptions.GamdlInterfaceMediaNotStreamableError: Media is not streamable: 1568443895";
        assert_eq!(classify_error(stderr), "media_not_streamable");
    }

    #[test]
    fn media_not_streamable_takes_precedence_over_not_found() {
        // The error string contains neither "not found" nor "404", but
        // even if the wider exception text happens to include "not found"
        // we want the specific `media_not_streamable` classification —
        // the bucket order is asserted here to guard against future
        // re-ordering regressions.
        let stderr = "Media is not streamable: 9876 — content not found in catalog";
        assert_eq!(classify_error(stderr), "media_not_streamable");
    }

    #[test]
    fn media_not_streamable_guidance_is_actionable() {
        // Users hitting this should know the content can't be played —
        // not retried, not codec-swapped — and what to try instead.
        let guidance = error_guidance("media_not_streamable");
        assert!(guidance.contains("storefront") || guidance.contains("region"));
        assert!(guidance.contains("removed") || guidance.contains("library"));
    }

    #[test]
    fn detects_wrapper_version_mismatch_forward_skew() {
        // GAMDL 3.8.2 exits immediately at CLI startup when wrapper-v2
        // reports anything other than "0.0.2" from `GET /me`.
        assert!(is_wrapper_version_mismatch_error(
            "Unsupported wrapper-v2 API version. gamdl requires wrapper-v2 0.0.2"
        ));
        // Case-insensitive — should match regardless of log wrapping.
        assert!(is_wrapper_version_mismatch_error(
            "ERROR    12:34:56 Unsupported wrapper-v2 API version. gamdl requires wrapper-v2 0.0.2"
        ));
    }

    #[test]
    fn detects_wrapper_version_mismatch_reverse_skew() {
        // GAMDL <= 3.8.1 still calls the HTTP `POST /decrypt` endpoint,
        // which wrapper-v2 0.0.2 removed in favour of a native TCP
        // protocol — surfaces as a 404 against that path.
        assert!(is_wrapper_version_mismatch_error(
            "wrapper-v2: POST /decrypt failed HTTP 404"
        ));
        assert!(is_wrapper_version_mismatch_error(
            "httpx.HTTPStatusError: Client error '404 Not Found' for url 'http://127.0.0.1:10020/decrypt'"
        ));
    }

    #[test]
    fn does_not_misclassify_unrelated_errors_as_wrapper_version_mismatch() {
        // A generic 404 with no `/decrypt` in it must not match — this
        // is what routes ordinary "content not found" errors to the
        // `not_found` bucket instead.
        assert!(!is_wrapper_version_mismatch_error("404 Not Found"));
        // `/decrypt` alone (e.g. a successful decrypt log line) must
        // not match without an accompanying 404.
        assert!(!is_wrapper_version_mismatch_error(
            "Connecting to wrapper-v2 /decrypt endpoint"
        ));
        assert!(!is_wrapper_version_mismatch_error(
            "wrapper-v2 API version supported"
        ));
    }

    #[test]
    fn classifies_wrapper_version_mismatch_forward_skew() {
        let stderr = "Unsupported wrapper-v2 API version. gamdl requires wrapper-v2 0.0.2";
        assert_eq!(classify_error(stderr), "wrapper_version_mismatch");
    }

    #[test]
    fn classifies_wrapper_version_mismatch_reverse_skew() {
        let stderr = "wrapper-v2: POST /decrypt failed HTTP 404";
        assert_eq!(classify_error(stderr), "wrapper_version_mismatch");
    }

    #[test]
    fn wrapper_version_mismatch_takes_precedence_over_not_found() {
        // The message also contains a generic "not found"-ish token —
        // the bucket order is asserted here to guard against future
        // re-ordering regressions putting the generic `not_found`
        // classification ahead of the specific version-mismatch one.
        let stderr =
            "Unsupported wrapper-v2 API version. gamdl requires wrapper-v2 0.0.2 — resource not found";
        assert_eq!(classify_error(stderr), "wrapper_version_mismatch");
    }

    #[test]
    fn wrapper_version_mismatch_guidance_is_actionable() {
        // Users hitting this should be told to align the two
        // independent projects' versions, not to retry or swap codecs.
        // GAMDL 3.8.2 requires wrapper-v2 0.0.2 (native TCP decrypt);
        // 3.6-3.8.1 requires 0.0.1 (HTTP decrypt) — the guidance must
        // reflect both eras, not just the older one.
        let guidance = error_guidance("wrapper_version_mismatch");
        assert!(guidance.contains("wrapper-v2"));
        assert!(guidance.contains("upgraded together"));
        assert!(guidance.contains("0.0.2"));
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
        // The upstream gamdl#306 shape: a 429 on the license-exchange endpoint
        // must classify as rate_limit (429 wins over the license-declined
        // branch), so the queue-pause guard fires.
        assert_eq!(
            classify_error("Error fetching license exchange data (Status code: 429)"),
            "rate_limit"
        );
    }

    #[test]
    fn classifies_non_429_license_refusal_as_license_declined() {
        // gamdl#307: Apple refused a license for one track with a non-429 status.
        assert_eq!(
            classify_error(
                "Error fetching license exchange data (Status code: 200): {\"status\":-1002}"
            ),
            "license_declined"
        );
        // Guidance must exist and NOT claim it's a rate limit.
        let g = error_guidance("license_declined");
        assert!(g.to_lowercase().contains("license"));
        assert!(g.to_lowercase().contains("not a rate limit"));
    }

    #[test]
    fn classifies_wrapper_decrypt_unavailable() {
        // gamdl#319: daemon up + authenticated but FairPlay not ready.
        assert_eq!(
            classify_error("OSError: wrapper-v2: decrypt unavailable (503)"),
            "wrapper_decrypt_unavailable"
        );
        let g = error_guidance("wrapper_decrypt_unavailable");
        assert!(g.to_lowercase().contains("restart"));
        assert!(g.to_lowercase().contains("aac-web"));
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
    fn v3_codec_skips_are_detected_as_codec_skip_events() {
        // The two "Skipping ... Requested format is not available" lines
        // are WARNING level from GAMDL's perspective. After #698 they
        // classify as `CodecSkip`, not `Error` — the queue's terminal
        // classifier inspects these via `is_codec_skip_message` to
        // distinguish "Apple doesn't offer this format" from a genuine
        // download failure. The downstream gap-fill / `is_codec_error`
        // signal still fires because those helpers are called against
        // the message string content, not against the variant tag (see
        // `v3_codec_skips_trigger_codec_error_classification` below).
        let events = classify_lines(FIXTURE_V3_CODEC_SKIPS);
        let codec_skips: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                GamdlOutputEvent::CodecSkip { message } => Some(message.clone()),
                _ => None,
            })
            .collect();
        assert!(
            codec_skips
                .iter()
                .any(|m| m.to_lowercase().contains("format is not available")),
            "Expected at least one 'format is not available' CodecSkip event \
             after #698, got: {codec_skips:?}"
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
    fn v31_track_regex_handles_dash_total_fallback() {
        // GAMDL v3.7.1 (commit `1d00e74`+) renders
        // `media_total or "-"` so a single-track URL produces
        // `[Track 1/-]` instead of `[Track 1/12]`. Pre-v3.7.1
        // MeedyaDL's TRACK_INFO_V2_REGEX required `\d+/\d+` and
        // silently rejected this line; commit `7b91c7a9` widened
        // the regex to accept `(\d+|-)` for the total slot, with
        // the downstream `parse::<u32>().ok()` consumer mapping
        // `-` to `None`. This test pins the new contract: line
        // parses as TrackInfo, `track_total` is `None`.
        let line = "[INFO     12:00:02] [Track   1/-  ] Downloading \"Flowers\"";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::TrackInfo {
                track_number,
                track_total,
                title,
                ..
            } => {
                assert_eq!(track_number, Some(1));
                assert_eq!(track_total, None, "`-` total must parse to None");
                assert_eq!(title, "Flowers");
            }
            other => panic!(
                "Track N/- should parse as TrackInfo on v3.7.1+, got {:?}",
                other
            ),
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
        if let GamdlOutputEvent::TrackInfo { .. } = parse_gamdl_output(line) {
            panic!("Album-wrapper line must not parse as TrackInfo");
        }
    }

    #[test]
    fn v32_playlist_wrapper_line_not_misclassified_as_track() {
        // Same guard as the album case, for playlists. The `[URL N/M]`
        // prefix is for URL-level progress, not per-track.
        let line = "[INFO     12:00:01] [URL   1/1  ] Processing \"https://music.apple.com/.../pl.abc\"";
        if let GamdlOutputEvent::TrackInfo { .. } = parse_gamdl_output(line) {
            panic!("URL-wrapper line must not parse as TrackInfo");
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

    // ==========================================================
    // Codec-skip classification (#698)
    // ==========================================================

    /// The canonical GAMDL "Skipping ... format is not available" line
    /// must classify as `CodecSkip`, not `Error`. Before #698 this hit
    /// Priority 7's `"skipping"` keyword and surfaced as a red error in
    /// the queue item's error field plus an "Error" entry in the activity
    /// log — the user-visible "format-not-available misclassification"
    /// symptom from the bug report.
    #[test]
    fn codec_skip_classifies_as_codec_skip_not_error() {
        let line = "[WARNING 22:32:23] [Track 23/24] Skipping \"Die Young (Deconstructed Mix)\": \
                    Requested format is not available (media ID: 592365442): \
                    [<SongCodec.ATMOS: 'atmos'>, <SongCodec.ALAC: 'alac'>, \
                    <SongCodec.AC3: 'ac3'>, <SongCodec.AAC: 'aac'>, \
                    <SongCodec.AAC_LEGACY: 'aac-legacy'>]";
        match parse_gamdl_output(line) {
            GamdlOutputEvent::CodecSkip { message } => {
                assert!(message.contains("Skipping"));
                assert!(message.contains("format is not available"));
            }
            other => panic!("expected CodecSkip, got {other:?}"),
        }
    }

    /// Older GAMDL phrasing variants must also classify as `CodecSkip`.
    #[test]
    fn codec_skip_recognises_lowercase_format_not_available() {
        let line = "Skipping track: format not available";
        assert!(matches!(
            parse_gamdl_output(line),
            GamdlOutputEvent::CodecSkip { .. }
        ));
    }

    /// `is_codec_skip_line` must require BOTH a skip verb AND a
    /// format-unavailable phrase. Lines that mention "skipping" alone
    /// (e.g. rate-limit retry skips, pre-existing-file skips) must NOT
    /// be misclassified.
    #[test]
    fn codec_skip_does_not_match_unrelated_skipping_lines() {
        // "Skipping" alone — no format keyword
        assert!(!is_codec_skip_line("[INFO] Skipping cover art (already downloaded)"));
        assert!(!is_codec_skip_line("Skipping rate-limited request, retrying in 30s"));
        // "Format" alone — no skip verb
        assert!(!is_codec_skip_line("Requested format: ATMOS"));
        assert!(!is_codec_skip_line("Format is not available — falling back"));
    }

    /// Non-codec-skip warnings that contain the `"skipping"` substring
    /// (e.g. genuine errors that mention skipping in their text) must
    /// still fall through to Priority 7's keyword match if they contain
    /// other error signal words. A line with only `"skipping"` and no
    /// other error keywords now classifies as `Unknown` — by design,
    /// since post-#698 we don't treat a bare `"skipping"` mention as
    /// inherently failure-shaped.
    #[test]
    fn bare_skipping_without_format_keyword_is_no_longer_error() {
        // This used to classify as Error via Priority 7's `"skipping"`.
        // Post-#698 it falls through to Unknown — the parser is no
        // longer the layer that decides whether a "Skipping" mention is
        // an error; the queue's terminal classifier does, by inspecting
        // the recorded warnings.
        let line = "Skipping cover art (already downloaded)";
        assert!(matches!(
            parse_gamdl_output(line),
            GamdlOutputEvent::Unknown { .. }
        ));
    }

    /// `is_codec_skip_message` is an alias for `is_codec_skip_line` used
    /// at the queue's terminal classifier. Both predicates must agree.
    #[test]
    fn is_codec_skip_message_agrees_with_is_codec_skip_line() {
        let canonical = "[WARNING] Skipping \"Track\": Requested format is not available";
        assert!(is_codec_skip_line(canonical));
        assert!(is_codec_skip_message(canonical));

        let unrelated = "[ERROR] Connection refused";
        assert!(!is_codec_skip_line(unrelated));
        assert!(!is_codec_skip_message(unrelated));
    }

    // ----------------------------------------------------------
    // humanise_codec_skip_line (Phase 3.5h)
    // ----------------------------------------------------------

    #[test]
    fn humanise_strips_media_id_and_codec_repr_single() {
        // Real captured line from the user's 2026-05-08 reproduction.
        let raw = "[WARNING  13:21:56] [Track   1/1  ] Skipping \"Pickle (3ballMTY Remix)\": Requested format is not available (media ID: 1578734917): [<SongCodec.AC3: 'ac3'>]";
        let humanised = humanise_codec_skip_line(raw);
        // Media ID stripped.
        assert!(!humanised.contains("media ID"), "should strip media ID: {humanised}");
        assert!(!humanised.contains("1578734917"), "should strip the numeric ID");
        // Codec repr replaced by friendly text.
        assert!(!humanised.contains("SongCodec"), "should strip Python repr: {humanised}");
        // #832: lowercase enum identifier `ac3` is now upgraded to the
        // proper display label `AC3` to match what users see elsewhere
        // in the UI (codec dropdowns, companion filename suffixes).
        assert!(humanised.contains("AC3 not available"), "should mention AC3: {humanised}");
        // Track title preserved.
        assert!(humanised.contains("\"Pickle (3ballMTY Remix)\""), "title preserved: {humanised}");
    }

    #[test]
    fn humanise_strips_media_id_and_codec_repr_multiple_codecs() {
        let raw = "[WARNING  22:32:23] [Track  23/24  ] Skipping \"Die Young (Deconstructed Mix)\": Requested format is not available (media ID: 592365442): [<SongCodec.ATMOS: 'atmos'>, <SongCodec.ALAC: 'alac'>]";
        let humanised = humanise_codec_skip_line(raw);
        assert!(!humanised.contains("media ID"));
        assert!(!humanised.contains("SongCodec"));
        // #832: pretty labels instead of the raw lowercase enum identifiers.
        assert!(
            humanised.contains("Atmos, ALAC not available"),
            "should list both codecs with display labels: {humanised}"
        );
    }

    /// GAMDL 3.x multi-codec line shape — the `atmos, alac, ac3, aac,
    /// aac-legacy not available` summary, plus the verbose
    /// `(Unavailable requested format candidates: …)` parenthetical
    /// (#832).
    #[test]
    fn humanise_rewrites_gamdl3_lowercase_multi_codec_summary() {
        let raw = "[WARNING  21:14:34] [Track   1/19 ] Skipping \"My Love (Radio Edit)\": Requested format is not available: atmos, alac, ac3, aac, aac-legacy not available (Unavailable requested format candidates: Dolby Atmos (Experimental) [atmos] -> Lossless (ALAC) (Experimental) [alac] -> Dolby Digital (AC3) (Experimental) [ac3] -> AAC (256kbps at up to 48kHz) (Experimental) [aac] -> AAC Legacy (256kbps at up to 44.1kHz) [aac-legacy])";
        let humanised = humanise_codec_skip_line(raw);
        // Verbose "(Unavailable requested format candidates: …)"
        // parenthetical gone — it was just the same codec list a
        // second time, with the redundant "(Experimental)" tags.
        assert!(
            !humanised.contains("Unavailable requested format candidates"),
            "should strip verbose parenthetical: {humanised}"
        );
        assert!(
            !humanised.contains("(Experimental)"),
            "Experimental tags gone too: {humanised}"
        );
        // Lowercase codec list rewritten with pretty labels.
        assert!(
            humanised.contains("Atmos, ALAC, AC3, AAC, AAC Legacy not available"),
            "should rewrite lowercase list with pretty labels: {humanised}"
        );
        // Track title and identifier preserved.
        assert!(
            humanised.contains("\"My Love (Radio Edit)\""),
            "title preserved: {humanised}"
        );
    }

    /// Defensive: a comma-separated lowercase run that ISN'T all known
    /// codecs (e.g. accidental match on some other GAMDL warning) must
    /// pass through unchanged so we never garble unrelated content.
    #[test]
    fn humanise_does_not_rewrite_unknown_token_runs() {
        let raw = "[WARNING] Skipping \"X\": Requested format is not available: foo, bar, baz not available";
        let humanised = humanise_codec_skip_line(raw);
        // foo, bar, baz are NOT known codecs → must pass through.
        assert!(
            humanised.contains("foo, bar, baz not available"),
            "unknown tokens preserved: {humanised}"
        );
    }

    /// Lines that aren't codec-skip shape must pass through unchanged.
    #[test]
    fn humanise_passes_through_unrelated_lines() {
        let raw = "[INFO 12:00:00] [Track 1/1] Downloading \"Some Song\"";
        assert_eq!(humanise_codec_skip_line(raw), raw);

        let raw2 = "[ERROR] Connection refused";
        assert_eq!(humanise_codec_skip_line(raw2), raw2);
    }

    /// Idempotent: running on already-humanised output is a no-op.
    #[test]
    fn humanise_is_idempotent() {
        let raw = "[WARNING  13:21:56] [Track   1/1  ] Skipping \"X\": Requested format is not available (media ID: 1): [<SongCodec.AC3: 'ac3'>]";
        let once = humanise_codec_skip_line(raw);
        let twice = humanise_codec_skip_line(&once);
        assert_eq!(once, twice, "second pass changed: {once} → {twice}");
    }

    /// `is_codec_error` must continue to recognise the same vocabulary —
    /// the gap-fill / fallback-retry decisions in `download_queue` rely
    /// on it, and #698 only changes how the parser routes the line, not
    /// what classification means downstream. Existing keyword set must
    /// stay intact.
    #[test]
    fn is_codec_error_still_matches_codec_skip_messages() {
        let line = "Skipping \"Track\": Requested format is not available: [<SongCodec.ATMOS>]";
        assert!(
            is_codec_error(line),
            "is_codec_error must still match the codec-skip vocabulary so the \
             queue's existing has_codec_error gate keeps firing for these lines"
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
                    if let GamdlOutputEvent::TrackInfo { .. } = parse_gamdl_output(line) {
                        panic!("URL-context line from {name} must not parse as TrackInfo: {line}");
                    }
                }
            }
        }
    }
}
