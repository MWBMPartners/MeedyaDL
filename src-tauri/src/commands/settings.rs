// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Settings management IPC commands.
// Handles reading, writing, and validating application settings.
// Settings are stored as JSON in the app data directory and are
// synced to GAMDL's config.ini file for CLI compatibility.

use serde::Serialize;
use tauri::AppHandle;

use crate::models::settings::AppSettings;
use crate::services::config_service;

/// Result of validating a Netscape-format cookies file.
/// Provides detailed information about the cookies found and their validity.
#[derive(Debug, Clone, Serialize)]
pub struct CookieValidation {
    /// Whether the file is a valid Netscape cookie file
    pub valid: bool,
    /// Total number of cookie entries found in the file
    pub cookie_count: usize,
    /// Unique domains present in the cookie file
    pub domains: Vec<String>,
    /// Number of cookies specifically for Apple Music domains
    pub apple_music_cookies: usize,
    /// Whether any Apple Music cookies have expired
    pub expired: bool,
    /// Warning messages (e.g., "Cookies expire in 3 days")
    pub warnings: Vec<String>,
}

/// Loads and returns the current application settings.
///
/// If no settings file exists (first run), returns default settings.
/// Settings are loaded from {app_data}/settings.json and are synced
/// with GAMDL's config.ini file.
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    config_service::load_settings(&app)
}

/// Saves application settings to disk.
///
/// Writes the settings as formatted JSON to {app_data}/settings.json.
/// Also syncs relevant settings to GAMDL's config.ini file so that
/// GAMDL CLI uses the same configuration as the GUI.
#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    config_service::save_settings(&app, &settings)
}

/// Validates a Netscape-format cookies file.
///
/// Parses the file to check:
/// - Whether it's a valid Netscape cookie format
/// - How many cookies it contains
/// - Whether Apple Music-specific cookies are present
/// - Whether any cookies have expired
///
/// # Arguments
/// * `path` - Absolute path to the cookies.txt file to validate
#[tauri::command]
pub async fn validate_cookies_file(path: String) -> Result<CookieValidation, String> {
    // Read the cookie file
    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read cookie file: {}", e))?;

    let mut cookie_count = 0;
    let mut domains = std::collections::HashSet::new();
    let mut apple_music_cookies = 0;
    let mut expired = false;
    let mut warnings = Vec::new();
    let now = chrono::Utc::now().timestamp();

    // Parse each line of the Netscape cookie file
    for line in contents.lines() {
        // Skip comments and empty lines
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Netscape cookie format: domain\tsubdomains\tpath\tsecure\texpiry\tname\tvalue
        let fields: Vec<&str> = trimmed.split('\t').collect();
        if fields.len() >= 7 {
            cookie_count += 1;
            let domain = fields[0].trim_start_matches('.');
            domains.insert(domain.to_string());

            // Check for Apple Music related domains
            if domain.contains("apple.com") || domain.contains("mzstatic.com") {
                apple_music_cookies += 1;

                // Check cookie expiry timestamp
                if let Ok(expiry) = fields[4].parse::<i64>() {
                    if expiry > 0 && expiry < now {
                        expired = true;
                    } else if expiry > 0 {
                        // Warn if cookies expire within 7 days
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

    // Build validation result
    let valid = cookie_count > 0;

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
        domains: domains.into_iter().collect(),
        apple_music_cookies,
        expired,
        warnings,
    })
}

/// Returns the default output path for downloaded music.
///
/// Uses platform-appropriate music directory:
/// - macOS: ~/Music/Apple Music/
/// - Windows: ~\Music\Apple Music\
/// - Linux: ~/Music/Apple Music/
#[tauri::command]
pub fn get_default_output_path() -> Result<String, String> {
    config_service::get_default_output_path()
}
