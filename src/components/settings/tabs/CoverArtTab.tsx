/**
 * Copyright (c) 2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file CoverArtTab.tsx -- Cover art preferences settings tab.
 *
 * Renders the "Cover Art" tab within the {@link SettingsPage} component.
 * This tab controls two main areas:
 *
 * ## Static Cover Art (GAMDL)
 *
 *   - **Save Cover Art** -- Toggle that enables/disables cover art saving.
 *     Maps to `settings.save_cover` and GAMDL's `--save-cover` flag.
 *     When disabled, the format and size controls are hidden.
 *
 *   - **Cover Format** -- The image format for saved cover art files:
 *       - Raw: Original format from Apple Music (typically JPEG)
 *       - JPEG: Compressed, smaller file size
 *       - PNG: Lossless, larger file size
 *     Maps to `settings.cover_format` and GAMDL's `--cover-format` flag.
 *
 *   - **Cover Size** -- Width and height in pixels (square) for the cover
 *     art image. Valid range: 100-10000. Maps to `settings.cover_size` and
 *     GAMDL's `--cover-size` flag.
 *
 * ## Animated Artwork (MusicKit API)
 *
 *   - **Download Animated Cover Art** -- Toggle to enable/disable automatic
 *     downloading of animated (motion) cover art after album downloads.
 *     Maps to `settings.animated_artwork_enabled`.
 *
 *   - **Hide Animated Artwork Files** -- Toggle to set the OS hidden attribute
 *     on downloaded artwork files. Only shown when animated artwork is enabled.
 *
 *   MusicKit credentials (Team ID, Key ID, Private Key) are configured in
 *   the Advanced tab's API Credentials section.
 *
 * ## Conditional Rendering
 *
 * Format/size controls are shown only when `save_cover` is true.
 * Animated artwork sub-options are shown only when `animated_artwork_enabled`
 * is true.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore`.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 * @see {@link @/types/index.ts}           -- CoverFormat type definition
 */

// Audit v2 #6 — per-field Zustand binding.
import { useSettingsField } from '@/hooks/useSettingsField';
import { useUiStore } from '@/stores/uiStore';

// Shared form components: Select for format dropdown, Toggle for the save switch,
// Input for the size number field.
import { Select, Toggle, Input, SettingsSection } from '@/components/common';

// TypeScript union types for cover settings.
import type { CoverFormat, CoverArtName } from '@/types';

/**
 * Dropdown options for the cover art format selector.
 * "raw" preserves the original format served by Apple Music's CDN.
 */
const COVER_FORMAT_OPTIONS = [
  { value: 'raw', label: 'Raw (original format from Apple Music)' },
  { value: 'jpg', label: 'JPEG (compressed, smaller file size)' },
  { value: 'png', label: 'PNG (lossless, larger file size)' },
];

/**
 * Dropdown options for the cover art filename selector.
 * Controls what GAMDL's default "Cover" filename is renamed to after download.
 */
const COVER_ART_NAME_OPTIONS = [
  { value: 'front_cover', label: 'FrontCover (matches animated artwork naming)' },
  { value: 'cover', label: 'Cover (GAMDL default)' },
  { value: 'folder', label: 'Folder (Windows Media Player convention)' },
];

/**
 * CoverArtTab -- Renders the Cover Art settings tab.
 *
 * Contains two visual sections:
 * 1. "Cover Art" -- Static cover art settings (toggle, format, size)
 * 2. "Animated Artwork" -- Motion cover art settings (toggle, MusicKit credentials)
 */
export function CoverArtTab() {
  // Per-field Zustand bindings (audit v2 #6).
  const saveCover = useSettingsField('save_cover');
  const coverFormat = useSettingsField('cover_format');
  const coverSize = useSettingsField('cover_size');
  const coverArtName = useSettingsField('cover_art_name');
  const animatedEnabled = useSettingsField('animated_artwork_enabled');
  const hideAnimated = useSettingsField('hide_animated_artwork');
  const artistPromo = useSettingsField('artist_promo_video_enabled');
  // #533 / #569: embed MV cover sidecar into MP4 + delete sidecar.
  const mvEmbedCoverSidecar = useSettingsField('music_video_embed_cover_sidecar');
  /** Navigate to a help topic (for the "Animated Artwork help page" link) */
  const navigateToHelp = useUiStore((s) => s.navigateToHelp);

  return (
    <div className="space-y-3">
      {/* ============================================================ */}
      {/* Section 1: Static Cover Art (GAMDL) */}
      {/* ============================================================ */}
      <SettingsSection title="Cover Art">
          {/* Save cover art */}
          <Toggle
            label="Save Cover Art"
            description="Download and save album cover art as a separate file"
            checked={saveCover.value}
            onChange={saveCover.set}
          />

          {/* Cover format (only shown when save_cover is enabled) */}
          {saveCover.value && (
            <>
              <Select
                label="Cover Format"
                description="Image format for saved cover art files"
                options={COVER_FORMAT_OPTIONS}
                value={coverFormat.value}
                onChange={(e) => coverFormat.set(e.target.value as CoverFormat)}
              />

              {/* Cover size -- numeric input with client-side validation.
                  The onChange handler parses the string to an integer and
                  only persists the value if it falls within the valid range
                  (100-3000 pixels). This prevents invalid values from
                  reaching the backend while still allowing the user to
                  type freely. The `step={100}` prop controls the increment
                  when using the browser's native spinner arrows. */}
              <Input
                label="Cover Size (pixels)"
                description="Width and height of the cover art image (max 10000)"
                type="number"
                min={100}
                max={10000}
                step={100}
                value={coverSize.value.toString()} /* Convert number to string for the input value */
                onChange={(e) => {
                  const size = parseInt(e.target.value, 10); // Parse the input string to a base-10 integer
                  if (!isNaN(size) && size >= 100 && size <= 10000) {
                    // Validate within acceptable range
                    coverSize.set(size); // Only persist valid values
                  }
                }}
              />

              {/* Cover art filename */}
              <Select
                label="Cover Art Filename"
                description="Filename for saved cover art images. GAMDL writes 'Cover' by default; this renames the file after download."
                options={COVER_ART_NAME_OPTIONS}
                value={coverArtName.value}
                onChange={(e) => coverArtName.set(e.target.value as CoverArtName)}
              />

              {/* #533 / #569: embed MV cover sidecar into MP4 + delete. */}
              <Toggle
                label="Embed Music Video Cover Thumbnail"
                description="Embed the music-video cover thumbnail into the MP4 as a poster atom and delete the sidecar .jpg/.png. Most modern players (VLC, mpv, QuickTime, Plex, Jellyfin) read the embedded poster directly, so the sidecar just clutters the library. When the embed fails for any reason, the sidecar is kept on disk and a warning is logged."
                checked={mvEmbedCoverSidecar.value}
                onChange={mvEmbedCoverSidecar.set}
              />
            </>
          )}
      </SettingsSection>

      {/* ============================================================ */}
      {/* Section 2: Animated Artwork (Apple MusicKit API) */}
      {/* ============================================================ */}
      <SettingsSection title="Animated Artwork">
          {/* Master toggle for animated artwork downloading */}
          <Toggle
            label="Download Animated Cover Art"
            description="Download animated (motion) cover art from Apple Music when available. Saves FrontCover.mp4 and FrontCoverPortrait.mp4 alongside album files."
            checked={animatedEnabled.value}
            onChange={animatedEnabled.set}
            helpTopic="settings-help"
          />

          {/* Hide animated artwork files toggle (only shown when enabled) */}
          {animatedEnabled.value && (
            <Toggle
              label="Hide Animated Artwork Files"
              description="Set the OS hidden attribute on FrontCover.mp4 and FrontCoverPortrait.mp4 to keep album folders clean. On macOS/Windows, files keep their original names. On Linux, files are renamed with a dot prefix."
              checked={hideAnimated.value}
              onChange={hideAnimated.set}
            />
          )}

          {/* Artist promo video toggle (only shown when animated artwork is enabled) */}
          {animatedEnabled.value && (
            <Toggle
              label="Download Artist Promo Video"
              description="Download the animated background video from the artist's Apple Music page and save it as ArtistCover.mp4 in the artist folder. Not all artists have a promo video. Skipped automatically if already downloaded."
              checked={artistPromo.value}
              onChange={artistPromo.set}
            />
          )}

          {/* MusicKit credentials note (credentials are in Settings > Advanced) */}
          {animatedEnabled.value && (
            <p className="text-xs text-content-secondary">
              Requires MusicKit credentials (Apple Developer account). Configure them in
              Settings &gt; Advanced &gt; API Credentials. See the{' '}
              <button
                type="button"
                className="text-accent underline cursor-pointer hover:opacity-80"
                onClick={() => navigateToHelp('animated-artwork')}
              >
                Animated Artwork help page
              </button>{' '}
              for setup instructions.
            </p>
          )}
      </SettingsSection>
    </div>
  );
}
