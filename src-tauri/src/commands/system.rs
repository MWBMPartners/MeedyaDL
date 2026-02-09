// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// System information IPC commands.
// Provides the frontend with platform detection and directory path resolution
// so the UI can adapt its theme and display platform-appropriate content.

use serde::Serialize;
use tauri::AppHandle;

use crate::utils::platform;

/// Platform information returned to the frontend for theme selection
/// and platform-specific UI adaptations.
#[derive(Debug, Clone, Serialize)]
pub struct PlatformInfo {
    /// Operating system name: "macos", "windows", or "linux"
    pub platform: String,
    /// CPU architecture: "aarch64", "x86_64", or "x86"
    pub arch: String,
    /// Rust target OS constant for reference
    pub os_type: String,
}

/// Returns information about the current platform and architecture.
///
/// The frontend uses this to:
/// - Load the correct platform CSS theme (macOS Liquid Glass, Windows Mica, etc.)
/// - Show platform-appropriate instructions (e.g., different cookie export steps)
/// - Display the correct architecture in the About page
#[tauri::command]
pub fn get_platform_info() -> PlatformInfo {
    PlatformInfo {
        // Map Rust OS constants to our frontend platform identifiers
        platform: match std::env::consts::OS {
            "macos" => "macos".to_string(),
            "windows" => "windows".to_string(),
            _ => "linux".to_string(), // Linux, FreeBSD, etc. all use the Linux theme
        },
        // CPU architecture identifier
        arch: std::env::consts::ARCH.to_string(),
        // Raw OS type for debugging/display
        os_type: std::env::consts::OS.to_string(),
    }
}

/// Returns the absolute path to the application's data directory.
///
/// This is the self-contained directory where Python, GAMDL, tools,
/// and settings are stored. The path varies by platform:
/// - macOS: ~/Library/Application Support/com.mwbmpartners.gamdl-gui/
/// - Windows: %APPDATA%/com.mwbmpartners.gamdl-gui/
/// - Linux: ~/.local/share/com.mwbmpartners.gamdl-gui/
#[tauri::command]
pub fn get_app_data_dir(app: AppHandle) -> Result<String, String> {
    // Get the app data directory from our platform utilities
    let dir = platform::get_app_data_dir(&app);

    // Convert the PathBuf to a string for JSON serialization
    dir.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Failed to convert app data path to string".to_string())
}
