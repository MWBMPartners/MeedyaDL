/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * General settings tab.
 * Contains output path, language, overwrite mode, and update check settings.
 */

import { useSettingsStore } from '@/stores/settingsStore';
import { Toggle, FilePickerButton, Select } from '@/components/common';

/** Available languages for GAMDL's metadata language preference */
const LANGUAGE_OPTIONS = [
  { value: 'en-US', label: 'English (US)' },
  { value: 'en-GB', label: 'English (UK)' },
  { value: 'ja-JP', label: 'Japanese' },
  { value: 'ko-KR', label: 'Korean' },
  { value: 'zh-CN', label: 'Chinese (Simplified)' },
  { value: 'zh-TW', label: 'Chinese (Traditional)' },
  { value: 'de-DE', label: 'German' },
  { value: 'fr-FR', label: 'French' },
  { value: 'es-ES', label: 'Spanish' },
  { value: 'pt-BR', label: 'Portuguese (Brazil)' },
  { value: 'it-IT', label: 'Italian' },
  { value: 'ru-RU', label: 'Russian' },
];

/**
 * Renders the General settings tab with output path, language,
 * overwrite toggle, and auto-update toggle.
 */
export function GeneralTab() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="space-y-6 max-w-xl">
      {/* Section: Output */}
      <div>
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Output
        </h3>

        {/* Output directory picker */}
        <FilePickerButton
          label="Output Directory"
          description="Where downloaded files will be saved"
          value={settings.output_path || null}
          onChange={(path) => updateSettings({ output_path: path || '' })}
          directory
          placeholder="Default: ~/Music/Apple Music"
        />
      </div>

      {/* Section: Preferences */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Preferences
        </h3>

        {/* Metadata language */}
        <Select
          label="Metadata Language"
          description="Language preference for track and album metadata"
          options={LANGUAGE_OPTIONS}
          value={settings.language}
          onChange={(e) => updateSettings({ language: e.target.value })}
        />

        {/* Overwrite existing files */}
        <Toggle
          label="Overwrite Existing Files"
          description="Re-download and replace files that already exist in the output directory"
          checked={settings.overwrite}
          onChange={(checked) => updateSettings({ overwrite: checked })}
        />

        {/* Auto-check for updates */}
        <Toggle
          label="Auto-Check for Updates"
          description="Automatically check for GAMDL and tool updates on startup"
          checked={settings.auto_check_updates}
          onChange={(checked) =>
            updateSettings({ auto_check_updates: checked })
          }
        />
      </div>
    </div>
  );
}
