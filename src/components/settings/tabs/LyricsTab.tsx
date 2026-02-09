/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Lyrics settings tab.
 * Configures synced lyrics format, whether to download synced lyrics,
 * and synced-lyrics-only mode.
 */

import { useSettingsStore } from '@/stores/settingsStore';
import { Select, Toggle } from '@/components/common';
import type { LyricsFormat } from '@/types';

/** Available synced lyrics format options */
const LYRICS_FORMAT_OPTIONS = [
  { value: 'lrc', label: 'LRC (standard lyrics format)' },
  { value: 'srt', label: 'SRT (SubRip subtitle format)' },
  { value: 'ttml', label: 'TTML (Timed Text Markup Language)' },
];

/**
 * Renders the Lyrics settings tab with format selection and
 * download behavior toggles.
 */
export function LyricsTab() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="space-y-6 max-w-xl">
      <div>
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Synced Lyrics
        </h3>

        <div className="space-y-4">
          {/* Synced lyrics format */}
          <Select
            label="Synced Lyrics Format"
            description="Format for downloading time-synced lyrics files"
            options={LYRICS_FORMAT_OPTIONS}
            value={settings.synced_lyrics_format}
            onChange={(e) =>
              updateSettings({
                synced_lyrics_format: e.target.value as LyricsFormat,
              })
            }
          />

          {/* Disable synced lyrics */}
          <Toggle
            label="Disable Synced Lyrics"
            description="Don't download synced lyrics files alongside tracks"
            checked={settings.no_synced_lyrics}
            onChange={(checked) =>
              updateSettings({ no_synced_lyrics: checked })
            }
          />

          {/* Synced lyrics only */}
          <Toggle
            label="Synced Lyrics Only"
            description="Only download synced lyrics without downloading the audio/video"
            checked={settings.synced_lyrics_only}
            onChange={(checked) =>
              updateSettings({ synced_lyrics_only: checked })
            }
          />
        </div>
      </div>
    </div>
  );
}
