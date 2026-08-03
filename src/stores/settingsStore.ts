// Copyright (c) 2026 MeedyaSuite
/**
 * @file settingsStore.ts -- Application Settings State Management Store
 * @license MIT -- See LICENSE file in the project root.
 *
 * Manages the complete lifecycle of application settings:
 *
 *   **Load flow** (on app startup):
 *   1. `<App>` calls `loadSettings()` during its initial `useEffect`.
 *   2. This invokes the Rust command `get_settings` via `tauri-commands.ts`.
 *   3. The Rust backend reads `settings.json` from the app data directory,
 *      merges it with defaults for any missing keys, and returns `AppSettings`.
 *   4. The store replaces its state with the loaded settings, clears `isDirty`.
 *
 *   **Save flow** (when user clicks "Save"):
 *   1. `<SettingsPage>` calls `saveSettings()`.
 *   2. This invokes the Rust command `save_settings` via `tauri-commands.ts`,
 *      passing the current `settings` object over the Tauri IPC bridge.
 *   3. The Rust backend writes `settings.json` AND syncs relevant fields to
 *      GAMDL's `config.ini` (so the CLI tool picks up the same preferences).
 *   4. On success the store clears `isDirty`; on failure it sets `error`.
 *
 *   **Edit flow** (in-memory only):
 *   - `updateSettings(partial)` shallow-merges partial changes and sets `isDirty = true`.
 *   - The settings page can check `isDirty` to show/enable the Save button.
 *
 * Consumed by: `<SettingsPage>`, `<App>` (startup), `<DownloadPage>` (quality defaults),
 * `<Sidebar>` (collapse preference), and any component needing user preferences.
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state} -- Zustand state updates
 * @see {@link https://v2.tauri.app/develop/calling-rust/} -- Tauri `invoke()` IPC bridge
 */

// Zustand `create` builds a React hook backed by a single store instance.
// Components call `useSettingsStore(selector)` to subscribe to specific fields.
import { create } from 'zustand';

// AppSettings -- the full settings shape mirroring the Rust `AppSettings` struct.
// Every field is non-optional at rest; partial updates use `Partial<AppSettings>`.
import type { AppSettings } from '@/types';

// Type-safe wrappers around `invoke()` -- each maps to a `#[tauri::command]` in Rust.
// `getSettings` -> Rust `get_settings`, `saveSettings` -> Rust `save_settings`.
import * as commands from '@/lib/tauri-commands';

// Debounce timer for auto-save operations. Prevents concurrent writes when
// multiple settings are toggled rapidly. Only the last save within 300ms fires.
let _saveDebounceTimer: ReturnType<typeof setTimeout> | null = null;

/**
 * Default settings used as the initial store state and as a reset target.
 *
 * These values mirror the `Default` trait implementation for `AppSettings`
 * in the Rust backend (`src-tauri/src/models/settings.rs`). Keeping them
 * in sync ensures the frontend can display sensible defaults even before
 * the first `loadSettings()` call completes.
 *
 * Key defaults:
 *   - `output_path: ''`       -- Empty string signals "use OS default Music folder"
 *   - `default_song_codec: 'alac'` -- Lossless Apple audio by default
 *   - `fallback_enabled: true` -- If preferred codec unavailable, try the chain
 *   - `download_mode: 'ytdlp'` -- Use yt-dlp for stream fetching
 */
const DEFAULT_SETTINGS: AppSettings = {
  output_path: '', // Resolved to ~/Music (or platform equivalent) by backend
  temp_path: '', // Resolved to {OS temp}/MeedyaDL by backend
  language: 'en-US', // Apple Music storefront language
  storefront: '', // Auto-detect from language region (e.g., en-GB → gb)
  overwrite: false, // Do not overwrite existing files by default
  ui_language: '', // Auto-detect UI language from OS locale
  auto_check_updates: true, // Automatically check for updates on startup
  check_pre_releases: false, // Only show stable releases by default
  update_channel: 'stable', // Subscribe to the stable release channel
  update_check_interval_hours: 6, // Check for updates every 6 hours
  gamdl_idle_timeout_minutes: 5, // Kill hung GAMDL after 5 min of silent output (#505)
  auto_start_queue: true, // Start processing immediately when items are enqueued
  abort_queue_confirm: true, // Show confirmation modal before abort fires (#620)
  desktop_notifications: true, // OS-native notifications for download events when window not focused
  notification_style: 'native_and_in_app' as const, // Both native + in-app by default
  smart_redownload_detection: true, // Detect changes via API lastModifiedDate before re-downloading
  clipboard_monitoring: true, // Monitor clipboard for supported URLs
  default_song_codec: 'alac', // Preferred audio codec: Apple Lossless
  default_video_resolution: '2160p', // Preferred video quality: 4K
  default_video_codec_priority: 'h265,h264', // Try H.265 first, fall back to H.264
  default_video_remux_format: 'm4v', // Container format for remuxed music videos
  fallback_enabled: true, // Enable quality fallback chains when preferred unavailable
  // Music codec fallback chain: tried in order when `default_song_codec` is unavailable
  music_fallback_chain: [
    'alac', // 1st choice -- lossless
    'atmos', // 2nd -- Dolby Atmos spatial audio
    'ac3', // 3rd -- Dolby Digital
    'aac-binaural', // 4th -- AAC binaural mix
    'aac', // 5th -- standard AAC 256kbps
    'aac-legacy', // 6th -- legacy AAC (44.1kHz cap)
  ],
  // Video resolution fallback chain: tried in order when preferred resolution unavailable
  video_fallback_chain: [
    '2160p', // 4K
    '1440p', // QHD
    '1080p', // Full HD
    '720p', // HD
    '540p', // qHD
    '480p', // SD
    '360p', // Low
    '240p', // Lowest
  ],
  companion_mode: 'atmos_to_lossless', // Atmos → also download ALAC companion (default)
  custom_companion_codecs: [], // Only relevant when companion_mode is 'custom'
  music_video_companion: false, // Disabled by default — requires MusicKit credentials
  musicbrainz_lookup: false, // MusicBrainz video/cross-platform discovery (no creds needed)
  artist_auto_select: null, // No default; let GAMDL use its own default for artist URLs
  artist_auto_select_multi: [], // Multi-mode: MeedyaDL creates N downloads for artist URLs
  // Pre-queue duplicate detection (#510). On by default, skipping songs that
  // appear in multiple artist-auto-select modes (e.g. album + single + compilation).
  // Does NOT affect companion-format downloads (ALAC/Atmos/AAC/etc).
  duplicate_detection: {
    scope: 'intra_and_queued',
    preference_order: [
      'main-albums',
      'singles-eps',
      'compilation-albums',
      'live-albums',
      'top-songs',
    ],
    key_strategy: 'song_id_isrc_fallback',
  },
  embed_lyrics_and_sidecar: true, // Embed lyrics in metadata
  keep_lyrics_sidecar: true, // Keep .lrc/.srt/.ttml sidecar files alongside embedded lyrics
  enhanced_lrc: true, // Convert TTML to Enhanced LRC with word-by-word sync
  lyrics_fallback_enabled: true, // If TTML unavailable, try LRC (audio) or SRT (video)
  generate_webvtt: false, // Opt-in: generate .vtt from TTML/SRT/LRC
  generate_rich_srt: true, // On by default: strictly improves SRT quality from TTML
  embed_subtitles: false, // Opt-in: embed SRT/VTT in media containers
  generate_ass: false, // Opt-in: generate ASS subtitles from TTML/WebVTT
  generate_lyricsfile: false, // Opt-in: generate Lyricsfile (.lyrics) YAML — experimental, #596
  content_advisory_in_filenames: true, // Append [Explicit]/[Clean] to filenames
  synced_lyrics_format: 'ttml', // Default lyrics format (TTML preserves word-level timing)
  no_synced_lyrics: false, // Do download synced lyrics
  synced_lyrics_only: false, // Also download plain-text lyrics
  companion_lyrics_formats: ['srt'], // SRT as companion format
  save_cover: true, // Save album artwork alongside audio files
  cover_format: 'jpg', // JPEG default; GAMDL 2.8.4 crashes with 'raw' format
  cover_size: 10000, // Request maximum available artwork resolution from Apple CDN
  cover_art_name: 'front_cover' as const, // Rename Cover → FrontCover after download (#448)
  music_video_embed_cover_sidecar: true, // Embed MV cover into MP4 + delete sidecar (#533 / #569)
  // Animated artwork (motion cover art) -- requires MusicKit credentials
  animated_artwork_enabled: true, // Enabled by default (#449); gracefully skips when no credentials
  hide_animated_artwork: false, // Show artwork files in file browsers by default (#449)
  artist_promo_video_enabled: true, // Download artist promo video to artist folder (#453)
  animated_artwork_resolution: 'fhd' as const, // Cap animated artwork HLS renditions at ~1080p by default (#972)
  best_cover_art_enabled: false, // Cross-platform highest-resolution cover-art picker (M9-3) — opt-in
  musickit_team_id: null, // Apple Developer Team ID (10-char)
  musickit_key_id: null, // MusicKit private key identifier (10-char)
  // Metadata enrichment (opt-in post-download processing)
  acoustid_enabled: false, // AcoustID fingerprinting (embedded Chromaprint)
  acoustid_api_key: '', // AcoustID application API key (user-provided)
  replaygain_enabled: false, // ReplayGain loudness analysis (uses FFmpeg)
  replaygain_reference_level: -18.0, // EBU R128 default (-18 LUFS)
  replaygain_prevent_clipping: true, // Limit gain to prevent clipping
  replaygain_album_gain: true, // Write album-level ReplayGain tags
  // File/folder naming templates -- use GAMDL's template variable syntax
  album_folder_template: '{album_artist}/{album}',
  compilation_folder_template: 'Compilations/{album}',
  no_album_folder_template: '{artist}/Unknown Album',
  // GAMDL v3.0+ only (#618). Stored unconditionally; the Rust side gates
  // CLI emission behind the detected GAMDL version so v2.9.x falls back to
  // upstream's built-in default.
  playlist_folder_template: 'Playlists/{playlist_artist}',
  single_disc_file_template: '{track:02d} {title}', // Zero-padded track number
  multi_disc_file_template: '{disc}-{track:02d} {title}', // Disc-track for multi-disc albums
  no_album_file_template: '{title}',
  playlist_file_template: 'Playlists/{playlist_artist}/{playlist_title}',
  // Padding strategies for {track} and {disc} placeholders (#587).
  // Auto-derive widths from track_total / disc_total — sorts box sets correctly.
  track_number_padding: 'auto' as const,
  disc_number_padding: 'auto' as const,
  // Tool paths -- null means "auto-detect from bundled/PATH"
  cookies_path: null, // Netscape-format cookies file for authentication
  ffmpeg_path: null, // FFmpeg binary for audio/video processing
  mp4decrypt_path: null, // Bento4 mp4decrypt for DRM decryption
  mp4box_path: null, // GPAC MP4Box for container manipulation
  nm3u8dlre_path: null, // N_m3u8DL-RE for HLS/DASH stream downloading
  mediainfo_path: null, // MediaInfo CLI for accurate codec detection
  download_mode: 'ytdlp', // Stream download backend: yt-dlp (default) or N_m3u8DL-RE
  remux_mode: 'ffmpeg', // Remuxing backend: FFmpeg (default) or MP4Box
  use_wrapper: false, // Whether to use a remote account wrapper service
  auto_retry_without_wrapper: false, // Auto-retry without wrapper when wrapper download fails
  storefront_fallback_on_failure: true, // Retry once with account region when URL storefront 404s (#666)
  wrapper_account_url: 'http://127.0.0.1:30020', // wrapper-v1 account URL (GAMDL <= 3.5.x)
  wrapper_m3u8_ip: '127.0.0.1:20020', // wrapper-v1 m3u8 address (GAMDL 3.1–3.5.x)
  wrapper_decrypt_ip: '127.0.0.1:10020', // wrapper-v1 decryption address (#743, GAMDL <= 3.5.x)
  wrapper_url: 'http://127.0.0.1', // wrapper-v2 HTTP base URL (#853, GAMDL >= 3.6)
  truncate: null, // Max filename length in characters; null = no truncation
  fetch_extra_tags: true, // Fetch extra metadata (normalization, smooth playback info)
  exclude_tags: [], // Metadata tags to exclude from output files
  sentry_enabled: false, // Opt-in anonymous crash reporting via Sentry (default: off)
  verbose_activity_log: false, // Detailed [VERBOSE] activity log (may expose sensitive data)
  verbose_gamdl_exceptions: false, // Pass --no-exceptions to GAMDL by default; flip on for upstream bug reports
  gamdl_log_level: 'INFO', // GAMDL subprocess --log-level. Default matches GAMDL's compiled-in default; Developer Tools surface flips it to DEBUG (#768).
  activity_log_path_override: '', // Empty = use {app_data_dir}/logs/ for on-disk activity log (#541)
  dev_access_enabled: false, // Internal developer access mode (hidden, not in normal Settings UI)
  spotify_consent_acknowledged: false, // M9-4: first-run consent acknowledgment for Spotify downloads
  relocation_declined: false, // #1057: macOS self-relocation offer not declined by default
  last_seen_version: '', // Last app version the user launched (empty = first run)
  setup_completed: false, // Whether the setup wizard has been completed at least once
  sidebar_collapsed: false, // UI preference: sidebar expanded by default
  theme_override: null, // null = follow OS theme; 'light' or 'dark' to override
  high_contrast: false, // High-contrast accessibility theme (auto-activates via OS prefers-contrast)
  colour_blind_mode: '', // Colour vision deficiency mode: '', 'deuteranopia', 'protanopia', or 'tritanopia'
};

/**
 * Combined state + actions interface for the settings store.
 *
 * Zustand stores co-locate state and actions in a single object, unlike Redux
 * which separates them into reducers and action creators. This keeps the API
 * surface small: components call `useSettingsStore((s) => s.someField)` for
 * reactive reads and `useSettingsStore.getState().someAction()` for fire-and-forget.
 *
 * The async actions (`loadSettings`, `saveSettings`) communicate with the Rust
 * backend via the Tauri IPC bridge (see `@/lib/tauri-commands.ts`).
 */
interface SettingsState {
  // ---------------------------------------------------------------------------
  // State fields
  // ---------------------------------------------------------------------------

  /**
   * The current in-memory settings object. Initialized to `DEFAULT_SETTINGS`
   * and replaced wholesale on `loadSettings()` success. Individual fields are
   * updated via `updateSettings(partial)`.
   */
  settings: AppSettings;

  /**
   * `true` while `loadSettings()` is awaiting the Rust backend response.
   * Components can show a loading spinner while this is set.
   */
  isLoading: boolean;

  /**
   * `true` when `updateSettings()` has been called but `saveSettings()` has not
   * yet been called (or has failed). The `<SettingsPage>` uses this to
   * conditionally enable the "Save" button and show an unsaved-changes indicator.
   */
  isDirty: boolean;

  /**
   * Human-readable error message from the most recent `loadSettings()` or
   * `saveSettings()` failure. `null` when there is no error.
   */
  error: string | null;

  // ---------------------------------------------------------------------------
  // Actions
  // ---------------------------------------------------------------------------

  /**
   * Load settings from the Rust backend.
   * Calls `commands.getSettings()` -> Rust `get_settings` command.
   * On success: replaces `settings` and clears `isDirty`.
   * On failure: sets `error` with the failure message.
   */
  loadSettings: () => Promise<void>;

  /**
   * Persist the current in-memory settings to disk via the Rust backend.
   * Calls `commands.saveSettings(settings)` -> Rust `save_settings` command.
   * The backend writes both `settings.json` and syncs to GAMDL `config.ini`.
   * On success: clears `isDirty`.
   * On failure: sets `error` and re-throws so the calling component can react.
   */
  saveSettings: () => Promise<void>;

  /** Debounced save — batches rapid changes within 300ms into one write. */
  debouncedSave: () => void;

  /**
   * Merge partial changes into the current settings (in-memory only).
   * Uses the spread operator to produce a new `settings` object, ensuring
   * Zustand detects the change via reference inequality.
   * Marks `isDirty = true` to signal unsaved changes.
   * @param partial -- One or more `AppSettings` fields to overwrite
   */
  updateSettings: (partial: Partial<AppSettings>) => void;

  /**
   * Reset all settings to `DEFAULT_SETTINGS`. Marks `isDirty = true` so the
   * user must explicitly save (or discard) the reset.
   */
  resetToDefaults: () => void;
}

/**
 * Zustand store hook for application settings.
 *
 * Usage in components:
 *   const codec = useSettingsStore((s) => s.settings.default_song_codec);
 *   const { saveSettings } = useSettingsStore();
 *
 * The store creator receives both `set` (for state updates) and `get` (for
 * reading current state inside async actions without stale closures).
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state}
 */
export const useSettingsStore = create<SettingsState>((set, get) => ({
  // -------------------------------------------------------------------------
  // Initial state -- populated with defaults until loadSettings() completes
  // -------------------------------------------------------------------------
  settings: DEFAULT_SETTINGS,
  isLoading: false, // No load in progress at creation time
  isDirty: false, // No unsaved changes at creation time
  error: null, // No error at creation time

  // -------------------------------------------------------------------------
  // Actions
  // -------------------------------------------------------------------------

  /**
   * Asynchronously load settings from the Rust backend.
   * Sets `isLoading` before the IPC call and clears it on completion.
   *
   * IPC call: `commands.getSettings()` -> Rust `get_settings` (#[tauri::command])
   * The Rust handler reads `settings.json` from the app data directory, merges
   * with struct defaults for any missing fields (forward compatibility), and
   * returns the fully-populated `AppSettings` struct.
   */
  loadSettings: async () => {
    // Signal loading state and clear any previous error before the IPC call.
    set({ isLoading: true, error: null });
    try {
      // Invoke the Rust `get_settings` command over the Tauri IPC bridge.
      const settings = await commands.getSettings();
      // Replace the entire settings object and mark as clean (not dirty).
      set({ settings, isLoading: false, isDirty: false });
    } catch (e) {
      // Normalize the error to a string regardless of its runtime type.
      const message = e instanceof Error ? e.message : String(e);
      set({ error: message, isLoading: false });
    }
  },

  /**
   * Persist the current in-memory settings to the Rust backend.
   * Uses `get()` to read the latest settings snapshot at call time,
   * avoiding stale closures when the action is called from an event handler.
   *
   * IPC call: `commands.saveSettings(settings)` -> Rust `save_settings`
   * The Rust handler writes `settings.json` to disk AND translates relevant
   * fields into GAMDL's `config.ini` format for CLI compatibility.
   *
   * On failure, the error is both stored in state (for UI display) and
   * re-thrown so the calling component's catch block can take additional action
   * (e.g., showing a toast notification).
   */
  saveSettings: async () => {
    // Clear any stale error before attempting the save.
    set({ error: null });
    try {
      // `get().settings` reads the current settings at invocation time.
      await commands.saveSettings(get().settings);
      // Mark as clean: no unsaved changes after a successful save.
      set({ isDirty: false });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      // Store the error for reactive UI display.
      set({ error: message });
      // Re-throw so callers can chain their own error handling.
      throw new Error(message, { cause: e });
    }
  },

  /**
   * Debounced save — batches rapid save calls into a single disk write.
   * Use this for auto-save triggers (e.g., toggle switches that save immediately).
   * The manual "Save" button should call `saveSettings()` directly for instant feedback.
   */
  debouncedSave: () => {
    if (_saveDebounceTimer) clearTimeout(_saveDebounceTimer);
    _saveDebounceTimer = setTimeout(async () => {
      _saveDebounceTimer = null;
      try {
        await commands.saveSettings(get().settings);
        set({ isDirty: false });
      } catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        set({ error: message });
      }
    }, 300);
  },

  /**
   * Merge a partial settings update into the current settings object.
   * Produces a new object reference via spread so Zustand detects the change.
   *
   * Example: `updateSettings({ default_song_codec: 'aac' })` changes only
   * the codec while preserving all other ~40 settings fields.
   *
   * This is an in-memory-only operation -- call `saveSettings()` afterward
   * to persist the change to disk.
   */
  updateSettings: (partial) =>
    set((state) => ({
      settings: { ...state.settings, ...partial },
      isDirty: true, // Flag that there are unsaved changes
    })),

  /**
   * Reset all settings fields back to `DEFAULT_SETTINGS`.
   * Creates a fresh copy via spread to ensure referential inequality.
   * Marks `isDirty = true` because the reset has not been saved to disk yet.
   */
  resetToDefaults: () => set({ settings: { ...DEFAULT_SETTINGS }, isDirty: true }),
}));
