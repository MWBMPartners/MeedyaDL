// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Platform utilities for OS detection and path resolution.
// All application data (Python, GAMDL, tools, settings) is stored
// in a self-contained directory within the OS app data location
// to avoid conflicts with system installations.

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// Returns the root application data directory for gamdl-GUI.
///
/// This is the self-contained directory where all application data lives:
/// - Python runtime
/// - GAMDL installation
/// - External tools (FFmpeg, mp4decrypt, etc.)
/// - Application settings
/// - GAMDL configuration
///
/// Platform-specific locations:
/// - macOS: ~/Library/Application Support/com.mwbmpartners.gamdl-gui/
/// - Windows: %APPDATA%\com.mwbmpartners.gamdl-gui\
/// - Linux: ~/.local/share/com.mwbmpartners.gamdl-gui/
pub fn get_app_data_dir(app: &AppHandle) -> PathBuf {
    // Use Tauri's built-in app data directory resolution
    // which reads the identifier from tauri.conf.json
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| {
            // Fallback: construct the path manually using the dirs crate
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("com.mwbmpartners.gamdl-gui")
        })
}

/// Returns the directory where the portable Python runtime is installed.
///
/// Path: {app_data}/python/
/// Contains the full Python installation extracted from python-build-standalone.
pub fn get_python_dir(app: &AppHandle) -> PathBuf {
    get_app_data_dir(app).join("python")
}

/// Returns the path to the Python binary for the current platform.
///
/// - macOS/Linux: {python_dir}/bin/python3
/// - Windows: {python_dir}/python.exe
pub fn get_python_binary_path(python_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        // Windows: Python executable is at the root of the installation
        python_dir.join("python.exe")
    } else {
        // macOS/Linux: Python executable is in the bin subdirectory
        python_dir.join("bin").join("python3")
    }
}

/// Returns the path to the pip binary for the current platform.
///
/// - macOS/Linux: {python_dir}/bin/pip3
/// - Windows: {python_dir}/Scripts/pip.exe
pub fn get_pip_binary_path(python_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        python_dir.join("Scripts").join("pip.exe")
    } else {
        python_dir.join("bin").join("pip3")
    }
}

/// Returns the directory where external tools are installed.
///
/// Path: {app_data}/tools/
/// Each tool gets its own subdirectory (e.g., tools/ffmpeg/, tools/mp4decrypt/).
pub fn get_tools_dir(app: &AppHandle) -> PathBuf {
    get_app_data_dir(app).join("tools")
}

/// Returns the directory for GAMDL-specific data (config, cache).
///
/// Path: {app_data}/gamdl/
/// Contains GAMDL's config.ini and any temporary files.
pub fn get_gamdl_data_dir(app: &AppHandle) -> PathBuf {
    get_app_data_dir(app).join("gamdl")
}

/// Returns the path to the GAMDL config file.
///
/// Path: {app_data}/gamdl/config.ini
/// This is passed to GAMDL via --config-path to keep it self-contained.
pub fn get_gamdl_config_path(app: &AppHandle) -> PathBuf {
    get_gamdl_data_dir(app).join("config.ini")
}
