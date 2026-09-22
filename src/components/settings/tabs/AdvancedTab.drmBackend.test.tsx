/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file AdvancedTab.drmBackend.test.tsx -- Unit tests for the "Unlocking
 * Method" section (#1189).
 *
 * GAMDL 3.9 taught the download engine a second way to unlock Apple
 * Music's copy-protected tracks (PlayReady, alongside the original,
 * built-in Widevine one). This section in Settings > Advanced lets a
 * user switch to it, but only once it's actually possible to use --
 * showing the choice on an install that can't act on it would be worse
 * than not showing it at all. These tests cover the three states that
 * decision produces:
 *
 *   1. Hidden -- installed GAMDL is older than 3.9 and the saved
 *      setting is still the default ('widevine'). Nothing to show.
 *   2. Shown -- installed GAMDL is 3.9+, so the choice is real.
 *   3. Shown, with a note -- installed GAMDL is older than 3.9, but the
 *      saved setting is already 'playready' (chosen on a newer install
 *      that has since been swapped for an older one). Hiding the
 *      section here would hide the fact that PlayReady is silently not
 *      doing anything -- so it stays visible and says so.
 *
 * A fourth test covers the "don't flash on-screen" rule: the section
 * must not appear at all while the capability check is still in
 * flight, only once it resolves one way or the other.
 *
 * This is a separate file from SettingsTabs.test.tsx (rather than
 * folding these cases into it) because those existing tests share one
 * module-level `getGamdlCapabilities` mock that returns a single fixed
 * answer for the whole file -- exactly the sort of "same answer for
 * every test" shape unit tests normally want, but the wrong shape for
 * a feature whose whole point is behaving differently depending on
 * that answer. Keeping the per-test-controllable mock in its own file
 * avoids touching that shared fixture for every other test in it.
 *
 * @see src/components/settings/tabs/AdvancedTab.tsx -- the component under test
 * @see src/lib/tauri-commands.ts -- `GamdlCapabilities.play_ready_drm`
 */

import { render, screen, fireEvent, act } from '@testing-library/react';

import { AdvancedTab } from './AdvancedTab';
import { useSettingsStore } from '@/stores/settingsStore';
import { getGamdlCapabilities, type GamdlCapabilities } from '@/lib/tauri-commands';

/**
 * Mock lucide-react the same way SettingsTabs.test.tsx does. AdvancedTab
 * (and the file-private sub-components it defines -- CrashReportSection,
 * DiagnosticBundleSection, IntegrityScanSection, NotificationDiagnosticsRow,
 * WrapperSignInSection, WrapperUrlSecurityHint) import every one of these
 * icons; a stub is needed for each or the render throws.
 */
vi.mock('lucide-react', () => {
  const stub = (name: string) =>
    (props: Record<string, unknown>) => <span data-testid={`icon-${name}`} {...props} />;

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
  };
});

/**
 * Mock the Tauri IPC command wrappers AdvancedTab calls on mount.
 * `getGamdlCapabilities` is deliberately left as a plain `vi.fn()` with no
 * default resolved value -- every test below sets its own answer via
 * `vi.mocked(getGamdlCapabilities).mockResolvedValueOnce(...)`, because the
 * whole point of this file is checking three DIFFERENT answers.
 */
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
}));

/** Mock the Tauri shell plugin used by AdvancedTab's openExternal helper. */
vi.mock('@tauri-apps/plugin-shell', () => ({
  open: vi.fn().mockResolvedValue(undefined),
}));

/**
 * Every capability flag false -- the "installed GAMDL doesn't understand
 * any of the newer stuff" shape, and specifically the shape that means
 * "not 3.9" for the purposes of this file. Individual tests spread over
 * this and flip only `play_ready_drm`, so a future capability added to
 * `GamdlCapabilities` doesn't silently need updating in three places here.
 */
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
  play_ready_drm: false,
};

beforeEach(() => {
  useSettingsStore.setState({
    settings: {
      ...useSettingsStore.getState().settings,
      drm_backend: 'widevine',
      prd_path: '',
    },
  });
  vi.mocked(getGamdlCapabilities).mockReset();
});

describe('AdvancedTab -- Unlocking Method section (#1189)', () => {
  it('stays hidden when the installed GAMDL is older than 3.9 and the setting is still the default', async () => {
    vi.mocked(getGamdlCapabilities).mockResolvedValueOnce(ALL_CAPS_FALSE);

    await act(async () => {
      render(<AdvancedTab />);
    });

    // Nothing named "Unlocking Method" anywhere -- not the section
    // header, not a stray warning. Older GAMDL, default setting: this
    // whole feature has nothing useful to offer, so it says nothing.
    expect(screen.queryByText('Unlocking Method')).not.toBeInTheDocument();
  });

  it('shows the section once the installed GAMDL is 3.9 or newer', async () => {
    vi.mocked(getGamdlCapabilities).mockResolvedValueOnce({
      ...ALL_CAPS_FALSE,
      play_ready_drm: true,
    });

    await act(async () => {
      render(<AdvancedTab />);
    });

    const header = screen.getByText('Unlocking Method');
    expect(header).toBeInTheDocument();

    // Open it and check the actual choice is offered, with no stale-setting
    // warning (the setting is still the default 'widevine' in this test,
    // and the capability is genuinely true, so nothing is stale).
    fireEvent.click(header);
    expect(screen.getByText('Widevine (built in)')).toBeInTheDocument();
    expect(screen.getByText('PlayReady')).toBeInTheDocument();
    expect(
      screen.queryByText(/installed GAMDL is older than the version that understands it/i)
    ).not.toBeInTheDocument();
  });

  it('shows the section with an explanatory note when PlayReady is saved but the installed GAMDL cannot use it', async () => {
    // The dangerous case the task exists to cover: someone picked
    // PlayReady on a newer GAMDL, then went back to (or reinstalled) an
    // older one. The capability is false, but the saved setting is not
    // the default -- so hiding the section would hide an active setting
    // silently doing nothing.
    useSettingsStore.setState({
      settings: {
        ...useSettingsStore.getState().settings,
        drm_backend: 'playready',
        prd_path: '',
      },
    });
    vi.mocked(getGamdlCapabilities).mockResolvedValueOnce(ALL_CAPS_FALSE);

    await act(async () => {
      render(<AdvancedTab />);
    });

    const header = screen.getByText('Unlocking Method');
    expect(header).toBeInTheDocument();

    fireEvent.click(header);

    // The note has to say plainly that downloads are using Widevine
    // instead -- not just that something is "unsupported".
    expect(
      screen.getByText(/installed GAMDL is older than the version that understands it/i)
    ).toBeInTheDocument();
    expect(screen.getByText(/using the built-in Widevine unlocking instead/i)).toBeInTheDocument();
  });

  it('does not render the section while the capability check is still in flight', async () => {
    // A manually-resolved promise stands in for the in-flight IPC call so
    // the test can assert on the "still loading" moment before deciding
    // how it resolves.
    let resolveCapabilities!: (value: GamdlCapabilities) => void;
    vi.mocked(getGamdlCapabilities).mockReturnValueOnce(
      new Promise<GamdlCapabilities>((resolve) => {
        resolveCapabilities = resolve;
      })
    );

    render(<AdvancedTab />);

    // Still resolving -- the section must not have appeared yet, even
    // though nothing about the eventual answer is known yet either.
    expect(screen.queryByText('Unlocking Method')).not.toBeInTheDocument();

    // Now let it resolve to "GAMDL is 3.9+", inside act() so React
    // flushes the resulting state update before the next assertion.
    await act(async () => {
      resolveCapabilities({ ...ALL_CAPS_FALSE, play_ready_drm: true });
    });

    expect(screen.getByText('Unlocking Method')).toBeInTheDocument();
  });
});
