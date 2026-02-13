/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file TemplatesTab.tsx -- File and folder naming template settings tab.
 *
 * Renders the "Templates" tab within the {@link SettingsPage} component.
 * GAMDL uses Python-style format strings to determine how downloaded files
 * and folders are named. This tab exposes all available template settings
 * with a reference card showing the available variables.
 *
 * ## Template Variables
 *
 * Templates use curly-brace placeholders that are replaced at download time:
 *
 * | Variable              | Description                              |
 * |-----------------------|------------------------------------------|
 * | `{artist}`            | Track artist name                        |
 * | `{album_artist}`      | Album artist name                        |
 * | `{album}`             | Album title                              |
 * | `{title}`             | Track title                              |
 * | `{track:02d}`         | Track number, zero-padded to 2 digits    |
 * | `{disc}`              | Disc number                              |
 * | `{year}`              | Release year                             |
 * | `{genre}`             | Genre                                    |
 * | `{playlist_artist}`   | Playlist curator name                    |
 * | `{playlist_title}`    | Playlist title                           |
 *
 * ## Settings Mapping
 *
 * | UI Label              | Setting Key                     | GAMDL Flag                      |
 * |-----------------------|---------------------------------|---------------------------------|
 * | Album Folder          | album_folder_template           | --album-folder-template         |
 * | Compilation Folder    | compilation_folder_template     | --compilation-folder-template   |
 * | No Album Folder       | no_album_folder_template        | --no-album-folder-template      |
 * | Single Disc File      | single_disc_file_template       | --single-disc-file-template     |
 * | Multi Disc File       | multi_disc_file_template        | --multi-disc-file-template      |
 * | No Album File         | no_album_file_template          | --no-album-file-template        |
 * | Playlist File         | playlist_file_template          | --playlist-file-template        |
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore`.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 */

// Zustand store for reading/writing template settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared Input component for text-based template fields.
import { Input } from '@/components/common';

/**
 * TemplatesTab -- Renders the Templates settings tab.
 *
 * Organised into three sections:
 *   1. **Template Variable Reference** -- An info card listing all available
 *      placeholder variables for quick reference while editing templates.
 *   2. **Folder Templates** -- Input fields for album, compilation, and
 *      no-album folder naming patterns.
 *   3. **File Templates** -- Input fields for single-disc, multi-disc,
 *      no-album, and playlist file naming patterns.
 *
 * Each Input's onChange passes the raw string value to `updateSettings`.
 */
export function TemplatesTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting template changes */
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
