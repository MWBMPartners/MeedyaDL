// Copyright (c) 2026 MeedyaSuite
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
use tauri::AppHandle;
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
/// Ref: <https://v2.tauri.app/develop/calling-rust/#events>
#[derive(Debug, Clone, Serialize)]
pub struct GamdlProgress {
    /// Unique identifier for the download this event belongs to.
    /// Matches the `download_id` assigned by `download_queue::enqueue()`.
    pub download_id: String,
    /// The structured event parsed from GAMDL's output line.
    /// Variants include `DownloadProgress`, `TrackInfo`, `ProcessingStep`, Complete, Error, etc.
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
/// * `target_version` - When `Some`, pin pip to install exactly this
///   version (e.g. an above-ceiling "Untested" upgrade the user has
///   explicitly opted into). When `None`, use the bounded support-window
///   spec — pip resolves to the newest release inside
///   `[minimum, maximum_tested]`. See [`super::gamdl_capabilities::pip_target_spec`]
///   for the rationale on explicit targeting.
///
/// # Errors
///
/// Returns `Err(String)` if Python is not installed, the pip command fails,
/// or GAMDL version verification fails after installation.
///
/// # Returns
/// * `Ok(version)` - The installed GAMDL version (e.g., "2.8.4")
/// * `Err(message)` - If installation failed (Python not found, pip error, etc.)
pub async fn install_gamdl(
    app: &AppHandle,
    target_version: Option<&str>,
) -> Result<String, String> {
    log::info!("Installing GAMDL via pip...");

    // Resolve the Python binary path
    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

    // Verify Python is installed before attempting pip install
    if !python_bin.exists() {
        return Err(
            "Cannot install GAMDL: Python is not installed. Run the setup wizard first."
                .to_string(),
        );
    }

    // Pip spec selection:
    // * `target_version = None` → bounded support-window spec
    //   (e.g. `gamdl>=2.9.1,<=3.3`). This is the routine path used by
    //   the setup wizard and by `Upgrade` clicks on tested updates.
    //   `--upgrade` + the bounded spec means "pick the newest release
    //   inside the tested range, even if an older one is installed".
    //   Without the ceiling, `--upgrade` silently pulls future majors
    //   that remove CLI flags MeedyaDL depends on — GAMDL v3.0
    //   dropped `--fetch-extra-tags` and caused exactly this class of
    //   fire.
    // * `target_version = Some(v)` → exact pin (`gamdl==v`). Used when
    //   the user has consciously opted into installing an above-ceiling
    //   "Untested" release from the Updates page (amber warning badge).
    //   Pinning bypasses the ceiling so the install actually lands on
    //   the version the banner advertised, instead of silently
    //   resolving down to `maximum_tested`.
    //
    // `-m pip` invokes pip as a module of our managed Python, so we
    // always use the correct pip instance (not any system pip).
    //
    // GAMDL's PyPI page: https://pypi.org/project/gamdl/
    let pip_spec = match target_version {
        Some(v) => super::gamdl_capabilities::pip_target_spec(v),
        None => super::gamdl_capabilities::pip_version_spec(),
    };
    log::info!("Installing GAMDL with version spec: {pip_spec}");
    let output = Command::new(&python_bin)
        // `--only-binary=gamdl` restricts ONLY the `gamdl` package itself
        // (not its dependency tree) to pre-built wheels. GAMDL 3.8.2
        // shipped a compiled Rust extension with a single
        // `cp310-cp310-manylinux_2_34_x86_64` wheel + an sdist — our
        // bundled python-build-standalone runtime is CPython 3.12
        // (`python_manager::PYTHON_VERSION`), so pip would otherwise
        // silently fall back to building the sdist from source, which
        // needs a Rust toolchain the bundled runtime doesn't have. This
        // flag turns that slow, confusing failure into a fast, clean
        // "no matching distribution" pip error. No-op for the current
        // support ceiling (3.8.1 ships a universal `py3-none-any` wheel).
        .args([
            "-m",
            "pip",
            "install",
            "--upgrade",
            "--only-binary=gamdl",
            &pip_spec,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run pip install: {e}"))?;

    // Check if pip install succeeded
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pip install gamdl failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("pip install output: {}", stdout.trim());

    // Get the installed version by parsing `pip show gamdl` output.
    // We do this after install to confirm the exact version that was installed
    // and return it to the frontend for display. `get_gamdl_version` also
    // refreshes the shared capability cache so every consumer (CLI-arg
    // builder, config.ini writer, download queue) sees the new version
    // immediately — critical for upgrades that add or remove CLI flags
    // (e.g. v2.x → v3.0 which dropped `--fetch-extra-tags`).
    let version = get_gamdl_version(app)
        .await?
        .unwrap_or_else(|| "unknown".to_string());

    // Post-install integrity verification: compute SHA-256 of the installed
    // gamdl package location for audit trail. This allows detection of
    // tampered packages after installation (e.g., compromised PyPI mirror).
    if let Ok(location_output) = Command::new(&python_bin)
        .args(["-m", "pip", "show", "gamdl", "--verbose"])
        .output()
        .await
    {
        let show_output = String::from_utf8_lossy(&location_output.stdout);
        // Extract the Location field to log the install path
        if let Some(loc_line) = show_output.lines().find(|l| l.starts_with("Location:")) {
            let location = loc_line.trim_start_matches("Location:").trim();
            log::info!(
                "GAMDL {version} installed at: {location} (verify via: pip show gamdl --verbose)"
            );
        }
    }

    log::info!("GAMDL {version} installed and verified successfully");
    Ok(version)
}

/// Installs a **specific** GAMDL version using `--force-reinstall`.
///
/// Unlike [`install_gamdl`] (which uses `--upgrade`), this function
/// works for **downgrades** as well as upgrades. Pip's resolver only
/// goes higher under `--upgrade`; to land on an explicit version
/// regardless of what's currently installed we have to combine
/// `--force-reinstall` with the exact `gamdl==X.Y.Z` pin.
///
/// Use cases (per #522):
///   - User auto-installed GAMDL v3.0, v3.0 has a regression they ran
///     into, they want to pin v2.9.3 until a fix ships.
///   - Power user on a MeedyaDL pre-release wants to validate v3.0.1
///     manually before the support ceiling moves.
///   - Support engineer asking "downgrade GAMDL to $version and see if
///     the bug reproduces" — needs a CLI-free path for non-technical
///     reporters.
///
/// The target version is **not** validated against the support window
/// here — the caller is expected to surface a warning to the user when
/// they pick an Unsupported / Untested version, and the user is
/// expected to confirm. We only sanity-check that the version string
/// parses as `MAJOR.MINOR.PATCH` (with optional suffix) so a typo
/// can't trigger an unbounded pip resolve.
///
/// After install we re-probe via `get_gamdl_version` which also
/// refreshes the capability cache, so every consumer sees the new
/// version immediately (critical when downgrading from v3.x → v2.x
/// flips feature gates like `FetchExtraTags`).
///
/// # Errors
///
/// * `target` doesn't parse as a recognisable version string.
/// * Python isn't installed.
/// * `pip install --force-reinstall` exits non-zero.
/// * Post-install version probe fails.
pub async fn install_gamdl_version(app: &AppHandle, target: &str) -> Result<String, String> {
    // Cheap sanity check on the version string. We accept anything pip
    // will accept (X.Y, X.Y.Z, X.Y.Z.devN, X.Y.ZrcN, …) — the bar is
    // just "no shell metacharacters, no whitespace, starts with a
    // digit". pip itself is the authoritative validator.
    let trimmed = target.trim();
    if trimmed.is_empty()
        || !trimmed.starts_with(|c: char| c.is_ascii_digit())
        || trimmed.chars().any(|c| {
            c.is_whitespace() || matches!(c, '`' | '$' | ';' | '&' | '|' | '<' | '>' | '"' | '\'')
        })
    {
        return Err(format!(
            "Invalid GAMDL version string '{target}' — expected a PyPI-compatible version like '2.9.3' or '3.5.2'."
        ));
    }

    log::info!("Installing GAMDL version {trimmed} via pip --force-reinstall (per #522)…");

    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

    if !python_bin.exists() {
        return Err(
            "Cannot install GAMDL: Python is not installed. Run the setup wizard first."
                .to_string(),
        );
    }

    let pip_spec = super::gamdl_capabilities::pip_target_spec(trimmed);
    log::info!("Pip spec: {pip_spec} (force-reinstall)");

    let output = Command::new(&python_bin)
        // See the comment on the `install_gamdl` pip invocation above:
        // `--only-binary=gamdl` keeps a future sdist-only GAMDL release
        // (e.g. one that ships a compiled extension without a wheel for
        // our bundled CPython ABI) from triggering a source build that
        // the bundled runtime has no toolchain for.
        .args([
            "-m",
            "pip",
            "install",
            "--force-reinstall",
            "--only-binary=gamdl",
            &pip_spec,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run pip install: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "pip install --force-reinstall {pip_spec} failed: {}",
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("pip install output: {}", stdout.trim());

    // Re-probe and refresh the capability cache.
    let version = get_gamdl_version(app)
        .await?
        .unwrap_or_else(|| "unknown".to_string());

    log::info!("GAMDL {version} installed (force-reinstall) and capability cache refreshed");
    Ok(version)
}

/// Checks whether GAMDL is installed and returns its version.
///
/// Runs `python -m pip show gamdl` and parses the "Version: X.Y.Z" line.
///
/// # Arguments
/// * `app` - The Tauri app handle
///
/// # Errors
///
/// Returns `Err(String)` if the pip show command fails to execute.
///
/// # Returns
/// * `Ok(Some(version))` - GAMDL is installed with the given version
/// * `Ok(None)` - GAMDL is not installed (pip show found no package)
/// * `Err(message)` - Python is not available or pip failed
pub async fn get_gamdl_version(app: &AppHandle) -> Result<Option<String>, String> {
    let python_dir = platform::get_python_dir(app);
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

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
    //
    // A 10-second timeout prevents indefinite blocking when the Python
    // environment is on an unresponsive network mount (e.g., disconnected
    // CloudMounter). This call may be made while holding the queue Mutex,
    // so a stall here would block all queue operations.
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        Command::new(&python_bin)
            .args(["-m", "pip", "show", "gamdl"])
            .output(),
    )
    .await
    .map_err(|_| {
        "pip show gamdl timed out (10s) — Python environment may be unresponsive".to_string()
    })?
    .map_err(|e| format!("Failed to run pip show: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse "Version: X.Y.Z" from pip show output.
    // We search for the line starting with "Version:" and extract the value after the colon.
    // Returns None if pip show produced no output (package not installed).
    let version = stdout
        .lines()
        .find(|line| line.starts_with("Version:"))
        .map(|line| line.trim_start_matches("Version:").trim().to_string());

    // Keep the shared capability cache in sync. Every reader (CLI-arg
    // builder, config.ini writer, download queue) asks this cache
    // whether a given flag / INI key is safe to emit for the currently
    // installed GAMDL release, so stale data here silently breaks
    // downloads after the user upgrades or downgrades GAMDL.
    super::gamdl_capabilities::set_detected_version(version.clone());

    Ok(version)
}

/// Executes a GAMDL download as a subprocess and streams parsed events to the frontend.
///
/// This is the core download execution function. It:
/// 1. Builds the full GAMDL CLI command with all options
/// 2. Spawns the process with piped stdout/stderr
/// 3. Reads output line-by-line in real-time
/// 4. Parses each line into a `GamdlOutputEvent`
/// 5. Emits the event to the frontend via Tauri's event system
///
/// # Arguments
/// * `app` - The Tauri app handle (for path resolution and event emission)
/// * `download_id` - Unique identifier for this download (for event routing)
/// * `urls` - One or more Apple Music URLs to download
/// * `options` - GAMDL CLI options (quality, format, paths, etc.)
///
/// # Errors
///
/// Returns `Err(String)` if the GAMDL process fails to start, exits with
/// a non-zero code, or encounters a fatal error during download.
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
    let cmd = build_gamdl_command(app, urls, options)?;

    // Delegate to the generic engine runner for subprocess lifecycle management
    // (piped stdio, concurrent stdout/stderr streaming, event emission, exit status).
    // The runner emits both "engine-output" (new generic) and "gamdl-output" (legacy)
    // events for backwards compatibility.
    super::engine_runner::run_engine(app, download_id, "gamdl", cmd).await
}

/// Public entry point for `build_gamdl_command`, used by `download_queue`.
///
/// Constructs a `tokio::process::Command` that runs:
/// `{python} -m gamdl {urls...} {--option value...}`
///
/// # Arguments
/// * `app` - The Tauri app handle (for path resolution)
/// * `urls` - Apple Music URLs to download
/// * `options` - GAMDL CLI options
///
/// # Errors
///
/// Returns `Err(String)` if the Python binary path cannot be resolved.
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
/// Automatically injects tool paths (`FFmpeg`, mp4decrypt, etc.) if
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
    let python_bin = platform::resolve_managed_python_binary(&python_dir);

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
    //
    // Validate that each URL starts with "https://" to prevent:
    // 1. Strings starting with "--" being misinterpreted as CLI flags by GAMDL
    // 2. Non-HTTP schemes (file://, ftp://) reaching the subprocess
    // While .arg() is shell-injection-safe, GAMDL itself could misparse inputs.
    for url in urls {
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(format!(
                "Invalid URL (must start with http:// or https://): {url}"
            ));
        }
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

    // Re-sync the managed config.ini immediately before each GAMDL invocation.
    // GAMDL 2.9.3 may overwrite config.ini with its own defaults when run,
    // so we must regenerate it fresh every time to ensure our settings take effect.
    // This also ensures the storefront is always set (required by GAMDL >= 2.9.3).
    if let Ok(settings) = crate::services::config_service::load_settings(app) {
        if let Err(e) = crate::services::config_service::sync_gamdl_config(app, &settings) {
            log::warn!("Failed to sync config.ini before GAMDL invocation: {e}");
        }
    }

    let config_path = platform::get_gamdl_config_path(app);
    if options.no_config_file != Some(true) && config_path.exists() {
        cmd.arg("--config-path");
        cmd.arg(config_path);
    }

    // Redact wrapper account URL from logged CLI args to prevent
    // credential tokens in query parameters from persisting in log files.
    let redacted_args: Vec<String> = {
        let mut result = Vec::with_capacity(cli_args.len());
        let mut skip_next = false;
        for arg in &cli_args {
            if skip_next {
                result.push("[REDACTED]".to_string());
                skip_next = false;
            } else if arg == "--wrapper-account-url" {
                result.push(arg.clone());
                skip_next = true;
            } else {
                result.push(arg.clone());
            }
        }
        result
    };
    log::info!("GAMDL command: python -m gamdl {urls:?} {redacted_args:?}");

    Ok(cmd)
}

/// Injects paths to managed tool installations into the GAMDL command.
///
/// For each tool (`FFmpeg`, mp4decrypt, etc.), if the user hasn't specified
/// a custom path in their options AND the managed version is installed,
/// we add the `--{tool}-path` argument pointing to our managed binary.
///
/// This implements a "managed with override" pattern:
/// - If the user sets a custom tool path in Settings, that path is used (via `GamdlOptions`)
/// - If no custom path is set, we check if our managed copy exists (installed by `dependency_manager`)
/// - If neither exists, GAMDL will try to find the tool on the system PATH
///
/// The tool paths correspond to these GAMDL CLI flags:
/// - `--ffmpeg-path` - `FFmpeg` binary for audio/video processing
/// - `--mp4decrypt-path` - Bento4 mp4decrypt for DRM decryption
/// - `--mp4box-path` - GPAC `MP4Box` for MP4 muxing
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

/// Checks whether a GAMDL version string meets a minimum version requirement.
///
/// Parses both version strings as "X.Y.Z" semver tuples and compares them.
/// Used to gate features that require specific GAMDL versions (e.g.,
/// `song_codec_priority` requires >= 2.9.1).
///
/// Unparseable version parts default to 0, and 2-part versions (e.g., "2.9")
/// are treated as "2.9.0".
///
/// # Examples
///
/// ```
/// use meedyadl::services::gamdl_service::is_version_at_least;
/// assert!(is_version_at_least("2.9.1", "2.9.1")); // exact match
/// assert!(!is_version_at_least("2.8.4", "2.9.1")); // lower
/// assert!(is_version_at_least("3.0.0", "2.9.1")); // higher
/// ```
#[must_use]
pub fn is_version_at_least(version: &str, minimum: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<&str> = v.split('.').collect();
        let major = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };
    parse(version) >= parse(minimum)
}

/// Checks the latest GAMDL version available on `PyPI`.
///
/// Queries the `PyPI` JSON API to determine the latest published version,
/// which is used for update notifications.
///
/// # Errors
///
/// Returns `Err(String)` if the `PyPI` API request fails or the response
/// cannot be parsed.
///
/// # Returns
/// * `Ok(version)` - The latest version on `PyPI` (e.g., "2.8.4")
/// * `Err(message)` - If the `PyPI` API request failed
pub async fn check_latest_gamdl_version() -> Result<String, String> {
    // Query the PyPI JSON API for the GAMDL package.
    // The PyPI JSON API returns package metadata including the latest version.
    // API format: https://pypi.org/pypi/{package}/json
    // Response structure: { "info": { "version": "2.8.4", ... }, "releases": { ... } }
    // Ref: https://warehouse.pypa.io/api-reference/json.html
    //
    // Uses the house `build_simple(10)` client (matching the sibling PyPI
    // lookup in `update_checker.rs::fetch_gamdl_release_wheel_filenames`)
    // instead of a bare `reqwest::get()`, which has NO timeout at all. On a
    // network that silently drops packets rather than refusing the
    // connection, the untimed call could hang for its full OS-level TCP
    // timeout (up to ~2 minutes) — delaying the setup wizard on a machine
    // whose very first screen is meant to get the user unblocked quickly.
    let url = "https://pypi.org/pypi/gamdl/json";
    let client = crate::utils::http_client::build_simple(10)?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to check PyPI: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("PyPI returned HTTP {}", response.status()));
    }

    // Parse the JSON response to extract the version field.
    // We use serde_json::Value for dynamic parsing since we only need one field
    // and don't want to define a full struct for the PyPI response schema.
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse PyPI response: {e}"))?;

    // Navigate to info.version in the JSON response.
    // This is the latest stable version published on PyPI.
    json["info"]["version"]
        .as_str()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| "Could not find version in PyPI response".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_at_least_exact_match() {
        assert!(is_version_at_least("2.9.1", "2.9.1"));
    }

    #[test]
    fn version_at_least_higher() {
        assert!(is_version_at_least("3.0.0", "2.9.1"));
        assert!(is_version_at_least("2.10.0", "2.9.1"));
        assert!(is_version_at_least("2.9.2", "2.9.1"));
    }

    #[test]
    fn version_at_least_lower() {
        assert!(!is_version_at_least("2.8.4", "2.9.1"));
        assert!(!is_version_at_least("2.9.0", "2.9.1"));
        assert!(!is_version_at_least("1.0.0", "2.9.1"));
    }

    #[test]
    fn version_at_least_two_part_versions() {
        // "2.9" is treated as "2.9.0" which is < "2.9.1"
        assert!(!is_version_at_least("2.9", "2.9.1"));
        // "3.0" is treated as "3.0.0" which is > "2.9.1"
        assert!(is_version_at_least("3.0", "2.9.1"));
    }

    #[test]
    fn version_at_least_invalid_strings() {
        // Unparseable parts default to 0
        assert!(!is_version_at_least("", "2.9.1"));
        assert!(!is_version_at_least("abc", "2.9.1"));
        assert!(is_version_at_least("2.9.1", ""));
    }
}
