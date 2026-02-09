// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Settings management IPC commands.
// Handles reading, writing, and validating application settings.
// Settings are stored as JSON in the app data directory and are
// separate from GAMDL's own config.ini file (which is also managed).

use serde::Serialize;
use tauri::AppHandle;

use crate::models::settings::AppSettings;
use crate::utils::platform;

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
/// Settings are loaded from {app_data}/settings.json.
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    // Resolve the settings file path
    let settings_path = platform::get_app_data_dir(&app).join("settings.json");

    // If settings file exists, read and parse it
    if settings_path.exists() {
        let contents =
            std::fs::read_to_string(&settings_path).map_err(|e| format!("Failed to read settings: {}", e))?;
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse settings: {}", e))
    } else {
        // Return default settings for first run
        Ok(AppSettings::default())
    }
}

/// Saves application settings to disk.
///
/// Writes the settings as formatted JSON to {app_data}/settings.json.
/// Also syncs relevant settings to GAMDL's config.ini file so that
/// GAMDL CLI uses the same configuration as the GUI.
#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    // Resolve the settings file path
    let settings_path = platform::get_app_data_dir(&app).join("settings.json");

    // Serialize settings to formatted JSON for readability
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    // Write to disk
    std::fs::write(&settings_path, json)
        .map_err(|e| format!("Failed to write settings: {}", e))?;

    log::info!("Settings saved to: {}", settings_path.display());

    // TODO: Phase 2 - Sync relevant settings to GAMDL's config.ini

    Ok(())
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

                // Check cookie expiry
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

    // Validate the results
    let valid = cookie_count > 0;

    if apple_music_cookies == 0 {
        warnings.push("No Apple Music cookies found in file".to_string());
    }

    if expired {
        warnings.push("Some Apple Music cookies have expired - you may need to re-export them".to_string());
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
    // Try to find the user's Music directory
    let music_dir = dirs::audio_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Could not determine home directory".to_string())?;

    // Append our subdirectory
    let output_path = music_dir.join("Apple Music");

    output_path
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Failed to convert output path to string".to_string())
}
