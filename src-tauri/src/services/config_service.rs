// Copyright (c) 2026 MeedyaDL
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
//   "default_song_codec": "alac",      song_codec = alac
//   "save_cover": true,         -->    save_cover = true
//   "cover_format": "raw",             cover_format = raw
//   "theme": "dark",                   (not synced - GUI-only setting)
//   ...                                ...
// }
// ```
//
// ## Key Names: Underscores, Not Hyphens
//
// GAMDL's config reader uses Python Click's `param.name` to look up keys.
// Click converts CLI flag names (e.g., `--save-cover`) to Python identifiers
// with underscores (`save_cover`). Since `configparser.ConfigParser` looks
// up keys by exact `param.name`, config keys MUST use underscores.
//
// ## Boolean Flags: `key = true`, Not Bare Keys
//
// GAMDL uses `configparser.ConfigParser(interpolation=None)` WITHOUT
// `allow_no_value=True`. Bare keys (e.g., `save_cover` on a line by itself)
// cause `configparser.ParsingError`. Boolean flags must use `key = true`.
//
// The JSON file is the single source of truth, managed by the React frontend
// via Tauri commands (load_settings, save_settings). When settings are saved,
// `sync_gamdl_config()` writes a config.ini that GAMDL can read natively.
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
use crate::models::gamdl_options::SongCodec;
use crate::models::settings::AppSettings;
// Platform utilities for resolving the app data directory and config file paths
// across macOS, Windows, and Linux.
use crate::utils::platform;

// ============================================================
// Settings file integrity check (SHA-256)
// ============================================================

/// Compute SHA-256 hex digest of the given string content.
fn sha256_hex(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Write a checksum companion file alongside the settings file.
/// The checksum file is `settings.json.sha256` containing the hex digest.
fn write_settings_checksum(settings_path: &std::path::Path, json_content: &str) {
    let checksum_path = settings_path.with_extension("json.sha256");
    let checksum = sha256_hex(json_content);
    if let Err(e) = std::fs::write(&checksum_path, &checksum) {
        log::warn!("Failed to write settings checksum: {e}");
    }
}

/// Verify the settings file integrity against its companion checksum file.
/// Returns `true` if the checksum matches or if no checksum file exists
/// (backwards compatibility with pre-checksum settings).
/// Returns `false` if the checksum file exists but doesn't match.
fn verify_settings_checksum(settings_path: &std::path::Path, json_content: &str) -> bool {
    let checksum_path = settings_path.with_extension("json.sha256");
    if !checksum_path.exists() {
        // No checksum file — settings were written by an older version.
        // Accept them (backwards compatible) but write a checksum for next time.
        write_settings_checksum(settings_path, json_content);
        return true;
    }
    match std::fs::read_to_string(&checksum_path) {
        Ok(stored) => {
            let computed = sha256_hex(json_content);
            if stored.trim() == computed {
                true
            } else {
                log::warn!(
                    "Settings file integrity check failed: checksum mismatch \
                     (stored={}, computed={}). File may have been modified externally.",
                    &stored.trim()[..8.min(stored.trim().len())],
                    &computed[..8],
                );
                false
            }
        }
        Err(e) => {
            log::warn!("Failed to read settings checksum file: {e}");
            true // Can't verify — accept the settings
        }
    }
}

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
/// # Errors
///
/// Returns `Err(String)` if the settings file exists but cannot be parsed.
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

    let mut settings = if settings_path.exists() {
        // Read the entire file into a string. This is synchronous (blocking I/O)
        // since settings are loaded during Tauri command handlers which run on
        // the Tokio thread pool and can tolerate brief blocking.
        let contents = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings file: {e}"))?;

        // Verify integrity: check the SHA-256 checksum companion file.
        // A mismatch means the file was modified externally (manual edit,
        // import from another machine, or tampering). We log a warning
        // but still load the settings — the user may have intentionally
        // edited the file.
        if !verify_settings_checksum(&settings_path, &contents) {
            log::warn!("Settings file may have been modified externally — loading anyway");
        }

        // Deserialize the JSON into AppSettings. serde_json handles:
        // - Missing fields: filled with #[serde(default)] values
        // - Extra fields: silently ignored (forward compatibility)
        // - Type mismatches: returns a descriptive parse error
        //
        // If parsing fails (e.g., a field type changed between versions),
        // fall back to defaults rather than returning an error. This prevents
        // settings from appearing "reset" to the user — they get defaults
        // for the broken field(s) while other fields are preserved by serde(default).
        // Ref: https://docs.rs/serde_json/latest/serde_json/fn.from_str.html
        match serde_json::from_str(&contents) {
            Ok(parsed) => parsed,
            Err(e) => {
                log::error!(
                    "Settings file corrupted or incompatible — using defaults. \
                     Error: {e}. File: {}. This typically happens when a field type \
                     changed between app versions. The settings file is preserved on \
                     disk; only the in-memory state uses defaults.",
                    settings_path.display()
                );
                AppSettings::default()
            }
        }
    } else {
        // First run: no settings file exists yet. Return the default settings
        // which are defined via #[derive(Default)] on AppSettings.
        // The defaults provide sensible out-of-the-box values for all settings.
        log::info!("No settings file found, using defaults");
        AppSettings::default()
    };

    // Version-aware verbose logging reset.
    //
    // The current app version determines whether verbose logging persists:
    //   - Pre-release (v0.x.x): verbose logging is preserved across restarts
    //     because it's critical for debugging pre-release issues.
    //   - Full release (v1.0.0+): verbose logging is reset to false every
    //     startup to prevent sensitive data exposure in production.
    //   - Upgrade from pre-release → full release: verbose logging is reset
    //     on the first launch of the new full release.
    //
    // The `last_seen_version` field tracks the previous app version so we
    // can detect version transitions.
    let current_version = env!("CARGO_PKG_VERSION");
    let is_prerelease = current_version.starts_with("0.");

    if is_prerelease {
        // Pre-release: preserve verbose_activity_log setting as-is.
        // This helps developers and testers keep verbose logging enabled
        // across restarts for continuous debugging.
        if settings.verbose_activity_log {
            log::info!(
                "Pre-release build (v{current_version}): preserving verbose_activity_log=true"
            );
        }
    } else {
        // Full release: always reset verbose logging to disabled on startup.
        // Verbose logging can expose sensitive data (cookies, auth tokens, API
        // responses, MusicKit credentials) so it must not persist across sessions.
        // Users can re-enable it per-session in Settings > Advanced > Diagnostics.
        if settings.verbose_activity_log {
            log::info!(
                "Full release (v{current_version}): resetting verbose_activity_log to false"
            );
            settings.verbose_activity_log = false;
        }
    }

    // Track version changes for first-load notices and transition logic.
    // The frontend reads `last_seen_version` to detect when a new version is
    // launched for the first time (e.g., to show a pre-release warning modal).
    let previous_version = settings.last_seen_version.clone();
    if previous_version != current_version {
        log::info!(
            "Version changed: {} → {}",
            if previous_version.is_empty() { "(first run)" } else { &previous_version },
            current_version
        );
        settings.last_seen_version = current_version.to_string();
        // Persist the updated last_seen_version immediately so subsequent
        // loads (e.g., from load_settings_from_default_path) see the new value.
        if let Err(e) = save_settings(app, &settings) {
            log::warn!("Failed to persist last_seen_version update: {e}");
        }
    }

    // Always regenerate GAMDL's config.ini on load to ensure the on-disk INI
    // matches the current code's key format (underscores, `key = true` booleans).
    // Without this, upgrading from a version that wrote hyphens/bare keys would
    // leave a stale config.ini that GAMDL's configparser rejects.
    if let Err(e) = sync_gamdl_config(app, &settings) {
        log::warn!("Failed to sync config.ini on load: {e}");
    }

    // Sync the verbose activity log flag to the global atomic.
    crate::utils::activity_log::set_verbose_logging(settings.verbose_activity_log);

    Ok(settings)
}

/// Loads settings from the default platform path without requiring a Tauri
/// `AppHandle`. Used during early startup (before Tauri is fully initialised)
/// to read settings like `sentry_enabled` for tracing/Sentry configuration.
///
/// Falls back to `AppSettings::default()` if the file is missing or unreadable.
/// Does NOT sync to GAMDL's config.ini (no `AppHandle` available).
pub fn load_settings_from_default_path() -> Result<AppSettings, String> {
    let settings_dir = dirs::data_dir()
        .map(|d| d.join("io.github.meedyadl"))
        .ok_or_else(|| "Cannot determine app data directory".to_string())?;
    let settings_path = settings_dir.join("settings.json");

    if settings_path.exists() {
        let contents = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings file: {e}"))?;
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse settings file: {e}"))
    } else {
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
///
/// # Errors
///
/// Returns `Err(String)` if directory creation, JSON serialization, or
/// file write fails.
pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let settings_path = platform::get_app_data_dir(app).join("settings.json");

    // Ensure the parent directory exists (important on first run or after data dir deletion)
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create settings directory: {e}"))?;
    }

    // Serialize to pretty-printed JSON for human readability.
    // Pretty printing adds indentation, making the file easy to inspect and debug.
    // Ref: https://docs.rs/serde_json/latest/serde_json/fn.to_string_pretty.html
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;

    // Atomic write: write to a temp file in the same directory, then rename.
    // This prevents corruption if the process crashes or loses power mid-write,
    // because rename() is atomic on all major filesystems (APFS, ext4, NTFS).
    // See: https://github.com/MWBMPartners/MeedyaDL/issues/230
    let temp_path = settings_path.with_extension("json.tmp");
    std::fs::write(&temp_path, &json)
        .map_err(|e| format!("Failed to write temp settings file: {e}"))?;
    std::fs::rename(&temp_path, &settings_path)
        .map_err(|e| format!("Failed to rename temp settings file: {e}"))?;

    // Write integrity checksum alongside the settings file.
    // Used by load_settings() to detect tampering or corruption.
    write_settings_checksum(&settings_path, &json);

    log::info!("Settings saved to {}", settings_path.display());

    // Sync relevant settings to GAMDL's config.ini (the derived config).
    // This is a best-effort operation: if it fails, the JSON save still succeeds.
    // GAMDL will still work via CLI flags; the config.ini is a convenience.
    if let Err(e) = sync_gamdl_config(app, settings) {
        log::warn!("Failed to sync settings to GAMDL config: {e}");
        // Don't fail the save operation — the JSON settings are the source of truth
    }

    // Sync the verbose activity log flag immediately so it takes effect
    // without requiring an app restart.
    crate::utils::activity_log::set_verbose_logging(settings.verbose_activity_log);

    Ok(())
}

/// Syncs relevant `AppSettings` fields to GAMDL's config.ini file.
///
/// GAMDL reads its configuration from an INI file. The GUI manages all
/// settings in JSON, but we also write a config.ini so that GAMDL CLI
/// commands use the same settings. We pass --config-path to GAMDL to
/// point it at our managed config.ini.
///
/// INI format example:
/// ```ini
/// [gamdl]
/// cookies_path = /path/to/cookies.txt
/// song_codec = alac
/// save_cover = true
/// cover_format = raw
/// ```
///
/// Key names use underscores (matching GAMDL's Click parameter names).
/// Boolean flags use `key = true` (omitted when false).
///
/// # Arguments
/// * `app` - The Tauri app handle
/// * `settings` - The application settings to convert and write
///
/// Syncs settings to GAMDL's config.ini. Public so `gamdl_service` can
/// call it immediately before each invocation to ensure fresh config.
pub fn sync_gamdl_config(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    // Resolve the GAMDL config.ini path: {app_data_dir}/gamdl/config.ini
    // This path is passed to GAMDL via --config-path in gamdl_service::build_gamdl_command().
    let config_path = platform::get_gamdl_config_path(app);

    // Ensure the GAMDL data directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create GAMDL config directory: {e}"))?;
    }

    // Build the INI file content by converting AppSettings fields to INI key-value pairs.
    // Only settings that GAMDL reads from its config file are included.
    let ini_content = settings_to_ini(settings);

    // Write the config file, completely replacing any previous content.
    // GAMDL parses this file using Python's configparser module.
    // Ref: https://docs.python.org/3/library/configparser.html
    std::fs::write(&config_path, ini_content)
        .map_err(|e| format!("Failed to write GAMDL config: {e}"))?;

    log::info!("GAMDL config synced to {}", config_path.display());
    Ok(())
}

/// Converts `AppSettings` into GAMDL's INI config format.
///
/// Only includes settings that GAMDL actually reads from its config file.
/// Key names use underscores to match GAMDL's Click parameter names
/// (e.g., `save_cover`, not `save-cover`). Boolean flags are written as
/// `key = true` (never bare keys) since GAMDL's configparser doesn't
/// use `allow_no_value=True`.
///
/// # Arguments
/// * `settings` - The application settings to convert
///
/// # Returns
/// The INI file content as a string
/// Sanitize a string value for safe inclusion in an INI file.
/// Strips newline and carriage return characters that could inject
/// additional INI keys when parsed by Python's configparser.
/// See: https://github.com/MWBMPartners/MeedyaDL/issues/226
fn sanitize_ini_value(value: &str) -> String {
    value.replace(['\n', '\r'], "")
}

fn settings_to_ini(settings: &AppSettings) -> String {
    let mut lines = Vec::new();

    // Section header required by Python's configparser.
    // GAMDL expects all settings under a [gamdl] section.
    // Ref: https://github.com/glomatico/gamdl#configuration
    lines.push("[gamdl]".to_string());

    // Build INI content from each settings section.
    ini_auth_section(&mut lines, settings);
    ini_audio_section(&mut lines, settings);
    ini_video_section(&mut lines, settings);
    ini_lyrics_section(&mut lines, settings);
    ini_cover_section(&mut lines, settings);
    ini_output_section(&mut lines, settings);
    ini_metadata_section(&mut lines, settings);
    ini_template_section(&mut lines, settings);
    ini_tool_path_section(&mut lines, settings);
    ini_advanced_section(&mut lines, settings);

    // Join all lines with newlines and add a trailing newline.
    // The trailing newline ensures the file ends properly for configparser.
    lines.join("\n") + "\n"
}

/// Appends authentication-related INI key-value pairs (cookies path).
fn ini_auth_section(lines: &mut Vec<String>, settings: &AppSettings) {
    // The cookies path is the most important setting — GAMDL needs it to
    // authenticate with Apple Music. Without cookies, downloads will fail.
    // The cookies.txt file is extracted from a browser session (Netscape format).
    if let Some(ref path) = settings.cookies_path {
        lines.push(format!("cookies_path = {}", sanitize_ini_value(path)));
    }
}

/// Appends audio quality INI key-value pairs (song codec, codec priority).
fn ini_audio_section(lines: &mut Vec<String>, settings: &AppSettings) {
    // Write single codec for GAMDL < 2.9.1 compatibility
    lines.push(format!(
        "song_codec = {}",
        settings.default_song_codec.to_cli_string()
    ));

    // Also write the full fallback chain as song_codec_priority for GAMDL >= 2.9.1.
    // GAMDL 2.9.1+ reads this key and ignores song_codec when it's present.
    // GAMDL < 2.9.1 silently ignores the unknown key.
    if settings.fallback_enabled && !settings.music_fallback_chain.is_empty() {
        let priority: String = settings
            .music_fallback_chain
            .iter()
            .map(SongCodec::to_cli_string)
            .collect::<Vec<&str>>()
            .join(",");
        lines.push(format!("song_codec_priority = {priority}"));
    }
}

/// Appends video quality INI key-value pairs (resolution, codec priority, remux format).
fn ini_video_section(lines: &mut Vec<String>, settings: &AppSettings) {
    // Maps to GAMDL's --music-video-resolution flag. Values like "1080p", "4k", etc.
    lines.push(format!(
        "music_video_resolution = {}",
        settings.default_video_resolution.to_cli_string()
    ));
    // Video codec priority is a comma-separated list (e.g., "h265,h264").
    // Only written if the user has set a preference.
    if !settings.default_video_codec_priority.is_empty() {
        lines.push(format!(
            "music_video_codec_priority = {}",
            settings.default_video_codec_priority
        ));
    }
    // Video remux format (e.g., "mkv", "mp4"). Only written if set.
    if !settings.default_video_remux_format.is_empty() {
        lines.push(format!(
            "music_video_remux_format = {}",
            settings.default_video_remux_format
        ));
    }
}

/// Appends lyrics-related INI key-value pairs (format, `no_synced_lyrics` flag).
fn ini_lyrics_section(lines: &mut Vec<String>, settings: &AppSettings) {
    // Synced lyrics format (e.g., "lrc", "srt").
    lines.push(format!(
        "synced_lyrics_format = {}",
        settings.synced_lyrics_format.to_cli_string()
    ));
    // Boolean flag: `key = true` when enabled, omitted when false (GAMDL defaults to false).
    if settings.no_synced_lyrics {
        lines.push("no_synced_lyrics = true".to_string());
    }
}

/// Appends cover art INI key-value pairs (`save_cover`, format, size).
fn ini_cover_section(lines: &mut Vec<String>, settings: &AppSettings) {
    // Boolean flag: when true, GAMDL saves cover art as a separate file.
    if settings.save_cover {
        lines.push("save_cover = true".to_string());
    }
    // Cover format (e.g., "raw" for original, "jpg", "png").
    lines.push(format!(
        "cover_format = {}",
        settings.cover_format.to_cli_string()
    ));
    // Cover size as a single integer (pixels). GAMDL uses square covers.
    lines.push(format!("cover_size = {}", settings.cover_size));
}

/// Appends output/path INI key-value pairs (`output_path`, `temp_path`, overwrite, truncate).
fn ini_output_section(lines: &mut Vec<String>, settings: &AppSettings) {
    // Output directory for downloaded files. Only written if non-empty.
    if !settings.output_path.is_empty() {
        lines.push(format!("output_path = {}", sanitize_ini_value(&settings.output_path)));
    }
    // Temp directory for intermediate files. If the user left this empty,
    // resolve to a "MeedyaDL" subdirectory within the OS temp directory so
    // GAMDL doesn't default to "." (unwritable from /Applications on macOS).
    if settings.temp_path.is_empty() {
        lines.push(format!(
            "temp_path = {}",
            sanitize_ini_value(&std::env::temp_dir().join("MeedyaDL").to_string_lossy())
        ));
    } else {
        lines.push(format!("temp_path = {}", sanitize_ini_value(&settings.temp_path)));
    }
    // Boolean flag: when true, existing files are overwritten without prompting.
    if settings.overwrite {
        lines.push("overwrite = true".to_string());
    }
    // Truncate file/folder names to this many characters (avoids filesystem limits).
    if let Some(truncate) = settings.truncate {
        lines.push(format!("truncate = {truncate}"));
    }
}

/// Appends metadata INI key-value pairs (language, storefront, `fetch_extra_tags`, `artist_auto_select`).
fn ini_metadata_section(lines: &mut Vec<String>, settings: &AppSettings) {
    // Language code for metadata (e.g., "en-US", "ja-JP").
    // Affects how track/album names are retrieved from Apple Music.
    lines.push(format!("language = {}", settings.language));

    // Storefront code for Apple Music API requests (e.g., "gb", "us", "jp").
    // Required by GAMDL >= 2.9.3 which no longer auto-detects the storefront.
    //
    // Logic:
    //   - If the user has set an explicit storefront → use it as-is
    //   - If empty (auto-detect) → derive from language region code
    //     (e.g., "en-GB" → "gb", "ja-JP" → "jp", fallback "us")
    let storefront = if !settings.storefront.is_empty() {
        settings.storefront.to_ascii_lowercase()
    } else {
        settings
            .language
            .split('-')
            .next_back()
            .filter(|s| s.len() == 2)
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_else(|| "us".to_string())
    };
    lines.push(format!("storefront = {storefront}"));
    // Boolean flag: when true, GAMDL fetches extra metadata tags
    // (normalization info, smooth playback data, etc.) from Apple Music.
    if settings.fetch_extra_tags {
        lines.push("fetch_extra_tags = true".to_string());
    }
    // Artist auto-selection mode (GAMDL >= 2.9.1). Controls which content
    // type is automatically downloaded when the user provides an artist URL.
    // Only written when explicitly set; omitted otherwise to let GAMDL use
    // its own default behavior.
    if let Some(ref mode) = settings.artist_auto_select {
        lines.push(format!("artist_auto_select = {}", mode.to_cli_string()));
    }
}

/// Appends output path template INI key-value pairs.
fn ini_template_section(lines: &mut Vec<String>, settings: &AppSettings) {
    // Output path templates use Python format strings with metadata placeholders.
    // Example: "{album_artist}/{album}" -> "Taylor Swift/1989 (Taylor's Version)"
    // Ref: https://github.com/glomatico/gamdl#output-path-template
    if !settings.album_folder_template.is_empty() {
        lines.push(format!(
            "album_folder_template = {}",
            sanitize_ini_value(&settings.album_folder_template)
        ));
    }
    if !settings.compilation_folder_template.is_empty() {
        lines.push(format!(
            "compilation_folder_template = {}",
            sanitize_ini_value(&settings.compilation_folder_template)
        ));
    }
    if !settings.no_album_folder_template.is_empty() {
        lines.push(format!(
            "no_album_folder_template = {}",
            sanitize_ini_value(&settings.no_album_folder_template)
        ));
    }
    if !settings.single_disc_file_template.is_empty() {
        lines.push(format!(
            "single_disc_file_template = {}",
            sanitize_ini_value(&settings.single_disc_file_template)
        ));
    }
    if !settings.multi_disc_file_template.is_empty() {
        lines.push(format!(
            "multi_disc_file_template = {}",
            sanitize_ini_value(&settings.multi_disc_file_template)
        ));
    }
    if !settings.no_album_file_template.is_empty() {
        lines.push(format!(
            "no_album_file_template = {}",
            sanitize_ini_value(&settings.no_album_file_template)
        ));
    }
    if !settings.playlist_file_template.is_empty() {
        lines.push(format!(
            "playlist_file_template = {}",
            sanitize_ini_value(&settings.playlist_file_template)
        ));
    }
}

/// Appends custom tool path INI key-value pairs (user-specified paths only).
fn ini_tool_path_section(lines: &mut Vec<String>, settings: &AppSettings) {
    // These are tool paths the user explicitly configured in Settings.
    // Note: managed tool paths (installed by dependency_manager.rs) are injected
    // separately via gamdl_service::inject_tool_paths() at command build time.
    if let Some(ref path) = settings.ffmpeg_path {
        lines.push(format!("ffmpeg_path = {}", sanitize_ini_value(path)));
    }
    if let Some(ref path) = settings.mp4decrypt_path {
        lines.push(format!("mp4decrypt_path = {}", sanitize_ini_value(path)));
    }
    if let Some(ref path) = settings.mp4box_path {
        lines.push(format!("mp4box_path = {}", sanitize_ini_value(path)));
    }
    if let Some(ref path) = settings.nm3u8dlre_path {
        lines.push(format!("nm3u8dlre_path = {}", sanitize_ini_value(path)));
    }
}

/// Appends advanced settings INI key-value pairs (wrapper mode).
fn ini_advanced_section(lines: &mut Vec<String>, settings: &AppSettings) {
    // Wrapper mode uses an alternative authentication method via a web service.
    // Both the flag and the URL must be present for GAMDL to use the wrapper.
    if settings.use_wrapper {
        lines.push("use_wrapper = true".to_string());
        lines.push(format!(
            "wrapper_account_url = {}",
            sanitize_ini_value(&settings.wrapper_account_url)
        ));
    }
}

/// Resolves the default output path for downloaded music.
///
/// Uses the platform's standard Music/Audio directory with an
/// "Apple Music" subdirectory appended.
///
/// # Errors
///
/// Returns `Err(String)` if the user's home directory cannot be determined.
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
        .map(std::string::ToString::to_string)
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
        assert!(ini.contains("song_codec = alac"));
    }

    #[test]
    fn ini_uses_custom_song_codec() {
        let mut settings = default_settings();
        settings.default_song_codec = crate::models::gamdl_options::SongCodec::Aac;
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("song_codec = aac"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: video resolution
    // ----------------------------------------------------------

    #[test]
    fn ini_contains_video_resolution() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("music_video_resolution = 2160p"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: lyrics
    // ----------------------------------------------------------

    #[test]
    fn ini_contains_lyrics_format() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("synced_lyrics_format = ttml"));
    }

    #[test]
    fn ini_omits_no_synced_lyrics_when_false() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("no_synced_lyrics"));
    }

    #[test]
    fn ini_includes_no_synced_lyrics_when_true() {
        let mut settings = default_settings();
        settings.no_synced_lyrics = true;
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("no_synced_lyrics = true"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: cover art
    // ----------------------------------------------------------

    #[test]
    fn ini_includes_save_cover_when_true() {
        let settings = default_settings(); // save_cover defaults to true
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("save_cover = true"));
    }

    #[test]
    fn ini_omits_save_cover_when_false() {
        let mut settings = default_settings();
        settings.save_cover = false;
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("save_cover"));
    }

    #[test]
    fn ini_formats_cover_size_as_integer() {
        let settings = default_settings(); // cover_size defaults to 10000
        let ini = settings_to_ini(&settings);
        // GAMDL expects a single integer (pixels), not WxH.
        assert!(ini.contains("cover_size = 10000"));
        assert!(!ini.contains("cover_size = 10000x10000"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: optional paths
    // ----------------------------------------------------------

    #[test]
    fn ini_omits_cookies_path_when_none() {
        let settings = default_settings(); // cookies_path defaults to None
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("cookies_path"));
    }

    #[test]
    fn ini_includes_cookies_path_when_set() {
        let mut settings = default_settings();
        settings.cookies_path = Some("/home/user/cookies.txt".to_string());
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("cookies_path = /home/user/cookies.txt"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: output path
    // ----------------------------------------------------------

    #[test]
    fn ini_omits_output_path_when_empty() {
        let settings = default_settings(); // output_path defaults to ""
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("output_path"));
    }

    #[test]
    fn ini_includes_output_path_when_set() {
        let mut settings = default_settings();
        settings.output_path = "/tmp/music".to_string();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("output_path = /tmp/music"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: boolean flags
    // ----------------------------------------------------------

    #[test]
    fn ini_omits_overwrite_when_false() {
        let settings = default_settings(); // overwrite defaults to false
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("overwrite"));
    }

    #[test]
    fn ini_includes_overwrite_when_true() {
        let mut settings = default_settings();
        settings.overwrite = true;
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("overwrite = true"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: wrapper
    // ----------------------------------------------------------

    #[test]
    fn ini_omits_wrapper_when_disabled() {
        let settings = default_settings(); // use_wrapper defaults to false
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("use_wrapper"));
        assert!(!ini.contains("wrapper_account_url"));
    }

    #[test]
    fn ini_includes_wrapper_when_enabled() {
        let mut settings = default_settings();
        settings.use_wrapper = true;
        settings.wrapper_account_url = "http://localhost:9999".to_string();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("use_wrapper = true"));
        assert!(ini.contains("wrapper_account_url = http://localhost:9999"));
    }

    // ----------------------------------------------------------
    // settings_to_ini: templates
    // ----------------------------------------------------------

    #[test]
    fn ini_includes_album_folder_template() {
        let settings = default_settings();
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("album_folder_template = {album_artist}/{album}"));
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

    #[test]
    fn ini_includes_storefront_auto_from_language() {
        let settings = default_settings(); // language = "en-US", storefront = ""
        let ini = settings_to_ini(&settings);
        assert!(
            ini.contains("storefront = us"),
            "Should auto-detect 'us' from 'en-US': {ini}"
        );
    }

    #[test]
    fn ini_uses_explicit_storefront_when_set() {
        let mut settings = default_settings();
        settings.storefront = "gb".to_string();
        let ini = settings_to_ini(&settings);
        assert!(
            ini.contains("storefront = gb"),
            "Should use explicit 'gb': {ini}"
        );
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
    // settings_to_ini: key format validation
    // ----------------------------------------------------------

    #[test]
    fn ini_uses_underscores_not_hyphens() {
        let mut settings = default_settings();
        settings.save_cover = true;
        settings.overwrite = true;
        settings.fetch_extra_tags = true;
        settings.output_path = "/tmp/music".to_string();
        let ini = settings_to_ini(&settings);
        // No hyphens in key names
        assert!(!ini.contains("save-cover"));
        assert!(!ini.contains("song-codec"));
        assert!(!ini.contains("output-path"));
        assert!(!ini.contains("cover-size"));
        assert!(!ini.contains("fetch-extra-tags"));
        // All use underscores
        assert!(ini.contains("save_cover = true"));
        assert!(ini.contains("song_codec"));
        assert!(ini.contains("output_path"));
        assert!(ini.contains("cover_size"));
        assert!(ini.contains("fetch_extra_tags = true"));
    }

    #[test]
    fn ini_booleans_use_equals_true_not_bare_keys() {
        let mut settings = default_settings();
        settings.save_cover = true;
        settings.overwrite = true;
        settings.no_synced_lyrics = true;
        settings.fetch_extra_tags = true;
        settings.use_wrapper = true;
        settings.wrapper_account_url = "http://test".to_string();
        let ini = settings_to_ini(&settings);
        // All booleans must have " = true"
        assert!(ini.contains("save_cover = true"));
        assert!(ini.contains("overwrite = true"));
        assert!(ini.contains("no_synced_lyrics = true"));
        assert!(ini.contains("fetch_extra_tags = true"));
        assert!(ini.contains("use_wrapper = true"));
        // No bare keys (lines that are just the key with no " = ")
        for line in ini.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('[') {
                continue;
            }
            assert!(
                trimmed.contains(" = "),
                "Bare key found (no ' = '): '{}'",
                trimmed
            );
        }
    }

    // ----------------------------------------------------------
    // settings_to_ini: song_codec_priority
    // ----------------------------------------------------------

    #[test]
    fn ini_contains_song_codec_priority_when_fallback_enabled() {
        let settings = default_settings(); // fallback_enabled is true by default
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("song_codec_priority ="));
        // Should contain the full default fallback chain
        assert!(ini.contains("alac"));
    }

    #[test]
    fn ini_omits_song_codec_priority_when_fallback_disabled() {
        let mut settings = default_settings();
        settings.fallback_enabled = false;
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("song_codec_priority ="));
    }

    // ----------------------------------------------------------
    // settings_to_ini: artist_auto_select
    // ----------------------------------------------------------

    #[test]
    fn ini_omits_artist_auto_select_when_none() {
        let settings = default_settings(); // artist_auto_select defaults to None
        let ini = settings_to_ini(&settings);
        assert!(!ini.contains("artist_auto_select"));
    }

    #[test]
    fn ini_includes_artist_auto_select_when_set() {
        let mut settings = default_settings();
        settings.artist_auto_select =
            Some(crate::models::gamdl_options::ArtistAutoSelect::AllAlbums);
        let ini = settings_to_ini(&settings);
        assert!(ini.contains("artist_auto_select = all-albums"));
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
        assert_eq!(deserialized.default_song_codec, settings.default_song_codec);
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
