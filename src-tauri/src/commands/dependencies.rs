// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Dependency management IPC commands.
// Handles checking installation status and installing Python, GAMDL,
// and external tool dependencies (FFmpeg, mp4decrypt, etc.).
// These commands are primarily used by the first-run setup wizard
// and the dependency status indicators throughout the UI.
//
// Delegates to service modules for the actual installation logic.

use serde::Serialize;
use tauri::AppHandle;

use crate::services::{dependency_manager, gamdl_service, python_manager};

/// Status information for a single dependency (Python, GAMDL, or tool).
/// Returned to the frontend for display in the setup wizard and status bar.
#[derive(Debug, Clone, Serialize)]
pub struct DependencyStatus {
    /// Human-readable name of the dependency (e.g., "Python 3.12")
    pub name: String,
    /// Whether this dependency is required for basic functionality
    pub required: bool,
    /// Whether the dependency is currently installed and accessible
    pub installed: bool,
    /// Installed version string, if available (e.g., "3.12.8", "2.8.4")
    pub version: Option<String>,
    /// Absolute path to the installed binary/executable
    pub path: Option<String>,
}

/// Checks whether the portable Python runtime is installed in the app data directory.
///
/// Returns status information including whether Python exists, its version,
/// and its binary path. Used by the setup wizard to determine if the
/// Python installation step can be skipped.
#[tauri::command]
pub async fn check_python_status(app: AppHandle) -> Result<DependencyStatus, String> {
    let version = python_manager::check_python_status(&app).await?;

    let python_dir = crate::utils::platform::get_python_dir(&app);
    let python_bin = crate::utils::platform::get_python_binary_path(&python_dir);

    Ok(DependencyStatus {
        name: format!("Python {}", python_manager::expected_python_version()),
        required: true,
        installed: version.is_some(),
        version,
        path: python_bin.to_str().map(|s| s.to_string()),
    })
}

/// Downloads and installs a portable Python runtime.
///
/// Downloads from python-build-standalone GitHub releases, extracts to
/// the app data directory, and verifies the installation. The frontend
/// should poll check_python_status() for progress, or listen for log events.
///
/// Returns the installed Python version string on success.
#[tauri::command]
pub async fn install_python(app: AppHandle) -> Result<String, String> {
    python_manager::install_python(&app).await
}

/// Checks whether GAMDL is installed in the portable Python environment.
///
/// Runs 'python -m pip show gamdl' to detect the package and extract
/// the version number from the output.
#[tauri::command]
pub async fn check_gamdl_status(app: AppHandle) -> Result<DependencyStatus, String> {
    let version = gamdl_service::get_gamdl_version(&app).await?;

    Ok(DependencyStatus {
        name: "GAMDL".to_string(),
        required: true,
        installed: version.is_some(),
        version,
        path: None, // GAMDL is a Python package, not a standalone binary
    })
}

/// Installs GAMDL via pip into the portable Python environment.
///
/// Runs 'pip install --upgrade gamdl' and returns the installed version.
/// Python must already be installed before calling this command.
#[tauri::command]
pub async fn install_gamdl(app: AppHandle) -> Result<String, String> {
    gamdl_service::install_gamdl(&app).await
}

/// Checks the installation status of all external tool dependencies.
///
/// Returns a list of all dependencies (FFmpeg, mp4decrypt, N_m3u8DL-RE,
/// MP4Box) with their current installation status, including whether
/// they're required and whether a binary exists at the expected path.
#[tauri::command]
pub async fn check_all_dependencies(app: AppHandle) -> Result<Vec<DependencyStatus>, String> {
    let tools = dependency_manager::get_all_tools();
    let mut results = Vec::new();

    for tool in tools {
        let binary_path = dependency_manager::get_tool_binary_path(&app, tool.id);
        let installed = binary_path.exists();

        results.push(DependencyStatus {
            name: tool.name.to_string(),
            required: tool.required,
            installed,
            version: None, // Version detection is slow; skip for batch checks
            path: if installed {
                binary_path.to_str().map(|s| s.to_string())
            } else {
                None
            },
        });
    }

    Ok(results)
}

/// Downloads and installs a specific tool dependency.
///
/// Determines the correct download URL for the current platform,
/// downloads the archive, extracts it, and verifies the binary works.
///
/// # Arguments
/// * `name` - The tool name: "ffmpeg", "mp4decrypt", "nm3u8dlre", or "mp4box"
#[tauri::command]
pub async fn install_dependency(app: AppHandle, name: String) -> Result<String, String> {
    dependency_manager::install_tool(&app, &name).await
}
