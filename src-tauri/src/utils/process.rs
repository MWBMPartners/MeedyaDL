// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Subprocess output parsing utilities.
// Parses GAMDL's stdout/stderr output to extract structured events
// (progress updates, track information, errors) that the UI can display.
// Stub implementation for Phase 1; full implementation in Phase 2.

use serde::Serialize;

/// A structured event parsed from a line of GAMDL's stdout/stderr output.
/// The frontend listens for these events to update the download progress UI.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GamdlOutputEvent {
    /// Information about the track currently being processed
    TrackInfo {
        /// Track title
        title: String,
        /// Artist name
        artist: String,
        /// Album name
        album: String,
    },

    /// Download progress update
    DownloadProgress {
        /// Progress percentage (0.0 to 100.0)
        percent: f64,
        /// Current download speed (e.g., "2.5 MB/s")
        speed: String,
        /// Estimated time remaining (e.g., "00:45")
        eta: String,
    },

    /// A post-download processing step (remuxing, tagging, etc.)
    ProcessingStep {
        /// Description of the current step
        step: String,
    },

    /// An error occurred during the download
    Error {
        /// Error message from GAMDL
        message: String,
    },

    /// Download completed successfully for a track/file
    Complete {
        /// Path to the output file
        path: String,
    },

    /// Unrecognized output line (logged for debugging)
    Unknown {
        /// The raw output line
        raw: String,
    },
}

/// Parses a single line of GAMDL output into a structured event.
///
/// GAMDL outputs progress and status information to stdout and stderr
/// in various formats. This parser recognizes common patterns and
/// converts them to GamdlOutputEvent variants for the UI.
///
/// # Arguments
/// * `line` - A single line from GAMDL's stdout or stderr
///
/// # Returns
/// A `GamdlOutputEvent` representing the parsed content of the line.
/// Returns `Unknown` for lines that don't match any known pattern.
pub fn parse_gamdl_output(line: &str) -> GamdlOutputEvent {
    let trimmed = line.trim();

    // Empty lines are ignored
    if trimmed.is_empty() {
        return GamdlOutputEvent::Unknown {
            raw: String::new(),
        };
    }

    // TODO: Phase 2 - Implement comprehensive output parsing
    // This will use regex patterns to match GAMDL's output format
    // and extract structured data (progress %, track info, errors).
    //
    // For now, categorize based on simple string matching:

    // Check for error patterns
    if trimmed.to_lowercase().contains("error")
        || trimmed.to_lowercase().contains("failed")
    {
        return GamdlOutputEvent::Error {
            message: trimmed.to_string(),
        };
    }

    // Check for completion patterns
    if trimmed.starts_with("Saved to") || trimmed.contains("saved to") {
        return GamdlOutputEvent::Complete {
            path: trimmed.to_string(),
        };
    }

    // Check for processing step patterns
    if trimmed.starts_with("Remuxing")
        || trimmed.starts_with("Tagging")
        || trimmed.starts_with("Embedding")
    {
        return GamdlOutputEvent::ProcessingStep {
            step: trimmed.to_string(),
        };
    }

    // Default: unrecognized output
    GamdlOutputEvent::Unknown {
        raw: trimmed.to_string(),
    }
}
