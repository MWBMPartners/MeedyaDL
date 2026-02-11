/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file PathsTab.tsx -- External tool binary path configuration.
 *
 * Renders the "Paths" tab within the {@link SettingsPage} component.
 * GAMDL depends on several external command-line tools for downloading,
 * decrypting, and remuxing media files. By default, these tools are
 * automatically managed (downloaded and installed into the application's
 * data directory during first-run setup). This tab allows power users to
 * override those paths with system-wide installations.
 *
 * ## Supported Tools
 *
 * | Tool          | Setting Key       | Purpose                                |
 * |---------------|-------------------|----------------------------------------|
 * | FFmpeg        | ffmpeg_path       | Audio/video processing and remuxing    |
 * | mp4decrypt    | mp4decrypt_path   | Decrypting DRM-protected streams       |
 * | N_m3u8DL-RE   | nm3u8dlre_path    | Alternative HLS stream downloader      |
 * | MP4Box        | mp4box_path       | Alternative remux tool                 |
 * | AMDecrypt     | amdecrypt_path    | Optional Apple Music decryption tool   |
 *
 * When a path is empty/null, the managed (auto-installed) version is used.
 * Each FilePickerButton opens the Tauri native file dialog so the user can
 * browse to the executable.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore` via `settings.*_path` fields
 * and `updateSettings({ *_path: ... })`.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 * @see {@link https://v2.tauri.app/develop/calling-rust/#commands} -- Tauri IPC commands
 */

// Zustand store for reading/writing tool path settings.
import { useSettingsStore } from '@/stores/settingsStore';

// FilePickerButton opens the Tauri native file dialog and displays the selected path.
import { FilePickerButton } from '@/components/common';

/**
 * PathsTab -- Renders the Paths settings tab.
 *
 * Displays one FilePickerButton per external tool, each bound to its
 * corresponding `settings.*_path` field. The onChange handler passes the
 * selected path (or null when cleared) directly to `updateSettings`.
 *
 * The placeholder text varies: managed tools show "Using managed version",
 * while AMDecrypt (which has no auto-install) shows "Not configured".
 */
export function PathsTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting path changes */
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="space-y-6 max-w-xl">
      <div>
        <p className="text-sm text-content-secondary mb-4">
          Override the paths to external tool binaries. Leave empty to use the
          managed (auto-installed) versions. Custom paths are useful if you have
          system-wide installations you prefer.
        </p>
      </div>

      {/* FFmpeg path */}
      <FilePickerButton
        label="FFmpeg"
        description="Used for audio/video processing and remuxing. Required for most operations."
        value={settings.ffmpeg_path}
        onChange={(path) => updateSettings({ ffmpeg_path: path })}
        placeholder="Using managed version"
      />

      {/* mp4decrypt path */}
      <FilePickerButton
        label="mp4decrypt"
        description="Used for decrypting DRM-protected streams."
        value={settings.mp4decrypt_path}
        onChange={(path) => updateSettings({ mp4decrypt_path: path })}
        placeholder="Using managed version"
      />

      {/* N_m3u8DL-RE path */}
      <FilePickerButton
        label="N_m3u8DL-RE"
        description="Alternative download mode for HLS streams. Used when download mode is set to nm3u8dlre."
        value={settings.nm3u8dlre_path}
        onChange={(path) => updateSettings({ nm3u8dlre_path: path })}
        placeholder="Using managed version"
      />

      {/* MP4Box path */}
      <FilePickerButton
        label="MP4Box"
        description="Alternative remux tool. Used when remux mode is set to mp4box."
        value={settings.mp4box_path}
        onChange={(path) => updateSettings({ mp4box_path: path })}
        placeholder="Using managed version"
      />

      {/* AMDecrypt path */}
      <FilePickerButton
        label="AMDecrypt"
        description="Optional Apple Music decryption tool (alternative to mp4decrypt)."
        value={settings.amdecrypt_path}
        onChange={(path) => updateSettings({ amdecrypt_path: path })}
        placeholder="Not configured"
      />
    </div>
  );
}
