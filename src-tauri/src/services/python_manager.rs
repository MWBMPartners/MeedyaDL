// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Python runtime manager service.
// Downloads, installs, and manages a portable Python runtime from the
// python-build-standalone project (https://github.com/indygreg/python-build-standalone).
// The portable Python is installed into the app data directory and is
// completely self-contained, avoiding conflicts with any system Python.

use std::path::PathBuf;
use tauri::AppHandle;

use crate::utils::{archive, platform};

// ============================================================
// Python version constants
// Update these when a new Python release is available.
// The release tag and version must match an actual release on GitHub.
// ============================================================

/// The Python version to install (used in the download URL and version checks)
const PYTHON_VERSION: &str = "3.12.8";

/// The python-build-standalone release tag on GitHub (date-based)
const RELEASE_TAG: &str = "20250106";

/// The base URL for python-build-standalone release downloads
const DOWNLOAD_BASE_URL: &str =
    "https://github.com/indygreg/python-build-standalone/releases/download";

/// Constructs the download URL for the portable Python archive matching
/// the current platform and architecture.
///
/// Uses the python-build-standalone project's install_only variant, which
/// provides a minimal Python installation suitable for running pip and
/// Python packages like GAMDL.
///
/// # URL format
/// `{base}/{tag}/cpython-{version}+{tag}-{triple}-install_only.tar.gz`
///
/// # Returns
/// * `Ok(url)` - The download URL for the current platform
/// * `Err(message)` - If the current platform is unsupported
fn get_python_download_url() -> Result<String, String> {
    // Determine the platform triple for python-build-standalone
    let triple = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        (os, arch) => {
            return Err(format!(
                "Unsupported platform for portable Python: {}/{}",
                os, arch
            ))
        }
    };

    // Build the full download URL
    let url = format!(
        "{}/{}/cpython-{}+{}-{}-install_only.tar.gz",
        DOWNLOAD_BASE_URL, RELEASE_TAG, PYTHON_VERSION, RELEASE_TAG, triple
    );

    log::info!("Python download URL: {}", url);
    Ok(url)
}

/// Downloads and installs the portable Python runtime.
///
/// This is the main entry point called by the setup wizard's Python
/// installation step. It performs the following:
/// 1. Determines the correct download URL for the current platform
/// 2. Cleans up any previous installation in the Python directory
/// 3. Downloads the tar.gz archive from python-build-standalone
/// 4. Extracts the archive to the app data directory
/// 5. Verifies the Python binary exists and works
/// 6. Returns the installed Python version string
///
/// The archive extracts to a `python/` subdirectory, which matches
/// the path returned by `platform::get_python_dir()`.
///
/// # Arguments
/// * `app` - The Tauri app handle, used for path resolution
///
/// # Returns
/// * `Ok(version)` - The installed Python version (e.g., "3.12.8")
/// * `Err(message)` - A descriptive error if installation failed
pub async fn install_python(app: &AppHandle) -> Result<String, String> {
    log::info!("Starting Python installation...");

    // Step 1: Determine the download URL for this platform
    let url = get_python_download_url()?;

    // Step 2: Resolve installation paths
    let python_dir = platform::get_python_dir(app);
    let app_data_dir = platform::get_app_data_dir(app);

    // Step 3: Clean up any existing Python installation
    if python_dir.exists() {
        log::info!(
            "Removing existing Python installation at {}",
            python_dir.display()
        );
        std::fs::remove_dir_all(&python_dir).map_err(|e| {
            format!(
                "Failed to remove existing Python directory {}: {}",
                python_dir.display(),
                e
            )
        })?;
    }

    // Ensure the app data directory exists (archive extracts into it)
    std::fs::create_dir_all(&app_data_dir).map_err(|e| {
        format!(
            "Failed to create app data directory {}: {}",
            app_data_dir.display(),
            e
        )
    })?;

    // Step 4: Download and extract the Python archive
    // The archive contains a `python/` directory at the top level, which
    // extracts to `app_data_dir/python/` — exactly our python_dir path.
    log::info!("Downloading and extracting Python to {}", app_data_dir.display());
    archive::download_and_extract(&url, &app_data_dir, archive::ArchiveFormat::TarGz).await?;

    // Step 5: Verify the installation by checking the binary exists
    let python_bin = platform::get_python_binary_path(&python_dir);
    if !python_bin.exists() {
        return Err(format!(
            "Python installation failed: binary not found at {}",
            python_bin.display()
        ));
    }

    // Ensure the binary is executable on Unix
    archive::set_executable(&python_bin)?;

    // Step 6: Get the installed version by running `python --version`
    let version = get_python_version_from_binary(&python_bin).await?;
    log::info!("Python {} installed successfully at {}", version, python_dir.display());

    Ok(version)
}

/// Gets the Python version string by running the binary with --version.
///
/// Executes `python --version` and parses the output "Python X.Y.Z"
/// to extract just the version number "X.Y.Z".
///
/// # Arguments
/// * `python_bin` - Path to the Python binary to run
///
/// # Returns
/// * `Ok(version)` - The version string (e.g., "3.12.8")
/// * `Err(message)` - If the binary couldn't be executed or output couldn't be parsed
async fn get_python_version_from_binary(python_bin: &PathBuf) -> Result<String, String> {
    let output = tokio::process::Command::new(python_bin)
        .arg("--version")
        .output()
        .await
        .map_err(|e| format!("Failed to run Python binary: {}", e))?;

    // Parse "Python 3.12.8" -> "3.12.8"
    let version_str = String::from_utf8_lossy(&output.stdout);
    let version = version_str
        .trim()
        .strip_prefix("Python ")
        .unwrap_or(version_str.trim())
        .to_string();

    if version.is_empty() {
        // Some versions might output to stderr instead
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Python --version returned empty output. stderr: {}",
            stderr.trim()
        ));
    }

    Ok(version)
}

/// Checks whether the portable Python runtime is installed and working.
///
/// Verifies that the Python binary exists at the expected path and
/// can successfully execute `--version`. Returns the version if installed.
///
/// # Arguments
/// * `app` - The Tauri app handle
///
/// # Returns
/// * `Ok(Some(version))` - Python is installed with the given version
/// * `Ok(None)` - Python is not installed
/// * `Err(message)` - An error occurred while checking
pub async fn check_python_status(app: &AppHandle) -> Result<Option<String>, String> {
    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::get_python_binary_path(&python_dir);

    // If the binary doesn't exist, Python is not installed
    if !python_bin.exists() {
        return Ok(None);
    }

    // Try to get the version
    match get_python_version_from_binary(&python_bin).await {
        Ok(version) => Ok(Some(version)),
        Err(e) => {
            log::warn!("Python binary exists but failed to get version: {}", e);
            // Binary exists but isn't working - consider it broken
            Ok(None)
        }
    }
}

/// Returns the expected Python version string for display in the UI.
///
/// This is the version that will be installed (or should be installed).
pub fn expected_python_version() -> &'static str {
    PYTHON_VERSION
}

/// Returns the target Python version constant.
/// Used by the update checker to compare installed vs available.
pub fn get_target_python_version() -> &'static str {
    PYTHON_VERSION
}

/// Returns the installed Python version by running the binary.
/// Returns None if Python is not installed or the binary fails.
///
/// # Arguments
/// * `python_dir` - The directory where Python is installed
pub async fn get_installed_python_version(python_dir: &PathBuf) -> Option<String> {
    let python_bin = crate::utils::platform::get_python_binary_path(python_dir);
    if !python_bin.exists() {
        return None;
    }
    get_python_version_from_binary(&python_bin).await.ok()
}

/// Removes the portable Python installation.
///
/// Used when the user wants to reinstall Python or the installation
/// is detected as corrupt.
///
/// # Arguments
/// * `app` - The Tauri app handle
pub async fn uninstall_python(app: &AppHandle) -> Result<(), String> {
    let python_dir = platform::get_python_dir(app);

    if python_dir.exists() {
        log::info!("Removing Python installation at {}", python_dir.display());
        tokio::fs::remove_dir_all(&python_dir)
            .await
            .map_err(|e| format!("Failed to remove Python directory: {}", e))?;
    }

    Ok(())
}
