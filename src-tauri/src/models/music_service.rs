// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Music service trait definition.
// Defines the abstract interface that all music download services must
// implement. Currently only GAMDL (Apple Music) is supported, but this
// trait establishes the pattern for future services like gytmdl (YouTube Music)
// and votify (Spotify).
//
// This file is part of Phase 5's extensibility architecture. The trait is
// not yet used at runtime — it serves as the design contract for Phase 6+
// when multiple services will be integrated.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================
// Service Identification
// ============================================================

/// Identifies which music service a download request targets.
/// The frontend detects the service from the URL domain and passes
/// this value to the backend for routing to the correct service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MusicServiceId {
    /// Apple Music via GAMDL
    AppleMusic,
    /// YouTube Music via gytmdl (future)
    YouTubeMusic,
    /// Spotify via votify (future)
    Spotify,
}

impl MusicServiceId {
    /// Returns the human-readable display name for the service.
    pub fn display_name(&self) -> &'static str {
        match self {
            MusicServiceId::AppleMusic => "Apple Music",
            MusicServiceId::YouTubeMusic => "YouTube Music",
            MusicServiceId::Spotify => "Spotify",
        }
    }

    /// Returns the URL domain pattern(s) used to detect this service.
    /// Used by the URL parser to auto-detect which service a URL belongs to.
    pub fn url_domains(&self) -> &'static [&'static str] {
        match self {
            MusicServiceId::AppleMusic => &["music.apple.com"],
            MusicServiceId::YouTubeMusic => &["music.youtube.com"],
            MusicServiceId::Spotify => &["open.spotify.com"],
        }
    }

    /// Returns the PyPI package name for the service's CLI tool.
    pub fn pip_package(&self) -> &'static str {
        match self {
            MusicServiceId::AppleMusic => "gamdl",
            MusicServiceId::YouTubeMusic => "gytmdl",
            MusicServiceId::Spotify => "votify",
        }
    }

    /// Detects the service from a URL by matching against known domains.
    /// Returns None if the URL doesn't match any known service.
    pub fn from_url(url: &str) -> Option<Self> {
        let url_lower = url.to_lowercase();
        for service in [
            MusicServiceId::AppleMusic,
            MusicServiceId::YouTubeMusic,
            MusicServiceId::Spotify,
        ] {
            for domain in service.url_domains() {
                if url_lower.contains(domain) {
                    return Some(service);
                }
            }
        }
        None
    }
}

// ============================================================
// Service Capabilities
// ============================================================

/// Describes what features a music service supports.
/// The frontend uses this to enable/disable UI elements based on the
/// active service (e.g., hiding lyrics options for a service that
/// doesn't support lyrics).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCapabilities {
    /// Whether the service supports lossless audio downloads
    pub supports_lossless: bool,
    /// Whether the service supports spatial audio (Atmos, etc.)
    pub supports_spatial_audio: bool,
    /// Whether the service supports music video downloads
    pub supports_music_videos: bool,
    /// Whether the service supports synced lyrics
    pub supports_lyrics: bool,
    /// Whether the service supports cover art download
    pub supports_cover_art: bool,
    /// Whether the service requires cookie-based authentication
    pub requires_cookies: bool,
    /// Whether the service requires OAuth or token-based authentication
    pub requires_oauth: bool,
    /// Content types the service supports (e.g., "song", "album", "playlist")
    pub supported_content_types: Vec<String>,
}

// ============================================================
// Service Configuration
// ============================================================

/// Configuration specific to a music service instance.
/// Stored in the app settings and passed to the service on initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Which service this config applies to
    pub service_id: MusicServiceId,
    /// Whether this service is enabled by the user
    pub enabled: bool,
    /// Path to the service's CLI tool (or None for auto-detection)
    pub cli_path: Option<PathBuf>,
    /// Path to cookies file (for services requiring cookie auth)
    pub cookies_path: Option<PathBuf>,
    /// Custom output path override (or None to use the global output path)
    pub output_path: Option<PathBuf>,
}

// ============================================================
// Service Trait (async_trait pattern)
// ============================================================

/// The abstract interface for a music download service.
///
/// Each service implementation wraps a CLI tool (gamdl, gytmdl, votify)
/// and provides a consistent interface for:
/// - Checking installation status
/// - Building CLI arguments from typed options
/// - Executing downloads as subprocesses
/// - Parsing output into structured events
///
/// # Future Implementation
///
/// When implementing a new service:
/// 1. Create a new module (e.g., `services/gytmdl_service.rs`)
/// 2. Implement this trait for a struct wrapping the CLI tool
/// 3. Register the service in the command dispatcher
/// 4. Add the service's URL domains to `MusicServiceId`
/// 5. Add a sidebar navigation entry in the frontend
///
/// Note: This trait uses the `async_trait` pattern via boxed futures
/// because Rust does not yet have native async trait support in stable.
/// Once async traits stabilize, this can be simplified.
pub trait MusicService: Send + Sync {
    /// Returns the service identifier.
    fn id(&self) -> MusicServiceId;

    /// Returns the human-readable display name for this service.
    fn display_name(&self) -> &str;

    /// Returns the capabilities of this service (what it supports).
    fn capabilities(&self) -> ServiceCapabilities;

    /// Checks whether the service's CLI tool is installed and returns
    /// the version string if found.
    fn check_installed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<String>> + Send + '_>>;

    /// Installs the service's CLI tool via pip.
    /// Returns the installed version string on success.
    fn install(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>,
    >;

    /// Checks for updates to the service's CLI tool.
    /// Returns the latest available version string.
    fn check_update(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>,
    >;
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_id_from_url() {
        assert_eq!(
            MusicServiceId::from_url("https://music.apple.com/us/album/test/123"),
            Some(MusicServiceId::AppleMusic)
        );
        assert_eq!(
            MusicServiceId::from_url("https://music.youtube.com/watch?v=abc"),
            Some(MusicServiceId::YouTubeMusic)
        );
        assert_eq!(
            MusicServiceId::from_url("https://open.spotify.com/track/abc"),
            Some(MusicServiceId::Spotify)
        );
        assert_eq!(
            MusicServiceId::from_url("https://example.com/music"),
            None
        );
    }

    #[test]
    fn test_service_display_name() {
        assert_eq!(
            MusicServiceId::AppleMusic.display_name(),
            "Apple Music"
        );
        assert_eq!(
            MusicServiceId::YouTubeMusic.display_name(),
            "YouTube Music"
        );
        assert_eq!(MusicServiceId::Spotify.display_name(), "Spotify");
    }

    #[test]
    fn test_service_pip_package() {
        assert_eq!(MusicServiceId::AppleMusic.pip_package(), "gamdl");
        assert_eq!(MusicServiceId::YouTubeMusic.pip_package(), "gytmdl");
        assert_eq!(MusicServiceId::Spotify.pip_package(), "votify");
    }
}
