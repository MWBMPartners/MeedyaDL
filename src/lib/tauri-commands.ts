/**
 * Copyright (c) 2026 MeedyaSuite
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
  ArtworkResult,
  ComponentUpdate,
  ComponentVersion,
  CookieCheckResult,
  CookieImportResult,
  CookieValidation,
  DependencyStatus,
  DetectedBrowser,
  DownloadRequest,
  PlatformInfo,
  QueueStatus,
  StartDownloadResult,
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
 * Returns: string (e.g., "~/Library/Application Support/io.github.meedyadl")
 *
 * The data directory stores settings.json, logs, and cached dependency binaries.
 *
 * @returns Promise resolving to the absolute filesystem path
 */
export function getAppDataDir(): Promise<string> {
  return invoke<string>('get_app_data_dir');
}

/**
 * Platform configuration entry from engines.toml.
 */
export interface FrontendPlatformConfig {
  id: string;
  name: string;
  icon: string | null;
  url_patterns: string[];
  enabled: boolean;
}

/**
 * Returns the platform configuration from engines.toml.
 *
 * Rust handler: `get_platform_config()` in `src-tauri/src/commands/system.rs`
 * Returns: Array of platform entries for URL detection and icon rendering.
 */
export function getPlatformConfig(): Promise<FrontendPlatformConfig[]> {
  return invoke<FrontendPlatformConfig[]>('get_platform_config');
}

/**
 * A download engine definition from engines.toml.
 */
export interface EngineDefinition {
  id: string;
  name: string;
  required: boolean;
  enabled: boolean;
  bundled: boolean;
  install_method: string;
  pip_package: string | null;
  cli_command: string;
  homepage: string;
  description: string;
}

/**
 * A platform (media service) definition from engines.toml.
 */
export interface PlatformDefinition {
  id: string;
  name: string;
  enabled: boolean;
  icon: string | null;
  url_patterns: string[];
  engines: string[];
  content_types: string[];
}

/**
 * Full engine and platform configuration from engines.toml.
 */
export interface EngineConfig {
  engines: EngineDefinition[];
  platforms: PlatformDefinition[];
}

/**
 * Returns the full engine and platform configuration from engines.toml.
 *
 * Rust handler: `get_engine_config()` in `src-tauri/src/commands/system.rs`
 * Returns: EngineConfig with all engines and platforms.
 */
export function getEngineConfig(): Promise<EngineConfig> {
  return invoke<EngineConfig>('get_engine_config');
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
 * Installs a **specific** GAMDL version (#522).
 *
 * Wraps `install_gamdl_version` in `src-tauri/src/commands/dependencies.rs`.
 * Uses `pip install --force-reinstall gamdl==<version>` so it supports
 * downgrades as well as upgrades. The standard `installGamdl()` above
 * uses `--upgrade` which only goes higher.
 *
 * Validates the version string format on the backend before pip runs.
 *
 * @param version PyPI-compatible version (e.g., "2.9.3", "3.5.2").
 * @returns Promise resolving to the post-install version reported by `pip show gamdl`.
 */
export function installGamdlVersion(version: string): Promise<string> {
  return invoke<string>('install_gamdl_version', { version });
}

/**
 * Enrichment gap detection (#759 Phase 1).
 *
 * Inspects an album directory + its `manifest.meedyadl` and reports
 * which enrichment stages (ReplayGain, AcoustID, MusicBrainz,
 * Enhanced LRC, animated artwork, …) are missing. Pure read — safe
 * to call as often as the Library Scan UI needs.
 *
 * Tag-embedded stages without a manifest record return "unknown"
 * so legacy downloads aren't falsely flagged as missing.
 */
export interface StageStatus {
  stage: string;
  status: 'complete' | 'missing' | 'unknown';
  label: string;
  key: string;
}
export interface EnrichmentGapReport {
  album_dir: string;
  audio_file_count: number;
  stages: StageStatus[];
  missing_count: number;
  complete_count: number;
  unknown_count: number;
}
export function scanEnrichmentGaps(albumDir: string): Promise<EnrichmentGapReport> {
  return invoke<EnrichmentGapReport>('scan_enrichment_gaps', { albumDir });
}

/**
 * Diagnostic bundle composer (#572 Phase 1).
 *
 * Builds a redacted Markdown bundle (version info + settings snapshot +
 * activity-log slice + output-dir tree) plus a pre-filled GitHub
 * issue URL. Privacy-first: no credentials, no file contents, no
 * auto-submit. Caller passes the activity-log slice from the
 * in-memory store + the version list from `getComponentVersions()`.
 */
export interface DiagnosticBundleInput {
  activity_log_lines: string[];
  component_versions: { name: string; version: string }[];
  user_summary?: string | null;
}
export interface DiagnosticBundle {
  markdown_body: string;
  github_issue_url: string;
  size_bytes: number;
}
export function buildDiagnosticBundle(input: DiagnosticBundleInput): Promise<DiagnosticBundle> {
  return invoke<DiagnosticBundle>('build_diagnostic_bundle', { input });
}

/**
 * Lifetime download analytics (#464). Aggregates the persistent
 * history into roll-up stats: totals, success rate, codec
 * distribution, top artist/album, last-7-day activity.
 */
export interface CodecCount { codec: string; count: number; }
export interface NameCount { name: string; count: number; }
export interface DayCount { date: string; count: number; }
export interface LifetimeStats {
  total: number;
  success: number;
  failed: number;
  success_rate: number;
  codec_distribution: CodecCount[];
  top_artist: NameCount | null;
  top_album: NameCount | null;
  last_7_days: DayCount[];
  earliest: string | null;
  latest: string | null;
}
export function getLifetimeStats(): Promise<LifetimeStats> {
  return invoke<LifetimeStats>('get_lifetime_stats');
}

/**
 * Snapshot + restore (#466).
 *
 * `createBackup` writes a timestamped snapshot of settings / queue /
 * history into `{appDataDir}/backups/<YYYYMMDD-HHMMSS>/`.
 * `listBackups` returns every existing snapshot, newest first.
 * `restoreFromBackup` overwrites the live state files from a chosen
 *   snapshot — the caller should prompt the user to restart MeedyaDL
 *   so the in-memory caches don't diverge from disk.
 * `deleteBackup` removes a single snapshot directory.
 */
export interface BackupSummary {
  snapshot_path: string;
  files: string[];
  total_snapshots: number;
}
export interface RestoreSummary {
  snapshot_path: string;
  restored: string[];
  skipped: string[];
}
export interface BackupEntry {
  path: string;
  name: string;
  size_bytes: number;
  file_count: number;
}
export function createBackup(): Promise<BackupSummary> {
  return invoke<BackupSummary>('create_backup');
}
export function listBackups(): Promise<BackupEntry[]> {
  return invoke<BackupEntry[]>('list_backups');
}
export function restoreFromBackup(snapshotPath: string): Promise<RestoreSummary> {
  return invoke<RestoreSummary>('restore_from_backup', { snapshotPath });
}
export function deleteBackup(snapshotPath: string): Promise<void> {
  return invoke<void>('delete_backup', { snapshotPath });
}

/**
 * Returns the MeedyaDL-tested GAMDL version window (#522).
 *
 * The frontend uses this to render the "Install recommended" button
 * label, the support badge, and to validate user-typed versions
 * against the known range.
 *
 * Reads from compiled-in `tool-versions.toml` — zero I/O, always succeeds.
 */
export interface GamdlSupportWindow {
  minimum: string;
  maximum_tested: string;
  recommended: string;
}
export function getGamdlSupportWindow(): Promise<GamdlSupportWindow> {
  return invoke<GamdlSupportWindow>('get_gamdl_support_window');
}

/**
 * Snapshot of the active GAMDL capability flags for the frontend (#853).
 *
 * Mirrors `GamdlCapabilities` in `src-tauri/src/commands/dependencies.rs`.
 *
 * Used by Settings > Advanced > Wrapper to choose between the v1
 * three-fields UI and the v2 single-URL UI, and to gate other panes
 * that need version-aware behaviour. Every flag is `false` when the
 * GAMDL version cache hasn't been populated yet (e.g. immediately
 * after launch before the first dependency check) — defensive default.
 */
export interface GamdlCapabilities {
  /** ≥3.6 — uses wrapper-v2 single HTTP endpoint */
  wrapper_v2: boolean;
  /** ≥3.6 — no external ffmpeg/mp4box/mp4decrypt CLI options */
  native_muxing: boolean;
  /** ≥3.6 — aac-web / aac-he-web (was aac-legacy / aac-he-legacy) */
  aac_web_codec_rename: boolean;
  /** ≤3.5.x — --music-video-remux-mode still accepted */
  music_video_remux_mode: boolean;
  /** 3.1–3.5.x — --wrapper-m3u8-ip accepted */
  wrapper_m3u8_ip: boolean;
  /** ≥3.0 — --playlist-folder-template accepted */
  playlist_folder_template: boolean;
  /** ≥2.9.1 — native --song-codec-priority chain accepted */
  native_codec_priority: boolean;
  /**
   * `--ffmpeg-path` / `ffmpeg_path` INI key — `true` when `<3.6` (original
   * tool-path era) OR `>=3.7` (REINSTATED for N_m3u8DL-RE's HLS streaming).
   * Only `false` on the `3.6.x` line. Used to grey out the FFmpeg-path
   * input in Settings → Tools when GAMDL 3.6.x is detected. (#867)
   */
  ffmpeg_path: boolean;
}
export function getGamdlCapabilities(): Promise<GamdlCapabilities> {
  return invoke<GamdlCapabilities>('get_gamdl_capabilities');
}

/**
 * Returns the installation status of votify (Spotify engine).
 *
 * Rust handler: `check_votify_status()` in `src-tauri/src/commands/dependencies.rs`
 */
export function checkVotifyStatus(): Promise<DependencyStatus> {
  return invoke<DependencyStatus>('check_votify_status');
}

/**
 * Installs votify via pip into the managed Python environment.
 *
 * Rust handler: `install_votify()` in `src-tauri/src/commands/dependencies.rs`
 */
export function installVotify(): Promise<string> {
  return invoke<string>('install_votify');
}

/**
 * Returns the installation status of OF-Scraper (optional engine, disabled by default).
 *
 * Rust handler: `check_ofscraper_status()` in `src-tauri/src/commands/dependencies.rs`
 * Note: Currently disabled in engines.toml (enabled = false).
 */
export function checkOfscraperStatus(): Promise<DependencyStatus> {
  return invoke<DependencyStatus>('check_ofscraper_status');
}

/**
 * Installs OF-Scraper via pip into the managed Python environment.
 *
 * Rust handler: `install_ofscraper()` in `src-tauri/src/commands/dependencies.rs`
 * Note: Currently disabled in engines.toml (enabled = false).
 */
export function installOfscraper(): Promise<string> {
  return invoke<string>('install_ofscraper');
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

/**
 * Retrieves version information for all MeedyaDL components.
 *
 * Rust handler: `get_component_versions()` in `src-tauri/src/commands/dependencies.rs`
 * Returns: Array of component version entries (name, version, installed status)
 *
 * Unlike `checkAllDependencies` (which skips version detection for speed),
 * this runs `--version` on each installed tool. Used by Help > About and
 * Activity Log startup messages.
 *
 * @returns Promise resolving to an array of ComponentVersion objects
 */
export function getComponentVersions(): Promise<ComponentVersion[]> {
  return invoke<ComponentVersion[]>('get_component_versions');
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
 * Checks whether a built-in AcoustID API key was embedded at compile time.
 *
 * Rust handler: `has_embedded_acoustid_key()` in `src-tauri/src/commands/settings.rs`
 * Returns: `true` if release builds include a pre-configured key, `false` for dev builds.
 *
 * Called by: MetadataTab (to adjust UI text for built-in vs user-provided key)
 *
 * @returns Promise resolving to boolean
 */
export function hasEmbeddedAcoustidKey(): Promise<boolean> {
  return invoke<boolean>('has_embedded_acoustid_key');
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
 * Checks whether cookies are valid and ready for an Apple Music download.
 *
 * Rust handler: `check_cookies_before_download()` in `src-tauri/src/commands/settings.rs`
 * Returns: `CookieCheckResult` with readiness flag and optional message
 *
 * Called by the download form before queuing a download. This catches
 * expired or missing cookies at submission time so the user gets
 * immediate feedback instead of a delayed GAMDL failure.
 *
 * Wrapper users bypass this check (the wrapper handles authentication).
 */
export function checkCookiesBeforeDownload(): Promise<CookieCheckResult> {
  return invoke<CookieCheckResult>('check_cookies_before_download');
}

/**
 * Checks whether the internet is reachable before queuing a download.
 *
 * Rust handler: `check_internet_before_download()` in `src-tauri/src/commands/settings.rs`
 * Returns: `CookieCheckResult` (reuses same shape) with readiness flag and optional message
 *
 * Called by the download form before the cookie check. Reuses the existing
 * health_check_service::check_internet_connectivity() (HTTP GET to apple.com).
 * If offline, blocks the download with an amber warning.
 */
export function checkInternetBeforeDownload(): Promise<CookieCheckResult> {
  return invoke<CookieCheckResult>('check_internet_before_download');
}

/**
 * Checks that the output directory is writable before queuing a download.
 *
 * Rust handler: `check_output_path_before_download()` in `src-tauri/src/commands/settings.rs`
 * Returns: `CookieCheckResult` (reuses same shape) with readiness flag and optional message
 *
 * Called by the download form after the internet check and before the cookie
 * check. Catches disconnected cloud mounts, full disks, and permission issues
 * before the download is queued.
 */
export function checkOutputPathBeforeDownload(): Promise<CookieCheckResult> {
  return invoke<CookieCheckResult>('check_output_path_before_download');
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
 * Starts a new download and returns the result including the download ID
 * and an optional duplicate warning.
 *
 * Rust handler: `start_download()` in `src-tauri/src/commands/gamdl.rs`
 * Argument: `request` - `DownloadRequest { urls, options? }`
 * Returns: `StartDownloadResult` with download ID and optional duplicate warning
 *
 * The Rust backend:
 * 1. Checks for duplicate URLs already in the active queue
 * 2. Creates a new queue item with a UUID
 * 3. Merges per-download options with global settings
 * 4. Spawns the GAMDL subprocess with the merged options
 * 5. Begins emitting `gamdl-output` events as the subprocess runs
 *
 * Called by: downloadStore.submitDownload()
 *
 * @param request - Download request with URLs and optional overrides
 * @param skipAutoStart - When true, the item is queued but queue processing
 *   is not triggered. Used when the device is offline.
 * @returns Promise resolving to the start download result
 */
export function startDownload(
  request: DownloadRequest,
  skipAutoStart?: boolean,
): Promise<StartDownloadResult> {
  return invoke<StartDownloadResult>('start_download', { request, skipAutoStart });
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
 * Summary returned by {@link abortAllDownloads} describing how many items
 * were stopped, grouped by their pre-abort state. Mirrors the Rust
 * `AbortSummary` struct in `download_queue.rs` (#620).
 *
 * Items already in `Complete` / `Cancelled` / `Error` are not counted —
 * the user keeps their history.
 */
export interface AbortSummary {
  queuedCancelled: number;
  downloadingStopped: number;
  processingStopped: number;
}

/**
 * Aborts every active and queued download in one IPC call (#620).
 *
 * Rust handler: `abort_all_downloads()` in `src-tauri/src/commands/gamdl.rs`.
 *
 * Unlike per-item cancel (which leaves the subprocess running until the
 * cancellation-poll loop ticks), this is the "stop everything now" escape
 * hatch — every `Queued` / `Downloading` / `Processing` item transitions
 * to `Cancelled` in one shot. The already-running cancellation polls then
 * reap their subprocesses on the next tick.
 *
 * Terminal items (`Complete` / `Cancelled` / `Error`) are untouched so
 * the user keeps their history.
 *
 * Emits a `downloads-aborted` event carrying the {@link AbortSummary}.
 *
 * @returns summary of items stopped, grouped by pre-abort state.
 */
export function abortAllDownloads(): Promise<AbortSummary> {
  return invoke<AbortSummary>('abort_all_downloads');
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
 * Result of a `retry_failed_bulk` call (#835).
 *
 * `requested` is the input length. `retried` is the count of items
 * that actually transitioned back to Queued. `skipped` is one entry
 * per item that couldn't be retried, with the reason — typically
 * "All N tracks already on disk" (smart-retry planner) or "Item is
 * not in a retryable state" (e.g. user-cancelled mid-flight, item
 * already completed before the bulk request landed).
 */
export interface BulkRetrySkipped {
  id: string;
  reason: string;
}

export interface BulkRetryReport {
  requested: number;
  retried: number;
  skipped: BulkRetrySkipped[];
}

/**
 * Bulk-retry every failed download in one IPC round-trip (#835).
 *
 * Pre-#835 the Queue page's "Retry All Failed" button mapped over
 * the failed items and called `retryDownload` per item via
 * `Promise.allSettled`. Each call held the queue mutex 3 times,
 * peeked smart-retry, persisted queue.json, and kicked
 * `process_queue` — 29 failed items = ~90 mutex acquisitions and
 * 29 disk writes, taking minutes on the affected user's system.
 *
 * This wrapper hands the whole batch to the Rust side which does
 * the same per-item work but persists / kicks process_queue / emits
 * one summary log line — ONCE for the whole batch instead of N
 * times. Per-item `download-queued` events are still emitted so
 * the per-row UI animation works as before.
 *
 * Always resolves (per-item failures are aggregated in `skipped`).
 */
export function retryFailedBulk(downloadIds: string[]): Promise<BulkRetryReport> {
  return invoke<BulkRetryReport>('retry_failed_bulk', { downloadIds });
}

/**
 * Move a `Queued` item to the top of the pending sub-sequence (#782).
 *
 * Rust handler: `move_queue_item_to_top()` in
 * `src-tauri/src/commands/gamdl.rs`.
 *
 * Active items (`Downloading` / `Processing`) keep their absolute slot;
 * the moved item lands at the front of the queued sub-sequence and
 * becomes the next item `next_pending` will pick. Reorders are
 * persisted to `queue.json` and survive an app restart.
 *
 * Returns `true` when the queue mutated, `false` for a no-op (item
 * not found, not Queued, or already at the top).
 *
 * @param downloadId - UUID of the queued item to promote
 * @returns true if the queue changed
 */
export function moveQueueItemToTop(downloadId: string): Promise<boolean> {
  return invoke<boolean>('move_queue_item_to_top', { downloadId });
}

/**
 * Move a `Queued` item to the bottom of the pending sub-sequence (#782).
 *
 * Rust handler: `move_queue_item_to_bottom()` in
 * `src-tauri/src/commands/gamdl.rs`.
 *
 * @param downloadId - UUID of the queued item to demote
 * @returns true if the queue changed
 */
export function moveQueueItemToBottom(downloadId: string): Promise<boolean> {
  return invoke<boolean>('move_queue_item_to_bottom', { downloadId });
}

/**
 * Swap a `Queued` item with the `Queued` item immediately above it (#782).
 *
 * Skips over any non-Queued items (active downloads, completed items
 * from prior sessions) — the swap is *within* the queued sub-sequence.
 *
 * @param downloadId - UUID of the queued item to move up one position
 * @returns true if the queue changed
 */
export function moveQueueItemUp(downloadId: string): Promise<boolean> {
  return invoke<boolean>('move_queue_item_up', { downloadId });
}

/**
 * Swap a `Queued` item with the `Queued` item immediately below it (#782).
 *
 * @param downloadId - UUID of the queued item to move down one position
 * @returns true if the queue changed
 */
export function moveQueueItemDown(downloadId: string): Promise<boolean> {
  return invoke<boolean>('move_queue_item_down', { downloadId });
}

/**
 * Retries a failed download with wrapper authentication disabled.
 *
 * Rust handler: `retry_download_without_wrapper()` in `src-tauri/src/commands/gamdl.rs`
 * Argument: `downloadId` - UUID of the download to retry
 *
 * Similar to retryDownload, but explicitly disables the wrapper system
 * and falls back to cookie-based authentication. Only applicable to
 * downloads that originally used the wrapper.
 *
 * Called by: DownloadQueue "Retry without Wrapper" button
 *
 * @param downloadId - UUID of the download to retry without wrapper
 * @returns Promise resolving when the retry is queued
 */
export function retryDownloadWithoutWrapper(downloadId: string): Promise<void> {
  return invoke<void>('retry_download_without_wrapper', { downloadId });
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
 * Clears ALL non-active items from the queue (completed, cancelled,
 * errored, and queued). Active downloads are preserved.
 *
 * Rust handler: `clear_all_queue()` in `src-tauri/src/commands/gamdl.rs`
 *
 * @returns Promise resolving to the count of cleared items
 */
export function clearAllQueue(): Promise<number> {
  return invoke<number>('clear_all_queue');
}

/**
 * Removes a single queue item by ID (#685).
 *
 * Rust handler: `delete_queue_item()` in `src-tauri/src/commands/gamdl.rs`
 *
 * Backend refuses active items (`Downloading` / `Processing`) — the caller
 * must `cancelDownload()` first. The frontend already hides the menu
 * entry for active rows, so the rejection is defense-in-depth.
 *
 * @param downloadId - UUID of the queue item to remove.
 * @returns Promise resolving when the item has been removed and persisted.
 */
export function deleteQueueItem(downloadId: string): Promise<void> {
  return invoke<void>('delete_queue_item', { downloadId });
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

/**
 * Exports the current download queue to a `.meedyadl` file.
 *
 * Rust handler: `export_queue()` in `src-tauri/src/commands/gamdl.rs`
 * Returns: number of items exported
 *
 * Opens a native save dialog with the `.meedyadl` file filter.
 * Only non-terminal items (queued/active) are exported.
 *
 * Called by: downloadStore.exportQueue()
 *
 * @returns Promise resolving to the count of exported items
 */
export function exportQueue(): Promise<number> {
  return invoke<number>('export_queue');
}

/**
 * Imports download queue items from a `.meedyadl` file.
 *
 * Rust handler: `import_queue()` in `src-tauri/src/commands/gamdl.rs`
 * Returns: number of items imported
 *
 * Opens a native file picker dialog with the `.meedyadl` file filter.
 * Imported items are enqueued as new downloads and processing starts.
 *
 * Called by: downloadStore.importQueue()
 *
 * @returns Promise resolving to the count of imported items
 */
export function importQueue(): Promise<number> {
  return invoke<number>('import_queue');
}

/**
 * Manually triggers download queue processing.
 *
 * Rust handler: `process_queue_manual()` in `src-tauri/src/commands/gamdl.rs`
 *
 * Used when `auto_start_queue` is disabled and the user wants to start
 * processing queued items. Also useful in auto mode if processing stalled.
 *
 * Called by: downloadStore.processQueue(), DownloadQueue "Start Queue" button
 *
 * @returns Promise resolving when processing has been triggered
 */
export function processQueue(): Promise<void> {
  return invoke<void>('process_queue_manual');
}

/**
 * Exports activity log entries to a `.log` file via a native save dialog.
 *
 * Rust handler: `export_activity_log()` in `src-tauri/src/commands/gamdl.rs`
 *
 * Called by: ActivityLog component's Export button
 *
 * @param entries - Array of ActivityLogEntry objects from the activity store
 * @returns Promise resolving to the count of exported entries
 */
export function exportActivityLog(entries: import('@/types').ActivityLogEntry[]): Promise<number> {
  return invoke<number>('export_activity_log', { entries });
}

/**
 * Exports the persistent on-disk activity log (#541) — the complete
 * forensic record written by `services::activity_log_writer`, not just
 * the in-memory (possibly-trimmed) entries.
 *
 * Concatenates the most recent N daily `activity-YYYY-MM-DD.log` files
 * (default 3) and saves them via a native save dialog.
 *
 * Rust handler: `export_disk_activity_log()` in
 * `src-tauri/src/commands/activity_log.rs`
 *
 * @param daysBack - Number of daily log files to include (default 3, minimum 1).
 * @returns Promise resolving to the number of bytes written to the exported file.
 */
export function exportDiskActivityLog(daysBack?: number): Promise<number> {
  return invoke<number>('export_disk_activity_log', { daysBack });
}

/**
 * Returns the absolute path of the active on-disk activity log
 * directory, creating it if necessary. Pair with
 * `@tauri-apps/plugin-shell`'s `open()` to reveal the folder in the
 * OS file manager (Finder / Explorer / xdg-open).
 *
 * Rust handler: `get_logs_folder_path()` in
 * `src-tauri/src/commands/activity_log.rs`
 *
 * Honours the user's `activity_log_path_override` setting when set.
 */
export function getLogsFolderPath(): Promise<string> {
  return invoke<string>('get_logs_folder_path');
}

/**
 * Returns the verbatim content of `ACKNOWLEDGEMENTS.md` (the inventory
 * of every direct dependency, its licence, and its purpose) — embedded
 * into the binary at compile time via `include_str!()`, so this is
 * always available on every platform regardless of bundle.resources
 * config.
 *
 * Rust handler: `get_acknowledgements_text()` in
 * `src-tauri/src/commands/legal.rs` (#802).
 */
export function getAcknowledgementsText(): Promise<string> {
  return invoke<string>('get_acknowledgements_text');
}

/**
 * Returns the verbatim content of `THIRD_PARTY_LICENSES.md` (the
 * actual MIT/BSD/Unlicense/LGPL/GPL/PSF licence text for the bundled
 * engines and tools, plus the written offer for source code for
 * LGPL/GPL components shipped in offline-installer builds). Embedded
 * at compile time via `include_str!()`.
 *
 * Rust handler: `get_third_party_licenses_text()` in
 * `src-tauri/src/commands/legal.rs` (#802).
 */
export function getThirdPartyLicensesText(): Promise<string> {
  return invoke<string>('get_third_party_licenses_text');
}

/**
 * Imports a `.meedyadl` manifest file via a native open dialog.
 *
 * Rust handler: `import_manifest()` in `src-tauri/src/commands/gamdl.rs`
 *
 * @returns Promise resolving to an array of download URLs from the manifest
 */
export function importManifest(): Promise<string[]> {
  return invoke<string[]>('import_manifest');
}

/**
 * Scans a folder for `manifest.meedyadl` files and returns metadata
 * from each discovered manifest (#456). Opens a native folder picker.
 *
 * Rust handler: `scan_folder_for_manifests()` in `src-tauri/src/commands/gamdl.rs`
 *
 * @returns Promise resolving to an array of scanned manifest data
 */
export function scanFolderForManifests(): Promise<ScannedManifest[]> {
  return invoke<ScannedManifest[]>('scan_folder_for_manifests');
}

// ============================================================
// Legacy sibling-folder merge (#789)
// ============================================================

/** A `Foo` + `Foo [Explicit]` sibling pair detected on disk. */
export interface SiblingPair {
  parent: string;
  unsuffixed_path: string;
  suffixed_path: string;
  suffix: string;
  album_basename: string;
}

/** Dry-run summary returned by `previewLegacyFolderMerge`. */
export interface MergePreview {
  pair: SiblingPair;
  audio_count: number;
  sidecar_count: number;
  other_count: number;
  potential_collisions: string[];
  will_merge_manifest: boolean;
}

/** Outcome of `executeLegacyFolderMerge`. */
export interface MergeReport {
  pair: SiblingPair;
  audio_moved: number;
  sidecars_moved: number;
  other_moved: number;
  collisions_disambiguated: string[];
  manifest_merged: boolean;
  source_folder_removed: boolean;
  warnings: string[];
}

/**
 * Walk a previously-scanned folder for sibling pairs left over
 * from pre-#528 downloads. Defensive — only returns pairs whose
 * un-suffixed sibling's audio carries the matching `rtng` advisory
 * rating, so unrelated folders that happen to have similar names
 * are never offered for merge.
 *
 * Rust handler: `detect_legacy_folder_pairs()` in `gamdl.rs`.
 */
export function detectLegacyFolderPairs(
  folderPath: string
): Promise<SiblingPair[]> {
  return invoke<SiblingPair[]>('detect_legacy_folder_pairs', { folderPath });
}

/**
 * Dry-run preview of what `executeLegacyFolderMerge` would do for
 * `pair`. Returns file counts + potential collisions WITHOUT
 * touching disk; powers the confirmation UI.
 *
 * Rust handler: `preview_legacy_folder_merge()` in `gamdl.rs`.
 */
export function previewLegacyFolderMerge(
  pair: SiblingPair
): Promise<MergePreview> {
  return invoke<MergePreview>('preview_legacy_folder_merge', { pair });
}

/**
 * Perform the merge. Renames audio + sidecars in the source folder
 * to add the advisory suffix, moves all files into the suffixed
 * sibling (auto-disambiguates collisions), merges `.meedyadl`
 * manifests, and removes the source folder if empty.
 *
 * Rust handler: `execute_legacy_folder_merge()` in `gamdl.rs`.
 */
export function executeLegacyFolderMerge(
  pair: SiblingPair
): Promise<MergeReport> {
  return invoke<MergeReport>('execute_legacy_folder_merge', { pair });
}

/** Result of scanning a folder for manifest files (#456). */
export interface ScannedManifest {
  /** Path to the manifest file on disk */
  manifest_path: string;
  /** Album directory containing the manifest */
  album_dir: string;
  /** Source URLs extracted from the manifest */
  urls: string[];
  /** Platform (e.g., "apple-music") */
  platform: string | null;
  /** Artist name (inferred from directory structure) */
  artist: string | null;
  /** Album name (inferred from directory name) */
  album: string | null;
  /** When this source was last downloaded (ISO 8601) */
  downloaded_at: string | null;
  /** Track count from the manifest */
  track_count: number;
  /** Current codec detected from files on disk (e.g., "aac", "alac") (#380) */
  current_codec: string | null;
  /** Number of audio files in the album directory */
  audio_file_count: number;
  /** Apple Music lastModifiedDate from manifest — for content refresh detection (#380) */
  last_modified_date: string | null;
}

/**
 * Smart-retry diff result for a single Library Scan row (Phase 5b, #717).
 *
 * Mirrors the Rust `LibraryScanDiff` enum (tagged-union by `kind`):
 * - `plan`: at least one track is missing; UI renders "X of Y missing".
 * - `all_present`: every track on disk; UI renders "All present".
 * - `not_applicable`: manifest missing/malformed/no-source-match; UI
 *   renders a neutral "Cannot diff" badge.
 *
 * Rust handler: `diff_library_scan_manifest()` in `src-tauri/src/commands/gamdl.rs`
 */
export type LibraryScanDiff =
  | { kind: 'plan'; missing_tracks: number; total_tracks: number }
  | { kind: 'all_present'; total_tracks: number }
  | { kind: 'not_applicable' };

/**
 * Diffs a scanned manifest against its on-disk state to determine
 * whether tracks are missing.
 *
 * Cheap (filesystem ops only — no network), safe to run synchronously
 * per row from the Library Scan page. Wraps the existing
 * `smart_retry_planner::plan_retry`.
 *
 * @param albumDir - The album directory path (parent of manifest.meedyadl).
 *                   Use `ScannedManifest.album_dir`.
 * @param sourceUrl - The manifest source URL to diff against. Use
 *                    `ScannedManifest.urls[0]`.
 */
export function diffLibraryScanManifest(
  albumDir: string,
  sourceUrl: string
): Promise<LibraryScanDiff> {
  return invoke<LibraryScanDiff>('diff_library_scan_manifest', {
    albumDir,
    sourceUrl,
  });
}

/**
 * Library Scan freshness result for a single row (#717 sub-feature 5c).
 *
 * Mirrors the Rust `LibraryScanFreshness` enum (tagged union).
 *
 * - `fresh` — manifest's `lastModifiedDate` matches Apple Music API.
 * - `updated` — Apple now reports a newer date; album may have gained
 *   tracks, swapped a mix, or earned Apple Digital Master.
 * - `unknown` — credentials missing, network error, or no manifest
 *   date to compare. UI renders no badge for this case.
 *
 * Rust handler: `check_library_scan_freshness()` in
 * `src-tauri/src/commands/gamdl.rs`
 */
export type LibraryScanFreshness =
  | { kind: 'fresh'; current_date: string }
  | { kind: 'updated'; manifest_date: string; current_date: string }
  | { kind: 'unknown'; reason: string };

/**
 * Calls Apple Music API for the URL's album and compares its
 * `lastModifiedDate` against the manifest-recorded value.
 *
 * Costs one HTTP request; the frontend rate-limits the dispatch to
 * keep large libraries from saturating the API. Returns `unknown`
 * cleanly for the typical "user has no MusicKit credentials" case
 * (this is the resting state for most users).
 *
 * @param sourceUrl - Album URL from `ScannedManifest.urls[0]`.
 * @param manifestDate - Recorded `last_modified_date` from the
 *   manifest, if any (`ScannedManifest.last_modified_date`). Pass
 *   `null` when the manifest has no recorded value — the IPC will
 *   return `unknown` instead of comparing.
 */
export function checkLibraryScanFreshness(
  sourceUrl: string,
  manifestDate: string | null
): Promise<LibraryScanFreshness> {
  return invoke<LibraryScanFreshness>('check_library_scan_freshness', {
    sourceUrl,
    manifestDate,
  });
}

/**
 * Checks whether a URL was previously downloaded for smart re-download
 * detection (#263). Returns metadata about the previous download if found.
 *
 * Rust handler: `check_redownload_status()` in `src-tauri/src/commands/gamdl.rs`
 *
 * @param url - The Apple Music URL to check
 * @returns Previous download info if found, null if first download
 */
export function checkRedownloadStatus(
  url: string
): Promise<{ downloaded_at: string; title: string | null; album: string | null } | null> {
  return invoke('check_redownload_status', { url });
}

// ============================================================
// Settings Utility Commands
// ============================================================

/**
 * Tests connectivity to the wrapper service at the given URL.
 *
 * Rust handler: `test_wrapper_connection()` in `src-tauri/src/commands/settings.rs`
 *
 * Makes an HTTP GET request with a 5-second timeout. Any HTTP response
 * (even 404) counts as "reachable" — we test network connectivity, not
 * endpoint correctness.
 *
 * Called by: AdvancedTab "Test Connection" button
 *
 * @param url - The wrapper account URL to test
 * @returns Promise resolving to the test result
 */
export function testWrapperConnection(url: string): Promise<import('@/types').WrapperTestResult> {
  return invoke<import('@/types').WrapperTestResult>('test_wrapper_connection', { url });
}

/**
 * Exports application settings to a JSON file via a native save dialog.
 *
 * Rust handler: `export_settings()` in `src-tauri/src/commands/settings.rs`
 * Returns: string path where the file was saved
 *
 * Sensitive fields (cookies path, wrapper URL, MusicKit credentials,
 * AcoustID API key) are cleared before export to prevent accidental
 * credential sharing.
 *
 * Called by: GeneralTab "Export Settings" button
 *
 * @returns Promise resolving to the export file path
 */
export function exportSettings(): Promise<string> {
  return invoke<string>('export_settings');
}

/**
 * Imports application settings from a JSON file via a native file picker.
 *
 * Rust handler: `import_settings()` in `src-tauri/src/commands/settings.rs`
 *
 * The selected file must be a valid MeedyaDL settings export (version 1).
 * Imported settings overwrite all non-sensitive fields. Sensitive fields
 * (cookies path, wrapper URL, MusicKit credentials, AcoustID API key)
 * are preserved from the current settings.
 *
 * Called by: GeneralTab "Import Settings" button
 *
 * @returns Promise resolving when import is complete
 */
export function importSettings(): Promise<void> {
  return invoke<void>('import_settings');
}

// ============================================================
// Profile Bundle Commands (#876)
// ============================================================

/**
 * Options for `exportProfile`. All fields are optional with sensible
 * defaults — queue + history are ON, everything else (DB, credentials,
 * activity log, manifests) is OFF.
 */
export interface ExportProfileOptions {
  include_queue?: boolean;
  include_history?: boolean;
  include_database?: boolean;
  include_credentials?: boolean;
  include_activity_log?: boolean;
  include_manifests?: boolean;
  note?: string | null;
}

/** Result of a successful `exportProfile` call. */
export interface ExportProfileResult {
  path: string;
  size_bytes: number;
  sections: string[];
}

/**
 * Exports the current install's profile to a `.meedyabundle` archive
 * via a native save dialog. Returns the resolved path + size + the
 * list of OPTIONAL sections actually included (settings is always
 * present).
 *
 * Rust handler: `export_profile()` in
 * `src-tauri/src/commands/profile_bundle.rs`.
 *
 * Called by: GeneralTab "Export Profile" button.
 */
export function exportProfile(
  options: ExportProfileOptions = {},
): Promise<ExportProfileResult> {
  return invoke<ExportProfileResult>('export_profile', { options });
}

/**
 * Summary returned by `peekProfileBundle` — what the bundle
 * contains, surfaced to the import wizard before the user confirms.
 */
export interface ProfileBundleSummary {
  bundle_version: number;
  meedyadl_version: string;
  platform: string;
  exported_at: string;
  exported_by: string | null;
  note: string | null;
  sections: string[];
  source_path: string;
}

/**
 * Inspect a `.meedyabundle` without extracting anything (#876 P3).
 * Pass `null`/omit to open a native file picker; pass an absolute
 * path to skip the picker (used by P5's first-launch auto-detect).
 */
export function peekProfileBundle(
  path?: string | null,
): Promise<ProfileBundleSummary> {
  return invoke<ProfileBundleSummary>('peek_profile_bundle', {
    path: path ?? null,
  });
}

/** Per-section conflict action: skip the existing file or replace it. */
export type ImportConflictAction = 'skip' | 'replace';

/** Options for `importProfile`. Source path is required (typically
 *  threaded from a prior `peekProfileBundle` call). */
export interface ImportProfileOptions {
  source_path: string;
  settings?: ImportConflictAction;
  queue?: ImportConflictAction;
  history?: ImportConflictAction;
  database?: ImportConflictAction;
  activity_log?: ImportConflictAction;
  manifests?: ImportConflictAction;
  credentials?: ImportConflictAction;
}

/** Result returned by `importProfile`. */
export interface ImportProfileResult {
  settings_restored: boolean;
  queue_restored: boolean;
  history_restored: boolean;
  database_restored: boolean;
  activity_log_files_restored: number;
  manifests_restored: number;
  credentials_skipped_p4: boolean;
}

/**
 * Restore selected sections from a `.meedyabundle` (#876 P3).
 * The user picks per-section conflict actions in the import
 * wizard before invoking this. The caller should encourage a
 * restart afterwards because background tasks (queue processor,
 * activity-log writer) hold the affected files open.
 */
export function importProfile(
  options: ImportProfileOptions,
): Promise<ImportProfileResult> {
  return invoke<ImportProfileResult>('import_profile', { options });
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

/**
 * Validates MusicKit credentials by generating a JWT and testing it
 * against the Apple Music API.
 *
 * Rust handler: `validate_musickit_credentials()` in `src-tauri/src/commands/credentials.rs`
 *
 * @returns Promise resolving to a success message, or rejecting with a
 *          descriptive error explaining what to check.
 */
export function validateMusicKitCredentials(): Promise<string> {
  return invoke<string>('validate_musickit_credentials');
}

/**
 * Validates MusicKit credentials using explicit Team ID and Key ID values.
 *
 * Rust handler: `validate_musickit_credentials()` in `src-tauri/src/commands/credentials.rs`
 *
 * This variant is used by Settings UI so validation tests the current
 * unsaved field values rather than stale values persisted on disk.
 */
export function validateMusicKitCredentialsWithInput(
  teamId: string | null,
  keyId: string | null
): Promise<string> {
  return invoke<string>('validate_musickit_credentials', { teamId, keyId });
}

/**
 * Checks whether a build-time MusicKit developer token is embedded.
 *
 * Rust handler: `has_embedded_musickit_token()` in `src-tauri/src/commands/credentials.rs`
 */
export function hasEmbeddedMusicKitToken(): Promise<boolean> {
  return invoke<boolean>('has_embedded_musickit_token');
}

// ============================================================
// Web Player Token Commands
// ============================================================

/**
 * Returns true if a web player developer token is stored in the OS keychain.
 *
 * Rust handler: `has_webplayer_token()` in `src-tauri/src/commands/credentials.rs`
 */
export function hasWebplayerToken(): Promise<boolean> {
  return invoke<boolean>('has_webplayer_token');
}

/**
 * Deletes the web player developer token from the OS keychain.
 *
 * Rust handler: `clear_webplayer_token()` in `src-tauri/src/commands/credentials.rs`
 */
export function clearWebplayerToken(): Promise<void> {
  return invoke<void>('clear_webplayer_token');
}

// ============================================================
// Developer Access Commands
// ============================================================

/**
 * Checks whether developer access is currently active.
 *
 * Rust handler: `check_dev_access()` in `src-tauri/src/commands/credentials.rs`
 */
export function checkDevAccess(): Promise<boolean> {
  return invoke<boolean>('check_dev_access');
}

/**
 * Activates developer access after validating the passphrase.
 * Returns true on success, false on incorrect passphrase.
 *
 * Rust handler: `activate_dev_access()` in `src-tauri/src/commands/credentials.rs`
 */
export function activateDevAccess(passphrase: string): Promise<boolean> {
  return invoke<boolean>('activate_dev_access', { passphrase });
}

/**
 * Deactivates developer access.
 *
 * Rust handler: `deactivate_dev_access()` in `src-tauri/src/commands/credentials.rs`
 */
export function deactivateDevAccess(): Promise<void> {
  return invoke<void>('deactivate_dev_access');
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
 * Upgrades GAMDL via pip.
 *
 * Rust handler: `upgrade_gamdl()` in `src-tauri/src/commands/updates.rs`
 * Returns: the new GAMDL version string (e.g., "3.3").
 *
 * Runs `pip install --upgrade gamdl` in the portable Python environment.
 * This may take a minute as pip resolves and downloads dependencies.
 *
 * @param targetVersion - When provided, pip pins to exactly this version
 *   (`gamdl=={targetVersion}`). The Updates page passes the latest PyPI
 *   version when the user is upgrading to an above-ceiling "Untested"
 *   release — without the pin, the bounded support-window spec would
 *   silently resolve down to `maximum_tested_version`. Omit this for
 *   routine "Upgrade" clicks on tested updates.
 *
 * Called by: UpdateBanner / UpdatesPage "Upgrade" buttons
 *
 * @returns Promise resolving to the installed version string
 */
export function upgradeGamdl(targetVersion?: string): Promise<string> {
  // Pass `null` (not `undefined`) when no explicit target is requested —
  // JSON.stringify drops `undefined` keys, which can interact awkwardly
  // with how Tauri 2 deserialises `Option<String>` arguments. Explicit
  // `null` always round-trips cleanly to `None` on the Rust side.
  return invoke<string>('upgrade_gamdl', {
    targetVersion: targetVersion ?? null,
  });
}

/**
 * Upgrades any pip-based engine to the latest version.
 *
 * Rust handler: `upgrade_pip_engine()` in `src-tauri/src/commands/updates.rs`
 * Works for votify, yt-dlp, ofscraper, etc. GAMDL has its own upgrade path.
 */
export function upgradePipEngine(packageName: string): Promise<string> {
  return invoke<string>('upgrade_pip_engine', { package: packageName });
}

/**
 * Checks the update status of a specific component by name.
 *
 * Rust handler: `check_component_update()` in `src-tauri/src/commands/update.rs`
 * Argument: `name` - component name (e.g., "gamdl", "meedyadl", "python")
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

/**
 * Downloads and installs a MeedyaDL app update from a specific GitHub release tag.
 *
 * Rust handler: `download_and_install_app_update()` in `src-tauri/src/commands/updates.rs`
 * Argument: `tag` - Git tag of the release to install (e.g., "v0.3.7")
 * Returns: string success message
 *
 * Uses the Tauri updater plugin to download a signed update binary from
 * GitHub Releases. The updater verifies the cryptographic signature before
 * applying the update. During download, emits `app-update-progress` events
 * with chunk progress data that the frontend can use for a progress bar.
 *
 * After successful installation, the user should restart the app using
 * `relaunch()` from `@tauri-apps/plugin-process` to load the new version.
 *
 * Called by: updateStore.downloadAndInstallAppUpdate()
 *
 * @param tag - Git tag of the release (e.g., "v0.3.7")
 * @returns Promise resolving to a success message string
 */
export function downloadAndInstallAppUpdate(tag: string): Promise<string> {
  return invoke<string>('download_and_install_app_update', { tag });
}

// ============================================================
// Cookie Management Commands
// ============================================================

/**
 * Detects which browsers are installed on the user's system.
 *
 * Rust handler: `detect_browsers()` in `src-tauri/src/commands/cookies.rs`
 * Returns: `DetectedBrowser[]` with id, name, icon hint, and FDA requirement
 *
 * Performs lightweight filesystem checks to find browser profile directories.
 * No cookies are read during detection, and no OS permission prompts appear.
 *
 * Called by: CookiesStep (SetupWizard), CookiesTab (SettingsPage)
 *
 * @returns Promise resolving to an array of detected browser entries
 */
export function detectBrowsers(): Promise<DetectedBrowser[]> {
  return invoke<DetectedBrowser[]>('detect_browsers');
}

/**
 * Imports Apple Music cookies from the specified browser.
 *
 * Rust handler: `import_cookies_from_browser()` in `src-tauri/src/commands/cookies.rs`
 * Argument: `browserId` - machine-readable browser ID (e.g., "chrome", "firefox")
 * Returns: `CookieImportResult` with success status, cookie counts, and file path
 *
 * The Rust backend extracts cookies matching Apple Music domains, converts them
 * to Netscape format, writes to `{app_data}/cookies.txt`, and updates settings.
 *
 * Platform notes:
 * - macOS (Chromium): May trigger a Keychain access prompt
 * - macOS (Safari): Requires Full Disk Access (check first with checkFullDiskAccess)
 * - Windows: Uses DPAPI for transparent decryption
 * - Linux: Uses D-Bus Secret Service for key decryption
 *
 * Called by: CookiesStep (SetupWizard), CookiesTab (SettingsPage)
 *
 * @param browserId - Machine-readable browser identifier
 * @returns Promise resolving to the import result
 */
export function importCookiesFromBrowser(browserId: string): Promise<CookieImportResult> {
  return invoke<CookieImportResult>('import_cookies_from_browser', {
    browserId,
  });
}

/**
 * Checks whether the application has macOS Full Disk Access.
 *
 * Rust handler: `check_full_disk_access()` in `src-tauri/src/commands/cookies.rs`
 * Returns: boolean (true = FDA granted or not required on this platform)
 *
 * Safari on macOS stores cookies in a TCC-protected location. Without FDA,
 * the app cannot read Safari's cookie database. The frontend calls this
 * before attempting a Safari import and shows instructions if FDA is needed.
 *
 * On non-macOS platforms, always returns true.
 *
 * Called by: CookiesStep (SetupWizard) when user selects Safari
 *
 * @returns Promise resolving to whether FDA is granted
 */
export function checkFullDiskAccess(): Promise<boolean> {
  return invoke<boolean>('check_full_disk_access');
}

// ============================================================
// Login Window Commands
// ============================================================

/**
 * Opens an embedded browser window for Apple Music login.
 *
 * Rust handler: `open_apple_login()` in `src-tauri/src/commands/login_window.rs`
 *
 * Creates a secondary webview window that loads https://music.apple.com.
 * The user can sign in with their Apple ID. The window automatically detects
 * when authentication cookies are set and emits a `login-cookies-extracted`
 * event to the main window.
 *
 * If a login window is already open, the existing window is focused instead.
 *
 * Called by: CookiesStep (SetupWizard), CookiesTab (SettingsPage)
 *
 * @returns Promise resolving when the window is opened
 */
export function openAppleLogin(): Promise<void> {
  return invoke<void>('open_apple_login');
}

/**
 * Manually triggers cookie extraction from the login browser window.
 *
 * Rust handler: `extract_login_cookies()` in `src-tauri/src/commands/login_window.rs`
 * Returns: `CookieImportResult` with success status, cookie counts, and file path
 *
 * This is the manual fallback for when auto-detection doesn't fire. The user
 * clicks "I've signed in" in the frontend, which calls this command to force
 * cookie extraction from the login webview.
 *
 * Called by: CookiesStep (SetupWizard), CookiesTab (SettingsPage)
 *
 * @returns Promise resolving to the cookie import result
 */
export function extractLoginCookies(): Promise<CookieImportResult> {
  return invoke<CookieImportResult>('extract_login_cookies');
}

/**
 * Closes the Apple Music login browser window.
 *
 * Rust handler: `close_apple_login()` in `src-tauri/src/commands/login_window.rs`
 *
 * Called when the user clicks "Cancel" to dismiss the login window.
 * If the window is not open, this is a no-op.
 *
 * Called by: CookiesStep (SetupWizard), CookiesTab (SettingsPage)
 *
 * @returns Promise resolving when the window is closed
 */
export function closeAppleLogin(): Promise<void> {
  return invoke<void>('close_apple_login');
}

// ============================================================
// Animated Artwork Commands
// ============================================================

/**
 * Manually triggers animated artwork download for an album.
 *
 * Rust handler: `download_animated_artwork()` in `src-tauri/src/commands/artwork.rs`
 * Arguments: `urls` - Apple Music URL(s), `outputDir` - album output directory
 * Returns: `ArtworkResult` indicating which artwork types were downloaded
 *
 * Queries the Apple Music catalog API for animated cover art (motion artwork)
 * and downloads them via FFmpeg HLS-to-MP4 conversion. Requires MusicKit
 * credentials to be configured in settings.
 *
 * Called by: DownloadQueue "Download Artwork" button (future), CoverArtTab
 *
 * @param urls - Apple Music URL(s) for the album
 * @param outputDir - The album directory where artwork files should be saved
 * @returns Promise resolving to the artwork download result
 */
export function downloadAnimatedArtwork(urls: string[], outputDir: string): Promise<ArtworkResult> {
  return invoke<ArtworkResult>('download_animated_artwork', { urls, outputDir });
}

// ============================================================
// Crash Report Commands
// ============================================================
//
// These commands interact with the crash report service to list,
// delete, export, and report saved crash reports. Crash reports
// are JSON files stored in `{app_data_dir}/crashes/` by the Rust
// panic handler and the frontend error persistence system.
//
// See: src-tauri/src/commands/crash_reports.rs

/**
 * Returns all crash reports, sorted by timestamp (newest first).
 *
 * Rust handler: `list_crash_reports()` in `src-tauri/src/commands/crash_reports.rs`
 * Returns: `CrashReport[]`
 *
 * Reads all `crash-*.json` files from the crashes directory and
 * deserializes them into `CrashReport` objects. Files that fail
 * to parse are silently skipped.
 *
 * Called by: CrashReportSection in Settings > Advanced
 *
 * @returns Promise resolving to an array of crash reports (newest first)
 */
export function listCrashReports(): Promise<import('@/types').CrashReport[]> {
  return invoke<import('@/types').CrashReport[]>('list_crash_reports');
}

/**
 * Deletes a crash report by its ID.
 *
 * Rust handler: `delete_crash_report()` in `src-tauri/src/commands/crash_reports.rs`
 *
 * Removes the JSON file from the crashes directory. Idempotent:
 * deleting a non-existent report is not an error.
 *
 * Called by: CrashReportSection delete button
 *
 * @param id - The crash report UUID to delete
 * @returns Promise resolving when the report is deleted
 */
export function deleteCrashReport(id: string): Promise<void> {
  return invoke<void>('delete_crash_report', { id });
}

/**
 * Deletes all crash reports from disk.
 *
 * Rust handler: `delete_all_crash_reports()` in `src-tauri/src/commands/crash_reports.rs`
 * Returns: The number of files deleted
 *
 * Removes every `.json` file in the crashes directory without parsing.
 * Used by the "Clear All" button in the CrashReportSection.
 *
 * Called by: CrashReportSection "Clear All" button
 *
 * @returns Promise resolving to the number of deleted reports
 */
export function deleteAllCrashReports(): Promise<number> {
  return invoke<number>('delete_all_crash_reports');
}

/**
 * Exports a crash report as a formatted Markdown string.
 *
 * Rust handler: `export_crash_report()` in `src-tauri/src/commands/crash_reports.rs`
 * Returns: Markdown string with headers, code blocks, and metadata sections
 *
 * The returned string is formatted for pasting into a GitHub issue
 * or sharing with a developer. Used by the CrashReportDialog to
 * show the user a preview of what will be reported.
 *
 * Called by: CrashReportDialog preview display
 *
 * @param id - The crash report UUID to export
 * @returns Promise resolving to a formatted Markdown string
 */
export function exportCrashReport(id: string): Promise<string> {
  return invoke<string>('export_crash_report', { id });
}

/**
 * Builds and returns a pre-filled GitHub new-issue URL for a crash report.
 *
 * Rust handler: `get_github_issue_url()` in `src-tauri/src/commands/crash_reports.rs`
 * Returns: Full URL string for `github.com/MWBMPartners/MeedyaDL/issues/new`
 *   with query parameters: `title`, `body` (Markdown), and `labels` (bug, crash-report)
 *
 * The body is truncated if it would exceed browser URL length limits
 * (~3500 chars raw). Backtrace is truncated to first 15 + last 5 lines
 * with a "[truncated]" marker when needed.
 *
 * Called by: CrashReportDialog "Open GitHub Issue" button
 *
 * @param id - The crash report UUID to build the GitHub issue URL for
 * @returns Promise resolving to a pre-filled GitHub new-issue URL
 */
export function getGitHubIssueUrl(id: string): Promise<string> {
  return invoke<string>('get_github_issue_url', { id });
}

// ============================================================
// Download History Commands
// ============================================================
//
// Commands for viewing, searching, and clearing the persistent
// download history stored in `{app_data_dir}/history.json`.
//
// See: src-tauri/src/commands/history.rs

/**
 * Returns all download history entries, sorted newest first.
 *
 * Rust handler: `list_history()` in `src-tauri/src/commands/history.rs`
 * Returns: `HistoryEntry[]`
 *
 * Called by: HistoryPage component
 *
 * @returns Promise resolving to an array of history entries (newest first)
 */
export function listHistory(): Promise<import('@/types').HistoryEntry[]> {
  return invoke<import('@/types').HistoryEntry[]>('list_history');
}

/**
 * Deletes all download history entries from disk.
 *
 * Rust handler: `clear_history()` in `src-tauri/src/commands/history.rs`
 *
 * Called by: HistoryPage "Clear History" button
 *
 * @returns Promise resolving when history is cleared
 */
export function clearHistory(): Promise<void> {
  return invoke<void>('clear_history');
}

/**
 * Searches history entries by case-insensitive substring match
 * on title, artist, album, and URL fields.
 *
 * Rust handler: `search_history()` in `src-tauri/src/commands/history.rs`
 *
 * Called by: HistoryPage search input
 *
 * @param query - Search string to match against history entries
 * @returns Promise resolving to matching history entries (newest first)
 */
export function searchHistory(query: string): Promise<import('@/types').HistoryEntry[]> {
  return invoke<import('@/types').HistoryEntry[]>('search_history', { query });
}

/**
 * Removes a single history entry by ID (#685).
 *
 * Rust handler: `delete_history_entry()` in `src-tauri/src/commands/history.rs`
 *
 * The Rust side rejects unknown IDs with an error so the caller can
 * distinguish "already gone" from a successful delete.
 *
 * @param id - UUID of the history entry to remove.
 */
export function deleteHistoryEntry(id: string): Promise<void> {
  return invoke<void>('delete_history_entry', { id });
}

// ============================================================
// API Field Audit Commands
// ============================================================
//
// Diagnostic tool that fetches an Apple Music album and compares
// the raw API response fields against the known tag definitions
// in tags.toml. Reports known/unknown/missing fields.
//
// See: src-tauri/src/commands/api_audit.rs

/**
 * Audit an Apple Music album's API fields against the tag registry.
 *
 * Rust handler: `audit_api_fields()` in `src-tauri/src/commands/api_audit.rs`
 * Returns: `ApiAuditResult` with known/unknown/missing field breakdowns
 *
 * Requires MusicKit credentials to be configured.
 *
 * @param albumUrl - Apple Music album URL to audit
 * @returns Promise resolving to the audit result
 */
export function auditApiFields(albumUrl: string): Promise<import('@/types').ApiAuditResult> {
  return invoke<import('@/types').ApiAuditResult>('audit_api_fields', { albumUrl });
}

// ================================================================
// Clipboard Monitoring
// ================================================================

/**
 * Read the current text content from the system clipboard.
 *
 * Returns the clipboard text if present, or `null` if the clipboard is
 * empty or contains non-text data. Used by the clipboard monitoring hook
 * to detect supported URLs copied by the user.
 *
 * IPC target: `read_clipboard` → `commands::clipboard::read_clipboard`
 */
export function readClipboard(): Promise<string | null> {
  return invoke<string | null>('read_clipboard');
}

/**
 * Saves trimmed activity log entries to a session log file for debugging.
 *
 * Called when the activity store trims entries beyond MAX_ENTRIES.
 * Entries are appended to {app_data_dir}/logs/session-{date}.log.
 *
 * IPC target: `save_session_log` → `commands::system::save_session_log`
 */
export function saveSessionLog(entries: string[]): Promise<void> {
  return invoke<void>('save_session_log', { entries });
}

/**
 * Backend-half of the desktop-notification diagnostics readout (#834).
 *
 * The Settings → Advanced → Diagnostics panel reads this snapshot to show
 * the user their current notification configuration — useful on macOS
 * where Tauri's `sendNotification()` can resolve successfully even when
 * the OS silently drops the notification.
 *
 * The frontend tops the result up with the JS-side
 * `isPermissionGranted()` value to give a complete picture.
 *
 * IPC target: `get_notification_diagnostics` → `commands::system::get_notification_diagnostics`
 */
export interface NotificationDiagnostics {
  desktop_notifications_enabled: boolean;
  notification_style: string;
  platform: string;
}

export function getNotificationDiagnostics(): Promise<NotificationDiagnostics> {
  return invoke<NotificationDiagnostics>('get_notification_diagnostics');
}

/**
 * Sends a one-off test notification through the **backend** notification
 * pipeline (#834).
 *
 * The pre-#834 "Send Test Notification" button in Settings → General
 * called the JS plugin directly. That tested a different code path than
 * what production downloads use AND couldn't surface OS-level failures
 * because `sendNotification()` resolves successfully even when macOS
 * drops the notification.
 *
 * This wrapper routes the test through the same Rust path as
 * `send_desktop_notification`, but bypasses the focus check and the
 * throttle — the user is explicitly testing, so silently no-opping
 * would defeat the purpose. Returns the actual OS-level error string
 * when the plugin call fails, which the caller renders into a toast so
 * the user can act on it (e.g. open System Settings → Notifications).
 *
 * IPC target: `test_desktop_notification` → `commands::system::test_desktop_notification`
 */
export function testDesktopNotification(): Promise<void> {
  return invoke<void>('test_desktop_notification');
}

/**
 * Single integrity-scan finding (#537 chunk B).
 *
 * Discriminated by `kind`:
 * - `degenerate_name` — filename/path matches one of the empty-tag
 *   MV pipeline signatures. `signature` says which one
 *   (`hyphen-dot-extension`, `unknown-folder-segment`,
 *   `unknown-album-with-empty-filename`).
 * - `zero_byte_cover` — fixed-name cover file (`FrontCover.mp4`,
 *   `PortraitCover.mp4`, `ArtistSpotlightCover.mp4`) is 0 bytes,
 *   likely from an interrupted HLS download.
 */
export type IntegrityIssue =
  | { kind: 'degenerate_name'; path: string; signature: string }
  | { kind: 'zero_byte_cover'; path: string };

export interface IntegrityScanReport {
  files_walked: number;
  issues: IntegrityIssue[];
  scanned_path: string;
}

/**
 * Walks the user's configured output directory and returns a report
 * of historic damage from pre-v1.6 broken builds (#537 chunk B).
 *
 * Detects: `-.mp4`/`-.jpg` empty-tag filenames, `[Unknown]/` folder
 * segments, zero-byte fixed-name covers. Read-only — does NOT
 * modify or remove anything. Quarantine action lands as a
 * follow-up; for now the report is purely informational.
 *
 * The Rust side runs the walk on a `spawn_blocking` thread so the
 * frontend can `await` this freely even on libraries with 1000+
 * albums (where the scan can take several seconds).
 */
export function runIntegrityScan(): Promise<IntegrityScanReport> {
  return invoke<IntegrityScanReport>('run_integrity_scan');
}
