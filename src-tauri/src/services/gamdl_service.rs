// Copyright (c) 2024-2026 MeedyaSuite
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

/// Decides whether an exact GAMDL version may be installed on this
/// machine, and says why not when the answer is no.
///
/// Every install path that pins an exact version calls this first. The
/// routine, unpinned install does not need it: its version range is
/// already capped at this platform's ceiling. An exact pin deliberately
/// ignores that range, so it needs its own check or the cap is only
/// half a cap.
///
/// Two reasons to refuse, and only two:
///
/// 1. **The release is on the known-bad list.** These are releases we
///    have checked and found broken — see `KNOWN_BAD_VERSIONS` in
///    `gamdl_capabilities`. There is no case for installing one, so
///    this is a flat refusal rather than a warning.
///
/// 2. **This platform is held below the general ceiling and the
///    version is above it.** Only platforms listed in
///    `[gamdl.platform_ceilings]` can fail this. That distinction is
///    the important part: a version above the GENERAL ceiling is
///    merely untested, and installing one on purpose is a supported
///    thing to do (that is what the amber "Untested" badge on the
///    Updates page offers). A version above a HELD-BACK platform's
///    ceiling is a different matter — it cannot be installed there at
///    all, because something it needs is not published for that
///    platform, so pip would try to build it from source and fail with
///    pages of compiler output. Refusing early, with a sentence saying
///    what to install instead, is the whole point.
///
/// `platform_id` is passed in rather than read from the running
/// machine so this can be tested for a platform other than the one the
/// test is running on.
///
/// # Errors
///
/// Returns `Err` with a message meant to be read by a user, not a
/// developer: what was refused, why, and what to install instead.
pub fn refuse_unsupported_target(target: &str, platform_id: &str) -> Result<(), String> {
    use super::gamdl_capabilities::{
        known_bad_advice, known_bad_version, platform_display_name,
    };

    let target = target.trim();

    if let Some(bad) = known_bad_version(target) {
        // `known_bad_advice` is what stops this refusal naming a fix
        // this same function refuses two lines below it — see its own
        // doc comment for the Windows-on-ARM case that motivated it.
        let advice = known_bad_advice(bad.reason, bad.fixed_in, target, platform_id);
        return Err(format!("MeedyaDL will not install GAMDL {target}. {advice}"));
    }

    // Only a platform genuinely held BELOW what everyone else can
    // install can fail this check — see reason 2 in the doc comment
    // above, and `platform_held_back_at` for why "has an entry" is not
    // the same question. A review round found this refusing a release
    // that the Updates screen, asking the better question, had just
    // offered as the user's to choose.
    if let Some(ceiling) = super::gamdl_capabilities::platform_held_back_at(platform_id) {
        let ceiling = ceiling.to_string();
        if !is_version_at_least(&ceiling, target) {
            return Err(format!(
                "MeedyaDL will not install GAMDL {target} on {platform}. The newest version that \
                 can be installed there is {ceiling} — newer releases need something that is not \
                 published for {platform}, so the install would fail part-way through. Install \
                 GAMDL {ceiling} or older instead.",
                platform = platform_display_name(platform_id),
            ));
        }
    }

    Ok(())
}

/// Builds the pip version spec for an explicit-target GAMDL install
/// (`gamdl==<target>`), after refusing any target this platform cannot
/// or must not install.
///
/// Pulled out of [`install_gamdl`] as its own pure, string-only helper
/// specifically so the trimming behaviour here is unit-testable without
/// spawning a real pip subprocess — see the mod tests below.
///
/// **The defect this fixes**: `target` is trimmed exactly once, up
/// front, and the SAME trimmed value is used both for the refusal
/// check and for the pip spec that gets built. Before this split,
/// [`refuse_unsupported_target`] trimmed its own internal copy of the
/// string (so the refusal check itself was always correct), but the
/// original, UNTRIMMED argument was what actually went into
/// [`super::gamdl_capabilities::pip_target_spec`] — so a version string
/// with stray surrounding whitespace (e.g. `" 3.9.1 "`, plausible from
/// a pasted value or a trailing newline) would pass the refusal check
/// on its trimmed form, then build a pip spec containing the literal
/// whitespace, which pip could reject even though nothing was actually
/// wrong with the version itself.
///
/// # Errors
///
/// Returns `Err` when [`refuse_unsupported_target`] turns the target
/// away — a release on the known-bad list, or one above this
/// platform's own ceiling.
fn explicit_target_pip_spec(target: &str, platform_id: &str) -> Result<String, String> {
    let trimmed = target.trim();
    refuse_unsupported_target(trimmed, platform_id)?;
    Ok(super::gamdl_capabilities::pip_target_spec(trimmed))
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
/// or GAMDL version verification fails after installation — or if an
/// explicit `target_version` is turned away by
/// [`refuse_unsupported_target`] (a release known to be broken, or one
/// this machine cannot install).
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
    //
    // An explicit target goes through `refuse_unsupported_target`
    // first. The bounded spec on the `None` branch already keeps
    // routine installs inside this platform's range, but an exact pin
    // walks straight past it — that is the whole point of a pin — so
    // without this check the "install the exact version shown" button
    // on the Updates page could still land a user on a release their
    // machine cannot build, or on one we know to be broken.
    let pip_spec = match target_version {
        Some(v) => explicit_target_pip_spec(v, super::gamdl_capabilities::current_platform_id())?,
        None => super::gamdl_capabilities::pip_version_spec(),
    };
    log::info!("Installing GAMDL with version spec: {pip_spec}");
    let output = Command::new(&python_bin)
        // `--only-binary=gamdl,cryptography` restricts those two
        // packages (not the whole dependency tree) to ready-built
        // packages, refusing to build either from source.
        //
        // `gamdl` is on the list because 3.8.2 started shipping a
        // compiled Rust extension. Where no ready-built package exists
        // for the machine's Python, pip would otherwise quietly fall
        // back to building it, which needs a Rust toolchain the bundled
        // runtime does not have. This turns a slow, confusing failure
        // into a fast, clear "no matching distribution" error.
        //
        // `cryptography` is on the list as a safety net behind the
        // per-platform ceiling, not as a replacement for it. GAMDL 3.9+
        // requires it (indirectly, through `pyplayready`) at a version
        // range that has no Windows ARM64 build. With this flag, that
        // whole branch becomes unsatisfiable on Windows ARM64 and pip
        // steps back to an older GAMDL by itself, instead of trying to
        // compile. That is worth having — but it is silent and slow,
        // and leaves the user on an older version with no explanation,
        // which is exactly why the ceiling check above exists and does
        // the refusing properly.
        //
        // What this cannot do: if a platform has no ready-built
        // `cryptography` at all, the install now stops rather than
        // compiling one. That is the intended trade — a clear failure
        // beats a source build in an environment with no compiler —
        // but it does mean a platform that quietly relied on building
        // it would notice. Checked when this was added: ready-built
        // packages exist for every platform MeedyaDL ships to,
        // including 32-bit ARM Linux.
        .args([
            "-m",
            "pip",
            "install",
            "--upgrade",
            "--only-binary=gamdl,cryptography",
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

    // Log where pip installed GAMDL, for troubleshooting. This is NOT an
    // integrity check: no checksum is computed or compared here, so it
    // cannot detect a tampered package. Installs checked against known file
    // fingerprints are tracked in #397.
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
                "GAMDL {version} installed at: {location} (details: pip show gamdl --verbose)"
            );
        }
    }

    log::info!("GAMDL {version} installed successfully");
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
/// version immediately (critical when crossing a feature-gate
/// boundary, e.g. downgrading below v3.1 flips `WrapperM3u8Ip`).
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

    // This command's contract is "install what the user asked for",
    // and that still holds for anything the user is entitled to judge
    // for themselves — an older release, or a newer untested one. It
    // does not extend to the two cases below, which are not judgement
    // calls: a release we have checked and found broken, and a release
    // that cannot be installed on this machine at all. Letting either
    // through would mean honouring the request by producing a failure.
    refuse_unsupported_target(trimmed, super::gamdl_capabilities::current_platform_id())?;

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
        // Same list, and for the same reasons, as the `install_gamdl`
        // call above: keep pip from building either `gamdl` or
        // `cryptography` from source in a Python environment that has
        // no compiler toolchain for it. Kept identical on purpose — if
        // one call refused source builds and the other allowed them,
        // the same version would install here and fail there.
        .args([
            "-m",
            "pip",
            "install",
            "--force-reinstall",
            "--only-binary=gamdl,cryptography",
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
    use crate::services::gamdl_capabilities::{detected_version, GamdlFeature};

    // Which of these four options the installed GAMDL actually accepts
    // differs by version, and GAMDL treats an option it does not know
    // about as a hard error — it prints "No such option" and stops with
    // exit code 2, before downloading anything. So each one is only
    // passed to a release that defines it.
    //
    // **This gating was missing until 2026-09-22, and that is worth
    // recording.** The same two flags are also emitted from
    // `GamdlOptions::to_cli_args`, where they have been correctly gated
    // since the GAMDL 3.7 work (#869). This second place was missed then,
    // and it sits on the live download path. Whether it ever actually
    // fired is unresolved: it only does so when the user has set no path
    // of their own AND the managed tool file exists, and that could not be
    // checked by reading the code alone. So this is either a fix for a
    // real fault or a guard against one that was waiting to happen —
    // either way the emission now matches what each GAMDL release accepts.
    // These two arguments exist only on releases BEFORE the engine started
    // doing its own muxing. Asking `supports` is not enough here: every
    // capability answers "no" when no version has been detected, and "no"
    // for this one reads as "this is an old release, so send them" — the
    // exact opposite of the caution intended. An independent review caught
    // it: with no version detected, the old condition still sent both
    // arguments, and a modern engine stops before downloading anything when
    // it is given an argument it does not recognise. So the version must be
    // KNOWN, and known to be one that accepts them.
    let tool_paths_accepted = tool_paths_accepted_on(detected_version().as_deref());

    // FFmpeg: accepted before 3.6, dropped in 3.6.x, then brought back in
    // 3.7 because N_m3u8DL-RE needs it.
    //
    // Note the shape of this condition, which is NOT the same as the
    // other two. Every capability answers "no" when no GAMDL version has
    // been detected yet, which is the right caution for an option that
    // only ever helps. This one is different: without it GAMDL looks for
    // FFmpeg on the system path, and MeedyaDL's own copy is deliberately
    // not on the system path — so staying quiet here does not mean
    // "slightly fewer options", it means the download cannot find FFmpeg
    // at all. So the flag is suppressed only when the installed version
    // is KNOWN to be one that rejects it, not merely unknown. Caught in
    // review: the first version of this gate used the plain capability
    // and would have stopped passing the path whenever the version probe
    // had not run.
    let ffmpeg_path_rejected = detected_version()
        .is_some_and(|v| !GamdlFeature::FFmpegPath.is_available_on(&v));
    if options.ffmpeg_path.is_none() && !ffmpeg_path_rejected {
        let ffmpeg_bin = dependency_manager::get_tool_binary_path(app, "ffmpeg");
        if ffmpeg_bin.exists() {
            cmd.arg("--ffmpeg-path");
            cmd.arg(&ffmpeg_bin);
        }
    }

    // mp4decrypt: GAMDL dropped this when it moved to doing the muxing
    // itself in 3.6, and has not brought it back. Nothing is lost by
    // staying quiet — those releases do not use the tool at all.
    if options.mp4decrypt_path.is_none() && tool_paths_accepted {
        let mp4decrypt_bin = dependency_manager::get_tool_binary_path(app, "mp4decrypt");
        if mp4decrypt_bin.exists() {
            cmd.arg("--mp4decrypt-path");
            cmd.arg(&mp4decrypt_bin);
        }
    }

    // MP4Box: dropped in 3.6 alongside mp4decrypt, for the same reason.
    if options.mp4box_path.is_none() && tool_paths_accepted {
        let mp4box_bin = dependency_manager::get_tool_binary_path(app, "mp4box");
        if mp4box_bin.exists() {
            cmd.arg("--mp4box-path");
            cmd.arg(&mp4box_bin);
        }
    }

    // N_m3u8DL-RE (optional): Alternative HLS/DASH stream downloader.
    // Can be faster than GAMDL's built-in downloader for certain content
    // types. Deliberately NOT gated: every GAMDL release in the supported
    // range still defines this option, including 3.9.1 (checked against
    // its own source, not assumed).
    if options.nm3u8dlre_path.is_none() {
        let nm3u8dlre_bin = dependency_manager::get_tool_binary_path(app, "nm3u8dlre");
        if nm3u8dlre_bin.exists() {
            cmd.arg("--nm3u8dlre-path");
            cmd.arg(&nm3u8dlre_bin);
        }
    }
}

/// Does this engine release still accept `--mp4decrypt-path` and
/// `--mp4box-path`?
///
/// `None` means no version has been detected yet, and the answer is then
/// **no**. That is the whole point of this function existing separately:
/// asking the capability directly answers "no" for an unknown version, and
/// for this particular feature "no" means "an old release, so send them" —
/// the opposite of the caution wanted. Getting it wrong costs the entire
/// download, because a release that does not recognise an argument stops
/// before downloading anything.
fn tool_paths_accepted_on(version: Option<&str>) -> bool {
    version.is_some_and(crate::services::gamdl_capabilities::tool_path_flags_accepted_on)
}

#[cfg(test)]
mod refusal_agreement_tests {
    use crate::services::gamdl_capabilities::{
        platform_held_back_at, support_window,
    };

    #[test]
    fn what_the_screen_offers_is_what_the_installer_accepts() {
        // A review round found these two asking different questions. The
        // Updates screen asked whether a platform's entry actually held
        // it back; the install path refused as soon as an entry existed
        // at all. So on a platform whose entry had caught up with the
        // general ceiling, somebody could be told a release was theirs
        // to choose and have it refused a click later. When a screen and
        // the thing it describes disagree, the screen is the one that
        // gets believed.
        //
        // They now ask through one function. This proves the answers
        // line up for every platform, at the boundary that matters.
        let window = support_window();
        for platform_id in [
            "macos",
            "windows-x86_64",
            "windows-aarch64",
            "linux-x86_64",
            "linux-aarch64",
            "linux-armv7",
        ] {
            // A release just above the general ceiling: the ordinary
            // "untested, your choice" case.
            let above_general = format!(
                "{}.999.0",
                window.maximum_tested.split('.').next().unwrap_or("3")
            );

            let held_back = platform_held_back_at(platform_id);
            let refused =
                super::refuse_unsupported_target(&above_general, platform_id).is_err();

            assert_eq!(
                held_back.is_some(),
                refused,
                "{platform_id}: the screen says held back = {held_back:?}, the installer says \
                 refused = {refused} — they must agree"
            );
        }
    }
}

#[cfg(test)]
mod tool_path_gate_tests {
    use super::tool_paths_accepted_on;

    // These call the SAME function the download path calls. An earlier
    // version of this test restated the rule in its own words, which a
    // reviewer rightly objected to: it would have gone on passing while
    // the real rule drifted away from it, which is the one thing a test
    // like this exists to prevent.

    #[test]
    fn an_unknown_engine_version_is_never_sent_the_removed_arguments() {
        // This is the defect an independent review found. Asking the
        // capability directly answers "no" when nothing has been detected,
        // and for this feature "no" reads as "an old release, so send
        // them" — the opposite of the caution intended. A modern engine
        // given an argument it does not know stops before downloading
        // anything, so the cost of guessing wrong here is the whole
        // download.
        //
        // Nothing is written to the shared record of which version was
        // detected, and that is the point: this asks the pure function
        // directly. A whole-branch review found the earlier version
        // clearing that record without holding the lock, which could
        // wipe a version another test had just set and make THAT test
        // fail, somewhere else, for no reason anyone could reproduce.
        // A test that needs no shared state should touch none.
        assert!(
            !tool_paths_accepted_on(None),
            "with no version detected, neither argument may be sent"
        );
    }

    #[test]
    fn only_releases_that_still_accept_them_are_sent_the_removed_arguments() {
        for old in ["3.0", "3.5.2"] {
            assert!(tool_paths_accepted_on(Some(old)), "{old} still accepts them");
        }
        for modern in ["3.6", "3.7.4", "3.9.1"] {
            assert!(
                !tool_paths_accepted_on(Some(modern)),
                "{modern} rejects them outright"
            );
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

    // ----------------------------------------------------------------
    // Refusing an exact version we cannot or must not install
    // ----------------------------------------------------------------

    #[test]
    fn refuses_a_known_bad_release_on_every_platform() {
        // Independent-review fix: this test used to assert `err.contains("3.9.1")`
        // on EVERY platform, including "windows-aarch64" and "linux-armv7" —
        // both held below 3.9.1 by `[gamdl.platform_ceilings]`. It therefore
        // locked in the exact bug the review found: on a held-back platform,
        // the refusal named a fix that platform's own ceiling check, a few
        // lines below in the same function, then turns around and refuses.
        // Passing while proving something self-contradictory is the failure
        // mode this rewrite exists to close — see `known_bad_advice`'s doc
        // comment in `gamdl_capabilities` for the three cases it now covers.
        use crate::services::gamdl_capabilities::support_window;

        let window = support_window();

        // Every platform held below the general ceiling: naming the fix
        // here would send the user straight into a refusal, so the
        // message must not do that.
        for (platform_id, ceiling) in &window.platform_ceilings {
            let err = refuse_unsupported_target("3.9", platform_id)
                .expect_err("GAMDL 3.9 must be refused on every platform");
            // Both platforms in the shipped table sit well below 3.9.1
            // today. Written as a guard rather than assumed, so a future
            // ceiling bump that reaches 3.9.1 fails this test loudly
            // instead of silently making it prove nothing.
            assert!(
                !is_version_at_least(ceiling, "3.9.1"),
                "{platform_id}'s ceiling ({ceiling}) can now install 3.9.1 — this test's \
                 held-back-platform assumption no longer holds, update it"
            );
            assert!(
                !err.contains("or newer"),
                "{platform_id} cannot install past its own ceiling ({ceiling}) — 'or newer' \
                 would re-permit the broken 3.9 release, got: {err}"
            );
            // Naming 3.9.1 is fine — and required — as part of
            // explaining why it's out of reach; what must never happen
            // is framing it as something to install.
            assert!(
                !err.contains("Update to GAMDL 3.9.1") && !err.contains("Install GAMDL 3.9.1"),
                "{platform_id}: 3.9.1 must never be framed as something to install, got: {err}"
            );
            assert!(
                err.contains("3.9.1"),
                "{platform_id}: must still name 3.9.1 to explain why it's unavailable, got: {err}"
            );
        }

        // Platforms with no entry of their own can install the actual
        // fix, and are told so plainly — "or newer" included, since
        // every later release is fine too.
        for platform_id in ["macos", "linux-x86_64", "linux-aarch64", "windows-x86_64"] {
            let err = refuse_unsupported_target("3.9", platform_id)
                .expect_err("GAMDL 3.9 must be refused on every platform");
            assert!(
                err.contains("3.9.1"),
                "{platform_id}: the refusal must name the release to install instead, got: {err}"
            );
            assert!(
                err.contains("or newer"),
                "{platform_id}: a platform that can install the fix may say 'or newer', got: {err}"
            );
        }

        // Written the other way round, it is the same release.
        assert!(refuse_unsupported_target("3.9.0", "macos").is_err());

        // Surrounding whitespace must not smuggle it past the check.
        assert!(refuse_unsupported_target("  3.9  ", "macos").is_err());

        // The release that fixes it is fine.
        assert!(refuse_unsupported_target("3.9.1", "macos").is_ok());
    }

    #[test]
    fn known_bad_refusal_says_move_back_on_a_held_back_platform() {
        // Windows on ARM is held at 3.8.5, BELOW the currently-installed
        // known-bad 3.9 — so the advised version (3.8.5) is a downgrade
        // from what's already there, and the message must say so plainly
        // rather than calling it an "update".
        use crate::services::gamdl_capabilities::support_window;

        let window = support_window();
        if !window.platform_ceilings.contains_key("windows-aarch64") {
            return; // Entry removed — nothing left to prove.
        }

        let err = refuse_unsupported_target("3.9", "windows-aarch64")
            .expect_err("GAMDL 3.9 must be refused on windows-aarch64");
        assert!(
            err.contains("move back"),
            "the advice is a downgrade from what's installed and must say so, got: {err}"
        );
    }

    #[test]
    fn known_bad_refusal_uses_the_same_sentence_as_known_bad_advice() {
        // The whole point of routing this through the shared helper: the
        // refusal message and `known_bad_advice`'s own output must be
        // the SAME words, not merely similar ones, on both an ordinary
        // platform and a held-back one.
        use crate::services::gamdl_capabilities::{known_bad_advice, known_bad_version};

        let bad = known_bad_version("3.9").expect("3.9 must be on the known-bad list");

        for platform_id in ["macos", "windows-aarch64"] {
            let err = refuse_unsupported_target("3.9", platform_id)
                .expect_err("GAMDL 3.9 must be refused");
            let expected_advice = known_bad_advice(bad.reason, bad.fixed_in, "3.9", platform_id);
            assert!(
                err.contains(&expected_advice),
                "{platform_id}: refusal message must embed known_bad_advice's exact sentence.\n\
                 expected to find: {expected_advice}\n\
                 got: {err}"
            );
        }
    }

    #[test]
    fn refuses_a_version_a_held_back_platform_cannot_install() {
        use crate::services::gamdl_capabilities::support_window;

        let window = support_window();
        let Some(ceiling) = window.platform_ceilings.get("windows-aarch64") else {
            return; // Entry removed — nothing left to refuse.
        };

        // The general ceiling is above the Windows-on-ARM one, so it
        // must be refused there with a message that says where to stop.
        let err = refuse_unsupported_target(&window.maximum_tested, "windows-aarch64")
            .expect_err("a version above the Windows-on-ARM ceiling must be refused");
        assert!(
            err.contains("Windows on ARM"),
            "the refusal must name the platform in plain words, got: {err}"
        );
        assert!(
            err.contains(ceiling.as_str()),
            "the refusal must say which version to install instead, got: {err}"
        );

        // Exactly at that platform's ceiling is allowed, as is older.
        assert!(refuse_unsupported_target(ceiling, "windows-aarch64").is_ok());
        assert!(refuse_unsupported_target(&window.minimum, "windows-aarch64").is_ok());

        // The same version is perfectly installable elsewhere.
        assert!(refuse_unsupported_target(&window.maximum_tested, "macos").is_ok());
        assert!(refuse_unsupported_target(&window.maximum_tested, "linux-x86_64").is_ok());
    }

    #[test]
    fn still_allows_a_deliberate_untested_install_where_it_can_work() {
        // Installing a release newer than anything MeedyaDL has tested
        // is a decision the user is allowed to make — it is what the
        // amber "Untested" badge on the Updates page offers. That must
        // keep working on platforms that have no ceiling of their own.
        assert!(refuse_unsupported_target("99.0.0", "macos").is_ok());
        assert!(refuse_unsupported_target("99.0.0", "linux-x86_64").is_ok());

        // But not on a platform held back on purpose: nothing above
        // its ceiling can be installed there at all, so an untested
        // release is not a judgement call, it is a failed install.
        if crate::services::gamdl_capabilities::platform_ceiling_override("windows-aarch64")
            .is_some()
        {
            assert!(refuse_unsupported_target("99.0.0", "windows-aarch64").is_err());
        }
    }

    // ----------------------------------------------------------------
    // Explicit-target pip spec — trim-once, use-everywhere (review fix)
    // ----------------------------------------------------------------

    #[test]
    fn explicit_target_pip_spec_trims_before_building_the_spec() {
        // The actual defect: surrounding whitespace on the version
        // string must not survive into the pip spec. Before the fix,
        // `refuse_unsupported_target` correctly checked the trimmed
        // form (so this particular case never errored), but the pip
        // spec itself was built from the raw, untrimmed argument —
        // so `pip install gamdl==" 3.9.1 "` (or with a trailing
        // newline) is what pip would actually have received.
        let spec = explicit_target_pip_spec("  3.9.1  ", "macos")
            .expect("3.9.1 is a clean, installable release");
        assert_eq!(spec, "gamdl==3.9.1");
    }

    #[test]
    fn explicit_target_pip_spec_trims_before_refusing_a_known_bad_release() {
        // Whitespace must not smuggle a known-bad release past the
        // refusal either — the trim has to happen before BOTH the
        // check and the spec-building use it, not just one.
        let err = explicit_target_pip_spec("  3.9  ", "macos")
            .expect_err("whitespace-padded 3.9 must still be refused");
        assert!(err.contains("3.9.1"), "got: {err}");
    }

    #[test]
    fn explicit_target_pip_spec_refuses_what_refuse_unsupported_target_refuses() {
        // The held-back-platform case, exercised through the same
        // helper the real install path calls — a version above
        // windows-aarch64's own ceiling must never reach a pip spec.
        use crate::services::gamdl_capabilities::support_window;
        let window = support_window();
        if window.platform_ceilings.contains_key("windows-aarch64") {
            assert!(
                explicit_target_pip_spec(&window.maximum_tested, "windows-aarch64").is_err()
            );
        }

        // And an ordinary, installable version still produces a normal
        // exact-pin spec.
        assert_eq!(
            explicit_target_pip_spec("3.0", "macos").unwrap(),
            "gamdl==3.0"
        );
    }
}
