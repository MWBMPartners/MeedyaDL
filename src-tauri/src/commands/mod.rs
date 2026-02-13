// Copyright (c) 2024-2026 MeedyaDL
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

/// Dependency management commands (Python, GAMDL, FFmpeg, mp4decrypt, etc.).
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
