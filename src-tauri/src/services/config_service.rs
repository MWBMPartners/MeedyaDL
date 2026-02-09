// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Settings and configuration service.
// Handles loading/saving the application's JSON settings file and
// syncing relevant settings to GAMDL's config.ini file. This ensures
// that GAMDL CLI commands use the same configuration as the GUI.
//
// The app has two config files:
// 1. settings.json - GUI settings (all AppSettings fields, JSON format)
// 2. config.ini - GAMDL's native config (INI format, subset of settings)

use tauri::AppHandle;

use crate::models::settings::AppSettings;
use crate::utils::platform;

/// Loads the application settings from the JSON settings file.
///
/// If the settings file doesn't exist (first run), returns default settings.
/// If the file exists but contains invalid JSON, returns an error.
/// If the file is missing some fields (e.g., after an app update added new
/// settings), serde's default values fill in the gaps.
///
/// # Arguments
/// * `app` - The Tauri app handle (for path resolution)
///
/// # Returns
/// * `Ok(settings)` - The loaded or default settings
/// * `Err(message)` - If the settings file exists but couldn't be parsed
pub fn load_settings(app: &AppHandle) -> Result<AppSettings, String> {
    let settings_path = platform::get_app_data_dir(app).join("settings.json");

    if settings_path.exists() {
        let contents = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings file: {}", e))?;

        serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse settings file: {}", e))
    } else {
        // First run: return default settings
        log::info!("No settings file found, using defaults");
        Ok(AppSettings::default())
    }
}

/// Saves the application settings to the JSON settings file.
///
/// Writes the settings as pretty-printed JSON for human readability.
/// Also syncs relevant settings to GAMDL's config.ini file so that
/// CLI commands launched by the app use the same configuration.
///
/// # Arguments
/// * `app` - The Tauri app handle (for path resolution)
/// * `settings` - The settings to save
pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let settings_path = platform::get_app_data_dir(app).join("settings.json");

    // Ensure the directory exists
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings directory: {}", e))?;
    }

    // Serialize to pretty JSON for readability
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    // Write the settings file
    std::fs::write(&settings_path, json)
        .map_err(|e| format!("Failed to write settings file: {}", e))?;

    log::info!("Settings saved to {}", settings_path.display());

    // Sync relevant settings to GAMDL's config.ini
    if let Err(e) = sync_to_gamdl_config(app, settings) {
        log::warn!("Failed to sync settings to GAMDL config: {}", e);
        // Don't fail the save operation for this
    }

    Ok(())
}

/// Syncs relevant AppSettings fields to GAMDL's config.ini file.
///
/// GAMDL reads its configuration from an INI file. The GUI manages all
/// settings in JSON, but we also write a config.ini so that GAMDL CLI
/// commands use the same settings. We pass --config-path to GAMDL to
/// point it at our managed config.ini.
///
/// INI format example:
/// ```ini
/// [gamdl]
/// cookies-path = /path/to/cookies.txt
/// song-codec = alac
/// save-cover
/// cover-format = raw
/// ```
///
/// Boolean flags are written as bare keys (present = true, absent = false).
/// Key-value options use `key = value` format.
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `settings` - The application settings to convert and write
fn sync_to_gamdl_config(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let config_path = platform::get_gamdl_config_path(app);

    // Ensure the GAMDL data directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create GAMDL config directory: {}", e))?;
    }

    // Build the INI file content
    let ini_content = settings_to_ini(settings);

    // Write the config file
    std::fs::write(&config_path, ini_content)
        .map_err(|e| format!("Failed to write GAMDL config: {}", e))?;

    log::info!("GAMDL config synced to {}", config_path.display());
    Ok(())
}

/// Converts AppSettings into GAMDL's INI config format.
///
/// Only includes settings that GAMDL actually reads from its config file.
/// Uses the same key names as GAMDL's CLI flags (without the -- prefix).
///
/// # Arguments
/// * `settings` - The application settings to convert
///
/// # Returns
/// The INI file content as a string
fn settings_to_ini(settings: &AppSettings) -> String {
    let mut lines = Vec::new();

    // Section header required by configparser
    lines.push("[gamdl]".to_string());

    // --- Authentication ---
    // Cookies path (most important setting for GAMDL to work)
    if let Some(ref path) = settings.cookies_path {
        lines.push(format!("cookies-path = {}", path));
    }

    // --- Audio Quality ---
    lines.push(format!(
        "song-codec = {}",
        settings.default_song_codec.to_cli_string()
    ));

    // --- Video Quality ---
    lines.push(format!(
        "music-video-resolution = {}",
        settings.default_video_resolution.to_cli_string()
    ));
    if !settings.default_video_codec_priority.is_empty() {
        lines.push(format!(
            "music-video-codec-priority = {}",
            settings.default_video_codec_priority
        ));
    }
    if !settings.default_video_remux_format.is_empty() {
        lines.push(format!(
            "music-video-remux-format = {}",
            settings.default_video_remux_format
        ));
    }

    // --- Lyrics ---
    lines.push(format!(
        "synced-lyrics-format = {}",
        settings.synced_lyrics_format.to_cli_string()
    ));
    if settings.no_synced_lyrics {
        lines.push("no-synced-lyrics".to_string());
    }

    // --- Cover Art ---
    if settings.save_cover {
        lines.push("save-cover".to_string());
    }
    lines.push(format!(
        "cover-format = {}",
        settings.cover_format.to_cli_string()
    ));
    lines.push(format!(
        "cover-size = {}x{}",
        settings.cover_size, settings.cover_size
    ));

    // --- Output ---
    if !settings.output_path.is_empty() {
        lines.push(format!("output-path = {}", settings.output_path));
    }
    if settings.overwrite {
        lines.push("overwrite".to_string());
    }
    if let Some(truncate) = settings.truncate {
        lines.push(format!("truncate = {}", truncate));
    }

    // --- Metadata ---
    lines.push(format!("language = {}", settings.language));

    // --- Templates ---
    if !settings.album_folder_template.is_empty() {
        lines.push(format!(
            "album-folder-template = {}",
            settings.album_folder_template
        ));
    }
    if !settings.compilation_folder_template.is_empty() {
        lines.push(format!(
            "compilation-folder-template = {}",
            settings.compilation_folder_template
        ));
    }
    if !settings.no_album_folder_template.is_empty() {
        lines.push(format!(
            "no-album-folder-template = {}",
            settings.no_album_folder_template
        ));
    }
    if !settings.single_disc_file_template.is_empty() {
        lines.push(format!(
            "single-disc-file-template = {}",
            settings.single_disc_file_template
        ));
    }
    if !settings.multi_disc_file_template.is_empty() {
        lines.push(format!(
            "multi-disc-file-template = {}",
            settings.multi_disc_file_template
        ));
    }
    if !settings.no_album_file_template.is_empty() {
        lines.push(format!(
            "no-album-file-template = {}",
            settings.no_album_file_template
        ));
    }
    if !settings.playlist_file_template.is_empty() {
        lines.push(format!(
            "playlist-file-template = {}",
            settings.playlist_file_template
        ));
    }

    // --- Tool Paths (custom user-specified paths only) ---
    if let Some(ref path) = settings.ffmpeg_path {
        lines.push(format!("ffmpeg-path = {}", path));
    }
    if let Some(ref path) = settings.mp4decrypt_path {
        lines.push(format!("mp4decrypt-path = {}", path));
    }
    if let Some(ref path) = settings.mp4box_path {
        lines.push(format!("mp4box-path = {}", path));
    }
    if let Some(ref path) = settings.nm3u8dlre_path {
        lines.push(format!("nm3u8dlre-path = {}", path));
    }

    // --- Advanced ---
    if settings.use_wrapper {
        lines.push("use-wrapper".to_string());
        lines.push(format!(
            "wrapper-account-url = {}",
            settings.wrapper_account_url
        ));
    }

    // Join all lines with newlines and add a trailing newline
    lines.join("\n") + "\n"
}

/// Resolves the default output path for downloaded music.
///
/// Uses the platform's standard Music/Audio directory with an
/// "Apple Music" subdirectory appended.
///
/// # Returns
/// * `Ok(path)` - The default output path string
/// * `Err(message)` - If the home directory couldn't be determined
pub fn get_default_output_path() -> Result<String, String> {
    let music_dir = dirs::audio_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Could not determine home directory".to_string())?;

    let output_path = music_dir.join("Apple Music");

    output_path
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Failed to convert output path to string".to_string())
}
