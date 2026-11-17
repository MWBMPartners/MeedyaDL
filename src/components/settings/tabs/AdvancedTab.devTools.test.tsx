/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file AdvancedTab.devTools.test.tsx -- Deactivating developer access
 * keeps unsaved changes (#1439).
 *
 * Deactivate writes one setting (`dev_access_enabled = false`) to disk.
 * It used to reload every setting from disk afterwards, which threw away
 * any change the person had made on the Settings screen and not yet saved.
 * Now only that one field is updated in the page's copy.
 *
 * @see src/components/settings/tabs/AdvancedTab.tsx -- DevToolsSection
 */

import { render, screen, fireEvent, act } from '@testing-library/react';

import { AdvancedTab } from './AdvancedTab';
import { useSettingsStore } from '@/stores/settingsStore';
import { deactivateDevAccess, type GamdlCapabilities } from '@/lib/tauri-commands';

vi.mock('lucide-react', () => {
  const stub = (name: string) => (props: Record<string, unknown>) => (
    <span data-testid={`icon-${name}`} {...props} />
  );

  return {
    CheckCircle: stub('CheckCircle'),
    AlertCircle: stub('AlertCircle'),
    AlertTriangle: stub('AlertTriangle'),
    Info: stub('Info'),
    X: stub('X'),
    ArrowUpCircle: stub('ArrowUpCircle'),
    ExternalLink: stub('ExternalLink'),
    RefreshCw: stub('RefreshCw'),
    Download: stub('Download'),
    RotateCcw: stub('RotateCcw'),
    FolderOpen: stub('FolderOpen'),
    File: stub('File'),
    HelpCircle: stub('HelpCircle'),
    GripVertical: stub('GripVertical'),
    ArrowUp: stub('ArrowUp'),
    ArrowDown: stub('ArrowDown'),
    Trash2: stub('Trash2'),
    Bell: stub('Bell'),
    Plus: stub('Plus'),
    Pause: stub('Pause'),
    Play: stub('Play'),
    Upload: stub('Upload'),
    Clock: stub('Clock'),
    Loader2: stub('Loader2'),
    XCircle: stub('XCircle'),
    ShieldCheck: stub('ShieldCheck'),
    Shield: stub('Shield'),
    ShieldOff: stub('ShieldOff'),
    Headphones: stub('Headphones'),
    ChevronRight: stub('ChevronRight'),
  };
});

vi.mock('@/lib/tauri-commands', () => ({
  testWrapperConnection: vi.fn().mockResolvedValue({ reachable: false }),
  storeCredential: vi.fn().mockResolvedValue(undefined),
  getCredential: vi.fn().mockResolvedValue(null),
  validateMusicKitCredentialsWithInput: vi.fn().mockResolvedValue('OK'),
  hasEmbeddedMusicKitToken: vi.fn().mockResolvedValue(false),
  hasEmbeddedAcoustidKey: vi.fn().mockResolvedValue(false),
  auditApiFields: vi.fn().mockResolvedValue(null),
  listCrashReports: vi.fn().mockResolvedValue([]),
  deleteCrashReport: vi.fn().mockResolvedValue(undefined),
  deleteAllCrashReports: vi.fn().mockResolvedValue(undefined),
  getGithubIssueUrl: vi.fn().mockResolvedValue(''),
  getGamdlCapabilities: vi.fn(),
  hasWebplayerToken: vi.fn().mockResolvedValue(false),
  clearWebplayerToken: vi.fn().mockResolvedValue(undefined),
  deactivateDevAccess: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@tauri-apps/plugin-shell', () => ({
  open: vi.fn().mockResolvedValue(undefined),
}));

const ALL_CAPS_FALSE: GamdlCapabilities = {
  wrapper_v2: false,
  native_muxing: false,
  aac_web_codec_rename: false,
  music_video_remux_mode: false,
  wrapper_m3u8_ip: false,
  playlist_folder_template: false,
  native_codec_priority: false,
  ffmpeg_path: false,
  assets_api_unlocks_lossy_codecs: false,
  wrapper_decrypt_host_port: false,
  play_ready_drm: false,
  gamdl_v4: false,
  gamdl_four_saves_album_cover: false,
};

describe('AdvancedTab -- Deactivate Developer Access (#1439)', () => {
  it('keeps an unsaved change made elsewhere on the Settings screen', async () => {
    const { getGamdlCapabilities } = await import('@/lib/tauri-commands');
    vi.mocked(getGamdlCapabilities).mockResolvedValue(ALL_CAPS_FALSE);
    const base = useSettingsStore.getState().settings;
    useSettingsStore.setState({
      settings: { ...base, dev_access_enabled: true, gamdl_log_level: 'DEBUG' },
      savedSettings: { ...base, dev_access_enabled: true, gamdl_log_level: 'INFO' },
      isDirty: true,
    });

    await act(async () => {
      render(<AdvancedTab />);
    });
    await act(async () => {
      fireEvent.click(screen.getByText('Deactivate Developer Access'));
    });

    expect(deactivateDevAccess).toHaveBeenCalledTimes(1);
    const state = useSettingsStore.getState();
    // Developer access is off, on screen and in the saved copy...
    expect(state.settings.dev_access_enabled).toBe(false);
    expect(state.savedSettings.dev_access_enabled).toBe(false);
    // ...and the unsaved log-level change is still there, still unsaved.
    expect(state.settings.gamdl_log_level).toBe('DEBUG');
    expect(state.isDirty).toBe(true);
  });

  it('leaves developer access on screen as it was when turning it off fails', async () => {
    const { getGamdlCapabilities } = await import('@/lib/tauri-commands');
    vi.mocked(getGamdlCapabilities).mockResolvedValue(ALL_CAPS_FALSE);
    vi.mocked(deactivateDevAccess).mockRejectedValueOnce('could not save');
    const base = useSettingsStore.getState().settings;
    useSettingsStore.setState({
      settings: { ...base, dev_access_enabled: true },
      savedSettings: { ...base, dev_access_enabled: true },
      isDirty: false,
    });

    await act(async () => {
      render(<AdvancedTab />);
    });
    await act(async () => {
      fireEvent.click(screen.getByText('Deactivate Developer Access'));
    });

    expect(useSettingsStore.getState().settings.dev_access_enabled).toBe(true);
    expect(useSettingsStore.getState().savedSettings.dev_access_enabled).toBe(true);
  });
});
