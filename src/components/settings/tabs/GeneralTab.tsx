/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file GeneralTab.tsx -- General preferences settings tab.
 *
 * Renders the "General" tab within the {@link SettingsPage} component.
 * This tab exposes the most commonly adjusted settings:
 *
 *   - **Theme** -- Dark, light, or auto (follow OS). Maps to
 *     `settings.theme_override` (null = auto, 'light', or 'dark').
 *     The useTheme hook applies the appropriate CSS class to <html>.
 *
 *   - **Output Directory** -- Where downloaded files are saved. Uses
 *     the Tauri file dialog to let the user browse for a directory.
 *     Maps to `settings.output_path` in the Zustand store and the
 *     Rust backend's `AppSettings.output_path` field.
 *
 *   - **Metadata Language** -- Preferred language for track and album
 *     metadata returned by the Apple Music API. Maps to
 *     `settings.language` (ISO locale code, e.g., `"en-US"`).
 *
 *   - **Overwrite Existing Files** -- Whether to re-download and replace
 *     files that already exist in the output directory. Maps to
 *     `settings.overwrite`.
 *
 *   - **Auto-Check for Updates** -- Whether the application checks for
 *     GAMDL and tool updates on startup. Maps to
 *     `settings.auto_check_updates`.
 *
 * ## Store Connection
 *
 * This component reads from and writes to the Zustand `settingsStore`.
 * It uses:
 *   - `useSettingsStore((s) => s.settings)` -- read the current settings object.
 *   - `useSettingsStore((s) => s.updateSettings)` -- apply a partial settings
 *     patch, which sets `isDirty = true` in the store so the parent
 *     {@link SettingsPage} knows unsaved changes exist.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container that renders this tab
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store backing this component
 * @see {@link https://react.dev/}         -- React documentation
 * @see {@link https://v2.tauri.app/}      -- Tauri 2.0 framework
 */

// Zustand store providing the shared settings state and mutation function.
// All settings tabs read from the same store instance, ensuring changes
// in one tab are immediately reflected if the user switches tabs.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared form control components:
// - Toggle: renders a labelled on/off switch
// - FilePickerButton: renders a button that opens the Tauri native file dialog
// - Select: renders a labelled <select> dropdown
import { Toggle, FilePickerButton, Select } from '@/components/common';

/**
 * Available language options for GAMDL's metadata language preference.
 * Each entry maps an ISO locale code (BCP 47) to a human-readable label.
 * The selected value is passed directly to GAMDL's `--language` flag.
 */
/**
 * Theme mode options for the appearance selector dropdown.
 *
 * - 'auto': Follow the operating system's dark/light preference (default).
 *           Internally stored as `null` in settings.theme_override.
 * - 'light': Force light mode regardless of OS setting.
 * - 'dark': Force dark mode regardless of OS setting.
 *
 * The useTheme hook in App.tsx reads the selected value and applies the
 * appropriate CSS class ('theme-light' or 'theme-dark') to the <html> element.
 *
 * @see src/hooks/useTheme.ts -- Hook that applies the theme class
 * @see src/styles/themes/base.css -- CSS rules that respond to the class
 */
const THEME_OPTIONS = [
  { value: 'auto', label: 'Auto (System)' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];

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
 * GeneralTab -- Renders the General settings tab.
 *
 * Displays four settings controls in two visual sections ("Output" and
 * "Preferences"). Each control's `onChange` handler calls `updateSettings`
 * with a partial patch object to update only the changed field.
 *
 * This component does not manage its own state -- it is a pure
 * "controlled" form that reads from and writes to the Zustand store.
 */
export function GeneralTab() {
  /** The full settings object (read-only snapshot from the store) */
  const settings = useSettingsStore((s) => s.settings);
  /** Applies a partial update to the settings; sets isDirty = true in the store */
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

      {/* Section: Appearance */}
      <div className="space-y-4">
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Appearance
        </h3>

        {/*
          * Theme mode selector -- controls dark/light/auto appearance.
          * The 'auto' option maps to null in theme_override (follow OS).
          * 'light' and 'dark' are stored as strings that the useTheme hook
          * reads to apply the corresponding CSS class to <html>.
          */}
        <Select
          label="Theme"
          description="Choose between light and dark mode, or follow your OS setting"
          options={THEME_OPTIONS}
          value={settings.theme_override || 'auto'}
          onChange={(e) =>
            updateSettings({
              theme_override: e.target.value === 'auto' ? null : e.target.value,
            })
          }
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
