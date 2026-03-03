/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file LyricsTab.tsx -- Lyrics format preferences settings tab.
 *
 * Renders the "Lyrics" tab within the {@link SettingsPage} component.
 * This tab configures how GAMDL handles time-synced lyrics:
 *
 *   - **Synced Lyrics Formats** -- Multi-select checkboxes for the file
 *     formats to download. The first checked format (in LRC -> SRT -> TTML
 *     order) becomes the primary format (downloaded with the audio). Any
 *     additional checked formats are downloaded as lightweight
 *     `--synced-lyrics-only` companion passes after the primary completes.
 *     Maps to `settings.synced_lyrics_format` (primary) and
 *     `settings.companion_lyrics_formats` (additional).
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

// Shared form component: Toggle for boolean switches.
import { Toggle } from '@/components/common';

// TypeScript union type for the lyrics format values.
import type { LyricsFormat } from '@/types';

/**
 * Canonical order of lyrics formats for checkbox display and primary
 * selection. The first checked format in this order becomes the primary.
 */
const LYRICS_FORMAT_OPTIONS: { value: LyricsFormat; label: string }[] = [
  { value: 'lrc', label: 'LRC (standard lyrics format)' },
  { value: 'srt', label: 'SRT (SubRip subtitle format)' },
  { value: 'ttml', label: 'TTML (Timed Text Markup Language)' },
];

/**
 * LyricsTab -- Renders the Lyrics settings tab.
 *
 * Contains a single visual section ("Synced Lyrics") with format
 * checkboxes and two toggles. All controls read from and write to
 * the shared Zustand settings store.
 */
export function LyricsTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting lyrics setting changes */
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  /**
   * Set of all currently selected lyrics formats (primary + companions).
   * Used to derive the checked state of each checkbox.
   */
  const checkedFormats = new Set<LyricsFormat>([
    settings.synced_lyrics_format,
    ...settings.companion_lyrics_formats,
  ]);

  /** Whether lyrics format checkboxes should be disabled */
  const formatsDisabled = settings.no_synced_lyrics && !settings.embed_lyrics_and_sidecar;

  /**
   * Handles toggling a lyrics format checkbox on or off.
   * Recomputes the primary format (first checked in canonical order)
   * and companion formats (remaining checked formats).
   */
  function handleFormatToggle(format: LyricsFormat, checked: boolean) {
    const current = new Set(checkedFormats);

    if (checked) {
      current.add(format);
    } else {
      /* Prevent unchecking the last remaining format */
      if (current.size <= 1) return;
      current.delete(format);
    }

    // When Enhanced LRC is on, TTML is always the primary (forced by
    // merge_options Layer 4 in Rust). Ensure TTML stays in the set and
    // is always first. Other selected formats become companions.
    if (settings.enhanced_lrc) {
      current.add('ttml'); // Ensure TTML is always present
      const companions = LYRICS_FORMAT_OPTIONS.map((f) => f.value)
        .filter((f) => f !== 'ttml' && current.has(f));
      updateSettings({
        synced_lyrics_format: 'ttml',
        companion_lyrics_formats: companions,
      });
      return;
    }

    /* Recompute primary and companions based on canonical order */
    const ordered = LYRICS_FORMAT_OPTIONS.map((f) => f.value).filter((f) => current.has(f));

    updateSettings({
      synced_lyrics_format: ordered[0],
      companion_lyrics_formats: ordered.slice(1),
    });
  }

  return (
    <div className="space-y-6 max-w-xl">
      <div>
        <h3 className="text-base font-semibold text-content-primary mb-4">Synced Lyrics</h3>

        <div className="space-y-4">
          {/* Enhanced LRC (word-by-word sync) */}
          <Toggle
            label="Enhanced Lyrics (Word-by-Word Sync)"
            description="Automatically converts Apple Music's TTML lyrics to Enhanced LRC with word-by-word synchronized timestamps. Creates a .lrc sidecar file and embeds in audio metadata. Falls back to line-level sync for songs without word-level data."
            checked={settings.enhanced_lrc}
            onChange={(checked) => updateSettings({ enhanced_lrc: checked })}
          />

          {/* Lyrics format fallback */}
          <Toggle
            label="Lyrics Format Fallback"
            description="When the primary lyrics format isn't available for some tracks, automatically try alternative formats. Audio: TTML → LRC → SRT. Video: TTML → SRT → LRC."
            checked={settings.lyrics_fallback_enabled}
            onChange={(checked) => updateSettings({ lyrics_fallback_enabled: checked })}
          />

          {/* WebVTT subtitle generation */}
          <Toggle
            label="Generate WebVTT Subtitles"
            description="Create .vtt subtitle files from downloaded lyrics (TTML, SRT, or LRC). WebVTT is the standard format for web-based video players."
            checked={settings.generate_webvtt}
            onChange={(checked) => updateSettings({ generate_webvtt: checked })}
          />

          {/* Rich SRT generation */}
          <Toggle
            label="Generate Rich SRT from TTML"
            description="Creates SRT subtitle files with formatting (bold, italic, underline, colours) from Apple Music TTML. Replaces plain SRT files with styled versions when TTML is available."
            checked={settings.generate_rich_srt}
            onChange={(checked) => updateSettings({ generate_rich_srt: checked })}
          />

          {/* Subtitle embedding */}
          <Toggle
            label="Embed Subtitles in Media Files"
            description="Embed SRT and WebVTT subtitle content directly into MP4/M4A/M4V containers as metadata. Useful for players that read embedded subtitles."
            checked={settings.embed_subtitles}
            onChange={(checked) => updateSettings({ embed_subtitles: checked })}
          />

          {/* ASS subtitle generation */}
          <Toggle
            label="Generate ASS Subtitles"
            description="Create Advanced SubStation Alpha (.ass) subtitle files from TTML or WebVTT with rich styling: colours, bold, italic, positioning, and background vocal styles. Supported by VLC, mpv, and MPC-HC."
            checked={settings.generate_ass}
            onChange={(checked) => updateSettings({ generate_ass: checked })}
          />

          {/* Embed lyrics + keep sidecar */}
          <Toggle
            label="Embed Lyrics and Keep Sidecar"
            description="Embed lyrics in audio file metadata and also save a separate lyrics file for maximum player compatibility"
            checked={settings.embed_lyrics_and_sidecar}
            onChange={(checked) => updateSettings({ embed_lyrics_and_sidecar: checked })}
          />

          {/* Synced lyrics format checkboxes */}
          <div className={formatsDisabled ? 'opacity-50' : ''}>
            <span className="text-sm font-medium text-content-primary">Synced Lyrics Formats</span>
            <span className="block text-xs text-content-tertiary mt-0.5 mb-2">
              {settings.enhanced_lrc
                ? 'TTML is the primary format (required for Enhanced Lyrics). Select additional formats to download as companions alongside TTML.'
                : 'Select one or more formats. The first format is used with the primary download; additional formats are downloaded as lightweight companion passes.'}
            </span>
            <div className="space-y-2">
              {LYRICS_FORMAT_OPTIONS.map(({ value, label }) => {
                const isChecked = checkedFormats.has(value);
                // When Enhanced LRC is on, TTML is always primary (forced by merge_options Layer 4).
                // Show "(Primary)" for TTML when enhanced_lrc is on, otherwise use the normal logic.
                const isPrimary = settings.enhanced_lrc
                  ? value === 'ttml'
                  : value === settings.synced_lyrics_format && checkedFormats.size > 1;
                // TTML cannot be unchecked when Enhanced LRC is on (it's the required primary).
                const isLocked = settings.enhanced_lrc && value === 'ttml';

                return (
                  <label
                    key={value}
                    className={`flex items-center gap-2.5 ${formatsDisabled || isLocked ? 'cursor-not-allowed' : 'cursor-pointer'}`}
                  >
                    <input
                      type="checkbox"
                      checked={settings.enhanced_lrc ? (value === 'ttml' || checkedFormats.has(value)) : isChecked}
                      disabled={formatsDisabled || isLocked}
                      onChange={(e) => handleFormatToggle(value, e.target.checked)}
                      className="h-4 w-4 rounded border-border accent-[var(--color-accent)] cursor-pointer disabled:cursor-not-allowed"
                    />
                    <span className="text-sm text-content-primary">{label}</span>
                    {isPrimary && <span className="text-xs text-content-tertiary">(Primary)</span>}
                  </label>
                );
              })}
            </div>
          </div>

          {/* Disable synced lyrics -- overridden when embed+sidecar is on */}
          <Toggle
            label="Disable Synced Lyrics"
            description={
              settings.embed_lyrics_and_sidecar
                ? 'Overridden by "Embed Lyrics and Keep Sidecar" above'
                : "Don't download synced lyrics files alongside tracks"
            }
            checked={settings.no_synced_lyrics}
            onChange={(checked) => updateSettings({ no_synced_lyrics: checked })}
            disabled={settings.embed_lyrics_and_sidecar}
          />

          {/* Synced lyrics only */}
          <Toggle
            label="Synced Lyrics Only"
            description="Only download synced lyrics without downloading the audio/video"
            checked={settings.synced_lyrics_only}
            onChange={(checked) => updateSettings({ synced_lyrics_only: checked })}
          />
        </div>
      </div>
    </div>
  );
}
