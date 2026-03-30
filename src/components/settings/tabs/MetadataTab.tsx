/**
 * Copyright (c) 2026 MeedyaDL
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
import { Toggle, SettingsSection } from '@/components/common';

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
    <div className="space-y-3 max-w-xl">
      {/* ================================================================
          Section 1: Automatic Tags
          ================================================================ */}
      <SettingsSection
        title="Automatic Tags"
        description="MeedyaDL automatically enriches downloaded files with metadata after every download. Codec tags (lossless, spatial audio), source tags, and channel configuration are always written."
      >
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
      </SettingsSection>

      {/* ================================================================
          Section 2: AcoustID Fingerprinting (opt-in)
          ================================================================ */}
      <SettingsSection title="AcoustID Fingerprinting">
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
      </SettingsSection>

      {/* ================================================================
          Section 3: ReplayGain Analysis (opt-in)
          ================================================================ */}
      <SettingsSection title="ReplayGain Analysis">
          <Toggle
            label="Enable ReplayGain Analysis"
            description="Analyse audio loudness and embed non-destructive ReplayGain metadata for volume normalisation. Calculates both per-track and per-album gain. Uses FFmpeg (already installed)."
            checked={settings.replaygain_enabled}
            onChange={(checked) => updateSettings({ replaygain_enabled: checked })}
          />

          {settings.replaygain_enabled && (
            <>
              <div className="mt-4">
                <label className="block text-sm font-medium text-content-primary mb-1">
                  Reference Level
                </label>
                <select
                  className="w-full rounded-platform border border-border-light bg-surface-elevated px-3 py-2 text-sm text-content-primary"
                  value={settings.replaygain_reference_level}
                  onChange={(e) =>
                    updateSettings({ replaygain_reference_level: parseFloat(e.target.value) })
                  }
                >
                  <option value={-18}>-18.0 LUFS (EBU R128 — recommended for music)</option>
                  <option value={-14}>-14.0 LUFS (Spotify / YouTube style — louder)</option>
                  <option value={-23}>-23.0 LUFS (EBU R128 broadcast — conservative)</option>
                  <option value={-16}>-16.0 LUFS (Apple Music / iTunes)</option>
                </select>
                <p className="text-xs text-content-tertiary mt-1">
                  Target loudness level. Players adjust volume so all tracks reach this level. Lower values = quieter target with more headroom.
                </p>
              </div>

              <Toggle
                label="Prevent Clipping"
                description="Limit gain so peak × gain never exceeds 0 dBFS. Prevents digital distortion on tracks that are already mastered near maximum loudness."
                checked={settings.replaygain_prevent_clipping}
                onChange={(checked) => updateSettings({ replaygain_prevent_clipping: checked })}
              />
            </>
          )}
      </SettingsSection>

    </div>
  );
}
