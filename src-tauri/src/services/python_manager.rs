// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Python runtime manager service.
// Downloads, installs, and manages a portable Python runtime from the
// python-build-standalone project (https://github.com/indygreg/python-build-standalone).
// The portable Python is installed into the app data directory and is
// completely self-contained, avoiding conflicts with any system Python.
//
// ## Architecture Overview
//
// This service is one of the first things invoked during the setup wizard flow.
// The setup wizard (frontend) calls the `install_python` Tauri command, which
// delegates here. Once installed, the Python binary is used by `gamdl_service.rs`
// to run `python -m gamdl` and by `gamdl_service::install_gamdl` to run
// `python -m pip install gamdl`.
//
// ## Flow Summary
//
// 1. `install_python()` - Main entry: downloads tar.gz, extracts, verifies binary
// 2. `check_python_status()` - Quick health check: binary exists + runs --version
// 3. `get_installed_python_version()` - Lightweight version query for update checks
// 4. `uninstall_python()` - Clean removal for reinstall scenarios
//
// ## References
//
// - python-build-standalone releases: https://github.com/indygreg/python-build-standalone/releases
// - Reqwest HTTP client (used by archive utilities): https://docs.rs/reqwest/latest/reqwest/
// - Tokio async runtime and process spawning: https://docs.rs/tokio/latest/tokio/
// - Tauri app handle and path resolution: https://v2.tauri.app/develop/calling-rust/

use std::path::PathBuf;
use tauri::AppHandle;

// `archive` provides download_and_extract() and set_executable() helpers.
// `platform` provides cross-platform path resolution for Python directories and binaries.
use crate::utils::{archive, platform};

// ============================================================
// Python version constants
// Update these when a new Python release is available.
// The release tag and version must match an actual release on GitHub.
// ============================================================

/// The Python version to install (used in the download URL and version checks).
/// This must correspond to a CPython version available in the python-build-standalone
/// release specified by RELEASE_TAG. Check the release assets at:
/// https://github.com/indygreg/python-build-standalone/releases/tag/{RELEASE_TAG}
const PYTHON_VERSION: &str = "3.12.8";

/// The python-build-standalone release tag on GitHub (date-based, format: YYYYMMDD).
/// Each release bundles multiple Python versions as pre-compiled archives.
/// Browse available releases: https://github.com/indygreg/python-build-standalone/releases
const RELEASE_TAG: &str = "20250106";

/// The base URL for python-build-standalone release downloads on GitHub.
/// The full URL is constructed as: {base}/{tag}/cpython-{version}+{tag}-{triple}-install_only.tar.gz
/// See: https://github.com/indygreg/python-build-standalone#available-distributions
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
    // Determine the platform triple for python-build-standalone.
    // These triples match the naming convention used in python-build-standalone releases.
    // The "install_only" variant is a minimal distribution containing just the Python
    // interpreter, standard library, and pip — ideal for running GAMDL as a pip package.
    // Ref: https://gregoryszorc.com/docs/python-build-standalone/main/running.html
    let triple = match (std::env::consts::OS, std::env::consts::ARCH) {
        // macOS: Apple Silicon (M1/M2/M3) and Intel
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        // Linux: x86_64 and ARM64 with glibc (musl not supported)
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        // Windows: x86_64 and ARM64 built with MSVC toolchain
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        (os, arch) => {
            return Err(format!(
                "Unsupported platform for portable Python: {}/{}",
                os, arch
            ))
        }
    };

    // Build the full download URL following python-build-standalone naming convention:
    // Example: https://github.com/indygreg/python-build-standalone/releases/download/
    //          20250106/cpython-3.12.8+20250106-aarch64-apple-darwin-install_only.tar.gz
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

    // Step 4: Download and extract the Python archive.
    // The archive contains a `python/` directory at the top level, which
    // extracts to `app_data_dir/python/` — exactly our python_dir path.
    // The download_and_extract utility uses reqwest to stream the download
    // and flate2/tar to extract the tar.gz archive.
    // Ref: https://docs.rs/reqwest/latest/reqwest/ (HTTP streaming download)
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

    // Ensure the binary is executable on Unix (chmod +x).
    // On Windows this is a no-op since executability is determined by file extension.
    archive::set_executable(&python_bin)?;

    // Step 6: Get the installed version by running `python --version`.
    // This validates that the binary is not just present but actually functional.
    // Uses tokio::process::Command for async subprocess execution.
    // Ref: https://docs.rs/tokio/latest/tokio/process/struct.Command.html
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
    // Spawn the Python process asynchronously using tokio::process::Command.
    // The .output() method captures both stdout and stderr and waits for completion.
    // Ref: https://docs.rs/tokio/latest/tokio/process/struct.Command.html#method.output
    let output = tokio::process::Command::new(python_bin)
        .arg("--version")
        .output()
        .await
        .map_err(|e| format!("Failed to run Python binary: {}", e))?;

    // Parse "Python 3.12.8" -> "3.12.8"
    // CPython always outputs "Python X.Y.Z" to stdout. We strip that prefix
    // to get the bare version string for display and comparison purposes.
    let version_str = String::from_utf8_lossy(&output.stdout);
    let version = version_str
        .trim()
        .strip_prefix("Python ")
        .unwrap_or(version_str.trim())
        .to_string();

    if version.is_empty() {
        // Some Python builds or error conditions output to stderr instead of stdout.
        // Report the stderr content to aid in debugging installation issues.
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

    // Try to get the version by actually running the binary.
    // This catches cases where the binary file exists but is corrupt,
    // has missing shared libraries, or is otherwise non-functional.
    match get_python_version_from_binary(&python_bin).await {
        Ok(version) => Ok(Some(version)),
        Err(e) => {
            log::warn!("Python binary exists but failed to get version: {}", e);
            // Binary exists but isn't working — treat as not installed so that
            // the setup wizard will offer to reinstall.
            Ok(None)
        }
    }
}

/// Returns the expected Python version string for display in the UI.
///
/// This is the version that will be installed (or should be installed).
/// Called by the frontend's setup wizard to show "Python X.Y.Z will be installed".
pub fn expected_python_version() -> &'static str {
    PYTHON_VERSION
}

/// Returns the target Python version constant.
/// Used by `update_checker.rs` to compare the installed version against the
/// version we expect, enabling the update UI to prompt for a Python update
/// when a newer python-build-standalone release is configured.
pub fn get_target_python_version() -> &'static str {
    PYTHON_VERSION
}

/// Returns the installed Python version by running the binary.
/// Returns None if Python is not installed or the binary fails.
///
/// This is a lightweight query used by `update_checker.rs` to determine
/// the currently installed version without needing the full AppHandle.
///
/// # Arguments
/// * `python_dir` - The directory where Python is installed (e.g., {app_data}/python/)
pub async fn get_installed_python_version(python_dir: &std::path::Path) -> Option<String> {
    // Resolve the binary path (e.g., python/bin/python3 on Unix, python/python.exe on Windows)
    let python_bin = crate::utils::platform::get_python_binary_path(python_dir);
    if !python_bin.exists() {
        return None;
    }
    // Silently return None on any error (binary corrupt, permissions, etc.)
    get_python_version_from_binary(&python_bin).await.ok()
}

/// Removes the portable Python installation entirely.
///
/// Used when the user wants to reinstall Python or the installation
/// is detected as corrupt. This also effectively uninstalls GAMDL and
/// all pip packages, since they live inside the portable Python directory.
///
/// Uses `tokio::fs::remove_dir_all` for async directory removal, which
/// avoids blocking the Tokio runtime on potentially slow filesystem operations.
/// Ref: https://docs.rs/tokio/latest/tokio/fs/fn.remove_dir_all.html
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
