/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Settings state store.
 * Manages application settings with load/save operations that sync
 * to the Rust backend. Settings are persisted as JSON in the app
 * data directory and also synced to GAMDL's config.ini.
 */

import { create } from 'zustand';
import type { AppSettings } from '@/types';
import * as commands from '@/lib/tauri-commands';

/** Default settings matching the Rust backend's Default impl */
const DEFAULT_SETTINGS: AppSettings = {
  output_path: '',
  language: 'en-US',
  overwrite: false,
  auto_check_updates: true,
  default_song_codec: 'alac',
  default_video_resolution: '2160p',
  default_video_codec_priority: 'h265,h264',
  default_video_remux_format: 'm4v',
  fallback_enabled: true,
  music_fallback_chain: [
    'alac',
    'atmos',
    'ac3',
    'aac-binaural',
    'aac',
    'aac-legacy',
  ],
  video_fallback_chain: [
    '2160p',
    '1440p',
    '1080p',
    '720p',
    '540p',
    '480p',
    '360p',
    '240p',
  ],
  synced_lyrics_format: 'lrc',
  no_synced_lyrics: false,
  synced_lyrics_only: false,
  save_cover: true,
  cover_format: 'raw',
  cover_size: 1200,
  album_folder_template: '{album_artist}/{album}',
  compilation_folder_template: 'Compilations/{album}',
  no_album_folder_template: '{artist}/Unknown Album',
  single_disc_file_template: '{track:02d} {title}',
  multi_disc_file_template: '{disc}-{track:02d} {title}',
  no_album_file_template: '{title}',
  playlist_file_template: 'Playlists/{playlist_artist}/{playlist_title}',
  cookies_path: null,
  ffmpeg_path: null,
  mp4decrypt_path: null,
  mp4box_path: null,
  nm3u8dlre_path: null,
  amdecrypt_path: null,
  download_mode: 'ytdlp',
  remux_mode: 'ffmpeg',
  use_wrapper: false,
  wrapper_account_url: 'http://127.0.0.1:30020',
  truncate: null,
  exclude_tags: [],
  sidebar_collapsed: false,
  theme_override: null,
};

interface SettingsState {
  /** Current application settings */
  settings: AppSettings;
  /** Whether settings are currently being loaded from disk */
  isLoading: boolean;
  /** Whether settings have unsaved changes */
  isDirty: boolean;
  /** Error message from the last load/save operation */
  error: string | null;

  /** Load settings from the backend (or use defaults if first run) */
  loadSettings: () => Promise<void>;
  /** Save current settings to disk */
  saveSettings: () => Promise<void>;
  /** Update one or more settings fields (marks as dirty) */
  updateSettings: (partial: Partial<AppSettings>) => void;
  /** Reset settings to defaults */
  resetToDefaults: () => void;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: DEFAULT_SETTINGS,
  isLoading: false,
  isDirty: false,
  error: null,

  loadSettings: async () => {
    set({ isLoading: true, error: null });
    try {
      const settings = await commands.getSettings();
      set({ settings, isLoading: false, isDirty: false });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      set({ error: message, isLoading: false });
    }
  },

  saveSettings: async () => {
    set({ error: null });
    try {
      await commands.saveSettings(get().settings);
      set({ isDirty: false });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      set({ error: message });
      throw new Error(message);
    }
  },

  updateSettings: (partial) =>
    set((state) => ({
      settings: { ...state.settings, ...partial },
      isDirty: true,
    })),

  resetToDefaults: () =>
    set({ settings: { ...DEFAULT_SETTINGS }, isDirty: true }),
}));
