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
//   +-- download.rs      -- DownloadRequest, QueueItem, QueueStatus
//   +-- settings.rs      -- AppSettings, QualityPreference, OutputFormat
//   +-- gamdl_options.rs -- GamdlOptions (maps to GAMDL CLI flags)
//   +-- dependency.rs    -- DependencyInfo, DependencyStatus
//   +-- music_service.rs -- MusicService trait, service identifiers
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

/// Music service trait and extensibility types (GAMDL, gytmdl, votify).
///
/// Defines the `MusicService` trait for abstracting over different music
/// download backends, and concrete identifiers for each supported service.
/// This enables future extensibility beyond Apple Music (GAMDL).
pub mod music_service;

/// Crash report data model.
///
/// Defines the `CrashReport` struct representing a single crash or error
/// report. Used by the Rust panic handler, the frontend error logging
/// command, and the crash report service for CRUD operations.
pub mod crash_report;

/// Universal codec registry for multi-service support (v2 groundwork).
///
/// Defines canonical MeedyaDL codec IDs and per-service CLI flag mappings.
/// Loaded from `codecs.toml` (compiled into binary via `include_str!`).
/// Bridge functions connect the existing `SongCodec` enum to registry IDs.
pub mod codec_registry;

/// Metadata tag registry — config-driven tag definitions from tags.toml.
///
/// Defines which Apple Music API JSON fields map to which MP4 freeform atoms.
/// Loaded from `tags.toml` (compiled into binary via `include_str!`).
/// Adding new tags requires only editing the TOML file — zero Rust code changes.
pub mod tag_registry;
