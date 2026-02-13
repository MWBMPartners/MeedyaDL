/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file LyricsTab.tsx -- Lyrics format preferences settings tab.
 *
 * Renders the "Lyrics" tab within the {@link SettingsPage} component.
 * This tab configures how GAMDL handles time-synced lyrics:
 *
 *   - **Synced Lyrics Format** -- The file format for downloaded lyrics
 *     files. Maps to `settings.synced_lyrics_format` and GAMDL's
 *     `--synced-lyrics-format` flag.
 *     Supported formats:
 *       - LRC -- Standard lyrics format widely supported by music players
 *       - SRT -- SubRip subtitle format (used by video players)
 *       - TTML -- Timed Text Markup Language (Apple's native format)
 *
 *   - **Disable Synced Lyrics** -- When enabled, synced lyrics files are
 *     not downloaded alongside audio tracks. Maps to
 *     `settings.no_synced_lyrics` and GAMDL's `--no-synced-lyrics` flag.
 *
 *   - **Synced Lyrics Only** -- When enabled, GAMDL downloads only the
 *     synced lyrics without downloading audio/video. Maps to
 *     `settings.synced_lyrics_only` and GAMDL's `--synced-lyrics-only` flag.
 *     Note: This is mutually exclusive with "Disable Synced Lyrics" in
 *     practice, though the UI does not enforce this constraint.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore`.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 * @see {@link @/types/index.ts}           -- LyricsFormat type definition
 */

// Zustand store for reading/writing lyrics settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared form components: Select for the format dropdown, Toggle for boolean switches.
import { Select, Toggle } from '@/components/common';

// TypeScript union type for the lyrics format values.
import type { LyricsFormat } from '@/types';

/**
 * Dropdown options for the synced lyrics format selector.
 * Each entry provides the value stored in settings and a descriptive label.
 */
const LYRICS_FORMAT_OPTIONS = [
  { value: 'lrc', label: 'LRC (standard lyrics format)' },
  { value: 'srt', label: 'SRT (SubRip subtitle format)' },
  { value: 'ttml', label: 'TTML (Timed Text Markup Language)' },
];

/**
 * LyricsTab -- Renders the Lyrics settings tab.
 *
 * Contains a single visual section ("Synced Lyrics") with three controls:
 * a format dropdown and two toggles. All controls read from and write to
 * the shared Zustand settings store.
 */
export function LyricsTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting lyrics setting changes */
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="space-y-6 max-w-xl">
      <div>
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Synced Lyrics
        </h3>

        <div className="space-y-4">
          {/* Embed lyrics + keep sidecar */}
          <Toggle
            label="Embed Lyrics and Keep Sidecar"
            description="Embed lyrics in audio file metadata and also save a separate lyrics file for maximum player compatibility"
            checked={settings.embed_lyrics_and_sidecar}
            onChange={(checked) =>
              updateSettings({ embed_lyrics_and_sidecar: checked })
            }
          />

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

          {/* Disable synced lyrics -- overridden when embed+sidecar is on */}
          <Toggle
            label="Disable Synced Lyrics"
            description={
              settings.embed_lyrics_and_sidecar
                ? 'Overridden by "Embed Lyrics and Keep Sidecar" above'
                : "Don't download synced lyrics files alongside tracks"
            }
            checked={settings.no_synced_lyrics}
            onChange={(checked) =>
              updateSettings({ no_synced_lyrics: checked })
            }
            disabled={settings.embed_lyrics_and_sidecar}
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
