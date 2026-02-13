// Copyright (c) 2024-2026 MeedyaDL
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
  output_path: '',               // Resolved to ~/Music (or platform equivalent) by backend
  language: 'en-US',             // Apple Music storefront language
  overwrite: false,              // Do not overwrite existing files by default
  auto_check_updates: true,      // Automatically check for updates on startup
  default_song_codec: 'alac',    // Preferred audio codec: Apple Lossless
  default_video_resolution: '2160p', // Preferred video quality: 4K
  default_video_codec_priority: 'h265,h264', // Try H.265 first, fall back to H.264
  default_video_remux_format: 'm4v', // Container format for remuxed music videos
  fallback_enabled: true,        // Enable quality fallback chains when preferred unavailable
  // Music codec fallback chain: tried in order when `default_song_codec` is unavailable
  music_fallback_chain: [
    'alac',           // 1st choice -- lossless
    'atmos',          // 2nd -- Dolby Atmos spatial audio
    'ac3',            // 3rd -- Dolby Digital
    'aac-binaural',   // 4th -- AAC binaural mix
    'aac',            // 5th -- standard AAC 256kbps
    'aac-legacy',     // 6th -- legacy AAC (44.1kHz cap)
  ],
  // Video resolution fallback chain: tried in order when preferred resolution unavailable
  video_fallback_chain: [
    '2160p',  // 4K
    '1440p',  // QHD
    '1080p',  // Full HD
    '720p',   // HD
    '540p',   // qHD
    '480p',   // SD
    '360p',   // Low
    '240p',   // Lowest
  ],
  synced_lyrics_format: 'lrc',   // Default lyrics format (LRC is most widely supported)
  no_synced_lyrics: false,       // Do download synced lyrics
  synced_lyrics_only: false,     // Also download plain-text lyrics
  save_cover: true,              // Save album artwork alongside audio files
  cover_format: 'raw',           // Keep original artwork format (usually JPEG from Apple)
  cover_size: 10000,             // Request maximum available artwork resolution from Apple CDN
  // Animated artwork (motion cover art) -- requires MusicKit credentials
  animated_artwork_enabled: false, // Disabled by default; needs Apple Developer setup
  musickit_team_id: null,          // Apple Developer Team ID (10-char)
  musickit_key_id: null,           // MusicKit private key identifier (10-char)
  // File/folder naming templates -- use GAMDL's template variable syntax
  album_folder_template: '{album_artist}/{album}',
  compilation_folder_template: 'Compilations/{album}',
  no_album_folder_template: '{artist}/Unknown Album',
  single_disc_file_template: '{track:02d} {title}',      // Zero-padded track number
  multi_disc_file_template: '{disc}-{track:02d} {title}', // Disc-track for multi-disc albums
  no_album_file_template: '{title}',
  playlist_file_template: 'Playlists/{playlist_artist}/{playlist_title}',
  // Tool paths -- null means "auto-detect from bundled/PATH"
  cookies_path: null,            // Netscape-format cookies file for authentication
  ffmpeg_path: null,             // FFmpeg binary for audio/video processing
  mp4decrypt_path: null,         // Bento4 mp4decrypt for DRM decryption
  mp4box_path: null,             // GPAC MP4Box for container manipulation
  nm3u8dlre_path: null,          // N_m3u8DL-RE for HLS/DASH stream downloading
  amdecrypt_path: null,          // Apple Music decryption tool
  download_mode: 'ytdlp',       // Stream download backend: yt-dlp (default) or N_m3u8DL-RE
  remux_mode: 'ffmpeg',         // Remuxing backend: FFmpeg (default) or MP4Box
  use_wrapper: false,            // Whether to use a remote account wrapper service
  wrapper_account_url: 'http://127.0.0.1:30020', // Default wrapper service URL (localhost)
  truncate: null,                // Max filename length in characters; null = no truncation
  fetch_extra_tags: true,        // Fetch extra metadata (normalization, smooth playback info)
  exclude_tags: [],              // Metadata tags to exclude from output files
  sidebar_collapsed: false,      // UI preference: sidebar expanded by default
  theme_override: null,          // null = follow OS theme; 'light' or 'dark' to override
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
  isLoading: false,  // No load in progress at creation time
  isDirty: false,    // No unsaved changes at creation time
  error: null,       // No error at creation time

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
      throw new Error(message);
    }
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
  resetToDefaults: () =>
    set({ settings: { ...DEFAULT_SETTINGS }, isDirty: true }),
}));
