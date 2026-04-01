// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// System information IPC commands.
// Provides the frontend with platform detection and directory path resolution
// so the UI can adapt its theme and display platform-appropriate content.
//
// ## Architecture
//
// These commands expose system-level information that the frontend needs
// at startup to configure itself correctly. They are among the first IPC
// calls made when the React app initializes:
//   1. `getPlatformInfo()` -> determines which CSS theme to load
//   2. `getAppDataDir()` -> used for displaying paths in the settings page
//
// Both commands are lightweight (no I/O, no network) and return immediately.
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command       | TypeScript Function  | Line |
// |--------------------|----------------------|------|
// | get_platform_info  | getPlatformInfo()    | ~27  |
// | get_app_data_dir   | getAppDataDir()      | ~32  |
//
// ## References
//
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - Rust std::env::consts: https://doc.rust-lang.org/std/env/consts/index.html

// serde::Serialize is required for PlatformInfo which is returned to the frontend.
use serde::Serialize;
// AppHandle is needed by get_app_data_dir() to resolve the platform-specific
// application data directory via Tauri's path resolver.
use tauri::AppHandle;

// Platform utilities module providing app data directory resolution
// and other platform-specific path helpers.
use crate::utils::platform;

/// Platform information returned to the frontend for theme selection
/// and platform-specific UI adaptations.
///
/// The frontend uses this at startup to:
/// - Load the correct platform CSS theme (macOS Liquid Glass, Windows Mica, Linux GTK)
/// - Show platform-appropriate instructions and UI elements
/// - Display system info in the About page
///
/// Implements `Serialize` for Tauri IPC serialization to JSON.
/// Maps to the `PlatformInfo` TypeScript interface in `src/types/index.ts`.
#[derive(Debug, Clone, Serialize)]
pub struct PlatformInfo {
    /// Operating system name: "macos", "windows", or "linux".
    /// Used as the key for theme selection in the frontend's platform provider.
    pub platform: String,
    /// CPU architecture string from `std::env::consts::ARCH`.
    /// Common values: "aarch64" (Apple Silicon / ARM64), "`x86_64`" (Intel/AMD 64-bit).
    /// Used for display purposes and dependency download URL resolution.
    pub arch: String,
    /// Raw Rust target OS constant from `std::env::consts::OS`.
    /// Same as `platform` for known OSes, but kept separate for
    /// debugging and display in the About page.
    pub os_type: String,
}

/// Returns information about the current platform and architecture.
///
/// **Frontend caller:** `getPlatformInfo()` in `src/lib/tauri-commands.ts`
///
/// The frontend uses this to:
/// - Load the correct platform CSS theme (macOS Liquid Glass, Windows Mica, etc.)
/// - Show platform-appropriate instructions (e.g., different cookie export steps)
/// - Display the correct architecture in the About page
///
/// This is a synchronous command (no `async`, no `AppHandle`) because all
/// information comes from compile-time constants (`std::env::consts`).
/// It returns instantly with no I/O overhead.
///
/// # Returns
/// `PlatformInfo` - Always succeeds (no Result wrapper needed since
/// `std::env::consts` values are always available).
/// Note: Commands that don't return `Result<T, String>` can still be
/// called from the frontend — Tauri wraps them in a Result automatically.
/// See: <https://v2.tauri.app/develop/calling-rust/#return-types>
#[tauri::command]
#[must_use]
pub fn get_platform_info() -> PlatformInfo {
    PlatformInfo {
        // Map Rust OS constants to our frontend platform identifiers.
        // std::env::consts::OS returns the target OS at compile time:
        // "macos", "windows", "linux", "freebsd", etc.
        // See: https://doc.rust-lang.org/std/env/consts/constant.OS.html
        platform: match std::env::consts::OS {
            "macos" => "macos".to_string(),
            "windows" => "windows".to_string(),
            // All non-macOS, non-Windows platforms fall back to the Linux theme.
            // This covers Linux, FreeBSD, and other Unix-like systems.
            _ => "linux".to_string(),
        },
        // CPU architecture identifier from std::env::consts::ARCH.
        // Common values: "x86_64", "aarch64", "x86".
        // See: https://doc.rust-lang.org/std/env/consts/constant.ARCH.html
        arch: std::env::consts::ARCH.to_string(),
        // Raw OS type for debugging/display — same constant but unmapped
        os_type: std::env::consts::OS.to_string(),
    }
}

/// Returns the absolute path to the application's data directory.
///
/// **Frontend caller:** `getAppDataDir()` in `src/lib/tauri-commands.ts`
///
/// This is the self-contained directory where Python, GAMDL, tools,
/// and settings are stored. The path varies by platform:
/// - macOS: `~/Library/Application Support/io.github.meedyadl/`
/// - Windows: `%APPDATA%/io.github.meedyadl/`
/// - Linux: `~/.local/share/io.github.meedyadl/`
///
/// The frontend uses this path for:
/// - Displaying the data directory location in the settings page
/// - Showing the user where their downloads and tools are stored
///
/// # Arguments
/// * `app` - Tauri `AppHandle`, needed for Tauri's path resolver which
///   provides the platform-appropriate app data directory.
///
/// # Errors
///
/// Returns `Err(String)` if the resolved app data path contains non-UTF-8
/// characters (extremely rare on modern systems).
///
/// # Returns
/// * `Ok(String)` - The absolute path as a UTF-8 string.
/// * `Err(String)` - If the path contains non-UTF-8 characters
///   (extremely rare on modern systems).
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri IPC commands require owned parameters
pub fn get_app_data_dir(app: AppHandle) -> Result<String, String> {
    // Resolve the app data directory using our platform utility.
    // This wraps Tauri's `app.path().app_data_dir()` with fallback logic.
    let dir = platform::get_app_data_dir(&app);

    // Convert PathBuf to String for JSON serialization.
    // PathBuf::to_str() returns None if the path is not valid UTF-8,
    // which is theoretically possible on some Unix systems but extremely rare.
    dir.to_str()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| "Failed to convert app data path to string".to_string())
}

/// Platform configuration entry from engines.toml for the frontend.
///
/// Contains only the fields the frontend needs for URL detection and
/// icon rendering — not the full engine/install configuration.
#[derive(serde::Serialize)]
pub struct FrontendPlatformConfig {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub url_patterns: Vec<String>,
    pub enabled: bool,
}

/// Returns the platform configuration from engines.toml.
///
/// Parses the compiled-in engines.toml and returns all platform entries
/// so the frontend can detect platforms from URLs and render icons
/// without hardcoding the list.
///
/// **Frontend caller:** `getPlatformConfig()` in `src/lib/tauri-commands.ts`
#[tauri::command]
pub fn get_platform_config() -> Vec<FrontendPlatformConfig> {
    let toml_str = include_str!("../../engines.toml");
    let parsed: toml::Value = match toml::from_str(toml_str) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Failed to parse engines.toml for platform config: {e}");
            return vec![];
        }
    };

    let Some(platforms) = parsed.get("platforms").and_then(|p| p.as_table()) else {
        return vec![];
    };

    platforms
        .iter()
        .filter_map(|(id, def)| {
            let name = def.get("name")?.as_str()?.to_string();
            let enabled = def.get("enabled")?.as_bool().unwrap_or(false);
            let icon = def
                .get("icon")
                .and_then(|v| v.as_str())
                .map(std::string::ToString::to_string);
            let url_patterns = def
                .get("url_patterns")?
                .as_array()?
                .iter()
                .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                .collect();

            Some(FrontendPlatformConfig {
                id: id.clone(),
                name,
                icon,
                url_patterns,
                enabled,
            })
        })
        .collect()
}

/// Detects which media service a URL belongs to.
///
/// Uses `MediaServiceId::from_url()` to match the URL against known service
/// domains. Returns the service ID string if recognised, or `None` if the
/// URL doesn't match any supported service.
///
/// **Frontend caller:** `detectService(url)` in `src/lib/tauri-commands.ts`
#[tauri::command]
pub fn detect_service(url: String) -> Option<crate::models::media_service::MediaServiceId> {
    crate::models::media_service::MediaServiceId::from_url(&url)
}
