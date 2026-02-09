// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download-related data models.
// Defines the structures used for download requests, queue items,
// and status tracking throughout the application.

use serde::{Deserialize, Serialize};

use super::gamdl_options::GamdlOptions;

/// A download request submitted by the user from the UI.
/// Contains the target URL(s) and optional quality/format overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    /// One or more Apple Music URLs to download
    pub urls: Vec<String>,

    /// Optional per-download overrides for GAMDL options.
    /// When None, the global settings are used for all options.
    /// When Some, only the specified fields override the globals.
    pub options: Option<GamdlOptions>,
}

/// The possible states of a download queue item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    /// Waiting in the queue to be processed
    Queued,
    /// Currently downloading content from Apple Music
    Downloading,
    /// Post-download processing (remuxing, tagging, embedding artwork)
    Processing,
    /// Download completed successfully
    Complete,
    /// Download failed with an error
    Error,
    /// Download was cancelled by the user
    Cancelled,
}

/// Detailed status of a single download queue item.
/// Sent to the frontend via events and the get_queue_status command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItemStatus {
    /// Unique identifier for this queue item
    pub id: String,

    /// The original URL(s) being downloaded
    pub urls: Vec<String>,

    /// Current state of this download
    pub state: DownloadState,

    /// Download progress as a percentage (0.0 to 100.0)
    pub progress: f64,

    /// Name of the track currently being downloaded (for albums/playlists)
    pub current_track: Option<String>,

    /// Total number of tracks to download (for albums/playlists)
    pub total_tracks: Option<usize>,

    /// Number of tracks completed so far
    pub completed_tracks: Option<usize>,

    /// Current download speed (e.g., "2.5 MB/s")
    pub speed: Option<String>,

    /// Estimated time remaining (e.g., "00:45")
    pub eta: Option<String>,

    /// Error message if the download failed
    pub error: Option<String>,

    /// Path to the output file/directory on completion
    pub output_path: Option<String>,

    /// The audio codec used for this download (may differ from requested
    /// if fallback was triggered)
    pub codec_used: Option<String>,

    /// Whether this download fell back to a different codec/quality
    pub fallback_occurred: bool,

    /// ISO 8601 timestamp when this item was added to the queue
    pub created_at: String,
}
