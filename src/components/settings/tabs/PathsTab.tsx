/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Paths settings tab.
 * Configures custom paths for external tool binaries (FFmpeg, mp4decrypt,
 * N_m3u8DL-RE, MP4Box, AMDecrypt). When empty, uses the managed (auto-installed)
 * versions from the app data directory.
 */

import { useSettingsStore } from '@/stores/settingsStore';
import { FilePickerButton } from '@/components/common';

/**
 * Renders the Paths settings tab with file pickers for each external tool.
 * Empty paths use the built-in managed versions.
 */
export function PathsTab() {
  const settings = useSettingsStore((s) => s.settings);
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
