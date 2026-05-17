/**
 * Copyright (c) 2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file TemplatesTab.tsx -- File and folder naming template settings tab.
 *
 * Renders the "Templates" tab within the {@link SettingsPage} component.
 * GAMDL uses Python-style format strings to determine how downloaded files
 * and folders are named. This tab uses the visual {@link TemplateBuilder}
 * component to let users build templates by selecting variables from a menu
 * rather than typing format strings manually. A raw-edit toggle is available
 * for power users who prefer direct text editing.
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
 * @see {@link ../SettingsPage.tsx}                        -- Parent container
 * @see {@link @/stores/settingsStore.ts}                  -- Zustand store
 * @see {@link @/components/common/TemplateBuilder.tsx}    -- Visual builder component
 * @see {@link @/lib/template-parser.ts}                   -- Parser/serializer
 */

// Audit v2 #6 — per-field Zustand binding (replaces the
// `settings.X` + `updateSettings({ X: v })` lambda pair).
import { useSettingsField } from '@/hooks/useSettingsField';

// Visual chip/pill builder for GAMDL template strings.
import { TemplateBuilder, Select, SettingsSection } from '@/components/common';
import type { DiscNumberPadding, TrackNumberPadding } from '@/types';

/**
 * Options for the {track} placeholder padding selector (#587).
 * `'auto'` is the recommended default — sorts box sets correctly without
 * the user having to know in advance whether the album has 12 or 200 tracks.
 */
const TRACK_PADDING_OPTIONS = [
  { value: 'auto', label: 'Auto (recommended) — derive width from album track count' },
  { value: 'none', label: 'None — 1, 2, 10, 100' },
  { value: 'two_digits', label: '2 digits — 01, 02, 99, 100' },
  { value: 'three_digits', label: '3 digits — 001, 002, 999' },
  { value: 'four_digits', label: '4 digits — 0001, 0002, 9999' },
] as const;

/**
 * Options for the {disc} placeholder padding selector (#587).
 * `'auto'` defaults to 1-digit for albums with <10 discs (the common case)
 * and 2-digit for box sets.
 */
const DISC_PADDING_OPTIONS = [
  { value: 'auto', label: 'Auto (recommended) — single-digit until 10+ discs' },
  { value: 'none', label: 'None — same as 1 digit' },
  { value: 'one_digit', label: '1 digit — 1, 2, 9' },
  { value: 'two_digits', label: '2 digits — 01, 02, 99' },
] as const;

/**
 * TemplatesTab -- Renders the Templates settings tab.
 *
 * Organised into three sections:
 *   1. **Template Variable Reference** -- An info card listing all available
 *      placeholder variables for quick reference while in raw editing mode.
 *   2. **Folder Templates** -- TemplateBuilder fields for album, compilation,
 *      and no-album folder naming patterns.
 *   3. **File Templates** -- TemplateBuilder fields for single-disc, multi-disc,
 *      no-album, and playlist file naming patterns.
 *
 * Each TemplateBuilder's onChange passes the serialized template string directly
 * to `updateSettings`. The playlist template includes the 'playlist' category
 * in its variable menu to show playlist-specific variables.
 */
export function TemplatesTab() {
  // Per-field bindings (audit v2 #6 — replaces the previous
  // `settings` + `updateSettings` lambda pair).
  const albumFolder = useSettingsField('album_folder_template');
  const compilationFolder = useSettingsField('compilation_folder_template');
  const noAlbumFolder = useSettingsField('no_album_folder_template');
  const playlistFolder = useSettingsField('playlist_folder_template');
  const singleDiscFile = useSettingsField('single_disc_file_template');
  const multiDiscFile = useSettingsField('multi_disc_file_template');
  const noAlbumFile = useSettingsField('no_album_file_template');
  const playlistFile = useSettingsField('playlist_file_template');
  const trackPadding = useSettingsField('track_number_padding');
  const discPadding = useSettingsField('disc_number_padding');

  return (
    <div className="space-y-3">
      {/* Template variable reference (useful when in raw edit mode) */}
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
            {'{playlist_artist}'}, {'{playlist_title}'}, {'{playlist_id}'}
          </p>
          <p>
            {'{album_id}'}, {'{platform}'}
          </p>
        </div>
      </div>

      {/* Section: Folder Templates */}
      <SettingsSection title="Folder Templates">

        <TemplateBuilder
          label="Album Folder"
          description="Folder structure for regular album downloads"
          value={albumFolder.value}
          onChange={albumFolder.set}
        />

        <TemplateBuilder
          label="Compilation Folder"
          description="Folder structure for compilation/various artist albums"
          value={compilationFolder.value}
          onChange={compilationFolder.set}
        />

        <TemplateBuilder
          label="No Album Folder"
          description="Folder structure when album information is not available"
          value={noAlbumFolder.value}
          onChange={noAlbumFolder.set}
        />

        {/*
          Playlist Folder template — GAMDL v3.0+ only (#618). MeedyaDL stores
          the value on every profile, but the CLI / INI emission is gated
          behind `GamdlFeature::PlaylistFolderTemplate` in the Rust layer.
          Users on v2.9.x can still edit the field here (it's persisted
          to settings.json) but GAMDL will fall back to its own built-in
          default until they upgrade. We show a small advisory under the
          description to make that expectation-setting obvious, matching
          the pattern used for `wrapper_m3u8_ip` in the Advanced tab.
        */}
        <TemplateBuilder
          label="Playlist Folder"
          description="Folder structure for playlist downloads. Requires GAMDL v3.0+ — earlier versions fall back to the upstream default layout regardless of this value."
          value={playlistFolder.value}
          onChange={playlistFolder.set}
          variableCategories={['common', 'playlist']}
        />
      </SettingsSection>

      {/* Section: File Templates */}
      <SettingsSection title="File Templates">

        <TemplateBuilder
          label="Single Disc File"
          description="Filename template for tracks from single-disc albums"
          value={singleDiscFile.value}
          onChange={singleDiscFile.set}
        />

        <TemplateBuilder
          label="Multi Disc File"
          description="Filename template for tracks from multi-disc albums"
          value={multiDiscFile.value}
          onChange={multiDiscFile.set}
        />

        <TemplateBuilder
          label="No Album File"
          description="Filename template when album information is not available"
          value={noAlbumFile.value}
          onChange={noAlbumFile.set}
        />

        <TemplateBuilder
          label="Playlist File"
          description="Filename template for playlist downloads"
          value={playlistFile.value}
          onChange={playlistFile.set}
          variableCategories={['common', 'track', 'album', 'playlist']}
        />

        {/* Padding controls (#587). Govern how `{track}` and `{disc}` get
            zero-padded when emitted into the template strings above.
            Previously hard-coded to 2-digit padding, which sorted box
            sets >99 tracks incorrectly. */}
        <Select
          label="Track Number Padding"
          description="Width for the {track} placeholder in the templates above. Auto sizes to the album's track count, so `100 Track 100.m4a` sorts correctly after `099 Track 99.m4a`."
          options={[...TRACK_PADDING_OPTIONS]}
          value={trackPadding.value}
          onChange={(e) => trackPadding.set(e.target.value as TrackNumberPadding)}
        />

        <Select
          label="Disc Number Padding"
          description="Width for the {disc} placeholder. Multi-disc albums use this in the template above. Auto stays single-digit for typical albums, jumps to 2-digit for >9 discs."
          options={[...DISC_PADDING_OPTIONS]}
          value={discPadding.value}
          onChange={(e) => discPadding.set(e.target.value as DiscNumberPadding)}
        />
      </SettingsSection>
    </div>
  );
}
