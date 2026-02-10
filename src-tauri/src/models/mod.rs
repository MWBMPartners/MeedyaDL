// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Data model modules.
// These structures define the shared types used across commands,
// services, and IPC communication with the frontend.

/// Download request and queue item models
pub mod download;

/// Application settings and quality preference models
pub mod settings;

/// GAMDL CLI option models (all supported command-line flags)
pub mod gamdl_options;

/// Dependency information models (Python, GAMDL, tools)
pub mod dependency;

/// Music service trait and extensibility types (GAMDL, gytmdl, votify)
pub mod music_service;
