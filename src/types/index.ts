/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * TypeScript type definitions mirroring Rust backend models.
 * These types ensure type safety across the IPC boundary between
 * the React frontend and the Tauri Rust backend.
 */

// ============================================================
// Audio/Video Quality Types
// ============================================================

/** All audio codec options supported by GAMDL's --song-codec flag */
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

/** Video resolution options for GAMDL */
export type VideoResolution =
  | '2160p'
  | '1440p'
  | '1080p'
  | '720p'
  | '540p'
  | '480p'
  | '360p'
  | '240p';

/** Synced lyrics format options */
export type LyricsFormat = 'lrc' | 'srt' | 'ttml';

/** Cover art image format options */
export type CoverFormat = 'jpg' | 'png' | 'raw';

/** Download mode (which tool to use for fetching streams) */
export type DownloadMode = 'ytdlp' | 'nm3u8dlre';

/** Remux mode (which tool to use for container conversion) */
export type RemuxMode = 'ffmpeg' | 'mp4box';

/** Log level for GAMDL's output verbosity */
export type LogLevel = 'DEBUG' | 'INFO' | 'WARNING' | 'ERROR';

// ============================================================
// Human-readable labels for codec/quality selectors
// ============================================================

/** Display names for audio codecs, shown in UI dropdowns */
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

/** Display names for video resolutions */
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

/** Complete set of GAMDL CLI options. All fields are optional for override merging. */
export interface GamdlOptions {
  song_codec?: SongCodec;
  music_video_codec_priority?: string;
  music_video_resolution?: VideoResolution;
  music_video_remux_format?: string;
  uploaded_video_quality?: string;
  disable_music_video_skip?: boolean;
  synced_lyrics_format?: LyricsFormat;
  no_synced_lyrics?: boolean;
  synced_lyrics_only?: boolean;
  save_cover?: boolean;
  cover_format?: CoverFormat;
  cover_size?: number;
  output_path?: string;
  temp_path?: string;
  overwrite?: boolean;
  truncate?: number;
  cookies_path?: string;
  use_wrapper?: boolean;
  wrapper_account_url?: string;
  wrapper_decrypt_ip?: string;
  language?: string;
  exclude_tags?: string;
  use_album_date?: boolean;
  fetch_extra_tags?: boolean;
  date_tag_template?: string;
  album_folder_template?: string;
  compilation_folder_template?: string;
  no_album_folder_template?: string;
  single_disc_file_template?: string;
  multi_disc_file_template?: string;
  no_album_file_template?: string;
  playlist_file_template?: string;
  ffmpeg_path?: string;
  mp4decrypt_path?: string;
  mp4box_path?: string;
  nm3u8dlre_path?: string;
  amdecrypt_path?: string;
  wvd_path?: string;
  download_mode?: DownloadMode;
  remux_mode?: RemuxMode;
  log_level?: LogLevel;
  no_exceptions?: boolean;
  save_playlist?: boolean;
  read_urls_as_txt?: boolean;
  no_config_file?: boolean;
}

// ============================================================
// Application Settings
// ============================================================

/** Complete application settings, persisted as JSON */
export interface AppSettings {
  output_path: string;
  language: string;
  overwrite: boolean;
  auto_check_updates: boolean;
  default_song_codec: SongCodec;
  default_video_resolution: VideoResolution;
  default_video_codec_priority: string;
  default_video_remux_format: string;
  fallback_enabled: boolean;
  music_fallback_chain: SongCodec[];
  video_fallback_chain: VideoResolution[];
  synced_lyrics_format: LyricsFormat;
  no_synced_lyrics: boolean;
  synced_lyrics_only: boolean;
  save_cover: boolean;
  cover_format: CoverFormat;
  cover_size: number;
  album_folder_template: string;
  compilation_folder_template: string;
  no_album_folder_template: string;
  single_disc_file_template: string;
  multi_disc_file_template: string;
  no_album_file_template: string;
  playlist_file_template: string;
  cookies_path: string | null;
  ffmpeg_path: string | null;
  mp4decrypt_path: string | null;
  mp4box_path: string | null;
  nm3u8dlre_path: string | null;
  amdecrypt_path: string | null;
  download_mode: DownloadMode;
  remux_mode: RemuxMode;
  use_wrapper: boolean;
  wrapper_account_url: string;
  truncate: number | null;
  exclude_tags: string[];
  sidebar_collapsed: boolean;
  theme_override: string | null;
}

// ============================================================
// Download Types
// ============================================================

/** A download request submitted from the UI */
export interface DownloadRequest {
  urls: string[];
  options?: GamdlOptions;
}

/** Possible states of a download queue item */
export type DownloadState =
  | 'queued'
  | 'downloading'
  | 'processing'
  | 'complete'
  | 'error'
  | 'cancelled';

/** Detailed status of a single download queue item */
export interface QueueItemStatus {
  id: string;
  urls: string[];
  state: DownloadState;
  progress: number;
  current_track: string | null;
  total_tracks: number | null;
  completed_tracks: number | null;
  speed: string | null;
  eta: string | null;
  error: string | null;
  output_path: string | null;
  codec_used: string | null;
  fallback_occurred: boolean;
  created_at: string;
}

/** Status of the entire download queue */
export interface QueueStatus {
  total: number;
  active: number;
  queued: number;
  completed: number;
  failed: number;
  items: QueueItemStatus[];
}

// ============================================================
// Dependency Types
// ============================================================

/** Status of a single dependency (Python, GAMDL, tool) */
export interface DependencyStatus {
  name: string;
  required: boolean;
  installed: boolean;
  version: string | null;
  path: string | null;
}

// ============================================================
// System Types
// ============================================================

/** Platform information from the backend */
export interface PlatformInfo {
  platform: string;
  arch: string;
  os_type: string;
}

/** Cookie file validation result */
export interface CookieValidation {
  valid: boolean;
  cookie_count: number;
  domains: string[];
  apple_music_cookies: number;
  expired: boolean;
  warnings: string[];
}

// ============================================================
// GAMDL Output Events (from subprocess parsing)
// ============================================================

/** Structured event parsed from GAMDL's stdout/stderr */
export type GamdlOutputEvent =
  | { type: 'track_info'; title: string; artist: string; album: string }
  | { type: 'download_progress'; percent: number; speed: string; eta: string }
  | { type: 'processing_step'; step: string }
  | { type: 'error'; message: string }
  | { type: 'complete'; path: string }
  | { type: 'unknown'; raw: string };

/** Progress event payload emitted via Tauri events */
export interface GamdlProgress {
  download_id: string;
  event: GamdlOutputEvent;
}

// ============================================================
// URL Parser Types
// ============================================================

/** Content types that can be detected from Apple Music URLs */
export type AppleMusicContentType =
  | 'song'
  | 'album'
  | 'playlist'
  | 'music-video'
  | 'artist'
  | 'unknown';

/** Result of parsing an Apple Music URL */
export interface ParsedUrl {
  url: string;
  contentType: AppleMusicContentType;
  isValid: boolean;
}

// ============================================================
// UI Types
// ============================================================

/** Navigation pages in the sidebar */
export type AppPage = 'download' | 'queue' | 'settings' | 'help';

/** Toast notification levels */
export type ToastType = 'success' | 'error' | 'warning' | 'info';

/** Toast notification data */
export interface Toast {
  id: string;
  message: string;
  type: ToastType;
  duration?: number;
}

/** Setup wizard step identifiers */
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

/** Identifies which music service a download targets */
export type MusicServiceId = 'apple-music' | 'youtube-music' | 'spotify';

/** Display labels for music services */
export const MUSIC_SERVICE_LABELS: Record<MusicServiceId, string> = {
  'apple-music': 'Apple Music',
  'youtube-music': 'YouTube Music',
  spotify: 'Spotify',
};

/** Capabilities that a music service may support */
export interface ServiceCapabilities {
  supports_lossless: boolean;
  supports_spatial_audio: boolean;
  supports_music_videos: boolean;
  supports_lyrics: boolean;
  supports_cover_art: boolean;
  requires_cookies: boolean;
  requires_oauth: boolean;
  supported_content_types: string[];
}

// ============================================================
// Update Types
// ============================================================

/** Update status for a single application component */
export interface ComponentUpdate {
  name: string;
  current_version: string | null;
  latest_version: string | null;
  update_available: boolean;
  is_compatible: boolean;
  description: string | null;
  release_url: string | null;
}

/** Combined update check result for all components */
export interface UpdateCheckResult {
  checked_at: string;
  has_updates: boolean;
  components: ComponentUpdate[];
  errors: string[];
}
