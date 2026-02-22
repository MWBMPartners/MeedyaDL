// Copyright (c) 2024-2026 MeedyaDL
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
//   |     +-- Spotify     -> spotify_service::build_votify_command() [stub]
//   |
//   +-- service_dispatch::parse_service_output(service_id, line)
//         |
//         +-- AppleMusic  -> utils::process::parse_gamdl_output()
//         +-- Others      -> [stub, returns Unknown]
// ```
//
// ## Status
//
// Currently, only Apple Music (GAMDL) is fully implemented. Other services
// return "not yet implemented" errors. As each service is implemented, the
// dispatch functions will be updated to route to the real implementations.
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
            GamdlOutputEvent::TrackInfo { title, artist, album } => {
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
        }
    }
}

/// Checks whether a service is fully implemented and ready for downloads.
///
/// Currently only Apple Music is implemented. Other services will return
/// `false` until their respective service modules are completed.
///
/// # Arguments
/// * `service_id` - The service to check.
///
/// # Returns
/// `true` if the service is ready for downloads, `false` otherwise.
pub fn is_service_implemented(service_id: &MediaServiceId) -> bool {
    matches!(service_id, MediaServiceId::AppleMusic)
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

/// Checks whether a service is enabled by the remote service status config.
///
/// Reads the cached `service-status.json` from disk (fast, no network call).
/// Returns `true` if the service is enabled or if no cached status exists
/// (fail-open design).
///
/// # Arguments
/// * `service_id` - The service to check.
/// * `app` - Tauri AppHandle for resolving the cache file path.
///
/// # Returns
/// `true` if the service is enabled or no status is cached, `false` if
/// the service has been remotely disabled.
pub fn is_service_remotely_enabled(service_id: &MediaServiceId, app: &tauri::AppHandle) -> bool {
    match crate::services::service_status::load_cached_status(app) {
        Some(config) => !crate::services::service_status::is_service_disabled(&config, service_id),
        None => true, // No cache = fail-open
    }
}

/// Returns a user-friendly error message for a remotely disabled service.
///
/// Includes the service-specific message from the remote config if available.
pub fn service_disabled_error(service_id: &MediaServiceId, app: &tauri::AppHandle) -> String {
    let base_msg = format!(
        "{} downloads are temporarily unavailable.",
        service_id.display_name()
    );
    match crate::services::service_status::load_cached_status(app) {
        Some(config) => {
            match crate::services::service_status::get_service_message(&config, service_id) {
                Some(msg) => format!("{} {}", base_msg, msg),
                None => base_msg,
            }
        }
        None => base_msg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_service_implemented() {
        assert!(is_service_implemented(&MediaServiceId::AppleMusic));
        assert!(!is_service_implemented(&MediaServiceId::YouTube));
        assert!(!is_service_implemented(&MediaServiceId::BBCiPlayer));
        assert!(!is_service_implemented(&MediaServiceId::Spotify));
    }

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
