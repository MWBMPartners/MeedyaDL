/**
 * Copyright (c) 2026 MeedyaDL
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
import { useSettingsStore } from '@/stores/settingsStore';

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
    /* Download components (pulled via barrel transitive) */
    Pause: stub('Pause'),
    Play: stub('Play'),
    Upload: stub('Upload'),
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
      /* AdvancedTab fields */
      use_wrapper: false,
      auto_retry_without_wrapper: false,
      wrapper_account_url: 'http://127.0.0.1:30020',
      wrapper_m3u8_ip: '127.0.0.1:20020',
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
