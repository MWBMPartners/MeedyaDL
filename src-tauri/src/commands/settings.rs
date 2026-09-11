// Copyright (c) 2024-2026 MeedyaSuite
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
// | Rust Command                   | TypeScript Function              |
// |--------------------------------|----------------------------------|
// | get_settings                   | getSettings()                    |
// | save_settings                  | saveSettings(settings)           |
// | set_sidebar_collapsed          | saveSidebarCollapsed(collapsed)  |
// | has_embedded_acoustid_key      | hasEmbeddedAcoustidKey()         |
// | validate_cookies_file          | validateCookiesFile(path)        |
// | check_cookies_before_download  | checkCookiesBeforeDownload()     |
// | check_internet_before_download | checkInternetBeforeDownload()    |
// | get_default_output_path        | getDefaultOutputPath()           |
// | test_wrapper_connection        | testWrapperConnection(url)       |
//
// This table used to carry a "Line" column pointing into
// `tauri-commands.ts`. Every number in it was wrong by roughly six
// hundred lines — `getSettings` was listed at ~75 and is at 689 — because
// a line number goes stale the moment anything above it changes and
// nothing here ever checked them. The column is gone rather than
// corrected: corrected numbers would simply be wrong again tomorrow, and
// a number that looks precise is trusted more than it deserves. Search
// for the function name instead; that survives edits.
//
// The pairing itself IS checked: `tools/audit-checks/check_ipc_commands.py`
// confirms every `#[tauri::command]` is registered and every frontend
// `invoke('x')` names a real command.
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

/// Remembers whether the sidebar is collapsed.
///
/// **Frontend caller:** `saveSidebarCollapsed(collapsed)` in
/// `src/lib/tauri-commands.ts`.
///
/// The sidebar's collapse button sits on every screen, including the
/// Settings screen while it has edits nobody has pressed "Save Changes"
/// on yet. So this deliberately does NOT accept a settings object from
/// the frontend. It takes one boolean and changes one field on disk, and
/// there is no parameter anything else could ride along in. Two earlier
/// attempts at remembering this preference did send the whole settings
/// object, committed the person's half-finished edits by accident, and
/// were both backed out — see #1175.
///
/// It also deliberately does not write "Settings saved" to the activity
/// log. Clicking an arrow to make the sidebar narrower is not a settings
/// change anyone wants a log line about, and one line per click would
/// bury the real ones.
///
/// # Errors
///
/// Returns `Err(String)` if the settings file cannot be read or written.
#[tauri::command]
pub async fn set_sidebar_collapsed(app: AppHandle, collapsed: bool) -> Result<(), String> {
    config_service::update_settings_field(&app, |s| s.sidebar_collapsed = collapsed)?;
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
        "odesli_api_key",
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
/// MusicKit credentials, the song.link key) are cleared before export
/// to prevent accidental credential sharing.
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
    settings.odesli_api_key = String::new();
}

/// Exports application settings to a JSON file via a native save dialog.
///
/// **Frontend caller:** `exportSettings()` in `src/lib/tauri-commands.ts`
///
/// Opens a native "Save As" dialog with the `.json` file filter. The
/// exported file contains all settings except sensitive fields (cookies
/// path, wrapper URL, MusicKit credentials, AcoustID API key, the
/// song.link key), which are cleared to prevent accidental credential
/// sharing.
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
    preserve_local_only_settings(&mut merged, &current);

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
/// Keeps the settings that must not travel in an exported file.
///
/// Three kinds of thing are put back to whatever this machine already had
/// (#229), and the difference between them and everything else is the point:
///
/// * **Things that point at this machine** — where the helper programs live,
///   where the cookies are. A path is meaningless on somebody else's computer,
///   and four of them are programs MeedyaDL runs, so accepting them would let
///   an imported file choose what gets executed.
/// * **Credentials and addresses** — the wrapper's address decides where a
///   sign-in is sent, so an imported file could otherwise redirect somebody's
///   Apple ID and password.
/// * **Records that a person agreed to something** — not preferences, but
///   evidence they were shown a warning and accepted it. A file somebody was
///   sent could otherwise arrive with the Spotify warning pre-acknowledged,
///   and they would never see it.
///
/// Ordinary preferences are deliberately NOT touched. Codecs, output folders,
/// naming patterns and the rest are exactly what somebody wants to carry
/// between their own machines, and none of them speaks for the person or
/// points at anything that runs.
///
/// Split out from the import command so it can be tested directly — the
/// command itself needs a running app and a file dialogue.
///
/// # Arguments
///
/// * `imported` -- The settings read from the file, changed in place.
/// * `current` -- What this machine already had.
pub(crate) fn preserve_local_only_settings(imported: &mut AppSettings, current: &AppSettings) {
    imported.cookies_path = current.cookies_path.clone();
    imported.wrapper_account_url = current.wrapper_account_url.clone();
    // Security: `wrapper_url` / `wrapper_decrypt_ip` / `wrapper_m3u8_ip`
    // must never be settable via an imported settings file — otherwise a
    // crafted import could redirect where wrapper-v2 sign-in POSTs the
    // user's Apple ID + password (credential exfiltration).
    //
    // `wrapper_m3u8_ip` used to be missing from this list. It decides
    // where GAMDL opens a TCP connection to ask for the HLS playlist URL
    // for every track it downloads (GAMDL v3.1+) — an imported file could
    // point that at any host it liked, and the download tool would then
    // connect there and use whatever it was handed back.
    imported.wrapper_url = current.wrapper_url.clone();
    imported.wrapper_decrypt_ip = current.wrapper_decrypt_ip.clone();
    imported.wrapper_m3u8_ip = current.wrapper_m3u8_ip.clone();
    imported.musickit_team_id = current.musickit_team_id.clone();
    imported.musickit_key_id = current.musickit_key_id.clone();
    imported.acoustid_api_key = current.acoustid_api_key.clone();
    imported.odesli_api_key = current.odesli_api_key.clone();
    // Security: the paths to the helper programs must never come from an
    // imported file (#229).
    //
    // Each of these is a path to a program that MeedyaDL runs. FFmpeg and the
    // others are passed to the download tool, which starts them; MediaInfo is
    // started directly. So a settings file that set one of them to any program
    // on the machine would have that program run during an ordinary download.
    // That is a settings file someone was sent and opened — exactly the case
    // this issue exists to guard.
    //
    // Nothing is lost by refusing them. A path is where a program sits on ONE
    // machine; it means nothing on anybody else's, so carrying these across in
    // an exported file was never useful even when it was safe.
    imported.ffmpeg_path = current.ffmpeg_path.clone();
    imported.mp4decrypt_path = current.mp4decrypt_path.clone();
    imported.mp4box_path = current.mp4box_path.clone();
    imported.nm3u8dlre_path = current.nm3u8dlre_path.clone();
    imported.mediainfo_path = current.mediainfo_path.clone();
    // Security: where the persistent activity log is written is also a
    // path on THIS machine, which is exactly what this whole function
    // exists to protect. Left un-preserved, an imported file could point
    // it anywhere on disk, and on the next app start the log writer
    // (`resolve_activity_log_dir` in `lib.rs`) would create that folder
    // and start writing files into it.
    imported.activity_log_path_override = current.activity_log_path_override.clone();
    // Security: dev-access gating must only change via the dedicated
    // activate/deactivate commands, never via a settings import.
    imported.dev_access_enabled = current.dev_access_enabled;
    // Security: an imported file must not record agreement on somebody's
    // behalf (#229).
    //
    // These are not preferences, they are records that a person was shown
    // something and agreed to it. A settings file somebody was sent could
    // otherwise arrive with the Spotify warning already acknowledged — the
    // warning that exists to explain how downloading there can get an account
    // flagged — and they would never see it. Crash reporting and usage
    // reporting are the same shape: consent to send data, which has to be
    // given rather than inherited.
    //
    // Preferences are deliberately still imported: codecs, paths, naming
    // patterns and the rest are exactly what somebody wants to carry between
    // their own machines, and none of them speaks for the person.
    imported.terms_accepted = current.terms_accepted;
    imported.spotify_consent_acknowledged = current.spotify_consent_acknowledged;
    imported.sentry_enabled = current.sentry_enabled;
    imported.analytics_enabled = current.analytics_enabled;
    // Security: which build stream someone receives must not change under
    // them. The unstable channels sit behind developer access, which is
    // clamped just above — importing a channel would step around that gate,
    // and would start delivering unfinished builds to somebody who never
    // asked. Re-picking a channel is one dropdown; being moved onto one
    // silently is not something they would think to check.
    imported.update_channel = current.update_channel;
}

pub(crate) fn sanitize_imported_settings(settings: &mut AppSettings) {
    const MAX_PATH: usize = 1024;
    const MAX_URL: usize = 2048;
    const MAX_TEMPLATE: usize = 512;

    // Security: never let an imported/bundled settings payload enable
    // developer access. Callers with a "preserve current value" policy
    // (e.g. `import_settings` above) overwrite this again afterward;
    // callers with no such policy get the safe default.
    settings.dev_access_enabled = false;

    /// Shortens a value to a sensible length and removes line breaks.
    ///
    /// The length limit is in bytes, but it has to be cut at a point that is a
    /// whole character. Rust refuses to cut a piece of text in the middle of a
    /// character and stops the program if asked to — and that is not a rare
    /// edge: any accented or non-Latin text uses more than one byte per
    /// character, so a long enough path in French, German, Greek, Arabic or
    /// Japanese will land the cut mid-character.
    ///
    /// Before this, importing a settings file with a long enough accented path
    /// took the whole backend down and filed a crash report — from a file the
    /// person had just been invited to open (#229). Confirmed by reproducing
    /// it: 400 euro signs is 1200 bytes, and byte 1024 is not a character
    /// boundary.
    ///
    /// Cutting at the last whole character before the limit keeps the value
    /// slightly shorter than asked, which is always safe.
    fn truncate(s: &mut String, max: usize) {
        if s.len() > max {
            // Walk back to the last point that starts a character. Byte 0
            // always is, so this cannot run off the front.
            let mut cut = max;
            while cut > 0 && !s.is_char_boundary(cut) {
                cut -= 1;
            }
            s.truncate(cut);
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
    truncate(&mut settings.activity_log_path_override, MAX_PATH);

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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Shortening a value must not crash the app (#229) ────────────────
    //
    // The length limit is in bytes, but text has to be cut at a whole
    // character. Rust stops the program if asked to cut mid-character, and
    // that is not a rare edge: accented and non-Latin text uses more than one
    // byte per character, so a long enough path in French, German, Greek,
    // Arabic or Japanese lands the cut in the middle of one.
    //
    // Importing such a file took the whole backend down and filed a crash
    // report — from a file the person had just been invited to open.
    // Reproduced as a real panic before this fix.

    #[test]
    fn a_long_accented_path_does_not_crash_the_app() {
        // 400 euro signs is 1200 bytes, and byte 1024 is mid-character.
        let mut settings = crate::models::settings::AppSettings {
            output_path: "\u{20ac}".repeat(400),
            ..Default::default()
        };
        sanitize_imported_settings(&mut settings);
        assert!(settings.output_path.len() <= 1024);
        // Still valid text, not a broken half-character.
        assert!(settings.output_path.chars().all(|c| c == '\u{20ac}'));
    }

    #[test]
    fn other_multi_byte_writing_systems_are_safe_too() {
        for sample in ["\u{65e5}", "\u{639}", "\u{3b1}", "\u{1f600}"] {
            let mut settings = crate::models::settings::AppSettings {
                output_path: sample.repeat(600),
                ..Default::default()
            };
            sanitize_imported_settings(&mut settings);
            assert!(
                settings.output_path.len() <= 1024,
                "{sample} was not shortened safely"
            );
        }
    }

    #[test]
    fn ordinary_paths_are_left_exactly_as_they_are() {
        let mut settings = crate::models::settings::AppSettings {
            output_path: "/Users/me/Music".to_string(),
            ..Default::default()
        };
        sanitize_imported_settings(&mut settings);
        assert_eq!(settings.output_path, "/Users/me/Music");
    }

    #[test]
    fn line_breaks_are_still_removed_after_shortening() {
        // The other half of this function, which must keep working: a line
        // break in a value can start a new setting in the download tool's own
        // configuration file.
        let mut settings = crate::models::settings::AppSettings {
            output_path: "/Users/me\nffmpeg_path = /tmp/attacker".to_string(),
            ..Default::default()
        };
        sanitize_imported_settings(&mut settings);
        assert!(!settings.output_path.contains('\n'));
    }

    #[test]
    fn developer_access_can_never_be_switched_on_by_an_imported_file() {
        // Pre-existing behaviour, pinned here because it is security-relevant
        // and sits in the same function.
        let mut settings = crate::models::settings::AppSettings {
            dev_access_enabled: true,
            ..Default::default()
        };
        sanitize_imported_settings(&mut settings);
        assert!(!settings.dev_access_enabled);
    }
    // ── What an imported settings file must not be able to change (#229) ─
    //
    // The line is between preferences, which people legitimately carry
    // between their own machines, and things that either point at THIS
    // machine or speak FOR the person. The second kind must never travel.

    fn settings_where_everything_is_set() -> crate::models::settings::AppSettings {
        crate::models::settings::AppSettings {
            // Points at a program that gets run.
            ffmpeg_path: Some("/tmp/attacker/ffmpeg".to_string()),
            mediainfo_path: Some("/tmp/attacker/mediainfo".to_string()),
            // Decides where a sign-in is sent.
            wrapper_url: "http://attacker.example/steal".to_string(),
            // Records that a person agreed to something.
            terms_accepted: true,
            spotify_consent_acknowledged: true,
            sentry_enabled: true,
            analytics_enabled: true,
            // An ordinary preference, which SHOULD travel.
            output_path: "/Users/them/Music".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn an_imported_file_cannot_choose_which_programs_run() {
        // Four of these are handed to the download tool, which starts them,
        // and one is started directly.
        let mut imported = settings_where_everything_is_set();
        let current = crate::models::settings::AppSettings::default();
        preserve_local_only_settings(&mut imported, &current);

        assert_eq!(imported.ffmpeg_path, current.ffmpeg_path);
        assert_eq!(imported.mediainfo_path, current.mediainfo_path);
        assert_eq!(imported.mp4decrypt_path, current.mp4decrypt_path);
        assert_eq!(imported.mp4box_path, current.mp4box_path);
        assert_eq!(imported.nm3u8dlre_path, current.nm3u8dlre_path);
    }

    #[test]
    fn an_imported_file_cannot_redirect_a_sign_in() {
        let mut imported = settings_where_everything_is_set();
        let current = crate::models::settings::AppSettings::default();
        preserve_local_only_settings(&mut imported, &current);
        assert_eq!(imported.wrapper_url, current.wrapper_url);
        assert_eq!(imported.wrapper_decrypt_ip, current.wrapper_decrypt_ip);
    }

    #[test]
    fn an_imported_file_cannot_agree_to_things_on_your_behalf() {
        // The Spotify one matters most: it is the acknowledgement of a
        // warning about how downloading there can get an account flagged.
        // Arriving pre-acknowledged means the person never sees it.
        let mut imported = settings_where_everything_is_set();
        let current = crate::models::settings::AppSettings::default();
        preserve_local_only_settings(&mut imported, &current);

        assert!(!imported.spotify_consent_acknowledged);
        assert!(!imported.terms_accepted);
        assert!(!imported.sentry_enabled, "consent to send crash data must be given, not inherited");
        assert!(!imported.analytics_enabled);
    }

    #[test]
    fn an_imported_file_cannot_move_you_onto_unfinished_builds() {
        // The unstable channels sit behind developer access, which is also
        // preserved — importing a channel would step around that gate.
        let mut imported = crate::models::settings::AppSettings {
            update_channel: crate::models::settings::UpdateChannel::Alpha,
            ..Default::default()
        };
        let current = crate::models::settings::AppSettings::default();
        preserve_local_only_settings(&mut imported, &current);
        assert_eq!(imported.update_channel, current.update_channel);
    }

    #[test]
    fn ordinary_preferences_still_travel_between_your_own_machines() {
        // The other half of the rule, and the reason this is not simply
        // "ignore the whole file". Someone moving to a new laptop should keep
        // their choices.
        let mut imported = settings_where_everything_is_set();
        let current = crate::models::settings::AppSettings::default();
        preserve_local_only_settings(&mut imported, &current);
        assert_eq!(
            imported.output_path, "/Users/them/Music",
            "a preference must not be reset — that would make importing pointless"
        );
    }

    #[test]
    fn an_imported_file_cannot_change_the_song_link_key() {
        // A settings file someone else made must never be able to swap in
        // their own song.link key. That key is credential-shaped — it is
        // sent to a third party on this person's behalf — so it follows
        // the same rule as the other credentials above: it stays whatever
        // this machine already had, not whatever the imported file says.
        let current = crate::models::settings::AppSettings {
            odesli_api_key: "this-machines-real-key".to_string(),
            ..Default::default()
        };
        let mut imported = crate::models::settings::AppSettings {
            odesli_api_key: "someone-elses-key".to_string(),
            ..Default::default()
        };
        preserve_local_only_settings(&mut imported, &current);
        assert_eq!(imported.odesli_api_key, "this-machines-real-key");
    }

    #[test]
    fn an_imported_file_cannot_redirect_the_m3u8_wrapper_socket() {
        // `wrapper_m3u8_ip` decides which host GAMDL (v3.1+) connects to
        // for the HLS playlist address of every track it downloads. It
        // sits alongside `wrapper_account_url` and `wrapper_decrypt_ip` —
        // both already preserved above — but had been missed, so an
        // imported settings file could point this one socket at any
        // host it liked and the download tool would connect there for
        // every track.
        let current = crate::models::settings::AppSettings {
            wrapper_m3u8_ip: "127.0.0.1:20020".to_string(),
            ..Default::default()
        };
        let mut imported = crate::models::settings::AppSettings {
            wrapper_m3u8_ip: "attacker.example:20020".to_string(),
            ..Default::default()
        };
        preserve_local_only_settings(&mut imported, &current);
        assert_eq!(imported.wrapper_m3u8_ip, "127.0.0.1:20020");
    }

    #[test]
    fn an_imported_file_cannot_choose_where_the_activity_log_is_written() {
        // `activity_log_path_override` is a path on THIS machine — the
        // exact thing this whole function exists to protect. Left
        // un-preserved, an imported file could point it anywhere on
        // disk, and the log writer would create that folder and start
        // writing files there on the next app start.
        let current = crate::models::settings::AppSettings {
            activity_log_path_override: "/Users/them/logs".to_string(),
            ..Default::default()
        };
        let mut imported = crate::models::settings::AppSettings {
            activity_log_path_override: "/tmp/attacker-controlled".to_string(),
            ..Default::default()
        };
        preserve_local_only_settings(&mut imported, &current);
        assert_eq!(imported.activity_log_path_override, "/Users/them/logs");
    }
}

