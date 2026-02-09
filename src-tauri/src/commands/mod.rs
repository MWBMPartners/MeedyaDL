// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Command modules for Tauri IPC handlers.
// Each sub-module exposes #[tauri::command] functions that the React frontend
// can call via the invoke() API. Commands are thin wrappers that delegate
// to the services layer for business logic.

/// System information commands (platform detection, directory paths)
pub mod system;

/// Dependency management commands (Python, GAMDL, FFmpeg, mp4decrypt, etc.)
pub mod dependencies;

/// Application settings commands (read, write, validate)
pub mod settings;

/// GAMDL download execution commands (start, cancel, status)
pub mod gamdl;

/// Secure credential storage commands (store, retrieve, delete)
pub mod credentials;
