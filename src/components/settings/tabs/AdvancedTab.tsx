/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Advanced settings tab.
 * Configures advanced options: download mode, remux mode, wrapper settings,
 * filename truncation, and excluded tags.
 */

import { useSettingsStore } from '@/stores/settingsStore';
import { Select, Toggle, Input } from '@/components/common';
import type { DownloadMode, RemuxMode } from '@/types';

/** Download mode options */
const DOWNLOAD_MODE_OPTIONS = [
  { value: 'ytdlp', label: 'yt-dlp (recommended)' },
  { value: 'nm3u8dlre', label: 'N_m3u8DL-RE (alternative)' },
];

/** Remux mode options */
const REMUX_MODE_OPTIONS = [
  { value: 'ffmpeg', label: 'FFmpeg (recommended)' },
  { value: 'mp4box', label: 'MP4Box (alternative)' },
];

/**
 * Renders the Advanced settings tab with download/remux mode selection,
 * wrapper configuration, and other advanced options.
 */
export function AdvancedTab() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

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

        <Toggle
          label="Use Wrapper"
          description="Use a wrapper service for account authentication instead of cookies"
          checked={settings.use_wrapper}
          onChange={(checked) => updateSettings({ use_wrapper: checked })}
        />

        {settings.use_wrapper && (
          <Input
            label="Wrapper Account URL"
            description="URL of the wrapper service endpoint"
            value={settings.wrapper_account_url}
            onChange={(e) =>
              updateSettings({ wrapper_account_url: e.target.value })
            }
          />
        )}
      </div>

      {/* Section: File Options */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          File Options
        </h3>

        {/* Filename truncation */}
        <Input
          label="Truncate Filenames"
          description="Maximum filename length (leave empty for no limit)"
          type="number"
          min={10}
          max={255}
          value={settings.truncate?.toString() ?? ''}
          placeholder="No limit"
          onChange={(e) => {
            const val = e.target.value;
            updateSettings({
              truncate: val ? parseInt(val, 10) : null,
            });
          }}
        />

        {/* Excluded tags */}
        <Input
          label="Excluded Tags"
          description="Comma-separated list of metadata tags to exclude from downloaded files"
          value={settings.exclude_tags.join(', ')}
          placeholder="e.g., lyrics, comment"
          onChange={(e) => {
            const tags = e.target.value
              .split(',')
              .map((t) => t.trim())
              .filter(Boolean);
            updateSettings({ exclude_tags: tags });
          }}
        />
      </div>
    </div>
  );
}
