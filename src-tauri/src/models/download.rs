// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download-related data models.
// Defines the structures used for download requests, queue items,
// and status tracking throughout the application.
//
// ## Architecture
//
// The download pipeline works as follows:
//
// 1. The React frontend submits a `DownloadRequest` via the `start_download`
//    Tauri command (see `commands/download.rs`).
// 2. The backend creates a `QueueItemStatus` for each URL, assigns it a
//    unique ID, and pushes it onto the download queue.
// 3. The download manager processes queue items sequentially (or with
//    concurrency, depending on configuration), updating the `DownloadState`
//    as the GAMDL subprocess progresses.
// 4. State transitions are emitted to the frontend as Tauri events so
//    the UI can update in real time.
//
// ## State machine
//
// ```text
//   ┌─────────┐      ┌─────────────┐      ┌────────────┐      ┌──────────┐
//   │ Queued  │─────>│ Downloading │─────>│ Processing │─────>│ Complete │
//   └─────────┘      └─────────────┘      └────────────┘      └──────────┘
//        │                  │                    │
//        │                  │                    │
//        ▼                  ▼                    ▼
//   ┌───────────┐    ┌───────────┐        ┌───────────┐
//   │ Cancelled │    │   Error   │        │   Error   │
//   └───────────┘    └───────────┘        └───────────┘
// ```
//
// - **Queued -> Downloading**: item is picked up by the download manager.
// - **Downloading -> Processing**: GAMDL finishes fetching; remuxing/tagging begins.
// - **Processing -> Complete**: all post-processing finished successfully.
// - **Any -> Error**: an unrecoverable error occurred at any stage.
// - **Queued/Downloading -> Cancelled**: user cancelled the download.
//
// ## References
//
// - serde: <https://docs.rs/serde/latest/serde/>
// - Tauri events: <https://v2.tauri.app/develop/calling-rust/#event-system>

use serde::{Deserialize, Serialize};

use super::gamdl_options::GamdlOptions;

/// A download request submitted by the user from the React frontend.
///
/// This struct is deserialized from the JSON payload of the `start_download`
/// Tauri command. It contains the target URL(s) and optional quality/format
/// overrides that take precedence over the global `AppSettings`.
///
/// ## Frontend origin
///
/// Created by the download form component (`src/components/DownloadForm.tsx`)
/// when the user clicks "Download". The frontend serializes this as JSON
/// and sends it via `invoke("start_download", { request: ... })`.
///
/// ## Processing
///
/// The backend's download command handler in `commands/download.rs`:
/// 1. Reads the current `AppSettings` and converts them to `GamdlOptions`.
/// 2. Merges `self.options` (if present) on top of the global options.
/// 3. Creates one `QueueItemStatus` per URL and enqueues them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    /// One or more Apple Music URLs to download. Each URL can be a song,
    /// album, playlist, or music video link. The download manager creates
    /// a separate `QueueItemStatus` for each URL.
    pub urls: Vec<String>,

    /// Optional per-download overrides for GAMDL options.
    ///
    /// - `None` -- use the global `AppSettings` for all options.
    /// - `Some(opts)` -- only the `Some(...)` fields within `opts` override
    ///   the global settings; `None` fields are inherited.
    ///
    /// See `GamdlOptions` in `gamdl_options.rs` for why all fields are
    /// `Option<T>` and how the merge works.
    pub options: Option<GamdlOptions>,
}

/// The possible states of a download queue item.
///
/// This enum models the state machine for a single download. See the
/// ASCII diagram in the module-level comment above for the full transition
/// graph.
///
/// ## Serialization
///
/// `#[serde(rename_all = "snake_case")]` ensures these serialize to
/// `"queued"`, `"downloading"`, etc. in JSON, matching the conventions
/// used in the React frontend's TypeScript types.
///
/// ## Frontend consumption
///
/// The React frontend pattern-matches on these string values to render
/// the appropriate status badge, progress bar, and action buttons for
/// each queue item in the download queue UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    /// **Initial state.** The item is waiting in the queue to be picked
    /// up by the download manager. The user can cancel from this state.
    Queued,

    /// **Active state.** The GAMDL subprocess is running and fetching
    /// content from Apple Music's CDN. Progress percentage and speed
    /// are updated in real time. The user can cancel from this state.
    Downloading,

    /// **Active state.** The download has finished and GAMDL is
    /// performing post-download processing: decrypting, remuxing into
    /// the target container format, embedding metadata tags, and
    /// attaching cover art. This stage cannot be cancelled.
    Processing,

    /// **Terminal state (success).** The download completed successfully.
    /// `QueueItemStatus::output_path` contains the path to the result.
    Complete,

    /// **Terminal state (failure).** An unrecoverable error occurred
    /// during downloading or processing. `QueueItemStatus::error`
    /// contains the error message. The user can retry from the UI.
    Error,

    /// **Terminal state (user-initiated).** The user cancelled the
    /// download while it was in the `Queued` or `Downloading` state.
    /// Any partially-downloaded files are cleaned up.
    Cancelled,
}

/// Detailed status of a single download queue item.
///
/// This struct is the primary data structure for communicating download
/// progress between the Rust backend and the React frontend. It is:
///
/// - Returned by the `get_queue_status` Tauri command (bulk query).
/// - Emitted as the payload of `download-progress` Tauri events
///   (real-time updates).
///
/// ## Unique ID
///
/// The `id` field is a UUID v4 string generated at queue insertion time.
/// It uniquely identifies this queue item across the lifetime of the
/// application session. The frontend uses it as a React list key and
/// to correlate incoming events with the correct queue item.
///
/// ## Reference
///
/// - UUID v4 generation: <https://docs.rs/uuid/latest/uuid/>
/// - Tauri events: <https://v2.tauri.app/develop/calling-rust/#event-system>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItemStatus {
    /// Unique identifier for this queue item (UUID v4 string).
    /// Generated once at creation time and never changes. Used as the
    /// correlation key between backend state and frontend UI elements.
    pub id: String,

    /// The original Apple Music URL(s) being downloaded. For single-track
    /// downloads this contains one URL; for batch requests it may contain
    /// multiple. Displayed in the queue UI as the item's title/subtitle.
    pub urls: Vec<String>,

    /// Current state in the download state machine. See `DownloadState`
    /// for the full list of states and allowed transitions.
    pub state: DownloadState,

    /// Download progress as a percentage (0.0 to 100.0). Updated in real
    /// time by parsing GAMDL's stdout output. For album/playlist downloads,
    /// this reflects overall progress across all tracks.
    pub progress: f64,

    /// Name of the track currently being downloaded. Populated when
    /// downloading albums or playlists; `None` for single-track downloads.
    /// Parsed from GAMDL's stdout progress lines.
    pub current_track: Option<String>,

    /// Total number of tracks to download. `Some(n)` for albums/playlists,
    /// `None` for single-track downloads. Used by the frontend to render
    /// "Track 3 of 12" style progress indicators.
    pub total_tracks: Option<usize>,

    /// Number of tracks completed so far. Incremented each time GAMDL
    /// reports a track as finished. Combined with `total_tracks` to
    /// calculate per-track progress.
    pub completed_tracks: Option<usize>,

    /// Current download speed as a human-readable string (e.g., `"2.5 MB/s"`).
    /// Parsed from GAMDL/yt-dlp stdout. `None` when not actively
    /// downloading (i.e., in Queued, Processing, or terminal states).
    pub speed: Option<String>,

    /// Estimated time remaining as a human-readable string (e.g., `"00:45"`).
    /// Parsed from GAMDL/yt-dlp stdout. `None` when ETA is not available.
    pub eta: Option<String>,

    /// Error message if the download failed (`state == Error`). Contains
    /// the stderr output or exception message from the GAMDL subprocess.
    /// `None` in all non-error states.
    pub error: Option<String>,

    /// Absolute path to the output file or directory on completion
    /// (`state == Complete`). For single tracks this is the file path;
    /// for albums/playlists this is the folder path. `None` before
    /// completion.
    pub output_path: Option<String>,

    /// The audio codec that was actually used for this download. May
    /// differ from the user's preferred codec if the fallback system
    /// was triggered (see `AppSettings::fallback_enabled`). Displayed
    /// in the queue UI so the user knows what quality they received.
    pub codec_used: Option<String>,

    /// Whether the fallback quality system was activated for this
    /// download. When `true`, the download succeeded but with a
    /// different codec or resolution than the user's first preference.
    /// The frontend uses this to show a warning indicator.
    pub fallback_occurred: bool,

    /// ISO 8601 timestamp (`YYYY-MM-DDTHH:MM:SS.sssZ`) when this item
    /// was added to the queue. Used for sorting the queue display and
    /// for calculating elapsed time.
    pub created_at: String,
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // DownloadState serde serialization
    // ----------------------------------------------------------

    /// Verifies that `DownloadState::Queued` serializes to the
    /// snake_case string `"queued"` as expected by the React frontend.
    #[test]
    fn download_state_queued_serializes_correctly() {
        let json = serde_json::to_string(&DownloadState::Queued).unwrap();
        assert_eq!(json, "\"queued\"");
    }

    /// Verifies that `DownloadState::Downloading` serializes to
    /// `"downloading"` for the frontend status badge rendering.
    #[test]
    fn download_state_downloading_serializes_correctly() {
        let json = serde_json::to_string(&DownloadState::Downloading).unwrap();
        assert_eq!(json, "\"downloading\"");
    }

    /// Verifies that `DownloadState::Processing` serializes to
    /// `"processing"` for the frontend progress indicator.
    #[test]
    fn download_state_processing_serializes_correctly() {
        let json = serde_json::to_string(&DownloadState::Processing).unwrap();
        assert_eq!(json, "\"processing\"");
    }

    /// Verifies that `DownloadState::Complete` serializes to
    /// `"complete"` for the frontend completion indicator.
    #[test]
    fn download_state_complete_serializes_correctly() {
        let json = serde_json::to_string(&DownloadState::Complete).unwrap();
        assert_eq!(json, "\"complete\"");
    }

    /// Verifies that `DownloadState::Error` serializes to
    /// `"error"` for the frontend error display.
    #[test]
    fn download_state_error_serializes_correctly() {
        let json = serde_json::to_string(&DownloadState::Error).unwrap();
        assert_eq!(json, "\"error\"");
    }

    /// Verifies that `DownloadState::Cancelled` serializes to
    /// `"cancelled"` for the frontend cancellation indicator.
    #[test]
    fn download_state_cancelled_serializes_correctly() {
        let json = serde_json::to_string(&DownloadState::Cancelled).unwrap();
        assert_eq!(json, "\"cancelled\"");
    }

    /// Verifies that all `DownloadState` variants survive a full
    /// serde roundtrip (serialize to JSON, then deserialize back)
    /// without data loss or corruption.
    #[test]
    fn download_state_serde_roundtrip_all_variants() {
        let variants = vec![
            DownloadState::Queued,
            DownloadState::Downloading,
            DownloadState::Processing,
            DownloadState::Complete,
            DownloadState::Error,
            DownloadState::Cancelled,
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: DownloadState = serde_json::from_str(&json).unwrap();
            assert_eq!(
                deserialized, variant,
                "Roundtrip failed for {:?} (json: {})",
                variant, json
            );
        }
    }

    // ----------------------------------------------------------
    // DownloadRequest serde roundtrip
    // ----------------------------------------------------------

    /// Verifies that a `DownloadRequest` with URLs but no per-download
    /// options survives a serde roundtrip, ensuring the `options: None`
    /// case is handled correctly.
    #[test]
    fn download_request_serde_roundtrip_without_options() {
        let request = DownloadRequest {
            urls: vec![
                "https://music.apple.com/us/album/example/123456789".to_string(),
                "https://music.apple.com/us/album/another/987654321".to_string(),
            ],
            options: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: DownloadRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.urls, request.urls);
        assert!(deserialized.options.is_none());
    }

    /// Verifies that a `DownloadRequest` with both URLs and per-download
    /// option overrides survives a serde roundtrip, preserving the
    /// nested `GamdlOptions` structure.
    #[test]
    fn download_request_serde_roundtrip_with_options() {
        let request = DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test/111".to_string()],
            options: Some(GamdlOptions {
                song_codec: Some(super::super::gamdl_options::SongCodec::Alac),
                overwrite: Some(true),
                ..Default::default()
            }),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: DownloadRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.urls.len(), 1);
        assert!(deserialized.options.is_some());
        let opts = deserialized.options.unwrap();
        assert_eq!(opts.song_codec, Some(super::super::gamdl_options::SongCodec::Alac));
        assert_eq!(opts.overwrite, Some(true));
    }

    // ----------------------------------------------------------
    // QueueItemStatus serde roundtrip
    // ----------------------------------------------------------

    /// Verifies that a fully populated `QueueItemStatus` with all
    /// optional fields set survives a serde roundtrip, ensuring no
    /// field data is lost during serialization.
    #[test]
    fn queue_item_status_serde_roundtrip_all_fields() {
        let status = QueueItemStatus {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
            state: DownloadState::Downloading,
            progress: 42.5,
            current_track: Some("Track Name".to_string()),
            total_tracks: Some(12),
            completed_tracks: Some(5),
            speed: Some("2.5 MB/s".to_string()),
            eta: Some("00:45".to_string()),
            error: None,
            output_path: None,
            codec_used: Some("alac".to_string()),
            fallback_occurred: false,
            created_at: "2025-01-15T10:30:00.000Z".to_string(),
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: QueueItemStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, status.id);
        assert_eq!(deserialized.urls, status.urls);
        assert_eq!(deserialized.state, status.state);
        assert!((deserialized.progress - status.progress).abs() < f64::EPSILON);
        assert_eq!(deserialized.current_track, status.current_track);
        assert_eq!(deserialized.total_tracks, status.total_tracks);
        assert_eq!(deserialized.completed_tracks, status.completed_tracks);
        assert_eq!(deserialized.speed, status.speed);
        assert_eq!(deserialized.eta, status.eta);
        assert_eq!(deserialized.error, status.error);
        assert_eq!(deserialized.output_path, status.output_path);
        assert_eq!(deserialized.codec_used, status.codec_used);
        assert_eq!(deserialized.fallback_occurred, status.fallback_occurred);
        assert_eq!(deserialized.created_at, status.created_at);
    }

    /// Verifies that a `QueueItemStatus` in the error terminal state
    /// with an error message and all progress fields as `None` survives
    /// a serde roundtrip correctly.
    #[test]
    fn queue_item_status_serde_roundtrip_error_state() {
        let status = QueueItemStatus {
            id: "error-item-id".to_string(),
            urls: vec!["https://music.apple.com/us/album/fail/999".to_string()],
            state: DownloadState::Error,
            progress: 0.0,
            current_track: None,
            total_tracks: None,
            completed_tracks: None,
            speed: None,
            eta: None,
            error: Some("Network timeout after 30 seconds".to_string()),
            output_path: None,
            codec_used: None,
            fallback_occurred: false,
            created_at: "2025-02-01T08:00:00.000Z".to_string(),
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: QueueItemStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.state, DownloadState::Error);
        assert_eq!(deserialized.error, Some("Network timeout after 30 seconds".to_string()));
        assert!(deserialized.current_track.is_none());
        assert!(deserialized.speed.is_none());
    }

    /// Verifies that a `QueueItemStatus` in the complete terminal state
    /// with an output path and codec information survives a serde
    /// roundtrip, including the fallback_occurred flag.
    #[test]
    fn queue_item_status_serde_roundtrip_complete_with_fallback() {
        let status = QueueItemStatus {
            id: "complete-item-id".to_string(),
            urls: vec!["https://music.apple.com/us/album/done/555".to_string()],
            state: DownloadState::Complete,
            progress: 100.0,
            current_track: None,
            total_tracks: Some(1),
            completed_tracks: Some(1),
            speed: None,
            eta: None,
            error: None,
            output_path: Some("/Users/test/Music/Artist/Album/01 Track.m4a".to_string()),
            codec_used: Some("aac".to_string()),
            fallback_occurred: true,
            created_at: "2025-03-10T14:22:00.000Z".to_string(),
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: QueueItemStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.state, DownloadState::Complete);
        assert_eq!(deserialized.output_path, Some("/Users/test/Music/Artist/Album/01 Track.m4a".to_string()));
        assert_eq!(deserialized.codec_used, Some("aac".to_string()));
        assert!(deserialized.fallback_occurred);
        assert!((deserialized.progress - 100.0).abs() < f64::EPSILON);
    }
}
