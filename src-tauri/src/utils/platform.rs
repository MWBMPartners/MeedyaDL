// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Platform utilities for OS detection and path resolution.
// =========================================================
//
// gamdl-GUI ships a fully self-contained environment: its own portable
// Python runtime, its own GAMDL installation (via pip), and its own
// copies of external tools (FFmpeg, mp4decrypt, etc.). This avoids
// conflicts with any system-level Python or tool installations the user
// may have.
//
// All of this data lives under a single root directory that varies by
// operating system:
//   - macOS:   ~/Library/Application Support/com.mwbmpartners.gamdl-gui/
//   - Windows: %APPDATA%\com.mwbmpartners.gamdl-gui\
//   - Linux:   ~/.local/share/com.mwbmpartners.gamdl-gui/
//
// The functions in this module resolve paths relative to that root. They
// are used by the `services` layer (python_manager, dependency_manager,
// config_service) and by the `commands::system` IPC handlers.
//
// Directory layout under the app data root:
//   {app_data}/
//   +-- python/          -- Portable Python runtime from python-build-standalone
//   |   +-- bin/python3  -- (macOS/Linux) or python.exe (Windows)
//   |   +-- bin/pip3     -- (macOS/Linux) or Scripts/pip.exe (Windows)
//   +-- tools/           -- External tool binaries
//   |   +-- ffmpeg/
//   |   +-- mp4decrypt/
//   |   +-- ...
//   +-- gamdl/           -- GAMDL-specific data
//   |   +-- config.ini   -- GAMDL configuration (synced from app settings)
//   +-- settings.json    -- App settings (managed by tauri-plugin-store)
//
// Reference: https://v2.tauri.app/reference/rust/tauri/struct.AppHandle.html
// Reference: https://doc.rust-lang.org/std/path/struct.PathBuf.html
// Reference: https://doc.rust-lang.org/std/path/struct.Path.html

use std::path::{Path, PathBuf};
// `AppHandle` is the primary handle to the running Tauri application, used
// to access managed state, emit events, and resolve paths.
// `Manager` is a trait that provides `.path()` for path resolution.
// Reference: https://docs.rs/tauri/latest/tauri/trait.Manager.html
use tauri::{AppHandle, Manager};

/// Returns the root application data directory for gamdl-GUI.
///
/// This is the self-contained directory where all application data lives:
/// - Python runtime
/// - GAMDL installation
/// - External tools (FFmpeg, mp4decrypt, etc.)
/// - Application settings
/// - GAMDL configuration
///
/// Platform-specific locations:
/// - macOS:   `~/Library/Application Support/com.mwbmpartners.gamdl-gui/`
/// - Windows: `%APPDATA%\com.mwbmpartners.gamdl-gui\`
/// - Linux:   `~/.local/share/com.mwbmpartners.gamdl-gui/`
///
/// # Fallback behaviour
/// If Tauri's path resolver fails (which should not happen in normal
/// operation), falls back to the `dirs` crate to manually construct the
/// platform-appropriate data directory. As a last resort, uses the
/// current working directory (`.`).
///
/// # Arguments
/// * `app` - A reference to the Tauri `AppHandle`, which provides access
///   to path resolution via the `Manager` trait.
///
/// # Returns
/// A `PathBuf` pointing to the application data root directory.
///
/// # Reference
/// - `AppHandle::path()`: <https://docs.rs/tauri/latest/tauri/struct.AppHandle.html#method.path>
/// - `PathResolver::app_data_dir()`: <https://docs.rs/tauri/latest/tauri/path/struct.PathResolver.html>
pub fn get_app_data_dir(app: &AppHandle) -> PathBuf {
    // Tauri's `app.path().app_data_dir()` reads the `identifier` field from
    // `tauri.conf.json` (e.g., "com.mwbmpartners.gamdl-gui") and appends it
    // to the OS-standard application data directory.
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| {
            // Fallback: construct the path manually using the `dirs` crate,
            // which queries the same OS environment variables / APIs that
            // Tauri uses internally. This path is only reached if Tauri's
            // path resolver encounters an unexpected error.
            // Reference: https://docs.rs/dirs/latest/dirs/fn.data_dir.html
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("com.mwbmpartners.gamdl-gui")
        })
}

/// Returns the directory where the portable Python runtime is installed.
///
/// Path: `{app_data}/python/`
///
/// Contains the full Python installation extracted from the
/// [python-build-standalone](https://github.com/indygreg/python-build-standalone)
/// GitHub releases. This is a relocatable, self-contained Python that does
/// not require a system-level installation.
///
/// # Arguments
/// * `app` - A reference to the Tauri `AppHandle`.
///
/// # Returns
/// A `PathBuf` to the Python installation root directory.
///
/// # Connection
/// Called by `services::python_manager` when installing or verifying Python.
pub fn get_python_dir(app: &AppHandle) -> PathBuf {
    get_app_data_dir(app).join("python")
}

/// Returns the path to the Python binary for the current platform.
///
/// The binary location differs by OS because python-build-standalone uses
/// different directory layouts:
/// - macOS/Linux: `{python_dir}/bin/python3`   (Unix FHS convention)
/// - Windows:     `{python_dir}/python.exe`    (flat layout convention)
///
/// # Arguments
/// * `python_dir` - The root of the portable Python installation (from
///   [`get_python_dir`]).
///
/// # Returns
/// A `PathBuf` to the platform-appropriate Python executable.
///
/// # Compile-time branching
/// Uses `cfg!(target_os = ...)` which is evaluated at **compile time**, so
/// the unused branch is eliminated entirely from the binary. This is
/// different from runtime OS detection.
///
/// # Reference
/// - `cfg!` macro: <https://doc.rust-lang.org/std/macro.cfg.html>
/// - `Path::join`: <https://doc.rust-lang.org/std/path/struct.Path.html#method.join>
pub fn get_python_binary_path(python_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        // Windows python-build-standalone layout: python.exe at root
        python_dir.join("python.exe")
    } else {
        // macOS/Linux python-build-standalone layout: bin/python3
        python_dir.join("bin").join("python3")
    }
}

/// Returns the path to the pip binary for the current platform.
///
/// The pip location mirrors the Python installation layout:
/// - macOS/Linux: `{python_dir}/bin/pip3`          (alongside python3)
/// - Windows:     `{python_dir}/Scripts/pip.exe`   (Windows convention)
///
/// # Arguments
/// * `python_dir` - The root of the portable Python installation.
///
/// # Returns
/// A `PathBuf` to the platform-appropriate pip executable.
///
/// # Connection
/// Called by `services::python_manager` to run `pip install gamdl` and
/// by `services::gamdl_service` to verify GAMDL is installed.
pub fn get_pip_binary_path(python_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        // Windows: pip is installed in the Scripts subdirectory
        python_dir.join("Scripts").join("pip.exe")
    } else {
        // macOS/Linux: pip is installed in the bin subdirectory alongside python3
        python_dir.join("bin").join("pip3")
    }
}

/// Returns the directory where external tools are installed.
///
/// Path: `{app_data}/tools/`
///
/// Each tool gets its own subdirectory:
/// - `tools/ffmpeg/`      -- audio/video transcoding
/// - `tools/mp4decrypt/`  -- Bento4 MPEG-DASH decryption
/// - `tools/mp4box/`      -- GPAC MP4 muxing
/// - `tools/nm3u8dl-re/`  -- HLS/DASH stream downloader
///
/// # Arguments
/// * `app` - A reference to the Tauri `AppHandle`.
///
/// # Returns
/// A `PathBuf` to the tools root directory.
///
/// # Connection
/// Called by `services::dependency_manager` when installing or locating
/// external tool binaries.
pub fn get_tools_dir(app: &AppHandle) -> PathBuf {
    get_app_data_dir(app).join("tools")
}

/// Returns the directory for GAMDL-specific data (config, cache).
///
/// Path: `{app_data}/gamdl/`
///
/// Contains GAMDL's `config.ini` and any temporary/cache files that GAMDL
/// creates during operation. Keeping GAMDL data under the app data root
/// ensures the entire application state can be removed by deleting a
/// single directory.
///
/// # Arguments
/// * `app` - A reference to the Tauri `AppHandle`.
///
/// # Returns
/// A `PathBuf` to the GAMDL data directory.
///
/// # Connection
/// Called by `services::config_service` when syncing settings to GAMDL's
/// INI format, and by `services::gamdl_service` when launching GAMDL
/// with `--config-path`.
pub fn get_gamdl_data_dir(app: &AppHandle) -> PathBuf {
    get_app_data_dir(app).join("gamdl")
}

/// Returns the path to the GAMDL config file.
///
/// Path: `{app_data}/gamdl/config.ini`
///
/// This config file is passed to the GAMDL CLI via the `--config-path`
/// flag so that GAMDL reads/writes its configuration from the app's
/// self-contained directory rather than from `~/.gamdl/config.ini`.
/// The `config_service` module translates the app's JSON settings into
/// GAMDL's INI format before each download.
///
/// # Arguments
/// * `app` - A reference to the Tauri `AppHandle`.
///
/// # Returns
/// A `PathBuf` to the GAMDL configuration INI file.
///
/// # Connection
/// Called by `services::config_service::sync_gamdl_config()` and
/// `services::gamdl_service::build_gamdl_command()`.
pub fn get_gamdl_config_path(app: &AppHandle) -> PathBuf {
    get_gamdl_data_dir(app).join("config.ini")
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ----------------------------------------------------------
    // get_python_binary_path
    // ----------------------------------------------------------

    /// Verifies that `get_python_binary_path` returns the correct
    /// platform-specific path relative to the given Python directory.
    /// On macOS/Linux the binary should be at `bin/python3`; on
    /// Windows it should be at `python.exe` in the root.
    #[test]
    fn python_binary_path_has_correct_suffix() {
        let base = Path::new("/opt/python");
        let result = get_python_binary_path(base);

        if cfg!(target_os = "windows") {
            assert!(
                result.ends_with("python.exe"),
                "On Windows, python binary path should end with 'python.exe', got: {:?}",
                result
            );
        } else {
            assert!(
                result.ends_with("bin/python3"),
                "On macOS/Linux, python binary path should end with 'bin/python3', got: {:?}",
                result
            );
        }
    }

    /// Verifies that `get_python_binary_path` correctly joins the
    /// base directory with the platform-specific binary location,
    /// producing a full absolute path.
    #[test]
    fn python_binary_path_includes_base_dir() {
        let base = Path::new("/opt/python");
        let result = get_python_binary_path(base);

        assert!(
            result.starts_with(base),
            "Python binary path should start with the base directory, got: {:?}",
            result
        );
    }

    /// Verifies that `get_python_binary_path` works correctly with
    /// a Windows-style base path (for cross-platform path construction).
    #[test]
    fn python_binary_path_with_windows_style_base() {
        let base = Path::new("C:\\Python");
        let result = get_python_binary_path(base);

        assert!(
            result.starts_with(base),
            "Python binary path should start with the Windows base directory, got: {:?}",
            result
        );
    }

    // ----------------------------------------------------------
    // get_pip_binary_path
    // ----------------------------------------------------------

    /// Verifies that `get_pip_binary_path` returns the correct
    /// platform-specific path relative to the given Python directory.
    /// On macOS/Linux the binary should be at `bin/pip3`; on
    /// Windows it should be at `Scripts/pip.exe`.
    #[test]
    fn pip_binary_path_has_correct_suffix() {
        let base = Path::new("/opt/python");
        let result = get_pip_binary_path(base);

        if cfg!(target_os = "windows") {
            assert!(
                result.ends_with("Scripts/pip.exe") || result.ends_with("Scripts\\pip.exe"),
                "On Windows, pip binary path should end with 'Scripts/pip.exe', got: {:?}",
                result
            );
        } else {
            assert!(
                result.ends_with("bin/pip3"),
                "On macOS/Linux, pip binary path should end with 'bin/pip3', got: {:?}",
                result
            );
        }
    }

    /// Verifies that `get_pip_binary_path` correctly joins the
    /// base directory with the platform-specific binary location,
    /// producing a full path rooted at the given directory.
    #[test]
    fn pip_binary_path_includes_base_dir() {
        let base = Path::new("/opt/python");
        let result = get_pip_binary_path(base);

        assert!(
            result.starts_with(base),
            "Pip binary path should start with the base directory, got: {:?}",
            result
        );
    }

    /// Verifies that `get_pip_binary_path` works correctly with
    /// a Windows-style base path (for cross-platform path construction).
    #[test]
    fn pip_binary_path_with_windows_style_base() {
        let base = Path::new("C:\\Python");
        let result = get_pip_binary_path(base);

        assert!(
            result.starts_with(base),
            "Pip binary path should start with the Windows base directory, got: {:?}",
            result
        );
    }

    /// Verifies that both `get_python_binary_path` and `get_pip_binary_path`
    /// share the same parent directory structure on Unix platforms
    /// (both live under `bin/`), ensuring consistent installation layout.
    #[test]
    fn python_and_pip_share_parent_directory() {
        let base = Path::new("/opt/python");
        let python_path = get_python_binary_path(base);
        let pip_path = get_pip_binary_path(base);

        let python_parent = python_path.parent().expect("python path should have a parent");
        let pip_parent = pip_path.parent().expect("pip path should have a parent");

        if cfg!(target_os = "windows") {
            // On Windows, python.exe is at root and pip.exe is in Scripts/
            // so parents will differ -- just verify they share the same root
            assert!(python_path.starts_with(base));
            assert!(pip_path.starts_with(base));
        } else {
            // On Unix, both should be in the bin/ subdirectory
            assert_eq!(
                python_parent, pip_parent,
                "On Unix, python3 and pip3 should share the same parent directory (bin/)"
            );
        }
    }
}
