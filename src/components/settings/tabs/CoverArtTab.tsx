/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Cover Art settings tab.
 * Configures cover art saving, format, and size options.
 */

import { useSettingsStore } from '@/stores/settingsStore';
import { Select, Toggle, Input } from '@/components/common';
import type { CoverFormat } from '@/types';

/** Available cover art format options */
const COVER_FORMAT_OPTIONS = [
  { value: 'raw', label: 'Raw (original format from Apple Music)' },
  { value: 'jpg', label: 'JPEG (compressed, smaller file size)' },
  { value: 'png', label: 'PNG (lossless, larger file size)' },
];

/**
 * Renders the Cover Art settings tab with save toggle, format selection,
 * and size configuration.
 */
export function CoverArtTab() {
  const settings = useSettingsStore((s) => s.settings);
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

              {/* Cover size */}
              <Input
                label="Cover Size (pixels)"
                description="Width and height of the cover art image (max 3000)"
                type="number"
                min={100}
                max={3000}
                step={100}
                value={settings.cover_size.toString()}
                onChange={(e) => {
                  const size = parseInt(e.target.value, 10);
                  if (!isNaN(size) && size >= 100 && size <= 3000) {
                    updateSettings({ cover_size: size });
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
