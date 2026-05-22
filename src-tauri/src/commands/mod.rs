// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Command modules for Tauri IPC handlers.
// =========================================
//
// This module aggregates all `#[tauri::command]` functions that the React
// frontend can call via `invoke("command_name", { ...args })`. Each
// sub-module groups related commands by domain.
//
// Architectural pattern:
//   Commands are **thin wrappers**. They accept deserialized arguments from
//   the frontend, extract managed state (via `State<'_, T>`), call the
//   appropriate `services` function to perform the actual work, and return
//   a serializable result. Business logic does NOT belong in command handlers.
//
// Registration:
//   All command functions must be listed in the `generate_handler!` macro
//   in `lib.rs`. Forgetting to register a command will cause a runtime
//   "command not found" error when the frontend tries to invoke it.
//
// Error handling:
//   Commands return `Result<T, String>` where `T` is the success payload
//   (serialized to JSON) and `String` is a human-readable error message.
//   Tauri automatically converts `Err(String)` into a rejected Promise
//   on the JavaScript side.
//
// Module map:
//   commands/
//   +-- system.rs       -- Platform info, app data directory path
//   +-- dependencies.rs -- Check/install Python, GAMDL, and external tools
//   +-- settings.rs     -- Read/write app settings, validate cookies file
//   +-- gamdl.rs        -- Start/cancel/retry downloads, queue management
//   +-- credentials.rs  -- Secure credential storage (keychain/credential vault)
//   +-- updates.rs      -- Check for component updates, upgrade GAMDL
//
// Reference: https://v2.tauri.app/develop/calling-rust/
// Reference: https://docs.rs/tauri/latest/tauri/macro.generate_handler.html

/// System information commands (platform detection, directory paths).
///
/// Provides `get_platform_info` and `get_app_data_dir` for the frontend
/// to discover the current OS, architecture, and data directory at startup.
pub mod system;

/// Dependency management commands (Python, GAMDL, `FFmpeg`, mp4decrypt, etc.).
///
/// Provides commands to check installation status and install each
/// dependency. Delegates to `services::python_manager` and
/// `services::dependency_manager` for the actual download/install work.
pub mod dependencies;

/// Application settings commands (read, write, validate).
///
/// Provides `get_settings`, `save_settings`, `validate_cookies_file`, and
/// `get_default_output_path`. Delegates to `services::config_service`.
pub mod settings;

/// GAMDL download execution commands (start, cancel, retry, queue status).
///
/// Provides `start_download`, `cancel_download`, `retry_download`,
/// `clear_queue`, `get_queue_status`, and `check_gamdl_update`. Delegates
/// to `services::download_queue` and `services::gamdl_service`.
pub mod gamdl;

/// Secure credential storage commands (store, retrieve, delete).
///
/// Uses the OS keychain (macOS Keychain, Windows Credential Vault, Linux
/// Secret Service) to store sensitive values like API tokens or cookies.
pub mod credentials;

/// Update checking commands (check versions, upgrade GAMDL).
///
/// Provides `check_all_updates`, `upgrade_gamdl`, and
/// `check_component_update`. Delegates to `services::update_checker`.
pub mod updates;

/// Cookie management commands (browser detection, auto-import, FDA check).
///
/// Provides `detect_browsers`, `import_cookies_from_browser`, and
/// `check_full_disk_access`. Delegates to `services::cookie_service`.
pub mod cookies;

/// Embedded Apple Music login window commands (open, extract, close).
///
/// Provides `open_apple_login`, `extract_login_cookies`, and
/// `close_apple_login`. Delegates to `services::login_window_service`.
pub mod login_window;

/// Animated artwork download commands (manual trigger for album artwork).
///
/// Provides `download_animated_artwork` for explicitly fetching animated
/// cover art from Apple Music for a specific album. Delegates to
/// `services::animated_artwork_service` for the actual API query and download.
pub mod artwork;

/// Crash report IPC commands (list, get, delete, export, log frontend errors).
///
/// Provides commands for the frontend to manage crash reports and to
/// persist frontend errors to the same crash report system used by the
/// Rust panic handler. Delegates to `services::crash_report_service`.
pub mod crash_reports;

/// Download history IPC commands (list, search, clear).
///
/// Provides commands for the frontend to view, search, and clear
/// the persistent download history. Delegates to
/// `services::history_service`.
pub mod history;

/// Clipboard IPC command for the clipboard monitoring feature.
///
/// Provides `read_clipboard` to read the current clipboard text.
/// The frontend polls this command to detect supported URLs.
pub mod clipboard;

/// API field audit command — diagnostic tool for discovering new API fields.
///
/// Provides `audit_api_fields` which fetches an Apple Music album and diffs
/// its JSON attributes against the known tag definitions in `tags.toml`.
/// Delegates to `services::api_audit_service`.
pub mod api_audit;

/// Service status command — checks remote service availability.
pub mod service_status;

/// Smart Download command — cross-platform quality search.
pub mod smart_download;

/// Activity log IPC commands (#541) — export the persistent on-disk
/// activity log and reveal the logs folder in the OS file manager.
/// Complements `commands::gamdl::export_activity_log`, which exports
/// the in-memory (possibly-trimmed) entries.
pub mod activity_log;

/// Legal / compliance IPC commands (#802) — surface the embedded
/// `ACKNOWLEDGEMENTS.md` and `THIRD_PARTY_LICENSES.md` files to the
/// frontend's Help > About > Open Source Acknowledgements view. The
/// files are compiled in via `include_str!()` so the notices ride
/// inside the binary on every platform without bundle-resource
/// path quirks.
pub mod legal;

/// Snapshot + restore commands (#466). Wraps `backup_service` with
/// IPC entry points: create, list, restore, delete. Each runs on a
/// blocking thread so the FS work doesn't block the main runtime.
pub mod backup;
