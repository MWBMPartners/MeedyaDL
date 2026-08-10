// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Dependency management IPC commands.
// Handles checking installation status and installing Python, GAMDL,
// and external tool dependencies (FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box).
// These commands are primarily used by the first-run setup wizard
// and the dependency status indicators throughout the UI.
//
// Delegates to service modules for the actual installation logic.
//
// ## Architecture
//
// This module is the IPC bridge for dependency management. The application
// manages a self-contained portable runtime consisting of:
//   1. **Python** - A standalone Python runtime (python-build-standalone)
//   2. **GAMDL** - The core Apple Music downloader, installed via pip
//   3. **External tools** - FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box
//
// Each dependency has "check" and "install" commands. The frontend setup
// wizard calls the check commands first, then offers install buttons for
// any missing dependencies.
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command             | TypeScript Function         | Line |
// |--------------------------|-----------------------------|------|
// | check_python_status      | checkPythonStatus()         | ~41  |
// | install_python           | installPython()             | ~46  |
// | check_gamdl_status       | checkGamdlStatus()          | ~51  |
// | install_gamdl            | installGamdl()              | ~56  |
// | check_all_dependencies   | checkAllDependencies()      | ~61  |
// | install_dependency       | installDependency(name)     | ~66  |
//
// ## References
//
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - python-build-standalone: https://github.com/indygreg/python-build-standalone

// serde::Serialize is required for structs returned to the frontend via IPC.
// Tauri serializes all return values to JSON before crossing the IPC bridge.
use serde::Serialize;
// AppHandle is injected automatically by Tauri into any command that declares it.
// Provides access to app data directories, managed state, and the event system.
use tauri::AppHandle;

// dependency_manager: handles downloading and installing external tools
//   (FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box) from platform-specific URLs.
// gamdl_service: manages the GAMDL Python package (install, version check, update).
// python_manager: manages the portable Python runtime (download, install, verify).
use crate::services::{dependency_manager, gamdl_service, package_manager, python_manager};
use crate::utils::activity_log::emit_app_log;
use std::path::PathBuf;
use std::sync::LazyLock;

/// Matches the first `MAJOR.MINOR.PATCH` triple in a version-command's output
/// (e.g. `gamdl, version 3.9.0` → `3.9.0`).
static GAMDL_VERSION_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\d+\.\d+\.\d+").expect("valid semver regex"));

/// Status information for a single dependency (Python, GAMDL, or tool).
///
/// Returned to the frontend for display in the setup wizard and status bar.
/// The frontend maps this to the `DependencyStatus` TypeScript interface
/// defined in `src/types/index.ts`.
///
/// Implements `Serialize` for Tauri IPC serialization to JSON.
/// See: <https://v2.tauri.app/develop/calling-rust/#return-types>
#[derive(Debug, Clone, Serialize)]
pub struct DependencyStatus {
    /// Human-readable name of the dependency (e.g., "Python 3.12", "`FFmpeg`").
    /// Displayed as the label in the setup wizard dependency list.
    pub name: String,
    /// Whether this dependency is required for basic functionality.
    /// Required dependencies block downloads; optional ones just limit features.
    /// For example, `FFmpeg` is required but `MP4Box` is optional.
    pub required: bool,
    /// Whether the dependency is currently installed and accessible.
    /// Determined by checking for the binary at the expected path.
    pub installed: bool,
    /// Installed version string, if available (e.g., "3.12.8", "2.8.4").
    /// `None` when version detection is skipped (batch checks) or not installed.
    pub version: Option<String>,
    /// Absolute path to the installed binary/executable.
    /// `None` when not installed or when the dependency is a Python package
    /// (GAMDL) rather than a standalone binary.
    pub path: Option<String>,
    /// Where the dependency was installed from.
    /// `Some("system")` if detected from the system PATH,
    /// `Some("managed")` if downloaded by the app,
    /// `None` for Python/GAMDL (always managed) or if not installed.
    pub source: Option<String>,
}

/// Checks whether the portable Python runtime is installed in the app data directory.
///
/// **Frontend caller:** `checkPythonStatus()` in `src/lib/tauri-commands.ts`
///
/// Returns status information including whether Python exists, its version,
/// and its binary path. Used by the setup wizard to determine if the
/// Python installation step can be skipped.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for resolving the app data directory path.
///
/// # Errors
///
/// Returns `Err(String)` if the Python status check fails (e.g., cannot
/// read the app data directory or the Python binary path cannot be resolved).
///
/// # Returns
/// * `Ok(DependencyStatus)` - Python status with version and path info.
/// * `Err(String)` - Error message if the status check itself failed
///   (e.g., cannot read the app data directory).
///
/// # Implementation Notes
/// The version is detected by running the Python binary with `--version`
/// inside `python_manager::check_python_status()`. If the binary doesn't
/// exist or fails to run, `version` will be `None` and `installed` will be `false`.
#[tauri::command]
pub async fn check_python_status(app: AppHandle) -> Result<DependencyStatus, String> {
    // Check if the Python binary exists and get its version string
    let version = python_manager::check_python_status(&app).await?;

    // Resolve the expected Python directory and binary paths for this platform.
    // These are deterministic paths based on the app data directory:
    //   macOS/Linux: {app_data}/python/bin/python3
    //   Windows:     {app_data}/python/python.exe
    let python_dir = crate::utils::platform::get_python_dir(&app);
    // Runtime-resolve so a system-Python venv (#1017) reports the venv binary
    // path (matters on Windows: venv → Scripts/python.exe, portable → root).
    let python_bin = crate::utils::platform::resolve_managed_python_binary(&python_dir);

    // A venv built from a system Python earns the "System" badge; the managed
    // portable runtime shows no badge (source: None).
    let source = python_manager::get_python_source(&app)
        .filter(python_manager::PythonSourceRecord::is_system_venv)
        .map(|_| "system".to_string());

    Ok(DependencyStatus {
        // Display the expected version in the name (e.g., "Python 3.12")
        name: format!("Python {}", python_manager::expected_python_version()),
        required: true, // Python is always required — GAMDL runs on Python
        installed: version.is_some(), // None means not installed
        version,
        // Convert PathBuf to String for JSON serialization
        path: python_bin.to_str().map(std::string::ToString::to_string),
        source,
    })
}

/// Downloads and installs a portable Python runtime.
///
/// **Frontend caller:** `installPython()` in `src/lib/tauri-commands.ts`
///
/// Downloads a platform-appropriate Python build from python-build-standalone
/// GitHub releases (<https://github.com/indygreg/python-build-standalone>),
/// extracts it to `{app_data}/python/`, and verifies the installation by
/// running `python --version`.
///
/// This is a long-running operation (downloads ~30-80MB depending on platform).
/// The frontend should show a loading indicator while awaiting the result.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for resolving download and extraction paths.
///
/// # Errors
///
/// Returns `Err(String)` if the Python download, extraction, or verification fails.
///
/// # Returns
/// * `Ok(String)` - The installed Python version string (e.g., "3.12.8").
/// * `Err(String)` - Download, extraction, or verification failure message.
#[tauri::command]
pub async fn install_python(app: AppHandle) -> Result<String, String> {
    // Delegates entirely to python_manager which handles:
    //   1. Determining the correct download URL for the current OS/arch
    //   2. Downloading the tarball/zip archive
    //   3. Extracting to the app data directory
    //   4. Verifying the installation by running python --version
    log::info!("Installing Python...");
    emit_app_log(&app, "Installing Python...");
    let version = python_manager::install_python(&app).await?;
    log::info!("Python {version} installed");
    emit_app_log(&app, &format!("Python {version} installed"));
    Ok(version)
}

/// Detects compatible Python interpreters already installed on the system (#1017).
///
/// **Frontend caller:** `detectSystemPythons()` in `src/lib/tauri-commands.ts`
///
/// Returns candidates newest-first (floor-meeting ahead of too-old) so the
/// setup wizard can offer "use your existing Python" instead of forcing the
/// portable download. Never errors — an empty list simply means nothing usable
/// was found and the UI falls back to the portable-download flow.
///
/// # Errors
/// Infallible in practice; the `Result` wrapper keeps the IPC signature uniform.
#[tauri::command]
pub async fn detect_system_pythons() -> Result<Vec<python_manager::SystemPython>, String> {
    Ok(python_manager::detect_system_pythons().await)
}

/// Provisions the managed Python by building a `venv` from a chosen system
/// interpreter (#1017) instead of downloading the portable runtime.
///
/// **Frontend caller:** `useSystemPython(interpreter)` in `src/lib/tauri-commands.ts`
///
/// Returns a [`DependencyStatus`] mirroring [`check_python_status`] so the
/// wizard can reuse the same "installed" rendering.
///
/// # Errors
/// Returns a user-facing error when the interpreter is unrunnable, below the
/// GAMDL floor (3.10), or missing the `venv`/`ensurepip` module — in which case
/// the frontend should fall back to the portable-download flow.
#[tauri::command]
pub async fn use_system_python(
    app: AppHandle,
    interpreter: String,
) -> Result<DependencyStatus, String> {
    log::info!("Provisioning managed Python from system interpreter: {interpreter}");
    emit_app_log(
        &app,
        &format!("Provisioning Python from your system interpreter: {interpreter}"),
    );
    let version = python_manager::provision_venv_from_system_python(&app, &interpreter).await?;
    emit_app_log(&app, &format!("Python {version} ready (from your system Python)"));

    let python_dir = crate::utils::platform::get_python_dir(&app);
    let python_bin = crate::utils::platform::resolve_managed_python_binary(&python_dir);
    Ok(DependencyStatus {
        name: format!("Python {version}"),
        required: true,
        installed: true,
        version: Some(version),
        path: python_bin.to_str().map(std::string::ToString::to_string),
        source: Some("system".to_string()),
    })
}

/// Checks whether GAMDL is installed in the portable Python environment.
///
/// **Frontend caller:** `checkGamdlStatus()` in `src/lib/tauri-commands.ts`
///
/// Runs `python -m pip show gamdl` using the managed Python runtime to
/// detect the package and extract the version number from pip's output.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for locating the Python binary.
///
/// # Errors
///
/// Returns `Err(String)` if the pip check command fails to execute.
///
/// # Returns
/// * `Ok(DependencyStatus)` - GAMDL status with version info.
///   `path` is always `None` because GAMDL is a Python package invoked
///   via `python -m gamdl`, not a standalone binary.
/// * `Err(String)` - Error if the pip check command itself fails to execute.
#[tauri::command]
pub async fn check_gamdl_status(app: AppHandle) -> Result<DependencyStatus, String> {
    // get_gamdl_version() runs pip and parses the "Version:" line from output.
    // Returns Some("x.y.z") if installed, None if not found.
    let version = gamdl_service::get_gamdl_version(&app).await?;

    Ok(DependencyStatus {
        name: "GAMDL".to_string(),
        required: true, // GAMDL is the core downloader — nothing works without it
        installed: version.is_some(),
        version,
        path: None,   // GAMDL is a Python package, not a standalone binary
        source: None, // GAMDL is always managed (pip package)
    })
}

/// Installs GAMDL via pip into the portable Python environment.
///
/// **Frontend caller:** `installGamdl()` in `src/lib/tauri-commands.ts`
///
/// Runs `pip install --upgrade gamdl` using the managed Python runtime.
/// Python must already be installed before calling this command — the
/// frontend setup wizard enforces this ordering.
///
/// The `--upgrade` flag ensures this command also works as an updater:
/// if GAMDL is already installed, it will be upgraded to the latest version.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for locating the Python/pip binaries.
///
/// # Errors
///
/// Returns `Err(String)` if pip installation of GAMDL fails.
///
/// # Returns
/// * `Ok(String)` - The installed GAMDL version string (e.g., "2.8.4").
/// * `Err(String)` - pip installation failure message.
#[tauri::command]
pub async fn install_gamdl(app: AppHandle) -> Result<String, String> {
    // Delegates to gamdl_service which runs pip and parses the output
    log::info!("Installing GAMDL...");
    emit_app_log(&app, "Installing GAMDL...");
    // Routine setup-wizard install: no explicit version target, use the
    // bounded `[minimum, maximum_tested]` spec.
    let version = gamdl_service::install_gamdl(&app, None).await?;
    log::info!("GAMDL v{version} installed");
    emit_app_log(&app, &format!("GAMDL v{version} installed"));
    Ok(version)
}

/// Installs a **specific** GAMDL version (supports downgrades) — #522.
///
/// **Frontend caller:** `installGamdlVersion(version)` in
/// `src/lib/tauri-commands.ts`, wired to the Settings > Tools "Install
/// recommended" and "Install specific version" controls.
///
/// Uses `pip install --force-reinstall gamdl==<version>` under the hood
/// so it works for both upgrades and downgrades. The bounded
/// `pip_version_spec` used by [`install_gamdl`] can't downgrade because
/// pip's `--upgrade` resolver only goes higher.
///
/// Logs a WARN to the activity log when the requested version falls
/// outside the support window, so the user has visible feedback that
/// they're opting into Unsupported / Untested behaviour. The install
/// still proceeds — this command's contract is "do what the user
/// asked"; the warning is a courtesy, not a gate.
///
/// # Arguments
/// * `version` — A PyPI-compatible version string (e.g., "2.9.3",
///   "3.5.2", "3.6.0a1"). Validated by `gamdl_service` before pip runs.
///
/// # Returns
/// * `Ok(String)` — The post-install version reported by `pip show gamdl`.
/// * `Err(String)` — Validation failure, pip failure, or version-probe
///   failure.
#[tauri::command]
pub async fn install_gamdl_version(app: AppHandle, version: String) -> Result<String, String> {
    log::info!("Installing specific GAMDL version: {version}");
    emit_app_log(&app, &format!("Installing GAMDL v{version} (force-reinstall)..."));

    // Classify against the support window so we can surface an
    // advisory warning to the user before the install lands. We don't
    // refuse — the user might be downgrading because of an upstream
    // regression we don't know about, and gating that would be
    // user-hostile.
    let classification = crate::services::gamdl_capabilities::classify(Some(&version));
    match &classification {
        crate::services::gamdl_capabilities::VersionSupport::Unsupported { .. } => {
            log::warn!(
                "User-requested GAMDL v{version} is OUTSIDE the supported version window — \
                 installing anyway per #522 user opt-in"
            );
            emit_app_log(
                &app,
                &format!(
                    "WARNING: GAMDL v{version} is outside MeedyaDL's tested version window — features may misbehave. \
                     Use 'Install recommended' to return to the validated version."
                ),
            );
        }
        crate::services::gamdl_capabilities::VersionSupport::Untested { .. } => {
            log::warn!(
                "User-requested GAMDL v{version} is ABOVE the tested ceiling — installing \
                 anyway per #522 user opt-in"
            );
            emit_app_log(
                &app,
                &format!(
                    "Notice: GAMDL v{version} is newer than the MeedyaDL-tested ceiling. \
                     Installing on user request; please report any regressions."
                ),
            );
        }
        _ => {
            // Supported or NotInstalled — no advisory needed.
        }
    }

    let installed = gamdl_service::install_gamdl_version(&app, &version).await?;
    log::info!("GAMDL v{installed} installed (force-reinstall, target was v{version})");
    emit_app_log(&app, &format!("GAMDL v{installed} installed"));
    Ok(installed)
}

/// Returns the GAMDL support window (minimum / maximum tested /
/// recommended) so the frontend can render the version-management UI
/// without hard-coding the values.
///
/// **Frontend caller:** `getGamdlSupportWindow()` in
/// `src/lib/tauri-commands.ts`.
///
/// Reads from the compiled-in `tool-versions.toml` (`include_str!`).
/// Zero I/O, always succeeds.
#[tauri::command]
pub fn get_gamdl_support_window() -> GamdlSupportWindowResponse {
    let window = crate::services::gamdl_capabilities::support_window();
    GamdlSupportWindowResponse {
        minimum: window.minimum.clone(),
        maximum_tested: window.maximum_tested.clone(),
        recommended: window.recommended.clone(),
    }
}

/// DTO mirrored by `GamdlSupportWindow` in TypeScript. Defined here so
/// the frontend doesn't have to depend on the private internals of
/// `gamdl_capabilities`.
#[derive(serde::Serialize)]
pub struct GamdlSupportWindowResponse {
    pub minimum: String,
    pub maximum_tested: String,
    pub recommended: String,
}

/// Snapshot of the active GAMDL capability flags for the frontend (#853).
///
/// Mirrors `GamdlCapabilities` in `src/types/index.ts`. The Settings UI
/// uses this to render the right wrapper UI block (v1 three-fields vs
/// v2 single-URL) and to enable / disable the legacy tool-path inputs.
/// Zero I/O — reads from the in-memory `detected_version` cache.
#[derive(serde::Serialize, Debug, Clone)]
pub struct GamdlCapabilities {
    /// Whether the detected GAMDL release uses the wrapper-v2 single-URL
    /// dispatch path (i.e. `--wrapper-url` is the wrapper flag and the
    /// three v1 sockets are removed). `true` for ≥ 3.6.
    pub wrapper_v2: bool,
    /// Whether the detected GAMDL release does its own native muxing
    /// (FFmpeg / MP4Box / mp4decrypt path options dropped). `true` for
    /// ≥ 3.6.
    pub native_muxing: bool,
    /// Whether the detected GAMDL release uses the new `aac-web` /
    /// `aac-he-web` codec identifiers (vs the historical `aac-legacy`
    /// / `aac-he-legacy`). `true` for ≥ 3.6.
    pub aac_web_codec_rename: bool,
    /// Whether the detected GAMDL release still accepts
    /// `--music-video-remux-mode`. `true` for ≤ 3.5.x.
    pub music_video_remux_mode: bool,
    /// Whether the detected GAMDL release accepts `--wrapper-m3u8-ip`.
    /// `true` for 3.1 – 3.5.x.
    pub wrapper_m3u8_ip: bool,
    /// Whether the detected GAMDL release recognises
    /// `--playlist-folder-template`. `true` for ≥ 3.0.
    pub playlist_folder_template: bool,
    /// Whether the detected GAMDL release supports the native
    /// `--song-codec-priority` chain. `true` for ≥ 2.9.1.
    pub native_codec_priority: bool,
    /// Whether the detected GAMDL release accepts `--ffmpeg-path`
    /// (and `ffmpeg_path` INI key). `true` on `<3.6` (original
    /// tool-path era) OR `>=3.7` (REINSTATED for N_m3u8DL-RE's HLS
    /// streaming). Only `false` on the `3.6.x` line. Used by the
    /// future Settings → Tools tab to grey out the FFmpeg-path
    /// input on v3.6.x where it would crash GAMDL. (#867)
    pub ffmpeg_path: bool,
    /// Whether the detected GAMDL release's `/v1/play/assets` endpoint
    /// unlocks every non-web song codec except ALAC for wrapper-less
    /// downloads (#963, #1002). `true` for ≥ 3.8. Drives version-aware
    /// prose in the frontend (e.g. `FallbackTab.tsx`'s wrapper-dependency
    /// note) via the `useGamdlCapabilities` hook — the codec dropdown's
    /// `(Experimental)` labels themselves stay unconditional (#965).
    pub assets_api_unlocks_lossy_codecs: bool,
}

/// Returns the currently active GAMDL capability flags (#853).
///
/// Used by the Settings UI's Advanced > Wrapper section to render the
/// v1 vs v2 UI, and by future Apple-Music-specific settings panes that
/// want to hide options that aren't relevant for the installed release.
///
/// Returns `false` for every flag when the version cache hasn't been
/// populated yet (mirrors `gamdl_capabilities::supports`).
///
/// **Frontend caller:** `getGamdlCapabilities()` in
/// `src/lib/tauri-commands.ts`.
#[tauri::command]
pub fn get_gamdl_capabilities() -> GamdlCapabilities {
    use crate::services::gamdl_capabilities::{supports, GamdlFeature};
    GamdlCapabilities {
        wrapper_v2: supports(GamdlFeature::WrapperUrl),
        native_muxing: supports(GamdlFeature::NativeMuxing),
        aac_web_codec_rename: supports(GamdlFeature::AacWebCodecRename),
        music_video_remux_mode: supports(GamdlFeature::MusicVideoRemuxMode),
        wrapper_m3u8_ip: supports(GamdlFeature::WrapperM3u8Ip),
        playlist_folder_template: supports(GamdlFeature::PlaylistFolderTemplate),
        native_codec_priority: supports(GamdlFeature::NativeCodecPriority),
        ffmpeg_path: supports(GamdlFeature::FFmpegPath),
        assets_api_unlocks_lossy_codecs: supports(GamdlFeature::AssetsApiUnlocksLossyCodecs),
    }
}

/// Checks whether votify is installed in the managed Python environment.
///
/// **Frontend caller:** `checkVotifyStatus()` in `src/lib/tauri-commands.ts`
///
/// votify is the Spotify download engine — required for Spotify support.
/// Uses the generic pip engine service for version detection.
#[tauri::command]
pub async fn check_votify_status(app: AppHandle) -> Result<DependencyStatus, String> {
    let version =
        crate::services::pip_engine_service::get_pip_engine_version(&app, "votify").await?;

    Ok(DependencyStatus {
        name: "votify".to_string(),
        required: true,
        installed: version.is_some(),
        version,
        path: None,
        source: None,
    })
}

/// Installs votify via pip into the managed Python environment.
///
/// **Frontend caller:** `installVotify()` in `src/lib/tauri-commands.ts`
///
/// # Version window (A4)
/// Routes through `spotify_service::install_votify(&app, None)`, which
/// resolves to the bounded `votify_capabilities::pip_version_spec()`
/// (`votify>={min},<={max}`) rather than an unbounded `pip install
/// --upgrade votify`. This is the routine setup-wizard install path — it
/// always lands on the newest version inside MeedyaDL's validated support
/// window, the same guarantee GAMDL's install path already had. An
/// above-ceiling "Untested" release is only ever installed when a user
/// explicitly opts in via `upgrade_votify(target_version)`.
#[tauri::command]
pub async fn install_votify(app: AppHandle) -> Result<String, String> {
    log::info!("Installing votify...");
    emit_app_log(&app, "Installing votify...");
    let version = crate::services::spotify_service::install_votify(&app, None).await?;
    log::info!("votify v{version} installed");
    emit_app_log(&app, &format!("votify v{version} installed"));
    Ok(version)
}

/// Checks whether OF-Scraper is installed in the managed Python environment.
///
/// **Frontend caller:** `checkOfscraperStatus()` in `src/lib/tauri-commands.ts`
///
/// OF-Scraper is an optional download engine — disabled by default.
/// Currently disabled in engines.toml (enabled = false); this command exists
/// for future use.
#[tauri::command]
pub async fn check_ofscraper_status(app: AppHandle) -> Result<DependencyStatus, String> {
    let version =
        crate::services::pip_engine_service::get_pip_engine_version(&app, "ofscraper").await?;

    Ok(DependencyStatus {
        name: "OF-Scraper".to_string(),
        required: false,
        installed: version.is_some(),
        version,
        path: None,
        source: None,
    })
}

/// Installs OF-Scraper via pip into the managed Python environment.
///
/// **Frontend caller:** `installOfscraper()` in `src/lib/tauri-commands.ts`
///
/// Currently disabled in engines.toml (enabled = false); this command exists
/// for future use.
#[tauri::command]
pub async fn install_ofscraper(app: AppHandle) -> Result<String, String> {
    log::info!("Installing OF-Scraper...");
    emit_app_log(&app, "Installing OF-Scraper...");
    let version =
        crate::services::pip_engine_service::install_pip_engine(&app, "ofscraper").await?;
    log::info!("OF-Scraper v{version} installed");
    emit_app_log(&app, &format!("OF-Scraper v{version} installed"));
    Ok(version)
}

/// Checks the installation status of all external tool dependencies.
///
/// **Frontend caller:** `checkAllDependencies()` in `src/lib/tauri-commands.ts`
///
/// Returns a list of all external tool dependencies (`FFmpeg`, mp4decrypt,
/// N_m3u8DL-RE, `MP4Box`) with their current installation status. Each tool
/// is checked by verifying whether a binary exists at its expected path
/// inside the app data directory.
///
/// Version detection is intentionally skipped in this batch check because
/// running each tool with `--version` is slow and unnecessary for the
/// setup wizard's "installed/not installed" display.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for resolving tool binary paths.
///
/// # Errors
///
/// Returns `Err(String)` if tool binary path resolution fails.
///
/// # Returns
/// * `Ok(Vec<DependencyStatus>)` - Status for each registered tool.
///   The order matches the tool registration order in `dependency_manager`.
#[tauri::command]
pub async fn check_all_dependencies(app: AppHandle) -> Result<Vec<DependencyStatus>, String> {
    // get_all_tools() returns the static list of tool definitions
    // (id, name, required, download URLs per platform)
    let tools = dependency_manager::get_all_tools();
    let mut results = Vec::new();

    // Check each tool's installation status by probing for its binary
    for tool in tools {
        // Resolve the binary path. get_tool_binary_path honours an already-
        // adopted system tool (its `.external-path` pointer) or a managed
        // install. If NEITHER exists, try to DETECT + adopt a system/Homebrew
        // install in place — #1081 status gap: previously detection only ran on
        // an explicit Install click, so `brew install`ed FFmpeg/MP4Box/… showed
        // as missing on a Finder-launched macOS app (minimal launchd PATH).
        let mut binary_path = dependency_manager::get_tool_binary_path(&app, tool.id);
        let mut adopted_source: Option<String> = None;
        if !binary_path.exists() {
            if let Some((sys_path, source)) =
                dependency_manager::adopt_system_tool_if_available(&app, tool.id).await
            {
                binary_path = sys_path;
                adopted_source = Some(source);
            }
        }
        // Functional check (#391): verify the binary exists AND can execute.
        // Reuses dependency_manager::get_tool_version() so each tool uses its
        // configured version flag and parser (e.g., FFmpeg uses -version, mp4decrypt
        // runs with no args) rather than assuming every tool supports --version.
        // Runs with a 2-second timeout to prevent stalling on broken binaries.
        //
        // Stricter than simple existence checking: `false` when the spawn fails
        // outright (permission denied, corrupt binary). Timeout is treated as
        // "still installed but slow" to avoid false negatives on loaded systems.
        let installed = if binary_path.exists() {
            let version_check_path = binary_path.clone();
            let tool_id_str = tool.id.to_string();
            match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                tokio::task::spawn(async move {
                    crate::services::dependency_manager::get_tool_version(
                        &version_check_path,
                        &tool_id_str,
                    )
                    .await
                }),
            )
            .await
            {
                Ok(Ok(Ok(_))) => true,
                Ok(Ok(Err(err))) => {
                    // Binary failed to spawn entirely (e.g., permission denied)
                    log::warn!(
                        "Tool {} exists but failed dependency-manager verification: {}",
                        tool.id,
                        err
                    );
                    false
                }
                Ok(Err(err)) => {
                    log::warn!(
                        "Tool {} exists but version check task failed: {}",
                        tool.id,
                        err
                    );
                    false
                }
                Err(_) => {
                    // Timed out — binary is slow but likely functional; treat as installed
                    log::warn!(
                        "Tool {} version check timed out — reporting as installed",
                        tool.id
                    );
                    true
                }
            }
        } else {
            false
        };

        // Provenance for the wizard badge: prefer the source we just detected
        // (adopted this call), else the persisted `.source` marker written by
        // install_tool / a prior adoption ("managed" / "system" / "homebrew:<formula>").
        let source = if installed {
            adopted_source.or_else(|| {
                let tool_dir = dependency_manager::get_tool_dir(&app, tool.id);
                std::fs::read_to_string(tool_dir.join(".source"))
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })
        } else {
            None
        };

        results.push(DependencyStatus {
            name: tool.name.to_string(),
            required: tool.required,
            installed,
            version: None, // Version detection is slow; skip for batch checks
            // Only include the path if the binary actually exists
            path: if installed {
                binary_path.to_str().map(std::string::ToString::to_string)
            } else {
                None
            },
            source,
        });
    }

    Ok(results)
}

/// Read-only information about a `gamdl` command-line entry point installed
/// OUTSIDE MeedyaDL's managed venv (typically `pipx install gamdl`).
///
/// The frontend uses this purely to INFORM the user ("GAMDL X is also
/// installed via pipx — MeedyaDL keeps its own tested copy"). MeedyaDL never
/// consumes or updates the external copy.
#[derive(Debug, Clone, Serialize)]
pub struct ExternalGamdlInfo {
    /// Absolute path to the external `gamdl` entry-point binary.
    pub path: String,
    /// Detected version (e.g. "3.9.0").
    pub version: String,
    /// The `.source`-style provenance marker (e.g. `pipx:gamdl`, `system`).
    pub source: String,
    /// Whether that version is inside MeedyaDL's tested support window
    /// (per the current platform's ceiling).
    pub in_support_window: bool,
    /// Human classification: `supported` | `untested` | `unsupported`.
    pub classification: String,
}

/// Detects a `gamdl` command-line entry point installed on the system OUTSIDE
/// MeedyaDL's managed venv, purely to INFORM the user.
///
/// **Frontend caller:** `detectExternalGamdl()` in `src/lib/tauri-commands.ts`
///
/// MeedyaDL always keeps and uses its own tested, version-controlled GAMDL
/// (bounded pip spec in its managed venv); it never consumes or mutates an
/// external one. Reusing a pipx/user-pip GAMDL would forfeit the
/// support-window / wrapper-era / wheel-ABI guarantees the download pipeline
/// depends on, and would mutate a tool the user owns for their own CLI use
/// (see the 2026-08-10 package-manager design doc, decision A). This command
/// is strictly read-only: no install, no upgrade, no filesystem mutation.
///
/// # Returns
/// * `Ok(Some(info))` when an external `gamdl` entry point is found.
/// * `Ok(None)` when none is found (or only MeedyaDL's own managed copy).
#[tauri::command]
pub async fn detect_external_gamdl(app: AppHandle) -> Result<Option<ExternalGamdlInfo>, String> {
    Ok(find_external_gamdl(&app).await)
}

/// Implementation behind [`detect_external_gamdl`]. Searches PATH, the pipx
/// user shim dir, and the system package-manager dirs for a `gamdl` entry
/// point, excluding MeedyaDL's own managed venv, and reports the first
/// trusted, version-probeable one.
async fn find_external_gamdl(app: &AppHandle) -> Option<ExternalGamdlInfo> {
    use crate::services::gamdl_capabilities::{
        classify_for_platform, current_platform_id, VersionSupport,
    };

    // MeedyaDL's own managed venv gamdl lives under the app python dir; never
    // report it as an "external" install.
    let managed_python_dir = crate::utils::platform::get_python_dir(app);
    let bin_name = if cfg!(windows) { "gamdl.exe" } else { "gamdl" };

    // Candidate entry points: PATH (`which`/`where`), the pipx user shim dir
    // (~/.local/bin), and the system package-manager search dirs.
    let mut candidates: Vec<PathBuf> = Vec::new();
    let which_cmd = if cfg!(windows) { "where" } else { "which" };
    if let Ok(out) = tokio::process::Command::new(which_cmd)
        .arg("gamdl")
        .output()
        .await
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let p = PathBuf::from(line.trim());
                if p.is_absolute() {
                    candidates.push(p);
                }
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(&home).join(".local/bin").join(bin_name));
    }
    for dir in dependency_manager::system_tool_search_dirs() {
        candidates.push(dir.join(bin_name));
    }

    for cand in candidates {
        if !cand.is_file() || cand.starts_with(&managed_python_dir) {
            continue;
        }
        if !dependency_manager::is_trusted_binary(&cand) {
            continue;
        }
        // Probe the version with a short timeout (a broken binary must not stall
        // the wizard).
        let version = match tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tokio::process::Command::new(&cand).arg("--version").output(),
        )
        .await
        {
            Ok(Ok(o)) => {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                GAMDL_VERSION_RE.find(&text).map(|m| m.as_str().to_string())
            }
            _ => None,
        };
        let Some(version) = version else {
            continue;
        };

        let (in_support_window, classification) =
            match classify_for_platform(Some(&version), current_platform_id()) {
                VersionSupport::Supported { .. } => (true, "supported"),
                VersionSupport::Untested { .. } => (false, "untested"),
                VersionSupport::Unsupported { .. } | VersionSupport::NotInstalled => {
                    (false, "unsupported")
                }
            };

        let source = package_manager::detect_owner(&cand)
            .await
            .map(|r| r.to_marker())
            .unwrap_or_else(|| "system".to_string());

        return Some(ExternalGamdlInfo {
            path: cand.to_string_lossy().to_string(),
            version,
            source,
            in_support_window,
            classification: classification.to_string(),
        });
    }

    None
}

/// Downloads and installs a specific tool dependency.
///
/// **Frontend caller:** `installDependency(name)` in `src/lib/tauri-commands.ts`
///
/// Determines the correct download URL for the current platform and
/// architecture, downloads the archive (zip/tar.gz), extracts the binary
/// to `{app_data}/tools/{tool_id}/`, and verifies it runs successfully.
///
/// This is a long-running operation that involves network downloads.
/// The frontend shows a loading state while awaiting the result.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for resolving paths and platform detection.
/// * `name` - The tool identifier string. Must be one of:
///   - `"ffmpeg"` - Audio/video codec tool (required for GAMDL)
///   - `"mp4decrypt"` - Bento4 MP4 decryption tool (required for DRM content)
///   - `"nm3u8dlre"` - N_m3u8DL-RE stream downloader (required for HLS streams)
///   - `"mp4box"` - GPAC `MP4Box` muxing tool (optional, improves metadata)
///
/// # Errors
///
/// Returns `Err(String)` if the tool download, extraction, or verification fails.
///
/// # Returns
/// * `Ok(String)` - Success message with the installed tool path.
/// * `Err(String)` - Download, extraction, or verification failure message.
#[tauri::command]
pub async fn install_dependency(app: AppHandle, name: String) -> Result<String, String> {
    // Delegates to dependency_manager which handles platform-specific
    // URL resolution, download, archive extraction, and binary verification.
    log::info!("Installing dependency: {name}");
    emit_app_log(&app, &format!("Updating {name}..."));
    match dependency_manager::install_tool(&app, &name).await {
        Ok(result) => {
            emit_app_log(&app, &format!("{name} updated successfully"));
            Ok(result)
        }
        Err(e) => {
            emit_app_log(&app, &format!("Failed to update {name}: {e}"));
            Err(e)
        }
    }
}

/// A single component version entry for the About screen and Activity Log.
///
/// Contains the human-readable name and detected version string for each
/// installed component (Python, GAMDL, external tools).
#[derive(Debug, Clone, Serialize)]
pub struct ComponentVersion {
    /// Component display name (e.g., "Python", "GAMDL", "FFmpeg")
    pub name: String,
    /// Detected version string (e.g., "3.12.8", "2.9.3", "ffmpeg version 7.1")
    /// `None` if the component is not installed.
    pub version: Option<String>,
    /// Whether the component is currently installed
    pub installed: bool,
}

/// Temporary inline votify version probe used by `get_component_versions` in PR M9-1 (#101).
///
/// Runs `python -m pip show votify` in the managed Python environment and
/// scrapes the `Version:` line. Returns `None` if Python isn't installed,
/// votify isn't in the env, or the subprocess fails/times out (10 s
/// budget — matches the GAMDL probe's allowance for stalled network
/// mounts).
///
/// **Replaced in PR M9-2** by the fully-fledged
/// `votify_service::get_votify_version` (mirroring
/// `gamdl_service::get_gamdl_version`). For M9-1 we just need a real
/// version string so the activity log + Updates page + capability cache
/// have something meaningful to display.
async fn probe_votify_version(app: &AppHandle) -> Option<String> {
    use crate::utils::platform;

    let python_dir = platform::get_python_dir(app);
    // Use the venv-aware resolver (#1017 / A3 fix) — a system-Python venv
    // on Windows puts `python.exe` under `Scripts/`, not at the portable
    // root. The pure `get_python_binary_path` only knows the portable
    // layout and would report "not installed" for a perfectly valid
    // system-venv Python, silently breaking votify version detection.
    let python_bin = platform::resolve_managed_python_binary(&python_dir);
    if !python_bin.exists() {
        return None;
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new(&python_bin)
            .args(["-m", "pip", "show", "votify"])
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("Version:") {
            let version = rest.trim().to_string();
            if !version.is_empty() {
                return Some(version);
            }
        }
    }
    None
}

/// Retrieves the version information for all MeedyaDL components.
///
/// **Frontend caller:** `getComponentVersions()` in `src/lib/tauri-commands.ts`
///
/// Unlike `check_all_dependencies` (which skips version detection for speed),
/// this command runs `--version` on each installed tool to gather actual version
/// strings. Intended for display in Help > About and for Activity Log startup
/// messages.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for resolving tool binary paths.
///
/// # Returns
/// * `Ok(Vec<ComponentVersion>)` - Version info for all components.
///   Components that are not installed will have `version: None`.
#[tauri::command]
pub async fn get_component_versions(app: AppHandle) -> Result<Vec<ComponentVersion>, String> {
    let mut versions = Vec::new();

    // Python version
    let python_version = python_manager::check_python_status(&app).await.ok().flatten();
    versions.push(ComponentVersion {
        name: "Python".to_string(),
        version: python_version.clone(),
        installed: python_version.is_some(),
    });

    // GAMDL version (via pip show)
    let gamdl_version = gamdl_service::get_gamdl_version(&app).await.ok().flatten();
    versions.push(ComponentVersion {
        name: "GAMDL".to_string(),
        version: gamdl_version.clone(),
        installed: gamdl_version.is_some(),
    });

    // votify version (via pip show) — PR M9-1 (#101).
    //
    // Temporary inline probe. The full `votify_service::get_votify_version()`
    // helper (mirroring `gamdl_service::get_gamdl_version`) lands in PR M9-2
    // alongside the rest of the votify subprocess wiring. For M9-1 we run
    // the same `pip show` shell-out here so the activity log and the
    // Updates / Diagnostics surfaces have a real version string to display
    // — and so the version cache in `votify_capabilities` is populated for
    // any caller that needs to gate on a feature.
    let votify_version = probe_votify_version(&app).await;
    super::super::services::votify_capabilities::set_detected_version(
        votify_version.clone(),
    );
    versions.push(ComponentVersion {
        name: "votify".to_string(),
        version: votify_version,
        installed: gamdl_version.is_some(), // Python presence proxies install availability
    });

    // External tools: FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box
    for tool in dependency_manager::get_all_tools() {
        let binary_path = dependency_manager::get_tool_binary_path(&app, tool.id);
        let installed = binary_path.exists();
        let version = if installed {
            dependency_manager::get_tool_version(&binary_path, tool.id)
                .await
                .ok()
        } else {
            None
        };
        versions.push(ComponentVersion {
            name: tool.name.to_string(),
            version,
            installed,
        });
    }

    Ok(versions)
}

/// Emits component version information to the Activity Log at startup.
///
/// Called during app setup to log all installed component versions as a
/// `[System]` entry. Useful for debugging user-reported issues.
///
/// Also emits a second `[System]` line classifying the installed GAMDL
/// version against this build's support window
/// (`services::gamdl_capabilities::classify`). Users / support staff
/// can see at a glance whether GAMDL is below our floor
/// (Unsupported), inside the range (Supported), above the tested
/// ceiling (Untested), or missing entirely (Not installed) — without
/// having to correlate version strings against the README support
/// matrix by hand.
pub async fn log_component_versions_to_activity(app: &AppHandle) {
    match get_component_versions(app.clone()).await {
        Ok(versions) => {
            let version_strings: Vec<String> = versions
                .iter()
                .filter(|v| v.installed)
                .map(|v| {
                    format!(
                        "{} {}",
                        v.name,
                        v.version.as_deref().unwrap_or("(unknown)")
                    )
                })
                .collect();
            if !version_strings.is_empty() {
                let msg = format!("Component versions: {}", version_strings.join(", "));
                emit_app_log(app, &msg);
                log::info!("{msg}");
            }

            // Pull the GAMDL version directly (the main list above
            // stringifies everything together, which loses the
            // ability to classify any one component).
            let gamdl_version = versions
                .iter()
                .find(|v| v.name == "GAMDL")
                .and_then(|v| v.version.clone());

            emit_gamdl_support_status(app, gamdl_version.as_deref());
        }
        Err(e) => {
            log::warn!("Failed to gather component versions: {e}");
        }
    }
}

/// Renders a `[System]` activity-log entry describing how the
/// installed GAMDL version relates to this build's support window.
///
/// The classification is visible to users (in the activity log) and
/// to us (in crash reports). On Unsupported / Untested we write a
/// `log::warn!` so the entry is also captured by the tracing sink
/// and shows up in the rotated log file even without activity-log
/// subscribers.
///
/// Uses the platform-aware [`classify_for_platform`] / effective ceiling
/// (#1014) rather than the plain global [`classify`] — on every platform
/// without a `[gamdl.platform_ceilings]` override (everything except
/// Linux ARMv7 today) this produces byte-identical output to before;
/// on Linux ARMv7 it reports the real, ARMv7-reachable ceiling instead
/// of a global one that platform can't actually install.
fn emit_gamdl_support_status(app: &AppHandle, gamdl_version: Option<&str>) {
    use crate::services::gamdl_capabilities::{
        classify_for_platform, current_platform_id, effective_maximum_tested, support_window,
        VersionSupport,
    };

    let window = support_window();
    let platform_id = current_platform_id();
    let maximum_tested = effective_maximum_tested(platform_id);
    let status = classify_for_platform(gamdl_version, platform_id);

    let line = match &status {
        VersionSupport::NotInstalled => format!(
            "GAMDL support: not installed (supported range {min}–{max})",
            min = window.minimum,
            max = maximum_tested,
        ),
        VersionSupport::Supported { installed } => format!(
            "GAMDL support: {installed} is inside the validated range \
             {min}–{max}",
            min = window.minimum,
            max = maximum_tested,
        ),
        VersionSupport::Unsupported { installed, minimum } => format!(
            "GAMDL support: {installed} is below the supported floor \
             ({minimum}). Some features may silently degrade; update \
             via Settings > Tools.",
        ),
        VersionSupport::Untested {
            installed,
            maximum_tested,
            recommended,
        } => format!(
            "GAMDL support: {installed} is newer than this MeedyaDL \
             build has validated ({maximum_tested}). Downloads may \
             fail on CLI changes; consider downgrading to \
             {recommended}.",
        ),
    };

    emit_app_log(app, &line);
    match &status {
        VersionSupport::Unsupported { .. } | VersionSupport::Untested { .. } => {
            log::warn!("{line}");
        }
        VersionSupport::Supported { .. } | VersionSupport::NotInstalled => {
            log::info!("{line}");
        }
    }
}
