/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/types/index.ts - Central TypeScript type definitions
 *
 * This file defines all TypeScript interfaces, type aliases, and constants
 * that are shared across the gamdl-GUI frontend. Every type here mirrors
 * (or is derived from) a corresponding Rust struct/enum in the Tauri backend,
 * ensuring type safety across the IPC serialization boundary.
 *
 * When the Rust backend serializes data via `serde_json`, Tauri's invoke()
 * returns it as a plain JavaScript object. These TypeScript types tell the
 * compiler what shape that object will have, enabling compile-time type
 * checking on the frontend side.
 *
 * Naming conventions:
 * - TypeScript types use PascalCase (matching Rust struct names)
 * - Rust field names use snake_case, and so do these interfaces (Tauri
 *   preserves the Rust field names in serialized JSON by default)
 * - Discriminated union types (like GamdlOutputEvent) mirror Rust enums
 *   serialized with serde's `#[serde(tag = "type")]` attribute
 *
 * @see {@link https://www.typescriptlang.org/docs/handbook/2/objects.html} - TS object types
 * @see {@link https://www.typescriptlang.org/docs/handbook/2/narrowing.html#discriminated-unions} - Discriminated unions
 * @see {@link https://v2.tauri.app/develop/calling-rust/#accessing-the-webviewwindow-in-commands} - Tauri IPC serialization
 */

// ============================================================
// Audio/Video Quality Types
// ============================================================

/**
 * All audio codec options supported by GAMDL's `--song-codec` CLI flag.
 *
 * Mirrors: Rust enum `SongCodec` in `src-tauri/src/models/settings.rs`
 *
 * Each variant maps to a specific Apple Music audio stream format:
 * - `alac`: Apple Lossless Audio Codec (16/24-bit, up to 192kHz)
 * - `atmos`: Dolby Atmos spatial audio (requires compatible decoder)
 * - `ac3`: Dolby Digital surround sound
 * - `aac-binaural`: AAC 256kbps with binaural spatial rendering
 * - `aac`: Standard AAC 256kbps (up to 48kHz sample rate)
 * - `aac-legacy`: AAC 256kbps capped at 44.1kHz for older device compatibility
 * - `aac-he-legacy`: High Efficiency AAC at 64kbps (smaller files)
 * - `aac-he`: HE-AAC experimental variant
 * - `aac-downmix`: AAC with surround-to-stereo downmix (experimental)
 * - `aac-he-binaural`: HE-AAC with binaural rendering (experimental)
 * - `aac-he-downmix`: HE-AAC with downmix (experimental)
 *
 * @see {@link https://www.typescriptlang.org/docs/handbook/2/everyday-types.html#literal-types} - String literal types
 */
export type SongCodec =
  | 'alac'
  | 'atmos'
  | 'ac3'
  | 'aac-binaural'
  | 'aac'
  | 'aac-legacy'
  | 'aac-he-legacy'
  | 'aac-he'
  | 'aac-downmix'
  | 'aac-he-binaural'
  | 'aac-he-downmix';

/**
 * Video resolution options for GAMDL's `--music-video-resolution` CLI flag.
 *
 * Mirrors: Rust enum `VideoResolution` in `src-tauri/src/models/settings.rs`
 *
 * These represent the maximum resolution GAMDL will attempt to download
 * for music videos. If the requested resolution is unavailable, GAMDL
 * falls back to the next lower available resolution.
 *
 * @see {@link https://www.typescriptlang.org/docs/handbook/2/everyday-types.html#literal-types}
 */
export type VideoResolution =
  | '2160p'
  | '1440p'
  | '1080p'
  | '720p'
  | '540p'
  | '480p'
  | '360p'
  | '240p';

/**
 * Synced lyrics output format options for GAMDL's `--synced-lyrics-format` flag.
 *
 * Mirrors: Rust enum `LyricsFormat` in `src-tauri/src/models/settings.rs`
 *
 * - `lrc`: LRC format (widely supported by music players)
 * - `srt`: SubRip subtitle format (used for video subtitles)
 * - `ttml`: Timed Text Markup Language (Apple's native lyrics format)
 */
export type LyricsFormat = 'lrc' | 'srt' | 'ttml';

/**
 * Cover art image format options for GAMDL's `--cover-format` flag.
 *
 * Mirrors: Rust enum `CoverFormat` in `src-tauri/src/models/settings.rs`
 *
 * - `jpg`: JPEG (lossy, smaller files)
 * - `png`: PNG (lossless, larger files)
 * - `raw`: Original format from Apple's servers (usually JPEG)
 */
export type CoverFormat = 'jpg' | 'png' | 'raw';

/**
 * Download mode: selects which tool GAMDL uses for fetching HLS streams.
 *
 * Mirrors: Rust enum `DownloadMode` in `src-tauri/src/models/settings.rs`
 *
 * - `ytdlp`: yt-dlp (popular, widely available)
 * - `nm3u8dlre`: N_m3u8DL-RE (specialized HLS downloader, often faster)
 */
export type DownloadMode = 'ytdlp' | 'nm3u8dlre';

/**
 * Remux mode: selects which tool GAMDL uses for container conversion.
 *
 * Mirrors: Rust enum `RemuxMode` in `src-tauri/src/models/settings.rs`
 *
 * - `ffmpeg`: FFmpeg (versatile, handles all formats)
 * - `mp4box`: MP4Box from GPAC (specialized MP4 muxer, can be faster)
 */
export type RemuxMode = 'ffmpeg' | 'mp4box';

/**
 * Log level for GAMDL's `--log-level` CLI flag.
 *
 * Mirrors: Rust enum `LogLevel` in `src-tauri/src/models/settings.rs`
 *
 * Controls the verbosity of GAMDL's stdout/stderr output,
 * which is parsed by the Rust backend into GamdlOutputEvent types.
 */
export type LogLevel = 'DEBUG' | 'INFO' | 'WARNING' | 'ERROR';

// ============================================================
// Human-readable labels for codec/quality selectors
// ============================================================

/**
 * Display names for audio codecs, shown in UI dropdown selectors.
 *
 * Uses `Record<SongCodec, string>` to ensure every SongCodec variant
 * has a corresponding label -- TypeScript will error if a variant is missing.
 * This pattern provides exhaustiveness checking at compile time.
 *
 * Used by: SettingsPage codec selector, DownloadForm codec override dropdown
 *
 * @see {@link https://www.typescriptlang.org/docs/handbook/utility-types.html#recordkeys-type} - Record utility type
 */
export const SONG_CODEC_LABELS: Record<SongCodec, string> = {
  alac: 'Lossless (ALAC)',
  atmos: 'Dolby Atmos',
  ac3: 'Dolby Digital (AC3)',
  'aac-binaural': 'AAC (256kbps) Binaural',
  aac: 'AAC (256kbps at up to 48kHz)',
  'aac-legacy': 'AAC Legacy (256kbps at up to 44.1kHz)',
  'aac-he-legacy': 'AAC-HE Legacy (64kbps)',
  'aac-he': 'AAC-HE (Experimental)',
  'aac-downmix': 'AAC Downmix (Experimental)',
  'aac-he-binaural': 'AAC-HE Binaural (Experimental)',
  'aac-he-downmix': 'AAC-HE Downmix (Experimental)',
};

/**
 * Display names for video resolutions, shown in UI dropdown selectors.
 *
 * Uses `Record<VideoResolution, string>` for compile-time exhaustiveness.
 * Labels include common marketing names (4K, QHD, Full HD, etc.) for
 * user-friendly display alongside the technical resolution.
 *
 * Used by: SettingsPage resolution selector, DownloadForm resolution override
 *
 * @see {@link https://www.typescriptlang.org/docs/handbook/utility-types.html#recordkeys-type}
 */
export const VIDEO_RESOLUTION_LABELS: Record<VideoResolution, string> = {
  '2160p': '4K (2160p)',
  '1440p': 'QHD (1440p)',
  '1080p': 'Full HD (1080p)',
  '720p': 'HD (720p)',
  '540p': 'qHD (540p)',
  '480p': 'SD (480p)',
  '360p': 'Low (360p)',
  '240p': 'Lowest (240p)',
};

// ============================================================
// GAMDL Options (maps to every CLI flag)
// ============================================================

/**
 * Complete set of GAMDL CLI options for a single download request.
 *
 * Mirrors: Rust struct `GamdlOptions` in `src-tauri/src/models/download.rs`
 *
 * All fields are optional (`?`) because this interface is used for
 * per-download overrides. When a download is started, these options are
 * merged with the global AppSettings defaults. Only the overridden fields
 * need to be specified; missing fields fall back to the saved settings.
 *
 * Each field corresponds to a GAMDL CLI flag. The Rust backend converts
 * these into command-line arguments when spawning the GAMDL subprocess.
 *
 * @see {@link https://www.typescriptlang.org/docs/handbook/2/objects.html#optional-properties} - Optional properties
 */
export interface GamdlOptions {
  /** Audio codec to use (--song-codec flag) */
  song_codec?: SongCodec;
  /** Codec priority string for music videos (e.g., "h265,h264") */
  music_video_codec_priority?: string;
  /** Maximum video resolution (--music-video-resolution flag) */
  music_video_resolution?: VideoResolution;
  /** Container format for remuxed music videos (e.g., "mp4", "mkv") */
  music_video_remux_format?: string;
  /** Quality setting for user-uploaded videos */
  uploaded_video_quality?: string;
  /** When true, does not skip music videos in album downloads */
  disable_music_video_skip?: boolean;
  /** Format for synced lyrics files (--synced-lyrics-format flag) */
  synced_lyrics_format?: LyricsFormat;
  /** When true, skips downloading synced lyrics */
  no_synced_lyrics?: boolean;
  /** When true, downloads only synced lyrics (no audio) */
  synced_lyrics_only?: boolean;
  /** When true, saves album cover art as a separate file */
  save_cover?: boolean;
  /** Image format for saved cover art */
  cover_format?: CoverFormat;
  /** Pixel dimensions for cover art (e.g., 1200 for 1200x1200) */
  cover_size?: number;
  /** Output directory for downloaded files */
  output_path?: string;
  /** Temporary directory for intermediate files during processing */
  temp_path?: string;
  /** When true, overwrites existing files without prompting */
  overwrite?: boolean;
  /** Maximum filename length before truncation (0 = no truncation) */
  truncate?: number;
  /** Path to a Netscape-format cookies file for authentication */
  cookies_path?: string;
  /** When true, uses the Apple Music API wrapper instead of cookies */
  use_wrapper?: boolean;
  /** URL of the Apple Music API wrapper account endpoint */
  wrapper_account_url?: string;
  /** IP address hint for the wrapper's decryption service */
  wrapper_decrypt_ip?: string;
  /** Language/locale for metadata (e.g., "en-US", "ja-JP") */
  language?: string;
  /** Comma-separated list of metadata tags to exclude from output */
  exclude_tags?: string;
  /** When true, uses album release date instead of track date */
  use_album_date?: boolean;
  /** When true, fetches additional metadata tags from the API */
  fetch_extra_tags?: boolean;
  /** Template string for the date tag format */
  date_tag_template?: string;
  /** Template for album folder names (supports {artist}, {album}, etc.) */
  album_folder_template?: string;
  /** Template for compilation album folder names */
  compilation_folder_template?: string;
  /** Template for folder names when album folder is disabled */
  no_album_folder_template?: string;
  /** Template for file names on single-disc albums */
  single_disc_file_template?: string;
  /** Template for file names on multi-disc albums (includes disc number) */
  multi_disc_file_template?: string;
  /** Template for file names when album folder is disabled */
  no_album_file_template?: string;
  /** Template for file names in playlist downloads */
  playlist_file_template?: string;
  /** Custom path to the FFmpeg binary */
  ffmpeg_path?: string;
  /** Custom path to the mp4decrypt binary (for DRM decryption) */
  mp4decrypt_path?: string;
  /** Custom path to the MP4Box binary (for remuxing) */
  mp4box_path?: string;
  /** Custom path to the N_m3u8DL-RE binary (for HLS downloads) */
  nm3u8dlre_path?: string;
  /** Custom path to the amdecrypt binary (for Apple Music DRM) */
  amdecrypt_path?: string;
  /** Custom path to the Widevine Device file (.wvd) */
  wvd_path?: string;
  /** Which download tool to use (yt-dlp or N_m3u8DL-RE) */
  download_mode?: DownloadMode;
  /** Which remux tool to use (FFmpeg or MP4Box) */
  remux_mode?: RemuxMode;
  /** Verbosity level for GAMDL's output */
  log_level?: LogLevel;
  /** When true, continues downloading even if some tracks fail */
  no_exceptions?: boolean;
  /** When true, saves the playlist metadata alongside downloads */
  save_playlist?: boolean;
  /** When true, reads URLs from a text file instead of CLI arguments */
  read_urls_as_txt?: boolean;
  /** When true, ignores the GAMDL config file (uses only CLI args) */
  no_config_file?: boolean;
}

// ============================================================
// Application Settings
// ============================================================

/**
 * Complete application settings, persisted as JSON on disk.
 *
 * Mirrors: Rust struct `AppSettings` in `src-tauri/src/models/settings.rs`
 *
 * This interface represents the full, flattened settings object stored
 * in the app's data directory (e.g., `~/Library/Application Support/gamdl-gui/settings.json`).
 * Unlike `GamdlOptions` (where all fields are optional), all fields here
 * are required because the Rust backend provides defaults for every setting.
 *
 * Settings flow:
 * 1. On startup, the Rust backend loads settings from disk (or creates defaults)
 * 2. The frontend fetches them via `getSettings()` IPC call
 * 3. User edits are saved via `saveSettings()` IPC call
 * 4. The Rust backend also syncs relevant settings to GAMDL's own config file
 *
 * Nullable fields (`string | null`) indicate settings that have no default
 * and are truly optional (e.g., custom binary paths, cookies file).
 *
 * @see {@link https://www.typescriptlang.org/docs/handbook/2/objects.html} - TS object types
 */
export interface AppSettings {
  /** Default output directory for downloaded files */
  output_path: string;
  /** Language/locale code for metadata (e.g., "en-US") */
  language: string;
  /** Whether to overwrite existing files by default */
  overwrite: boolean;
  /** Whether to automatically check for updates on app startup */
  auto_check_updates: boolean;
  /** Default audio codec for song downloads */
  default_song_codec: SongCodec;
  /** Default maximum video resolution */
  default_video_resolution: VideoResolution;
  /** Default codec priority string for music videos */
  default_video_codec_priority: string;
  /** Default container format for remuxed music videos */
  default_video_remux_format: string;
  /** Whether fallback codec/resolution chains are enabled */
  fallback_enabled: boolean;
  /** Ordered list of codecs to try if the primary codec is unavailable */
  music_fallback_chain: SongCodec[];
  /** Ordered list of resolutions to try if the primary resolution is unavailable */
  video_fallback_chain: VideoResolution[];
  /** Default format for synced lyrics output */
  synced_lyrics_format: LyricsFormat;
  /** Whether to skip synced lyrics by default */
  no_synced_lyrics: boolean;
  /** Whether to download only synced lyrics (no audio) */
  synced_lyrics_only: boolean;
  /** Whether to save album cover art as separate files */
  save_cover: boolean;
  /** Default image format for saved cover art */
  cover_format: CoverFormat;
  /** Default pixel dimensions for cover art */
  cover_size: number;
  /** Template for album folder naming */
  album_folder_template: string;
  /** Template for compilation album folder naming */
  compilation_folder_template: string;
  /** Template for folder naming when album folders are disabled */
  no_album_folder_template: string;
  /** Template for file naming on single-disc albums */
  single_disc_file_template: string;
  /** Template for file naming on multi-disc albums */
  multi_disc_file_template: string;
  /** Template for file naming when album folders are disabled */
  no_album_file_template: string;
  /** Template for file naming in playlist downloads */
  playlist_file_template: string;
  /** Path to Netscape-format cookies file, or null if not set */
  cookies_path: string | null;
  /** Custom FFmpeg binary path, or null to use bundled/PATH version */
  ffmpeg_path: string | null;
  /** Custom mp4decrypt binary path, or null to use bundled/PATH version */
  mp4decrypt_path: string | null;
  /** Custom MP4Box binary path, or null to use bundled/PATH version */
  mp4box_path: string | null;
  /** Custom N_m3u8DL-RE binary path, or null to use bundled/PATH version */
  nm3u8dlre_path: string | null;
  /** Custom amdecrypt binary path, or null to use bundled/PATH version */
  amdecrypt_path: string | null;
  /** Which download tool to use by default */
  download_mode: DownloadMode;
  /** Which remux tool to use by default */
  remux_mode: RemuxMode;
  /** Whether to use the Apple Music API wrapper */
  use_wrapper: boolean;
  /** URL for the API wrapper account endpoint */
  wrapper_account_url: string;
  /** Maximum filename length, or null for no truncation */
  truncate: number | null;
  /** List of metadata tags to exclude from output files */
  exclude_tags: string[];
  /** Whether the sidebar is in collapsed (icon-only) mode */
  sidebar_collapsed: boolean;
  /** CSS theme override string, or null for auto-detection */
  theme_override: string | null;
}

// ============================================================
// Download Types
// ============================================================

/**
 * A download request submitted from the UI to the Rust backend.
 *
 * Mirrors: Rust struct `DownloadRequest` in `src-tauri/src/models/download.rs`
 *
 * Sent via the `start_download` IPC command. The backend creates a new
 * queue item, merges the optional overrides with saved settings, and
 * starts the GAMDL subprocess.
 *
 * @see src/lib/tauri-commands.ts - startDownload() wrapper
 */
export interface DownloadRequest {
  /** One or more Apple Music URLs to download */
  urls: string[];
  /** Optional per-download overrides (merged with global settings) */
  options?: GamdlOptions;
}

/**
 * Possible states of a download queue item (state machine).
 *
 * Mirrors: Rust enum `DownloadState` in `src-tauri/src/models/download.rs`
 *
 * State transitions:
 * - queued -> downloading (when the GAMDL subprocess starts)
 * - downloading -> processing (when download completes, post-processing begins)
 * - downloading -> error (on GAMDL failure or crash)
 * - downloading -> cancelled (on user cancellation)
 * - processing -> complete (when all post-processing finishes)
 * - processing -> error (on post-processing failure)
 * - error -> queued (on retry)
 * - cancelled -> queued (on retry)
 */
export type DownloadState =
  | 'queued'
  | 'downloading'
  | 'processing'
  | 'complete'
  | 'error'
  | 'cancelled';

/**
 * Detailed status of a single download queue item.
 *
 * Mirrors: Rust struct `QueueItemStatus` in `src-tauri/src/models/download.rs`
 *
 * This is the primary data structure for rendering queue items in the
 * DownloadQueue component. Updated in real-time via Tauri events.
 */
export interface QueueItemStatus {
  /** Unique identifier for this download (UUID v4) */
  id: string;
  /** The Apple Music URL(s) being downloaded */
  urls: string[];
  /** Current state in the download lifecycle */
  state: DownloadState;
  /** Download progress as a percentage (0-100) */
  progress: number;
  /** Name of the track currently being downloaded, or null if not started */
  current_track: string | null;
  /** Total number of tracks in this download, or null if unknown */
  total_tracks: number | null;
  /** Number of tracks completed so far, or null if not applicable */
  completed_tracks: number | null;
  /** Current download speed string (e.g., "2.5 MB/s"), or null */
  speed: string | null;
  /** Estimated time remaining string (e.g., "00:45"), or null */
  eta: string | null;
  /** Error message if state is 'error', otherwise null */
  error: string | null;
  /** Output directory where files were saved, or null if not complete */
  output_path: string | null;
  /** Actual codec used (may differ from requested if fallback occurred) */
  codec_used: string | null;
  /** Whether a codec/resolution fallback was used for this download */
  fallback_occurred: boolean;
  /** ISO 8601 timestamp when this download was queued */
  created_at: string;
}

/**
 * Status of the entire download queue (aggregate view).
 *
 * Mirrors: Rust struct `QueueStatus` in `src-tauri/src/models/download.rs`
 *
 * Returned by the `get_queue_status` IPC command. Contains both
 * aggregate counts and the full list of individual queue items.
 * Used by the DownloadQueue component and the sidebar badge counter.
 */
export interface QueueStatus {
  /** Total number of items in the queue (all states) */
  total: number;
  /** Number of currently downloading/processing items */
  active: number;
  /** Number of items waiting to start */
  queued: number;
  /** Number of successfully completed items */
  completed: number;
  /** Number of failed items */
  failed: number;
  /** Full list of queue items with detailed status */
  items: QueueItemStatus[];
}

// ============================================================
// Dependency Types
// ============================================================

/**
 * Status of a single external dependency (Python, GAMDL, or tool).
 *
 * Mirrors: Rust struct `DependencyStatus` in `src-tauri/src/models/dependency.rs`
 *
 * Returned by dependency check IPC commands. Used by the setup wizard
 * to determine which dependencies need to be installed, and by the
 * settings page to show installation status.
 */
export interface DependencyStatus {
  /** Human-readable name of the dependency (e.g., "Python", "FFmpeg") */
  name: string;
  /** Whether this dependency is required for basic operation */
  required: boolean;
  /** Whether the dependency is currently installed and accessible */
  installed: boolean;
  /** Detected version string (e.g., "3.12.1"), or null if not installed */
  version: string | null;
  /** Filesystem path to the binary, or null if not installed */
  path: string | null;
}

// ============================================================
// System Types
// ============================================================

/**
 * Platform information returned by the Rust backend.
 *
 * Mirrors: Rust struct `PlatformInfo` in `src-tauri/src/models/system.rs`
 *
 * Used for platform detection, download URL selection (choosing the
 * correct binary architecture), and UI theme selection.
 */
export interface PlatformInfo {
  /** OS identifier: "macos", "windows", or "linux" */
  platform: string;
  /** CPU architecture: "x86_64", "aarch64", etc. */
  arch: string;
  /** OS type string from the Rust `std::env::consts::OS` */
  os_type: string;
}

/**
 * Result of validating a Netscape-format cookies file.
 *
 * Mirrors: Rust struct `CookieValidation` in `src-tauri/src/models/system.rs`
 *
 * Returned by the `validate_cookies_file` IPC command. The setup wizard
 * and settings page use this to show the user whether their cookies file
 * is valid and contains the necessary Apple Music authentication cookies.
 */
export interface CookieValidation {
  /** Whether the file is a valid Netscape cookies file */
  valid: boolean;
  /** Total number of cookies found in the file */
  cookie_count: number;
  /** List of unique domains found in the cookies */
  domains: string[];
  /** Number of cookies specifically for Apple Music domains */
  apple_music_cookies: number;
  /** Whether any cookies have expired */
  expired: boolean;
  /** Warning messages (e.g., "Some cookies are expired") */
  warnings: string[];
}

// ============================================================
// GAMDL Output Events (from subprocess parsing)
// ============================================================

/**
 * Structured event parsed from GAMDL's stdout/stderr output.
 *
 * Mirrors: Rust enum `GamdlOutputEvent` in `src-tauri/src/services/gamdl_parser.rs`
 *
 * This is a discriminated union type -- the `type` field acts as the
 * discriminant, allowing TypeScript to narrow the type in switch/if blocks.
 * The Rust backend parses raw GAMDL subprocess output lines and converts
 * them into these structured events via regex pattern matching.
 *
 * Variants:
 * - `track_info`: Emitted when GAMDL starts downloading a new track
 * - `download_progress`: Emitted periodically with progress percentage
 * - `processing_step`: Emitted during post-download processing (remux, tag, etc.)
 * - `error`: Emitted when GAMDL reports an error
 * - `complete`: Emitted when a track finishes successfully with output path
 * - `unknown`: Fallback for unparseable output lines (contains raw text)
 *
 * @see {@link https://www.typescriptlang.org/docs/handbook/2/narrowing.html#discriminated-unions} - Discriminated unions
 */
export type GamdlOutputEvent =
  | { type: 'track_info'; title: string; artist: string; album: string }
  | { type: 'download_progress'; percent: number; speed: string; eta: string }
  | { type: 'processing_step'; step: string }
  | { type: 'error'; message: string }
  | { type: 'complete'; path: string }
  | { type: 'unknown'; raw: string };

/**
 * Progress event payload emitted via the `gamdl-output` Tauri event.
 *
 * Mirrors: Rust struct `GamdlProgress` in `src-tauri/src/services/gamdl_service.rs`
 *
 * The Rust backend emits this via `app_handle.emit("gamdl-output", payload)`
 * whenever the GAMDL subprocess produces parseable output. The frontend
 * listens for these events in App.tsx and delegates to the download store.
 *
 * @see App.tsx Effect 5 for the event listener registration
 */
export interface GamdlProgress {
  /** Links this event to a specific queue item */
  download_id: string;
  /** The parsed event data (discriminated union) */
  event: GamdlOutputEvent;
}

// ============================================================
// URL Parser Types
// ============================================================

/**
 * Content types that can be detected from Apple Music URLs.
 *
 * Used by the URL parser (src/lib/url-parser.ts) and the DownloadForm
 * component to show content-type indicators and apply appropriate
 * default options (e.g., video options for music-video URLs).
 *
 * The 'unknown' variant is used when a URL cannot be classified,
 * either because it's not a valid Apple Music URL or the path
 * structure doesn't match any known pattern.
 */
export type AppleMusicContentType =
  | 'song'
  | 'album'
  | 'playlist'
  | 'music-video'
  | 'artist'
  | 'unknown';

/**
 * Result of parsing an Apple Music URL.
 *
 * Returned by `parseAppleMusicUrl()` in src/lib/url-parser.ts.
 * Used by the DownloadForm to validate URLs and display content-type
 * badges next to each URL in the input list.
 */
export interface ParsedUrl {
  /** The original URL string (trimmed of whitespace) */
  url: string;
  /** Detected content type based on URL path analysis */
  contentType: AppleMusicContentType;
  /** Whether the URL is a valid, classifiable Apple Music URL */
  isValid: boolean;
}

// ============================================================
// UI Types
// ============================================================

/**
 * Navigation pages in the sidebar.
 *
 * These string literals correspond to the page components rendered by
 * the `renderPage()` function in App.tsx. The uiStore's `currentPage`
 * field holds one of these values, driving the lightweight client-side router.
 *
 * @see App.tsx renderPage() for the page-to-component mapping
 */
export type AppPage = 'download' | 'queue' | 'settings' | 'help';

/**
 * Toast notification severity levels.
 *
 * Controls the visual style (color, icon) of toast notifications:
 * - `success`: green, checkmark icon
 * - `error`: red, X icon
 * - `warning`: amber, exclamation icon
 * - `info`: blue, info icon
 */
export type ToastType = 'success' | 'error' | 'warning' | 'info';

/**
 * Toast notification data structure.
 *
 * Managed by the uiStore's toast queue. Toasts auto-dismiss after
 * `duration` milliseconds (default varies by type).
 */
export interface Toast {
  /** Unique identifier for this toast (UUID, used as React key) */
  id: string;
  /** The message text displayed in the toast */
  message: string;
  /** Severity level controlling visual style */
  type: ToastType;
  /** Auto-dismiss duration in milliseconds (optional, has defaults per type) */
  duration?: number;
}

/**
 * Setup wizard step identifiers.
 *
 * The wizard progresses through these steps in order:
 * 1. `welcome` - Introduction and system requirements check
 * 2. `python` - Install or detect Python runtime
 * 3. `gamdl` - Install GAMDL via pip
 * 4. `dependencies` - Install external tools (FFmpeg, mp4decrypt, etc.)
 * 5. `cookies` - Configure Apple Music authentication cookies
 * 6. `complete` - Summary and launch into main application
 *
 * @see ./components/setup/SetupWizard.tsx for the step renderer
 */
export type SetupStep =
  | 'welcome'
  | 'python'
  | 'gamdl'
  | 'dependencies'
  | 'cookies'
  | 'complete';

// ============================================================
// Music Service Types (extensibility architecture)
// ============================================================

/**
 * Identifies which music service a download targets.
 *
 * Currently only Apple Music is fully implemented. YouTube Music and
 * Spotify are defined here for future extensibility -- the architecture
 * supports multiple service backends with different capabilities.
 *
 * Used by the DownloadForm to determine which URL patterns and options
 * are applicable for a given download.
 */
export type MusicServiceId = 'apple-music' | 'youtube-music' | 'spotify';

/**
 * Display labels for music services, shown in UI dropdowns and badges.
 *
 * Uses `Record<MusicServiceId, string>` for compile-time exhaustiveness
 * checking -- adding a new MusicServiceId variant will cause a TypeScript
 * error until a label is added here.
 *
 * @see {@link https://www.typescriptlang.org/docs/handbook/utility-types.html#recordkeys-type}
 */
export const MUSIC_SERVICE_LABELS: Record<MusicServiceId, string> = {
  'apple-music': 'Apple Music',
  'youtube-music': 'YouTube Music',
  spotify: 'Spotify',
};

/**
 * Capabilities that a music service may support.
 *
 * This interface enables the UI to adapt based on what a given service
 * supports. For example, if a service doesn't support lossless audio,
 * the codec selector can hide lossless options automatically.
 *
 * Designed for future multi-service support. Currently, Apple Music
 * is the only fully implemented service.
 */
export interface ServiceCapabilities {
  /** Whether the service offers lossless audio streams */
  supports_lossless: boolean;
  /** Whether the service offers spatial/Atmos audio */
  supports_spatial_audio: boolean;
  /** Whether the service hosts music videos */
  supports_music_videos: boolean;
  /** Whether the service provides synced lyrics */
  supports_lyrics: boolean;
  /** Whether the service provides album cover art */
  supports_cover_art: boolean;
  /** Whether the service requires cookie-based authentication */
  requires_cookies: boolean;
  /** Whether the service requires OAuth token authentication */
  requires_oauth: boolean;
  /** Content types supported by this service (e.g., ["song", "album"]) */
  supported_content_types: string[];
}

// ============================================================
// Update Types
// ============================================================

/**
 * Update status for a single application component.
 *
 * Mirrors: Rust struct `ComponentUpdate` in `src-tauri/src/models/update.rs`
 *
 * Returned as part of `UpdateCheckResult`. Each component (GAMDL, the GUI
 * app itself, Python runtime) has its own update status. The UpdateBanner
 * component and settings page use this to show available updates.
 */
export interface ComponentUpdate {
  /** Component name (e.g., "gamdl", "gamdl-gui", "python") */
  name: string;
  /** Currently installed version, or null if not installed */
  current_version: string | null;
  /** Latest available version from the update source, or null if check failed */
  latest_version: string | null;
  /** Whether an update is available (latest > current) */
  update_available: boolean;
  /** Whether the latest version is compatible with this app version */
  is_compatible: boolean;
  /** Release notes or description of the update, or null */
  description: string | null;
  /** URL to the release page (GitHub releases, PyPI, etc.), or null */
  release_url: string | null;
}

/**
 * Combined update check result for all application components.
 *
 * Mirrors: Rust struct `UpdateCheckResult` in `src-tauri/src/models/update.rs`
 *
 * Returned by the `check_all_updates` IPC command. The `has_updates` flag
 * is a convenience field that is true if any component has an update.
 * The `errors` array captures non-fatal errors (e.g., network timeout
 * when checking one component -- others may still succeed).
 */
export interface UpdateCheckResult {
  /** ISO 8601 timestamp of when this check was performed */
  checked_at: string;
  /** Convenience flag: true if any component has update_available == true */
  has_updates: boolean;
  /** Individual update status for each component */
  components: ComponentUpdate[];
  /** Non-fatal error messages from the update check process */
  errors: string[];
}
