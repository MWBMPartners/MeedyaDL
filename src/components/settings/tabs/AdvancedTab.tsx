/**
 * Copyright (c) 2024-2026 MeedyaDL
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

// Zustand store for reading/writing advanced settings.
import { useSettingsStore } from '@/stores/settingsStore';

// Shared form components: Select for mode dropdowns, Toggle for boolean switches,
// Input for text/number fields, Button for actions.
import { Select, Toggle, Input, Button, HelpButton } from '@/components/common';

// TypeScript union types for download and remux mode values.
import type { DownloadMode, RemuxMode, WrapperTestResult, ApiAuditResult } from '@/types';

// Platform detection hook for Wrapper feature gating.
import { usePlatform } from '@/hooks/usePlatform';

// IPC commands for wrapper testing, credentials, AcoustID key check, and API audit.
import {
  testWrapperConnection,
  storeCredential,
  getCredential,
  validateMusicKitCredentials,
  hasEmbeddedAcoustidKey,
  auditApiFields,
} from '@/lib/tauri-commands';

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
 * AdvancedTab -- Renders the Advanced settings tab.
 *
 * Contains sections: Processing, Wrapper, File Options, Error Reporting,
 * Diagnostics, API Credentials (with API Field Audit), and Setup.
 */
export function AdvancedTab() {
  /** Current settings snapshot */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting advanced setting changes */
  const updateSettings = useSettingsStore((s) => s.updateSettings);
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

  // ── API Field Audit state ──
  const [auditUrl, setAuditUrl] = useState('');
  const [auditLoading, setAuditLoading] = useState(false);
  const [auditResult, setAuditResult] = useState<ApiAuditResult | null>(null);
  const [auditError, setAuditError] = useState<string | null>(null);
  const [auditExpanded, setAuditExpanded] = useState(false);

  /** Whether MusicKit credentials are configured (required for audit) */
  const hasMusicKitCredentials =
    !!settings.musickit_team_id?.trim() && !!settings.musickit_key_id?.trim();

  // Reset test result when the wrapper URL changes
  useEffect(() => {
    setTestState('idle');
    setTestResult(null);
  }, [settings?.wrapper_account_url]);

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

  /** Handles the "Test Connection" button click */
  const handleTestConnection = async () => {
    if (!settings?.wrapper_account_url) return;
    setTestState('testing');
    try {
      const result = await testWrapperConnection(settings.wrapper_account_url);
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
      const result = await validateMusicKitCredentials();
      setValidationResult(result);
    } catch (err) {
      setValidationResult(`Error: ${err}`);
    } finally {
      setValidating(false);
    }
  }, []);

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
    <div className="space-y-6 max-w-xl">
      {/* Section: Processing */}
      <div className="space-y-4">
        <h3 className="text-base font-semibold text-content-primary mb-4">Processing</h3>

        {/* Download mode */}
        <Select
          label="Download Mode"
          description="Which tool to use for downloading HLS streams"
          options={DOWNLOAD_MODE_OPTIONS}
          value={settings.download_mode}
          onChange={(e) => updateSettings({ download_mode: e.target.value as DownloadMode })}
        />

        {/* Remux mode */}
        <Select
          label="Remux Mode"
          description="Which tool to use for video remuxing. MP4Box handles subtitle/CC tracks better."
          options={REMUX_MODE_OPTIONS}
          value={settings.remux_mode}
          onChange={(e) => updateSettings({ remux_mode: e.target.value as RemuxMode })}
        />
      </div>

      {/* Section: Wrapper */}
      <div className="space-y-4">
        <h3 className="text-base font-semibold text-content-primary mb-4">Wrapper</h3>

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
          checked={settings.use_wrapper}
          onChange={(checked) => updateSettings({ use_wrapper: checked })}
          helpTopic="wrapper"
        />

        {settings.use_wrapper && (
          <>
            <Toggle
              label="Auto-Retry without Wrapper"
              description="When a download fails after exhausting all retries, automatically re-queue it with wrapper disabled (falls back to cookie-based authentication). Without this, you'll be prompted to retry manually."
              checked={settings.auto_retry_without_wrapper}
              onChange={(checked) => updateSettings({ auto_retry_without_wrapper: checked })}
            />

            <Input
              label="Wrapper Account URL"
              description="URL of your locally-running wrapper service. The default (http://127.0.0.1:30020) works if the wrapper is running on your machine with default settings. See Help > Wrapper for setup instructions."
              value={settings.wrapper_account_url}
              onChange={(e) => updateSettings({ wrapper_account_url: e.target.value })}
            />

            {/* Test Connection button with inline result display */}
            <div className="flex items-center gap-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={handleTestConnection}
                disabled={testState === 'testing' || !settings.wrapper_account_url}
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
      </div>

      {/* Section: File Options */}
      <div className="space-y-4">
        <h3 className="text-base font-semibold text-content-primary mb-4">File Options</h3>

        {/* Filename truncation -- nullable number field.
            When the input is empty, `truncate` is set to `null` (no limit).
            When the input has a value, it is parsed to an integer. The
            nullish coalescing operator (`?? ''`) converts `null` to an
            empty string for the input's display value. */}
        <Input
          label="Truncate Filenames"
          description="Maximum filename length (leave empty for no limit)"
          type="number"
          min={10}
          max={255}
          value={settings.truncate?.toString() ?? ''} /* null -> '' for display */
          placeholder="No limit"
          onChange={(e) => {
            const val = e.target.value;
            updateSettings({
              truncate: val ? parseInt(val, 10) : null, // Empty string -> null (no limit)
            });
          }}
        />

        {/* Excluded tags -- comma-separated string <-> string[] conversion.
            The display value joins the array with ', ' for readability.
            The onChange handler splits on commas, trims whitespace from
            each segment, and filters out empty strings to avoid storing
            blank entries in the array. */}
        <Input
          label="Excluded Tags"
          description="Comma-separated list of metadata tags to exclude from downloaded files"
          value={settings.exclude_tags.join(', ')} /* string[] -> display string */
          placeholder="e.g., lyrics, comment"
          onChange={(e) => {
            const tags = e.target.value
              .split(',') // Split on commas into segments
              .map((t) => t.trim()) // Trim whitespace from each segment
              .filter(Boolean); // Remove empty strings (e.g., trailing comma)
            updateSettings({ exclude_tags: tags }); // Persist as string[]
          }}
        />
      </div>

      {/* Section: Error Reporting */}
      <div className="space-y-4">
        <h3 className="text-base font-semibold text-content-primary mb-4">Error Reporting</h3>

        <Toggle
          label="Send Anonymous Crash Reports"
          description="When enabled, anonymous crash data (error message, stack trace, app version, OS) is sent to our error tracking service to help identify and fix bugs. No personal data, download history, or account information is ever included. Requires an app restart to take effect."
          checked={settings.sentry_enabled}
          onChange={(checked) => updateSettings({ sentry_enabled: checked })}
        />

        <p className="text-xs text-content-tertiary">
          Error reports (crashes and download failures) are always saved locally to your app data
          directory regardless of this setting. You can view and report them below.
        </p>

        {/* Recent error reports list with GitHub reporting */}
        <CrashReportSection />
      </div>

      {/* Section: Diagnostics */}
      <div className="space-y-4">
        <h3 className="text-base font-semibold text-content-primary mb-4">Diagnostics</h3>

        <Toggle
          label="Verbose Activity Log"
          description="Emits detailed [VERBOSE] messages to the Activity Log for issue tracking and debugging. This setting is session-only and will reset to off when the application is restarted."
          checked={settings.verbose_activity_log}
          onChange={(checked) => updateSettings({ verbose_activity_log: checked })}
        />

        {settings.verbose_activity_log && (
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
              As a safety measure, this setting will automatically reset to off when the application
              is restarted. You will need to re-enable it each session if needed.
            </p>
          </div>
        )}
      </div>

      {/* ================================================================
          Section: API Credentials
          ================================================================ */}
      <div className="space-y-4">
        <h3 className="text-base font-semibold text-content-primary mb-4">API Credentials</h3>

        {/* ── MusicKit Credentials ── */}
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

          {/* Team ID */}
          <Input
            label="MusicKit Team ID"
            description="Your Apple Developer Team ID (10-character alphanumeric)"
            value={settings.musickit_team_id ?? ''}
            placeholder="XXXXXXXXXX"
            onChange={(e) => updateSettings({ musickit_team_id: e.target.value || null })}
          />

          {/* Key ID */}
          <Input
            label="MusicKit Key ID"
            description="The Key ID for your MusicKit private key (10-character alphanumeric)"
            value={settings.musickit_key_id ?? ''}
            placeholder="XXXXXXXXXX"
            onChange={(e) => updateSettings({ musickit_key_id: e.target.value || null })}
          />

          {/* Private Key (stored in OS keychain) */}
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

          {/* Test Credentials button */}
          <div className="flex items-start gap-2">
            <Button
              className="shrink-0"
              variant="secondary"
              size="sm"
              onClick={handleTestCredentials}
              disabled={
                validating ||
                !settings.musickit_team_id?.trim() ||
                !settings.musickit_key_id?.trim() ||
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

        {/* Divider between MusicKit and AcoustID */}
        <div className="border-t border-border" />

        {/* ── AcoustID API Key ── */}
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
            value={settings.acoustid_api_key ?? ''}
            placeholder={hasBuiltInKey ? 'Using built-in key' : 'Your AcoustID application API key'}
            onChange={(e) => updateSettings({ acoustid_api_key: e.target.value })}
          />
        </div>

        {/* Divider between AcoustID and API Field Audit */}
        <div className="border-t border-border" />

        {/* ── API Field Audit (developer diagnostic tool) ── */}
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
      </div>

      {/* ================================================================
          Section: Setup
          ================================================================ */}
      <div>
        <h3 className="text-base font-semibold text-content-primary mb-2">Setup</h3>
        <p className="text-xs text-content-secondary mb-3">
          Re-run the first-time setup wizard to verify and reinstall dependencies. Your existing
          settings will be preserved.
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
            updateSettings({ setup_completed: false });
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
      </div>
    </div>
  );
}
