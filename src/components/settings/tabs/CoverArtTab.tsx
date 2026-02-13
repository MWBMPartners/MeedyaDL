/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file CoverArtTab.tsx -- Cover art preferences settings tab.
 *
 * Renders the "Cover Art" tab within the {@link SettingsPage} component.
 * This tab controls whether album cover art is saved as a separate file
 * alongside downloaded tracks, and if so, what format and size to use:
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
 *     art image. Valid range: 100-3000. Maps to `settings.cover_size` and
 *     GAMDL's `--cover-size` flag.
 *
 * ## Conditional Rendering
 *
 * The format and size controls are only rendered when `settings.save_cover`
 * is true. This uses a conditional JSX block (`{settings.save_cover && ...}`)
 * to keep the UI clean when cover art saving is disabled.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore`.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 * @see {@link @/types/index.ts}           -- CoverFormat type definition
 */

// Zustand store for reading/writing cover art settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared form components: Select for format dropdown, Toggle for the save switch,
// Input for the size number field.
import { Select, Toggle, Input } from '@/components/common';

// TypeScript union type for cover format values.
import type { CoverFormat } from '@/types';

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
 * CoverArtTab -- Renders the Cover Art settings tab.
 *
 * Contains a single visual section ("Cover Art") with a toggle and two
 * conditional controls (format select, size input). The format and size
 * controls are only displayed when `save_cover` is enabled.
 */
export function CoverArtTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting cover art setting changes */
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="space-y-6 max-w-xl">
      <div>
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Cover Art
        </h3>

        <div className="space-y-4">
          {/* Save cover art */}
          <Toggle
            label="Save Cover Art"
            description="Download and save album cover art as a separate file"
            checked={settings.save_cover}
            onChange={(checked) => updateSettings({ save_cover: checked })}
          />

          {/* Cover format (only shown when save_cover is enabled) */}
          {settings.save_cover && (
            <>
              <Select
                label="Cover Format"
                description="Image format for saved cover art files"
                options={COVER_FORMAT_OPTIONS}
                value={settings.cover_format}
                onChange={(e) =>
                  updateSettings({
                    cover_format: e.target.value as CoverFormat,
                  })
                }
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
                description="Width and height of the cover art image (max 3000)"
                type="number"
                min={100}
                max={3000}
                step={100}
                value={settings.cover_size.toString()} /* Convert number to string for the input value */
                onChange={(e) => {
                  const size = parseInt(e.target.value, 10); // Parse the input string to a base-10 integer
                  if (!isNaN(size) && size >= 100 && size <= 3000) { // Validate within acceptable range
                    updateSettings({ cover_size: size }); // Only persist valid values
                  }
                }}
              />
            </>
          )}
        </div>
      </div>
    </div>
  );
}
