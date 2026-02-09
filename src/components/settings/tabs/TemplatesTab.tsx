/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Templates settings tab.
 * Configures file and folder naming templates used by GAMDL when
 * organizing downloaded files. Supports GAMDL's template variables.
 */

import { useSettingsStore } from '@/stores/settingsStore';
import { Input } from '@/components/common';

/**
 * Renders the Templates settings tab with input fields for all
 * folder and file naming templates.
 */
export function TemplatesTab() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="space-y-6 max-w-xl">
      {/* Template variable reference */}
      <div className="p-4 rounded-platform border border-border-light bg-surface-elevated">
        <h4 className="text-xs font-semibold text-content-primary mb-2">
          Available Template Variables
        </h4>
        <div className="text-xs text-content-secondary font-mono space-y-0.5">
          <p>
            {'{artist}'}, {'{album_artist}'}, {'{album}'}, {'{title}'}
          </p>
          <p>
            {'{track:02d}'}, {'{disc}'}, {'{year}'}, {'{genre}'}
          </p>
          <p>
            {'{playlist_artist}'}, {'{playlist_title}'}
          </p>
        </div>
      </div>

      {/* Section: Folder Templates */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-content-primary">
          Folder Templates
        </h3>

        <Input
          label="Album Folder"
          description="Folder structure for regular album downloads"
          value={settings.album_folder_template}
          onChange={(e) =>
            updateSettings({ album_folder_template: e.target.value })
          }
        />

        <Input
          label="Compilation Folder"
          description="Folder structure for compilation/various artist albums"
          value={settings.compilation_folder_template}
          onChange={(e) =>
            updateSettings({ compilation_folder_template: e.target.value })
          }
        />

        <Input
          label="No Album Folder"
          description="Folder structure when album information is not available"
          value={settings.no_album_folder_template}
          onChange={(e) =>
            updateSettings({ no_album_folder_template: e.target.value })
          }
        />
      </div>

      {/* Section: File Templates */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-content-primary">
          File Templates
        </h3>

        <Input
          label="Single Disc File"
          description="Filename template for tracks from single-disc albums"
          value={settings.single_disc_file_template}
          onChange={(e) =>
            updateSettings({ single_disc_file_template: e.target.value })
          }
        />

        <Input
          label="Multi Disc File"
          description="Filename template for tracks from multi-disc albums"
          value={settings.multi_disc_file_template}
          onChange={(e) =>
            updateSettings({ multi_disc_file_template: e.target.value })
          }
        />

        <Input
          label="No Album File"
          description="Filename template when album information is not available"
          value={settings.no_album_file_template}
          onChange={(e) =>
            updateSettings({ no_album_file_template: e.target.value })
          }
        />

        <Input
          label="Playlist File"
          description="Filename template for playlist downloads"
          value={settings.playlist_file_template}
          onChange={(e) =>
            updateSettings({ playlist_file_template: e.target.value })
          }
        />
      </div>
    </div>
  );
}
