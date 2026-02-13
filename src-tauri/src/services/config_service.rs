// Copyright (c) 2024-2026 MeedyaDL
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
//
// ## Dual-Config Pattern
//
// This application uses a dual-config architecture:
//
// ```
// settings.json (source of truth)     config.ini (derived, for GAMDL CLI)
// ================================     ====================================
// {                                    [gamdl]
//   "default_song_codec": "alac",      song-codec = alac
//   "save_cover": true,         -->    save-cover
//   "cover_format": "raw",             cover-format = raw
//   "theme": "dark",                   (not synced - GUI-only setting)
//   ...                                ...
// }
// ```
//
// The JSON file is the single source of truth, managed by the React frontend
// via Tauri commands (load_settings, save_settings). When settings are saved,
// `sync_to_gamdl_config()` writes a config.ini that GAMDL can read natively.
//
// The config.ini is passed to GAMDL via `--config-path` in gamdl_service.rs.
// This avoids having to pass every setting as a CLI flag, and allows GAMDL
// to read settings it may look up that we don't explicitly pass as flags.
//
// GUI-only settings (e.g., theme, window size) exist only in settings.json
// and are not synced to config.ini since GAMDL doesn't need them.
//
// ## References
//
// - serde_json for JSON serialization: https://docs.rs/serde_json/latest/serde_json/
// - configparser crate (GAMDL uses Python's configparser): https://docs.rs/configparser/latest/configparser/
// - GAMDL config file format: https://github.com/glomatico/gamdl#configuration
// - dirs crate for platform-standard directories: https://docs.rs/dirs/latest/dirs/

use tauri::AppHandle;

// AppSettings is the Rust struct that mirrors all GUI settings.
// It derives Serialize/Deserialize for JSON round-tripping and Default for first-run defaults.
// Defined in models/settings.rs.
use crate::models::settings::AppSettings;
// Platform utilities for resolving the app data directory and config file paths
// across macOS, Windows, and Linux.
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
    // Resolve the settings file path: {app_data_dir}/settings.json
    // On macOS: ~/Library/Application Support/io.github.meedyadl/settings.json
    // On Windows: %APPDATA%\io.github.meedyadl\settings.json
    // On Linux: ~/.config/io.github.meedyadl/settings.json
    let settings_path = platform::get_app_data_dir(app).join("settings.json");

    if settings_path.exists() {
        // Read the entire file into a string. This is synchronous (blocking I/O)
        // since settings are loaded during Tauri command handlers which run on
        // the Tokio thread pool and can tolerate brief blocking.
        let contents = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings file: {}", e))?;

        // Deserialize the JSON into AppSettings. serde_json handles:
        // - Missing fields: filled with #[serde(default)] values
        // - Extra fields: silently ignored (forward compatibility)
        // - Type mismatches: returns a descriptive parse error
        // Ref: https://docs.rs/serde_json/latest/serde_json/fn.from_str.html
        serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse settings file: {}", e))
    } else {
        // First run: no settings file exists yet. Return the default settings
        // which are defined via #[derive(Default)] on AppSettings.
        // The defaults provide sensible out-of-the-box values for all settings.
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

    // Ensure the parent directory exists (important on first run or after data dir deletion)
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings directory: {}", e))?;
    }

    // Serialize to pretty-printed JSON for human readability.
    // Pretty printing adds indentation, making the file easy to inspect and debug.
    // Ref: https://docs.rs/serde_json/latest/serde_json/fn.to_string_pretty.html
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    // Atomically write the settings file. Note: std::fs::write is not truly atomic
    // (no rename-based write), but is sufficient for our use case where concurrent
    // writes are prevented by the Tauri command serialization.
    std::fs::write(&settings_path, json)
        .map_err(|e| format!("Failed to write settings file: {}", e))?;

    log::info!("Settings saved to {}", settings_path.display());

    // Sync relevant settings to GAMDL's config.ini (the derived config).
    // This is a best-effort operation: if it fails, the JSON save still succeeds.
    // GAMDL will still work via CLI flags; the config.ini is a convenience.
    if let Err(e) = sync_to_gamdl_config(app, settings) {
        log::warn!("Failed to sync settings to GAMDL config: {}", e);
        // Don't fail the save operation — the JSON settings are the source of truth
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
    // Resolve the GAMDL config.ini path: {app_data_dir}/gamdl/config.ini
    // This path is passed to GAMDL via --config-path in gamdl_service::build_gamdl_command().
    let config_path = platform::get_gamdl_config_path(app);

    // Ensure the GAMDL data directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create GAMDL config directory: {}", e))?;
    }

    // Build the INI file content by converting AppSettings fields to INI key-value pairs.
    // Only settings that GAMDL reads from its config file are included.
    let ini_content = settings_to_ini(settings);

    // Write the config file, completely replacing any previous content.
    // GAMDL parses this file using Python's configparser module.
    // Ref: https://docs.python.org/3/library/configparser.html
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

    // Section header required by Python's configparser.
    // GAMDL expects all settings under a [gamdl] section.
    // Ref: https://github.com/glomatico/gamdl#configuration
    lines.push("[gamdl]".to_string());

    // === Authentication ===
    // The cookies path is the most important setting — GAMDL needs it to
    // authenticate with Apple Music. Without cookies, downloads will fail.
    // The cookies.txt file is extracted from a browser session (Netscape format).
    if let Some(ref path) = settings.cookies_path {
        lines.push(format!("cookies-path = {}", path));
    }

    // === Audio Quality ===
    // Maps to GAMDL's --song-codec flag. Valid values: alac, aac-he, aac-binaural, etc.
    // The to_cli_string() method on the SongCodec enum returns the GAMDL-compatible string.
    lines.push(format!(
        "song-codec = {}",
        settings.default_song_codec.to_cli_string()
    ));

    // === Video Quality ===
    // Maps to GAMDL's --music-video-resolution flag. Values like "1080p", "4k", etc.
    lines.push(format!(
        "music-video-resolution = {}",
        settings.default_video_resolution.to_cli_string()
    ));
    // Video codec priority is a comma-separated list (e.g., "h265,h264").
    // Only written if the user has set a preference.
    if !settings.default_video_codec_priority.is_empty() {
        lines.push(format!(
            "music-video-codec-priority = {}",
            settings.default_video_codec_priority
        ));
    }
    // Video remux format (e.g., "mkv", "mp4"). Only written if set.
    if !settings.default_video_remux_format.is_empty() {
        lines.push(format!(
            "music-video-remux-format = {}",
            settings.default_video_remux_format
        ));
    }

    // === Lyrics ===
    // Synced lyrics format (e.g., "lrc", "srt").
    lines.push(format!(
        "synced-lyrics-format = {}",
        settings.synced_lyrics_format.to_cli_string()
    ));
    // Boolean flag: presence means true, absence means false.
    // In INI format, boolean flags are written as bare keys (no value).
    if settings.no_synced_lyrics {
        lines.push("no-synced-lyrics".to_string());
    }

    // === Cover Art ===
    // Boolean flag: when present, GAMDL saves cover art as a separate file.
    if settings.save_cover {
        lines.push("save-cover".to_string());
    }
    // Cover format (e.g., "raw" for original, "jpg", "png").
    lines.push(format!(
        "cover-format = {}",
        settings.cover_format.to_cli_string()
    ));
    // Cover size as WxH (GAMDL uses square covers, e.g., "1200x1200").
    lines.push(format!(
        "cover-size = {}x{}",
        settings.cover_size, settings.cover_size
    ));

    // === Output ===
    // Output directory for downloaded files. Only written if non-empty.
    if !settings.output_path.is_empty() {
        lines.push(format!("output-path = {}", settings.output_path));
    }
    // Boolean flag: when present, existing files are overwritten without prompting.
    if settings.overwrite {
        lines.push("overwrite".to_string());
    }
    // Truncate file/folder names to this many characters (avoids filesystem limits).
    if let Some(truncate) = settings.truncate {
        lines.push(format!("truncate = {}", truncate));
    }

    // === Metadata ===
    // Language code for metadata (e.g., "en-US", "ja-JP").
    // Affects how track/album names are retrieved from Apple Music.
    lines.push(format!("language = {}", settings.language));

    // === Templates ===
    // Output path templates use Python format strings with metadata placeholders.
    // Example: "{album_artist}/{album}" -> "Taylor Swift/1989 (Taylor's Version)"
    // Ref: https://github.com/glomatico/gamdl#output-path-template
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

    // === Tool Paths (custom user-specified paths only) ===
    // These are tool paths the user explicitly configured in Settings.
    // Note: managed tool paths (installed by dependency_manager.rs) are injected
    // separately via gamdl_service::inject_tool_paths() at command build time.
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

    // === Advanced ===
    // Wrapper mode uses an alternative authentication method via a web service.
    // Both the flag and the URL must be present for GAMDL to use the wrapper.
    if settings.use_wrapper {
        lines.push("use-wrapper".to_string());
        lines.push(format!(
            "wrapper-account-url = {}",
            settings.wrapper_account_url
        ));
    }

    // Join all lines with newlines and add a trailing newline.
    // The trailing newline ensures the file ends properly for configparser.
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
    // Attempt to resolve the platform's standard audio/music directory first:
    // - macOS: ~/Music
    // - Windows: C:\Users\{user}\Music
    // - Linux: ~/Music (XDG_MUSIC_DIR, typically)
    // Falls back to the home directory if no audio directory is configured.
    // Ref: https://docs.rs/dirs/latest/dirs/fn.audio_dir.html
    let music_dir = dirs::audio_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Could not determine home directory".to_string())?;

    // Append "Apple Music" subdirectory to keep downloads organized.
    // Example: ~/Music/Apple Music/
    let output_path = music_dir.join("Apple Music");

    // Convert to a string for storage in settings. This can fail if the path
    // contains non-UTF-8 characters, which is extremely rare on modern systems.
    output_path
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Failed to convert output path to string".to_string())
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create default settings for testing
    fn default_settings() -> AppSettings {
        AppSettings::default()
    }

    // ----------------------------------------------------------
    // settings_to_ini: section header
    // ----------------------------------------------------------

    #[test]
    fn ini_starts_with_gamdl_section() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(ini.starts_with("[gamdl]\n"));
    }

    #[test]
    fn ini_ends_with_newline() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(ini.ends_with('\n'));
    }

    // ----------------------------------------------------------
    // settings_to_ini: audio codec
    // ----------------------------------------------------------

    #[test]
    fn ini_contains_default_song_codec() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("song-codec = alac"));
    }

    #[test]
    fn ini_uses_custom_song_codec() {
        let mut settings = default_settings();
        settings.default_song_codec =
            crate::models::gamdl_options::SongCodec::Aac;
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("song-codec = aac"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: video resolution
    // ----------------------------------------------------------

    #[test]
    fn ini_contains_video_resolution() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("music-video-resolution = 2160p"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: lyrics
    // ----------------------------------------------------------

    #[test]
    fn ini_contains_lyrics_format() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("synced-lyrics-format = lrc"));
    }

    #[test]
    fn ini_omits_no_synced_lyrics_when_false() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("no-synced-lyrics"));
    }

    #[test]
    fn ini_includes_no_synced_lyrics_when_true() {
        let mut settings = default_settings();
        settings.no_synced_lyrics = true;
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("no-synced-lyrics"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: cover art
    // ----------------------------------------------------------

    #[test]
    fn ini_includes_save_cover_when_true() {
        let settings = default_settings(); // save_cover defaults to true
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("save-cover"));
    }

    #[test]
    fn ini_omits_save_cover_when_false() {
        let mut settings = default_settings();
        settings.save_cover = false;
        let ini = settings_to_ini(&settings);
        // Should not contain bare "save-cover" (but may contain "cover-format")
        assert!(!ini.lines().any(|l| l.trim() == "save-cover"));
    }

    #[test]
    fn ini_formats_cover_size_as_wxh() {
        let settings = default_settings(); // cover_size defaults to 1200
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("cover-size = 1200x1200"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: optional paths
    // ----------------------------------------------------------

    #[test]
    fn ini_omits_cookies_path_when_none() {
        let settings = default_settings(); // cookies_path defaults to None
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("cookies-path"));
    }

    #[test]
    fn ini_includes_cookies_path_when_set() {
        let mut settings = default_settings();
        settings.cookies_path = Some("/home/user/cookies.txt".to_string());
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("cookies-path = /home/user/cookies.txt"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: output path
    // ----------------------------------------------------------

    #[test]
    fn ini_omits_output_path_when_empty() {
        let settings = default_settings(); // output_path defaults to ""
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("output-path"));
    }

    #[test]
    fn ini_includes_output_path_when_set() {
        let mut settings = default_settings();
        settings.output_path = "/tmp/music".to_string();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("output-path = /tmp/music"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: boolean flags
    // ----------------------------------------------------------

    #[test]
    fn ini_omits_overwrite_when_false() {
        let settings = default_settings(); // overwrite defaults to false
        let ini = settings_to_ini(&settings);
        assert!(!ini.lines().any(|l| l.trim() == "overwrite"));
    }

    #[test]
    fn ini_includes_overwrite_when_true() {
        let mut settings = default_settings();
        settings.overwrite = true;
        let ini = settings_to_ini(&settings);
        assert!(ini.lines().any(|l| l.trim() == "overwrite"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: wrapper
    // ----------------------------------------------------------

    #[test]
    fn ini_omits_wrapper_when_disabled() {
        let settings = default_settings(); // use_wrapper defaults to false
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("use-wrapper"));
        assert!(!ini.contains("wrapper-account-url"));
    }

    #[test]
    fn ini_includes_wrapper_when_enabled() {
        let mut settings = default_settings();
        settings.use_wrapper = true;
        settings.wrapper_account_url = "http://localhost:9999".to_string();
        let ini = settings_to_ini(&settings);
        assert!(ini.lines().any(|l| l.trim() == "use-wrapper"));
        assert!(ini.contains("wrapper-account-url = http://localhost:9999"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: templates
    // ----------------------------------------------------------

    #[test]
    fn ini_includes_album_folder_template() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("album-folder-template = {album_artist}/{album}"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: language
    // ----------------------------------------------------------

    #[test]
    fn ini_includes_language() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("language = en-US"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: truncate
    // ----------------------------------------------------------

    #[test]
    fn ini_omits_truncate_when_none() {
        let settings = default_settings(); // truncate defaults to None
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("truncate"));
    }

    #[test]
    fn ini_includes_truncate_when_set() {
        let mut settings = default_settings();
        settings.truncate = Some(200);
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("truncate = 200"));
    }

    // ----------------------------------------------------------
    // get_default_output_path
    // ----------------------------------------------------------

    #[test]
    fn default_output_path_ends_with_apple_music() {
        // This test should work on all platforms since dirs::audio_dir()
        // or dirs::home_dir() should return a valid path.
        let result = get_default_output_path();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("Apple Music"));
    }

    // ----------------------------------------------------------
    // AppSettings serde roundtrip
    // ----------------------------------------------------------

    #[test]
    fn settings_serde_roundtrip() {
        let settings = default_settings();
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();

        // Compare a representative sample of fields
        assert_eq!(
            deserialized.default_song_codec,
            settings.default_song_codec
        );
        assert_eq!(deserialized.output_path, settings.output_path);
        assert_eq!(deserialized.save_cover, settings.save_cover);
        assert_eq!(deserialized.cover_size, settings.cover_size);
        assert_eq!(deserialized.language, settings.language);
        assert_eq!(deserialized.fallback_enabled, settings.fallback_enabled);
        assert_eq!(
            deserialized.music_fallback_chain,
            settings.music_fallback_chain
        );
    }
}
