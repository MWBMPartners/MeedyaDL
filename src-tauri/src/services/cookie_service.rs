// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Browser cookie extraction service.
// =====================================
//
// This service automates the extraction of Apple Music authentication cookies
// from the user's installed web browsers. It eliminates the manual process of
// installing a browser extension, navigating to music.apple.com, exporting
// cookies, and then selecting the file in the app.
//
// ## How It Works
//
// 1. **Browser detection**: Checks the filesystem for known browser profile
//    directories to determine which browsers are installed (no cookies are
//    read during this step).
//
// 2. **Cookie extraction**: Uses the `rookie` crate to read cookies from the
//    selected browser's local storage. Only cookies matching Apple Music
//    domains (apple.com, mzstatic.com) are extracted.
//
// 3. **Netscape format conversion**: Converts the extracted `rookie::enums::Cookie`
//    structs into Netscape cookie format lines (tab-separated, 7 fields per
//    line), which is the format GAMDL expects.
//
// 4. **File output**: Writes the converted cookies to `{app_data}/cookies.txt`
//    and updates the application settings to point to this file.
//
// ## Platform-Specific Behaviour
//
// - **macOS (Chromium browsers)**: The OS may prompt for Keychain access when
//   the rookie crate attempts to decrypt browser cookies. This is a standard
//   macOS security prompt -- no app-level code is needed to handle it.
//
// - **macOS (Safari)**: Requires Full Disk Access (FDA) for the app to read
//   Safari's cookie database. The `check_full_disk_access()` function tests
//   whether FDA is granted by attempting to read a protected file.
//
// - **Windows**: Chromium-based browsers use DPAPI for cookie encryption, which
//   the rookie crate handles transparently. No user interaction is needed.
//
// - **Linux**: Chromium-based browsers may use the GNOME Keyring or KWallet
//   for cookie encryption keys. The rookie crate accesses these via D-Bus.
//
// ## Privacy
//
// Only cookies for Apple Music domains are extracted. No other browsing data
// is read, and the extracted cookies are stored locally in the app data
// directory. The user explicitly initiates each import by clicking a button.
//
// ## References
//
// - rookie crate: https://docs.rs/rookie/
// - Netscape cookie format: https://curl.se/docs/http-cookies.html
// - GAMDL cookie requirements: https://github.com/glomatico/gamdl#cookies

use serde::Serialize;
use tauri::AppHandle;

use crate::services::config_service;
use crate::utils::platform;

// ============================================================
// Data Types
// ============================================================

/// Information about a detected browser installation.
///
/// Returned by `detect_browsers()` to list which browsers the user has
/// installed. The frontend displays these as options for auto-importing
/// cookies. No cookies are read during detection -- only filesystem
/// checks for known profile directories.
///
/// ## Fields
///
/// - `id`: Machine-readable browser identifier (e.g., "chrome", "firefox").
///   Used as the argument to `extract_and_save()` to specify which browser
///   to extract from.
/// - `name`: Human-readable browser name for UI display (e.g., "Google Chrome").
/// - `icon_hint`: Hint for the frontend to select the appropriate browser icon.
///   Matches the `id` field in most cases.
/// - `requires_fda`: `true` only for Safari on macOS, which requires Full Disk
///   Access to read its cookie database. The frontend uses this to show an FDA
///   instruction panel before attempting import.
#[derive(Debug, Clone, Serialize)]
pub struct DetectedBrowser {
    /// Machine-readable identifier (e.g., "chrome", "firefox", "safari")
    pub id: String,
    /// Human-readable display name (e.g., "Google Chrome")
    pub name: String,
    /// Icon hint for the frontend (typically matches `id`)
    pub icon_hint: String,
    /// Whether this browser requires macOS Full Disk Access to read cookies
    pub requires_fda: bool,
}

/// Result of an automated cookie import operation.
///
/// Returned by `extract_and_save()` after extracting cookies from a browser,
/// converting them to Netscape format, and writing to the app data directory.
/// The frontend uses this to display success/failure status and cookie counts.
#[derive(Debug, Clone, Serialize)]
pub struct CookieImportResult {
    /// Whether the import completed successfully with usable cookies
    pub success: bool,
    /// Total number of cookies extracted (across all matching domains)
    pub cookie_count: usize,
    /// Number of cookies specifically for Apple Music domains
    pub apple_music_cookies: usize,
    /// Warning messages (e.g., "Some cookies are expired")
    pub warnings: Vec<String>,
    /// Absolute path where the cookies file was saved
    pub path: String,
}

// ============================================================
// Browser Profile Directory Definitions
// ============================================================

/// Defines a browser that can be detected and extracted from.
/// Each entry specifies the browser metadata and the platform-specific
/// directories where its profile data is stored.
struct BrowserDef {
    /// Machine-readable identifier
    id: &'static str,
    /// Human-readable display name
    name: &'static str,
    /// Whether this browser requires macOS Full Disk Access
    requires_fda: bool,
    /// macOS profile directory (relative to ~/Library/Application Support/)
    #[cfg(target_os = "macos")]
    macos_path: Option<&'static str>,
    /// Windows profile directory (relative to %LOCALAPPDATA%/)
    #[cfg(target_os = "windows")]
    windows_path: Option<&'static str>,
    /// Linux profile directory (relative to ~/.config/)
    #[cfg(target_os = "linux")]
    linux_path: Option<&'static str>,
}

/// List of all supported browsers with their platform-specific profile paths.
/// These paths are used to detect which browsers are installed by checking
/// whether the profile directory exists on disk.
#[cfg(target_os = "macos")]
const BROWSER_DEFS: &[BrowserDef] = &[
    BrowserDef {
        id: "chrome",
        name: "Google Chrome",
        requires_fda: false,
        macos_path: Some("Google/Chrome"),
    },
    BrowserDef {
        id: "firefox",
        name: "Mozilla Firefox",
        requires_fda: false,
        macos_path: Some("Firefox"),
    },
    BrowserDef {
        id: "edge",
        name: "Microsoft Edge",
        requires_fda: false,
        macos_path: Some("Microsoft Edge"),
    },
    BrowserDef {
        id: "brave",
        name: "Brave Browser",
        requires_fda: false,
        macos_path: Some("BraveSoftware/Brave-Browser"),
    },
    BrowserDef {
        id: "safari",
        name: "Safari",
        requires_fda: true,
        macos_path: None, // Safari is always installed on macOS
    },
    BrowserDef {
        id: "chromium",
        name: "Chromium",
        requires_fda: false,
        macos_path: Some("Chromium"),
    },
    BrowserDef {
        id: "opera",
        name: "Opera",
        requires_fda: false,
        macos_path: Some("com.operasoftware.Opera"),
    },
    BrowserDef {
        id: "vivaldi",
        name: "Vivaldi",
        requires_fda: false,
        macos_path: Some("Vivaldi"),
    },
    BrowserDef {
        id: "arc",
        name: "Arc",
        requires_fda: false,
        macos_path: Some("Arc"),
    },
];

#[cfg(target_os = "windows")]
const BROWSER_DEFS: &[BrowserDef] = &[
    BrowserDef {
        id: "chrome",
        name: "Google Chrome",
        requires_fda: false,
        windows_path: Some("Google\\Chrome\\User Data"),
    },
    BrowserDef {
        id: "firefox",
        name: "Mozilla Firefox",
        requires_fda: false,
        windows_path: Some("Mozilla\\Firefox"),
    },
    BrowserDef {
        id: "edge",
        name: "Microsoft Edge",
        requires_fda: false,
        windows_path: Some("Microsoft\\Edge\\User Data"),
    },
    BrowserDef {
        id: "brave",
        name: "Brave Browser",
        requires_fda: false,
        windows_path: Some("BraveSoftware\\Brave-Browser\\User Data"),
    },
    BrowserDef {
        id: "chromium",
        name: "Chromium",
        requires_fda: false,
        windows_path: Some("Chromium\\User Data"),
    },
    BrowserDef {
        id: "opera",
        name: "Opera",
        requires_fda: false,
        windows_path: Some("Opera Software\\Opera Stable"),
    },
    BrowserDef {
        id: "vivaldi",
        name: "Vivaldi",
        requires_fda: false,
        windows_path: Some("Vivaldi\\User Data"),
    },
];

#[cfg(target_os = "linux")]
const BROWSER_DEFS: &[BrowserDef] = &[
    BrowserDef {
        id: "chrome",
        name: "Google Chrome",
        requires_fda: false,
        linux_path: Some("google-chrome"),
    },
    BrowserDef {
        id: "firefox",
        name: "Mozilla Firefox",
        requires_fda: false,
        linux_path: Some("firefox"), // Snap/Flatpak may differ but ~/.mozilla/firefox is common
    },
    BrowserDef {
        id: "edge",
        name: "Microsoft Edge",
        requires_fda: false,
        linux_path: Some("microsoft-edge"),
    },
    BrowserDef {
        id: "brave",
        name: "Brave Browser",
        requires_fda: false,
        linux_path: Some("BraveSoftware/Brave-Browser"),
    },
    BrowserDef {
        id: "chromium",
        name: "Chromium",
        requires_fda: false,
        linux_path: Some("chromium"),
    },
    BrowserDef {
        id: "opera",
        name: "Opera",
        requires_fda: false,
        linux_path: Some("opera"),
    },
    BrowserDef {
        id: "vivaldi",
        name: "Vivaldi",
        requires_fda: false,
        linux_path: Some("vivaldi"),
    },
];

// ============================================================
// Browser Detection
// ============================================================

/// Detects which browsers are installed on the user's system.
///
/// Checks for known browser profile directories on disk. This is a
/// lightweight filesystem check -- no cookies are read, no browser
/// processes are started, and no OS permission prompts are triggered.
///
/// Safari on macOS is always reported as installed (it ships with macOS)
/// but is flagged with `requires_fda: true` so the frontend can show
/// Full Disk Access instructions before attempting import.
///
/// # Returns
/// A vector of `DetectedBrowser` entries for each installed browser.
pub fn detect_browsers() -> Vec<DetectedBrowser> {
    let mut detected = Vec::new();

    for def in BROWSER_DEFS {
        if is_browser_installed(def) {
            detected.push(DetectedBrowser {
                id: def.id.to_string(),
                name: def.name.to_string(),
                icon_hint: def.id.to_string(),
                requires_fda: def.requires_fda,
            });
        }
    }

    detected
}

/// Checks whether a browser is installed by looking for its profile directory.
///
/// For Safari on macOS, always returns true (Safari ships with macOS).
/// For all other browsers, checks whether the platform-specific profile
/// directory exists on disk.
fn is_browser_installed(def: &BrowserDef) -> bool {
    // Safari on macOS is always installed
    #[cfg(target_os = "macos")]
    if def.id == "safari" {
        return true;
    }

    // Get the base directory for browser profiles
    let base_dir = get_browser_base_dir();
    let base_dir = match base_dir {
        Some(dir) => dir,
        None => return false,
    };

    // Check if the browser's profile directory exists
    let profile_path = get_browser_profile_path(def, &base_dir);
    match profile_path {
        Some(path) => path.exists(),
        None => false,
    }
}

/// Returns the base directory where browsers store their profiles.
///
/// - macOS: `~/Library/Application Support/`
/// - Windows: `%LOCALAPPDATA%/` (typically `C:\Users\{user}\AppData\Local\`)
/// - Linux: `~/.config/`
fn get_browser_base_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::data_dir() // ~/Library/Application Support/
    }

    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir() // %LOCALAPPDATA%
    }

    #[cfg(target_os = "linux")]
    {
        dirs::config_dir() // ~/.config/
    }
}

/// Resolves the full profile path for a browser definition.
///
/// Combines the platform base directory with the browser-specific
/// relative path from the `BrowserDef`.
fn get_browser_profile_path(
    def: &BrowserDef,
    base_dir: &std::path::Path,
) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        def.macos_path.map(|p| base_dir.join(p))
    }

    #[cfg(target_os = "windows")]
    {
        def.windows_path.map(|p| base_dir.join(p))
    }

    #[cfg(target_os = "linux")]
    {
        // Firefox on Linux uses ~/.mozilla/firefox, not ~/.config/firefox
        if def.id == "firefox" {
            dirs::home_dir().map(|h| h.join(".mozilla").join("firefox"))
        } else {
            def.linux_path.map(|p| base_dir.join(p))
        }
    }
}

// ============================================================
// Cookie Extraction & Conversion
// ============================================================

/// Apple Music domains to filter cookies for.
/// Only cookies matching these domains are extracted from the browser.
/// `apple.com` covers the main Apple Music site and subdomains.
/// `mzstatic.com` serves Apple's media assets and content delivery.
const APPLE_MUSIC_DOMAINS: &[&str] = &["apple.com", "mzstatic.com"];

/// Extracts Apple Music cookies from the specified browser and saves them
/// as a Netscape-format cookies file in the app data directory.
///
/// This function:
/// 1. Calls the appropriate `rookie::browser_name()` function with domain filtering
/// 2. Converts each `Cookie` struct to a Netscape format line
/// 3. Writes the result to `{app_data}/cookies.txt`
/// 4. Updates the application settings to point to the new file
///
/// ## Platform Behaviour
///
/// - **macOS (Chromium)**: May trigger a macOS Keychain access prompt
/// - **macOS (Safari)**: Requires Full Disk Access (check with `check_full_disk_access()` first)
/// - **Windows/Linux**: Transparent decryption via DPAPI / Secret Service
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving the app data directory and updating settings
/// * `browser_id` - The machine-readable browser identifier (e.g., "chrome", "firefox")
///
/// # Returns
/// * `Ok(CookieImportResult)` - Import result with cookie counts and file path
/// * `Err(String)` - Error message if extraction failed
pub fn extract_and_save(app: &AppHandle, browser_id: &str) -> Result<CookieImportResult, String> {
    // Step 1: Extract cookies from the browser using the rookie crate
    let domain_filter = Some(
        APPLE_MUSIC_DOMAINS
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<String>>(),
    );

    let cookies = call_rookie(browser_id, domain_filter)?;

    // Step 2: Convert to Netscape format
    let netscape_content = cookies_to_netscape(&cookies);

    // Step 3: Write to app data directory
    let cookies_path = platform::get_app_data_dir(app).join("cookies.txt");

    // Ensure the parent directory exists
    if let Some(parent) = cookies_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create cookies directory: {}", e))?;
    }

    std::fs::write(&cookies_path, &netscape_content)
        .map_err(|e| format!("Failed to write cookies file: {}", e))?;

    let cookies_path_str = cookies_path
        .to_str()
        .ok_or_else(|| "Failed to convert cookies path to string".to_string())?
        .to_string();

    log::info!(
        "Extracted {} cookies from {} to {}",
        cookies.len(),
        browser_id,
        cookies_path_str
    );

    // Step 4: Update settings to point to the new cookies file
    if let Ok(mut settings) = config_service::load_settings(app) {
        settings.cookies_path = Some(cookies_path_str.clone());
        if let Err(e) = config_service::save_settings(app, &settings) {
            log::warn!("Failed to update settings with new cookies path: {}", e);
        }
    }

    // Step 5: Build the result with validation info
    let mut warnings = Vec::new();
    let now = chrono::Utc::now().timestamp();

    // Count Apple Music cookies and check expiry
    let mut apple_music_count = 0;
    let mut has_expired = false;
    for cookie in &cookies {
        if cookie.domain.contains("apple.com") || cookie.domain.contains("mzstatic.com") {
            apple_music_count += 1;
            if let Some(expires) = cookie.expires {
                let expires_i64 = expires as i64;
                if expires_i64 > 0 && expires_i64 < now {
                    has_expired = true;
                } else if expires_i64 > 0 {
                    let days_until = (expires_i64 - now) / 86400;
                    if days_until < 7 {
                        warnings.push(format!(
                            "Apple Music cookies expire in {} day(s)",
                            days_until
                        ));
                    }
                }
            }
        }
    }

    if apple_music_count == 0 {
        warnings.push("No Apple Music cookies found in browser".to_string());
    }
    if has_expired {
        warnings.push(
            "Some Apple Music cookies have expired - you may need to log in again".to_string(),
        );
    }

    let success = apple_music_count > 0 && !has_expired;

    Ok(CookieImportResult {
        success,
        cookie_count: cookies.len(),
        apple_music_cookies: apple_music_count,
        warnings,
        path: cookies_path_str,
    })
}

/// Dispatches to the appropriate rookie crate function based on browser ID.
///
/// The rookie crate provides separate functions for each browser because
/// each uses different storage formats and encryption methods.
///
/// # Arguments
/// * `browser_id` - Browser identifier (e.g., "chrome", "firefox")
/// * `domains` - Optional domain filter (e.g., `Some(vec!["apple.com"])`)
///
/// # Returns
/// * `Ok(Vec<rookie::enums::Cookie>)` - Extracted cookies
/// * `Err(String)` - Error message if extraction failed
fn call_rookie(
    browser_id: &str,
    domains: Option<Vec<String>>,
) -> Result<Vec<rookie::enums::Cookie>, String> {
    let result = match browser_id {
        "chrome" => rookie::chrome(domains),
        "firefox" => rookie::firefox(domains),
        "edge" => rookie::edge(domains),
        "brave" => rookie::brave(domains),
        "chromium" => rookie::chromium(domains),
        "opera" => rookie::opera(domains),
        "vivaldi" => rookie::vivaldi(domains),
        "arc" => rookie::arc(domains),
        #[cfg(target_os = "macos")]
        "safari" => rookie::safari(domains),
        _ => return Err(format!("Unsupported browser: {}", browser_id)),
    };

    result.map_err(|e| format!("Failed to extract cookies from {}: {}", browser_id, e))
}

/// Converts a vector of rookie::enums::Cookie structs to Netscape cookie file format.
///
/// The Netscape cookie format is a tab-separated text format with 7 fields per line:
///   `domain \t subdomain_flag \t path \t secure \t expires \t name \t value`
///
/// This is the standard format that curl, wget, yt-dlp, and GAMDL expect.
///
/// # Arguments
/// * `cookies` - Vector of Cookie structs from the rookie crate
///
/// # Returns
/// The complete Netscape cookie file content as a string, including the
/// standard header comment.
///
/// # Reference
/// https://curl.se/docs/http-cookies.html
fn cookies_to_netscape(cookies: &[rookie::enums::Cookie]) -> String {
    let mut lines = Vec::new();

    // Standard Netscape cookie file header
    lines.push("# Netscape HTTP Cookie File".to_string());
    lines.push("# Extracted by gamdl-GUI from browser cookies".to_string());
    lines.push(String::new()); // Empty line after header

    for cookie in cookies {
        // Field 1: domain (add leading dot for subdomain matching if not present)
        let domain = if cookie.domain.starts_with('.') {
            cookie.domain.clone()
        } else {
            format!(".{}", cookie.domain)
        };

        // Field 2: subdomain flag (TRUE if domain starts with a dot)
        let subdomain_flag = "TRUE";

        // Field 3: path
        let path = &cookie.path;

        // Field 4: secure flag (TRUE/FALSE)
        let secure = if cookie.secure { "TRUE" } else { "FALSE" };

        // Field 5: expiry (Unix timestamp, 0 for session cookies)
        let expires = cookie.expires.unwrap_or(0);

        // Field 6: cookie name
        let name = &cookie.name;

        // Field 7: cookie value
        let value = &cookie.value;

        lines.push(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            domain, subdomain_flag, path, secure, expires, name, value
        ));
    }

    // Trailing newline for compatibility with configparser
    lines.join("\n") + "\n"
}

// ============================================================
// macOS Full Disk Access Check
// ============================================================

/// Checks whether the application has Full Disk Access on macOS.
///
/// Safari stores its cookie database in a location protected by macOS's
/// Transparency, Consent, and Control (TCC) framework. Without Full Disk
/// Access, the app cannot read Safari's cookies.
///
/// This function tests FDA by attempting to read a known protected file
/// (`~/Library/Safari/Bookmarks.plist`). If the read succeeds (or the file
/// doesn't exist but the directory is accessible), FDA is granted.
///
/// On non-macOS platforms, this always returns `true` (FDA is a macOS-only
/// concept).
///
/// # Returns
/// `true` if Full Disk Access is granted (or not required on this platform).
pub fn check_full_disk_access() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Try to read a file protected by TCC (Full Disk Access).
        // ~/Library/Safari/Bookmarks.plist is a reliable test target because:
        // 1. Safari is always installed on macOS
        // 2. The file is always TCC-protected
        // 3. Reading it requires FDA (not just the Safari data directory)
        if let Some(home) = dirs::home_dir() {
            let test_path = home.join("Library/Safari/Bookmarks.plist");
            // Attempt to open the file. If we can open it, FDA is granted.
            // If the error is "permission denied", FDA is not granted.
            // If the error is "not found", the file doesn't exist but FDA
            // may still be granted -- we check directory access instead.
            match std::fs::File::open(&test_path) {
                Ok(_) => true,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        // File doesn't exist, but try the directory
                        let safari_dir = home.join("Library/Safari");
                        std::fs::read_dir(safari_dir).is_ok()
                    } else {
                        // Permission denied or other error -- FDA not granted
                        false
                    }
                }
            }
        } else {
            false
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        // FDA is a macOS-only concept; other platforms don't need it
        true
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // cookies_to_netscape: header format
    // ----------------------------------------------------------

    #[test]
    fn netscape_output_starts_with_header() {
        let cookies: Vec<rookie::enums::Cookie> = vec![];
        let output = cookies_to_netscape(&cookies);
        assert!(output.starts_with("# Netscape HTTP Cookie File"));
    }

    #[test]
    fn netscape_output_ends_with_newline() {
        let cookies: Vec<rookie::enums::Cookie> = vec![];
        let output = cookies_to_netscape(&cookies);
        assert!(output.ends_with('\n'));
    }

    // ----------------------------------------------------------
    // cookies_to_netscape: cookie formatting
    // ----------------------------------------------------------

    #[test]
    fn netscape_formats_cookie_correctly() {
        let cookies = vec![rookie::enums::Cookie {
            domain: "apple.com".to_string(),
            path: "/".to_string(),
            secure: true,
            expires: Some(1700000000),
            name: "test_cookie".to_string(),
            value: "test_value".to_string(),
            http_only: false,
            same_site: 0,
        }];

        let output = cookies_to_netscape(&cookies);
        assert!(output.contains(".apple.com\tTRUE\t/\tTRUE\t1700000000\ttest_cookie\ttest_value"));
    }

    #[test]
    fn netscape_adds_leading_dot_to_domain() {
        let cookies = vec![rookie::enums::Cookie {
            domain: "example.com".to_string(),
            path: "/".to_string(),
            secure: false,
            expires: Some(0),
            name: "name".to_string(),
            value: "val".to_string(),
            http_only: false,
            same_site: 0,
        }];

        let output = cookies_to_netscape(&cookies);
        assert!(output.contains(".example.com\t"));
    }

    #[test]
    fn netscape_preserves_existing_leading_dot() {
        let cookies = vec![rookie::enums::Cookie {
            domain: ".apple.com".to_string(),
            path: "/".to_string(),
            secure: true,
            expires: Some(999),
            name: "n".to_string(),
            value: "v".to_string(),
            http_only: false,
            same_site: 0,
        }];

        let output = cookies_to_netscape(&cookies);
        // Should NOT have double dot
        assert!(output.contains(".apple.com\t"));
        assert!(!output.contains("..apple.com"));
    }

    #[test]
    fn netscape_session_cookie_has_zero_expiry() {
        let cookies = vec![rookie::enums::Cookie {
            domain: "apple.com".to_string(),
            path: "/".to_string(),
            secure: false,
            expires: None,
            name: "session".to_string(),
            value: "val".to_string(),
            http_only: false,
            same_site: 0,
        }];

        let output = cookies_to_netscape(&cookies);
        assert!(output.contains("\t0\tsession\t"));
    }

    // ----------------------------------------------------------
    // detect_browsers: basic check
    // ----------------------------------------------------------

    #[test]
    fn detect_browsers_returns_vec() {
        // Should not panic; may return empty on CI where no browsers are installed
        let browsers = detect_browsers();
        assert!(browsers.len() <= BROWSER_DEFS.len());
    }

    // ----------------------------------------------------------
    // check_full_disk_access: basic check
    // ----------------------------------------------------------

    #[test]
    fn full_disk_access_returns_bool() {
        // Should not panic on any platform
        let _has_fda = check_full_disk_access();
    }
}
