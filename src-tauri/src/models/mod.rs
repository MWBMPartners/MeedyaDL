// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Data model modules.
// ====================
//
// This module aggregates all shared data types (structs, enums, traits)
// used across the `commands`, `services`, and `utils` layers. Models
// serve as the **common language** between the Rust backend and the
// React/TypeScript frontend: they are serialised to JSON when crossing
// the IPC boundary via Tauri's `invoke()` and event system.
//
// Design principles:
//   - All models that cross the IPC boundary derive `Serialize` (and
//     often `Deserialize`) from the `serde` crate.
//   - Models are plain data (no methods with side effects). Business
//     logic belongs in the `services` layer.
//   - Enum variants use `#[serde(rename_all = "snake_case")]` or
//     explicit `#[serde(rename = "...")]` to match the TypeScript
//     naming conventions used in the React frontend.
//
// Module map:
//   models/
//   +-- download.rs          -- DownloadRequest, QueueItem, QueueStatus
//   +-- settings.rs          -- AppSettings, QualityPreference, OutputFormat
//   +-- gamdl_options.rs     -- GamdlOptions (maps to GAMDL CLI flags)
//   +-- dependency.rs        -- DependencyInfo, DependencyStatus
//   +-- media_service.rs     -- MediaService trait, service identifiers
//   +-- download_options.rs  -- DownloadOptions enum (service-agnostic wrapper)
//   +-- ytdlp_options.rs     -- YtdlpOptions (maps to yt-dlp CLI flags)
//   +-- votify_options.rs    -- VotifyOptions (maps to votify CLI flags)
//   +-- get_iplayer_options.rs -- GetIplayerOptions (maps to get_iplayer CLI flags)
//
// Reference: https://serde.rs/
// Reference: https://v2.tauri.app/develop/calling-rust/#returning-data

/// Download request and queue item models.
///
/// Defines `DownloadRequest` (sent from the frontend to start a download),
/// `QueueItem` (tracks a single download's lifecycle in the queue), and
/// `QueueStatus` (returned by `get_queue_status` to summarise the queue).
pub mod download;

/// Application settings and quality preference models.
///
/// Defines `AppSettings` (the top-level settings object persisted by
/// tauri-plugin-store), `QualityPreference` (audio codec/quality chain),
/// and `OutputFormat` (M4A, FLAC, etc.).
pub mod settings;

/// GAMDL CLI option models (all supported command-line flags).
///
/// Defines `GamdlOptions` which maps every GAMDL CLI flag to a typed
/// Rust field. Used by `services::gamdl_service` to build the command
/// line for subprocess execution.
pub mod gamdl_options;

/// Dependency information models (Python, GAMDL, external tools).
///
/// Defines `DependencyInfo` (name, version, install path, status) and
/// `DependencyStatus` (installed/missing/outdated). Used by the
/// dependency checking and installation commands.
pub mod dependency;

/// Media service trait and extensibility types.
///
/// Defines the `MediaService` trait for abstracting over different media
/// download backends, and concrete identifiers for each supported service
/// (Apple Music, YouTube, BBC iPlayer, Spotify).
pub mod media_service;

/// Unified download options enum (service-agnostic wrapper).
///
/// Defines `DownloadOptions` which wraps service-specific option types
/// (`GamdlOptions`, `YtdlpOptions`, `VotifyOptions`, `GetIplayerOptions`)
/// into a tagged enum for the download queue to store and route.
pub mod download_options;

/// yt-dlp CLI option models for YouTube and BBC iPlayer downloads.
///
/// Defines `YtdlpOptions` which maps commonly used yt-dlp CLI flags
/// to typed Rust fields.
pub mod ytdlp_options;

/// Votify CLI option models for Spotify downloads.
///
/// Defines `VotifyOptions` which maps votify CLI flags to typed Rust fields.
pub mod votify_options;

/// get_iplayer CLI option models for BBC iPlayer downloads.
///
/// Defines `GetIplayerOptions` which maps get_iplayer CLI flags to typed
/// Rust fields.
pub mod get_iplayer_options;

/// Remote service status models for the kill-switch system.
///
/// Defines `ServiceStatusConfig` and `ServiceStatusEntry` which represent
/// the remote JSON configuration controlling per-service enable/disable
/// flags. Fetched from GitHub on launch and every 4 hours.
pub mod service_status;

/// Content matching models for the cross-platform Smart Download feature.
///
/// Defines `ContentIdentifiers` (ISRC, UPC, title/artist for matching),
/// `QualityTier` (normalised cross-platform quality ranking), `CrossPlatformMatch`
/// (a single service's match result), and `SmartDownloadResult` (the complete
/// search result returned to the frontend).
pub mod content_match;
