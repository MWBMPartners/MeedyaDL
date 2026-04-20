// Copyright (c) 2026 MeedyaDL
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
use crate::services::{dependency_manager, gamdl_service, python_manager};
use crate::utils::activity_log::emit_app_log;

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
    let python_bin = crate::utils::platform::get_python_binary_path(&python_dir);

    Ok(DependencyStatus {
        // Display the expected version in the name (e.g., "Python 3.12")
        name: format!("Python {}", python_manager::expected_python_version()),
        required: true, // Python is always required — GAMDL runs on Python
        installed: version.is_some(), // None means not installed
        version,
        // Convert PathBuf to String for JSON serialization
        path: python_bin.to_str().map(std::string::ToString::to_string),
        source: None, // Python is always managed (portable runtime)
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
    let version = gamdl_service::install_gamdl(&app).await?;
    log::info!("GAMDL v{version} installed");
    emit_app_log(&app, &format!("GAMDL v{version} installed"));
    Ok(version)
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
#[tauri::command]
pub async fn install_votify(app: AppHandle) -> Result<String, String> {
    log::info!("Installing votify...");
    emit_app_log(&app, "Installing votify...");
    let version =
        crate::services::pip_engine_service::install_pip_engine(&app, "votify").await?;
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
        // Resolve the expected binary path: {app_data}/tools/{tool_id}/{binary_name}
        let binary_path = dependency_manager::get_tool_binary_path(&app, tool.id);
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

        // Read the .source marker file to determine where the tool was installed from.
        // Written by install_tool() as "system" or "managed" during installation.
        let source = if installed {
            let tool_dir = dependency_manager::get_tool_dir(&app, tool.id);
            let source_file = tool_dir.join(".source");
            std::fs::read_to_string(source_file)
                .ok()
                .map(|s| s.trim().to_string())
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
        }
        Err(e) => {
            log::warn!("Failed to gather component versions: {e}");
        }
    }
}
