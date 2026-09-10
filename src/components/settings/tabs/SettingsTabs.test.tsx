/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file SettingsTabs.test.tsx -- Component rendering tests for settings tabs.
 *
 * Tests the GeneralTab, QualityTab, and AdvancedTab components for:
 *   - Toggle rendering with correct initial checked state from the settings store
 *   - Toggle click handling (verifies updateSettings is called with new values)
 *   - Conditional visibility (wrapper section shown only when use_wrapper is true)
 *   - Select dropdowns rendering with correct options and initial values
 *
 * Mocking strategy:
 *   - `lucide-react` is mocked to avoid importing the full SVG icon library.
 *   - `@/lib/tauri-commands` is mocked because the AdvancedTab imports IPC
 *     functions that require the Tauri runtime (unavailable in jsdom tests).
 *   - `@tauri-apps/plugin-shell` is mocked because the AdvancedTab dynamically
 *     imports it for opening external URLs.
 *   - The `useSettingsStore` Zustand store is initialized with known defaults
 *     before each test and read back after interactions to verify state changes.
 *
 * @see src/components/settings/tabs/GeneralTab.tsx
 * @see src/components/settings/tabs/QualityTab.tsx
 * @see src/components/settings/tabs/AdvancedTab.tsx
 * @see src/stores/settingsStore.ts
 */

import { render, screen, fireEvent, act } from '@testing-library/react';

import { GeneralTab } from './GeneralTab';
import { QualityTab } from './QualityTab';
import { AdvancedTab } from './AdvancedTab';
import { MetadataTab } from './MetadataTab';
import { CoverArtTab } from './CoverArtTab';
import { useSettingsStore } from '@/stores/settingsStore';

// Test-only helper that switches the shared i18next instance to German or
// French without a real network fetch (see src/testing/i18n.ts). Used by
// the "GeneralTab language notice" tests below to check what the screen
// looks like once the app is actually displaying one of those languages.
import { useTestLanguage } from '@/testing/i18n';

/**
 * Mock lucide-react to avoid importing the full SVG icon library in tests.
 *
 * Every icon imported by the settings tab components and their transitive
 * dependencies (common barrel re-exports ToastContainer, UpdateBanner, etc.)
 * is replaced with a lightweight <span> stub. The complete list is derived
 * from all `import { ... } from 'lucide-react'` statements in the src/ tree.
 */
vi.mock('lucide-react', () => {
  /** Factory that creates a stub span component for a given icon name */
  const stub = (name: string) =>
    (props: Record<string, unknown>) => <span data-testid={`icon-${name}`} {...props} />;

  return {
    /* Common components */
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
    /* Settings tabs */
    Trash2: stub('Trash2'),
    /* GeneralTab: Send Test Notification button (#658) */
    Bell: stub('Bell'),
    /* FallbackChainList: remove / re-add buttons (#659) */
    Plus: stub('Plus'),
    /* Download components (pulled via barrel transitive) */
    Pause: stub('Pause'),
    Play: stub('Play'),
    Upload: stub('Upload'),
    /* StatusPill (#911-4) is re-exported from @/components/common and
     * imports its own state-→-icon map. The mock needs these icons or
     * StatusPill fails to render when the barrel is touched. */
    Clock: stub('Clock'),
    Loader2: stub('Loader2'),
    XCircle: stub('XCircle'),
    /* M9-UI: RiskPill three-tier icons + ShieldOff for the Revoke
     * Consent button + Headphones for the Spotify tab sidebar icon. */
    ShieldCheck: stub('ShieldCheck'),
    Shield: stub('Shield'),
    ShieldOff: stub('ShieldOff'),
    Headphones: stub('Headphones'),
  };
});

/**
 * Mock the Tauri IPC command wrappers used by AdvancedTab.
 *
 * AdvancedTab imports `testWrapperConnection`, `storeCredential`,
 * `getCredential`, `validateMusicKitCredentialsWithInput`,
 * `hasEmbeddedMusicKitToken`, `hasEmbeddedAcoustidKey`, and `auditApiFields`
 * directly from `@/lib/tauri-commands`. These functions call `invoke()` which
 * is unavailable in the jsdom test environment.
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
  // #887: AdvancedTab calls `getGamdlCapabilities()` on mount to decide
  // whether to show the wrapper-m3u8 input and the FFmpeg-path field.
  // Return a stub with every capability `false` so the test renders the
  // post-3.6 layout — none of the tests assert against the gated rows.
  getGamdlCapabilities: vi.fn().mockResolvedValue({
    wrapper_v2: false,
    native_muxing: false,
    aac_web_codec_rename: false,
    music_video_remux_mode: false,
    wrapper_m3u8_ip: false,
    playlist_folder_template: false,
    native_codec_priority: false,
    ffmpeg_path: false,
    assets_api_unlocks_lossy_codecs: false,
  }),
}));

/**
 * Mock the Tauri shell plugin used by AdvancedTab's openExternal helper.
 * The dynamic `import('@tauri-apps/plugin-shell')` resolves to this mock.
 */
vi.mock('@tauri-apps/plugin-shell', () => ({
  open: vi.fn().mockResolvedValue(undefined),
}));

/**
 * Reset the settings store to a clean default state before each test.
 *
 * Zustand stores are singletons -- mutations from one test would leak into
 * the next without this explicit reset. We spread the existing defaults and
 * override specific fields to ensure a predictable starting state.
 */
beforeEach(() => {
  useSettingsStore.setState({
    settings: {
      ...useSettingsStore.getState().settings,
      /* GeneralTab fields */
      overwrite: false,
      auto_start_queue: true,
      auto_check_updates: true,
      check_pre_releases: false,
      update_channel: 'stable',
      update_check_interval_hours: 6,
      gamdl_idle_timeout_minutes: 5,
      theme_override: null,
      language: 'en-US',
      ui_language: '',
      /* QualityTab fields */
      default_song_codec: 'alac',
      default_video_resolution: '2160p',
      fallback_enabled: true,
      companion_mode: 'atmos_to_lossless',
      music_video_companion: false,
      musicbrainz_lookup: false,
      musicbrainz_search_fallback: true,
      /* AdvancedTab fields */
      use_wrapper: false,
      auto_retry_without_wrapper: false,
      wrapper_account_url: 'http://127.0.0.1:30020',
      wrapper_m3u8_ip: '127.0.0.1:20020',
      wrapper_decrypt_ip: '127.0.0.1:10020',
      download_mode: 'ytdlp',
      remux_mode: 'ffmpeg',
      sentry_enabled: false,
      verbose_activity_log: false,
      verbose_gamdl_exceptions: false,
      activity_log_path_override: '',
    },
    isDirty: false,
    isLoading: false,
    error: null,
  });
});

// =============================================================================
// GeneralTab
// =============================================================================
describe('GeneralTab', () => {
  // ===========================================================================
  // Toggle rendering -- initial state
  // ===========================================================================

  /**
   * Verifies that the "Overwrite Existing Files" toggle renders as unchecked
   * when `settings.overwrite` is false (the default). The Toggle component
   * uses `role="switch"` and `aria-checked` for accessibility.
   */
  it('renders Overwrite toggle as unchecked when overwrite is false', () => {
    render(<GeneralTab />);

    const toggle = screen.getByRole('switch', { name: /overwrite existing files/i });
    expect(toggle).toBeInTheDocument();
    expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  /**
   * Verifies that the "Auto-Start Downloads" toggle renders as checked
   * when `settings.auto_start_queue` is true (the default).
   */
  it('renders Auto-Start Downloads toggle as checked when auto_start_queue is true', () => {
    render(<GeneralTab />);

    const toggle = screen.getByRole('switch', { name: /auto-start downloads/i });
    expect(toggle).toBeInTheDocument();
    expect(toggle).toHaveAttribute('aria-checked', 'true');
  });

  // ===========================================================================
  // Toggle click handling
  // ===========================================================================

  /**
   * Verifies that clicking the "Overwrite Existing Files" toggle updates the
   * settings store. When clicked, the Toggle component calls
   * `onChange(!checked)` which triggers `updateSettings({ overwrite: true })`.
   */
  it('updates settings store when Overwrite toggle is clicked', () => {
    render(<GeneralTab />);

    const toggle = screen.getByRole('switch', { name: /overwrite existing files/i });
    fireEvent.click(toggle);

    /* The store should now have overwrite set to true */
    const { settings } = useSettingsStore.getState();
    expect(settings.overwrite).toBe(true);
  });

  /**
   * Verifies that clicking the "Auto-Start Downloads" toggle flips the
   * setting from true to false and updates the store accordingly.
   */
  it('updates settings store when Auto-Start Downloads toggle is clicked', () => {
    render(<GeneralTab />);

    const toggle = screen.getByRole('switch', { name: /auto-start downloads/i });
    fireEvent.click(toggle);

    const { settings } = useSettingsStore.getState();
    expect(settings.auto_start_queue).toBe(false);
  });

  // ===========================================================================
  // Conditional visibility -- Update check interval
  // ===========================================================================

  /**
   * Verifies that the "Check Interval" select dropdown is visible when
   * `auto_check_updates` is true. The GeneralTab conditionally renders
   * this dropdown only when auto-check is enabled.
   */
  it('shows Check Interval select when auto_check_updates is true', () => {
    render(<GeneralTab />);

    expect(screen.getByLabelText(/check interval/i)).toBeInTheDocument();
  });

  /**
   * Verifies that the "Check Interval" select dropdown is hidden when
   * `auto_check_updates` is false.
   */
  it('hides Check Interval select when auto_check_updates is false', () => {
    useSettingsStore.setState({
      settings: {
        ...useSettingsStore.getState().settings,
        auto_check_updates: false,
      },
    });

    render(<GeneralTab />);

    expect(screen.queryByLabelText(/check interval/i)).not.toBeInTheDocument();
  });

  // ===========================================================================
  // Language dropdown -- machine-translation disclosure (#111)
  // ===========================================================================

  /**
   * German and French were translated by a machine and have not been
   * read through by a person who speaks the language. The dropdown says
   * so directly on the option itself -- the option text comes straight
   * from `LOCALES` in `src/lib/i18n.ts` (`nativeName` + the language's
   * own `machineAssistedLabel`), so this test is really checking that
   * GeneralTab builds its options from that data instead of a
   * hand-written label that could drift out of sync with it.
   */
  it('shows the machine-translation qualifier on the German and French options, but not on English', () => {
    render(<GeneralTab />);

    const select = screen.getByLabelText('Language') as HTMLSelectElement;
    const options = Array.from(select.options);

    expect(options.find((o) => o.value === 'de')?.textContent).toBe(
      'Deutsch (automatische Übersetzung)',
    );
    expect(options.find((o) => o.value === 'fr')?.textContent).toBe(
      'Français (traduction automatique)',
    );
    expect(options.find((o) => o.value === 'en')?.textContent).toBe('English');
  });

  /**
   * The note below the dropdown is not shown at all while the app is
   * displaying English -- there is nothing to disclaim about the
   * original wording.
   */
  it('does not show the machine-assisted note while the app is displaying English', () => {
    render(<GeneralTab />);

    expect(
      screen.queryByText(/was made by a machine/i),
    ).not.toBeInTheDocument();
  });

  /**
   * Once the app is actually showing German (not just "German is picked
   * in the dropdown" -- see the comment on GeneralTab's `i18n.language`
   * hook usage for why those are different moments), the note appears,
   * written in German, telling the reader plainly that the translation
   * hasn't been checked by a person yet.
   */
  it('shows the machine-assisted note, in German, once the app is displaying German', async () => {
    await useTestLanguage('de');
    try {
      render(<GeneralTab />);
      expect(
        screen.getByText(/wurde maschinell erstellt/),
      ).toBeInTheDocument();
    } finally {
      await useTestLanguage('en');
    }
  });

  /** Same check, for French. */
  it('shows the machine-assisted note, in French, once the app is displaying French', async () => {
    await useTestLanguage('fr');
    try {
      render(<GeneralTab />);
      expect(
        screen.getByText(/réalisée par une machine/),
      ).toBeInTheDocument();
    } finally {
      await useTestLanguage('en');
    }
  });
});

// =============================================================================
// QualityTab
// =============================================================================
describe('QualityTab', () => {
  // ===========================================================================
  // Select dropdown -- audio codec
  // ===========================================================================

  /**
   * Verifies that the "Default Audio Codec" select renders with the correct
   * initial value from the settings store. The default is 'alac' (Lossless).
   */
  it('renders Default Audio Codec select with initial value from settings', () => {
    render(<QualityTab />);

    const select = screen.getByLabelText(/default audio codec/i) as HTMLSelectElement;
    expect(select).toBeInTheDocument();
    expect(select.value).toBe('alac');
  });

  /**
   * Verifies that the audio codec select contains the expected codec options.
   * The options are derived from the SONG_CODEC_LABELS record in types/index.ts.
   */
  it('renders audio codec select with all codec options', () => {
    render(<QualityTab />);

    const select = screen.getByLabelText(/default audio codec/i) as HTMLSelectElement;
    const options = Array.from(select.options);

    /* Check that key codec options are present */
    const values = options.map((o) => o.value);
    expect(values).toContain('alac');
    expect(values).toContain('atmos');
    expect(values).toContain('aac');
    expect(values).toContain('aac-legacy');
  });

  /**
   * Verifies that changing the audio codec select updates the settings store.
   */
  it('updates settings store when audio codec is changed', () => {
    render(<QualityTab />);

    const select = screen.getByLabelText(/default audio codec/i);
    fireEvent.change(select, { target: { value: 'aac' } });

    const { settings } = useSettingsStore.getState();
    expect(settings.default_song_codec).toBe('aac');
  });

  // ===========================================================================
  // Select dropdown -- video resolution
  // ===========================================================================

  /**
   * Verifies that the "Default Video Resolution" select renders with the
   * correct initial value. The default is '2160p' (4K UHD).
   */
  it('renders Default Video Resolution select with initial value from settings', () => {
    render(<QualityTab />);

    const select = screen.getByLabelText(/default video resolution/i) as HTMLSelectElement;
    expect(select).toBeInTheDocument();
    expect(select.value).toBe('2160p');
  });

  /**
   * Verifies that the video resolution select contains all 8 resolution options
   * from 4K down to 240p.
   */
  it('renders video resolution select with all resolution options', () => {
    render(<QualityTab />);

    const select = screen.getByLabelText(/default video resolution/i) as HTMLSelectElement;
    const options = Array.from(select.options);
    const values = options.map((o) => o.value);

    expect(values).toContain('2160p');
    expect(values).toContain('1080p');
    expect(values).toContain('720p');
    expect(values).toContain('240p');
    expect(options).toHaveLength(8);
  });

  // ===========================================================================
  // Toggle -- fallback chain
  // ===========================================================================

  /**
   * Verifies that the "Enable Fallback Chain" toggle renders as checked
   * when `settings.fallback_enabled` is true (the default).
   */
  it('renders Fallback Chain toggle as checked when fallback_enabled is true', () => {
    render(<QualityTab />);

    const toggle = screen.getByRole('switch', { name: /enable fallback chain/i });
    expect(toggle).toHaveAttribute('aria-checked', 'true');
  });

  /**
   * Verifies that clicking the fallback chain toggle updates the store.
   */
  it('updates settings store when Fallback Chain toggle is clicked', () => {
    render(<QualityTab />);

    const toggle = screen.getByRole('switch', { name: /enable fallback chain/i });
    fireEvent.click(toggle);

    const { settings } = useSettingsStore.getState();
    expect(settings.fallback_enabled).toBe(false);
  });
});

// =============================================================================
// AdvancedTab
// =============================================================================
describe('AdvancedTab', () => {
  // ===========================================================================
  // Toggle rendering -- wrapper toggle
  // ===========================================================================

  /**
   * Verifies that the "Use Wrapper" toggle renders with aria-checked="false"
   * when `settings.use_wrapper` is false (the default). The Wrapper section
   * in AdvancedTab uses `defaultOpen={false}`, so we need to open the section
   * first by clicking its header.
   */
  it('renders Use Wrapper toggle as unchecked when use_wrapper is false', async () => {
    /*
     * AdvancedTab renders CrashReportSection which calls listCrashReports()
     * in a useEffect. Wrapping in act() flushes those async state updates
     * and suppresses the "not wrapped in act()" warnings.
     */
    await act(async () => {
      render(<AdvancedTab />);
    });

    /* The Wrapper section has defaultOpen={false}, so click its header to expand it */
    const wrapperHeader = screen.getByText('Wrapper');
    fireEvent.click(wrapperHeader);

    /*
     * The "Use Wrapper" toggle has a helpTopic prop which renders a nested
     * HelpButton inside the label. This complicates accessible name resolution,
     * so we find the toggle by locating its label text and traversing to the
     * sibling switch button within the same <label> wrapper.
     */
    const labelText = screen.getByText('Use Wrapper');
    const label = labelText.closest('label')!;
    const toggle = label.querySelector('[role="switch"]')!;
    expect(toggle).toBeInTheDocument();
    expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  // ===========================================================================
  // Conditional visibility -- wrapper section
  // ===========================================================================

  /**
   * Verifies that the "Auto-Retry without Wrapper" toggle and
   * "Wrapper Account URL" input are NOT shown when `use_wrapper` is false.
   * The AdvancedTab conditionally renders these elements only when the
   * wrapper toggle is enabled.
   */
  it('hides wrapper detail controls when use_wrapper is false', async () => {
    await act(async () => {
      render(<AdvancedTab />);
    });

    /* Open the Wrapper section */
    const wrapperHeader = screen.getByText('Wrapper');
    fireEvent.click(wrapperHeader);

    /* The auto-retry toggle and URL input should not be present */
    expect(screen.queryByRole('switch', { name: /auto-retry without wrapper/i })).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/wrapper account url/i)).not.toBeInTheDocument();
  });

  /**
   * Verifies that enabling `use_wrapper` reveals the wrapper detail controls:
   * the "Auto-Retry without Wrapper" toggle, the URL input, and the
   * "Test Connection" button.
   */
  it('shows wrapper detail controls when use_wrapper is true', async () => {
    useSettingsStore.setState({
      settings: {
        ...useSettingsStore.getState().settings,
        use_wrapper: true,
      },
    });

    await act(async () => {
      render(<AdvancedTab />);
    });

    /* Open the Wrapper section */
    const wrapperHeader = screen.getByText('Wrapper');
    fireEvent.click(wrapperHeader);

    /* The auto-retry toggle, URL input, and test button should now be visible */
    expect(screen.getByRole('switch', { name: /auto-retry without wrapper/i })).toBeInTheDocument();
    expect(screen.getByText('Test Connection')).toBeInTheDocument();
  });

  /**
   * Verifies that clicking the "Use Wrapper" toggle in the AdvancedTab
   * updates the store and causes the wrapper detail controls to appear.
   */
  it('updates settings store and reveals details when Use Wrapper is toggled on', async () => {
    await act(async () => {
      render(<AdvancedTab />);
    });

    /* Open the Wrapper section */
    const wrapperHeader = screen.getByText('Wrapper');
    fireEvent.click(wrapperHeader);

    /*
     * Click the Use Wrapper toggle to enable it. We locate the toggle via
     * its label text and traverse to the sibling switch button (same approach
     * as the render test above, since the HelpButton complicates name lookup).
     */
    const labelText = screen.getByText('Use Wrapper');
    const label = labelText.closest('label')!;
    const toggle = label.querySelector('[role="switch"]') as HTMLElement;
    fireEvent.click(toggle);

    /* The store should reflect the change */
    const { settings } = useSettingsStore.getState();
    expect(settings.use_wrapper).toBe(true);

    /* The "Auto-Retry without Wrapper" toggle should now be visible */
    expect(screen.getByRole('switch', { name: /auto-retry without wrapper/i })).toBeInTheDocument();
  });

  // ===========================================================================
  // Select dropdown -- processing modes
  // ===========================================================================

  /**
   * Verifies that the "Download Mode" select renders with the default value
   * of 'ytdlp' and contains both available options.
   */
  it('renders Download Mode select with correct initial value and options', async () => {
    await act(async () => {
      render(<AdvancedTab />);
    });

    const select = screen.getByLabelText(/download mode/i) as HTMLSelectElement;
    expect(select.value).toBe('ytdlp');

    const options = Array.from(select.options);
    expect(options).toHaveLength(2);
    expect(options[0].value).toBe('ytdlp');
    expect(options[1].value).toBe('nm3u8dlre');
  });

  /**
   * Verifies that changing the Download Mode select updates the settings store.
   */
  it('updates settings store when Download Mode is changed', async () => {
    await act(async () => {
      render(<AdvancedTab />);
    });

    const select = screen.getByLabelText(/download mode/i);
    fireEvent.change(select, { target: { value: 'nm3u8dlre' } });

    const { settings } = useSettingsStore.getState();
    expect(settings.download_mode).toBe('nm3u8dlre');
  });

  // ===========================================================================
  // Toggle -- error reporting
  // ===========================================================================

  /**
   * Verifies that the "Send Anonymous Crash Reports" toggle renders as
   * unchecked when `settings.sentry_enabled` is false (the default).
   */
  it('renders Sentry toggle as unchecked when sentry_enabled is false', async () => {
    await act(async () => {
      render(<AdvancedTab />);
    });

    const toggle = screen.getByRole('switch', { name: /send anonymous crash reports/i });
    expect(toggle).toHaveAttribute('aria-checked', 'false');
  });
});

// =============================================================================
// MetadataTab
// =============================================================================
describe('MetadataTab', () => {
  // ===========================================================================
  // Toggle -- song.link lookup
  // ===========================================================================

  /**
   * Verifies that clicking the "Look up where else each album is
   * available" toggle updates the store's `odesli_lookup_enabled` field.
   */
  it('updates settings store when song.link lookup toggle is clicked', () => {
    render(<MetadataTab />);

    const toggle = screen.getByRole('switch', {
      name: /look up where else each album is available/i,
    });
    fireEvent.click(toggle);

    const { settings } = useSettingsStore.getState();
    expect(settings.odesli_lookup_enabled).toBe(true);
  });
});

describe('CoverArtTab', () => {
  // ===========================================================================
  // Toggle -- cross-service cover art upgrade (#1159)
  // ===========================================================================

  /**
   * Verifies that clicking the "Upgrade Cover Art From Other Services"
   * toggle updates the store's `best_cover_art_enabled` field. The toggle
   * only renders when `save_cover` is on, which it is by default.
   */
  it('updates settings store when the cover art upgrade toggle is clicked', () => {
    render(<CoverArtTab />);

    const toggle = screen.getByRole('switch', {
      name: /upgrade cover art from other services/i,
    });
    fireEvent.click(toggle);

    const { settings } = useSettingsStore.getState();
    expect(settings.best_cover_art_enabled).toBe(true);
  });

  // ===========================================================================
  // Cover Size field -- validated on blur, not on every keystroke (a11y audit Fix 11)
  // ===========================================================================
  //
  // This field used to be wired straight to the stored setting: the
  // <input>'s `value` came directly from `coverSize.value`, and
  // `onChange` only called `coverSize.set()` when the typed text was
  // ALREADY a whole number between 100 and 10000. Since a controlled
  // input's displayed value is whatever `value` says it is, and "1"
  // (the first character of typing "1000" from an empty box) is not a
  // valid size, `set()` never ran -- so the box just showed the OLD
  // stored number again on the very next render, as if the "1" had
  // been rejected. Typing a new multi-digit value from scratch was
  // impossible, and nothing on screen explained why.

  it('keeps exactly what was typed while typing, even before it becomes a valid number', () => {
    render(<CoverArtTab />);
    const input = screen.getByLabelText(/cover size/i);

    // Simulates typing "1000" one character at a time into an empty
    // field. "1", "10", and "100" are all below the 100-pixel minimum
    // and so, under the old implementation, would each have been
    // silently rejected -- the box would show the previous stored
    // value (10000, the default) instead of what was actually typed.
    fireEvent.change(input, { target: { value: '1' } });
    expect(input).toHaveValue(1);

    fireEvent.change(input, { target: { value: '10' } });
    expect(input).toHaveValue(10);

    fireEvent.change(input, { target: { value: '100' } });
    expect(input).toHaveValue(100);

    fireEvent.change(input, { target: { value: '1000' } });
    expect(input).toHaveValue(1000);

    // Leaving the field with a valid number commits it, and shows no error.
    fireEvent.blur(input);
    expect(useSettingsStore.getState().settings.cover_size).toBe(1000);
    expect(
      screen.queryByText(/enter a whole number between 100 and 10000/i)
    ).not.toBeInTheDocument();
  });

  it('shows a message and does not save an out-of-range value when the field is left', () => {
    // Set an explicit, known baseline rather than relying on whatever
    // default `cover_size` happens to carry -- the previous test in
    // this file legitimately changes it via the settings store, which
    // is a real cross-test singleton, not a per-test isolate.
    useSettingsStore.setState({
      settings: { ...useSettingsStore.getState().settings, cover_size: 5000 },
    });
    render(<CoverArtTab />);
    const input = screen.getByLabelText(/cover size/i);

    fireEvent.change(input, { target: { value: '50' } }); // below the 100-pixel minimum
    fireEvent.blur(input);

    /* Says what is allowed, instead of silently doing nothing. */
    expect(
      screen.getByText(/enter a whole number between 100 and 10000/i)
    ).toBeInTheDocument();
    /* What was typed is still visible -- not silently reverted. */
    expect(input).toHaveValue(50);
    /* The invalid value never reached the stored setting. */
    expect(useSettingsStore.getState().settings.cover_size).toBe(5000);
  });
});
