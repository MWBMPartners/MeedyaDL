// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Media service trait definition.
// Defines the abstract interface that all media download services must
// implement. Currently only GAMDL (Apple Music) is fully supported, but this
// trait establishes the pattern for YouTube (yt-dlp), BBC iPlayer (get_iplayer),
// and Spotify (votify).
//
// ## Extensibility pattern
//
// The design follows the Strategy pattern via Rust traits:
//
// 1. `MediaServiceId` -- an enum identifying each service. URL detection
//    and service routing use this enum.
// 2. `ServiceCapabilities` -- a struct describing what each service
//    supports. The frontend queries this to enable/disable UI elements.
// 3. `ServiceConfig` -- per-service configuration persisted in settings.
// 4. `MediaService` trait -- the abstract interface that service
//    implementations must satisfy. Each implementation wraps a CLI tool.
//
// To add a new service, follow the steps documented on the `MediaService`
// trait definition below.
//
// ## References
//
// - Rust traits: <https://doc.rust-lang.org/book/ch10-02-traits.html>
// - Strategy pattern in Rust: <https://refactoring.guru/design-patterns/strategy>
// - GAMDL (Apple Music): <https://github.com/glomatico/gamdl>
// - yt-dlp (YouTube): <https://github.com/yt-dlp/yt-dlp>
// - get_iplayer (BBC iPlayer): <https://github.com/get-iplayer/get_iplayer>
// - votify (Spotify): <https://github.com/glomatico/votify>
// - serde: <https://docs.rs/serde/latest/serde/>

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================
// Service Identification
// ============================================================

/// Identifies which media service a download request targets.
///
/// The frontend detects the service from the URL domain (via `from_url()`)
/// and passes this value to the backend for routing to the correct service
/// implementation. This enum is also used as a key in `HashMap`s and
/// `HashSet`s (it derives `Eq` + `Hash`), for example when storing
/// per-service configurations.
///
/// ## Derive traits
///
/// - `Copy` -- small enum, no heap data; pass by value.
/// - `Eq + Hash` -- allows use as HashMap/HashSet keys.
/// - `Serialize + Deserialize` -- for Tauri IPC and settings persistence.
///
/// ## Adding a new service
///
/// 1. Add a new variant to this enum.
/// 2. Add entries in `display_name()`, `url_domains()`, `pip_package()`,
///    and `cli_tool_name()`.
/// 3. Update `from_url()` to iterate over the new variant.
/// 4. Implement the `MediaService` trait for a new struct.
/// 5. Register the implementation in the service dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaServiceId {
    /// Apple Music -- supported via the GAMDL CLI tool.
    /// This is the only service currently implemented at runtime.
    /// CLI tool: `gamdl` (PyPI: <https://pypi.org/project/gamdl/>).
    AppleMusic,

    /// YouTube -- general YouTube support (youtube.com + music.youtube.com).
    /// Handles video, music, playlists, channels, and live streams.
    /// CLI tool: `yt-dlp` (PyPI: <https://pypi.org/project/yt-dlp/>).
    /// Not yet implemented; included here to define the interface contract.
    YouTube,

    /// BBC iPlayer -- UK public broadcast on-demand content.
    /// Handles TV shows, movies, audio programmes, and short clips.
    /// Primary CLI tool: `get_iplayer` with `yt-dlp` as fallback.
    /// Not yet implemented; included here to define the interface contract.
    BBCiPlayer,

    /// Spotify -- planned support via the votify CLI tool.
    /// CLI tool: `votify` (PyPI: <https://pypi.org/project/votify/>).
    /// Not yet implemented; included here to define the interface contract.
    Spotify,
}

impl Default for MediaServiceId {
    /// Defaults to `AppleMusic` for backward compatibility with existing
    /// `DownloadRequest` and `QueueItemStatus` data that predates the
    /// `service_id` field.
    fn default() -> Self {
        Self::AppleMusic
    }
}

impl MediaServiceId {
    /// Returns the human-readable display name for the service.
    ///
    /// Used in the React frontend's sidebar, status messages, and error
    /// dialogs to identify which service a download is associated with.
    pub fn display_name(&self) -> &'static str {
        match self {
            MediaServiceId::AppleMusic => "Apple Music",
            MediaServiceId::YouTube => "YouTube",
            MediaServiceId::BBCiPlayer => "BBC iPlayer",
            MediaServiceId::Spotify => "Spotify",
        }
    }

    /// Returns the URL domain pattern(s) used to detect this service.
    ///
    /// Used by `from_url()` to auto-detect which service a pasted URL
    /// belongs to. Each service may have multiple domains. The detection
    /// uses a simple `String::contains()` check, so these are substring
    /// patterns rather than full-domain matches.
    ///
    /// Note: YouTube domains are ordered with `music.youtube.com` first
    /// to ensure it matches before the more general `youtube.com`.
    pub fn url_domains(&self) -> &'static [&'static str] {
        match self {
            MediaServiceId::AppleMusic => &["music.apple.com", "itunes.apple.com"],
            MediaServiceId::YouTube => &["music.youtube.com", "youtube.com", "youtu.be"],
            MediaServiceId::BBCiPlayer => &["bbc.co.uk/iplayer", "bbc.co.uk/sounds"],
            MediaServiceId::Spotify => &["open.spotify.com"],
        }
    }

    /// Returns the PyPI package name for the service's primary CLI tool.
    ///
    /// Used by the dependency management system (`commands/dependency.rs`)
    /// to install and update the CLI tool via `pip install <package>`.
    /// Note: BBC iPlayer's `get_iplayer` is not pip-installed; it uses
    /// yt-dlp as its pip dependency and get_iplayer is installed separately.
    pub fn pip_package(&self) -> &'static str {
        match self {
            MediaServiceId::AppleMusic => "gamdl",
            MediaServiceId::YouTube => "yt-dlp",
            MediaServiceId::BBCiPlayer => "yt-dlp",
            MediaServiceId::Spotify => "votify",
        }
    }

    /// Returns the primary CLI tool name for this service.
    ///
    /// This is the binary name used when building subprocess commands.
    /// Distinct from `pip_package()` because BBC iPlayer uses `get_iplayer`
    /// as its primary tool but shares yt-dlp as a pip dependency/fallback.
    pub fn cli_tool_name(&self) -> &'static str {
        match self {
            MediaServiceId::AppleMusic => "gamdl",
            MediaServiceId::YouTube => "yt-dlp",
            MediaServiceId::BBCiPlayer => "get_iplayer",
            MediaServiceId::Spotify => "votify",
        }
    }

    /// Detects the service from a URL by matching against known domains.
    ///
    /// Returns `Some(service_id)` if the URL contains a recognised domain
    /// substring, or `None` if no service matches. The comparison is
    /// case-insensitive (the URL is lowercased before matching).
    ///
    /// ## Algorithm
    ///
    /// Iterates over every `MediaServiceId` variant and checks each
    /// variant's `url_domains()` against the input URL. The first match
    /// wins. BBC iPlayer is checked before YouTube because `bbc.co.uk`
    /// is more specific than `youtube.com`.
    pub fn from_url(url: &str) -> Option<Self> {
        // Lowercase the URL once so domain matching is case-insensitive.
        let url_lower = url.to_lowercase();
        // Iterate over all known services. BBC iPlayer is checked before
        // YouTube because `bbc.co.uk` is more specific than `youtube.com`.
        for service in [
            MediaServiceId::AppleMusic,
            MediaServiceId::BBCiPlayer,
            MediaServiceId::Spotify,
            MediaServiceId::YouTube,
        ] {
            // Check each domain pattern for this service.
            for domain in service.url_domains() {
                if url_lower.contains(domain) {
                    return Some(service);
                }
            }
        }
        // No known service domain found in the URL.
        None
    }
}

// ============================================================
// Service Capabilities
// ============================================================

/// Describes what features a media service supports.
///
/// The frontend queries this via Tauri commands and uses the flags to
/// conditionally render UI elements. For example, if `supports_lyrics`
/// is `false`, the lyrics format dropdown is hidden in the settings
/// panel for that service.
///
/// ## Per-service capability examples
///
/// | Capability             | Apple Music | YouTube | BBC iPlayer | Spotify |
/// |------------------------|:-----------:|:-------:|:-----------:|:-------:|
/// | Lossless audio         |     Yes     |   No    |      No     |   No*   |
/// | Spatial audio          |     Yes     |   No    |      No     |   No    |
/// | Music videos           |     Yes     |   Yes   |      No     |   No    |
/// | General video content  |     No      |   Yes   |     Yes     |   No    |
/// | Live streams           |     No      |   Yes   |      No     |   No    |
/// | Synced lyrics          |     Yes     |   No    |      No     |   Yes   |
/// | Cover art              |     Yes     |   Yes   |     Yes     |   Yes   |
/// | Requires cookies       |     Yes     |   Yes   |      No     |   No    |
/// | Requires OAuth         |     No      |   No    |      No     |   Yes   |
///
/// *Spotify offers lossless via HiFi tier but votify may not support it yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCapabilities {
    /// Whether the service supports lossless audio downloads (e.g., ALAC
    /// for Apple Music). Controls visibility of lossless codec options.
    pub supports_lossless: bool,

    /// Whether the service supports spatial audio formats (e.g., Dolby
    /// Atmos, AC-3). Controls visibility of spatial audio codec options.
    pub supports_spatial_audio: bool,

    /// Whether the service supports music video downloads. Controls
    /// visibility of the "Video Quality" settings section.
    pub supports_music_videos: bool,

    /// Whether the service supports general video content (TV shows,
    /// movies, clips) beyond music videos. For YouTube and BBC iPlayer.
    pub supports_video_content: bool,

    /// Whether the service supports live stream recording/downloading.
    /// Currently only YouTube supports this.
    pub supports_live_streams: bool,

    /// Whether the service supports synced (time-stamped) lyrics.
    /// Controls visibility of the lyrics format and lyrics-related
    /// toggles in the settings panel.
    pub supports_lyrics: bool,

    /// Whether the service supports downloading cover art as a separate
    /// image file. Controls visibility of the cover art settings.
    pub supports_cover_art: bool,

    /// Whether the service requires a Netscape-format cookies file for
    /// authentication (exported from a logged-in browser session).
    /// When `true`, the settings panel shows a cookies file picker.
    pub requires_cookies: bool,

    /// Whether the service requires OAuth or token-based authentication
    /// (e.g., Spotify's authorization flow). When `true`, the settings
    /// panel shows an "Authenticate" button instead of a cookies picker.
    pub requires_oauth: bool,

    /// Content types the service supports as a list of strings (e.g.,
    /// `["song", "album", "playlist", "music-video"]`). The frontend
    /// uses this to validate pasted URLs and show appropriate error
    /// messages for unsupported content types.
    pub supported_content_types: Vec<String>,
}

// ============================================================
// Service Configuration
// ============================================================

/// Configuration specific to a media service instance.
///
/// Stored in the application settings and passed to the service
/// implementation on initialization. Each service gets its own config
/// so users can, for example, use different output paths or cookies
/// files for different services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Which service this config applies to. Used as the lookup key
    /// when the download manager needs to find the config for a
    /// given service.
    pub service_id: MediaServiceId,

    /// Whether this service is enabled by the user. Disabled services
    /// are hidden from the sidebar and their URLs are rejected by the
    /// download manager.
    pub enabled: bool,

    /// Absolute path to the service's CLI tool binary. `None` means
    /// auto-detect from the managed installation in the app data
    /// directory (see `dependency.rs`).
    pub cli_path: Option<PathBuf>,

    /// Path to a Netscape-format cookies file for services that
    /// require cookie-based authentication. `None` when the service
    /// uses OAuth or when cookies have not been configured.
    pub cookies_path: Option<PathBuf>,

    /// Custom output path override for this service's downloads.
    /// `None` means use the global `AppSettings::output_path`.
    pub output_path: Option<PathBuf>,
}

// ============================================================
// Service Trait (async_trait pattern)
// ============================================================

/// The abstract interface for a media download service.
///
/// Each service implementation wraps a CLI tool (`gamdl`, `yt-dlp`,
/// `get_iplayer`, `votify`) and provides a consistent interface for:
///
/// - **Discovery**: checking installation status and version.
/// - **Installation**: installing/updating the CLI tool via pip.
/// - **Capability reporting**: telling the frontend what features
///   the service supports so the UI can adapt.
///
/// ## Adding a new service
///
/// 1. Create a new module: `services/<name>_service.rs`.
/// 2. Define a struct that holds service-specific state.
/// 3. Implement `MediaService` for that struct.
/// 4. Add a variant to `MediaServiceId` and update its methods.
/// 5. Register the new service in the service dispatcher.
/// 6. Add per-service settings and a settings tab in the frontend.
///
/// ## Async methods and boxed futures
///
/// This trait uses manually boxed futures instead of `async fn`
/// because native async trait methods are not yet object-safe in all
/// scenarios we need (`dyn MediaService` trait objects for the service
/// registry).
pub trait MediaService: Send + Sync {
    /// Returns the unique identifier for this service.
    fn id(&self) -> MediaServiceId;

    /// Returns the human-readable display name for this service.
    fn display_name(&self) -> &str;

    /// Returns the capability descriptor for this service.
    fn capabilities(&self) -> ServiceCapabilities;

    /// Checks whether the service's CLI tool is installed and returns
    /// the version string if found.
    fn check_installed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<String>> + Send + '_>>;

    /// Installs the service's CLI tool via pip into the managed
    /// virtual environment.
    fn install(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>,
    >;

    /// Checks for updates to the service's CLI tool by querying the
    /// upstream package registry (PyPI).
    fn check_update(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>,
    >;
}

// ============================================================
// Tests
// ============================================================

/// Unit tests for `MediaServiceId` methods.
#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that `from_url()` correctly identifies the service from
    /// various URL formats and returns `None` for unrecognised domains.
    #[test]
    fn test_service_id_from_url() {
        // Apple Music URLs
        assert_eq!(
            MediaServiceId::from_url("https://music.apple.com/us/album/test/123"),
            Some(MediaServiceId::AppleMusic)
        );

        // YouTube URLs (general youtube.com)
        assert_eq!(
            MediaServiceId::from_url("https://www.youtube.com/watch?v=abc"),
            Some(MediaServiceId::YouTube)
        );

        // YouTube Music URLs (music.youtube.com)
        assert_eq!(
            MediaServiceId::from_url("https://music.youtube.com/watch?v=abc"),
            Some(MediaServiceId::YouTube)
        );

        // YouTube short URLs (youtu.be)
        assert_eq!(
            MediaServiceId::from_url("https://youtu.be/abc123"),
            Some(MediaServiceId::YouTube)
        );

        // BBC iPlayer URLs
        assert_eq!(
            MediaServiceId::from_url("https://www.bbc.co.uk/iplayer/episode/p0abc123"),
            Some(MediaServiceId::BBCiPlayer)
        );

        // BBC Sounds URLs
        assert_eq!(
            MediaServiceId::from_url("https://www.bbc.co.uk/sounds/play/p0abc123"),
            Some(MediaServiceId::BBCiPlayer)
        );

        // Spotify URLs
        assert_eq!(
            MediaServiceId::from_url("https://open.spotify.com/track/abc"),
            Some(MediaServiceId::Spotify)
        );

        // Unknown URLs
        assert_eq!(
            MediaServiceId::from_url("https://example.com/music"),
            None
        );
    }

    /// Verifies that `display_name()` returns the expected user-facing
    /// strings for each service variant.
    #[test]
    fn test_service_display_name() {
        assert_eq!(MediaServiceId::AppleMusic.display_name(), "Apple Music");
        assert_eq!(MediaServiceId::YouTube.display_name(), "YouTube");
        assert_eq!(MediaServiceId::BBCiPlayer.display_name(), "BBC iPlayer");
        assert_eq!(MediaServiceId::Spotify.display_name(), "Spotify");
    }

    /// Verifies that `pip_package()` returns the correct PyPI package
    /// name for each service.
    #[test]
    fn test_service_pip_package() {
        assert_eq!(MediaServiceId::AppleMusic.pip_package(), "gamdl");
        assert_eq!(MediaServiceId::YouTube.pip_package(), "yt-dlp");
        assert_eq!(MediaServiceId::BBCiPlayer.pip_package(), "yt-dlp");
        assert_eq!(MediaServiceId::Spotify.pip_package(), "votify");
    }

    /// Verifies that `cli_tool_name()` returns the correct binary name
    /// for each service's primary CLI tool.
    #[test]
    fn test_service_cli_tool_name() {
        assert_eq!(MediaServiceId::AppleMusic.cli_tool_name(), "gamdl");
        assert_eq!(MediaServiceId::YouTube.cli_tool_name(), "yt-dlp");
        assert_eq!(MediaServiceId::BBCiPlayer.cli_tool_name(), "get_iplayer");
        assert_eq!(MediaServiceId::Spotify.cli_tool_name(), "votify");
    }

    /// Verifies that the default `MediaServiceId` is `AppleMusic`.
    #[test]
    fn test_default_is_apple_music() {
        assert_eq!(MediaServiceId::default(), MediaServiceId::AppleMusic);
    }
}
