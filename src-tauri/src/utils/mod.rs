// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Utility modules providing cross-cutting concerns.
// ==================================================
//
// This module aggregates three utility sub-modules that are used throughout
// the application by both the `commands` and `services` layers. None of
// these modules hold state; they are purely functional helpers.
//
// Module map:
//   utils/
//   +-- platform.rs   -- OS detection and path resolution
//   +-- archive.rs    -- HTTP download + archive extraction (ZIP, TAR.GZ)
//   +-- process.rs    -- GAMDL subprocess output parsing (regex-based)
//
// These utilities are imported by services like `python_manager`,
// `gamdl_service`, and `dependency_manager` to perform platform-specific
// file operations, download and unpack release archives, and parse GAMDL's
// stdout/stderr into structured events for the React frontend.

/// Platform detection, path resolution, and OS-specific utilities.
///
/// Provides functions to resolve the app data directory, Python binary
/// path, pip binary path, tools directory, and GAMDL config path for
/// the current operating system (macOS, Windows, Linux).
///
/// Used by: `services::python_manager`, `services::dependency_manager`,
///          `services::config_service`, `commands::system`
pub mod platform;

/// Archive download and extraction utilities (ZIP, TAR.GZ).
///
/// Provides an async `download_file()` that streams HTTP responses to disk
/// with progress logging, plus format-specific extractors (`extract_zip`,
/// `extract_tar_gz`) that run synchronous I/O on Tokio's blocking thread
/// pool. The high-level `download_and_extract()` combines both steps.
///
/// Used by: `services::python_manager`, `services::dependency_manager`
pub mod archive;

/// Subprocess stdout/stderr parsing for GAMDL CLI output.
///
/// Uses compiled regex patterns (via `LazyLock`) to parse yt-dlp-style
/// download progress lines, GAMDL track information, post-processing
/// steps, error messages, and file-save confirmations into a
/// `GamdlOutputEvent` enum that the frontend can render as a progress UI.
///
/// Used by: `services::gamdl_service`, `services::download_queue`
pub mod process;

/// App-wide activity log emission helpers.
///
/// Provides `emit_download_log()` and `emit_app_log()` functions for emitting
/// structured activity log events from any backend module. Centralises the
/// `ActivityLogEvent` struct that was previously private to `download_queue.rs`.
///
/// Used by: `services::download_queue`, `services::metadata_tag_service`,
///          `commands::gamdl`, `commands::updates`, `commands::dependencies`,
///          `commands::cookies`, `commands::settings`, `commands::login_window`
pub mod activity_log;

/// IPC command rate limiter.
///
/// Simple sliding-window rate limiter that prevents abuse of expensive
/// commands (downloads, API calls, updates). Each command has a configurable
/// max calls / window seconds limit.
///
/// Used by: `commands::gamdl`, `commands::updates`, `commands::cookies`
pub mod rate_limiter;
