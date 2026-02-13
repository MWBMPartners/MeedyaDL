// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// GAMDL CLI wrapper service.
// Handles installing GAMDL via pip, checking its version, and executing
// GAMDL downloads as subprocesses. The service builds CLI arguments from
// typed GamdlOptions, spawns the GAMDL process, and parses its output
// into structured events that the frontend can display.
//
// ## Architecture Overview
//
// This service wraps the GAMDL CLI tool (https://github.com/glomatico/gamdl).
// GAMDL is a Python package that downloads Apple Music content. Rather than
// linking to GAMDL as a library, we invoke it as a subprocess via
// `python -m gamdl` using the portable Python managed by `python_manager.rs`.
//
// The service has three main responsibilities:
// 1. **Installation** - `install_gamdl()` runs `pip install --upgrade gamdl`
// 2. **Version detection** - `get_gamdl_version()` parses `pip show gamdl` output
// 3. **Download execution** - `run_gamdl()` spawns the CLI, streams output, and
//    emits parsed events to the frontend via Tauri's event system
//
// ## Event Flow (Download Execution)
//
// ```
// run_gamdl() -> spawn GAMDL process -> pipe stdout/stderr
//     |                                       |
//     v                                       v
// process::parse_gamdl_output()  -->  GamdlProgress event
//     |
//     v
// app.emit("gamdl-output", progress)  -->  Frontend React listener
// ```
//
// The `download_queue.rs` service uses `build_gamdl_command_public()` to
// build commands and manages its own stdout/stderr reading with additional
// queue-level progress tracking.
//
// ## References
//
// - GAMDL CLI usage: https://github.com/glomatico/gamdl
// - Tokio async process spawning: https://docs.rs/tokio/latest/tokio/process/
// - Tokio BufReader for line-by-line async I/O: https://docs.rs/tokio/latest/tokio/io/struct.BufReader.html
// - Tauri event emission: https://v2.tauri.app/develop/calling-rust/#events
// - PyPI JSON API (version check): https://pypi.org/pypi/{package}/json

use serde::Serialize;
// Emitter trait provides the `app.emit()` method for sending events to the frontend.
// Ref: https://v2.tauri.app/develop/calling-rust/#events
use tauri::{AppHandle, Emitter};
// AsyncBufReadExt provides the `.lines()` method for async line-by-line reading of process output.
// BufReader wraps the raw stdout/stderr ChildStdout/ChildStderr for buffered reading.
// Ref: https://docs.rs/tokio/latest/tokio/io/trait.AsyncBufReadExt.html
use tokio::io::{AsyncBufReadExt, BufReader};
// Tokio's Command is the async equivalent of std::process::Command.
// It spawns child processes on the Tokio runtime without blocking the executor.
// Ref: https://docs.rs/tokio/latest/tokio/process/struct.Command.html
use tokio::process::Command;

// GamdlOptions is the typed representation of GAMDL CLI arguments.
// It provides `to_cli_args()` which converts the struct fields into a Vec<String> of CLI flags.
use crate::models::gamdl_options::GamdlOptions;
// dependency_manager provides paths to managed tool binaries (FFmpeg, mp4decrypt, etc.)
use crate::services::dependency_manager;
// `platform` provides cross-platform path resolution; `process` provides GAMDL output parsing.
use crate::utils::{platform, process};

// ============================================================
// Progress event payload sent to the frontend via Tauri events
// ============================================================

/// Progress event emitted during GAMDL subprocess execution.
///
/// The frontend listens for "gamdl-output" events to update the download UI.
/// Each event is tagged with a `download_id` so the React frontend can route
/// the event to the correct download card in the queue UI.
///
/// Serialized to JSON via serde and sent through Tauri's event system.
/// The frontend receives this as: `{ download_id: string, event: GamdlOutputEvent }`.
/// Ref: https://v2.tauri.app/develop/calling-rust/#events
#[derive(Debug, Clone, Serialize)]
pub struct GamdlProgress {
    /// Unique identifier for the download this event belongs to.
    /// Matches the download_id assigned by `download_queue::enqueue()`.
    pub download_id: String,
    /// The structured event parsed from GAMDL's output line.
    /// Variants include DownloadProgress, TrackInfo, ProcessingStep, Complete, Error, etc.
    /// See `process::parse_gamdl_output()` for the parsing logic.
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

    // Run `python -m pip install --upgrade gamdl`.
    // `-m pip` invokes pip as a module of our managed Python, ensuring we use
    // the correct pip instance rather than any system pip.
    // `--upgrade` ensures we get the latest version even if an older one exists,
    // which is important for the update flow.
    // GAMDL's PyPI page: https://pypi.org/project/gamdl/
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

    // Get the installed version by parsing `pip show gamdl` output.
    // We do this after install to confirm the exact version that was installed
    // and return it to the frontend for display.
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

    // Run `python -m pip show gamdl` to check installation status.
    // `pip show` outputs package metadata in a key-value format:
    //   Name: gamdl
    //   Version: 2.8.4
    //   Summary: ...
    // If the package is not installed, pip show exits with code 1 and prints a warning.
    let output = Command::new(&python_bin)
        .args(["-m", "pip", "show", "gamdl"])
        .output()
        .await
        .map_err(|e| format!("Failed to run pip show: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse "Version: X.Y.Z" from pip show output.
    // We search for the line starting with "Version:" and extract the value after the colon.
    // Returns None if pip show produced no output (package not installed).
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

    // Build the command with all arguments (python -m gamdl {urls} {--options}).
    // See build_gamdl_command() below for the full argument construction logic.
    let mut cmd = build_gamdl_command(app, urls, options)?;

    // Configure the process to pipe stdout and stderr for real-time parsing.
    // Piped streams allow us to read output line-by-line as it's produced,
    // rather than waiting for the process to exit.
    // Ref: https://docs.rs/tokio/latest/tokio/process/struct.Command.html#method.stdout
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Spawn the GAMDL subprocess.
    // The child process runs independently; we read its output via the piped handles.
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start GAMDL process: {}", e))?;

    // Clone the download_id and app handle for use in the spawned reader tasks.
    // Each reader task needs its own owned copies since they run independently.
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

    // Spawn a tokio task to read stdout line-by-line and emit parsed events.
    // Each line from GAMDL's output is parsed by process::parse_gamdl_output()
    // into a structured GamdlOutputEvent enum variant (DownloadProgress, TrackInfo, etc.).
    // The parsed event is then emitted to the frontend via Tauri's event system.
    // Ref: https://docs.rs/tokio/latest/tokio/fn.spawn.html
    let stdout_task = {
        let download_id = download_id_clone.clone();
        let app = app_clone.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            // Read lines until EOF (process exit or pipe close)
            while let Ok(Some(line)) = lines.next_line().await {
                // Parse the raw output line into a structured event.
                // The parser uses regex patterns to identify progress bars,
                // track info, errors, and other GAMDL output formats.
                let event = process::parse_gamdl_output(&line);
                log::debug!("[gamdl stdout] {}", line);

                // Emit the parsed event to the frontend as a "gamdl-output" event.
                // The frontend's React useEffect listener receives this and updates
                // the download progress card in the queue UI.
                let progress = GamdlProgress {
                    download_id: download_id.clone(),
                    event,
                };
                let _ = app.emit("gamdl-output", &progress);
            }
        })
    };

    // Spawn a separate task for stderr reading, identical in structure to stdout.
    // GAMDL uses stderr for progress bars (tqdm) and error messages, so we parse
    // it the same way. Both streams are read concurrently to avoid deadlocks
    // that would occur if one pipe's buffer filled while we only read the other.
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

    // Wait for the GAMDL process to exit. This is a non-blocking await
    // that yields the current task until the child process terminates.
    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for GAMDL process: {}", e))?;

    // Wait for the stdout/stderr reader tasks to finish draining all remaining output.
    // This ensures we don't miss any final output lines emitted just before process exit.
    // The `let _ = ...` pattern discards JoinError which would only occur if the task panicked.
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    // Check the exit status: GAMDL returns 0 on success, non-zero on failure.
    // Individual track failures may still result in a 0 exit code if the overall
    // batch had some successes — check per-track events for detailed status.
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
    // Resolve the portable Python binary path from app data directory.
    // This is the same Python installed by python_manager::install_python().
    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::get_python_binary_path(&python_dir);

    if !python_bin.exists() {
        return Err("Python is not installed. Run the setup wizard first.".to_string());
    }

    // Start building the command: `python -m gamdl`
    // The `-m gamdl` flag runs GAMDL as a Python module, equivalent to running
    // the `gamdl` command-line entry point but ensuring we use our managed Python.
    // Ref: https://github.com/glomatico/gamdl#usage
    let mut cmd = Command::new(&python_bin);
    cmd.args(["-m", "gamdl"]);

    // Add the Apple Music URLs as positional arguments.
    // GAMDL accepts one or more URLs (albums, playlists, songs, music videos).
    // Example: python -m gamdl https://music.apple.com/us/album/... https://...
    for url in urls {
        cmd.arg(url);
    }

    // Convert the typed GamdlOptions struct into CLI argument strings.
    // GamdlOptions::to_cli_args() maps each field to its corresponding GAMDL
    // CLI flag (e.g., song_codec: Some(Alac) -> ["--song-codec", "alac"]).
    // See models/gamdl_options.rs for the mapping implementation.
    let cli_args = options.to_cli_args();
    cmd.args(&cli_args);

    // Inject managed tool paths (FFmpeg, mp4decrypt, etc.) if the user hasn't
    // specified custom paths. This auto-detection allows the app to work
    // out-of-the-box with the tools installed by dependency_manager.rs.
    inject_tool_paths(app, &mut cmd, options);

    // Always pass our managed GAMDL config path (config.ini) to keep
    // configuration self-contained within the app data directory.
    // This config.ini is synced from GUI settings by config_service::sync_to_gamdl_config().
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
/// This implements a "managed with override" pattern:
/// - If the user sets a custom tool path in Settings, that path is used (via GamdlOptions)
/// - If no custom path is set, we check if our managed copy exists (installed by dependency_manager)
/// - If neither exists, GAMDL will try to find the tool on the system PATH
///
/// The tool paths correspond to these GAMDL CLI flags:
/// - `--ffmpeg-path` - FFmpeg binary for audio/video processing
/// - `--mp4decrypt-path` - Bento4 mp4decrypt for DRM decryption
/// - `--mp4box-path` - GPAC MP4Box for MP4 muxing
/// - `--nm3u8dlre-path` - N_m3u8DL-RE for HLS/DASH downloading
///
/// # Arguments
/// * `app` - The Tauri app handle (for resolving tool installation paths)
/// * `cmd` - The command to add arguments to (mutated in place)
/// * `options` - The user's options (checked for custom path overrides)
fn inject_tool_paths(app: &AppHandle, cmd: &mut Command, options: &GamdlOptions) {
    // FFmpeg (required by GAMDL): inject managed path if no custom path specified.
    // FFmpeg handles audio codec conversion and video remuxing.
    if options.ffmpeg_path.is_none() {
        let ffmpeg_bin = dependency_manager::get_tool_binary_path(app, "ffmpeg");
        if ffmpeg_bin.exists() {
            cmd.arg("--ffmpeg-path");
            cmd.arg(&ffmpeg_bin);
        }
    }

    // mp4decrypt (optional): Bento4 tool for decrypting Widevine-protected content.
    // Only needed when downloading DRM-protected tracks.
    if options.mp4decrypt_path.is_none() {
        let mp4decrypt_bin = dependency_manager::get_tool_binary_path(app, "mp4decrypt");
        if mp4decrypt_bin.exists() {
            cmd.arg("--mp4decrypt-path");
            cmd.arg(&mp4decrypt_bin);
        }
    }

    // MP4Box (optional): GPAC tool for alternative MP4 muxing.
    // Provides an alternative to FFmpeg for certain container operations.
    if options.mp4box_path.is_none() {
        let mp4box_bin = dependency_manager::get_tool_binary_path(app, "mp4box");
        if mp4box_bin.exists() {
            cmd.arg("--mp4box-path");
            cmd.arg(&mp4box_bin);
        }
    }

    // N_m3u8DL-RE (optional): Alternative HLS/DASH stream downloader.
    // Can be faster than GAMDL's built-in downloader for certain content types.
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
    // Query the PyPI JSON API for the GAMDL package.
    // The PyPI JSON API returns package metadata including the latest version.
    // API format: https://pypi.org/pypi/{package}/json
    // Response structure: { "info": { "version": "2.8.4", ... }, "releases": { ... } }
    // Ref: https://warehouse.pypa.io/api-reference/json.html
    let url = "https://pypi.org/pypi/gamdl/json";
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to check PyPI: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("PyPI returned HTTP {}", response.status()));
    }

    // Parse the JSON response to extract the version field.
    // We use serde_json::Value for dynamic parsing since we only need one field
    // and don't want to define a full struct for the PyPI response schema.
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse PyPI response: {}", e))?;

    // Navigate to info.version in the JSON response.
    // This is the latest stable version published on PyPI.
    json["info"]["version"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Could not find version in PyPI response".to_string())
}
