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

// NOTE: The following services will be added in later phases:
// pub mod download_queue;       // Phase 4: Queue management with fallback quality
// pub mod update_checker;       // Phase 5: Version update checking for all components
