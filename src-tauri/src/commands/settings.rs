// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Settings management IPC commands.
// Handles reading, writing, and validating application settings.
// Settings are stored as JSON in the app data directory and are
// synced to GAMDL's config.ini file for CLI compatibility.
//
// ## Architecture
//
// Application settings are persisted in two formats:
//   1. **settings.json** — The canonical settings file used by this GUI app.
//      Located at `{app_data}/settings.json`. Contains all GUI-specific settings
//      plus GAMDL configuration values.
//   2. **config.ini** — GAMDL's native config file (INI format).
//      When settings are saved, relevant fields are synced to config.ini so the
//      GAMDL CLI subprocess reads the correct configuration.
//
// The settings model (`AppSettings`) is defined in `src-tauri/src/models/settings.rs`
// and includes fields for output path, codec, quality, cookies path, and more.
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command            | TypeScript Function        | Line |
// |-------------------------|----------------------------|------|
// | get_settings            | getSettings()              | ~75  |
// | save_settings           | saveSettings(settings)     | ~80  |
// | validate_cookies_file   | validateCookiesFile(path)  | ~85  |
// | get_default_output_path | getDefaultOutputPath()     | ~90  |
//
// ## References
//
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - Netscape cookie format: https://curl.se/docs/http-cookies.html

// serde::Serialize is required for CookieValidation which is returned to the frontend.
use serde::Serialize;
// AppHandle for resolving app data directory paths (settings.json location).
use tauri::AppHandle;

// AppSettings is the Rust struct representing the full application settings.
// It implements both Serialize (for returning to frontend) and Deserialize
// (for accepting from frontend when saving).
use crate::models::settings::AppSettings;
// config_service handles the actual file I/O: reading/writing settings.json
// and syncing to GAMDL's config.ini file.
use crate::services::config_service;

/// Result of validating a Netscape-format cookies file.
///
/// Provides detailed information about the cookies found and their validity.
/// This is used by the frontend's cookie file picker to give the user
/// immediate feedback about whether their exported cookies file is usable.
///
/// The Netscape cookie format is a tab-separated text format originally
/// defined by Netscape Navigator and still used by curl, wget, and browser
/// cookie export extensions.
/// See: https://curl.se/docs/http-cookies.html
///
/// Implements `Serialize` for Tauri IPC serialization to JSON.
#[derive(Debug, Clone, Serialize)]
pub struct CookieValidation {
    /// Whether the file is a valid Netscape cookie file (has at least one parseable entry)
    pub valid: bool,
    /// Total number of cookie entries found in the file (across all domains)
    pub cookie_count: usize,
    /// Unique domains present in the cookie file (e.g., ["apple.com", "mzstatic.com"])
    pub domains: Vec<String>,
    /// Number of cookies specifically for Apple Music domains (apple.com, mzstatic.com).
    /// GAMDL requires Apple Music cookies for authentication.
    pub apple_music_cookies: usize,
    /// Whether any Apple Music cookies have expired (timestamp < now)
    pub expired: bool,
    /// Warning messages for the user, e.g.:
    /// - "Apple Music cookies expire in 3 day(s)"
    /// - "No Apple Music cookies found in file"
    /// - "Some Apple Music cookies have expired"
    pub warnings: Vec<String>,
}

/// Loads and returns the current application settings.
///
/// **Frontend caller:** `getSettings()` in `src/lib/tauri-commands.ts`
///
/// If no settings file exists (first run), returns `AppSettings::default()`
/// which provides sensible defaults (AAC codec, 256kbps, etc.).
/// Settings are loaded from `{app_data}/settings.json`.
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving the settings.json file path.
///
/// # Returns
/// * `Ok(AppSettings)` - The current settings, serialized to JSON for the frontend.
///   The frontend stores these in React state for the settings page.
/// * `Err(String)` - File read or JSON parse error.
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    config_service::load_settings(&app)
}

/// Saves application settings to disk.
///
/// **Frontend caller:** `saveSettings(settings)` in `src/lib/tauri-commands.ts`
///
/// Writes the settings as pretty-printed JSON to `{app_data}/settings.json`.
/// Also syncs relevant settings to GAMDL's `config.ini` file so that
/// the GAMDL CLI subprocess reads the same configuration as the GUI.
///
/// The sync to config.ini is important because GAMDL reads its own config
/// file (not settings.json) when invoked as a subprocess during downloads.
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving file paths.
/// * `settings` - The complete settings object from the frontend.
///   Deserialized from the JSON payload sent by `invoke("save_settings", { settings })`.
///   See: https://v2.tauri.app/develop/calling-rust/#command-arguments
///
/// # Returns
/// * `Ok(())` - Settings saved and synced successfully.
/// * `Err(String)` - File write or serialization error.
#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    // save_settings() in config_service performs two writes:
    //   1. settings.json — full AppSettings struct as JSON
    //   2. config.ini — relevant fields translated to GAMDL's INI format
    config_service::save_settings(&app, &settings)
}

/// Validates a Netscape-format cookies file.
///
/// **Frontend caller:** `validateCookiesFile(path)` in `src/lib/tauri-commands.ts`
///
/// Parses the file to check:
/// - Whether it's a valid Netscape cookie format (has parseable entries)
/// - How many cookies it contains (across all domains)
/// - Whether Apple Music-specific cookies are present (required for GAMDL)
/// - Whether any Apple Music cookies have expired
/// - Whether cookies are about to expire (within 7 days)
///
/// This command does NOT require the AppHandle because it only reads the
/// file at the user-provided path — no app state or data directory needed.
///
/// # Arguments
/// * `path` - Absolute path to the cookies.txt file to validate.
///   Provided by the frontend's file picker dialog.
///
/// # Returns
/// * `Ok(CookieValidation)` - Detailed validation result with counts and warnings.
/// * `Err(String)` - File read error (file not found, permission denied, etc.).
///
/// # Netscape Cookie Format
/// Each cookie line is tab-separated with 7 fields:
/// `domain \t subdomains \t path \t secure \t expiry \t name \t value`
/// Lines starting with `#` are comments. Empty lines are skipped.
/// See: https://curl.se/docs/http-cookies.html
#[tauri::command]
pub async fn validate_cookies_file(path: String) -> Result<CookieValidation, String> {
    // Read the entire cookie file into memory.
    // Cookie files are typically small (< 100KB) so reading all at once is fine.
    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read cookie file: {}", e))?;

    // Tracking variables for the validation scan
    let mut cookie_count = 0;
    let mut domains = std::collections::HashSet::new(); // Deduplicates domain names
    let mut apple_music_cookies = 0;
    let mut expired = false;
    let mut warnings = Vec::new();
    // Current UTC timestamp for comparing against cookie expiry times
    let now = chrono::Utc::now().timestamp();

    // Parse each line of the Netscape cookie file
    for line in contents.lines() {
        // Skip comments (lines starting with #) and empty lines.
        // The Netscape format uses "# Netscape HTTP Cookie File" as a header comment.
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Split the line by tabs into the 7 expected fields.
        // Format: domain \t subdomains \t path \t secure \t expiry \t name \t value
        // Fields[0] = domain (may have leading dot for subdomain matching)
        // Fields[4] = expiry (Unix timestamp, 0 means session cookie)
        let fields: Vec<&str> = trimmed.split('\t').collect();
        if fields.len() >= 7 {
            cookie_count += 1;
            // Strip the leading dot from domain (e.g., ".apple.com" -> "apple.com")
            // for consistent deduplication and display
            let domain = fields[0].trim_start_matches('.');
            domains.insert(domain.to_string());

            // Check for Apple Music related domains — these are the cookies
            // GAMDL needs for authenticated access to Apple Music content.
            // apple.com covers the main site; mzstatic.com serves media assets.
            if domain.contains("apple.com") || domain.contains("mzstatic.com") {
                apple_music_cookies += 1;

                // Parse the expiry timestamp (field index 4) to check validity.
                // A value of 0 means "session cookie" (no expiry) — skip those.
                if let Ok(expiry) = fields[4].parse::<i64>() {
                    if expiry > 0 && expiry < now {
                        // This cookie has already expired
                        expired = true;
                    } else if expiry > 0 {
                        // Cookie is still valid — warn if it expires within 7 days
                        // (86400 seconds per day)
                        let days_until_expiry = (expiry - now) / 86400;
                        if days_until_expiry < 7 {
                            warnings.push(format!(
                                "Apple Music cookies expire in {} day(s)",
                                days_until_expiry
                            ));
                        }
                    }
                }
            }
        }
    }

    // A file is considered "valid" if it contains at least one parseable cookie entry
    let valid = cookie_count > 0;

    // Add warnings for common issues that would prevent GAMDL from working
    if apple_music_cookies == 0 {
        warnings.push("No Apple Music cookies found in file".to_string());
    }

    if expired {
        warnings.push(
            "Some Apple Music cookies have expired - you may need to re-export them".to_string(),
        );
    }

    Ok(CookieValidation {
        valid,
        cookie_count,
        // Convert HashSet to Vec for JSON serialization (HashSet is not serializable)
        domains: domains.into_iter().collect(),
        apple_music_cookies,
        expired,
        warnings,
    })
}

/// Returns the default output path for downloaded music.
///
/// **Frontend caller:** `getDefaultOutputPath()` in `src/lib/tauri-commands.ts`
///
/// Uses the platform-appropriate music directory as the base, with an
/// "Apple Music" subdirectory:
/// - macOS: `~/Music/Apple Music/`
/// - Windows: `~\Music\Apple Music\`
/// - Linux: `~/Music/Apple Music/`
///
/// This is a synchronous command (no `async`) because it only resolves
/// paths using environment variables — no I/O or network access needed.
/// Note: Tauri allows both sync and async command handlers.
/// See: https://v2.tauri.app/develop/calling-rust/#async-commands
///
/// # Returns
/// * `Ok(String)` - The absolute path to the default music output directory.
/// * `Err(String)` - If the user's home/music directory cannot be determined.
#[tauri::command]
pub fn get_default_output_path() -> Result<String, String> {
    config_service::get_default_output_path()
}
