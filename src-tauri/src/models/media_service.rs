// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Media service trait definition.
// Defines the abstract interface that all media download services must
// implement. Currently only GAMDL (Apple Music) is supported, but this
// trait establishes the pattern for future services like votify (Spotify),
// yt-dlp (YouTube), and get_iplayer (BBC iPlayer).
//
// This file is part of Phase 5's extensibility architecture. The trait is
// not yet used at runtime -- it serves as the design contract for Phase 6+
// when multiple services will be integrated.
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
/// 4. Implement the `MediaService` trait for a new struct.
/// 5. Register the implementation in the command dispatcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MediaServiceId {
    /// Apple Music -- supported via the GAMDL CLI tool.
    /// This is the only service currently implemented at runtime.
    /// CLI tool: `gamdl` (`PyPI`: <https://pypi.org/project/gamdl/>).
    AppleMusic,

    /// YouTube Music -- will use the gytmdl CLI tool.
    /// CLI tool: `gytmdl` (`PyPI`: <https://pypi.org/project/gytmdl/>).
    YouTubeMusic,

    /// YouTube (generic) -- will use yt-dlp.
    /// CLI tool: `yt-dlp` (`PyPI`: <https://pypi.org/project/yt-dlp/>).
    YouTube,

    /// Spotify -- will use the votify CLI tool.
    /// CLI tool: `votify` (`PyPI`: <https://pypi.org/project/votify/>).
    Spotify,

    /// BBC iPlayer -- will use get_iplayer with yt-dlp fallback.
    /// Region-restricted to the UK.
    BBCiPlayer,
}

/// Formats the service ID as a kebab-case platform identifier string
/// matching the keys in `engines.toml` (e.g., `"apple-music"`, `"spotify"`).
///
/// This is used throughout the codebase for engine registry lookups,
/// manifest metadata, queue persistence, and service-aware routing.
/// For human-readable names, use `display_name()` instead.
impl std::fmt::Display for MediaServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id = match self {
            Self::AppleMusic => "apple-music",
            Self::YouTubeMusic => "youtube-music",
            Self::YouTube => "youtube",
            Self::Spotify => "spotify",
            Self::BBCiPlayer => "bbc-iplayer",
        };
        write!(f, "{id}")
    }
}

impl MediaServiceId {
    /// Returns the human-readable display name for the service.
    ///
    /// Used in the React frontend's sidebar, status messages, and error
    /// dialogs to identify which service a download is associated with.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::AppleMusic => "Apple Music",
            Self::YouTubeMusic => "YouTube Music",
            Self::YouTube => "YouTube",
            Self::Spotify => "Spotify",
            Self::BBCiPlayer => "BBC iPlayer",
        }
    }

    /// Returns the remote feature-availability flag key for this service.
    ///
    /// Used by `services::feature_flag_service::service_gate` to look a
    /// service up in the resolved verdict map. The keys are namespaced with
    /// the `service-` prefix, matching the `core-` / `feature-` convention
    /// documented on `FeatureFlagsSnapshot::verdicts`.
    ///
    /// ## Kebab-case is required, not stylistic
    ///
    /// The backend's `InputSanitizer::slug()` key grammar is
    /// `^[a-z0-9-]+$` (max 100 chars) and **rejects dots outright**, so a
    /// dotted key such as `"service.apple-music"` is a string the server
    /// could never create or serve — it would resolve as an unknown key and
    /// silently fail open forever. The unit test below pins the grammar.
    ///
    /// Deliberately distinct from `Display` (which yields the *engine
    /// registry* platform id, e.g. `"apple-music"`) even though the two
    /// currently differ only by the prefix: the flag key is a wire contract
    /// with MWBM-IntAppsAPI, whereas the platform id is a local
    /// `engines.toml` lookup key. Coupling them would make a future rename
    /// on either side silently break the other.
    #[must_use]
    pub const fn flag_key(&self) -> &'static str {
        match self {
            Self::AppleMusic => "service-apple-music",
            Self::YouTubeMusic => "service-youtube-music",
            Self::YouTube => "service-youtube",
            Self::Spotify => "service-spotify",
            Self::BBCiPlayer => "service-bbc-iplayer",
        }
    }

    /// Returns the URL domain pattern(s) used to detect this service.
    ///
    /// Used by `from_url()` to auto-detect which service a pasted URL
    /// belongs to. Each service may have multiple domains (e.g., if a
    /// service has regional subdomains). The detection uses a simple
    /// `String::contains()` check, so these are substring patterns
    /// rather than full-domain matches.
    #[must_use]
    pub const fn url_domains(&self) -> &'static [&'static str] {
        match self {
            Self::AppleMusic => &["music.apple.com", "classical.apple.com", "itunes.apple.com"],
            // YouTube Music must be checked before YouTube (more specific domain first)
            Self::YouTubeMusic => &["music.youtube.com"],
            Self::YouTube => &["youtube.com", "youtu.be"],
            Self::Spotify => &["open.spotify.com"],
            Self::BBCiPlayer => &["bbc.co.uk/iplayer", "bbc.co.uk/sounds"],
        }
    }

    /// Returns the `PyPI` package name for the service's CLI tool.
    ///
    /// Used by the dependency management system (`commands/dependency.rs`)
    /// to install and update the CLI tool via `pip install <package>`.
    /// Each service's CLI tool is distributed as a Python package on `PyPI`.
    #[must_use]
    pub const fn pip_package(&self) -> &'static str {
        match self {
            Self::AppleMusic => "gamdl",
            Self::YouTubeMusic => "gytmdl",
            Self::YouTube | Self::BBCiPlayer => "yt-dlp",
            Self::Spotify => "votify",
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
    /// wins. This is O(services * `domains_per_service`), which is trivial
    /// given the small number of services.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use meedyadl::models::media_service::MediaServiceId;
    /// let id = MediaServiceId::from_url("https://music.apple.com/us/album/test/123");
    /// assert_eq!(id, Some(MediaServiceId::AppleMusic));
    /// ```
    #[must_use]
    pub fn from_url(url: &str) -> Option<Self> {
        // Lowercase the URL once so domain matching is case-insensitive.
        let url_lower = url.to_lowercase();
        // Iterate over all known services. The order does not matter
        // because each service has unique, non-overlapping domains.
        // Order matters: YouTubeMusic must come before YouTube since
        // music.youtube.com contains "youtube.com".
        for service in [Self::AppleMusic, Self::YouTubeMusic, Self::YouTube, Self::Spotify, Self::BBCiPlayer] {
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
/// | Capability           | Apple Music | `YouTube` Music | Spotify (planned) |
/// |----------------------|:-----------:|:-------------:|:-----------------:|
/// | Lossless audio       |     Yes     |      No       |       No*         |
/// | Spatial audio        |     Yes     |      No       |       No          |
/// | Music videos         |     Yes     |      Yes      |       No          |
/// | Synced lyrics        |     Yes     |      No       |       Yes         |
/// | Cover art            |     Yes     |      Yes      |       Yes         |
/// | Requires cookies     |     Yes     |      Yes      |       No          |
/// | Requires OAuth       |     No      |      No       |       Yes         |
///
/// *Spotify offers lossless via `HiFi` tier but votify may not support it yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Independent capability flags for each media service
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
    pub service_id: MediaServiceId,

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
/// 3. Implement `MediaService` for that struct.
/// 4. Add a variant to `MediaServiceId` and update its methods.
/// 5. Register the new service in the Tauri app builder (see `main.rs`).
/// 6. Add a sidebar entry in the React frontend's navigation component.
///
/// ## Async methods and boxed futures
///
/// This trait uses manually boxed futures
/// (`Pin<Box<dyn Future<...> + Send + '_>>`) instead of `async fn`
/// because Rust's native async trait methods (stabilized in Rust 1.75)
/// are not yet object-safe in all scenarios we need. Specifically, we
/// need `dyn MediaService` trait objects for the service registry, and
/// `async fn` in traits requires `impl Trait` return types that are
/// not object-safe. The `async_trait` crate is an alternative, but we
/// avoid the macro dependency by using explicit boxing.
///
/// ## References
///
/// - Rust traits: <https://doc.rust-lang.org/book/ch10-02-traits.html>
/// - Object safety: <https://doc.rust-lang.org/reference/items/traits.html#object-safety>
/// - Pin and boxing futures: <https://doc.rust-lang.org/std/pin/struct.Pin.html>
pub trait MediaService: Send + Sync {
    /// Returns the unique identifier for this service.
    ///
    /// Used by the download manager to route URLs to the correct service
    /// and by the frontend to look up service-specific configuration.
    fn id(&self) -> MediaServiceId;

    /// Returns the human-readable display name for this service.
    ///
    /// Shown in the UI sidebar, status bar, and error messages.
    /// Typically delegates to `MediaServiceId::display_name()`.
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
    /// `<package>` comes from `MediaServiceId::pip_package()`) and
    /// return the installed version string on success, or an error
    /// message on failure.
    ///
    /// This is an async operation because installation involves
    /// network I/O and subprocess execution.
    fn install(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>>;

    /// Checks for updates to the service's CLI tool by querying the
    /// upstream package registry (`PyPI`).
    ///
    /// Returns `Ok("x.y.z")` with the latest available version, or
    /// `Err(message)` if the check failed (e.g., network error).
    /// The caller compares this with the installed version to determine
    /// whether an update is available.
    fn check_update(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + '_>>;
}

// ============================================================
// Tests
// ============================================================

/// Unit tests for `MediaServiceId` methods.
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
        // Apple Music (all three domains)
        assert_eq!(
            MediaServiceId::from_url("https://music.apple.com/us/album/test/123"),
            Some(MediaServiceId::AppleMusic)
        );
        assert_eq!(
            MediaServiceId::from_url("https://classical.apple.com/us/album/test/123"),
            Some(MediaServiceId::AppleMusic)
        );
        assert_eq!(
            MediaServiceId::from_url("https://itunes.apple.com/us/album/test/123"),
            Some(MediaServiceId::AppleMusic)
        );
        // YouTube Music (must be detected before generic YouTube)
        assert_eq!(
            MediaServiceId::from_url("https://music.youtube.com/watch?v=abc"),
            Some(MediaServiceId::YouTubeMusic)
        );
        // YouTube (generic)
        assert_eq!(
            MediaServiceId::from_url("https://www.youtube.com/watch?v=abc"),
            Some(MediaServiceId::YouTube)
        );
        assert_eq!(
            MediaServiceId::from_url("https://youtu.be/abc"),
            Some(MediaServiceId::YouTube)
        );
        // Spotify
        assert_eq!(
            MediaServiceId::from_url("https://open.spotify.com/track/abc"),
            Some(MediaServiceId::Spotify)
        );
        // BBC iPlayer
        assert_eq!(
            MediaServiceId::from_url("https://www.bbc.co.uk/iplayer/episode/b0000001"),
            Some(MediaServiceId::BBCiPlayer)
        );
        assert_eq!(
            MediaServiceId::from_url("https://www.bbc.co.uk/sounds/play/m001234"),
            Some(MediaServiceId::BBCiPlayer)
        );
        // Unknown
        assert_eq!(MediaServiceId::from_url("https://example.com/music"), None);
    }

    /// Verifies that `display_name()` returns the expected user-facing
    /// strings for each service variant.
    #[test]
    fn test_service_display_name() {
        assert_eq!(MediaServiceId::AppleMusic.display_name(), "Apple Music");
        assert_eq!(MediaServiceId::YouTubeMusic.display_name(), "YouTube Music");
        assert_eq!(MediaServiceId::YouTube.display_name(), "YouTube");
        assert_eq!(MediaServiceId::Spotify.display_name(), "Spotify");
        assert_eq!(MediaServiceId::BBCiPlayer.display_name(), "BBC iPlayer");
    }

    /// Verifies that every `flag_key()` value satisfies the backend's key
    /// grammar (`^[a-z0-9-]+$`) and that no two services share a key.
    ///
    /// The grammar check is the important half: MWBM-IntAppsAPI's
    /// `InputSanitizer::slug()` rejects dots, uppercase, and underscores, so
    /// a key that fails this assertion could never be served — the gate for
    /// that service would fail open permanently and silently. Uniqueness is
    /// checked because two services sharing a key would make one service's
    /// pause silently pause the other as well.
    #[test]
    fn test_service_flag_keys_are_kebab_case_and_unique() {
        let all = [
            MediaServiceId::AppleMusic,
            MediaServiceId::YouTubeMusic,
            MediaServiceId::YouTube,
            MediaServiceId::Spotify,
            MediaServiceId::BBCiPlayer,
        ];

        let mut seen = std::collections::HashSet::new();
        for service in all {
            let key = service.flag_key();

            // `^[a-z0-9-]+$` — asserted without pulling in the regex crate
            // for a five-element check.
            assert!(!key.is_empty(), "{service:?} has an empty flag key");
            assert!(
                key.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "flag key '{key}' for {service:?} must match ^[a-z0-9-]+$ \
                 (the backend key sanitiser rejects dots, underscores and uppercase)"
            );
            assert!(
                key.len() <= 100,
                "flag key '{key}' exceeds the server's 100-character key limit"
            );
            assert!(
                key.starts_with("service-"),
                "flag key '{key}' should carry the 'service-' namespace prefix"
            );
            assert!(
                seen.insert(key),
                "flag key '{key}' is used by more than one service"
            );
        }

        // Spot-check the exact wire values so a rename is a deliberate,
        // visible change rather than a silent contract break.
        assert_eq!(MediaServiceId::AppleMusic.flag_key(), "service-apple-music");
        assert_eq!(MediaServiceId::Spotify.flag_key(), "service-spotify");
        assert_eq!(MediaServiceId::BBCiPlayer.flag_key(), "service-bbc-iplayer");
    }

    /// Verifies that `pip_package()` returns the correct PyPI package
    /// name for each service, ensuring the dependency manager installs
    /// the right package.
    #[test]
    fn test_service_pip_package() {
        assert_eq!(MediaServiceId::AppleMusic.pip_package(), "gamdl");
        assert_eq!(MediaServiceId::YouTubeMusic.pip_package(), "gytmdl");
        assert_eq!(MediaServiceId::YouTube.pip_package(), "yt-dlp");
        assert_eq!(MediaServiceId::Spotify.pip_package(), "votify");
        assert_eq!(MediaServiceId::BBCiPlayer.pip_package(), "yt-dlp");
    }
}
