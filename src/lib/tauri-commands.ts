/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Type-safe wrappers around Tauri's invoke() IPC calls.
 * Each function maps to a #[tauri::command] handler in the Rust backend,
 * providing TypeScript type safety for both arguments and return values.
 */

import { invoke } from '@tauri-apps/api/core';
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

/** Returns platform information (OS, architecture) for theme selection */
export function getPlatformInfo(): Promise<PlatformInfo> {
  return invoke<PlatformInfo>('get_platform_info');
}

/** Returns the absolute path to the application's data directory */
export function getAppDataDir(): Promise<string> {
  return invoke<string>('get_app_data_dir');
}

// ============================================================
// Dependency Management Commands
// ============================================================

/** Checks if the portable Python runtime is installed */
export function checkPythonStatus(): Promise<DependencyStatus> {
  return invoke<DependencyStatus>('check_python_status');
}

/** Downloads and installs the portable Python runtime */
export function installPython(): Promise<string> {
  return invoke<string>('install_python');
}

/** Checks if GAMDL is installed in the Python environment */
export function checkGamdlStatus(): Promise<DependencyStatus> {
  return invoke<DependencyStatus>('check_gamdl_status');
}

/** Installs GAMDL via pip into the portable Python */
export function installGamdl(): Promise<string> {
  return invoke<string>('install_gamdl');
}

/** Returns the installation status of all external tool dependencies */
export function checkAllDependencies(): Promise<DependencyStatus[]> {
  return invoke<DependencyStatus[]>('check_all_dependencies');
}

/** Downloads and installs a specific tool dependency */
export function installDependency(name: string): Promise<string> {
  return invoke<string>('install_dependency', { name });
}

// ============================================================
// Settings Commands
// ============================================================

/** Loads the current application settings (or defaults if first run) */
export function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>('get_settings');
}

/** Saves application settings to disk and syncs to GAMDL config */
export function saveSettings(settings: AppSettings): Promise<void> {
  return invoke<void>('save_settings', { settings });
}

/** Validates a Netscape-format cookies file */
export function validateCookiesFile(path: string): Promise<CookieValidation> {
  return invoke<CookieValidation>('validate_cookies_file', { path });
}

/** Returns the default output path for downloaded music */
export function getDefaultOutputPath(): Promise<string> {
  return invoke<string>('get_default_output_path');
}

// ============================================================
// Download Commands
// ============================================================

/** Starts a new download and returns the download ID */
export function startDownload(request: DownloadRequest): Promise<string> {
  return invoke<string>('start_download', { request });
}

/** Cancels an active or queued download */
export function cancelDownload(downloadId: string): Promise<void> {
  return invoke<void>('cancel_download', { downloadId });
}

/** Retries a failed or cancelled download (resets to queued with fresh options) */
export function retryDownload(downloadId: string): Promise<void> {
  return invoke<void>('retry_download', { downloadId });
}

/** Clears all completed, failed, and cancelled items from the queue */
export function clearQueue(): Promise<number> {
  return invoke<number>('clear_queue');
}

/** Returns the current status of the download queue */
export function getQueueStatus(): Promise<QueueStatus> {
  return invoke<QueueStatus>('get_queue_status');
}

/** Checks the latest GAMDL version on PyPI */
export function checkGamdlUpdate(): Promise<string> {
  return invoke<string>('check_gamdl_update');
}

// ============================================================
// Credential Commands
// ============================================================

/** Stores a credential securely in the OS keychain */
export function storeCredential(key: string, value: string): Promise<void> {
  return invoke<void>('store_credential', { key, value });
}

/** Retrieves a credential from the OS keychain */
export function getCredential(key: string): Promise<string | null> {
  return invoke<string | null>('get_credential', { key });
}

/** Deletes a credential from the OS keychain */
export function deleteCredential(key: string): Promise<void> {
  return invoke<void>('delete_credential', { key });
}

// ============================================================
// Update Commands
// ============================================================

/** Checks for updates to all application components (GAMDL, app, Python) */
export function checkAllUpdates(): Promise<UpdateCheckResult> {
  return invoke<UpdateCheckResult>('check_all_updates');
}

/** Upgrades GAMDL to the latest compatible version via pip */
export function upgradeGamdl(): Promise<string> {
  return invoke<string>('upgrade_gamdl');
}

/** Checks the update status of a specific component by name */
export function checkComponentUpdate(name: string): Promise<ComponentUpdate> {
  return invoke<ComponentUpdate>('check_component_update', { name });
}
