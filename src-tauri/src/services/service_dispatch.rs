// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Service dispatch module.
// =========================
//
// Provides a service-agnostic dispatch layer that routes download operations
// to the correct service backend based on `MediaServiceId`. This module acts
// as the central routing point between the download queue and the individual
// service implementations (GAMDL, yt-dlp, get_iplayer, votify).
//
// ## Architecture
//
// ```
// DownloadQueue
//   |
//   +-- service_dispatch::build_service_command(service_id, ...)
//   |     |
//   |     +-- AppleMusic  -> gamdl_service::build_gamdl_command_public()
//   |     +-- YouTube     -> youtube_service::build_ytdlp_command() [stub]
//   |     +-- BBCiPlayer  -> bbc_iplayer_service::build_get_iplayer_command() [stub]
//   |     +-- Spotify     -> spotify_service::build_votify_command() [real, M9]
//   |
//   +-- service_dispatch::parse_service_output(service_id, line)
//         |
//         +-- AppleMusic  -> utils::process::parse_gamdl_output()
//         +-- Others      -> [stub, returns Unknown]
// ```
//
// ## Status
//
// Apple Music (GAMDL) is fully implemented, and so is Spotify
// (`spotify_service`, M9) — though Spotify still sits behind a
// dev-access-only preview flag until it's ready for regular users, so
// it doesn't show up as a normal option yet. YouTube and BBC iPlayer
// still return "not yet implemented" errors from every function in
// this module's dispatch table. As each remaining service is
// implemented, the dispatch functions will be updated to route to the
// real implementation. (This module previously carried an
// `is_service_implemented()` helper meant to answer "is this service
// ready?" in one place — it was deleted because it had no caller
// anywhere except its own test, and having gone stale unnoticed once
// already — hardcoding only Apple Music as implemented well after
// Spotify became real — it was a bigger risk left in than removed. If
// a real caller needs this answer, prefer asking the engine registry
// / feature-flag gate directly rather than re-adding a second,
// hand-maintained source of truth.)
//
// ## Remote enable/disable does NOT live here
//
// This module used to carry `is_service_remotely_enabled()` and
// `service_disabled_error()`, which read the interim `service_status.json`
// transport. Both were removed: they had zero call sites, and leaving them
// in place invited a future implementer to wire enforcement to the dead
// backend.
//
// Remote availability is now owned by
// `services::feature_flag_service::service_gate(app, &MediaServiceId)`,
// resolved from the feature-flag verdict map (in-memory snapshot -> sticky
// disk cache -> compiled all-enabled defaults). Call it at **enqueue seams
// only** — `start_download`, `retry_download`, `retry_failed_bulk`,
// `import_queue` — never from `process_queue`, startup recovery, fallback
// retries, companions or enrichment, so a pause can stop new work without
// ever stranding a download that is already in flight.
//
// ## References
//
// - Strategy pattern: each service implements its own command builder
// - `GamdlOutputEvent` in `utils/process.rs`: the existing event format
// - `MediaServiceId` in `models/media_service.rs`: the service identifier enum

use serde::Serialize;

use crate::models::media_service::MediaServiceId;

/// Service-agnostic output event.
///
/// Wraps the output events from different download service backends into a
/// unified format. This allows the download queue to process output events
/// from any service without knowing the specifics of each service's output
/// format.
///
/// The variants mirror `GamdlOutputEvent` from `utils/process.rs` but are
/// designed to be service-agnostic. As new services are added, their output
/// parsers will produce `ServiceOutputEvent` values directly.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServiceOutputEvent {
    /// Information about the track/content currently being processed.
    TrackInfo {
        title: String,
        artist: String,
        album: String,
    },

    /// Download progress update.
    DownloadProgress {
        percent: f64,
        speed: String,
        eta: String,
    },

    /// A post-download processing step (remuxing, tagging, etc.).
    ProcessingStep {
        step: String,
    },

    /// An error occurred during the download.
    Error {
        message: String,
    },

    /// Download completed successfully.
    Complete {
        path: String,
    },

    /// Unrecognized output line.
    Unknown {
        raw: String,
    },
}

/// Converts a `GamdlOutputEvent` into a `ServiceOutputEvent`.
///
/// This is a 1:1 mapping since `ServiceOutputEvent` was designed to be
/// a superset of `GamdlOutputEvent`. Used by the Apple Music path in the
/// download queue to normalize output events.
impl From<crate::utils::process::GamdlOutputEvent> for ServiceOutputEvent {
    fn from(event: crate::utils::process::GamdlOutputEvent) -> Self {
        use crate::utils::process::GamdlOutputEvent;
        match event {
            GamdlOutputEvent::TrackInfo { title, artist, album, track_number: _, track_total: _ } => {
                ServiceOutputEvent::TrackInfo { title, artist, album }
            }
            GamdlOutputEvent::DownloadProgress { percent, speed, eta } => {
                ServiceOutputEvent::DownloadProgress { percent, speed, eta }
            }
            GamdlOutputEvent::ProcessingStep { step } => {
                ServiceOutputEvent::ProcessingStep { step }
            }
            GamdlOutputEvent::Error { message } => {
                ServiceOutputEvent::Error { message }
            }
            GamdlOutputEvent::Complete { path } => {
                ServiceOutputEvent::Complete { path }
            }
            GamdlOutputEvent::Unknown { raw } => {
                ServiceOutputEvent::Unknown { raw }
            }
            // Two additional GamdlOutputEvent variants that landed in
            // alpha after this branch was forked (#660 traceback-noise
            // suppression + #698 codec-skip handling). Map both to the
            // service-level `Unknown` bucket — `ServiceOutputEvent`
            // isn't read from anywhere today, and the precise variant
            // distinction is only meaningful inside the per-engine
            // parser (download_queue keys off these directly via the
            // GamdlOutputEvent enum, not via this conversion). When a
            // future per-service parser produces structured equivalents
            // we can grow the enum then.
            GamdlOutputEvent::TracebackFrame { raw } => {
                ServiceOutputEvent::Unknown { raw }
            }
            GamdlOutputEvent::CodecSkip { message, .. } => {
                ServiceOutputEvent::Unknown { raw: message }
            }
        }
    }
}

/// Returns a user-friendly "not yet implemented" error message for a service.
///
/// Used by the download queue and command handlers when a user tries to
/// download from a service that isn't yet implemented.
///
/// # Arguments
/// * `service_id` - The service that was requested.
///
/// # Returns
/// A formatted error string with the service name.
pub fn not_implemented_error(service_id: &MediaServiceId) -> String {
    format!(
        "{} downloads are not yet implemented. Coming soon!",
        service_id.display_name()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_implemented_error() {
        let err = not_implemented_error(&MediaServiceId::YouTube);
        assert!(err.contains("YouTube"));
        assert!(err.contains("Coming soon"));
    }

    #[test]
    fn test_gamdl_event_conversion() {
        use crate::utils::process::GamdlOutputEvent;

        let gamdl_event = GamdlOutputEvent::DownloadProgress {
            percent: 42.5,
            speed: "1.2MiB/s".to_string(),
            eta: "00:30".to_string(),
        };

        let service_event: ServiceOutputEvent = gamdl_event.into();
        match service_event {
            ServiceOutputEvent::DownloadProgress { percent, speed, eta } => {
                assert!((percent - 42.5).abs() < f64::EPSILON);
                assert_eq!(speed, "1.2MiB/s");
                assert_eq!(eta, "00:30");
            }
            _ => panic!("Expected DownloadProgress variant"),
        }
    }
}
