/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file MetadataTab.tsx -- Metadata enrichment settings tab.
 *
 * Renders the "Metadata" tab within the {@link SettingsPage} component.
 * This tab configures post-download metadata enrichment features:
 *
 *   - **Automatic Tags** -- Always-on metadata enrichment (codec tags,
 *     source tags, channel configuration), plus a toggle to control
 *     whether extra API metadata (normalization, spatial properties) is
 *     fetched. Maps to `settings.fetch_extra_tags`.
 *
 *   - **AcoustID Fingerprinting** (opt-in) -- When enabled, generates
 *     Chromaprint audio fingerprints using the embedded fingerprinting engine and looks up AcoustID
 *     identifiers from acoustid.org. Writes `Acoustid Id` and
 *     `Acoustid Fingerprint` freeform atoms. Maps to
 *     `settings.acoustid_enabled`. Release builds ship with a built-in
 *     API key; users can optionally override it with their own.
 *
 *   - **ReplayGain Analysis** (opt-in) -- When enabled, analyses audio
 *     loudness using FFmpeg's EBU R128 filter and writes non-destructive
 *     ReplayGain metadata tags (`replaygain_track_gain`,
 *     `replaygain_track_peak`). Maps to `settings.replaygain_enabled`.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore`.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 */

// Zustand store for reading/writing metadata enrichment settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared form components: Toggle for boolean switches.
import { Toggle } from '@/components/common';

/**
 * MetadataTab -- Renders the Metadata settings tab.
 *
 * Contains three sections: an informational block about automatic tags,
 * an AcoustID toggle, and a ReplayGain toggle. The automatic tags
 * section is read-only (no controls) since those tags are always written
 * when applicable.
 */
export function MetadataTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting metadata setting changes */
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="space-y-6 max-w-xl">
      {/* ================================================================
          Section 1: Automatic Tags
          ================================================================ */}
      <div>
        <h3 className="text-base font-semibold text-content-primary mb-2">Automatic Tags</h3>
        <p className="text-sm text-content-secondary leading-relaxed mb-2">
          MeedyaDL automatically enriches downloaded files with metadata after every download. Codec
          tags (lossless, spatial audio), source tags, and channel configuration are always written.
        </p>
        <p className="text-sm text-content-tertiary leading-relaxed mb-4">
          API-derived tags (ISRC, UPC, genre, advisory ratings, artist IDs) require MusicKit
          credentials. Configure them in Settings &gt; Advanced &gt; API Credentials.
        </p>

        <Toggle
          label="Fetch Extra Tags"
          description="Fetch additional metadata from Apple Music (normalization, spatial/lossless properties, smooth playback info). Adds a small delay per track. Disable if you only want basic codec/source tags."
          checked={settings.fetch_extra_tags}
          onChange={(checked) => updateSettings({ fetch_extra_tags: checked })}
        />

        <Toggle
          label="Content Advisory in Filenames"
          description="Append [Explicit] or [Clean] to album folder names and track filenames based on Apple Music content ratings. Useful for distinguishing explicit and clean versions of the same album."
          checked={settings.content_advisory_in_filenames}
          onChange={(checked) => updateSettings({ content_advisory_in_filenames: checked })}
        />
      </div>

      {/* ================================================================
          Section 2: AcoustID Fingerprinting (opt-in)
          ================================================================ */}
      <div>
        <h3 className="text-base font-semibold text-content-primary mb-4">
          AcoustID Fingerprinting
        </h3>

        <div className="space-y-4">
          <Toggle
            label="Enable AcoustID Fingerprinting"
            description="Generate audio fingerprints and look up AcoustID identifiers for each track. Enables music identification via MusicBrainz. Processes each file individually."
            checked={settings.acoustid_enabled}
            onChange={(checked) => updateSettings({ acoustid_enabled: checked })}
          />

          {settings.acoustid_enabled && (
            <p className="text-xs text-content-tertiary">
              Release builds include a built-in API key. To use your own key, configure it in
              Settings &gt; Advanced &gt; API Credentials.
            </p>
          )}
        </div>
      </div>

      {/* ================================================================
          Section 3: ReplayGain Analysis (opt-in)
          ================================================================ */}
      <div>
        <h3 className="text-base font-semibold text-content-primary mb-4">ReplayGain Analysis</h3>

        <div className="space-y-4">
          <Toggle
            label="Enable ReplayGain Analysis"
            description="Analyse audio loudness and embed non-destructive ReplayGain metadata for volume normalisation. Uses FFmpeg (already installed). Analyses each file individually."
            checked={settings.replaygain_enabled}
            onChange={(checked) => updateSettings({ replaygain_enabled: checked })}
          />
        </div>
      </div>

    </div>
  );
}
