// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Service modules containing the core business logic.
// Services are called by command handlers and encapsulate all
// interactions with external tools, the filesystem, and APIs.

/// Python runtime manager: download/install/verify portable Python
/// from python-build-standalone GitHub releases
pub mod python_manager;

/// GAMDL CLI wrapper: install via pip, execute downloads as subprocesses,
/// parse output into structured events for the frontend
pub mod gamdl_service;

/// Dependency manager: download/install external tools (FFmpeg, mp4decrypt,
/// N_m3u8DL-RE, MP4Box) from their official release sources
pub mod dependency_manager;

/// Settings and config service: load/save JSON settings, sync to
/// GAMDL's config.ini format for CLI compatibility
pub mod config_service;

/// Download queue manager: queue management with concurrent execution,
/// fallback quality chain retries, and cancellation support
pub mod download_queue;

/// Update checker: check for new versions of GAMDL, Python, tools,
/// and the app itself from PyPI and GitHub Releases
pub mod update_checker;
