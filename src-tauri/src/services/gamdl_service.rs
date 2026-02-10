// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// GAMDL CLI wrapper service.
// Handles installing GAMDL via pip, checking its version, and executing
// GAMDL downloads as subprocesses. The service builds CLI arguments from
// typed GamdlOptions, spawns the GAMDL process, and parses its output
// into structured events that the frontend can display.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::models::gamdl_options::GamdlOptions;
use crate::services::dependency_manager;
use crate::utils::{platform, process};

// ============================================================
// Progress event payload sent to the frontend via Tauri events
// ============================================================

/// Progress event emitted during GAMDL subprocess execution.
/// The frontend listens for "gamdl-output" events to update the download UI.
#[derive(Debug, Clone, Serialize)]
pub struct GamdlProgress {
    /// Unique identifier for the download this event belongs to
    pub download_id: String,
    /// The structured event parsed from GAMDL's output
    pub event: process::GamdlOutputEvent,
}

/// Installs GAMDL into the portable Python environment via pip.
///
/// Runs `python -m pip install gamdl` using the managed Python runtime.
/// The installed GAMDL version is returned on success.
///
/// # Arguments
/// * `app` - The Tauri app handle
///
/// # Returns
/// * `Ok(version)` - The installed GAMDL version (e.g., "2.8.4")
/// * `Err(message)` - If installation failed (Python not found, pip error, etc.)
pub async fn install_gamdl(app: &AppHandle) -> Result<String, String> {
    log::info!("Installing GAMDL via pip...");

    // Resolve the Python binary path
    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::get_python_binary_path(&python_dir);

    // Verify Python is installed before attempting pip install
    if !python_bin.exists() {
        return Err(
            "Cannot install GAMDL: Python is not installed. Run the setup wizard first."
                .to_string(),
        );
    }

    // Run `python -m pip install --upgrade gamdl`
    // --upgrade ensures we get the latest version even if an older one exists
    let output = Command::new(&python_bin)
        .args(["-m", "pip", "install", "--upgrade", "gamdl"])
        .output()
        .await
        .map_err(|e| format!("Failed to run pip install: {}", e))?;

    // Check if pip install succeeded
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pip install gamdl failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("pip install output: {}", stdout.trim());

    // Get the installed version
    let version = get_gamdl_version(app)
        .await?
        .unwrap_or_else(|| "unknown".to_string());

    log::info!("GAMDL {} installed successfully", version);
    Ok(version)
}

/// Checks whether GAMDL is installed and returns its version.
///
/// Runs `python -m pip show gamdl` and parses the "Version: X.Y.Z" line.
///
/// # Arguments
/// * `app` - The Tauri app handle
///
/// # Returns
/// * `Ok(Some(version))` - GAMDL is installed with the given version
/// * `Ok(None)` - GAMDL is not installed (pip show found no package)
/// * `Err(message)` - Python is not available or pip failed
pub async fn get_gamdl_version(app: &AppHandle) -> Result<Option<String>, String> {
    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::get_python_binary_path(&python_dir);

    // If Python isn't installed, GAMDL can't be either
    if !python_bin.exists() {
        return Ok(None);
    }

    // Run `python -m pip show gamdl` to check installation status
    let output = Command::new(&python_bin)
        .args(["-m", "pip", "show", "gamdl"])
        .output()
        .await
        .map_err(|e| format!("Failed to run pip show: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse "Version: X.Y.Z" from pip show output
    let version = stdout
        .lines()
        .find(|line| line.starts_with("Version:"))
        .map(|line| line.trim_start_matches("Version:").trim().to_string());

    Ok(version)
}

/// Executes a GAMDL download as a subprocess and streams parsed events to the frontend.
///
/// This is the core download execution function. It:
/// 1. Builds the full GAMDL CLI command with all options
/// 2. Spawns the process with piped stdout/stderr
/// 3. Reads output line-by-line in real-time
/// 4. Parses each line into a GamdlOutputEvent
/// 5. Emits the event to the frontend via Tauri's event system
///
/// # Arguments
/// * `app` - The Tauri app handle (for path resolution and event emission)
/// * `download_id` - Unique identifier for this download (for event routing)
/// * `urls` - One or more Apple Music URLs to download
/// * `options` - GAMDL CLI options (quality, format, paths, etc.)
///
/// # Returns
/// * `Ok(())` - The download completed (check events for per-track status)
/// * `Err(message)` - The process failed to start or exited with a fatal error
pub async fn run_gamdl(
    app: &AppHandle,
    download_id: &str,
    urls: &[String],
    options: &GamdlOptions,
) -> Result<(), String> {
    log::info!(
        "Starting GAMDL download {} for {} URL(s)",
        download_id,
        urls.len()
    );

    // Build the command with all arguments
    let mut cmd = build_gamdl_command(app, urls, options)?;

    // Configure the process to pipe stdout and stderr for real-time parsing
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Spawn the GAMDL subprocess
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start GAMDL process: {}", e))?;

    // Read stdout and stderr concurrently in separate tasks
    let download_id_clone = download_id.to_string();
    let app_clone = app.clone();

    // Take ownership of the stdout/stderr handles
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture GAMDL stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture GAMDL stderr".to_string())?;

    // Spawn a task to read stdout line-by-line and emit parsed events
    let stdout_task = {
        let download_id = download_id_clone.clone();
        let app = app_clone.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // Parse the output line into a structured event
                let event = process::parse_gamdl_output(&line);
                log::debug!("[gamdl stdout] {}", line);

                // Emit the event to the frontend
                let progress = GamdlProgress {
                    download_id: download_id.clone(),
                    event,
                };
                let _ = app.emit("gamdl-output", &progress);
            }
        })
    };

    // Spawn a task to read stderr line-by-line and emit parsed events
    let stderr_task = {
        let download_id = download_id_clone;
        let app = app_clone;
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let event = process::parse_gamdl_output(&line);
                log::debug!("[gamdl stderr] {}", line);

                let progress = GamdlProgress {
                    download_id: download_id.clone(),
                    event,
                };
                let _ = app.emit("gamdl-output", &progress);
            }
        })
    };

    // Wait for the process to exit and for output reading to complete
    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for GAMDL process: {}", e))?;

    // Wait for output reader tasks to finish
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    // Check the exit status
    if status.success() {
        log::info!("GAMDL download {} completed successfully", download_id);
        Ok(())
    } else {
        let code = status.code().unwrap_or(-1);
        Err(format!("GAMDL process exited with code {}", code))
    }
}

/// Public entry point for build_gamdl_command, used by download_queue.
///
/// Constructs a `tokio::process::Command` that runs:
/// `{python} -m gamdl {urls...} {--option value...}`
///
/// # Arguments
/// * `app` - The Tauri app handle (for path resolution)
/// * `urls` - Apple Music URLs to download
/// * `options` - GAMDL CLI options
pub fn build_gamdl_command_public(
    app: &AppHandle,
    urls: &[String],
    options: &GamdlOptions,
) -> Result<Command, String> {
    build_gamdl_command(app, urls, options)
}

/// Builds the complete GAMDL command with all arguments.
///
/// Constructs a `tokio::process::Command` that runs:
/// `{python} -m gamdl {urls...} {--option value...}`
///
/// Automatically injects tool paths (FFmpeg, mp4decrypt, etc.) if
/// managed versions are installed and no custom path is specified.
///
/// # Arguments
/// * `app` - The Tauri app handle (for path resolution)
/// * `urls` - Apple Music URLs to download
/// * `options` - GAMDL CLI options
fn build_gamdl_command(
    app: &AppHandle,
    urls: &[String],
    options: &GamdlOptions,
) -> Result<Command, String> {
    // Resolve the Python binary path
    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::get_python_binary_path(&python_dir);

    if !python_bin.exists() {
        return Err("Python is not installed. Run the setup wizard first.".to_string());
    }

    // Start building the command: python -m gamdl
    let mut cmd = Command::new(&python_bin);
    cmd.args(["-m", "gamdl"]);

    // Add the URLs as positional arguments
    for url in urls {
        cmd.arg(url);
    }

    // Convert the typed options to CLI argument strings
    let cli_args = options.to_cli_args();
    cmd.args(&cli_args);

    // Inject managed tool paths if available and not overridden by user
    inject_tool_paths(app, &mut cmd, options);

    // Always pass our GAMDL config path to keep it self-contained
    let config_path = platform::get_gamdl_config_path(app);
    if config_path.exists() {
        cmd.arg("--config-path");
        cmd.arg(config_path);
    }

    log::debug!("GAMDL command: python -m gamdl {:?} {:?}", urls, cli_args);

    Ok(cmd)
}

/// Injects paths to managed tool installations into the GAMDL command.
///
/// For each tool (FFmpeg, mp4decrypt, etc.), if the user hasn't specified
/// a custom path in their options AND the managed version is installed,
/// we add the `--{tool}-path` argument pointing to our managed binary.
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `cmd` - The command to add arguments to
/// * `options` - The user's options (checked for custom paths)
fn inject_tool_paths(app: &AppHandle, cmd: &mut Command, options: &GamdlOptions) {
    // FFmpeg: inject if no custom path specified
    if options.ffmpeg_path.is_none() {
        let ffmpeg_bin = dependency_manager::get_tool_binary_path(app, "ffmpeg");
        if ffmpeg_bin.exists() {
            cmd.arg("--ffmpeg-path");
            cmd.arg(&ffmpeg_bin);
        }
    }

    // mp4decrypt: inject if no custom path specified
    if options.mp4decrypt_path.is_none() {
        let mp4decrypt_bin = dependency_manager::get_tool_binary_path(app, "mp4decrypt");
        if mp4decrypt_bin.exists() {
            cmd.arg("--mp4decrypt-path");
            cmd.arg(&mp4decrypt_bin);
        }
    }

    // MP4Box: inject if no custom path specified
    if options.mp4box_path.is_none() {
        let mp4box_bin = dependency_manager::get_tool_binary_path(app, "mp4box");
        if mp4box_bin.exists() {
            cmd.arg("--mp4box-path");
            cmd.arg(&mp4box_bin);
        }
    }

    // N_m3u8DL-RE: inject if no custom path specified
    if options.nm3u8dlre_path.is_none() {
        let nm3u8dlre_bin = dependency_manager::get_tool_binary_path(app, "nm3u8dlre");
        if nm3u8dlre_bin.exists() {
            cmd.arg("--nm3u8dlre-path");
            cmd.arg(&nm3u8dlre_bin);
        }
    }
}

/// Checks the latest GAMDL version available on PyPI.
///
/// Queries the PyPI JSON API to determine the latest published version,
/// which is used for update notifications.
///
/// # Returns
/// * `Ok(version)` - The latest version on PyPI (e.g., "2.8.4")
/// * `Err(message)` - If the PyPI API request failed
pub async fn check_latest_gamdl_version() -> Result<String, String> {
    // Query the PyPI JSON API for the GAMDL package
    let url = "https://pypi.org/pypi/gamdl/json";
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to check PyPI: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("PyPI returned HTTP {}", response.status()));
    }

    // Parse the JSON response to extract the version field
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse PyPI response: {}", e))?;

    json["info"]["version"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Could not find version in PyPI response".to_string())
}
