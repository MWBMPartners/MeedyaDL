// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Cookie management IPC commands.
// =================================
//
// Provides Tauri IPC commands for automated browser cookie extraction.
// These commands enable the frontend to:
//   1. Detect which browsers are installed on the user's system
//   2. Import Apple Music cookies from a selected browser
//   3. Check macOS Full Disk Access status (needed for Safari)
//
// All heavy lifting is delegated to `services::cookie_service`. These
// command handlers are thin wrappers that extract arguments and call
// the service layer, following the project's architectural pattern.
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command                   | TypeScript Function              |
// |--------------------------------|----------------------------------|
// | detect_browsers                | detectBrowsers()                 |
// | import_cookies_from_browser    | importCookiesFromBrowser(id)     |
// | check_full_disk_access         | checkFullDiskAccess()            |
//
// ## References
//
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - Cookie service: src-tauri/src/services/cookie_service.rs

use tauri::AppHandle;

use crate::services::cookie_service;
use crate::services::cookie_service::{CookieImportResult, DetectedBrowser};

/// Detects which browsers are installed on the user's system.
///
/// **Frontend caller:** `detectBrowsers()` in `src/lib/tauri-commands.ts`
///
/// Performs lightweight filesystem checks to determine which browsers have
/// profile directories on disk. No cookies are read during detection, and
/// no OS permission prompts are triggered.
///
/// # Returns
/// * `Ok(Vec<DetectedBrowser>)` - List of installed browsers with metadata
///   (id, name, icon hint, and whether Full Disk Access is required).
/// * `Err(String)` - Unlikely to fail, but wrapped in Result for consistency.
#[tauri::command]
pub async fn detect_browsers() -> Result<Vec<DetectedBrowser>, String> {
    Ok(cookie_service::detect_browsers())
}

/// Imports Apple Music cookies from the specified browser.
///
/// **Frontend caller:** `importCookiesFromBrowser(browserId)` in `src/lib/tauri-commands.ts`
///
/// This command:
/// 1. Extracts cookies matching Apple Music domains from the browser
/// 2. Converts them to Netscape format
/// 3. Saves the result to `{app_data}/cookies.txt`
/// 4. Updates `settings.cookies_path` to point to the new file
///
/// ## Platform Behaviour
///
/// - **macOS (Chromium browsers)**: May trigger a macOS Keychain access prompt
///   asking for the user's password. This is expected and handled by the OS.
/// - **macOS (Safari)**: Requires Full Disk Access. The frontend should call
///   `check_full_disk_access` first and guide the user to grant FDA if needed.
/// - **Windows**: Uses DPAPI for transparent cookie decryption.
/// - **Linux**: Uses D-Bus Secret Service for Chromium key decryption.
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving paths and updating settings
/// * `browser_id` - Machine-readable browser identifier (e.g., "chrome", "firefox")
///
/// # Returns
/// * `Ok(CookieImportResult)` - Result with cookie counts, warnings, and saved file path
/// * `Err(String)` - Error message if extraction failed (browser not found, permission denied, etc.)
#[tauri::command]
pub async fn import_cookies_from_browser(
    app: AppHandle,
    browser_id: String,
) -> Result<CookieImportResult, String> {
    log::info!("Importing cookies from browser: {}", browser_id);
    cookie_service::extract_and_save(&app, &browser_id)
}

/// Checks whether the application has macOS Full Disk Access.
///
/// **Frontend caller:** `checkFullDiskAccess()` in `src/lib/tauri-commands.ts`
///
/// Full Disk Access (FDA) is required on macOS to read Safari's cookie
/// database. The frontend calls this before attempting a Safari cookie
/// import, and shows instructions to grant FDA if it's not enabled.
///
/// On non-macOS platforms, this always returns `true` since FDA is a
/// macOS-only concept.
///
/// # Returns
/// * `Ok(true)` - FDA is granted (or not required on this platform)
/// * `Ok(false)` - FDA is not granted (macOS only)
#[tauri::command]
pub async fn check_full_disk_access() -> Result<bool, String> {
    Ok(cookie_service::check_full_disk_access())
}
