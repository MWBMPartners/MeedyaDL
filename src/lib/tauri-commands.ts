/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/lib/tauri-commands.ts - Type-safe IPC command wrappers
 *
 * This module provides a type-safe TypeScript abstraction over Tauri's
 * `invoke()` IPC mechanism. Every exported function here corresponds to
 * a `#[tauri::command]` handler in the Rust backend.
 *
 * IPC Bridge Pattern:
 * ┌─────────────────────────────────────────────────────────────────┐
 * │ Frontend (TypeScript)           │  Backend (Rust)               │
 * ├─────────────────────────────────┼───────────────────────────────┤
 * │ getPlatformInfo()               │  get_platform_info()          │
 * │   → invoke<PlatformInfo>(       │    → #[tauri::command]        │
 * │       'get_platform_info')      │       fn get_platform_info()  │
 * │   → Promise<PlatformInfo>       │       → PlatformInfo          │
 * └─────────────────────────────────┴───────────────────────────────┘
 *
 * How it works:
 * 1. The frontend calls `invoke<ReturnType>('command_name', { args })`
 * 2. Tauri serializes the args to JSON and sends them to the Rust process
 * 3. The Rust command handler deserializes the args, executes logic
 * 4. The return value is serialized via serde and sent back to the frontend
 * 5. The Promise resolves with the typed return value
 *
 * Command name convention:
 * - TypeScript: camelCase function names (e.g., `getPlatformInfo`)
 * - Rust: snake_case function names (e.g., `get_platform_info`)
 * - The invoke string uses the Rust snake_case name
 *
 * Error handling:
 * - If the Rust command returns `Result::Err`, the Promise rejects
 * - Callers should use try/catch or .catch() to handle IPC errors
 *
 * @see {@link https://v2.tauri.app/develop/calling-rust/} - Tauri invoke pattern
 * @see {@link https://v2.tauri.app/reference/javascript/api/namespacecore/} - Core API reference
 */

/**
 * Tauri's core `invoke()` function for calling Rust command handlers.
 *
 * Generic signature: `invoke<T>(cmd: string, args?: object): Promise<T>`
 * - `T` is the expected return type (deserialized from Rust's serde output)
 * - `cmd` is the snake_case name of the `#[tauri::command]` function
 * - `args` is an optional object whose keys match the Rust function parameters
 *
 * @see {@link https://v2.tauri.app/reference/javascript/api/namespacecore/#invoke}
 */
import { invoke } from '@tauri-apps/api/core';

/**
 * TypeScript type imports for IPC argument and return types.
 * These types mirror the corresponding Rust structs, ensuring compile-time
 * type safety across the serialization boundary.
 *
 * The `@/types` path alias is configured in tsconfig.json and vite.config.ts
 * to resolve to `src/types/index.ts`.
 *
 * @see src/types/index.ts for full type definitions
 */
import type {
  AppSettings,
  ComponentUpdate,
  CookieValidation,
  DependencyStatus,
  DownloadRequest,
  PlatformInfo,
  QueueStatus,
  UpdateCheckResult,
} from '@/types';

// ============================================================
// System Commands
// ============================================================

/**
 * Returns platform information (OS, architecture) for theme selection
 * and platform-specific behavior.
 *
 * Rust handler: `get_platform_info()` in `src-tauri/src/commands/system.rs`
 * Returns: `PlatformInfo { platform, arch, os_type }`
 *
 * Called by: usePlatform hook (indirectly), dependency installer (for arch-specific binaries)
 *
 * @returns Promise resolving to platform information
 * @see {@link https://v2.tauri.app/develop/calling-rust/#commands} - Tauri commands
 */
export function getPlatformInfo(): Promise<PlatformInfo> {
  return invoke<PlatformInfo>('get_platform_info');
}

/**
 * Returns the absolute path to the application's data directory.
 *
 * Rust handler: `get_app_data_dir()` in `src-tauri/src/commands/system.rs`
 * Returns: string (e.g., "~/Library/Application Support/com.mwbm.gamdl-gui")
 *
 * The data directory stores settings.json, logs, and cached dependency binaries.
 *
 * @returns Promise resolving to the absolute filesystem path
 */
export function getAppDataDir(): Promise<string> {
  return invoke<string>('get_app_data_dir');
}

// ============================================================
// Dependency Management Commands
// ============================================================

/**
 * Checks if the portable Python runtime is installed and accessible.
 *
 * Rust handler: `check_python_status()` in `src-tauri/src/commands/dependency.rs`
 * Returns: `DependencyStatus { name: "Python", installed, version, path }`
 *
 * The Rust backend checks for the bundled portable Python in the app
 * data directory, or falls back to system Python if available.
 *
 * Called by: dependencyStore.checkAll(), SetupWizard Python step
 *
 * @returns Promise resolving to Python installation status
 */
export function checkPythonStatus(): Promise<DependencyStatus> {
  return invoke<DependencyStatus>('check_python_status');
}

/**
 * Downloads and installs the portable Python runtime.
 *
 * Rust handler: `install_python()` in `src-tauri/src/commands/dependency.rs`
 * Returns: string message indicating success (e.g., "Python 3.12.1 installed")
 *
 * This is a long-running operation that downloads a platform-specific
 * portable Python build and extracts it to the app data directory.
 * The Promise may take several minutes to resolve depending on network speed.
 *
 * Called by: SetupWizard Python step
 *
 * @returns Promise resolving to a success message string
 */
export function installPython(): Promise<string> {
  return invoke<string>('install_python');
}

/**
 * Checks if GAMDL is installed in the Python environment.
 *
 * Rust handler: `check_gamdl_status()` in `src-tauri/src/commands/dependency.rs`
 * Returns: `DependencyStatus { name: "GAMDL", installed, version, path }`
 *
 * Runs `pip show gamdl` in the portable Python environment to detect
 * whether GAMDL is installed and extract its version number.
 *
 * Called by: dependencyStore.checkAll(), SetupWizard GAMDL step
 *
 * @returns Promise resolving to GAMDL installation status
 */
export function checkGamdlStatus(): Promise<DependencyStatus> {
  return invoke<DependencyStatus>('check_gamdl_status');
}

/**
 * Installs GAMDL via pip into the portable Python environment.
 *
 * Rust handler: `install_gamdl()` in `src-tauri/src/commands/dependency.rs`
 * Returns: string message indicating success (e.g., "GAMDL 1.5.0 installed")
 *
 * Runs `pip install gamdl` in the portable Python. May take a minute
 * as it downloads GAMDL and its Python dependencies.
 *
 * Called by: SetupWizard GAMDL step
 *
 * @returns Promise resolving to a success message string
 */
export function installGamdl(): Promise<string> {
  return invoke<string>('install_gamdl');
}

/**
 * Returns the installation status of all external tool dependencies.
 *
 * Rust handler: `check_all_dependencies()` in `src-tauri/src/commands/dependency.rs`
 * Returns: `DependencyStatus[]` for FFmpeg, mp4decrypt, MP4Box, N_m3u8DL-RE, etc.
 *
 * Checks each tool binary by running its version command and parsing the output.
 * The checks run in parallel on the Rust side for faster results.
 *
 * Called by: dependencyStore.checkAll(), SetupWizard dependencies step
 *
 * @returns Promise resolving to an array of dependency statuses
 */
export function checkAllDependencies(): Promise<DependencyStatus[]> {
  return invoke<DependencyStatus[]>('check_all_dependencies');
}

/**
 * Downloads and installs a specific tool dependency by name.
 *
 * Rust handler: `install_dependency()` in `src-tauri/src/commands/dependency.rs`
 * Argument: `name` - the dependency name (e.g., "ffmpeg", "mp4decrypt")
 * Returns: string message indicating success
 *
 * The Rust backend downloads the platform/arch-appropriate binary from
 * the official release source and places it in the app data directory.
 *
 * Called by: SetupWizard dependencies step
 *
 * @param name - Name of the dependency to install
 * @returns Promise resolving to a success message string
 */
export function installDependency(name: string): Promise<string> {
  return invoke<string>('install_dependency', { name });
}

// ============================================================
// Settings Commands
// ============================================================

/**
 * Loads the current application settings from disk.
 *
 * Rust handler: `get_settings()` in `src-tauri/src/commands/settings.rs`
 * Returns: complete `AppSettings` object
 *
 * On first run, the Rust backend creates a default settings file and
 * returns the defaults. On subsequent runs, it reads and deserializes
 * the saved settings.json from the app data directory.
 *
 * Called by: settingsStore.loadSettings()
 *
 * @returns Promise resolving to the complete settings object
 */
export function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>('get_settings');
}

/**
 * Saves application settings to disk and syncs relevant values to GAMDL's config.
 *
 * Rust handler: `save_settings()` in `src-tauri/src/commands/settings.rs`
 * Argument: `settings` - the complete AppSettings object to persist
 *
 * The Rust backend performs two operations:
 * 1. Serializes settings to JSON and writes to settings.json
 * 2. Syncs applicable settings (output_path, codec, etc.) to GAMDL's
 *    own config file so the CLI tool stays in sync with the GUI
 *
 * Called by: settingsStore.saveSettings()
 *
 * @param settings - Complete settings object to save
 * @returns Promise resolving when save is complete
 */
export function saveSettings(settings: AppSettings): Promise<void> {
  return invoke<void>('save_settings', { settings });
}

/**
 * Validates a Netscape-format cookies file for Apple Music authentication.
 *
 * Rust handler: `validate_cookies_file()` in `src-tauri/src/commands/settings.rs`
 * Argument: `path` - absolute filesystem path to the cookies.txt file
 * Returns: `CookieValidation` with validity, domain info, and warnings
 *
 * The Rust backend parses the cookies file, counts entries, checks for
 * Apple Music-specific domains, and detects expired cookies.
 *
 * Called by: SetupWizard cookies step, SettingsPage cookies selector
 *
 * @param path - Absolute path to the cookies file
 * @returns Promise resolving to validation results
 */
export function validateCookiesFile(path: string): Promise<CookieValidation> {
  return invoke<CookieValidation>('validate_cookies_file', { path });
}

/**
 * Returns the default output path for downloaded music.
 *
 * Rust handler: `get_default_output_path()` in `src-tauri/src/commands/settings.rs`
 * Returns: string path (e.g., "~/Music/GAMDL" on macOS)
 *
 * The default path is platform-specific, using the OS standard music
 * directory as a base with a "GAMDL" subdirectory.
 *
 * Called by: SettingsPage (for the "Reset to default" button)
 *
 * @returns Promise resolving to the default output directory path
 */
export function getDefaultOutputPath(): Promise<string> {
  return invoke<string>('get_default_output_path');
}

// ============================================================
// Download Commands
// ============================================================

/**
 * Starts a new download and returns the unique download ID.
 *
 * Rust handler: `start_download()` in `src-tauri/src/commands/download.rs`
 * Argument: `request` - `DownloadRequest { urls, options? }`
 * Returns: string UUID of the newly created queue item
 *
 * The Rust backend:
 * 1. Creates a new queue item with a UUID
 * 2. Merges per-download options with global settings
 * 3. Spawns the GAMDL subprocess with the merged options
 * 4. Begins emitting `gamdl-output` events as the subprocess runs
 *
 * Called by: downloadStore.submitDownload()
 *
 * @param request - Download request with URLs and optional overrides
 * @returns Promise resolving to the download ID (UUID v4 string)
 */
export function startDownload(request: DownloadRequest): Promise<string> {
  return invoke<string>('start_download', { request });
}

/**
 * Cancels an active or queued download.
 *
 * Rust handler: `cancel_download()` in `src-tauri/src/commands/download.rs`
 * Argument: `downloadId` - UUID of the download to cancel
 *
 * If the download is actively running, the Rust backend kills the GAMDL
 * subprocess. If queued, it's simply removed from the pending queue.
 * Emits a `download-cancelled` event on success.
 *
 * Called by: DownloadQueue item cancel button
 *
 * @param downloadId - UUID of the download to cancel
 * @returns Promise resolving when cancellation is complete
 */
export function cancelDownload(downloadId: string): Promise<void> {
  return invoke<void>('cancel_download', { downloadId });
}

/**
 * Retries a failed or cancelled download.
 *
 * Rust handler: `retry_download()` in `src-tauri/src/commands/download.rs`
 * Argument: `downloadId` - UUID of the download to retry
 *
 * Resets the queue item state to 'queued' and re-queues it for execution
 * with the same URLs and options. Emits a `download-queued` event.
 *
 * Called by: DownloadQueue item retry button
 *
 * @param downloadId - UUID of the download to retry
 * @returns Promise resolving when the retry is queued
 */
export function retryDownload(downloadId: string): Promise<void> {
  return invoke<void>('retry_download', { downloadId });
}

/**
 * Clears all completed, failed, and cancelled items from the queue.
 *
 * Rust handler: `clear_queue()` in `src-tauri/src/commands/download.rs`
 * Returns: number of items removed
 *
 * Only removes items in terminal states (complete, error, cancelled).
 * Active and queued downloads are not affected.
 *
 * Called by: DownloadQueue "Clear Queue" button
 *
 * @returns Promise resolving to the count of cleared items
 */
export function clearQueue(): Promise<number> {
  return invoke<number>('clear_queue');
}

/**
 * Returns the current status of the entire download queue.
 *
 * Rust handler: `get_queue_status()` in `src-tauri/src/commands/download.rs`
 * Returns: `QueueStatus { total, active, queued, completed, failed, items }`
 *
 * This is the primary data-fetching command for the DownloadQueue component.
 * Also used by the sidebar to show the active download count badge.
 *
 * Called by: downloadStore.refreshQueue()
 *
 * @returns Promise resolving to the complete queue status
 */
export function getQueueStatus(): Promise<QueueStatus> {
  return invoke<QueueStatus>('get_queue_status');
}

/**
 * Checks the latest GAMDL version available on PyPI.
 *
 * Rust handler: `check_gamdl_update()` in `src-tauri/src/commands/download.rs`
 * Returns: string version (e.g., "1.5.2")
 *
 * Makes an HTTP request to the PyPI JSON API to fetch the latest version.
 *
 * @returns Promise resolving to the latest version string
 */
export function checkGamdlUpdate(): Promise<string> {
  return invoke<string>('check_gamdl_update');
}

// ============================================================
// Credential Commands
// ============================================================

/**
 * Stores a credential securely in the OS keychain (Keychain on macOS,
 * Credential Manager on Windows, Secret Service on Linux).
 *
 * Rust handler: `store_credential()` in `src-tauri/src/commands/credential.rs`
 * Arguments: `key` - credential identifier, `value` - secret value
 *
 * Used for securely storing API wrapper tokens and other secrets that
 * should not be stored in plaintext in the settings file.
 *
 * @param key - Credential identifier (e.g., "wrapper_api_token")
 * @param value - The secret value to store
 * @returns Promise resolving when the credential is stored
 */
export function storeCredential(key: string, value: string): Promise<void> {
  return invoke<void>('store_credential', { key, value });
}

/**
 * Retrieves a credential from the OS keychain.
 *
 * Rust handler: `get_credential()` in `src-tauri/src/commands/credential.rs`
 * Argument: `key` - credential identifier to look up
 * Returns: the secret value string, or null if not found
 *
 * @param key - Credential identifier to retrieve
 * @returns Promise resolving to the secret value, or null if not found
 */
export function getCredential(key: string): Promise<string | null> {
  return invoke<string | null>('get_credential', { key });
}

/**
 * Deletes a credential from the OS keychain.
 *
 * Rust handler: `delete_credential()` in `src-tauri/src/commands/credential.rs`
 * Argument: `key` - credential identifier to delete
 *
 * @param key - Credential identifier to delete
 * @returns Promise resolving when the credential is deleted
 */
export function deleteCredential(key: string): Promise<void> {
  return invoke<void>('delete_credential', { key });
}

// ============================================================
// Update Commands
// ============================================================

/**
 * Checks for updates to all application components (GAMDL, the GUI app, Python).
 *
 * Rust handler: `check_all_updates()` in `src-tauri/src/commands/update.rs`
 * Returns: `UpdateCheckResult { checked_at, has_updates, components, errors }`
 *
 * The Rust backend checks each component in parallel:
 * - GAMDL: queries PyPI JSON API for latest version
 * - GUI app: queries GitHub Releases API for latest release
 * - Python: checks the portable Python version against latest stable
 *
 * Non-fatal errors (e.g., network timeout for one component) are captured
 * in the `errors` array; other components still report their status.
 *
 * Called by: updateStore.checkForUpdates(), App.tsx auto-update effect
 *
 * @returns Promise resolving to the combined update check result
 */
export function checkAllUpdates(): Promise<UpdateCheckResult> {
  return invoke<UpdateCheckResult>('check_all_updates');
}

/**
 * Upgrades GAMDL to the latest compatible version via pip.
 *
 * Rust handler: `upgrade_gamdl()` in `src-tauri/src/commands/update.rs`
 * Returns: string message (e.g., "GAMDL upgraded to 1.5.2")
 *
 * Runs `pip install --upgrade gamdl` in the portable Python environment.
 * This may take a minute as pip resolves and downloads dependencies.
 *
 * Called by: UpdateBanner "Update" button, SettingsPage update section
 *
 * @returns Promise resolving to a success message string
 */
export function upgradeGamdl(): Promise<string> {
  return invoke<string>('upgrade_gamdl');
}

/**
 * Checks the update status of a specific component by name.
 *
 * Rust handler: `check_component_update()` in `src-tauri/src/commands/update.rs`
 * Argument: `name` - component name (e.g., "gamdl", "gamdl-gui", "python")
 * Returns: `ComponentUpdate` with version comparison and release info
 *
 * Useful for checking a single component without the overhead of checking all.
 *
 * @param name - Component name to check
 * @returns Promise resolving to the component's update status
 */
export function checkComponentUpdate(name: string): Promise<ComponentUpdate> {
  return invoke<ComponentUpdate>('check_component_update', { name });
}
