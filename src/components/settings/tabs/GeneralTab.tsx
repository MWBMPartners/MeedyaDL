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

import { useState } from 'react';

// Zustand store providing the shared settings state and mutation function.
// All settings tabs read from the same store instance, ensuring changes
// in one tab are immediately reflected if the user switches tabs.
import { useSettingsStore } from '@/stores/settingsStore';

// Update store for manual update checking trigger.
import { useUpdateStore } from '@/stores/updateStore';

// UI store for toast notifications (used by settings export/import).
import { useUiStore } from '@/stores/uiStore';

// IPC command wrappers for settings export/import.
import { exportSettings, importSettings } from '@/lib/tauri-commands';

// Shared form control components:
// - Toggle: renders a labelled on/off switch
// - FilePickerButton: renders a button that opens the Tauri native file dialog
// - Select: renders a labelled <select> dropdown
// - Button: platform-adaptive button with loading/icon support
import { Toggle, FilePickerButton, Select, Input, Button, SettingsSection } from '@/components/common';

// Lucide icons for the refresh/check action button and export/import buttons.
import { Download, RefreshCw, Upload } from 'lucide-react';

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

/**
 * UI display language options for the language dropdown.
 *
 * - 'auto': Detect from OS locale (default). Internally stored as `""`
 *           in `settings.ui_language`. i18next's LanguageDetector resolves it.
 * - Other codes map to `public/locales/{code}/translation.json` files.
 *
 * To add a new language: create the locale JSON file, then add an entry here.
 */
const UI_LANGUAGE_OPTIONS = [
  { value: 'auto', label: 'Auto (System)' },
  { value: 'en', label: 'English' },
  { value: 'de', label: 'Deutsch' },
  { value: 'fr', label: 'Français' },
];

/**
 * Update check interval options. Value is in hours.
 * Shown only when "Auto-Check for Updates" is enabled.
 */
// Listed in ascending frequency order (most frequent → least frequent).
// "Startup only" is last because 0 = no periodic checks (least frequent).
const UPDATE_INTERVAL_OPTIONS = [
  { value: '1', label: 'Every hour' },
  { value: '6', label: 'Every 6 hours' },
  { value: '12', label: 'Every 12 hours' },
  { value: '24', label: 'Every 24 hours' },
  { value: '0', label: 'Startup only' },
];

/**
 * Colour vision deficiency options for the accessibility selector dropdown.
 *
 * - 'none': Standard colours (default). Internally stored as `""`.
 * - 'deuteranopia': Red-green blindness (most common, ~6% of males).
 * - 'protanopia': Red-green blindness (reduced red sensitivity).
 * - 'tritanopia': Blue-yellow blindness (rare, ~0.01%).
 *
 * The useTheme hook reads the selected value and applies the corresponding
 * CVD CSS class ('cvd-deuteranopia', 'cvd-protanopia', 'cvd-tritanopia')
 * to the <html> element.
 *
 * @see src/hooks/useTheme.ts -- Hook that applies the CVD class
 * @see src/styles/themes/a11y-colour-blind.css -- CSS rules for each variant
 */
const COLOUR_VISION_OPTIONS = [
  { value: 'none', label: 'Normal' },
  { value: 'deuteranopia', label: 'Deuteranopia (Red-Green)' },
  { value: 'protanopia', label: 'Protanopia (Red-Green)' },
  { value: 'tritanopia', label: 'Tritanopia (Blue-Yellow)' },
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

  /** Whether an update check is currently in progress */
  const isChecking = useUpdateStore((s) => s.isChecking);
  /** Trigger a manual update check */
  const checkForUpdates = useUpdateStore((s) => s.checkForUpdates);
  /** Error from the last check */
  const checkError = useUpdateStore((s) => s.error);
  /** Transient message shown after a check completes */
  const [checkMessage, setCheckMessage] = useState<string | null>(null);

  /** Toast notification helper */
  const addToast = useUiStore((s) => s.addToast);
  /** Load settings from backend (used after import to refresh UI) */
  const loadSettings = useSettingsStore((s) => s.loadSettings);
  /** Whether a settings export is in progress */
  const [isExporting, setIsExporting] = useState(false);
  /** Whether a settings import is in progress */
  const [isImporting, setIsImporting] = useState(false);

  /**
   * Handle the "Check for Updates" button click.
   * Calls the backend update check and shows a brief result message.
   */
  const handleCheckForUpdates = async () => {
    setCheckMessage(null);
    try {
      const result = await checkForUpdates();
      if (result.has_updates) {
        const count = result.components.filter((c) => c.update_available && c.is_compatible).length;
        setCheckMessage(`${count} update${count !== 1 ? 's' : ''} available`);
      } else {
        setCheckMessage('Everything is up to date');
      }
    } catch {
      // Error is stored in the update store's error field
    }
  };

  /**
   * Export current settings to a JSON file via native save dialog.
   * Sensitive fields (credentials, cookies) are excluded automatically.
   */
  const handleExportSettings = async () => {
    setIsExporting(true);
    try {
      await exportSettings();
      addToast('Settings exported successfully', 'success');
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      // Don't show a toast for user cancellation
      if (!message.includes('cancelled')) {
        addToast(`Failed to export settings: ${message}`, 'error');
      }
    } finally {
      setIsExporting(false);
    }
  };

  /**
   * Import settings from a JSON file via native file picker.
   * After import, reloads settings from backend to refresh the UI.
   */
  const handleImportSettings = async () => {
    setIsImporting(true);
    try {
      await importSettings();
      // Reload settings from backend so the UI reflects the imported values
      await loadSettings();
      addToast('Settings imported successfully', 'success');
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      // Don't show a toast for user cancellation
      if (!message.includes('cancelled')) {
        addToast(`Failed to import settings: ${message}`, 'error');
      }
    } finally {
      setIsImporting(false);
    }
  };

  return (
    <div className="space-y-3 max-w-xl">
      {/* Section: Output */}
      <SettingsSection title="Output">
        {/* Output directory picker */}
        <FilePickerButton
          label="Output Directory"
          description="Where downloaded files will be saved"
          value={settings.output_path || null}
          onChange={(path) => updateSettings({ output_path: path || '' })}
          directory
          placeholder="Default: ~/Music/Apple Music"
        />
      </SettingsSection>

      {/* Section: Appearance */}
      <SettingsSection title="Appearance">

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

        {/*
         * UI display language selector -- controls translation files loaded by i18next.
         * 'auto' maps to empty string in settings (OS auto-detection).
         * Other values are language codes that map to public/locales/{code}/.
         * Requires app restart to take full effect across all components.
         */}
        {/* High-contrast accessibility toggle */}
        <Toggle
          label="High Contrast"
          description="Increase visual contrast for accessibility. Uses stronger borders, pure black/white text, and thicker focus indicators. Also auto-activates when your OS has high-contrast mode enabled."
          checked={settings.high_contrast}
          onChange={(checked) => updateSettings({ high_contrast: checked })}
        />

        {/*
         * Colour vision deficiency selector -- remaps status colours to palettes
         * that are distinguishable for users with specific types of colour blindness.
         * An empty string (mapped to 'none' in the UI) means disabled.
         * The useTheme hook applies the corresponding cvd-* CSS class to <html>.
         */}
        <Select
          label="Colour Vision"
          description="Adjust status colours for colour vision deficiency. Remaps success, error, warning, and info colours to a palette distinguishable for the selected condition."
          options={COLOUR_VISION_OPTIONS}
          value={settings.colour_blind_mode || 'none'}
          onChange={(e) =>
            updateSettings({
              colour_blind_mode: e.target.value === 'none' ? '' : e.target.value,
            })
          }
        />

        <Select
          label="Language"
          description="Application display language (requires restart to take full effect)"
          options={UI_LANGUAGE_OPTIONS}
          value={settings.ui_language || 'auto'}
          onChange={(e) => {
            const val = e.target.value;
            updateSettings({ ui_language: val === 'auto' ? '' : val });
          }}
        />
      </SettingsSection>

      {/* Section: Preferences */}
      <SettingsSection title="Preferences">

        {/* Metadata language */}
        <Select
          label="Metadata Language"
          description="Language preference for track and album metadata"
          options={LANGUAGE_OPTIONS}
          value={settings.language}
          onChange={(e) => updateSettings({ language: e.target.value })}
        />

        {/* Apple Music storefront region */}
        <Input
          label="Storefront"
          description="Apple Music storefront region code (e.g., gb, us, jp). Leave blank to auto-detect from metadata language."
          placeholder="Auto-detect"
          value={settings.storefront}
          onChange={(e) => updateSettings({ storefront: e.target.value.toLowerCase().trim() })}
        />

        {/* Overwrite existing files */}
        <Toggle
          label="Overwrite Existing Files"
          description="Re-download and replace files that already exist in the output directory"
          checked={settings.overwrite}
          onChange={(checked) => updateSettings({ overwrite: checked })}
        />

        {/* Auto-start queue processing */}
        <Toggle
          label="Auto-Start Downloads"
          description="Start processing immediately when items are added to the queue. When disabled, items are queued and you can start processing manually from the Queue page."
          checked={settings.auto_start_queue}
          onChange={(checked) => updateSettings({ auto_start_queue: checked })}
        />

        {/* Auto-check for updates */}
        <Toggle
          label="Auto-Check for Updates"
          description="Automatically check for GAMDL and tool updates on startup"
          checked={settings.auto_check_updates}
          onChange={(checked) => updateSettings({ auto_check_updates: checked })}
        />

        {/* Update check interval — only visible when auto-check is enabled */}
        {settings.auto_check_updates && (
          <Select
            label="Check Interval"
            description="How often to check for updates while the app is running (takes effect on restart)"
            options={UPDATE_INTERVAL_OPTIONS}
            value={String(settings.update_check_interval_hours)}
            onChange={(e) =>
              updateSettings({ update_check_interval_hours: Number(e.target.value) })
            }
          />
        )}

        {/* Pre-release channel toggle */}
        <Toggle
          label="Include Pre-Release Versions"
          description="Check for pre-release (beta/RC) versions in addition to stable releases. Pre-release versions may contain bugs or incomplete features and are not yet fully supported."
          checked={settings.check_pre_releases}
          onChange={(checked) => updateSettings({ check_pre_releases: checked })}
        />

        {/* Manual update check button */}
        <div className="pt-2 space-y-2">
          <div className="flex items-center gap-3">
            <Button
              variant="secondary"
              size="sm"
              icon={<RefreshCw size={14} />}
              loading={isChecking}
              onClick={handleCheckForUpdates}
            >
              {isChecking ? 'Checking...' : 'Check for Updates'}
            </Button>
            {checkMessage && !isChecking && (
              <span className="text-xs text-content-secondary">{checkMessage}</span>
            )}
          </div>
          {checkError && !isChecking && <p className="text-xs text-status-error">{checkError}</p>}
        </div>
      </SettingsSection>

      {/* Section: Backup */}
      <SettingsSection title="Backup">
        <p className="text-xs text-content-secondary mb-2">
          Export your settings to a file for backup or transfer to another device.
          Sensitive fields (cookies, credentials) are excluded from the export.
        </p>
        <div className="flex items-center gap-3">
          <Button
            variant="secondary"
            size="sm"
            icon={<Upload size={14} />}
            loading={isExporting}
            onClick={handleExportSettings}
          >
            {isExporting ? 'Exporting...' : 'Export Settings'}
          </Button>
          <Button
            variant="secondary"
            size="sm"
            icon={<Download size={14} />}
            loading={isImporting}
            onClick={handleImportSettings}
          >
            {isImporting ? 'Importing...' : 'Import Settings'}
          </Button>
        </div>
      </SettingsSection>
    </div>
  );
}
