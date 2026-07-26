// Copyright (c) 2026 MeedyaSuite
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
// | Rust Command                  | TypeScript Function              | Line |
// |-------------------------------|----------------------------------|------|
// | get_settings                  | getSettings()                    | ~75  |
// | save_settings                 | saveSettings(settings)           | ~80  |
// | has_embedded_acoustid_key     | hasEmbeddedAcoustidKey()         | ~83  |
// | validate_cookies_file         | validateCookiesFile(path)        | ~85  |
// | check_cookies_before_download | checkCookiesBeforeDownload()     | ~88  |
// | check_internet_before_download| checkInternetBeforeDownload()    | ~91  |
// | get_default_output_path       | getDefaultOutputPath()           | ~95  |
// | test_wrapper_connection       | testWrapperConnection(url)       | ~100 |
//
// ## References
//
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - Netscape cookie format: https://curl.se/docs/http-cookies.html

// serde::Serialize is required for CookieValidation which is returned to the frontend.
// serde::Deserialize is required for SettingsExportFile which is read from user-provided files.
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
// AppHandle for resolving app data directory paths (settings.json location).
use tauri::{AppHandle, Manager};

// AppSettings is the Rust struct representing the full application settings.
// It implements both Serialize (for returning to frontend) and Deserialize
// (for accepting from frontend when saving).
use crate::models::settings::AppSettings;
// config_service handles the actual file I/O: reading/writing settings.json
// and syncing to GAMDL's config.ini file.
use crate::services::config_service;
use crate::utils::activity_log::{emit_app_log, emit_verbose_app_log};

/// Result of validating a Netscape-format cookies file.
///
/// Provides detailed information about the cookies found and their validity.
/// This is used by the frontend's cookie file picker to give the user
/// immediate feedback about whether their exported cookies file is usable.
///
/// The Netscape cookie format is a tab-separated text format originally
/// defined by Netscape Navigator and still used by curl, wget, and browser
/// cookie export extensions.
/// See: <https://curl.se/docs/http-cookies.html>
///
/// Implements `Serialize` for Tauri IPC serialization to JSON.
#[derive(Debug, Clone, Serialize)]
pub struct CookieValidation {
    /// Whether the file is a valid Netscape cookie file (has at least one parseable entry)
    pub valid: bool,
    /// Total number of cookie entries found in the file (across all domains)
    pub cookie_count: usize,
    /// Unique domains present in the cookie file (e.g., `["apple.com", "mzstatic.com"]`)
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
/// * `app` - Tauri `AppHandle` for resolving the settings.json file path.
///
/// # Errors
///
/// Returns `Err(String)` if the settings file cannot be read or parsed.
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
/// * `app` - Tauri `AppHandle` for resolving file paths.
/// * `settings` - The complete settings object from the frontend.
///   Deserialized from the JSON payload sent by `invoke("save_settings", { settings })`.
///   See: <https://v2.tauri.app/develop/calling-rust/#command-arguments>
///
/// # Errors
///
/// Returns `Err(String)` if settings serialization or file write fails.
///
/// # Returns
/// * `Ok(())` - Settings saved and synced successfully.
/// * `Err(String)` - File write or serialization error.
#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    static MUSICKIT_ID_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[A-Z0-9]{10}$").expect("Invalid MusicKit ID regex"));

    let mut settings = settings;

    let normalize_musickit_id =
        |label: &str, value: Option<String>| -> Result<Option<String>, String> {
            let Some(raw) = value else {
                return Ok(None);
            };
            let normalized = raw.trim().to_ascii_uppercase();
            if normalized.is_empty() {
                return Ok(None);
            }
            if !MUSICKIT_ID_RE.is_match(&normalized) {
                return Err(format!(
                    "{label} must be exactly 10 uppercase letters/numbers (A-Z, 0-9)."
                ));
            }
            Ok(Some(normalized))
        };

    settings.musickit_team_id =
        normalize_musickit_id("MusicKit Team ID", settings.musickit_team_id.take())?;
    settings.musickit_key_id =
        normalize_musickit_id("MusicKit Key ID", settings.musickit_key_id.take())?;

    // Load previous settings for diff logging (best-effort — if this fails,
    // we still save the new settings, just without the verbose diff).
    let previous = config_service::load_settings(&app).ok();

    // Security: `dev_access_enabled` must only be toggled by the dedicated
    // `activate_dev_access` / `deactivate_dev_access` commands, which
    // validate a passphrase (or clear the keychain sentinel) before
    // persisting via `config_service::save_settings` directly — a path this
    // clamp does NOT intercept. A general settings write (this IPC) must
    // never be able to flip the flag on, regardless of what the incoming
    // payload contains.
    settings.dev_access_enabled = previous
        .as_ref()
        .is_some_and(|p| p.dev_access_enabled);

    // save_settings() in config_service performs two writes:
    //   1. settings.json — full AppSettings struct as JSON
    //   2. config.ini — relevant fields translated to GAMDL's INI format
    config_service::save_settings(&app, &settings)?;

    // #690: refresh the in-process settings cache so the next
    // `load_settings_for_queue` reader sees the post-save snapshot
    // without re-touching the disk. If the cache isn't registered
    // (test contexts), this is a no-op.
    if let Some(cache) =
        app.try_state::<crate::services::settings_cache::SettingsCache>()
    {
        cache.refresh(settings.clone());
    }

    // Always emit the basic "Settings saved" message
    emit_app_log(&app, "Settings saved");

    // In verbose mode, emit a diff of what changed
    if let Some(ref prev) = previous {
        let changes = diff_settings(prev, &settings);
        if changes.is_empty() {
            emit_verbose_app_log(&app, "Settings saved (no changes detected)");
        } else {
            for change in &changes {
                emit_verbose_app_log(&app, &format!("Setting changed: {change}"));
            }
        }
    }

    Ok(())
}

/// Compare two `AppSettings` structs and return a list of human-readable change descriptions.
///
/// Serializes both to `serde_json::Value` maps and compares each top-level key.
/// Sensitive fields (cookies_path, wrapper_account_url, musickit_*) are redacted.
fn diff_settings(old: &AppSettings, new: &AppSettings) -> Vec<String> {
    let Ok(old_val) = serde_json::to_value(old) else {
        return vec![];
    };
    let Ok(new_val) = serde_json::to_value(new) else {
        return vec![];
    };

    let (Some(old_map), Some(new_map)) = (old_val.as_object(), new_val.as_object()) else {
        return vec![];
    };

    // Fields whose values should be redacted in logs (contain sensitive data)
    const REDACTED_FIELDS: &[&str] = &[
        "cookies_path",
        "wrapper_account_url",
        "wrapper_m3u8_ip",
        "wrapper_decrypt_ip",
        "musickit_team_id",
        "musickit_key_id",
        "acoustid_api_key",
    ];

    let mut changes = Vec::new();
    for (key, new_v) in new_map {
        let old_v = old_map.get(key);
        if old_v == Some(new_v) {
            continue;
        }
        if REDACTED_FIELDS.contains(&key.as_str()) {
            // Show that it changed but not the actual value
            let status = if new_v.is_null() || (new_v.is_string() && new_v.as_str() == Some("")) {
                "cleared"
            } else {
                "updated"
            };
            changes.push(format!("{key} → [{status}]"));
        } else {
            // Format the value compactly
            let fmt = |v: &serde_json::Value| -> String {
                match v {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Null => "null".to_string(),
                    serde_json::Value::Array(a) => {
                        let items: Vec<String> = a
                            .iter()
                            .map(|i| i.as_str().map_or_else(|| i.to_string(), str::to_string))
                            .collect();
                        format!("[{}]", items.join(", "))
                    }
                    other => other.to_string(),
                }
            };
            let old_str = old_v.map_or("(none)".to_string(), fmt);
            let new_str = fmt(new_v);
            changes.push(format!("{key}: {old_str} → {new_str}"));
        }
    }
    changes
}

/// Checks whether a built-in AcoustID API key was embedded at compile time.
///
/// **Frontend caller:** `hasEmbeddedAcoustidKey()` in `src/lib/tauri-commands.ts`
///
/// Release builds include the key via the `ACOUSTID_API_KEY` environment
/// variable (set from a GitHub Actions secret). Local dev builds typically
/// do not have this set, so this returns `false`.
///
/// The frontend uses this to adjust the AcoustID settings UI: when a
/// built-in key exists, the API key input is shown as optional (override)
/// rather than required.
#[tauri::command]
pub fn has_embedded_acoustid_key() -> bool {
    crate::services::acoustid_service::resolve_api_key("").is_some()
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
/// This command does NOT require the `AppHandle` because it only reads the
/// file at the user-provided path — no app state or data directory needed.
///
/// # Arguments
/// * `path` - Absolute path to the cookies.txt file to validate.
///   Provided by the frontend's file picker dialog.
///
/// # Errors
///
/// Returns `Err(String)` if the cookies file cannot be read (not found,
/// permission denied, etc.).
///
/// # Returns
/// * `Ok(CookieValidation)` - Detailed validation result with counts and warnings.
/// * `Err(String)` - File read error (file not found, permission denied, etc.).
///
/// # Netscape Cookie Format
/// Each cookie line is tab-separated with 7 fields:
/// `domain \t subdomains \t path \t secure \t expiry \t name \t value`
/// Lines starting with `#` are comments. Empty lines are skipped.
/// See: <https://curl.se/docs/http-cookies.html>
#[tauri::command]
pub async fn validate_cookies_file(path: String) -> Result<CookieValidation, String> {
    // Delegate to the shared health check service for cookie parsing.
    // The parsing logic (Netscape format, expiry checks, Apple domain filtering)
    // is reused by both this command and the pre-flight health checks in
    // download_queue.rs. See health_check_service::parse_cookies_file() for details.
    crate::services::health_check_service::parse_cookies_file(&path)
}

/// Result of checking cookie readiness before queuing a download.
///
/// Returned by `check_cookies_before_download()`. When `ready` is `false`,
/// the frontend should block the download and display `message` to the user.
#[derive(Serialize)]
pub struct CookieCheckResult {
    /// Whether cookies are valid and ready for downloading
    pub ready: bool,
    /// Human-readable explanation when `ready` is `false` (e.g., "Cookies expired")
    pub message: Option<String>,
}

/// Checks whether cookies are valid and ready for an Apple Music download.
///
/// **Frontend caller:** `checkCookiesBeforeDownload()` in `src/lib/tauri-commands.ts`
///
/// Called by the download form before queuing a download. This catches
/// expired or missing cookies at submission time — before GAMDL is invoked —
/// so the user gets immediate feedback instead of a delayed failure.
///
/// Wrapper users bypass this check (the wrapper handles authentication).
///
/// Reuses `health_check_service::parse_cookies_file()` for the actual
/// cookie parsing and expiry detection.
#[tauri::command]
pub fn check_cookies_before_download(app: AppHandle) -> Result<CookieCheckResult, String> {
    let settings = crate::services::config_service::load_settings(&app).unwrap_or_default();

    // Wrapper users don't need cookies — the wrapper handles authentication
    if settings.use_wrapper {
        return Ok(CookieCheckResult {
            ready: true,
            message: None,
        });
    }

    // No cookies file configured at all
    let Some(ref path) = settings.cookies_path else {
        return Ok(CookieCheckResult {
            ready: false,
            message: Some(
                "No cookies file configured. Go to Settings \u{203A} Cookies to import your Apple Music cookies.".to_string(),
            ),
        });
    };

    // Validate the cookies file contents
    match crate::services::health_check_service::parse_cookies_file(path) {
        Ok(v) if !v.valid || v.apple_music_cookies == 0 => Ok(CookieCheckResult {
            ready: false,
            message: Some(
                "Cookies file contains no Apple Music cookies. Re-import in Settings \u{203A} Cookies.".to_string(),
            ),
        }),
        Ok(v) if v.expired => Ok(CookieCheckResult {
            ready: false,
            message: Some(
                "Apple Music cookies have expired. Re-import fresh cookies in Settings \u{203A} Cookies.".to_string(),
            ),
        }),
        Ok(_) => Ok(CookieCheckResult {
            ready: true,
            message: None,
        }),
        Err(e) => Ok(CookieCheckResult {
            ready: false,
            message: Some(format!("Cannot read cookies file: {e}")),
        }),
    }
}

/// Checks whether the internet (and target service) is reachable before
/// queuing a download.
///
/// **Frontend caller:** `checkInternetBeforeDownload(urls?)` in `src/lib/tauri-commands.ts`
///
/// Called by the download form before the cookie check. Reuses the existing
/// `check_internet_connectivity()` health check (Tier 1: provider-neutral
/// general connectivity; Tier 2: the API of the download service detected
/// from `urls`). Returns the same `CookieCheckResult` shape so the frontend
/// can handle it with the same pattern.
///
/// `urls` is the batch of URLs about to be queued (A1) — the first URL that
/// resolves to a known [`crate::models::media_service::MediaServiceId`]
/// picks the Tier 2 probe (e.g. Spotify URLs probe Spotify's API instead of
/// Apple Music's). `None`/empty/all-unrecognised falls back to the Apple
/// Music probe, matching this command's pre-A1 behaviour exactly.
///
/// If the check fails, the frontend shows an amber warning and blocks the
/// download. This prevents queuing downloads that will immediately fail
/// due to no internet, and avoids generating unhelpful error reports.
#[tauri::command]
pub async fn check_internet_before_download(
    urls: Option<Vec<String>>,
) -> Result<CookieCheckResult, String> {
    let service = urls
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .find_map(|url| crate::models::media_service::MediaServiceId::from_url(url));

    match crate::services::health_check_service::check_internet_connectivity(service).await {
        None => Ok(CookieCheckResult {
            ready: true,
            message: None,
        }),
        Some(warning) => Ok(CookieCheckResult {
            ready: false,
            message: Some(warning.message),
        }),
    }
}

/// Checks that the output directory is writable before queuing a download.
///
/// **Frontend caller:** `checkOutputPathBeforeDownload()` in `src/lib/tauri-commands.ts`
///
/// Resolves the output path from current settings (using the default if empty)
/// and probes writability. Catches disconnected cloud mounts, full disks, and
/// permission issues before the download is queued.
///
/// Unlike the queue pre-flight check (which is non-blocking), this is called
/// from `DownloadForm.tsx` and is **blocking** — the download won't be queued
/// if the output path is inaccessible.
///
/// Reuses the `CookieCheckResult` shape for frontend consistency.
#[tauri::command]
pub async fn check_output_path_before_download(
    app: tauri::AppHandle,
) -> Result<CookieCheckResult, String> {
    let settings = crate::services::config_service::load_settings(&app)?;
    let resolved_path = if settings.output_path.is_empty() {
        crate::services::config_service::get_default_output_path()?
    } else {
        settings.output_path.clone()
    };
    match crate::services::health_check_service::check_output_path(&resolved_path).await {
        None => Ok(CookieCheckResult {
            ready: true,
            message: None,
        }),
        Some(warning) => Ok(CookieCheckResult {
            ready: false,
            message: Some(warning.message),
        }),
    }
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
/// See: <https://v2.tauri.app/develop/calling-rust/#async-commands>
///
/// # Errors
///
/// Returns `Err(String)` if the user's home or music directory cannot be determined.
///
/// # Returns
/// * `Ok(String)` - The absolute path to the default music output directory.
/// * `Err(String)` - If the user's home/music directory cannot be determined.
#[tauri::command]
pub fn get_default_output_path() -> Result<String, String> {
    config_service::get_default_output_path()
}

/// Result of testing connectivity to the wrapper service.
///
/// Returned by `test_wrapper_connection()`. The command always returns
/// `Ok(WrapperTestResult)` for both reachable and unreachable hosts —
/// it only returns `Err` for invalid URL format or client build failure.
///
/// Any HTTP response (even 404/500) counts as "reachable" since we are
/// testing network connectivity, not endpoint correctness.
#[derive(Debug, Clone, Serialize)]
pub struct WrapperTestResult {
    /// Whether the wrapper service responded at all
    pub reachable: bool,
    /// HTTP status code if a response was received
    pub status_code: Option<u16>,
    /// Round-trip time in milliseconds
    pub response_time_ms: Option<u64>,
    /// Human-readable error message if connection failed
    pub error: Option<String>,
}

/// Tests connectivity to the configured wrapper service URL.
///
/// **Frontend caller:** `testWrapperConnection(url)` in `src/lib/tauri-commands.ts`
///
/// Makes an HTTP GET request to the provided wrapper URL with a short
/// timeout (5 seconds). Returns a structured result indicating whether the
/// connection succeeded, the HTTP status code (if any), and the response
/// time in milliseconds.
///
/// This does NOT validate that the wrapper is functioning correctly for
/// authentication — it only verifies network reachability and that
/// something is listening on the specified address/port.
///
/// # Arguments
/// * `url` - The wrapper account URL to test (e.g., `"http://192.168.3.179:30020"`)
///
/// # Returns
/// * `Ok(WrapperTestResult)` - Connection test completed (may indicate failure via `reachable: false`)
/// * `Err(String)` - Invalid URL or unexpected error
#[tauri::command]
pub async fn test_wrapper_connection(url: String) -> Result<WrapperTestResult, String> {
    // Basic URL validation before attempting the request
    let parsed = url::Url::parse(&url).map_err(|e| format!("Invalid URL: {e}"))?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("URL must use http:// or https:// scheme".to_string());
    }

    let client = crate::utils::http_client::build_simple(5)?;

    let start = std::time::Instant::now();
    match client.get(&url).send().await {
        Ok(response) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            Ok(WrapperTestResult {
                reachable: true,
                status_code: Some(response.status().as_u16()),
                response_time_ms: Some(elapsed_ms),
                error: None,
            })
        }
        Err(e) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let error_msg = if e.is_timeout() {
                "Connection timed out (5s)".to_string()
            } else if e.is_connect() {
                format!("Connection refused — is the wrapper running at {url}?")
            } else {
                format!("{e}")
            };
            Ok(WrapperTestResult {
                reachable: false,
                status_code: None,
                response_time_ms: Some(elapsed_ms),
                error: Some(error_msg),
            })
        }
    }
}

// ============================================================
// Settings Export/Import
// ============================================================

/// Wrapper struct for the settings export file format.
///
/// Contains a schema version, app identifier, timestamp, and the
/// actual settings data. Sensitive fields (cookies path, wrapper URL,
/// MusicKit credentials) are cleared before export to prevent
/// accidental credential sharing.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettingsExportFile {
    /// Schema version for forward compatibility. Currently `1`.
    version: u32,
    /// Application identifier. Must be `"MeedyaDL"` for import validation.
    app: String,
    /// ISO 8601 timestamp of when the export was created.
    exported_at: String,
    /// The actual settings data (with sensitive fields cleared).
    settings: AppSettings,
}

/// Clears sensitive fields from an `AppSettings` clone before export.
///
/// This prevents accidental credential leakage when sharing settings
/// files. The cleared fields are device-specific (cookie paths) or
/// contain authentication secrets (wrapper URL, MusicKit credentials).
fn clear_sensitive_fields(settings: &mut AppSettings) {
    settings.cookies_path = None;
    settings.wrapper_account_url = String::new();
    settings.musickit_team_id = None;
    settings.musickit_key_id = None;
    settings.acoustid_api_key = String::new();
}

/// Exports application settings to a JSON file via a native save dialog.
///
/// **Frontend caller:** `exportSettings()` in `src/lib/tauri-commands.ts`
///
/// Opens a native "Save As" dialog with the `.json` file filter. The
/// exported file contains all settings except sensitive fields (cookies
/// path, wrapper URL, MusicKit credentials, AcoustID API key), which
/// are cleared to prevent accidental credential sharing.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for loading current settings and opening the dialog.
///
/// # Returns
/// * `Ok(String)` - The absolute path where the file was saved.
/// * `Err(String)` - Settings load failure, dialog cancelled, or write error.
#[tauri::command]
pub async fn export_settings(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    // Load current settings
    let mut settings = config_service::load_settings(&app)?;

    // Clear sensitive fields before export
    clear_sensitive_fields(&mut settings);

    // Build the export file structure
    let export_file = SettingsExportFile {
        version: 1,
        app: "MeedyaDL".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        settings,
    };

    // Serialize to pretty-printed JSON
    let json = serde_json::to_string_pretty(&export_file)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;

    // Open a native save dialog with .json filter
    let file_path = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_file_name("meedyadl-settings.json")
        .blocking_save_file();

    match file_path {
        Some(path) => {
            let resolved = path
                .as_path()
                .ok_or_else(|| "Failed to resolve export file path".to_string())?;
            std::fs::write(resolved, &json)
                .map_err(|e| format!("Failed to write settings file: {e}"))?;
            let filename = resolved
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("meedyadl-settings.json");
            let export_path = resolved.to_string_lossy().to_string();
            log::info!("Settings exported to {filename}");
            emit_app_log(&app, &format!("Settings exported to {filename}"));
            Ok(export_path)
        }
        None => Err("Export cancelled".to_string()),
    }
}

/// Imports application settings from a JSON file via a native file picker.
///
/// **Frontend caller:** `importSettings()` in `src/lib/tauri-commands.ts`
///
/// Opens a native file picker dialog with the `.json` file filter. The
/// selected file must be a valid `SettingsExportFile` with version `1`
/// and app identifier `"MeedyaDL"`. Imported settings are merged into
/// the current settings (overwriting all non-sensitive fields), saved
/// to disk, and synced to GAMDL's config.ini.
///
/// Sensitive fields from the current settings are preserved — the
/// import does not overwrite cookies path, wrapper URL, MusicKit
/// credentials, or AcoustID API key, since these are device-specific.
///
/// # Arguments
/// * `app` - Tauri `AppHandle` for loading/saving settings and opening the dialog.
///
/// # Returns
/// * `Ok(())` - Settings imported and saved successfully.
/// * `Err(String)` - Dialog cancelled, invalid file, or parse/save error.
#[tauri::command]
pub async fn import_settings(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;

    // Open a native file picker with .json filter
    let file_path = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_pick_file();

    let Some(path) = file_path else {
        return Err("Import cancelled".to_string());
    };

    // Read and parse the export file
    let resolved = path
        .as_path()
        .ok_or_else(|| "Failed to resolve import file path".to_string())?;
    let json = std::fs::read_to_string(resolved)
        .map_err(|e| format!("Failed to read settings file: {e}"))?;

    let export_file: SettingsExportFile =
        serde_json::from_str(&json).map_err(|e| format!("Invalid settings file format: {e}"))?;

    // Validate schema version
    if export_file.version != 1 {
        return Err(format!(
            "Unsupported settings file version: {} (expected 1)",
            export_file.version
        ));
    }

    // Validate app identifier
    if export_file.app != "MeedyaDL" {
        return Err(format!(
            "Invalid settings file: app identifier is {:?} (expected \"MeedyaDL\")",
            export_file.app
        ));
    }

    // Load current settings to preserve sensitive fields
    let current = config_service::load_settings(&app).unwrap_or_default();

    // Sanitize imported settings to prevent injection via crafted files.
    // Truncate excessively long strings that could cause memory issues
    // and strip path traversal sequences from path fields.
    // See: https://github.com/MWBMPartners/MeedyaDL/issues/229
    let mut merged = export_file.settings;
    sanitize_imported_settings(&mut merged);
    merged.cookies_path = current.cookies_path;
    merged.wrapper_account_url = current.wrapper_account_url;
    // Security: `wrapper_url` / `wrapper_decrypt_ip` must never be
    // settable via an imported settings file — otherwise a crafted
    // import could redirect where wrapper-v2 sign-in POSTs the user's
    // Apple ID + password (credential exfiltration).
    merged.wrapper_url = current.wrapper_url;
    merged.wrapper_decrypt_ip = current.wrapper_decrypt_ip;
    merged.musickit_team_id = current.musickit_team_id;
    merged.musickit_key_id = current.musickit_key_id;
    merged.acoustid_api_key = current.acoustid_api_key;
    // Security: dev-access gating must only change via the dedicated
    // activate/deactivate commands, never via a settings import.
    merged.dev_access_enabled = current.dev_access_enabled;

    // Save the merged settings (also syncs to GAMDL config.ini)
    config_service::save_settings(&app, &merged)?;

    let filename = resolved
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("settings file");
    log::info!("Settings imported from {filename}");
    emit_app_log(&app, &format!("Settings imported from {filename}"));

    Ok(())
}

/// Sanitize imported settings to prevent injection and resource exhaustion.
/// Truncates excessively long string values and strips control characters.
/// Applied after deserialization but before merging with current settings.
///
/// `pub(crate)` so `commands::profile_bundle::import_profile` can reuse the
/// same sanitisation for `.meedyabundle` settings sections.
pub(crate) fn sanitize_imported_settings(settings: &mut AppSettings) {
    const MAX_PATH: usize = 1024;
    const MAX_URL: usize = 2048;
    const MAX_TEMPLATE: usize = 512;

    // Security: never let an imported/bundled settings payload enable
    // developer access. Callers with a "preserve current value" policy
    // (e.g. `import_settings` above) overwrite this again afterward;
    // callers with no such policy get the safe default.
    settings.dev_access_enabled = false;

    fn truncate(s: &mut String, max: usize) {
        if s.len() > max {
            s.truncate(max);
        }
        // Strip newlines and carriage returns (INI injection prevention)
        *s = s.replace(['\n', '\r'], "");
    }

    fn truncate_opt(s: &mut Option<String>, max: usize) {
        if let Some(ref mut v) = s {
            truncate(v, max);
        }
    }

    // Paths
    truncate(&mut settings.output_path, MAX_PATH);
    truncate(&mut settings.temp_path, MAX_PATH);
    truncate_opt(&mut settings.cookies_path, MAX_PATH);
    truncate_opt(&mut settings.ffmpeg_path, MAX_PATH);
    truncate_opt(&mut settings.mp4decrypt_path, MAX_PATH);
    truncate_opt(&mut settings.mp4box_path, MAX_PATH);
    truncate_opt(&mut settings.nm3u8dlre_path, MAX_PATH);

    // URLs / addresses
    truncate(&mut settings.wrapper_account_url, MAX_URL);
    // `host:port` address — 64 chars is plenty (IPv6 + port fits in ~45).
    truncate(&mut settings.wrapper_m3u8_ip, 64);

    // Templates
    truncate(&mut settings.album_folder_template, MAX_TEMPLATE);
    truncate(&mut settings.compilation_folder_template, MAX_TEMPLATE);
    truncate(&mut settings.no_album_folder_template, MAX_TEMPLATE);
    truncate(&mut settings.single_disc_file_template, MAX_TEMPLATE);
    truncate(&mut settings.multi_disc_file_template, MAX_TEMPLATE);
    truncate(&mut settings.no_album_file_template, MAX_TEMPLATE);
    truncate(&mut settings.playlist_file_template, MAX_TEMPLATE);

    // Language/storefront (short strings)
    truncate(&mut settings.language, 20);
    truncate(&mut settings.storefront, 10);
    truncate(&mut settings.ui_language, 20);

    // Exclude tags (prevent excessively large arrays)
    if settings.exclude_tags.len() > 50 {
        settings.exclude_tags.truncate(50);
    }
    for tag in &mut settings.exclude_tags {
        truncate(tag, 100);
    }

    // Validate notification_style enum value
    const VALID_STYLES: &[&str] = &["in_app_only", "native_and_in_app", "native_only"];
    if !VALID_STYLES.contains(&settings.notification_style.as_str()) {
        settings.notification_style = "native_and_in_app".to_string();
    }
}
