/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file CookiesStep.tsx -- Cookie import step of the setup wizard.
 *
 * Renders the "Cookies" step within the {@link SetupWizard}. This step
 * guides the user through importing their Apple Music authentication
 * cookies, which GAMDL needs to access protected content.
 *
 * ## Two-Mode Design
 *
 * The step presents two import modes:
 *
 * 1. **Auto-Import (default)**: Detects installed browsers and allows
 *    one-click cookie extraction using the `rookie` crate. Only Apple
 *    Music cookies (apple.com, mzstatic.com) are read. The user sees
 *    exactly which browsers are available and clicks "Import" next to
 *    their preferred browser.
 *
 * 2. **Manual Import (fallback)**: The traditional flow where the user
 *    installs a browser extension, exports cookies to a file, and
 *    selects the file via a file picker. Accessed by clicking "Import
 *    from file instead".
 *
 * ## Platform-Specific Behaviour
 *
 * - **macOS (Chromium browsers)**: A Keychain access prompt may appear.
 *   The step shows a brief notice about this expected behaviour.
 * - **macOS (Safari)**: Requires Full Disk Access. The step shows an
 *   instruction panel with a button to open System Settings.
 * - **Windows/Linux**: Cookie extraction is transparent.
 *
 * ## Completion Detection
 *
 * The step can be completed in three ways:
 *   - **Auto-import success**: When cookies are successfully extracted
 *   - **Manual validation success**: When `validation.valid` is true
 *   - **Skip**: When the user clicks "Skip for Now"
 *
 * ## Store Connections
 *
 * - **settingsStore**: Reads/writes `settings.cookies_path`.
 * - **setupStore**: `completeStep('cookies')`.
 * - **Tauri commands**: `detectBrowsers`, `importCookiesFromBrowser`,
 *   `checkFullDiskAccess`, `validateCookiesFile`.
 *
 * @see {@link ../SetupWizard.tsx}                             -- Parent wizard container
 * @see {@link ../../settings/tabs/CookiesTab.tsx}             -- Full-featured cookies settings
 * @see {@link @/stores/settingsStore.ts}                      -- Zustand store for settings
 * @see {@link @/stores/setupStore.ts}                         -- Zustand store for wizard state
 * @see {@link @/lib/tauri-commands.ts}                        -- Tauri IPC command wrappers
 */

// React hooks for local state, effects, and memoization.
import { useState, useEffect, useCallback } from 'react';

// Lucide icons for the various UI elements.
import {
  Shield,         // Authentication context icon
  CheckCircle,    // Valid/success indicator
  AlertTriangle,  // Warning indicator
  XCircle,        // Error indicator
  Globe,          // Browser icon
  FileText,       // Manual import icon
  Loader2,        // Loading spinner
  ExternalLink,   // External link icon (System Settings)
  Info,           // Info notice icon
} from 'lucide-react';

// Zustand stores for settings and wizard state.
import { useSettingsStore } from '@/stores/settingsStore';
import { useSetupStore } from '@/stores/setupStore';

// Tauri IPC command wrappers.
import * as commands from '@/lib/tauri-commands';

// Shared UI components.
import { FilePickerButton, Button } from '@/components/common';

// TypeScript types for cookie data.
import type { CookieValidation, DetectedBrowser, CookieImportResult } from '@/types';

// Platform detection hook for macOS-specific UI.
import { usePlatform } from '@/hooks/usePlatform';

// ============================================================
// Sub-components
// ============================================================

/**
 * Renders a single browser row in the auto-import browser list.
 * Shows the browser name, an optional FDA warning badge (Safari on macOS),
 * and an "Import" button that triggers cookie extraction.
 */
function BrowserRow({
  browser,
  isImporting,
  importingBrowserId,
  onImport,
}: {
  browser: DetectedBrowser;
  isImporting: boolean;
  importingBrowserId: string | null;
  onImport: (id: string) => void;
}) {
  /** Whether this specific browser is currently being imported from */
  const isThisBrowserImporting =
    isImporting && importingBrowserId === browser.id;

  return (
    <div className="flex items-center justify-between py-2.5 px-3 rounded-platform hover:bg-surface-secondary transition-colors">
      <div className="flex items-center gap-2.5">
        <Globe size={16} className="text-accent flex-shrink-0" />
        <div>
          <span className="text-sm font-medium text-content-primary">
            {browser.name}
          </span>
          {browser.requires_fda && (
            <span className="ml-2 text-xs text-status-warning">
              (Full Disk Access required)
            </span>
          )}
        </div>
      </div>
      <Button
        variant="secondary"
        size="sm"
        loading={isThisBrowserImporting}
        disabled={isImporting && !isThisBrowserImporting}
        onClick={() => onImport(browser.id)}
      >
        Import
      </Button>
    </div>
  );
}

/**
 * Renders the macOS Full Disk Access instruction panel.
 * Shown when the user attempts to import from Safari without FDA.
 */
function FdaInstructionPanel({
  onRetry,
  onCancel,
}: {
  onRetry: () => void;
  onCancel: () => void;
}) {
  /**
   * Opens macOS System Settings to the Full Disk Access privacy pane.
   * Uses the Tauri shell plugin to open the URL scheme.
   */
  const handleOpenSettings = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-shell');
      await open(
        'x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles',
      );
    } catch {
      // Fallback: try opening the general privacy settings
      try {
        const { open } = await import('@tauri-apps/plugin-shell');
        await open('x-apple.systempreferences:com.apple.preference.security');
      } catch {
        // Silent failure -- user can navigate manually
      }
    }
  };

  return (
    <div className="p-4 rounded-platform border border-status-warning bg-yellow-50 dark:bg-yellow-950 space-y-3">
      <div className="flex items-start gap-2.5">
        <AlertTriangle
          size={16}
          className="text-status-warning flex-shrink-0 mt-0.5"
        />
        <div className="text-sm text-content-primary space-y-2">
          <p className="font-medium">Safari requires Full Disk Access</p>
          <p className="text-content-secondary text-xs">
            macOS protects Safari&apos;s cookie database. To import cookies from
            Safari, you need to grant Full Disk Access to GAMDL.
          </p>
          <ol className="list-decimal list-inside text-xs text-content-secondary space-y-1.5 ml-1">
            <li>
              Open{' '}
              <span className="font-medium text-content-primary">
                System Settings &gt; Privacy &amp; Security &gt; Full Disk
                Access
              </span>
            </li>
            <li>
              Click the{' '}
              <span className="font-medium text-content-primary">&quot;+&quot;</span>{' '}
              button and add{' '}
              <span className="font-medium text-content-primary">GAMDL</span>
            </li>
            <li>Return here and click &quot;Try Again&quot;</li>
          </ol>
        </div>
      </div>
      <div className="flex gap-2 ml-6">
        <Button variant="secondary" size="sm" onClick={handleOpenSettings}>
          <ExternalLink size={14} className="mr-1.5" />
          Open System Settings
        </Button>
        <Button variant="ghost" size="sm" onClick={onRetry}>
          Try Again
        </Button>
        <Button variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}

/**
 * Renders the import result panel (success or failure).
 */
function ImportResultPanel({
  result,
}: {
  result: CookieImportResult;
}) {
  return (
    <div
      className={`p-4 rounded-platform border ${
        result.success
          ? 'border-status-success bg-green-50 dark:bg-green-950'
          : 'border-status-error bg-red-50 dark:bg-red-950'
      }`}
    >
      <div className="flex items-center gap-2 mb-2">
        {result.success ? (
          <CheckCircle size={16} className="text-status-success" />
        ) : (
          <XCircle size={16} className="text-status-error" />
        )}
        <span className="text-sm font-medium text-content-primary">
          {result.success ? 'Cookies Imported Successfully' : 'Import Failed'}
        </span>
      </div>
      <div className="text-xs text-content-secondary space-y-1 ml-6">
        <p>{result.apple_music_cookies} Apple Music cookies imported</p>
        <p>{result.cookie_count} total cookies extracted</p>
        {result.warnings.map((w, i) => (
          <p key={i} className="text-status-warning">
            {w}
          </p>
        ))}
      </div>
    </div>
  );
}

// ============================================================
// Main Component
// ============================================================

/**
 * CookiesStep -- Renders the cookie import step with two modes:
 * auto-import (browser detection + one-click extraction) and manual
 * import (file picker + validation).
 *
 * Layout:
 *   1. Heading and description
 *   2. Auto-import: Detected browsers list with Import buttons
 *   3. OR Manual import: File picker + Validate button
 *   4. Privacy notice
 *   5. "Import from file instead" / "Import from browser instead" toggle
 *   6. "Skip for Now" button
 *   7. Import/Validation results panel
 */
export function CookiesStep() {
  // --- Store bindings ---
  /** Current settings (for reading cookies_path) */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting the cookies path */
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  /** Marks the 'cookies' wizard step as completed */
  const completeStep = useSetupStore((s) => s.completeStep);

  // --- Platform detection ---
  const { platform } = usePlatform();
  const isMacOS = platform === 'macos';

  // --- Local state ---
  /** Whether we're in manual import mode (false = auto-import mode) */
  const [isManualMode, setIsManualMode] = useState(false);
  /** List of detected browsers */
  const [browsers, setBrowsers] = useState<DetectedBrowser[]>([]);
  /** Whether browser detection is in progress */
  const [isDetecting, setIsDetecting] = useState(true);
  /** Whether a cookie import is in progress */
  const [isImporting, setIsImporting] = useState(false);
  /** Which browser is currently being imported from */
  const [importingBrowserId, setImportingBrowserId] = useState<string | null>(
    null,
  );
  /** Result of the most recent auto-import */
  const [importResult, setImportResult] = useState<CookieImportResult | null>(
    null,
  );
  /** Whether to show the FDA instruction panel */
  const [showFdaPanel, setShowFdaPanel] = useState(false);
  /** Error message from a failed import attempt */
  const [importError, setImportError] = useState<string | null>(null);
  /** Manual mode: validation result */
  const [validation, setValidation] = useState<CookieValidation | null>(null);
  /** Manual mode: whether validation is in progress */
  const [isValidating, setIsValidating] = useState(false);

  // --- Effects ---

  /**
   * Detect installed browsers on mount.
   * Calls the detect_browsers IPC command and populates the browser list.
   */
  useEffect(() => {
    let cancelled = false;
    async function detect() {
      try {
        const detected = await commands.detectBrowsers();
        if (!cancelled) {
          setBrowsers(detected);
        }
      } catch (err) {
        if (!cancelled) {
          // If detection fails, switch to manual mode
          console.warn('[CookiesStep] Browser detection failed:', err);
          setIsManualMode(true);
        }
      }
      if (!cancelled) {
        setIsDetecting(false);
      }
    }
    detect();
    return () => {
      cancelled = true;
    };
  }, []);

  /**
   * Auto-complete this step when auto-import succeeds.
   */
  useEffect(() => {
    if (importResult?.success) {
      completeStep('cookies');
    }
  }, [importResult, completeStep]);

  /**
   * Auto-complete this step when manual validation succeeds.
   */
  useEffect(() => {
    if (validation?.valid) {
      completeStep('cookies');
    }
  }, [validation, completeStep]);

  // --- Handlers ---

  /**
   * Handles the "Import" button click for a specific browser.
   * For Safari on macOS, checks FDA first. For Chromium browsers on macOS,
   * shows a Keychain notice. Then extracts cookies.
   */
  const handleImport = useCallback(
    async (browserId: string) => {
      setImportError(null);
      setImportResult(null);

      // Safari on macOS: check FDA first
      if (browserId === 'safari' && isMacOS) {
        try {
          const hasFda = await commands.checkFullDiskAccess();
          if (!hasFda) {
            setShowFdaPanel(true);
            return;
          }
        } catch {
          setShowFdaPanel(true);
          return;
        }
      }

      // Proceed with import
      setIsImporting(true);
      setImportingBrowserId(browserId);

      try {
        const result = await commands.importCookiesFromBrowser(browserId);
        setImportResult(result);

        // If the import updated settings, refresh the settings store
        if (result.success && result.path) {
          updateSettings({ cookies_path: result.path });
        }
      } catch (err) {
        const message =
          err instanceof Error ? err.message : String(err);
        setImportError(message);
      }

      setIsImporting(false);
      setImportingBrowserId(null);
    },
    [isMacOS, updateSettings],
  );

  /**
   * Retries the Safari FDA check after the user has granted access.
   */
  const handleFdaRetry = useCallback(() => {
    setShowFdaPanel(false);
    handleImport('safari');
  }, [handleImport]);

  /**
   * Validates the selected cookies file (manual mode).
   */
  const handleValidate = useCallback(async () => {
    if (!settings.cookies_path) return;
    setIsValidating(true);
    try {
      const result = await commands.validateCookiesFile(settings.cookies_path);
      setValidation(result);
    } catch {
      setValidation(null);
    }
    setIsValidating(false);
  }, [settings.cookies_path]);

  /**
   * Skips the cookie import step.
   */
  const handleSkip = useCallback(() => {
    completeStep('cookies');
  }, [completeStep]);

  // --- Render ---
  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h2 className="text-xl font-semibold text-content-primary">
          Apple Music Cookies
        </h2>
        <p className="text-sm text-content-secondary mt-1">
          GAMDL needs your Apple Music cookies to authenticate downloads.
          {' '}
          <span className="font-medium text-content-primary">
            You must be logged in to Apple Music in your browser.
          </span>
        </p>
      </div>

      {/* ================================================
          Auto-Import Mode
          ================================================ */}
      {!isManualMode && (
        <>
          {/* Browser list */}
          {isDetecting ? (
            <div className="flex items-center gap-2 py-4 text-sm text-content-secondary">
              <Loader2 size={16} className="animate-spin" />
              Detecting installed browsers...
            </div>
          ) : browsers.length > 0 ? (
            <div className="rounded-platform border border-border-light bg-surface-elevated overflow-hidden">
              <div className="px-3 py-2 border-b border-border-light">
                <span className="text-xs font-medium text-content-secondary uppercase tracking-wide">
                  Detected Browsers
                </span>
              </div>
              <div className="divide-y divide-border-light">
                {browsers.map((browser) => (
                  <BrowserRow
                    key={browser.id}
                    browser={browser}
                    isImporting={isImporting}
                    importingBrowserId={importingBrowserId}
                    onImport={handleImport}
                  />
                ))}
              </div>
            </div>
          ) : (
            <div className="p-4 rounded-platform border border-border-light bg-surface-elevated text-sm text-content-secondary">
              No browsers detected. Please use the manual import option below.
            </div>
          )}

          {/* macOS Keychain notice (shown for Chromium browsers on macOS) */}
          {isMacOS && !showFdaPanel && !importResult && (
            <div className="flex items-start gap-2.5 p-3 rounded-platform border border-border-light bg-surface-elevated">
              <Info size={14} className="text-content-tertiary flex-shrink-0 mt-0.5" />
              <p className="text-xs text-content-secondary">
                macOS may ask for your password when importing cookies from
                Chrome, Edge, or other Chromium-based browsers. This is a
                standard macOS security prompt to access browser data.
              </p>
            </div>
          )}

          {/* FDA instruction panel (Safari on macOS) */}
          {showFdaPanel && (
            <FdaInstructionPanel
              onRetry={handleFdaRetry}
              onCancel={() => setShowFdaPanel(false)}
            />
          )}

          {/* Import error */}
          {importError && (
            <div className="p-4 rounded-platform border border-status-error bg-red-50 dark:bg-red-950">
              <div className="flex items-center gap-2 mb-1">
                <XCircle size={16} className="text-status-error" />
                <span className="text-sm font-medium text-content-primary">
                  Import Failed
                </span>
              </div>
              <p className="text-xs text-content-secondary ml-6">
                {importError}
              </p>
            </div>
          )}

          {/* Import result */}
          {importResult && <ImportResultPanel result={importResult} />}
        </>
      )}

      {/* ================================================
          Manual Import Mode
          ================================================ */}
      {isManualMode && (
        <>
          {/* Instructions card */}
          <div className="p-4 rounded-platform-lg border border-border-light bg-surface-elevated space-y-3">
            <div className="flex items-start gap-3">
              <Shield size={18} className="text-accent flex-shrink-0 mt-0.5" />
              <div className="text-sm text-content-secondary space-y-2">
                <p className="font-medium text-content-primary">
                  How to export cookies:
                </p>
                <ol className="list-decimal list-inside space-y-1.5 ml-1">
                  <li>
                    Install the{' '}
                    <span className="font-medium text-content-primary">
                      cookies.txt
                    </span>{' '}
                    browser extension
                  </li>
                  <li>
                    Go to{' '}
                    <span className="font-mono text-xs text-accent">
                      music.apple.com
                    </span>{' '}
                    and log in to your account
                  </li>
                  <li>
                    Click the cookies.txt extension icon and export cookies
                  </li>
                  <li>Save the file and select it below</li>
                </ol>
              </div>
            </div>
          </div>

          {/* Cookie file picker */}
          <FilePickerButton
            label="Cookies File"
            description="Select your exported cookies.txt file"
            value={settings.cookies_path}
            onChange={(path) => {
              updateSettings({ cookies_path: path });
              setValidation(null);
            }}
            placeholder="No cookies file selected"
            filters={[{ name: 'Text Files', extensions: ['txt'] }]}
          />

          {/* Validate button */}
          {settings.cookies_path && (
            <Button
              variant="primary"
              loading={isValidating}
              onClick={handleValidate}
            >
              Validate Cookies
            </Button>
          )}

          {/* Validation results */}
          {validation && (
            <div
              className={`p-4 rounded-platform border ${
                validation.valid
                  ? 'border-status-success bg-green-50 dark:bg-green-950'
                  : 'border-status-error bg-red-50 dark:bg-red-950'
              }`}
            >
              <div className="flex items-center gap-2 mb-2">
                {validation.valid ? (
                  <CheckCircle size={16} className="text-status-success" />
                ) : (
                  <AlertTriangle size={16} className="text-status-error" />
                )}
                <span className="text-sm font-medium text-content-primary">
                  {validation.valid ? 'Cookies Valid' : 'Cookies Invalid'}
                </span>
              </div>
              <div className="text-xs text-content-secondary space-y-1 ml-6">
                <p>
                  {validation.apple_music_cookies} Apple Music cookies found
                </p>
                {validation.warnings.map((w, i) => (
                  <p key={i} className="text-status-warning">
                    {w}
                  </p>
                ))}
              </div>
            </div>
          )}
        </>
      )}

      {/* ================================================
          Privacy Notice
          ================================================ */}
      {!isManualMode && (
        <div className="flex items-start gap-2.5 p-3 rounded-platform border border-border-light bg-surface-elevated">
          <Shield size={14} className="text-accent flex-shrink-0 mt-0.5" />
          <p className="text-xs text-content-secondary">
            Only cookies for{' '}
            <span className="font-medium text-content-primary">
              apple.com
            </span>{' '}
            and{' '}
            <span className="font-medium text-content-primary">
              mzstatic.com
            </span>{' '}
            are read. No other browsing data is accessed.
          </p>
        </div>
      )}

      {/* ================================================
          Mode Toggle & Skip
          ================================================ */}
      <div className="flex gap-3">
        {isManualMode ? (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setIsManualMode(false);
              setValidation(null);
            }}
          >
            <Globe size={14} className="mr-1.5" />
            Import from browser instead
          </Button>
        ) : (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setIsManualMode(true);
              setImportResult(null);
              setImportError(null);
            }}
          >
            <FileText size={14} className="mr-1.5" />
            Import from file instead
          </Button>
        )}
        <Button variant="ghost" size="sm" onClick={handleSkip}>
          Skip for Now
        </Button>
      </div>
    </div>
  );
}
