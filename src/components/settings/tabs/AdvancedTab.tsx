/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file AdvancedTab.tsx -- Advanced options settings tab.
 *
 * Renders the "Advanced" tab within the {@link SettingsPage} component.
 * This tab exposes expert-level settings that most users will not need to
 * change, organised into three sections:
 *
 * ## Section 1: Processing
 *
 *   - **Download Mode** -- Selects which tool is used for fetching HLS
 *     streams from Apple Music's CDN. Maps to `settings.download_mode`
 *     and GAMDL's `--download-mode` flag.
 *       - `ytdlp` (recommended): Uses yt-dlp, the standard choice
 *       - `nm3u8dlre`: Uses N_m3u8DL-RE as an alternative
 *
 *   - **Remux Mode** -- Selects which tool remuxes downloaded streams into
 *     the final container format. Maps to `settings.remux_mode` and
 *     GAMDL's `--remux-mode` flag.
 *       - `ffmpeg` (recommended): Uses FFmpeg for remuxing
 *       - `mp4box`: Uses MP4Box as an alternative
 *
 * ## Section 2: Wrapper
 *
 *   - **Use Wrapper** -- Toggle to use a wrapper service for account
 *     authentication instead of cookies. Maps to `settings.use_wrapper`.
 *   - **Wrapper Account URL** -- The endpoint URL for the wrapper service.
 *     Only shown when the wrapper toggle is enabled (conditional render).
 *     Maps to `settings.wrapper_account_url`.
 *
 * ## Section 3: File Options
 *
 *   - **Truncate Filenames** -- Maximum filename length in characters.
 *     When set, filenames exceeding this length are truncated. Maps to
 *     `settings.truncate` (nullable number).
 *   - **Excluded Tags** -- Comma-separated list of metadata tags to strip
 *     from downloaded files (e.g., "lyrics, comment"). Maps to
 *     `settings.exclude_tags: string[]`.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore`.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 * @see {@link @/types/index.ts}           -- DownloadMode, RemuxMode types
 */

import { useState, useEffect } from 'react';

// Zustand store for reading/writing advanced settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared form components: Select for mode dropdowns, Toggle for boolean switches,
// Input for text/number fields, Button for actions.
import { Select, Toggle, Input, Button } from '@/components/common';

// TypeScript union types for download and remux mode values.
import type { DownloadMode, RemuxMode, WrapperTestResult } from '@/types';

// Platform detection hook for Wrapper/AMdecrypt feature gating.
import { usePlatform } from '@/hooks/usePlatform';

// IPC command for wrapper connection testing.
import { testWrapperConnection } from '@/lib/tauri-commands';

/**
 * Download mode dropdown options.
 * yt-dlp is the recommended default; N_m3u8DL-RE is provided as an
 * alternative for users who prefer or require it.
 */
const DOWNLOAD_MODE_OPTIONS = [
  { value: 'ytdlp', label: 'yt-dlp (recommended)' },
  { value: 'nm3u8dlre', label: 'N_m3u8DL-RE (alternative)' },
];

/**
 * Remux mode dropdown options.
 * FFmpeg is the recommended default; MP4Box is provided as an alternative.
 */
const REMUX_MODE_OPTIONS = [
  { value: 'ffmpeg', label: 'FFmpeg (recommended)' },
  { value: 'mp4box', label: 'MP4Box (alternative)' },
];

/**
 * AdvancedTab -- Renders the Advanced settings tab.
 *
 * Contains three sections: Processing (download/remux mode), Wrapper
 * (authentication service config), and File Options (truncation and
 * excluded tags). The Wrapper URL field uses conditional rendering,
 * only appearing when `settings.use_wrapper` is true.
 */
export function AdvancedTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting advanced setting changes */
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  /** Platform detection for Wrapper feature gating (Linux x86_64 only) */
  const { supportsWrapper } = usePlatform();

  /** Wrapper connection test state */
  const [testState, setTestState] = useState<
    'idle' | 'testing' | 'success' | 'error'
  >('idle');
  const [testResult, setTestResult] = useState<WrapperTestResult | null>(null);

  // Reset test result when the wrapper URL changes
  useEffect(() => {
    setTestState('idle');
    setTestResult(null);
  }, [settings?.wrapper_account_url]);

  /** Handles the "Test Connection" button click */
  const handleTestConnection = async () => {
    if (!settings?.wrapper_account_url) return;
    setTestState('testing');
    try {
      const result = await testWrapperConnection(settings.wrapper_account_url);
      setTestResult(result);
      setTestState(result.reachable ? 'success' : 'error');
    } catch {
      setTestResult(null);
      setTestState('error');
    }
  };

  return (
    <div className="space-y-6 max-w-xl">
      {/* Section: Processing */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Processing
        </h3>

        {/* Download mode */}
        <Select
          label="Download Mode"
          description="Which tool to use for downloading HLS streams"
          options={DOWNLOAD_MODE_OPTIONS}
          value={settings.download_mode}
          onChange={(e) =>
            updateSettings({ download_mode: e.target.value as DownloadMode })
          }
        />

        {/* Remux mode */}
        <Select
          label="Remux Mode"
          description="Which tool to use for container format conversion"
          options={REMUX_MODE_OPTIONS}
          value={settings.remux_mode}
          onChange={(e) =>
            updateSettings({ remux_mode: e.target.value as RemuxMode })
          }
        />
      </div>

      {/* Section: Wrapper */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Wrapper
        </h3>

        {!supportsWrapper && (
          <p className="text-xs text-content-tertiary bg-surface-secondary rounded-platform p-2">
            The Wrapper service only provides native binaries for Linux x86_64.
            On this platform you would need to run Wrapper remotely (e.g. on a
            Linux server or via Docker) and point the URL below to it. See Help
            &gt; Wrapper / AMdecrypt for details.
          </p>
        )}

        <Toggle
          label="Use Wrapper"
          description="Use a locally-running wrapper service for authentication instead of browser cookies. The wrapper handles Apple ID login and DRM key exchange, providing more reliable access to Dolby Atmos and other protected formats. Most users should leave this disabled and use cookies instead."
          checked={settings.use_wrapper}
          onChange={(checked) => updateSettings({ use_wrapper: checked })}
          helpTopic="wrapper"
        />

        {settings.use_wrapper && (
          <>
            <Input
              label="Wrapper Account URL"
              description="URL of your locally-running wrapper service. The default (http://127.0.0.1:30020) works if the wrapper is running on your machine with default settings. See Help > Wrapper / AMdecrypt for setup instructions."
              value={settings.wrapper_account_url}
              onChange={(e) =>
                updateSettings({ wrapper_account_url: e.target.value })
              }
            />

            {/* Test Connection button with inline result display */}
            <div className="flex items-center gap-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={handleTestConnection}
                disabled={
                  testState === 'testing' || !settings.wrapper_account_url
                }
              >
                {testState === 'testing' ? 'Testing...' : 'Test Connection'}
              </Button>
              {testState === 'success' && testResult && (
                <span className="text-xs text-status-success">
                  Connected ({testResult.response_time_ms}ms)
                </span>
              )}
              {testState === 'error' && (
                <span className="text-xs text-status-error">
                  {testResult?.error || 'Connection failed'}
                </span>
              )}
            </div>
          </>
        )}
      </div>

      {/* Section: File Options */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          File Options
        </h3>

        {/* Filename truncation -- nullable number field.
            When the input is empty, `truncate` is set to `null` (no limit).
            When the input has a value, it is parsed to an integer. The
            nullish coalescing operator (`?? ''`) converts `null` to an
            empty string for the input's display value. */}
        <Input
          label="Truncate Filenames"
          description="Maximum filename length (leave empty for no limit)"
          type="number"
          min={10}
          max={255}
          value={settings.truncate?.toString() ?? ''} /* null -> '' for display */
          placeholder="No limit"
          onChange={(e) => {
            const val = e.target.value;
            updateSettings({
              truncate: val ? parseInt(val, 10) : null, // Empty string -> null (no limit)
            });
          }}
        />

        {/* Excluded tags -- comma-separated string <-> string[] conversion.
            The display value joins the array with ', ' for readability.
            The onChange handler splits on commas, trims whitespace from
            each segment, and filters out empty strings to avoid storing
            blank entries in the array. */}
        <Input
          label="Excluded Tags"
          description="Comma-separated list of metadata tags to exclude from downloaded files"
          value={settings.exclude_tags.join(', ')} /* string[] -> display string */
          placeholder="e.g., lyrics, comment"
          onChange={(e) => {
            const tags = e.target.value
              .split(',')             // Split on commas into segments
              .map((t) => t.trim())   // Trim whitespace from each segment
              .filter(Boolean);       // Remove empty strings (e.g., trailing comma)
            updateSettings({ exclude_tags: tags }); // Persist as string[]
          }}
        />
      </div>
    </div>
  );
}
