// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Embedded Apple Music login window service.
// =============================================
//
// This service manages a secondary webview window where users can sign in to
// Apple Music directly within the application. It addresses the scenario where
// a user has no existing browser cookies to auto-import -- they can log in via
// the embedded browser instead, and their authentication cookies are captured
// automatically.
//
// ## How It Works
//
// 1. **Window creation**: Opens a secondary `WebviewWindow` via Tauri's
//    `WebviewWindowBuilder`, loading `https://music.apple.com` as an external
//    URL. The window is separate from the main app window and renders Apple's
//    actual website.
//
// 2. **Login detection**: An `on_page_load` callback fires after each page
//    navigation. When the page finishes loading on `music.apple.com`, the
//    service asynchronously checks for the `media-user-token` cookie, which
//    is the primary indicator that the user has successfully authenticated.
//
// 3. **Cookie extraction**: Uses Tauri 2.10+'s native `cookies_for_url()` API
//    to read ALL cookies from the webview's cookie store, including HttpOnly
//    and Secure cookies that JavaScript's `document.cookie` cannot access.
//
// 4. **Netscape conversion**: Converts the extracted cookies to Netscape
//    format (the same tab-separated format used by curl, wget, yt-dlp, and
//    GAMDL), filters to Apple Music domains, and writes to
//    `{app_data}/cookies.txt`.
//
// 5. **Settings sync**: Updates `settings.cookies_path` to point to the
//    newly written file, so GAMDL can immediately use the cookies.
//
// 6. **Event emission**: Emits `login-cookies-extracted` to the main window
//    when cookies are successfully captured, and `login-window-closed` when
//    the login window is closed (whether programmatically or by the user).
//
// ## Cross-Platform Webview Engines
//
// The embedded browser uses each platform's native webview:
//   - **macOS**: WKWebView (Safari engine)
//   - **Windows**: WebView2 (Chromium-based, Edge engine)
//   - **Linux**: webkit2gtk (WebKitGTK engine)
//
// ## Important: Windows Deadlock Avoidance
//
// On Windows, calling `cookies_for_url()` from a synchronous context (such as
// an `on_page_load` callback) causes a deadlock. All cookie extraction MUST be
// dispatched to an async task via `tauri::async_runtime::spawn()`.
//
// ## References
//
// - Tauri WebviewWindowBuilder: https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html
// - Tauri cookies_for_url: https://docs.rs/tauri/latest/tauri/webview/struct.Webview.html#method.cookies_for_url
// - Tauri Cookie struct: https://docs.rs/tauri/latest/tauri/webview/struct.Cookie.html
// - cookie crate: https://docs.rs/cookie/0.18/cookie/
// - Netscape cookie format: https://curl.se/docs/http-cookies.html
// - GAMDL cookie requirements: https://github.com/glomatico/gamdl#cookies

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use url::Url;

use crate::services::config_service;
use crate::services::cookie_service::CookieImportResult;
use crate::utils::platform;

// ============================================================
// Constants
// ============================================================

/// The label used for the login webview window.
/// Tauri identifies windows by label, so this must be unique across the app.
const LOGIN_WINDOW_LABEL: &str = "apple-login";

/// The URL that the login window navigates to on open.
const APPLE_MUSIC_URL: &str = "https://music.apple.com";

/// The cookie name that indicates a successful Apple Music login.
/// This is Apple's primary authentication token for Apple Music.
const AUTH_COOKIE_NAME: &str = "media-user-token";

/// Apple Music domains to filter cookies for (same as cookie_service).
/// Only cookies matching these domains are saved to the cookies file.
const APPLE_MUSIC_DOMAINS: &[&str] = &["apple.com", "mzstatic.com"];

/// Delay (in milliseconds) before auto-closing the login window after
/// successful cookie extraction. Gives the user a moment to see the
/// page has loaded before the window disappears.
const AUTO_CLOSE_DELAY_MS: u64 = 1000;

// ============================================================
// Public Functions
// ============================================================

/// Opens an embedded browser window for Apple Music login.
///
/// Creates a secondary `WebviewWindow` that loads `https://music.apple.com`.
/// The user can sign in with their Apple ID in this window. An `on_page_load`
/// callback monitors page navigations and automatically extracts cookies when
/// a successful login is detected (via the `media-user-token` cookie).
///
/// If a login window is already open, this function focuses the existing
/// window instead of creating a duplicate.
///
/// ## Events Emitted
///
/// - `login-cookies-extracted` (CookieImportResult): Emitted to the main
///   window when authentication cookies are successfully extracted and saved.
/// - `login-window-closed` (unit): Emitted to the main window when the login
///   window is closed, whether by the user, programmatically, or after auto-
///   detection.
///
/// # Arguments
/// * `app` - Tauri AppHandle for window creation and event emission.
///
/// # Returns
/// * `Ok(())` - Window was created (or existing window was focused).
/// * `Err(String)` - Window creation failed.
pub fn open_login_window(app: &AppHandle) -> Result<(), String> {
    // If a login window already exists, focus it instead of creating a duplicate.
    // This prevents multiple login windows from being open simultaneously.
    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        existing
            .set_focus()
            .map_err(|e| format!("Failed to focus login window: {}", e))?;
        return Ok(());
    }

    // Parse the Apple Music URL for the WebviewUrl::External variant.
    // This tells Tauri to load an external website (not the app's own frontend).
    let url = WebviewUrl::External(
        APPLE_MUSIC_URL
            .parse()
            .map_err(|e| format!("Failed to parse Apple Music URL: {}", e))?,
    );

    // Clone the app handle for use inside the on_page_load callback.
    // The callback outlives this function, so it needs its own owned handle.
    let app_for_page_load = app.clone();

    // Build and create the login window using Tauri's WebviewWindowBuilder.
    // The window opens to music.apple.com with a reasonable size for a login flow.
    let window = WebviewWindowBuilder::new(app, LOGIN_WINDOW_LABEL, url)
        // Window title shown in the title bar / taskbar
        .title("Sign in to Apple Music")
        // Size the window large enough for Apple's login pages to render properly
        .inner_size(1000.0, 700.0)
        // Center the window on the primary monitor
        .center()
        // Allow resizing in case the login page needs more space
        .resizable(true)
        // Register the on_page_load callback to detect successful logins.
        // This fires on every page navigation within the webview.
        .on_page_load(move |_webview, payload| {
            // Only check cookies after the page has fully loaded (not on "Started").
            // The Finished event fires when the DOM is ready and initial resources
            // are loaded, which is when cookies from the login flow will be set.
            if let tauri::webview::PageLoadEvent::Finished = payload.event() {
                let url_str = payload.url().to_string();

                // Only check for cookies when we're on Apple Music's main site.
                // During login, Apple redirects through idmsa.apple.com and other
                // auth domains -- we only want to check on the final destination.
                if url_str.contains("music.apple.com") {
                    // Clone the app handle for the async task.
                    // IMPORTANT: All cookie extraction MUST happen in an async task
                    // to avoid deadlocking on Windows (WebView2 limitation).
                    let app_handle = app_for_page_load.clone();

                    // Spawn an async task to check cookies without blocking the UI.
                    tauri::async_runtime::spawn(async move {
                        match check_for_auth_cookies(&app_handle).await {
                            Ok(Some(result)) => {
                                // Authentication cookies were found and saved.
                                // Emit the result to the main window so the frontend
                                // can update its state.
                                if let Some(main_window) =
                                    app_handle.get_webview_window("main")
                                {
                                    let _ = main_window
                                        .emit("login-cookies-extracted", &result);
                                }

                                // Wait briefly before closing the login window,
                                // giving the user a moment to see the logged-in page.
                                tokio::time::sleep(std::time::Duration::from_millis(
                                    AUTO_CLOSE_DELAY_MS,
                                ))
                                .await;

                                // Auto-close the login window after successful extraction
                                close_login_window(&app_handle);
                            }
                            Ok(None) => {
                                // No auth cookies yet -- user hasn't completed login.
                                // This is normal during the login flow.
                                log::debug!(
                                    "No auth cookies found yet on page: {}",
                                    url_str
                                );
                            }
                            Err(e) => {
                                // Cookie check failed -- log but don't surface to user.
                                // The manual "I've signed in" button provides a fallback.
                                log::warn!(
                                    "Failed to check auth cookies: {}",
                                    e
                                );
                            }
                        }
                    });
                }
            }
        })
        .build()
        .map_err(|e| format!("Failed to create login window: {}", e))?;

    // Register a window event handler to detect when the login window is closed
    // (either by the user clicking X, or programmatically via close_login_window).
    // This emits the "login-window-closed" event so the frontend can reset its state.
    let app_for_close = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            log::info!("Login window closed");
            // Emit the close event to all listeners (the main window's frontend).
            let _ = app_for_close.emit("login-window-closed", ());
        }
    });

    log::info!("Login window opened to {}", APPLE_MUSIC_URL);
    Ok(())
}

/// Extracts cookies from the login window and saves them for GAMDL.
///
/// This is the manual extraction path, called when the user clicks
/// "I've signed in" in the frontend. It performs the same extraction as the
/// auto-detection in `on_page_load`, but is triggered explicitly by the user.
///
/// The function reads all cookies from the webview's cookie store for the
/// Apple Music URL, filters to Apple Music domains, converts to Netscape
/// format, writes to `{app_data}/cookies.txt`, and updates settings.
///
/// # Arguments
/// * `app` - Tauri AppHandle for webview access and settings update.
///
/// # Returns
/// * `Ok(CookieImportResult)` - Extraction result with cookie counts and path.
/// * `Err(String)` - If the login window isn't open or extraction failed.
pub async fn extract_login_cookies(
    app: &AppHandle,
) -> Result<CookieImportResult, String> {
    // Get the login window -- it must be open for extraction to work.
    let window = app
        .get_webview_window(LOGIN_WINDOW_LABEL)
        .ok_or_else(|| "Login window is not open. Please open it first.".to_string())?;

    // Parse the URL for cookie extraction.
    // cookies_for_url() requires a url::Url instance.
    let url = Url::parse(APPLE_MUSIC_URL)
        .map_err(|e| format!("Failed to parse URL: {}", e))?;

    // Extract all cookies from the webview's cookie store for this URL.
    // This returns ALL cookies including HttpOnly and Secure ones.
    // NOTE: cookies_for_url() is synchronous, but should be called from
    // an async context on Windows to avoid blocking the main thread.
    let cookies = window
        .cookies_for_url(url)
        .map_err(|e| format!("Failed to extract cookies from webview: {}", e))?;

    log::info!(
        "Extracted {} total cookies from login webview",
        cookies.len()
    );

    // Filter, convert, save, and return the result.
    save_cookies_from_webview(app, &cookies)
}

/// Closes the Apple Music login window if it is open.
///
/// This is called when the user clicks "Cancel" in the frontend, or after
/// successful cookie auto-detection. If the window is already closed, this
/// is a no-op (returns Ok).
///
/// The `on_window_event` handler registered during window creation will
/// emit the `login-window-closed` event when the window is destroyed.
///
/// # Arguments
/// * `app` - Tauri AppHandle for window access.
pub fn close_login_window(app: &AppHandle) {
    // Attempt to close the login window. If it doesn't exist, silently succeed.
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW_LABEL) {
        if let Err(e) = window.close() {
            log::warn!("Failed to close login window: {}", e);
        }
    }
}

// ============================================================
// Private Functions
// ============================================================

/// Checks whether authentication cookies exist in the login webview.
///
/// Called from the `on_page_load` callback after each page navigation on
/// `music.apple.com`. Looks specifically for the `media-user-token` cookie,
/// which indicates a successful Apple Music login.
///
/// If the auth cookie is found, performs the full extraction and save flow,
/// returning the `CookieImportResult`. If not found (user hasn't logged in
/// yet), returns `Ok(None)`.
///
/// # Arguments
/// * `app` - Tauri AppHandle for webview access.
///
/// # Returns
/// * `Ok(Some(result))` - Auth cookies found, extracted, and saved.
/// * `Ok(None)` - No auth cookies found yet (login not complete).
/// * `Err(String)` - Extraction failed.
async fn check_for_auth_cookies(
    app: &AppHandle,
) -> Result<Option<CookieImportResult>, String> {
    // Get the login window.
    let window = match app.get_webview_window(LOGIN_WINDOW_LABEL) {
        Some(w) => w,
        None => return Ok(None), // Window was closed; nothing to check
    };

    // Parse the URL for cookie extraction.
    let url = Url::parse(APPLE_MUSIC_URL)
        .map_err(|e| format!("Failed to parse URL: {}", e))?;

    // Extract cookies from the webview's cookie store.
    let cookies = window
        .cookies_for_url(url)
        .map_err(|e| format!("Failed to read cookies: {}", e))?;

    // Check if the media-user-token cookie exists.
    // This is the primary authentication indicator for Apple Music.
    let has_auth_token = cookies.iter().any(|c| c.name() == AUTH_COOKIE_NAME);

    if !has_auth_token {
        // User hasn't completed login yet -- this is expected during
        // the multi-step auth flow (Apple ID login, 2FA, etc.).
        return Ok(None);
    }

    log::info!(
        "Found {} cookie among {} total cookies",
        AUTH_COOKIE_NAME,
        cookies.len()
    );

    // Auth cookie found -- perform the full extraction and save.
    let result = save_cookies_from_webview(app, &cookies)?;
    Ok(Some(result))
}

/// Filters, converts, and saves cookies from the login webview.
///
/// This is the shared save path used by both auto-detection (`check_for_auth_cookies`)
/// and manual extraction (`extract_login_cookies`). It:
/// 1. Filters cookies to Apple Music domains only
/// 2. Converts to Netscape cookie file format
/// 3. Writes to `{app_data}/cookies.txt`
/// 4. Updates `settings.cookies_path`
/// 5. Returns a `CookieImportResult`
///
/// # Arguments
/// * `app` - Tauri AppHandle for file paths and settings.
/// * `cookies` - All cookies extracted from the webview.
///
/// # Returns
/// * `Ok(CookieImportResult)` - Save result with cookie counts.
/// * `Err(String)` - If file write or settings update failed.
fn save_cookies_from_webview(
    app: &AppHandle,
    cookies: &[cookie::Cookie<'static>],
) -> Result<CookieImportResult, String> {
    // Filter cookies to Apple Music domains only.
    // We don't want to save unrelated cookies from Apple's auth flow
    // (e.g., idmsa.apple.com session cookies that aren't needed by GAMDL).
    let apple_cookies: Vec<&cookie::Cookie> = cookies
        .iter()
        .filter(|c| is_apple_music_domain(c.domain().unwrap_or("")))
        .collect();

    let apple_music_count = apple_cookies.len();
    let total_count = cookies.len();

    log::info!(
        "Filtered to {} Apple Music cookies out of {} total",
        apple_music_count,
        total_count
    );

    // Convert filtered cookies to Netscape format.
    let netscape_content = webview_cookies_to_netscape(&apple_cookies);

    // Write the cookies file to the app data directory.
    let cookies_path = platform::get_app_data_dir(app).join("cookies.txt");

    // Ensure the parent directory exists (first-run safety).
    if let Some(parent) = cookies_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create cookies directory: {}", e))?;
    }

    std::fs::write(&cookies_path, &netscape_content)
        .map_err(|e| format!("Failed to write cookies file: {}", e))?;

    // Convert path to string for the result and settings.
    let cookies_path_str = cookies_path
        .to_str()
        .ok_or_else(|| "Failed to convert cookies path to string".to_string())?
        .to_string();

    log::info!(
        "Saved {} Apple Music cookies to {}",
        apple_music_count,
        cookies_path_str
    );

    // Update application settings to point to the new cookies file.
    if let Ok(mut settings) = config_service::load_settings(app) {
        settings.cookies_path = Some(cookies_path_str.clone());
        if let Err(e) = config_service::save_settings(app, &settings) {
            log::warn!("Failed to update settings with new cookies path: {}", e);
        }
    }

    // Build warnings based on cookie analysis.
    let mut warnings = Vec::new();
    if apple_music_count == 0 {
        warnings.push(
            "No Apple Music cookies found. Make sure you completed the sign-in process."
                .to_string(),
        );
    }

    // Check for the auth token specifically.
    let has_auth_token = apple_cookies
        .iter()
        .any(|c| c.name() == AUTH_COOKIE_NAME);
    if apple_music_count > 0 && !has_auth_token {
        warnings.push(format!(
            "The {} cookie was not found. Authentication may not work.",
            AUTH_COOKIE_NAME
        ));
    }

    // Check cookie expiry (if expiry info is available).
    let now = chrono::Utc::now().timestamp();
    let mut has_expired = false;
    for cookie in &apple_cookies {
        if let Some(cookie::Expiration::DateTime(dt)) = cookie.expires() {
            let expires_ts = dt.unix_timestamp();
            if expires_ts > 0 && expires_ts < now {
                has_expired = true;
            } else if expires_ts > 0 {
                let days_until = (expires_ts - now) / 86400;
                if days_until < 7 {
                    warnings.push(format!(
                        "Apple Music cookies expire in {} day(s)",
                        days_until
                    ));
                    break; // Only report the first expiry warning
                }
            }
        }
    }

    if has_expired {
        warnings.push(
            "Some Apple Music cookies have expired -- you may need to sign in again."
                .to_string(),
        );
    }

    let success = apple_music_count > 0 && !has_expired;

    Ok(CookieImportResult {
        success,
        cookie_count: total_count,
        apple_music_cookies: apple_music_count,
        warnings,
        path: cookies_path_str,
    })
}

/// Checks whether a cookie domain belongs to Apple Music.
///
/// Matches both exact domains and subdomains. For example, both
/// `.apple.com` and `music.apple.com` match against `apple.com`.
///
/// # Arguments
/// * `domain` - The cookie's domain string (may have a leading dot).
///
/// # Returns
/// `true` if the domain matches any of the Apple Music domains.
fn is_apple_music_domain(domain: &str) -> bool {
    // Strip the leading dot that cookies often have (e.g., ".apple.com")
    let clean_domain = domain.trim_start_matches('.');

    APPLE_MUSIC_DOMAINS
        .iter()
        .any(|&d| clean_domain == d || clean_domain.ends_with(&format!(".{}", d)))
}

/// Converts a slice of webview cookies to Netscape cookie file format.
///
/// Produces the same output format as `cookie_service::cookies_to_netscape()`,
/// but accepts Tauri/cookie-crate `Cookie` structs instead of rookie's Cookie.
///
/// The Netscape format is tab-separated with 7 fields per line:
///   `domain \t subdomain_flag \t path \t secure \t expires \t name \t value`
///
/// # Arguments
/// * `cookies` - Slice of cookie references from the webview.
///
/// # Returns
/// The complete Netscape cookie file content as a string.
fn webview_cookies_to_netscape(cookies: &[&cookie::Cookie]) -> String {
    let mut lines = Vec::new();

    // Standard Netscape cookie file header (same as cookie_service)
    lines.push("# Netscape HTTP Cookie File".to_string());
    lines.push("# Extracted by gamdl-GUI from embedded browser login".to_string());
    lines.push(String::new()); // Empty line after header

    for cookie in cookies {
        lines.push(format_netscape_line(cookie));
    }

    // Trailing newline for compatibility
    lines.join("\n") + "\n"
}

/// Formats a single cookie as a Netscape cookie file line.
///
/// Extracts all fields from the cookie::Cookie struct and formats them
/// as a tab-separated line in the Netscape/Mozilla cookie format.
///
/// ## Field Mapping
///
/// | # | Field           | Source                     | Notes                              |
/// |---|-----------------|----------------------------|------------------------------------|
/// | 1 | domain          | `cookie.domain()`          | Leading dot added if missing       |
/// | 2 | subdomain flag  | (always TRUE)              | Enables subdomain matching         |
/// | 3 | path            | `cookie.path()`            | Defaults to "/" if missing          |
/// | 4 | secure flag     | `cookie.secure()`          | "TRUE" or "FALSE"                  |
/// | 5 | expires         | `cookie.expires()`         | Unix timestamp, 0 for session      |
/// | 6 | name            | `cookie.name()`            | Cookie name                        |
/// | 7 | value           | `cookie.value()`           | Cookie value                       |
///
/// # Arguments
/// * `cookie` - A reference to a cookie::Cookie struct.
///
/// # Returns
/// A single tab-separated line in Netscape format (without trailing newline).
fn format_netscape_line(cookie: &cookie::Cookie) -> String {
    // Field 1: domain (add leading dot for subdomain matching if not present)
    let raw_domain = cookie.domain().unwrap_or("unknown");
    let domain = if raw_domain.starts_with('.') {
        raw_domain.to_string()
    } else {
        format!(".{}", raw_domain)
    };

    // Field 2: subdomain flag (always TRUE for dotted domains)
    let subdomain_flag = "TRUE";

    // Field 3: path (default to "/" if not set)
    let path = cookie.path().unwrap_or("/");

    // Field 4: secure flag
    let secure = if cookie.secure().unwrap_or(false) {
        "TRUE"
    } else {
        "FALSE"
    };

    // Field 5: expiry as Unix timestamp (0 for session cookies)
    let expires = match cookie.expires() {
        Some(cookie::Expiration::DateTime(dt)) => dt.unix_timestamp(),
        _ => 0, // Session cookies or missing expiry → 0
    };

    // Field 6: cookie name
    let name = cookie.name();

    // Field 7: cookie value
    let value = cookie.value();

    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        domain, subdomain_flag, path, secure, expires, name, value
    )
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // is_apple_music_domain: domain matching
    // ----------------------------------------------------------

    /// Verifies that bare apple.com domain matches.
    #[test]
    fn apple_domain_matches() {
        assert!(is_apple_music_domain("apple.com"));
    }

    /// Verifies that dotted .apple.com domain matches.
    #[test]
    fn dotted_apple_domain_matches() {
        assert!(is_apple_music_domain(".apple.com"));
    }

    /// Verifies that subdomain music.apple.com matches.
    #[test]
    fn subdomain_apple_matches() {
        assert!(is_apple_music_domain("music.apple.com"));
    }

    /// Verifies that mzstatic.com matches.
    #[test]
    fn mzstatic_domain_matches() {
        assert!(is_apple_music_domain("mzstatic.com"));
    }

    /// Verifies that dotted .mzstatic.com matches.
    #[test]
    fn dotted_mzstatic_matches() {
        assert!(is_apple_music_domain(".mzstatic.com"));
    }

    /// Verifies that subdomain is1-ssl.mzstatic.com matches.
    #[test]
    fn mzstatic_subdomain_matches() {
        assert!(is_apple_music_domain("is1-ssl.mzstatic.com"));
    }

    /// Verifies that unrelated domains do not match.
    #[test]
    fn unrelated_domain_does_not_match() {
        assert!(!is_apple_music_domain("google.com"));
        assert!(!is_apple_music_domain("fakeapple.com"));
        assert!(!is_apple_music_domain("notmzstatic.com"));
    }

    // ----------------------------------------------------------
    // format_netscape_line: cookie formatting
    // ----------------------------------------------------------

    /// Verifies basic cookie formatting with all fields present.
    #[test]
    fn formats_basic_cookie_correctly() {
        let cookie = cookie::Cookie::build(("test_name", "test_value"))
            .domain(".apple.com")
            .path("/")
            .secure(true)
            .build();

        let line = format_netscape_line(&cookie);

        // Domain should be .apple.com (already has dot)
        assert!(line.starts_with(".apple.com\t"));
        // Should contain the name and value at the end
        assert!(line.ends_with("test_name\ttest_value"));
        // Should have TRUE for secure
        assert!(line.contains("\tTRUE\t"));
        // Should have 7 tab-separated fields
        assert_eq!(line.split('\t').count(), 7);
    }

    /// Verifies that domains without a leading dot get one added.
    #[test]
    fn adds_leading_dot_to_domain() {
        let cookie = cookie::Cookie::build(("n", "v"))
            .domain("apple.com")
            .path("/")
            .build();

        let line = format_netscape_line(&cookie);
        assert!(line.starts_with(".apple.com\t"));
    }

    /// Verifies that domains already having a leading dot are preserved.
    #[test]
    fn preserves_existing_leading_dot() {
        let cookie = cookie::Cookie::build(("n", "v"))
            .domain(".apple.com")
            .path("/")
            .build();

        let line = format_netscape_line(&cookie);
        // Should NOT have double dot
        assert!(line.starts_with(".apple.com\t"));
        assert!(!line.starts_with(".."));
    }

    /// Verifies that insecure cookies have FALSE for the secure flag.
    #[test]
    fn insecure_cookie_has_false_flag() {
        let cookie = cookie::Cookie::build(("n", "v"))
            .domain(".apple.com")
            .path("/")
            .secure(false)
            .build();

        let line = format_netscape_line(&cookie);
        // The 4th field (secure) should be FALSE
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields[3], "FALSE");
    }

    /// Verifies that session cookies (no expiry) get timestamp 0.
    #[test]
    fn session_cookie_has_zero_expiry() {
        let cookie = cookie::Cookie::build(("session", "val"))
            .domain(".apple.com")
            .path("/")
            .build();

        let line = format_netscape_line(&cookie);
        // The 5th field (expires) should be 0
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields[4], "0");
    }

    /// Verifies that the path defaults to "/" when not set on the cookie.
    #[test]
    fn default_path_is_slash() {
        let cookie = cookie::Cookie::build(("n", "v"))
            .domain(".apple.com")
            .build();

        let line = format_netscape_line(&cookie);
        // The 3rd field (path) should be /
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields[2], "/");
    }

    // ----------------------------------------------------------
    // webview_cookies_to_netscape: full file format
    // ----------------------------------------------------------

    /// Verifies that the output starts with the standard Netscape header.
    #[test]
    fn netscape_output_starts_with_header() {
        let cookies: Vec<&cookie::Cookie> = vec![];
        let output = webview_cookies_to_netscape(&cookies);
        assert!(output.starts_with("# Netscape HTTP Cookie File"));
    }

    /// Verifies that the output ends with a trailing newline.
    #[test]
    fn netscape_output_ends_with_newline() {
        let cookies: Vec<&cookie::Cookie> = vec![];
        let output = webview_cookies_to_netscape(&cookies);
        assert!(output.ends_with('\n'));
    }

    /// Verifies that multiple cookies are included in the output.
    #[test]
    fn netscape_output_includes_all_cookies() {
        let c1 = cookie::Cookie::build(("name1", "val1"))
            .domain(".apple.com")
            .path("/")
            .build();
        let c2 = cookie::Cookie::build(("name2", "val2"))
            .domain(".mzstatic.com")
            .path("/")
            .build();

        let cookies: Vec<&cookie::Cookie> = vec![&c1, &c2];
        let output = webview_cookies_to_netscape(&cookies);

        assert!(output.contains("name1\tval1"));
        assert!(output.contains("name2\tval2"));
    }
}
