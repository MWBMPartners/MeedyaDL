// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Generic pip-based engine management service.
// =============================================
//
// Provides install and version-check functions for any Python package
// that MeedyaDL manages as a download engine (e.g., GAMDL, votify,
// OF-Scraper). Uses the managed Python runtime from python_manager.rs.
//
// This module generalises the pattern from gamdl_service.rs so new
// pip-based engines can be added with zero new Rust service code —
// just call `install_pip_engine("votify")` or `get_pip_engine_version("ofscraper")`.
//
// The engines.toml registry defines which packages are available,
// their required/optional status, and whether they're enabled.

use tauri::AppHandle;
use tokio::process::Command;

use crate::utils::platform;

/// Installs a pip package into the managed Python environment.
///
/// Runs `python -m pip install --upgrade <package>` and returns
/// the installed version on success.
///
/// # Arguments
/// * `app` - Tauri app handle for locating the Python binary
/// * `package` - PyPI package name (e.g., "votify", "ofscraper", "yt-dlp")
///
/// # Returns
/// * `Ok(version)` - The installed version string
/// * `Err(message)` - If Python is missing or pip install failed
pub async fn install_pip_engine(app: &AppHandle, package: &str) -> Result<String, String> {
    log::info!("Installing {package} via pip...");

    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::get_python_binary_path(&python_dir);

    if !python_bin.exists() {
        return Err(format!(
            "Cannot install {package}: Python is not installed. Run the setup wizard first."
        ));
    }

    let output = Command::new(&python_bin)
        .args(["-m", "pip", "install", "--upgrade", package])
        .output()
        .await
        .map_err(|e| format!("Failed to run pip install {package}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pip install {package} failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("pip install {package} output: {}", stdout.trim());

    let version = get_pip_engine_version(app, package)
        .await?
        .unwrap_or_else(|| "unknown".to_string());

    log::info!("{package} v{version} installed successfully");
    Ok(version)
}

/// Checks whether a pip package is installed and returns its version.
///
/// Runs `python -m pip show <package>` and parses the "Version:" line.
///
/// # Arguments
/// * `app` - Tauri app handle for locating the Python binary
/// * `package` - PyPI package name
///
/// # Returns
/// * `Ok(Some(version))` - Package is installed with the given version
/// * `Ok(None)` - Package is not installed
/// * `Err(message)` - Python is not available or pip failed
pub async fn get_pip_engine_version(
    app: &AppHandle,
    package: &str,
) -> Result<Option<String>, String> {
    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::get_python_binary_path(&python_dir);

    if !python_bin.exists() {
        return Ok(None);
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        Command::new(&python_bin)
            .args(["-m", "pip", "show", package])
            .output(),
    )
    .await
    .map_err(|_| {
        format!("pip show {package} timed out (10s) — Python environment may be unresponsive")
    })?
    .map_err(|e| format!("Failed to run pip show {package}: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse "Version: X.Y.Z" from pip show output
    let version = stdout
        .lines()
        .find(|line| line.starts_with("Version:"))
        .map(|line| line.trim_start_matches("Version:").trim().to_string());

    Ok(version)
}

/// Uninstalls a pip package from the managed Python environment.
///
/// Runs `python -m pip uninstall -y <package>`.
///
/// # Arguments
/// * `app` - Tauri app handle for locating the Python binary
/// * `package` - PyPI package name
pub async fn uninstall_pip_engine(app: &AppHandle, package: &str) -> Result<(), String> {
    log::info!("Uninstalling {package} via pip...");

    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::get_python_binary_path(&python_dir);

    if !python_bin.exists() {
        return Err("Python is not installed".to_string());
    }

    let output = Command::new(&python_bin)
        .args(["-m", "pip", "uninstall", "-y", package])
        .output()
        .await
        .map_err(|e| format!("Failed to run pip uninstall {package}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "pip uninstall {package} failed: {}",
            stderr.trim()
        ));
    }

    log::info!("{package} uninstalled successfully");
    Ok(())
}
