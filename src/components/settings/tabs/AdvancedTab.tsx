/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file AdvancedTab.tsx -- Advanced options settings tab.
 *
 * Renders the "Advanced" tab within the {@link SettingsPage} component.
 * This tab exposes expert-level settings that most users will not need to
 * change, organised into sections:
 *
 * ## Section 1: Processing
 *
 *   - **Download Mode** -- Selects which tool is used for fetching HLS
 *     streams from Apple Music's CDN. Maps to `settings.download_mode`
 *     and GAMDL's `--download-mode` flag.
 *       - `ytdlp` (recommended): Uses yt-dlp, the standard choice
 *       - `nm3u8dlre`: Uses N_m3u8DL-RE as an alternative
 *
 *   - **Remux Mode** -- Selects which tool remuxes downloaded streams into
 *     the final container format. Maps to `settings.remux_mode` and
 *     GAMDL's `--remux-mode` flag.
 *       - `ffmpeg` (recommended): Uses FFmpeg for remuxing
 *       - `mp4box`: Uses MP4Box as an alternative
 *
 * ## Section 2: Wrapper
 *
 *   - **Use Wrapper** -- Toggle to use a wrapper service for account
 *     authentication instead of cookies. Maps to `settings.use_wrapper`.
 *   - **Wrapper Account URL** -- The endpoint URL for the wrapper service.
 *     Only shown when the wrapper toggle is enabled (conditional render).
 *     Maps to `settings.wrapper_account_url`.
 *
 * ## Section 3: File Options
 *
 *   - **Truncate Filenames** -- Maximum filename length in characters.
 *     When set, filenames exceeding this length are truncated. Maps to
 *     `settings.truncate` (nullable number).
 *   - **Excluded Tags** -- Comma-separated list of metadata tags to strip
 *     from downloaded files (e.g., "lyrics, comment"). Maps to
 *     `settings.exclude_tags: string[]`.
 *
 * ## Section 4: Error Reporting
 *
 *   - **Send Anonymous Crash Reports** -- Toggle for opt-in Sentry
 *     telemetry. Maps to `settings.sentry_enabled`.
 *   - **Recent Crash Reports** -- List of locally saved crash reports
 *     with "Report" (opens GitHub Issue) and "Delete" actions.
 *     Rendered by {@link CrashReportSection}.
 *
 * ## Section 5: Diagnostics
 *
 *   - **Verbose Activity Log** -- Session-only toggle for detailed
 *     [VERBOSE] messages in the Activity Log.
 *
 * ## Section 6: API Credentials
 *
 *   - **MusicKit (Apple Developer)** -- Team ID, Key ID, and private key
 *     for Apple Music API access. Used by animated artwork, metadata
 *     enrichment, and music video companion downloads. Private key stored
 *     in the OS keychain via `storeCredential` / `getCredential` IPC.
 *   - **AcoustID** -- Optional API key override. Release builds ship with
 *     a built-in key; users can provide their own if desired.
 *   - **API Field Audit** -- Developer tool: fetch an album from the
 *     Apple Music API and compare its fields against tags.toml.
 *
 * ## Section 7: Setup
 *
 *   - **Re-run Setup Wizard** -- Button that resets `setup_completed` to
 *     `false`, saves settings, and reloads the app. The setup wizard then
 *     appears on reload to verify and reinstall dependencies.
 *
 * ## Store Connection
 *
 * Reads and writes the Zustand `settingsStore`.
 *
 * @see {@link ../SettingsPage.tsx}        -- Parent container
 * @see {@link @/stores/settingsStore.ts}  -- Zustand store
 * @see {@link @/types/index.ts}           -- DownloadMode, RemuxMode types
 */

import { useState, useEffect, useCallback } from 'react';

// Zustand store hooks. `useSettingsStore` is retained for the
// `saveSettings` and `loadSettings` actions only (used by the setup
// wizard reset and DevToolsSection's deactivate flow). Per-field
// reads/writes go through `useSettingsField` (audit v2 #6).
import { useSettingsStore } from '@/stores/settingsStore';
import { useSettingsField } from '@/hooks/useSettingsField';

// Shared form components: Select for mode dropdowns, Toggle for boolean switches,
// Input for text/number fields, Button for actions.
import { Select, Toggle, Input, Button, HelpButton, SettingsSection } from '@/components/common';

// TypeScript union types for download and remux mode values.
import type { DownloadMode, RemuxMode, WrapperTestResult, ApiAuditResult, LogLevel } from '@/types';

// Platform detection hook for Wrapper feature gating.
import { usePlatform } from '@/hooks/usePlatform';

// IPC commands for wrapper testing, credentials, AcoustID key check, and API audit.
import {
  testWrapperConnection,
  storeCredential,
  getCredential,
  validateMusicKitCredentialsWithInput,
  hasEmbeddedMusicKitToken,
  hasEmbeddedAcoustidKey,
  auditApiFields,
  hasWebplayerToken,
  clearWebplayerToken,
  deactivateDevAccess,
  getLogsFolderPath,
  buildDiagnosticBundle,
  getComponentVersions,
  getNotificationDiagnostics,
  type DiagnosticBundle,
  type NotificationDiagnostics,
} from '@/lib/tauri-commands';
import { useActivityStore } from '@/stores/activityStore';

// Toast notifications for the "Open folder" button failure modes.
import { useUiStore } from '@/stores/uiStore';

// Crash report list sub-component with GitHub reporting functionality.
import { CrashReportSection } from './CrashReportSection';

/**
 * Download mode dropdown options.
 * yt-dlp is the recommended default; N_m3u8DL-RE is provided as an
 * alternative for users who prefer or require it.
 */
const DOWNLOAD_MODE_OPTIONS = [
  { value: 'ytdlp', label: 'yt-dlp (recommended)' },
  { value: 'nm3u8dlre', label: 'N_m3u8DL-RE (alternative)' },
];

/**
 * Remux mode dropdown options.
 * MP4Box is the recommended default — handles subtitle/CC tracks in music
 * videos better than FFmpeg (avoids "Invalid data" errors).
 */
const REMUX_MODE_OPTIONS = [
  { value: 'mp4box', label: 'MP4Box (recommended)' },
  { value: 'ffmpeg', label: 'FFmpeg (alternative)' },
];

/**
 * GAMDL idle-output timeout options, in minutes (#507).
 * Values passed to `settings.gamdl_idle_timeout_minutes`; the watchdog
 * in `services::companion_supervisor` treats this as the max span between
 * stdout/stderr lines during the active-download phase before killing
 * the child. The watchdog pauses automatically once post-processing
 * starts so a slow remux on a network volume doesn't trip it.
 */
const GAMDL_IDLE_TIMEOUT_OPTIONS = [
  { value: '2', label: '2 minutes' },
  { value: '5', label: '5 minutes (default)' },
  { value: '10', label: '10 minutes' },
  { value: '15', label: '15 minutes' },
  { value: '30', label: '30 minutes' },
];

/**
 * AdvancedTab -- Renders the Advanced settings tab.
 *
 * Contains sections: Processing, Wrapper, File Options, Error Reporting,
 * Diagnostics, API Credentials (with API Field Audit), and Setup.
 */
export function AdvancedTab() {
  // Per-field bindings (audit v2 #6).
  const downloadMode = useSettingsField('download_mode');
  const remuxMode = useSettingsField('remux_mode');
  const gamdlIdleTimeout = useSettingsField('gamdl_idle_timeout_minutes');
  const truncate = useSettingsField('truncate');
  const excludeTags = useSettingsField('exclude_tags');
  const sentryEnabled = useSettingsField('sentry_enabled');
  const analyticsEnabled = useSettingsField('analytics_enabled');
  const verboseActivityLog = useSettingsField('verbose_activity_log');
  const verboseGamdlExceptions = useSettingsField('verbose_gamdl_exceptions');
  const activityLogPathOverride = useSettingsField('activity_log_path_override');
  const useWrapper = useSettingsField('use_wrapper');
  const autoRetryWithoutWrapper = useSettingsField('auto_retry_without_wrapper');
  const wrapperAccountUrl = useSettingsField('wrapper_account_url');
  const wrapperM3u8Ip = useSettingsField('wrapper_m3u8_ip');
  const wrapperDecryptIp = useSettingsField('wrapper_decrypt_ip');
  const musickitTeamId = useSettingsField('musickit_team_id');
  const musickitKeyId = useSettingsField('musickit_key_id');
  const acoustidApiKey = useSettingsField('acoustid_api_key');
  const setupCompleted = useSettingsField('setup_completed');
  const devAccessEnabled = useSettingsField('dev_access_enabled');
  /** Persist settings to disk (needed for setup wizard reset) */
  const saveSettings = useSettingsStore((s) => s.saveSettings);
  /** Platform detection for Wrapper feature gating (Linux x86_64 only) */
  const { supportsWrapper } = usePlatform();

  /** Wrapper connection test state */
  const [testState, setTestState] = useState<'idle' | 'testing' | 'success' | 'error'>('idle');
  const [testResult, setTestResult] = useState<WrapperTestResult | null>(null);

  // ── MusicKit credential state ──
  /** Whether a private key is stored in the OS keychain */
  const [keyStored, setKeyStored] = useState(false);
  /** Current private key textarea input */
  const [keyInput, setKeyInput] = useState('');
  /** Status message after saving key to keychain */
  const [keyStatus, setKeyStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');
  /** Whether a credential validation test is in progress */
  const [validating, setValidating] = useState(false);
  /** Result message from the credential validation test */
  const [validationResult, setValidationResult] = useState<string | null>(null);

  // ── AcoustID state ──
  /** Whether a built-in AcoustID API key is available (embedded at compile time) */
  const [hasBuiltInKey, setHasBuiltInKey] = useState(false);
  /** Whether a build-time MusicKit developer token is available */
  const [hasBuiltInMusicKitToken, setHasBuiltInMusicKitToken] = useState(false);

  // ── API Field Audit state ──
  const [auditUrl, setAuditUrl] = useState('');
  const [auditLoading, setAuditLoading] = useState(false);
  const [auditResult, setAuditResult] = useState<ApiAuditResult | null>(null);
  const [auditError, setAuditError] = useState<string | null>(null);
  const [auditExpanded, setAuditExpanded] = useState(false);

  /** Whether MusicKit credentials are configured (required for audit) */
  const hasMusicKitCredentials =
    !!musickitTeamId.value?.trim() && !!musickitKeyId.value?.trim();

  // Reset test result when the wrapper URL changes
  useEffect(() => {
    setTestState('idle');
    setTestResult(null);
  }, [wrapperAccountUrl.value]);

  // Check for stored MusicKit private key on mount
  useEffect(() => {
    getCredential('musickit_private_key')
      .then((val) => setKeyStored(!!val))
      .catch(() => setKeyStored(false));
  }, []);

  // Check for built-in AcoustID key on mount
  useEffect(() => {
    hasEmbeddedAcoustidKey()
      .then(setHasBuiltInKey)
      .catch(() => setHasBuiltInKey(false));
  }, []);

  // Check for build-time MusicKit token on mount
  useEffect(() => {
    hasEmbeddedMusicKitToken()
      .then(setHasBuiltInMusicKitToken)
      .catch(() => setHasBuiltInMusicKitToken(false));
  }, []);

  /** Handles the "Test Connection" button click */
  const handleTestConnection = async () => {
    if (!wrapperAccountUrl.value) return;
    setTestState('testing');
    try {
      const result = await testWrapperConnection(wrapperAccountUrl.value);
      setTestResult(result);
      setTestState(result.reachable ? 'success' : 'error');
    } catch {
      setTestResult(null);
      setTestState('error');
    }
  };

  /** Saves the MusicKit private key to the OS keychain. */
  const handleSaveKey = useCallback(async () => {
    if (!keyInput.trim()) return;
    setKeyStatus('saving');
    try {
      await storeCredential('musickit_private_key', keyInput.trim());
      setKeyStatus('saved');
      setKeyStored(true);
      setKeyInput('');
    } catch {
      setKeyStatus('error');
    }
  }, [keyInput]);

  /** Tests MusicKit credentials by generating a JWT and hitting the Apple Music API. */
  const handleTestCredentials = useCallback(async () => {
    setValidating(true);
    setValidationResult(null);
    try {
      const result = await validateMusicKitCredentialsWithInput(
        musickitTeamId.value ?? null,
        musickitKeyId.value ?? null
      );
      setValidationResult(result);
    } catch (err) {
      setValidationResult(`Error: ${err}`);
    } finally {
      setValidating(false);
    }
  }, [musickitKeyId.value, musickitTeamId.value]);

  /**
   * Opens a URL in the system default browser via the Tauri shell plugin.
   * Used for the AcoustID registration link.
   */
  const openExternal = useCallback(async (url: string) => {
    const { open } = await import('@tauri-apps/plugin-shell');
    await open(url);
  }, []);

  /** Run the API field audit */
  const handleAudit = async () => {
    if (!auditUrl.trim()) return;
    setAuditLoading(true);
    setAuditError(null);
    setAuditResult(null);
    try {
      const result = await auditApiFields(auditUrl.trim());
      setAuditResult(result);
    } catch (err) {
      setAuditError(typeof err === 'string' ? err : String(err));
    } finally {
      setAuditLoading(false);
    }
  };

  return (
    <div className="space-y-3">
      {/* ── Processing ── */}
      <SettingsSection title="Processing" description="Download and remux tool selection.">
        <Select
          label="Download Mode"
          description="Which tool to use for downloading HLS streams"
          options={DOWNLOAD_MODE_OPTIONS}
          value={downloadMode.value}
          onChange={(e) => downloadMode.set(e.target.value as DownloadMode)}
        />
        <Select
          label="Remux Mode"
          description="Which tool to use for video remuxing. MP4Box handles subtitle/CC tracks better."
          options={REMUX_MODE_OPTIONS}
          value={remuxMode.value}
          onChange={(e) => remuxMode.set(e.target.value as RemuxMode)}
        />
        <Select
          label="GAMDL Idle Timeout"
          description="Kill the GAMDL process if no output arrives for this many minutes. The watchdog pauses automatically once post-processing (remux / decrypt) begins, so this won't cut short a slow remux on a network volume."
          options={GAMDL_IDLE_TIMEOUT_OPTIONS}
          value={String(gamdlIdleTimeout.value)}
          onChange={(e) => gamdlIdleTimeout.set(Number(e.target.value))}
        />
      </SettingsSection>

      {/* ── File Options ── */}
      <SettingsSection title="File Options" description="Filename truncation and metadata tag exclusions.">
        <Input
          label="Truncate Filenames"
          description="Maximum filename length (leave empty for no limit)"
          type="number"
          min={10}
          max={255}
          value={truncate.value?.toString() ?? ''}
          placeholder="No limit"
          onChange={(e) => {
            const val = e.target.value;
            truncate.set(val ? parseInt(val, 10) : null);
          }}
        />
        <Input
          label="Excluded Tags"
          description="Comma-separated list of metadata tags to exclude from downloaded files"
          value={excludeTags.value.join(', ')}
          placeholder="e.g., lyrics, comment"
          onChange={(e) => {
            const tags = e.target.value
              .split(',')
              .map((t) => t.trim())
              .filter(Boolean);
            excludeTags.set(tags);
          }}
        />
      </SettingsSection>

      {/* ── Error Reporting ── */}
      <SettingsSection title="Error Reporting" description="Crash report settings and local error history.">
        <Toggle
          label="Send Anonymous Crash Reports"
          description="When enabled, anonymous crash data (error message, stack trace, app version, OS) is sent to our error tracking service to help identify and fix bugs. No personal data, download history, or account information is ever included. Requires an app restart to take effect."
          checked={sentryEnabled.value}
          onChange={sentryEnabled.set}
        />
        <Toggle
          label="Anonymous Usage Analytics"
          description="Send anonymised feature usage data (which features are used, platform, download counts) to help prioritise development. No personal data, URLs, or content information is ever collected."
          checked={analyticsEnabled.value ?? false}
          onChange={analyticsEnabled.set}
        />
        <p className="text-xs text-content-tertiary">
          Error reports (crashes and download failures) are always saved locally to your app data
          directory regardless of this setting. You can view and report them below.
        </p>
        <CrashReportSection />
      </SettingsSection>

      {/* ── Diagnostics ── */}
      <SettingsSection title="Diagnostics" description="Verbose logging for troubleshooting.">
        {/* Native notification diagnostic readout (#834). Surfaces the
            current notification configuration AND the OS-level
            permission state so users on macOS — where `sendNotification`
            can succeed silently while the OS drops the notification —
            can see at a glance whether the toggle / style / permission
            are aligned. Pure read-only: NEVER triggers a permission
            prompt (use Send Test Notification in General for that). */}
        <NotificationDiagnosticsRow />

        <Toggle
          label="Verbose Activity Log"
          description="Emits detailed [VERBOSE] messages to the Activity Log for issue tracking and debugging. In pre-release versions (v0.x.x), this setting is preserved across restarts. In full releases, it resets to off on each restart as a safety measure."
          checked={verboseActivityLog.value}
          onChange={verboseActivityLog.set}
        />
        {verboseActivityLog.value && (
          <div className="p-3 rounded-lg bg-status-warning-bg border border-status-warning">
            <p className="text-xs font-semibold text-status-warning mb-1">
              Sensitive Data Warning
            </p>
            <p className="text-xs text-status-warning">
              Verbose logging includes detailed information that may contain sensitive data such as
              cookie file paths/values, wrapper URLs with authentication tokens, Apple Music API responses,
              MusicKit credentials, and full download URLs. Disable this setting before sharing
              activity logs with others.
            </p>
            <p className="text-xs text-status-warning mt-2">
              In pre-release versions (v0.x.x), this setting is preserved across restarts to aid
              debugging. In full releases, it automatically resets to off on restart. You may need
              to re-enable it each session in full release builds.
            </p>
          </div>
        )}
        <Toggle
          label="Show Full GAMDL Tracebacks"
          description="Stops MeedyaDL from passing --no-exceptions to GAMDL, so uncaught Python exceptions include the full stack trace instead of a single user-facing line. Useful for reporting bugs against upstream GAMDL; noisy for everyday use because v3.0 structlog lines interleave with raw tracebacks."
          checked={verboseGamdlExceptions.value}
          onChange={verboseGamdlExceptions.set}
        />

        {/* ── On-disk activity log location (#541) ── */}
        <div className="pt-2">
          <div className="flex items-center justify-between mb-1">
            <label className="text-sm font-medium text-content-primary">
              On-disk activity log location
            </label>
            <div className="flex items-center gap-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={async () => {
                  try {
                    const { open } = await import('@tauri-apps/plugin-dialog');
                    const selected = await open({
                      multiple: false,
                      directory: true,
                      title: 'Choose a folder for MeedyaDL activity logs',
                    });
                    if (typeof selected === 'string' && selected.length > 0) {
                      activityLogPathOverride.set(selected);
                    }
                  } catch {
                    /* User cancelled */
                  }
                }}
              >
                Browse…
              </Button>
              {/*
                Open the currently-configured log folder in the OS
                native file viewer (Finder / Explorer / Nautilus). Uses
                the same `get_logs_folder_path` IPC + `plugin-shell`
                open() pattern that powers the Activity Log page's
                "Reveal" button, so the behaviour is consistent across
                the two entry points (#581).
              */}
              <Button
                variant="secondary"
                size="sm"
                onClick={async () => {
                  const addToast = useUiStore.getState().addToast;
                  try {
                    const path = await getLogsFolderPath();
                    const { open } = await import('@tauri-apps/plugin-shell');
                    await open(path);
                  } catch (err) {
                    const msg = err instanceof Error ? err.message : String(err);
                    addToast(`Failed to open logs folder: ${msg}`, 'error');
                  }
                }}
              >
                Open folder
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={!activityLogPathOverride.value}
                onClick={() => activityLogPathOverride.set('')}
              >
                Reset
              </Button>
            </div>
          </div>
          <Input
            value={activityLogPathOverride.value}
            onChange={(e) => activityLogPathOverride.set(e.target.value)}
            placeholder="Default — uses the app data directory"
            aria-label="On-disk activity log path override"
          />
          <p className="text-xs text-content-secondary mt-1">
            Optional. Leave blank to store on-disk activity logs in the default
            location alongside the tracing log. Pointing at an external drive
            can save space on the system disk. Files older than 7 days are
            pruned automatically. <strong>Applies on the next app restart.</strong>
          </p>
        </div>

        {/* Diagnostic bundle (#572 Phase 1) */}
        <DiagnosticBundleSection />
      </SettingsSection>

      {/* ── Setup ── */}
      <SettingsSection title="Setup" description="Re-run the first-time setup wizard.">
        <p className="text-xs text-content-secondary">
          Verify and reinstall dependencies. Your existing settings will be preserved.
        </p>
        <Button
          variant="secondary"
          size="sm"
          onClick={async () => {
            const confirmed = window.confirm(
              'This will reset the setup wizard flag and reload the app. ' +
                'Your settings will be preserved, but the setup wizard will ' +
                'appear on next load to verify your dependencies. Continue?'
            );
            if (!confirmed) return;
            setupCompleted.set(false);
            try {
              await saveSettings();
            } catch {
              /* Reload anyway — in-memory state has setup_completed: false */
            }
            window.location.reload();
          }}
        >
          Re-run Setup Wizard
        </Button>
      </SettingsSection>

      {/* ── Wrapper ── */}
      <SettingsSection title="Wrapper" description="Alternative authentication via a locally-running wrapper service." defaultOpen={false}>
        {!supportsWrapper && (
          <p className="text-xs text-content-tertiary bg-surface-secondary rounded-platform p-2">
            The Wrapper service only provides native binaries for Linux x86_64. On this platform you
            would need to run Wrapper remotely (e.g. on a Linux server or via Docker) and point the
            URL below to it. See Help &gt; Wrapper for details.
          </p>
        )}

        <Toggle
          label="Use Wrapper"
          description="Use a locally-running wrapper service for authentication instead of browser cookies. The wrapper handles Apple ID login and DRM key exchange, providing more reliable access to Dolby Atmos and other protected formats. Most users should leave this disabled and use cookies instead."
          checked={useWrapper.value}
          onChange={useWrapper.set}
          helpTopic="wrapper"
        />

        {useWrapper.value && (
          <>
            <Toggle
              label="Auto-Retry without Wrapper"
              description="When a download fails after exhausting all retries, automatically re-queue it with wrapper disabled (falls back to cookie-based authentication). Without this, you'll be prompted to retry manually."
              checked={autoRetryWithoutWrapper.value}
              onChange={autoRetryWithoutWrapper.set}
            />
            <Input
              label="Wrapper Account URL"
              description="URL of your locally-running wrapper service. The default (http://127.0.0.1:30020) works if the wrapper is running on your machine with default settings. See Help > Wrapper for setup instructions."
              value={wrapperAccountUrl.value}
              onChange={(e) => wrapperAccountUrl.set(e.target.value)}
            />
            <Input
              label="Wrapper m3u8 Address"
              description="GAMDL 3.1+ fetches the HLS master playlist URL from a TCP socket on this address (default: 127.0.0.1:20020). Your wrapper must expose an m3u8 service on this host:port. Ignored on GAMDL 3.0 and earlier."
              value={wrapperM3u8Ip.value}
              onChange={(e) => wrapperM3u8Ip.set(e.target.value)}
              placeholder="127.0.0.1:20020"
            />
            <Input
              label="Wrapper Decryption Address"
              description="GAMDL opens a TCP connection to this address to send encrypted samples for FairPlay decryption (default: 127.0.0.1:10020). Set this to your wrapper's host:port — required when the wrapper runs on a different machine than MeedyaDL (e.g. a Raspberry Pi on the same LAN). Use the same host as the m3u8 address; the port differs (10020 vs 20020)."
              value={wrapperDecryptIp.value}
              onChange={(e) => wrapperDecryptIp.set(e.target.value)}
              placeholder="127.0.0.1:10020"
            />
            <div className="flex items-center gap-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={handleTestConnection}
                disabled={testState === 'testing' || !wrapperAccountUrl.value}
              >
                {testState === 'testing' ? 'Testing...' : 'Test Connection'}
              </Button>
              {testState === 'success' && testResult && (
                <span className="text-xs text-status-success">
                  Connected ({testResult.response_time_ms}ms)
                </span>
              )}
              {testState === 'error' && (
                <span className="text-xs text-status-error">
                  {testResult?.error || 'Connection failed'}
                </span>
              )}
            </div>
          </>
        )}
      </SettingsSection>

      {/* ── API Credentials ── */}
      <SettingsSection title="API Credentials" description="MusicKit, AcoustID, and developer tools." defaultOpen={false}>
        {/* MusicKit */}
        <div className="space-y-3">
          <div className="flex items-center gap-2">
            <h4 className="text-sm font-medium text-content-secondary">MusicKit (Apple Developer)</h4>
            <HelpButton topic="animated-artwork" />
          </div>
          <p className="text-xs text-content-tertiary">
            Required for animated artwork, API metadata enrichment, and music video companion
            downloads. Get your credentials from an{' '}
            <button
              type="button"
              className="text-accent hover:text-accent-hover underline transition-colors"
              onClick={() => openExternal('https://developer.apple.com/account/resources/authkeys/list')}
            >
              Apple Developer account
            </button>
            .
          </p>
          {hasBuiltInMusicKitToken && (
            <div className="rounded-platform border border-status-info/40 bg-status-info/10 px-3 py-2 text-xs text-status-info">
              A build-time MusicKit developer token is embedded in this release. Most end users do
              not need to enter Apple Developer credentials unless they want to override the built-in
              token for testing.
            </div>
          )}
          <Input
            label="MusicKit Team ID"
            description="Your Apple Developer Team ID (10-character alphanumeric)"
            value={musickitTeamId.value ?? ''}
            placeholder="XXXXXXXXXX"
            onChange={(e) => musickitTeamId.set(e.target.value.toUpperCase() || null)}
          />
          <Input
            label="MusicKit Key ID"
            description="The Key ID for your MusicKit private key (10-character alphanumeric)"
            value={musickitKeyId.value ?? ''}
            placeholder="XXXXXXXXXX"
            onChange={(e) => musickitKeyId.set(e.target.value.toUpperCase() || null)}
          />
          <div className="space-y-2">
            <label className="block text-sm font-medium text-content-primary">
              MusicKit Private Key (.p8)
            </label>
            <p className="text-xs text-content-secondary">
              {keyStored
                ? 'A private key is stored in your OS keychain. Paste a new key below to replace it.'
                : 'Paste the contents of your .p8 private key file. Stored securely in the OS keychain, not in settings files.'}
            </p>
            <textarea
              value={keyInput}
              onChange={(e) => {
                setKeyInput(e.target.value);
                setKeyStatus('idle');
              }}
              placeholder="-----BEGIN PRIVATE KEY-----&#10;...&#10;-----END PRIVATE KEY-----"
              rows={4}
              className="w-full rounded-platform border border-border bg-surface-secondary px-3 py-2 text-xs font-mono text-content-primary placeholder:text-content-tertiary focus:outline-none focus:ring-1 focus:ring-accent resize-none"
            />
            <div className="flex items-center gap-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={handleSaveKey}
                disabled={!keyInput.trim() || keyStatus === 'saving'}
              >
                {keyStatus === 'saving' ? 'Saving...' : 'Save to Keychain'}
              </Button>
              {keyStatus === 'saved' && (
                <span className="text-xs text-status-success">Saved to keychain</span>
              )}
              {keyStatus === 'error' && (
                <span className="text-xs text-status-error">Failed to save</span>
              )}
              {keyStored && keyStatus === 'idle' && (
                <span className="text-xs text-status-success">Key stored in keychain</span>
              )}
            </div>
          </div>
          <div className="flex items-start gap-2">
            <Button
              className="shrink-0"
              variant="secondary"
              size="sm"
              onClick={handleTestCredentials}
              disabled={
                validating ||
                !musickitTeamId.value?.trim() ||
                !musickitKeyId.value?.trim() ||
                !keyStored
              }
            >
              {validating ? 'Testing...' : 'Test Credentials'}
            </Button>
            {validationResult && (
              <span
                className={`text-xs leading-relaxed pt-1 ${validationResult.startsWith('Error') || validationResult.startsWith('error') ? 'text-status-error' : 'text-status-success'}`}
              >
                {validationResult}
              </span>
            )}
          </div>
        </div>

        <div className="border-t border-border" />

        {/* AcoustID */}
        <div className="space-y-3">
          <h4 className="text-sm font-medium text-content-secondary">AcoustID</h4>
          <Input
            label={hasBuiltInKey ? 'AcoustID API Key (Optional Override)' : 'AcoustID API Key'}
            description={
              hasBuiltInKey ? (
                <>
                  A built-in API key is included with this release. You can optionally override it
                  with your own key registered at{' '}
                  <button
                    type="button"
                    className="text-accent hover:text-accent-hover underline transition-colors"
                    onClick={(e) => {
                      e.preventDefault();
                      openExternal('https://acoustid.org/new-application');
                    }}
                  >
                    acoustid.org/new-application
                  </button>
                  . Leave blank to use the built-in key.
                </>
              ) : (
                <>
                  Register a free application API key at{' '}
                  <button
                    type="button"
                    className="text-accent hover:text-accent-hover underline transition-colors"
                    onClick={(e) => {
                      e.preventDefault();
                      openExternal('https://acoustid.org/new-application');
                    }}
                  >
                    acoustid.org/new-application
                  </button>
                  . Required for AcoustID lookups.
                </>
              )
            }
            value={acoustidApiKey.value ?? ''}
            placeholder={hasBuiltInKey ? 'Using built-in key' : 'Your AcoustID application API key'}
            onChange={(e) => acoustidApiKey.set(e.target.value)}
          />
        </div>

        <div className="border-t border-border" />

        {/* API Field Audit */}
        <div>
          <button
            type="button"
            className="flex items-center gap-2 text-sm font-medium text-content-secondary mb-2"
            onClick={() => setAuditExpanded(!auditExpanded)}
          >
            <span className="text-sm text-content-tertiary">{auditExpanded ? '▼' : '▶'}</span>
            API Field Audit
          </button>
          <p className="text-xs text-content-tertiary leading-relaxed mb-4">
            Developer tool: fetch an album from the Apple Music API and compare its fields against the
            known tag definitions in tags.toml. Discovers new or unknown API fields.
          </p>

          {auditExpanded && (
            <div className="space-y-4">
              <div className="flex gap-2">
                <Input
                  label=""
                  value={auditUrl}
                  placeholder="https://music.apple.com/us/album/.../1234567890"
                  onChange={(e) => setAuditUrl(e.target.value)}
                  className="flex-1"
                />
                <button
                  type="button"
                  className="px-4 py-2 text-sm font-medium rounded-md bg-accent text-white hover:bg-accent-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed self-end"
                  onClick={handleAudit}
                  disabled={auditLoading || !auditUrl.trim() || !hasMusicKitCredentials}
                >
                  {auditLoading ? 'Auditing...' : 'Audit'}
                </button>
              </div>
              {!hasMusicKitCredentials && (
                <p className="text-sm text-status-warning">
                  MusicKit credentials required. Configure Team ID and Key ID above.
                </p>
              )}
              {auditError && (
                <p className="text-sm text-status-error">{auditError}</p>
              )}
              {auditResult && (
                <div className="space-y-3 text-sm">
                  <div className="flex gap-4 flex-wrap">
                    <span className="text-content-primary font-medium">
                      {auditResult.album_name ?? auditResult.album_id}
                    </span>
                    <span className="text-content-tertiary">
                      {auditResult.track_count} tracks
                    </span>
                  </div>
                  <div className="flex gap-4 flex-wrap text-sm">
                    <span className="px-2 py-0.5 rounded bg-green-500/20 text-green-400">
                      {auditResult.known_fields.length} known
                    </span>
                    <span className="px-2 py-0.5 rounded bg-amber-500/20 text-amber-400">
                      {auditResult.unknown_fields.length} unknown
                    </span>
                    <span className="px-2 py-0.5 rounded bg-gray-500/20 text-gray-400">
                      {auditResult.missing_fields.length} missing
                    </span>
                  </div>
                  {auditResult.unknown_fields.length > 0 && (
                    <div>
                      <h4 className="text-sm font-medium text-amber-400 mb-1">
                        Unknown Fields (not in tags.toml)
                      </h4>
                      <div className="max-h-48 overflow-y-auto bg-surface-secondary rounded p-2 space-y-1">
                        {auditResult.unknown_fields.map((field) => (
                          <div
                            key={`${field.scope}-${field.json_path}`}
                            className="text-xs font-mono text-content-secondary"
                          >
                            <span className="text-content-tertiary">[{field.scope}]</span>{' '}
                            {field.json_path}{' '}
                            <span className="text-content-tertiary">({field.value_type})</span>
                            {field.sample_value && (
                              <span className="text-content-tertiary ml-1">
                                = {field.sample_value.length > 60
                                  ? `${field.sample_value.slice(0, 60)}...`
                                  : field.sample_value}
                              </span>
                            )}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                  {auditResult.missing_fields.length > 0 && (
                    <div>
                      <h4 className="text-sm font-medium text-gray-400 mb-1">
                        Missing Fields (in tags.toml but not in API response)
                      </h4>
                      <div className="text-xs font-mono text-content-tertiary">
                        {auditResult.missing_fields.join(', ')}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </SettingsSection>

      {/* ── Developer Tools (hidden unless dev access is active) ──── */}
      {devAccessEnabled.value && (
        <DevToolsSection />
      )}
    </div>
  );
}

/**
 * DevToolsSection -- Internal developer tools panel.
 *
 * Only rendered when `dev_access_enabled` is true (activated via Konami code).
 * Shows token status, web player token management, and a deactivate button.
 */

/**
 * Native notification configuration + OS permission diagnostic readout (#834).
 *
 * Compact 4-row table showing:
 *   - "Desktop Notifications" toggle state (from settings)
 *   - "Notification Style" selection (from settings)
 *   - Platform (`darwin` / `linux` / `windows-style`)
 *   - OS-level permission grant (from the JS plugin's `isPermissionGranted`)
 *
 * Lives in Settings → Advanced → Diagnostics rather than Settings → General
 * (where the Send Test Notification button is) because Advanced is where
 * users go to *diagnose* issues. General is for everyday tuning.
 *
 * Pure read-only: does NOT call `requestPermission()` — only
 * `isPermissionGranted()` which silently returns the current cached
 * state without prompting. Triggering a prompt from a diagnostic
 * readout would confuse users who just wanted to see their config.
 */
function NotificationDiagnosticsRow() {
  const [diag, setDiag] = useState<NotificationDiagnostics | null>(null);
  const [osPermission, setOsPermission] = useState<string>('unknown');
  const [error, setError] = useState<string | null>(null);

  // Load both halves on mount: the backend snapshot and the JS-side
  // permission state. Wrapped in a single async IIFE so errors from
  // either source land in the same `setError` and we don't render a
  // half-loaded readout.
  useEffect(() => {
    (async () => {
      try {
        const backend = await getNotificationDiagnostics();
        setDiag(backend);
      } catch (e) {
        setError(`Failed to load backend diagnostics: ${e instanceof Error ? e.message : String(e)}`);
        return;
      }
      try {
        const { isPermissionGranted } = await import('@tauri-apps/plugin-notification');
        const granted = await isPermissionGranted();
        setOsPermission(granted ? 'granted' : 'not granted');
      } catch (e) {
        setOsPermission(`error: ${e instanceof Error ? e.message : String(e)}`);
      }
    })();
  }, []);

  if (error) {
    return (
      <div className="p-3 rounded-lg bg-status-error-bg border border-status-error">
        <p className="text-xs text-status-error">{error}</p>
      </div>
    );
  }

  if (!diag) {
    return (
      <div className="text-xs text-content-tertiary">Loading notification diagnostics…</div>
    );
  }

  // Pretty-format the style key — the backend stores it as snake_case
  // so the diagnostics readout matches what the dropdown shows in the
  // General tab. Falls back to the raw key for unknown values so a
  // future style enum addition doesn't render as a blank cell.
  const styleLabels: Record<string, string> = {
    in_app_only: 'In-app only',
    native_and_in_app: 'Native + in-app',
    native_only: 'Native only',
  };
  const styleLabel = styleLabels[diag.notification_style] ?? diag.notification_style;

  return (
    <div className="p-3 rounded-lg bg-surface-secondary border border-border-default">
      <p className="text-xs font-semibold text-content-primary mb-2">
        Native notification diagnostics (#834)
      </p>
      <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
        <span className="text-content-tertiary">Desktop Notifications</span>
        <span className="text-content-primary">
          {diag.desktop_notifications_enabled ? 'ON' : 'OFF'}
        </span>
        <span className="text-content-tertiary">Notification Style</span>
        <span className="text-content-primary">{styleLabel}</span>
        <span className="text-content-tertiary">Platform</span>
        <span className="text-content-primary">{diag.platform}</span>
        <span className="text-content-tertiary">OS Permission</span>
        <span
          className={
            osPermission === 'granted'
              ? 'text-status-success'
              : osPermission === 'not granted'
                ? 'text-status-warning'
                : 'text-content-primary'
          }
        >
          {osPermission}
        </span>
      </div>
      <p className="text-xs text-content-tertiary mt-2">
        If notifications aren't appearing despite all four rows looking correct, check
        macOS System Settings → Notifications → MeedyaDL, and ensure Focus / Do Not
        Disturb is off. Use <strong>Send Test Notification</strong> in Settings → General
        to verify the full pipeline end-to-end.
      </p>
    </div>
  );
}

function DevToolsSection() {
  // Per-field bindings for the two MusicKit fields read by this
  // section. `loadSettings` retained because the deactivate flow
  // needs to refresh the entire settings snapshot from disk.
  const musickitTeamId = useSettingsField('musickit_team_id');
  const musickitKeyId = useSettingsField('musickit_key_id');
  // GAMDL --log-level binding (#768). Lives in Dev Tools only; default
  // INFO matches GAMDL's compiled-in level so this is a no-op for
  // every user who never opens this panel.
  const gamdlLogLevel = useSettingsField('gamdl_log_level');
  const loadSettings = useSettingsStore((s) => s.loadSettings);

  const [webplayerStatus, setWebplayerStatus] = useState<boolean | null>(null);
  const [embeddedStatus, setEmbeddedStatus] = useState<boolean | null>(null);

  // Check token status on mount.
  useEffect(() => {
    hasWebplayerToken().then(setWebplayerStatus).catch(() => setWebplayerStatus(false));
    hasEmbeddedMusicKitToken().then(setEmbeddedStatus).catch(() => setEmbeddedStatus(false));
  }, []);

  const hasUserCreds =
    !!musickitTeamId.value?.trim() && !!musickitKeyId.value?.trim();

  const handleClearWebplayerToken = async () => {
    await clearWebplayerToken();
    setWebplayerStatus(false);
  };

  const handleDeactivate = async () => {
    await deactivateDevAccess();
    loadSettings();
  };

  return (
    <SettingsSection
      title="Developer Tools"
      description="Internal diagnostics and token management."
    >
      <div className="space-y-4">
        {/* Token resolution hierarchy status */}
        <div>
          <h4 className="text-sm font-medium mb-2">MusicKit Token Resolution</h4>
          <div className="space-y-1 text-xs">
            <div className="flex items-center gap-2">
              <span className={hasUserCreds ? 'text-green-500' : 'text-gray-400'}>
                {hasUserCreds ? '\u2713' : '\u2717'}
              </span>
              <span>Priority 1: User credentials (Team ID + Key ID + .p8)</span>
            </div>
            <div className="flex items-center gap-2">
              <span className={embeddedStatus ? 'text-green-500' : 'text-gray-400'}>
                {embeddedStatus ? '\u2713' : '\u2717'}
              </span>
              <span>Priority 2: Embedded build token</span>
            </div>
            <div className="flex items-center gap-2">
              <span className={webplayerStatus ? 'text-green-500' : 'text-gray-400'}>
                {webplayerStatus ? '\u2713' : '\u2717'}
              </span>
              <span>Priority 3: Web session token</span>
              {webplayerStatus && (
                <button
                  type="button"
                  onClick={handleClearWebplayerToken}
                  className="ml-2 text-xs text-red-400 hover:text-red-300 underline"
                >
                  Clear
                </button>
              )}
            </div>
          </div>
        </div>

        {/*
          GAMDL --log-level dropdown (#768). GAMDL accepts
          DEBUG / INFO / WARNING / ERROR on every release in our
          support window, so no version gate is needed. Flipping to
          DEBUG surfaces v3.5.2+ structlog diagnostics (e.g.
          `m3u8_master_url=...`) that are otherwise invisible to
          MeedyaDL because GAMDL's default level is INFO.

          Activity-log noise consideration: at DEBUG GAMDL emits
          ~10x more lines per track. Users debugging a specific
          issue will want this; everyone else should leave it on
          INFO. Hence its placement in Dev Tools rather than the
          user-facing Settings tabs.
        */}
        <div>
          <h4 className="text-sm font-medium mb-2">GAMDL log level</h4>
          <Select
            options={[
              { value: 'DEBUG', label: 'DEBUG — verbose (HTTP, decryption, m3u8 URLs)' },
              { value: 'INFO', label: 'INFO — recommended (track names, progress)' },
              { value: 'WARNING', label: 'WARNING — warnings + errors only' },
              { value: 'ERROR', label: 'ERROR — fatal errors only' },
            ]}
            value={gamdlLogLevel.value}
            onChange={(e) => gamdlLogLevel.set(e.target.value as LogLevel)}
            aria-label="GAMDL subprocess log level"
          />
          <p className="text-xs text-content-secondary mt-1">
            Emits <code>--log-level</code> to the gamdl subprocess. Default
            <code> INFO</code> matches gamdl's compiled-in default. Switch to
            <code> DEBUG</code> when filing upstream bug reports — it surfaces
            v3.5.2+ structlog lines (e.g. <code>m3u8_master_url=</code>) that
            are otherwise hidden, at the cost of ~10× more activity-log
            output per track.
          </p>
        </div>

        {/* Deactivate */}
        <button
          type="button"
          onClick={handleDeactivate}
          className="rounded border border-red-500 px-3 py-1.5 text-xs text-red-400 hover:bg-red-500/10"
        >
          Deactivate Developer Access
        </button>
      </div>
    </SettingsSection>
  );
}

/**
 * Diagnostic bundle section (#572 Phase 1 MVP).
 *
 * Renders a "Generate Diagnostic Bundle" button. On click, captures
 * the last 200 activity-log entries + the component versions + the
 * user's free-text summary, composes a redacted Markdown bundle on
 * the backend, and opens a review modal where the user can:
 *
 *   - Read the full bundle before sharing it.
 *   - Copy the Markdown to clipboard for inclusion in an existing
 *     thread.
 *   - Click "Open GitHub Issue" to pop the system browser to a
 *     pre-filled new-issue form. User reviews + clicks GitHub's
 *     own Submit button — never auto-submitted from MeedyaDL.
 *
 * Privacy: the backend redacts credential fields, username paths,
 * and never reads file contents. The review modal is the user's
 * second-chance veto.
 */
function DiagnosticBundleSection() {
  const addToast = useUiStore((s) => s.addToast);
  const recentEntries = useActivityStore((s) => s.entries);
  const [summary, setSummary] = useState('');
  const [bundle, setBundle] = useState<DiagnosticBundle | null>(null);
  const [busy, setBusy] = useState(false);

  const handleGenerate = async () => {
    setBusy(true);
    try {
      // Pull the last 200 entries — caps bundle size at ~50 KB
      // typical, well below the GitHub issue body limit.
      const slice = recentEntries
        .slice(-200)
        .map((e) => `${e.timestamp} [${e.download_id}] [${e.stream}] ${e.line}`);
      const versions = await getComponentVersions().catch(() => []);
      const result = await buildDiagnosticBundle({
        activity_log_lines: slice,
        component_versions: versions.map((v) => ({
          name: v.name,
          version: v.version ?? 'unknown',
        })),
        user_summary: summary.trim() || null,
      });
      setBundle(result);
    } catch (e) {
      addToast(
        `Diagnostic bundle failed: ${e instanceof Error ? e.message : String(e)}`,
        'error',
      );
    } finally {
      setBusy(false);
    }
  };

  const handleCopy = async () => {
    if (!bundle) return;
    try {
      await navigator.clipboard.writeText(bundle.markdown_body);
      addToast('Bundle copied to clipboard.', 'success');
    } catch (e) {
      addToast(
        `Copy failed: ${e instanceof Error ? e.message : String(e)}`,
        'error',
      );
    }
  };

  const handleOpenIssue = async () => {
    if (!bundle) return;
    try {
      const { open } = await import('@tauri-apps/plugin-shell');
      await open(bundle.github_issue_url);
    } catch (e) {
      addToast(
        `Failed to open browser: ${e instanceof Error ? e.message : String(e)}`,
        'error',
      );
    }
  };

  return (
    <div className="pt-2 border-t border-border-light mt-2">
      <h4 className="text-sm font-medium text-content-primary mb-1">
        Diagnostic Bundle
      </h4>
      <p className="text-xs text-content-secondary mb-2">
        Captures the last 200 activity-log entries, component versions,
        a redacted settings snapshot, and your output-directory tree
        into a Markdown bundle suitable for pasting into a GitHub
        issue. Credentials, file contents, and keychain material are
        never included. Username paths are redacted to{' '}
        <code>/Users/{'{user}'}</code>.
      </p>
      <Input
        value={summary}
        onChange={(e) => setSummary(e.target.value)}
        placeholder="Optional: short summary (e.g. 'MV download fails for AnneMarie album')"
        aria-label="Diagnostic bundle summary"
      />
      <div className="mt-2">
        <Button
          variant="secondary"
          size="sm"
          loading={busy}
          disabled={busy}
          onClick={handleGenerate}
        >
          Generate Diagnostic Bundle
        </Button>
      </div>

      {bundle && (
        <div
          className="fixed inset-0 z-50 bg-black/40 flex items-center justify-center"
          onClick={() => setBundle(null)}
          role="dialog"
          aria-modal="true"
        >
          <div
            className="bg-surface-primary border border-border-light rounded-platform p-5 max-w-3xl w-[90vw] max-h-[85vh] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-base font-semibold text-content-primary">
                Review diagnostic bundle ({Math.round(bundle.size_bytes / 1024)} KB)
              </h3>
              <Button variant="ghost" size="sm" onClick={() => setBundle(null)}>
                Close
              </Button>
            </div>
            <p className="text-xs text-content-secondary mb-3">
              Review the contents below before sharing. Click <strong>Open
              GitHub Issue</strong> to pop the new-issue form in your
              browser — MeedyaDL never auto-submits.
            </p>
            <pre className="text-xs bg-surface-elevated border border-border-light rounded-platform p-3 overflow-auto max-h-[55vh] whitespace-pre-wrap break-all">
              {bundle.markdown_body}
            </pre>
            <div className="flex justify-end gap-2 mt-3">
              <Button variant="ghost" size="sm" onClick={handleCopy}>
                Copy to clipboard
              </Button>
              <Button variant="primary" size="sm" onClick={handleOpenIssue}>
                Open GitHub Issue
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
