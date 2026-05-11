/**
 * Copyright (c) 2026 MeedyaDL
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

// Zustand store hooks. `useSettingsStore` is retained for the
// `loadSettings` action only (used by the import flow). All per-
// field reads/writes go through `useSettingsField` (audit v2 #6).
import { useSettingsStore } from '@/stores/settingsStore';
import { useSettingsField } from '@/hooks/useSettingsField';

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
import { Toggle, FilePickerButton, Select, Button, SettingsSection } from '@/components/common';
import ChannelSwitchWarning from '@/components/settings/ChannelSwitchWarning';
import { PRE_RELEASE_CHANNELS, type UpdateChannel } from '@/types';

// Lucide icons for the refresh/check action button and export/import buttons.
import { Bell, Download, RefreshCw, Upload } from 'lucide-react';
import type { AfterQueueAction } from '@/types';

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
 * Update-channel options. Listed from least to most stable so the dropdown
 * reads top-down as "riskiest first" and the default (Stable) is the last
 * entry — matching conventional release-channel pickers.
 *
 * The installer guard refuses to apply a release whose channel is less
 * stable than the user's selection, so switching down the list is the
 * explicit opt-in for early builds.
 */
const UPDATE_CHANNEL_OPTIONS_FULL = [
  { value: 'nightly', label: 'Nightly — built daily, may be broken' },
  { value: 'weekly', label: 'Weekly — integrated weekly, often unstable' },
  { value: 'monthly', label: 'Monthly — integrated monthly' },
  { value: 'alpha', label: 'Alpha — feature-complete, expect bugs' },
  { value: 'beta', label: 'Beta — pre-RC integration' },
  { value: 'rc', label: 'Release Candidate — near-stable, last validation pass' },
  { value: 'stable', label: 'Stable — production release (recommended)' },
];

/**
 * Channel options shown to users without Dev Access. Hides the four
 * experimental channels (nightly/weekly/monthly/alpha) so casual users
 * cannot accidentally subscribe to internal-only builds. Matches
 * `DEV_ACCESS_CHANNELS` in `src/types/index.ts` and the Rust
 * `UpdateChannel::requires_dev_access()` predicate.
 */
const UPDATE_CHANNEL_OPTIONS_PUBLIC = UPDATE_CHANNEL_OPTIONS_FULL.filter(
  (opt) => !['nightly', 'weekly', 'monthly', 'alpha'].includes(opt.value)
);

/**
 * Notification auto-dismiss duration options (seconds).
 */
const NOTIFICATION_STYLE_OPTIONS = [
  { value: 'in_app_only', label: 'In-app only' },
  { value: 'native_and_in_app', label: 'Native + in-app (default)' },
  { value: 'native_only', label: 'Native only' },
];

const NOTIFICATION_DISMISS_OPTIONS = [
  { value: '3', label: '3 seconds' },
  { value: '5', label: '5 seconds (default)' },
  { value: '10', label: '10 seconds' },
  { value: '15', label: '15 seconds' },
  { value: '30', label: '30 seconds' },
  { value: '60', label: '60 seconds' },
];

/**
 * After-queue action options.
 * "once" variants are handled via the context menu on the Download page;
 * the Settings dropdown controls the persistent "always" action.
 */
const AFTER_QUEUE_OPTIONS = [
  { value: 'do_nothing', label: 'Do nothing' },
  { value: 'open_output_folder', label: 'Open output folder' },
  { value: 'play_sound', label: 'Play notification sound' },
  { value: 'close_meedyadl', label: 'Close MeedyaDL' },
  { value: 'restart_computer', label: 'Restart computer' },
  { value: 'hibernate_computer', label: 'Hibernate / Sleep' },
  { value: 'shutdown_computer', label: 'Shut down computer' },
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
  // Per-field Zustand bindings (audit v2 #6).
  const outputPath = useSettingsField('output_path');
  const themeOverride = useSettingsField('theme_override');
  const highContrast = useSettingsField('high_contrast');
  const colourBlindMode = useSettingsField('colour_blind_mode');
  const uiLanguage = useSettingsField('ui_language');
  const language = useSettingsField('language');
  const storefront = useSettingsField('storefront');
  const storefrontFallback = useSettingsField('storefront_fallback_on_failure');
  const overwrite = useSettingsField('overwrite');
  const autoStartQueue = useSettingsField('auto_start_queue');
  const afterQueueAction = useSettingsField('after_queue_action');
  const desktopNotifications = useSettingsField('desktop_notifications');
  const notificationStyle = useSettingsField('notification_style');
  const notificationDismissSeconds = useSettingsField('notification_auto_dismiss_seconds');
  const smartRedownload = useSettingsField('smart_redownload_detection');
  const clipboardMonitoring = useSettingsField('clipboard_monitoring');
  const autoCheckUpdates = useSettingsField('auto_check_updates');
  const updateCheckInterval = useSettingsField('update_check_interval_hours');
  const checkPreReleases = useSettingsField('check_pre_releases');
  const updateChannel = useSettingsField('update_channel');
  const devAccessEnabled = useSettingsField('dev_access_enabled');

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
   * Pending channel switch awaiting user confirmation. When the user picks a
   * pre-release channel from the dropdown, the actual settings update is
   * deferred until they accept the stability warning. `null` = no pending
   * switch; the dropdown reflects the persisted `settings.update_channel`.
   */
  const [pendingChannel, setPendingChannel] = useState<UpdateChannel | null>(null);

  /**
   * Filter the channel dropdown by Dev Access. The four most-experimental
   * channels (Nightly/Weekly/Monthly/Alpha) are hidden from non-dev users
   * to prevent casual subscription. Existing subscribers keep their channel
   * even if they later disable Dev Access — they just can't re-pick it.
   */
  const channelOptions = devAccessEnabled.value
    ? UPDATE_CHANNEL_OPTIONS_FULL
    : UPDATE_CHANNEL_OPTIONS_PUBLIC;

  /**
   * Channel-switch handler with pre-release confirmation. Switching to any
   * less-stable channel triggers `ChannelSwitchWarning`. Switching toward
   * Stable (or to the channel already selected) bypasses the modal.
   */
  const handleChannelChange = (next: UpdateChannel) => {
    if (next === updateChannel.value) return;
    if (PRE_RELEASE_CHANNELS.includes(next)) {
      setPendingChannel(next);
    } else {
      updateChannel.set(next);
    }
  };

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
    <div className="space-y-3">
      {/* Section: Output */}
      <SettingsSection title="Output">
        {/* Output directory picker */}
        <FilePickerButton
          label="Output Directory"
          description="Where downloaded files will be saved"
          value={outputPath.value || null}
          onChange={(path) => outputPath.set(path || '')}
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
          value={themeOverride.value || 'auto'}
          onChange={(e) =>
            themeOverride.set(e.target.value === 'auto' ? null : e.target.value)
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
          checked={highContrast.value}
          onChange={highContrast.set}
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
          value={colourBlindMode.value || 'none'}
          onChange={(e) =>
            colourBlindMode.set(e.target.value === 'none' ? '' : e.target.value)
          }
        />

        <Select
          label="Language"
          description="Application display language (requires restart to take full effect)"
          options={UI_LANGUAGE_OPTIONS}
          value={uiLanguage.value || 'auto'}
          onChange={(e) => {
            const val = e.target.value;
            uiLanguage.set(val === 'auto' ? '' : val);
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
          value={language.value}
          onChange={(e) => language.set(e.target.value)}
        />

        {/* Apple Music storefront region */}
        <div>
          <label className="block text-sm font-medium text-content-primary mb-1">
            Storefront
          </label>
          <select
            className="w-full rounded-platform border border-border-light bg-surface-elevated px-3 py-2 text-sm text-content-primary"
            value={storefront.value}
            onChange={(e) => storefront.set(e.target.value)}
          >
            <option value="">Auto-detect</option>
            <option disabled>──────────</option>
            <option value="au">Australia</option>
            <option value="at">Austria</option>
            <option value="be">Belgium</option>
            <option value="br">Brazil</option>
            <option value="ca">Canada</option>
            <option value="cn">China</option>
            <option value="co">Colombia</option>
            <option value="cz">Czech Republic</option>
            <option value="dk">Denmark</option>
            <option value="eg">Egypt</option>
            <option value="fi">Finland</option>
            <option value="fr">France</option>
            <option value="de">Germany</option>
            <option value="gr">Greece</option>
            <option value="hk">Hong Kong</option>
            <option value="hu">Hungary</option>
            <option value="in">India</option>
            <option value="id">Indonesia</option>
            <option value="ie">Ireland</option>
            <option value="il">Israel</option>
            <option value="it">Italy</option>
            <option value="jp">Japan</option>
            <option value="ke">Kenya</option>
            <option value="kr">Korea, South</option>
            <option value="my">Malaysia</option>
            <option value="mx">Mexico</option>
            <option value="nl">Netherlands</option>
            <option value="nz">New Zealand</option>
            <option value="ng">Nigeria</option>
            <option value="no">Norway</option>
            <option value="pk">Pakistan</option>
            <option value="ph">Philippines</option>
            <option value="pl">Poland</option>
            <option value="pt">Portugal</option>
            <option value="ro">Romania</option>
            <option value="ru">Russia</option>
            <option value="sa">Saudi Arabia</option>
            <option value="sg">Singapore</option>
            <option value="za">South Africa</option>
            <option value="es">Spain</option>
            <option value="se">Sweden</option>
            <option value="ch">Switzerland</option>
            <option value="tw">Taiwan</option>
            <option value="th">Thailand</option>
            <option value="tr">Turkey</option>
            <option value="ae">United Arab Emirates</option>
            <option value="gb">United Kingdom</option>
            <option value="us">United States</option>
            <option value="vn">Vietnam</option>
          </select>
          <p className="text-xs text-content-tertiary mt-1">
            Apple Music storefront region. Leave as &quot;Auto-detect&quot; to derive from metadata language setting.
          </p>
        </div>

        {/* Storefront fallback on failure (#666) */}
        <Toggle
          label="Auto-retry with your region when a URL's storefront fails"
          description="When a download fails because the album isn't published in the URL's storefront (e.g., a /us/ link your account can't access), retry once using your account region above. Only fires after the original URL has already failed, so it never overrides a working download."
          checked={storefrontFallback.value}
          onChange={storefrontFallback.set}
        />

        {/* Overwrite existing files */}
        <Toggle
          label="Overwrite Existing Files"
          description="Re-download and replace files that already exist in the output directory"
          checked={overwrite.value}
          onChange={overwrite.set}
        />

        {/* Auto-start queue processing */}
        <Toggle
          label="Auto-Start Downloads"
          description="Start processing immediately when items are added to the queue. When disabled, items are queued and you can start processing manually from the Queue page."
          checked={autoStartQueue.value}
          onChange={autoStartQueue.set}
        />

        {/* After-queue action */}
        <Select
          label="After Queue Completes"
          description="Action to perform automatically when all downloads finish. This is the persistent setting — use the right-click menu on the Download page for a one-time override."
          options={AFTER_QUEUE_OPTIONS}
          value={afterQueueAction.value ?? 'do_nothing'}
          onChange={(e) => afterQueueAction.set(e.target.value as AfterQueueAction)}
        />

        {/* Desktop notifications */}
        <Toggle
          label="Desktop Notifications"
          description="Show native OS notifications when downloads complete or fail. Notifications are only shown when the app window is not focused."
          checked={desktopNotifications.value}
          onChange={desktopNotifications.set}
        />

        {/* Notification style — controls how notifications are delivered */}
        {desktopNotifications.value && (
          <Select
            label="Notification Style"
            description="How notifications are delivered. 'In-app only' shows toasts inside the app. 'Native + in-app' shows both OS notifications and in-app toasts. 'Native only' uses OS notifications exclusively."
            options={NOTIFICATION_STYLE_OPTIONS}
            value={notificationStyle.value ?? 'native_and_in_app'}
            onChange={(e) =>
              notificationStyle.set(e.target.value as 'in_app_only' | 'native_and_in_app' | 'native_only')
            }
          />
        )}

        {/* Test Notification button (#658).
            Fires a real native notification so the user can verify that the
            OS-level permission has been granted and the plugin pipeline works.
            On macOS this also forces the system permission prompt the first
            time it is invoked if the user dismissed the startup prompt. */}
        {desktopNotifications.value &&
          (notificationStyle.value ?? 'native_and_in_app') !== 'in_app_only' && (
            <div className="flex flex-col gap-1">
              <Button
                variant="secondary"
                icon={<Bell size={14} />}
                onClick={async () => {
                  try {
                    const { isPermissionGranted, requestPermission, sendNotification } =
                      await import('@tauri-apps/plugin-notification');
                    let granted = await isPermissionGranted();
                    if (!granted) {
                      const result = await requestPermission();
                      granted = result === 'granted';
                    }
                    if (!granted) {
                      addToast(
                        'Notification permission was not granted. Open System Settings > Notifications > MeedyaDL to enable.',
                        'warning',
                      );
                      return;
                    }
                    sendNotification({
                      title: 'MeedyaDL — Test',
                      body: 'If you can read this, native notifications are working.',
                    });
                    addToast('Test notification sent. If you do not see it, check OS notification settings.', 'info');
                  } catch (err) {
                    addToast(
                      `Test notification failed: ${err instanceof Error ? err.message : String(err)}`,
                      'error',
                    );
                  }
                }}
              >
                Send Test Notification
              </Button>
              <p className="text-xs text-content-tertiary">
                Sends a one-off native notification so you can verify the OS pipeline.
              </p>
            </div>
          )}

        {/* Notification auto-dismiss duration — only visible when notifications are enabled */}
        {desktopNotifications.value && (
          <Select
            label="Notification Auto-Dismiss"
            description="How long transient notifications (download complete, added to queue) stay visible before auto-dismissing. Error and warning notifications always require manual dismissal."
            options={NOTIFICATION_DISMISS_OPTIONS}
            value={String(notificationDismissSeconds.value ?? 5)}
            onChange={(e) => notificationDismissSeconds.set(Number(e.target.value))}
          />
        )}

        {/* Smart re-download detection */}
        <Toggle
          label="Smart Re-Download Detection"
          description="When re-downloading an album, check if it has changed since your last download using the Apple Music API. Shows an info message if the album is unchanged, but still allows you to proceed."
          checked={smartRedownload.value}
          onChange={smartRedownload.set}
        />

        {/* Clipboard monitoring */}
        <Toggle
          label="Clipboard Monitoring"
          description="Detect supported URLs (e.g., Apple Music) copied to the clipboard and offer to download them. Only checks for URL patterns — clipboard contents are never stored."
          checked={clipboardMonitoring.value}
          onChange={clipboardMonitoring.set}
        />

        {/* Auto-check for updates */}
        <Toggle
          label="Auto-Check for Updates"
          description="Automatically check for GAMDL and tool updates on startup"
          checked={autoCheckUpdates.value}
          onChange={autoCheckUpdates.set}
        />

        {/* Update check interval — only visible when auto-check is enabled */}
        {autoCheckUpdates.value && (
          <Select
            label="Check Interval"
            description="How often to check for updates while the app is running (takes effect on restart)"
            options={UPDATE_INTERVAL_OPTIONS}
            value={String(updateCheckInterval.value)}
            onChange={(e) => updateCheckInterval.set(Number(e.target.value))}
          />
        )}

        {/* Pre-release channel toggle */}
        <Toggle
          label="Include Pre-Release Versions"
          description="Check for pre-release (beta/RC) versions in addition to stable releases. Pre-release versions may contain bugs or incomplete features and are not yet fully supported."
          checked={checkPreReleases.value}
          onChange={checkPreReleases.set}
        />

        {/* Update channel selector — guards auto-updates from crossing down the
            stability ladder. Selecting anything other than Stable implicitly
            enables pre-release checks on the backend. */}
        <Select
          label="Update Channel"
          description="Controls which release channel you receive updates from. Subscribing to a channel surfaces releases at-or-above that stability tier (e.g. Beta sees Beta, RC, Stable). Nightly/Weekly/Monthly/Alpha are hidden unless Dev Access is enabled. Switching to any pre-release channel requires explicit confirmation."
          options={channelOptions}
          value={updateChannel.value}
          onChange={(e) => handleChannelChange(e.target.value as UpdateChannel)}
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

      {/* Pre-release stability warning. Open whenever the user has picked a
          pre-release channel from the dropdown but not yet confirmed the
          switch. Confirming applies the change to the settings store;
          cancelling discards it (the dropdown still reflects the persisted
          value, since `value={updateChannel.value}` was never updated). */}
      {pendingChannel && (
        <ChannelSwitchWarning
          open={true}
          targetChannel={pendingChannel}
          onConfirm={() => {
            updateChannel.set(pendingChannel);
            setPendingChannel(null);
          }}
          onCancel={() => setPendingChannel(null)}
        />
      )}
    </div>
  );
}
