// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Dependency management IPC commands.
// Handles checking installation status and installing Python, GAMDL,
// and external tool dependencies (FFmpeg, mp4decrypt, etc.).
// These commands are primarily used by the first-run setup wizard
// and the dependency status indicators throughout the UI.

use serde::Serialize;
use tauri::AppHandle;

use crate::utils::platform;

/// Status information for a single dependency (Python, GAMDL, or tool).
#[derive(Debug, Clone, Serialize)]
pub struct DependencyStatus {
    /// Human-readable name of the dependency (e.g., "Python 3.12")
    pub name: String,
    /// Whether this dependency is required for basic functionality
    pub required: bool,
    /// Whether the dependency is currently installed and accessible
    pub installed: bool,
    /// Installed version string, if available (e.g., "3.12.1", "2.8.4")
    pub version: Option<String>,
    /// Absolute path to the installed binary/executable
    pub path: Option<String>,
}

/// Checks whether a portable Python runtime is installed in the app data directory.
///
/// Returns true if the Python binary exists and is executable, along with
/// the detected version. Used by the setup wizard to determine if the
/// Python installation step can be skipped.
#[tauri::command]
pub async fn check_python_status(app: AppHandle) -> Result<DependencyStatus, String> {
    // Resolve the expected Python binary path for this platform
    let python_dir = platform::get_python_dir(&app);
    let python_bin = platform::get_python_binary_path(&python_dir);

    // Check if the Python binary exists at the expected location
    let installed = python_bin.exists();

    // If installed, try to get the version by running 'python --version'
    let version = if installed {
        match tokio::process::Command::new(&python_bin)
            .arg("--version")
            .output()
            .await
        {
            Ok(output) => {
                // Parse "Python 3.12.1" -> "3.12.1"
                let version_str = String::from_utf8_lossy(&output.stdout);
                Some(
                    version_str
                        .trim()
                        .strip_prefix("Python ")
                        .unwrap_or(version_str.trim())
                        .to_string(),
                )
            }
            Err(_) => None,
        }
    } else {
        None
    };

    Ok(DependencyStatus {
        name: "Python".to_string(),
        required: true,
        installed,
        version,
        path: python_bin.to_str().map(|s| s.to_string()),
    })
}

/// Downloads and installs a portable Python runtime.
///
/// Downloads from python-build-standalone GitHub releases, extracts to
/// the app data directory, and verifies the installation. Emits progress
/// events that the frontend setup wizard displays as a progress bar.
///
/// Returns the path to the installed Python binary on success.
#[tauri::command]
pub async fn install_python(app: AppHandle) -> Result<String, String> {
    // TODO: Phase 2 - Implement full Python download and installation
    // For now, return an error indicating this is not yet implemented
    let python_dir = platform::get_python_dir(&app);
    log::info!("Python will be installed to: {}", python_dir.display());
    Err("Python installation not yet implemented (Phase 2)".to_string())
}

/// Checks whether GAMDL is installed in the portable Python environment.
///
/// Runs 'pip show gamdl' to detect if the package is installed and
/// extracts the version number from the output.
#[tauri::command]
pub async fn check_gamdl_status(app: AppHandle) -> Result<DependencyStatus, String> {
    // First check if Python is available (GAMDL requires Python)
    let python_dir = platform::get_python_dir(&app);
    let python_bin = platform::get_python_binary_path(&python_dir);

    if !python_bin.exists() {
        return Ok(DependencyStatus {
            name: "GAMDL".to_string(),
            required: true,
            installed: false,
            version: None,
            path: None,
        });
    }

    // Check GAMDL installation by running 'python -m pip show gamdl'
    match tokio::process::Command::new(&python_bin)
        .args(["-m", "pip", "show", "gamdl"])
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse "Version: 2.8.4" from pip show output
            let version = stdout
                .lines()
                .find(|line| line.starts_with("Version:"))
                .map(|line| line.trim_start_matches("Version:").trim().to_string());
            let installed = version.is_some();

            Ok(DependencyStatus {
                name: "GAMDL".to_string(),
                required: true,
                installed,
                version,
                path: None, // GAMDL is a Python package, not a standalone binary
            })
        }
        Err(e) => Err(format!("Failed to check GAMDL status: {}", e)),
    }
}

/// Installs GAMDL via pip into the portable Python environment.
///
/// Runs 'pip install gamdl' and returns the installed version on success.
#[tauri::command]
pub async fn install_gamdl(app: AppHandle) -> Result<String, String> {
    // TODO: Phase 2 - Implement GAMDL pip installation
    let python_dir = platform::get_python_dir(&app);
    log::info!(
        "GAMDL will be installed via pip in: {}",
        python_dir.display()
    );
    Err("GAMDL installation not yet implemented (Phase 2)".to_string())
}

/// Checks the installation status of all external tool dependencies.
///
/// Returns a list of all dependencies (FFmpeg, mp4decrypt, N_m3u8DL-RE,
/// MP4Box) with their current installation status, version, and path.
#[tauri::command]
pub async fn check_all_dependencies(app: AppHandle) -> Result<Vec<DependencyStatus>, String> {
    let tools_dir = platform::get_tools_dir(&app);

    // Define all tool dependencies with their required/optional status
    let dependencies = vec![
        ("FFmpeg", true),
        ("mp4decrypt", false),
        ("N_m3u8DL-RE", false),
        ("MP4Box", false),
    ];

    let mut results = Vec::new();

    for (name, required) in dependencies {
        // Check if the tool binary exists in the tools directory
        let tool_path = tools_dir.join(name.to_lowercase());
        let installed = tool_path.exists();

        results.push(DependencyStatus {
            name: name.to_string(),
            required,
            installed,
            version: None, // TODO: Phase 2 - Run --version to get version
            path: if installed {
                tool_path.to_str().map(|s| s.to_string())
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
    // TODO: Phase 2 - Implement tool dependency installation
    let tools_dir = platform::get_tools_dir(&app);
    log::info!(
        "Tool '{}' will be installed to: {}",
        name,
        tools_dir.display()
    );
    Err(format!(
        "Dependency installation for '{}' not yet implemented (Phase 2)",
        name
    ))
}
