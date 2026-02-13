// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Embedded Apple Music login window IPC commands.
// =================================================
//
// Provides Tauri IPC commands for managing the embedded Apple Music login
// browser window. These commands enable the frontend to:
//   1. Open an embedded browser window to https://music.apple.com
//   2. Manually trigger cookie extraction after the user signs in
//   3. Close the login window
//
// All heavy lifting is delegated to `services::login_window_service`. These
// command handlers are thin wrappers that extract arguments and call the
// service layer, following the project's architectural pattern.
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command              | TypeScript Function           |
// |---------------------------|-------------------------------|
// | open_apple_login          | openAppleLogin()              |
// | extract_login_cookies     | extractLoginCookies()         |
// | close_apple_login         | closeAppleLogin()             |
//
// ## Events Emitted (by the service layer, not directly by commands)
//
// | Event Name                | Payload               | When                              |
// |---------------------------|-----------------------|-----------------------------------|
// | login-cookies-extracted   | CookieImportResult    | Auth cookies detected and saved   |
// | login-window-closed       | ()                    | Login window closed for any reason|
//
// ## References
//
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - Login window service: src-tauri/src/services/login_window_service.rs

use tauri::AppHandle;

use crate::services::cookie_service::CookieImportResult;
use crate::services::login_window_service;

/// Opens an embedded browser window for Apple Music login.
///
/// **Frontend caller:** `openAppleLogin()` in `src/lib/tauri-commands.ts`
///
/// Creates a secondary webview window that loads `https://music.apple.com`.
/// The user can sign in with their Apple ID. The window monitors page loads
/// and automatically detects when authentication cookies are set, extracting
/// and saving them for GAMDL.
///
/// If a login window is already open, the existing window is focused instead
/// of creating a duplicate.
///
/// # Returns
/// * `Ok(())` - Window opened (or existing window focused).
/// * `Err(String)` - Window creation failed.
#[tauri::command]
pub async fn open_apple_login(app: AppHandle) -> Result<(), String> {
    log::info!("Opening Apple Music login window");
    login_window_service::open_login_window(&app)
}

/// Manually triggers cookie extraction from the login browser window.
///
/// **Frontend caller:** `extractLoginCookies()` in `src/lib/tauri-commands.ts`
///
/// This is the manual fallback for when auto-detection (via `on_page_load`)
/// doesn't fire. The user clicks "I've signed in" in the frontend, which
/// calls this command to force a cookie check and extraction.
///
/// Reads all cookies from the login webview, filters to Apple Music domains,
/// converts to Netscape format, saves to `{app_data}/cookies.txt`, and
/// updates `settings.cookies_path`.
///
/// # Returns
/// * `Ok(CookieImportResult)` - Extraction result with cookie counts and path.
/// * `Err(String)` - If the login window isn't open or extraction failed.
#[tauri::command]
pub async fn extract_login_cookies(
    app: AppHandle,
) -> Result<CookieImportResult, String> {
    log::info!("Manually extracting cookies from login window");
    login_window_service::extract_login_cookies(&app).await
}

/// Closes the Apple Music login browser window.
///
/// **Frontend caller:** `closeAppleLogin()` in `src/lib/tauri-commands.ts`
///
/// Called when the user clicks "Cancel" in the frontend. If the login window
/// is not open, this is a no-op.
///
/// The `login-window-closed` event will be emitted by the window's destroy
/// handler when the window actually closes.
///
/// # Returns
/// * `Ok(())` - Window closed (or was already closed).
#[tauri::command]
pub async fn close_apple_login(app: AppHandle) -> Result<(), String> {
    log::info!("Closing Apple Music login window");
    login_window_service::close_login_window(&app);
    Ok(())
}
