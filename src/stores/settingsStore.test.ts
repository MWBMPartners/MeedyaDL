/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/stores/settingsStore.test.ts - Unit tests for the settings store
 *
 * Tests the settingsStore's load/save lifecycle, in-memory updates,
 * reset-to-defaults functionality, and error handling.
 *
 * The Tauri IPC commands (`getSettings`, `saveSettings`) are mocked via
 * vi.mock() so tests run without a Rust backend.
 *
 * @see src/stores/settingsStore.ts - The store under test
 * @see src/lib/tauri-commands.ts - The IPC wrappers being mocked
 */

import { useSettingsStore } from '@/stores/settingsStore';
import * as commands from '@/lib/tauri-commands';

import type { AppSettings } from '@/types';

/**
 * Mock the tauri-commands module so we can control IPC responses.
 * vi.mock() is hoisted by Vitest to execute before any imports.
 */
vi.mock('@/lib/tauri-commands', () => ({
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
}));

/**
 * A complete mock AppSettings object for testing.
 * Uses non-default values so we can distinguish loaded settings from defaults.
 */
const MOCK_SETTINGS: AppSettings = {
  output_path: '/tmp/test-output',
  language: 'ja-JP',
  overwrite: true,
  auto_check_updates: false,
  default_song_codec: 'aac',
  default_video_resolution: '1080p',
  default_video_codec_priority: 'h264,h265',
  default_video_remux_format: 'mp4',
  fallback_enabled: false,
  music_fallback_chain: ['aac', 'aac-legacy'],
  video_fallback_chain: ['1080p', '720p'],
  synced_lyrics_format: 'srt',
  no_synced_lyrics: true,
  synced_lyrics_only: false,
  save_cover: false,
  cover_format: 'png',
  cover_size: 600,
  album_folder_template: '{artist}/{album}',
  compilation_folder_template: 'Various/{album}',
  no_album_folder_template: '{artist}/Singles',
  single_disc_file_template: '{track} {title}',
  multi_disc_file_template: '{disc}-{track} {title}',
  no_album_file_template: '{title}',
  playlist_file_template: 'Playlists/{playlist_title}',
  cookies_path: '/tmp/cookies.txt',
  ffmpeg_path: null,
  mp4decrypt_path: null,
  mp4box_path: null,
  nm3u8dlre_path: null,
  amdecrypt_path: null,
  download_mode: 'nm3u8dlre',
  remux_mode: 'mp4box',
  use_wrapper: true,
  wrapper_account_url: 'http://localhost:9999',
  truncate: 100,
  exclude_tags: ['rating'],
  sidebar_collapsed: true,
  theme_override: 'dark',
};

/**
 * Reset the store and mocks before each test to prevent state leakage.
 */
beforeEach(() => {
  vi.clearAllMocks();
  useSettingsStore.setState({
    settings: {
      output_path: '',
      language: 'en-US',
      overwrite: false,
      auto_check_updates: true,
      default_song_codec: 'alac',
      default_video_resolution: '2160p',
      default_video_codec_priority: 'h265,h264',
      default_video_remux_format: 'm4v',
      fallback_enabled: true,
      music_fallback_chain: ['alac', 'atmos', 'ac3', 'aac-binaural', 'aac', 'aac-legacy'],
      video_fallback_chain: ['2160p', '1440p', '1080p', '720p', '540p', '480p', '360p', '240p'],
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
    },
    isLoading: false,
    isDirty: false,
    error: null,
  });
});

describe('settingsStore', () => {
  // =========================================================================
  // Initial State
  // =========================================================================
  describe('initial state', () => {
    it('starts with default settings', () => {
      const { settings } = useSettingsStore.getState();
      expect(settings.default_song_codec).toBe('alac');
      expect(settings.output_path).toBe('');
      expect(settings.fallback_enabled).toBe(true);
    });

    it('starts not loading', () => {
      expect(useSettingsStore.getState().isLoading).toBe(false);
    });

    it('starts not dirty', () => {
      expect(useSettingsStore.getState().isDirty).toBe(false);
    });

    it('starts with no error', () => {
      expect(useSettingsStore.getState().error).toBeNull();
    });
  });

  // =========================================================================
  // loadSettings
  // =========================================================================
  describe('loadSettings', () => {
    it('loads settings from the backend and updates the store', async () => {
      vi.mocked(commands.getSettings).mockResolvedValueOnce(MOCK_SETTINGS);

      await useSettingsStore.getState().loadSettings();

      const state = useSettingsStore.getState();
      expect(state.settings.output_path).toBe('/tmp/test-output');
      expect(state.settings.default_song_codec).toBe('aac');
      expect(state.settings.language).toBe('ja-JP');
      expect(state.isLoading).toBe(false);
      expect(state.isDirty).toBe(false);
    });

    it('sets isLoading during the load', async () => {
      /* Create a promise we control to inspect intermediate state */
      let resolve!: (value: AppSettings) => void;
      const pending = new Promise<AppSettings>((r) => { resolve = r; });
      vi.mocked(commands.getSettings).mockReturnValueOnce(pending);

      const loadPromise = useSettingsStore.getState().loadSettings();
      expect(useSettingsStore.getState().isLoading).toBe(true);

      resolve(MOCK_SETTINGS);
      await loadPromise;
      expect(useSettingsStore.getState().isLoading).toBe(false);
    });

    it('sets error on backend failure', async () => {
      vi.mocked(commands.getSettings).mockRejectedValueOnce(
        new Error('File not found'),
      );

      await useSettingsStore.getState().loadSettings();

      const state = useSettingsStore.getState();
      expect(state.error).toBe('File not found');
      expect(state.isLoading).toBe(false);
    });

    it('handles non-Error rejections', async () => {
      vi.mocked(commands.getSettings).mockRejectedValueOnce('string error');

      await useSettingsStore.getState().loadSettings();

      expect(useSettingsStore.getState().error).toBe('string error');
    });

    it('clears previous error on new load', async () => {
      /* First load fails */
      vi.mocked(commands.getSettings).mockRejectedValueOnce(new Error('fail'));
      await useSettingsStore.getState().loadSettings();
      expect(useSettingsStore.getState().error).toBe('fail');

      /* Second load succeeds */
      vi.mocked(commands.getSettings).mockResolvedValueOnce(MOCK_SETTINGS);
      await useSettingsStore.getState().loadSettings();
      expect(useSettingsStore.getState().error).toBeNull();
    });
  });

  // =========================================================================
  // saveSettings
  // =========================================================================
  describe('saveSettings', () => {
    it('saves current settings to the backend', async () => {
      vi.mocked(commands.saveSettings).mockResolvedValueOnce(undefined);

      /* Mark as dirty first */
      useSettingsStore.getState().updateSettings({ output_path: '/new/path' });
      expect(useSettingsStore.getState().isDirty).toBe(true);

      await useSettingsStore.getState().saveSettings();

      /* Verify the save command was called with the current settings */
      expect(commands.saveSettings).toHaveBeenCalledWith(
        expect.objectContaining({ output_path: '/new/path' }),
      );
      expect(useSettingsStore.getState().isDirty).toBe(false);
    });

    it('sets error and re-throws on backend failure', async () => {
      vi.mocked(commands.saveSettings).mockRejectedValueOnce(
        new Error('Disk full'),
      );

      await expect(useSettingsStore.getState().saveSettings()).rejects.toThrow(
        'Disk full',
      );

      expect(useSettingsStore.getState().error).toBe('Disk full');
    });
  });

  // =========================================================================
  // updateSettings (in-memory)
  // =========================================================================
  describe('updateSettings', () => {
    it('merges partial updates into settings', () => {
      useSettingsStore.getState().updateSettings({
        default_song_codec: 'atmos',
        cover_size: 500,
      });

      const { settings } = useSettingsStore.getState();
      expect(settings.default_song_codec).toBe('atmos');
      expect(settings.cover_size).toBe(500);
      /* Other fields should be preserved */
      expect(settings.language).toBe('en-US');
    });

    it('marks the store as dirty', () => {
      useSettingsStore.getState().updateSettings({ language: 'fr-FR' });
      expect(useSettingsStore.getState().isDirty).toBe(true);
    });

    it('preserves all other settings when updating one field', () => {
      const before = { ...useSettingsStore.getState().settings };
      useSettingsStore.getState().updateSettings({ overwrite: true });

      const after = useSettingsStore.getState().settings;
      /* Only overwrite should have changed */
      expect(after.overwrite).toBe(true);
      expect(after.default_song_codec).toBe(before.default_song_codec);
      expect(after.output_path).toBe(before.output_path);
      expect(after.music_fallback_chain).toEqual(before.music_fallback_chain);
    });
  });

  // =========================================================================
  // resetToDefaults
  // =========================================================================
  describe('resetToDefaults', () => {
    it('resets all settings to default values', () => {
      /* First change some settings */
      useSettingsStore.getState().updateSettings({
        default_song_codec: 'atmos',
        cover_size: 500,
        output_path: '/custom/path',
      });

      useSettingsStore.getState().resetToDefaults();

      const { settings } = useSettingsStore.getState();
      expect(settings.default_song_codec).toBe('alac');
      expect(settings.cover_size).toBe(1200);
      expect(settings.output_path).toBe('');
    });

    it('marks the store as dirty after reset', () => {
      useSettingsStore.getState().resetToDefaults();
      expect(useSettingsStore.getState().isDirty).toBe(true);
    });
  });
});
