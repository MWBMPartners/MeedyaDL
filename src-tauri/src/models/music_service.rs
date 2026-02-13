// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Music service trait definition.
// Defines the abstract interface that all music download services must
// implement. Currently only GAMDL (Apple Music) is supported, but this
// trait establishes the pattern for future services like gytmdl (YouTube Music)
// and votify (Spotify).
//
// This file is part of Phase 5's extensibility architecture. The trait is
// not yet used at runtime -- it serves as the design contract for Phase 6+
// when multiple services will be integrated.
//
// ## Extensibility pattern
//
// The design follows the Strategy pattern via Rust traits:
//
// 1. `MusicServiceId` -- an enum identifying each service. URL detection
//    and service routing use this enum.
// 2. `ServiceCapabilities` -- a struct describing what each service
//    supports. The frontend queries this to enable/disable UI elements.
// 3. `ServiceConfig` -- per-service configuration persisted in settings.
// 4. `MusicService` trait -- the abstract interface that service
//    implementations must satisfy. Each implementation wraps a CLI tool.
//
// To add a new service, follow the steps documented on the `MusicService`
// trait definition below.
//
// ## References
//
// - Rust traits: <https://doc.rust-lang.org/book/ch10-02-traits.html>
// - Strategy pattern in Rust: <https://refactoring.guru/design-patterns/strategy>
// - GAMDL (Apple Music): <https://github.com/glomatico/gamdl>
// - gytmdl (YouTube Music): <https://github.com/glomatico/gytmdl>
// - votify (Spotify): <https://github.com/glomatico/votify>
// - serde: <https://docs.rs/serde/latest/serde/>

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================
// Service Identification
// ============================================================

/// Identifies which music service a download request targets.
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
/// 2. Add entries in `display_name()`, `url_domains()`, and `pip_package()`.
/// 3. Update `from_url()` to iterate over the new variant.
/// 4. Implement the `MusicService` trait for a new struct.
/// 5. Register the implementation in the command dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MusicServiceId {
    /// Apple Music -- supported via the GAMDL CLI tool.
    /// This is the only service currently implemented at runtime.
    /// CLI tool: `gamdl` (PyPI: <https://pypi.org/project/gamdl/>).
    AppleMusic,

    /// YouTube Music -- planned for Phase 6+, will use the gytmdl CLI tool.
    /// CLI tool: `gytmdl` (PyPI: <https://pypi.org/project/gytmdl/>).
    /// Not yet implemented; included here to define the interface contract.
    YouTubeMusic,

    /// Spotify -- planned for Phase 6+, will use the votify CLI tool.
    /// CLI tool: `votify` (PyPI: <https://pypi.org/project/votify/>).
    /// Not yet implemented; included here to define the interface contract.
    Spotify,
}

impl MusicServiceId {
    /// Returns the human-readable display name for the service.
    ///
    /// Used in the React frontend's sidebar, status messages, and error
    /// dialogs to identify which service a download is associated with.
    pub fn display_name(&self) -> &'static str {
        match self {
            MusicServiceId::AppleMusic => "Apple Music",
            MusicServiceId::YouTubeMusic => "YouTube Music",
            MusicServiceId::Spotify => "Spotify",
        }
    }

    /// Returns the URL domain pattern(s) used to detect this service.
    ///
    /// Used by `from_url()` to auto-detect which service a pasted URL
    /// belongs to. Each service may have multiple domains (e.g., if a
    /// service has regional subdomains). The detection uses a simple
    /// `String::contains()` check, so these are substring patterns
    /// rather than full-domain matches.
    pub fn url_domains(&self) -> &'static [&'static str] {
        match self {
            MusicServiceId::AppleMusic => &["music.apple.com"],
            MusicServiceId::YouTubeMusic => &["music.youtube.com"],
            MusicServiceId::Spotify => &["open.spotify.com"],
        }
    }

    /// Returns the PyPI package name for the service's CLI tool.
    ///
    /// Used by the dependency management system (`commands/dependency.rs`)
    /// to install and update the CLI tool via `pip install <package>`.
    /// Each service's CLI tool is distributed as a Python package on PyPI.
    pub fn pip_package(&self) -> &'static str {
        match self {
            MusicServiceId::AppleMusic => "gamdl",
            MusicServiceId::YouTubeMusic => "gytmdl",
            MusicServiceId::Spotify => "votify",
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
    /// Iterates over every `MusicServiceId` variant and checks each
    /// variant's `url_domains()` against the input URL. The first match
    /// wins. This is O(services * domains_per_service), which is trivial
    /// given the small number of services.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use meedyadl::models::music_service::MusicServiceId;
    /// let id = MusicServiceId::from_url("https://music.apple.com/us/album/test/123");
    /// assert_eq!(id, Some(MusicServiceId::AppleMusic));
    /// ```
    pub fn from_url(url: &str) -> Option<Self> {
        // Lowercase the URL once so domain matching is case-insensitive.
        let url_lower = url.to_lowercase();
        // Iterate over all known services. The order does not matter
        // because each service has unique, non-overlapping domains.
        for service in [
            MusicServiceId::AppleMusic,
            MusicServiceId::YouTubeMusic,
            MusicServiceId::Spotify,
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

/// Describes what features a music service supports.
///
/// The frontend queries this via the `get_service_capabilities` Tauri
/// command and uses the flags to conditionally render UI elements. For
/// example, if `supports_lyrics` is `false`, the lyrics format dropdown
/// is hidden in the settings panel for that service.
///
/// ## Per-service capability examples
///
/// | Capability           | Apple Music | YouTube Music | Spotify (planned) |
/// |----------------------|:-----------:|:-------------:|:-----------------:|
/// | Lossless audio       |     Yes     |      No       |       No*         |
/// | Spatial audio        |     Yes     |      No       |       No          |
/// | Music videos         |     Yes     |      Yes      |       No          |
/// | Synced lyrics        |     Yes     |      No       |       Yes         |
/// | Cover art            |     Yes     |      Yes      |       Yes         |
/// | Requires cookies     |     Yes     |      Yes      |       No          |
/// | Requires OAuth       |     No      |      No       |       Yes         |
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
    /// visibility of the entire "Video Quality" settings section.
    pub supports_music_videos: bool,

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

/// Configuration specific to a music service instance.
///
/// Stored in the application settings (future: as part of a
/// `Vec<ServiceConfig>` in `AppSettings`) and passed to the service
/// implementation on initialization. Each service gets its own config
/// so users can, for example, use different output paths or cookies
/// files for different services.
///
/// ## Relationship to `AppSettings`
///
/// Currently, Apple Music settings are stored directly in `AppSettings`
/// fields (e.g., `cookies_path`, `output_path`). When multi-service
/// support is added in Phase 6+, these will be migrated into per-service
/// `ServiceConfig` instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Which service this config applies to. Used as the lookup key
    /// when the download manager needs to find the config for a
    /// given service.
    pub service_id: MusicServiceId,

    /// Whether this service is enabled by the user. Disabled services
    /// are hidden from the sidebar and their URLs are rejected by the
    /// download manager.
    pub enabled: bool,

    /// Absolute path to the service's CLI tool binary. `None` means
    /// auto-detect from the managed installation in the app data
    /// directory (see `dependency.rs`). Users can override this with
    /// a custom path if they have their own installation.
    pub cli_path: Option<PathBuf>,

    /// Path to a Netscape-format cookies file for services that
    /// require cookie-based authentication (see
    /// `ServiceCapabilities::requires_cookies`). `None` when the
    /// service uses OAuth or when cookies have not been configured.
    pub cookies_path: Option<PathBuf>,

    /// Custom output path override for this service's downloads.
    /// `None` means use the global `AppSettings::output_path`. Allows
    /// users to organize downloads from different services into
    /// separate directories.
    pub output_path: Option<PathBuf>,
}

// ============================================================
// Service Trait (async_trait pattern)
// ============================================================

/// The abstract interface for a music download service.
///
/// Each service implementation wraps a CLI tool (`gamdl`, `gytmdl`,
/// `votify`) and provides a consistent interface for:
///
/// - **Discovery**: checking installation status and version.
/// - **Installation**: installing/updating the CLI tool via pip.
/// - **Capability reporting**: telling the frontend what features
///   the service supports so the UI can adapt.
///
/// This trait is the core of the application's extensibility architecture.
/// By programming against this interface, the download manager and
/// frontend can support multiple services without knowing the details
/// of each CLI tool.
///
/// ## Adding a new service (Phase 6+ guide)
///
/// 1. Create a new module: `services/<name>_service.rs`.
/// 2. Define a struct (e.g., `GytmdlService`) that holds any
///    service-specific state (CLI path, config, etc.).
/// 3. Implement `MusicService` for that struct.
/// 4. Add a variant to `MusicServiceId` and update its methods.
/// 5. Register the new service in the Tauri app builder (see `main.rs`).
/// 6. Add a sidebar entry in the React frontend's navigation component.
///
/// ## Async methods and boxed futures
///
/// This trait uses manually boxed futures
/// (`Pin<Box<dyn Future<...> + Send + '_>>`) instead of `async fn`
/// because Rust's native async trait methods (stabilized in Rust 1.75)
/// are not yet object-safe in all scenarios we need. Specifically, we
/// need `dyn MusicService` trait objects for the service registry, and
/// `async fn` in traits requires `impl Trait` return types that are
/// not object-safe. The `async_trait` crate is an alternative, but we
/// avoid the macro dependency by using explicit boxing.
///
/// ## References
///
/// - Rust traits: <https://doc.rust-lang.org/book/ch10-02-traits.html>
/// - Object safety: <https://doc.rust-lang.org/reference/items/traits.html#object-safety>
/// - Pin and boxing futures: <https://doc.rust-lang.org/std/pin/struct.Pin.html>
pub trait MusicService: Send + Sync {
    /// Returns the unique identifier for this service.
    ///
    /// Used by the download manager to route URLs to the correct service
    /// and by the frontend to look up service-specific configuration.
    fn id(&self) -> MusicServiceId;

    /// Returns the human-readable display name for this service.
    ///
    /// Shown in the UI sidebar, status bar, and error messages.
    /// Typically delegates to `MusicServiceId::display_name()`.
    fn display_name(&self) -> &str;

    /// Returns the capability descriptor for this service.
    ///
    /// The frontend calls this on startup and caches the result to
    /// conditionally render UI elements (e.g., hiding the "Video Quality"
    /// section for services that do not support music videos).
    fn capabilities(&self) -> ServiceCapabilities;

    /// Checks whether the service's CLI tool is installed and returns
    /// the version string if found.
    ///
    /// Implementation should run the CLI tool with a `--version` flag
    /// and parse the output. Returns `Some("x.y.z")` if installed,
    /// `None` if not found or the version could not be determined.
    ///
    /// This is an async operation because it spawns a subprocess.
    fn check_installed(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<String>> + Send + '_>>;

    /// Installs the service's CLI tool via pip into the managed
    /// virtual environment.
    ///
    /// Implementation should run `pip install <package>` (where
    /// `<package>` comes from `MusicServiceId::pip_package()`) and
    /// return the installed version string on success, or an error
    /// message on failure.
    ///
    /// This is an async operation because installation involves
    /// network I/O and subprocess execution.
    fn install(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>,
    >;

    /// Checks for updates to the service's CLI tool by querying the
    /// upstream package registry (PyPI).
    ///
    /// Returns `Ok("x.y.z")` with the latest available version, or
    /// `Err(message)` if the check failed (e.g., network error).
    /// The caller compares this with the installed version to determine
    /// whether an update is available.
    fn check_update(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>,
    >;
}

// ============================================================
// Tests
// ============================================================

/// Unit tests for `MusicServiceId` methods.
///
/// These tests verify URL detection, display names, and pip package
/// names for all service variants. They run as part of `cargo test`
/// and do not require any external dependencies or network access.
#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that `from_url()` correctly identifies the service from
    /// various URL formats, including full album/track/playlist URLs
    /// and returns `None` for unrecognised domains.
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

    /// Verifies that `display_name()` returns the expected user-facing
    /// strings for each service variant.
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

    /// Verifies that `pip_package()` returns the correct PyPI package
    /// name for each service, ensuring the dependency manager installs
    /// the right package.
    #[test]
    fn test_service_pip_package() {
        assert_eq!(MusicServiceId::AppleMusic.pip_package(), "gamdl");
        assert_eq!(MusicServiceId::YouTubeMusic.pip_package(), "gytmdl");
        assert_eq!(MusicServiceId::Spotify.pip_package(), "votify");
    }
}
