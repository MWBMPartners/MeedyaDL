/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file CookiesTab.tsx -- Cookie import and validation settings tab.
 *
 * Renders the "Cookies" tab within the {@link SettingsPage} component.
 * This is one of the most feature-rich settings tabs, providing:
 *
 *   1. **Status badge** -- Inline indicator showing the current cookie state
 *      (not-set, valid, invalid, or expired).
 *   2. **Information box** -- Explains why cookies are needed for Apple Music
 *      authentication.
 *   3. **Auto-import from browser** -- Dropdown with detected browsers and
 *      one-click cookie extraction via the `rookie` crate.
 *   4. **Sign in with Apple Music** -- Opens an embedded browser window for
 *      direct Apple Music login when no browser cookies exist.
 *   5. **Browser instructions** -- Collapsible accordion with step-by-step
 *      guides for Chrome, Firefox, Edge, and Safari.
 *   6. **File picker** -- Lets the user browse for their cookies.txt file
 *      using the Tauri native file dialog.
 *   7. **Validate button** -- Sends the selected file to the Rust backend
 *      for validation via the `validate_cookies_file` Tauri IPC command.
 *   8. **Copy path button** -- Copies the file path to the system clipboard.
 *   9. **Validation results panel** -- Displays cookie count, detected
 *      domains, expiry warnings, and any additional backend warnings.
 *
 * ## Validation Flow
 *
 * 1. User selects a cookies.txt file via the FilePickerButton.
 * 2. The path is persisted to `settings.cookies_path` in the store.
 * 3. User clicks "Validate Cookies", which calls
 *    `commands.validateCookiesFile(path)` (a Tauri IPC command).
 * 4. The Rust backend reads the file, counts cookies, checks for Apple Music
 *    domains, and returns a `CookieValidation` result.
 * 5. The result is stored in local state and rendered in the results panel.
 *
 * ## Sub-components (file-private)
 *
 * - `StatusBadge` -- Renders the inline status indicator.
 * - `BrowserInstructions` -- Collapsible browser-specific export guides.
 * - `DetectedDomains` -- Renders the detected domain pills.
 * - `ExpiryWarning` -- Renders expiry warning/error banners.
 *
 * ## Store Connection
 *
 * - **settingsStore**: Reads `settings.cookies_path`; writes via
 *   `updateSettings({ cookies_path })`.
 * - **Tauri commands**: Calls `validateCookiesFile`, `detectBrowsers`,
 *   `importCookiesFromBrowser`, `openAppleLogin`, `extractLoginCookies`,
 *   `closeAppleLogin` for backend operations.
 * - **Tauri events**: Listens for `login-cookies-extracted` and
 *   `login-window-closed` events from the embedded login browser.
 *
 * @see {@link ../SettingsPage.tsx}                  -- Parent container
 * @see {@link @/stores/settingsStore.ts}            -- Zustand store
 * @see {@link @/lib/tauri-commands.ts}              -- Tauri IPC command wrappers
 * @see {@link @/types/index.ts}                     -- CookieValidation type
 * @see {@link https://v2.tauri.app/develop/calling-rust/} -- Tauri command invocation
 */

// React hooks: useState for local UI state, useMemo for derived values,
// useCallback for stable handler references.
// @see https://react.dev/reference/react/useState
// @see https://react.dev/reference/react/useMemo
// @see https://react.dev/reference/react/useCallback
import { useState, useEffect, useMemo, useCallback } from 'react';

// Lucide icons used throughout the tab's various sub-components.
// @see https://lucide.dev/icons/
import {
  Shield,         // Info box icon (security/authentication context)
  AlertTriangle,  // Warning/error indicator
  CheckCircle,    // Success indicator
  XCircle,        // Failure indicator
  ChevronDown,    // Expanded state arrow
  ChevronRight,   // Collapsed state arrow
  Copy,           // Copy-to-clipboard button icon
  Clock,          // Expiry warning icon
  Globe,          // Domain indicator icon
  Cookie,         // Tab/section header icon
  Info,           // "How to Export" section icon
  CircleDot,      // "Not set" status icon
  Loader2,        // Loading spinner
  LogIn,          // Sign-in icon for embedded browser login
} from 'lucide-react';

// Tauri event listener for receiving events from the Rust backend.
import { listen } from '@tauri-apps/api/event';

// Zustand store for reading the cookies_path and writing path updates.
import { useSettingsStore } from '@/stores/settingsStore';

// Tauri IPC command wrappers -- specifically `validateCookiesFile` which
// invokes the Rust backend's cookie validation logic.
import * as commands from '@/lib/tauri-commands';

// Shared UI components used in the form controls and action buttons.
import { FilePickerButton, Button, Tooltip } from '@/components/common';

// TypeScript types for cookie data.
import type { CookieValidation, DetectedBrowser, CookieImportResult } from '@/types';

// Platform detection hook for macOS-specific UI.
import { usePlatform } from '@/hooks/usePlatform';

// ============================================================
// Constants
// ============================================================

/**
 * Step-by-step browser instructions for exporting cookies.
 * Each entry contains the browser name and an ordered list of
 * user-facing instruction steps. These are rendered inside the
 * expandable "How to Export Cookies" section.
 */
const BROWSER_INSTRUCTIONS = [
  {
    browser: 'Google Chrome',
    steps: [
      'Install the "Get cookies.txt LOCALLY" extension from the Chrome Web Store.',
      'Navigate to https://music.apple.com and sign in with your Apple ID.',
      'Click the extension icon in the toolbar.',
      'Click "Export" to download the cookies as a Netscape-format .txt file.',
      'Save the file somewhere you can find it (e.g., your Downloads folder).',
      'Use the "Browse" button below to select the exported file.',
    ],
  },
  {
    browser: 'Mozilla Firefox',
    steps: [
      'Install the "cookies.txt" add-on from the Firefox Add-ons site.',
      'Navigate to https://music.apple.com and sign in with your Apple ID.',
      'Click the extension icon in the toolbar.',
      'Select "Current Site" to export only Apple Music cookies.',
      'Save the cookies.txt file to a convenient location.',
      'Use the "Browse" button below to select the exported file.',
    ],
  },
  {
    browser: 'Microsoft Edge',
    steps: [
      'Install the "Get cookies.txt LOCALLY" extension from the Edge Add-ons store.',
      'Navigate to https://music.apple.com and sign in with your Apple ID.',
      'Click the extension icon in the toolbar.',
      'Click "Export" to download the cookies in Netscape format.',
      'Save the file to a location you can easily browse to.',
      'Use the "Browse" button below to select the exported file.',
    ],
  },
  {
    browser: 'Safari (macOS)',
    steps: [
      'Safari does not have a direct cookie export extension.',
      'Use a third-party tool like "Cookie Exporter" or export via developer tools.',
      'Alternatively, open music.apple.com in Chrome or Firefox to export cookies.',
      'Ensure the exported file is in Netscape/Mozilla cookie format.',
      'Use the "Browse" button below to select the exported file.',
    ],
  },
] as const;

/**
 * Threshold in days below which we display a warning that
 * cookies are approaching expiry.
 */
const EXPIRY_WARNING_DAYS = 7;

// ============================================================
// Status Badge Types
// ============================================================

/**
 * Possible cookie states used to render the status badge.
 * - 'not-set': No cookies file has been selected yet.
 * - 'valid': Cookies file has been validated successfully and is not expired.
 * - 'invalid': Validation ran but the cookies file failed checks.
 * - 'expired': Cookies file is valid structurally but cookies have expired.
 */
type CookieStatus = 'not-set' | 'valid' | 'invalid' | 'expired';

/**
 * Maps each CookieStatus to its display configuration.
 * Used by the StatusBadge component to render the appropriate
 * icon, label, and colour classes.
 */
const STATUS_CONFIG: Record<
  CookieStatus,
  { label: string; colorClass: string; bgClass: string }
> = {
  'not-set': {
    label: 'Not Set',
    colorClass: 'text-content-tertiary',
    bgClass: 'bg-surface-secondary',
  },
  valid: {
    label: 'Valid',
    colorClass: 'text-status-success',
    bgClass: 'bg-green-50 dark:bg-green-950',
  },
  invalid: {
    label: 'Invalid',
    colorClass: 'text-status-error',
    bgClass: 'bg-red-50 dark:bg-red-950',
  },
  expired: {
    label: 'Expired',
    colorClass: 'text-status-warning',
    bgClass: 'bg-yellow-50 dark:bg-yellow-950',
  },
};

// ============================================================
// Helper Functions
// ============================================================

/**
 * Derives the overall cookie status from the current settings
 * and the most recent validation result.
 *
 * @param cookiesPath - The currently selected cookies file path (or null).
 * @param validation - The result of the last validation call (or null if
 *   validation has not been run yet).
 * @returns The computed CookieStatus enum value.
 */
function deriveCookieStatus(
  cookiesPath: string | null,
  validation: CookieValidation | null,
): CookieStatus {
  /* No file has been selected at all */
  if (!cookiesPath) return 'not-set';

  /* A file is selected but we haven't validated it yet */
  if (!validation) return 'not-set';

  /* Validation returned but the cookies have expired */
  if (validation.expired) return 'expired';

  /* Validation returned a definitive pass/fail */
  return validation.valid ? 'valid' : 'invalid';
}

/**
 * Estimates the number of days until cookie expiry based on the
 * current validation warnings. The Rust backend typically emits
 * a warning string like "Cookies expire in N days" when expiry
 * is approaching. This function parses that value out of the
 * warnings array.
 *
 * @param warnings - Array of warning strings from the validation result.
 * @returns The estimated days until expiry, or null if no expiry
 *   information was found in the warnings.
 */
function parseExpiryDays(warnings: string[]): number | null {
  for (const warning of warnings) {
    /* Match patterns like "expire in 5 days", "expires in 12 days", etc. */
    const match = warning.match(/expir\w*\s+in\s+(\d+)\s+day/i);
    if (match) {
      return parseInt(match[1], 10);
    }
  }
  return null;
}

// ============================================================
// Sub-components
// ============================================================

/**
 * Renders a small inline badge that visually communicates the
 * current cookie state. Displays an icon alongside a text label,
 * colour-coded to match the severity.
 *
 * @param status - The derived CookieStatus value.
 */
function StatusBadge({ status }: { status: CookieStatus }) {
  const config = STATUS_CONFIG[status];

  /**
   * Selects the appropriate icon for the given status.
   * Each icon is sized at 14px for compact inline display.
   */
  const renderIcon = () => {
    switch (status) {
      case 'valid':
        return <CheckCircle size={14} />;
      case 'invalid':
        return <XCircle size={14} />;
      case 'expired':
        return <AlertTriangle size={14} />;
      case 'not-set':
      default:
        return <CircleDot size={14} />;
    }
  };

  return (
    <span
      className={`
        inline-flex items-center gap-1.5 px-2.5 py-1
        text-xs font-medium rounded-full
        border border-border-light
        ${config.colorClass} ${config.bgClass}
      `}
      role="status"
      aria-label={`Cookie status: ${config.label}`}
    >
      {renderIcon()}
      {config.label}
    </span>
  );
}

/**
 * Renders a collapsible section with step-by-step browser
 * instructions for exporting cookies. Each browser gets its own
 * expandable sub-section so users can quickly find the
 * instructions relevant to their browser.
 */
function BrowserInstructions() {
  /**
   * Tracks which browser instruction panel is currently expanded.
   * Only one browser section can be open at a time to avoid
   * overwhelming the user with too much text. A value of null
   * means all browser sections are collapsed.
   */
  const [expandedBrowser, setExpandedBrowser] = useState<string | null>(null);

  /**
   * Tracks whether the outer "How to Export Cookies" container
   * is expanded or collapsed. Defaults to collapsed so the tab
   * is not visually cluttered on first render.
   */
  const [isOpen, setIsOpen] = useState(false);

  /**
   * Toggles a browser's instruction panel. If the tapped browser
   * is already open, collapse it; otherwise open the new one and
   * close any previously open panel.
   */
  const toggleBrowser = useCallback((browser: string) => {
    setExpandedBrowser((prev) => (prev === browser ? null : browser));
  }, []);

  return (
    <div className="rounded-platform border border-border-light bg-surface-elevated overflow-hidden">
      {/* Outer collapsible header */}
      <button
        type="button"
        onClick={() => setIsOpen((prev) => !prev)}
        className="
          w-full flex items-center gap-3 px-4 py-3
          text-sm font-medium text-content-primary
          hover:bg-surface-secondary transition-colors
          cursor-pointer
        "
        aria-expanded={isOpen}
      >
        {/* Chevron rotates to indicate open/closed state */}
        {isOpen ? (
          <ChevronDown size={16} className="text-content-tertiary flex-shrink-0" />
        ) : (
          <ChevronRight size={16} className="text-content-tertiary flex-shrink-0" />
        )}
        <Info size={16} className="text-accent flex-shrink-0" />
        <span>How to Export Cookies</span>
      </button>

      {/* Expandable instruction content */}
      {isOpen && (
        <div className="border-t border-border-light px-4 pb-4 pt-2 space-y-2">
          {/* Introductory paragraph */}
          <p className="text-xs text-content-secondary mb-3">
            Select your browser below for step-by-step instructions on how to
            export your Apple Music cookies in Netscape format.
          </p>

          {/* One accordion panel per browser */}
          {BROWSER_INSTRUCTIONS.map(({ browser, steps }) => (
            <div
              key={browser}
              className="rounded-platform border border-border-light overflow-hidden"
            >
              {/* Browser header button */}
              <button
                type="button"
                onClick={() => toggleBrowser(browser)}
                className="
                  w-full flex items-center gap-2 px-3 py-2
                  text-xs font-medium text-content-primary
                  hover:bg-surface-secondary transition-colors
                  cursor-pointer
                "
                aria-expanded={expandedBrowser === browser}
              >
                {expandedBrowser === browser ? (
                  <ChevronDown size={14} className="text-content-tertiary flex-shrink-0" />
                ) : (
                  <ChevronRight size={14} className="text-content-tertiary flex-shrink-0" />
                )}
                <Globe size={14} className="text-accent flex-shrink-0" />
                <span>{browser}</span>
              </button>

              {/* Numbered step list (shown only when this browser is expanded) */}
              {expandedBrowser === browser && (
                <ol className="list-decimal list-inside px-4 pb-3 pt-1 space-y-1.5 text-xs text-content-secondary border-t border-border-light">
                  {steps.map((step, index) => (
                    <li key={index} className="leading-relaxed">
                      {step}
                    </li>
                  ))}
                </ol>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Renders the list of detected domains found during cookie
 * validation. Each domain is displayed as a small pill/tag so
 * the user can quickly see which sites' cookies are present in
 * the file.
 *
 * @param domains - Array of domain strings from the validation result.
 */
function DetectedDomains({ domains }: { domains: string[] }) {
  /* Don't render anything if there are no domains */
  if (domains.length === 0) return null;

  return (
    <div className="space-y-2">
      {/* Section heading */}
      <div className="flex items-center gap-2">
        <Globe size={14} className="text-content-tertiary" />
        <span className="text-xs font-medium text-content-secondary">
          Detected Domains ({domains.length})
        </span>
      </div>

      {/* Domain pill list */}
      <div className="flex flex-wrap gap-1.5">
        {domains.map((domain) => (
          <span
            key={domain}
            className={`
              inline-flex items-center px-2 py-0.5
              text-xs rounded-full border
              ${
                domain.includes('apple.com')
                  ? 'bg-green-50 dark:bg-green-950 border-status-success text-status-success'
                  : 'bg-surface-secondary border-border-light text-content-tertiary'
              }
            `}
          >
            {domain}
          </span>
        ))}
      </div>
    </div>
  );
}

/**
 * Renders a warning banner when cookies are expired or
 * approaching expiry. Shows the estimated number of days
 * remaining (if parseable from warnings) alongside a clock icon.
 *
 * @param expired - Whether the cookies have fully expired.
 * @param warnings - Array of warning strings from the validation result,
 *   potentially containing expiry-day information.
 */
function ExpiryWarning({
  expired,
  warnings,
}: {
  expired: boolean;
  warnings: string[];
}) {
  /**
   * Parse the number of days until expiry from the warnings.
   * Memoized to avoid re-parsing on every render.
   */
  const daysUntilExpiry = useMemo(() => parseExpiryDays(warnings), [warnings]);

  /*
   * If cookies are not expired and there is no parseable days-until-expiry
   * value, there is nothing actionable to display.
   */
  if (!expired && daysUntilExpiry === null) return null;

  /**
   * Determine whether the remaining days are critically low.
   * Used to escalate the visual severity from warning (yellow)
   * to error (red).
   */
  const isCritical =
    expired || (daysUntilExpiry !== null && daysUntilExpiry <= EXPIRY_WARNING_DAYS);

  return (
    <div
      className={`
        flex items-start gap-2.5 p-3 rounded-platform border text-xs
        ${
          isCritical
            ? 'border-status-error bg-red-50 dark:bg-red-950 text-status-error'
            : 'border-status-warning bg-yellow-50 dark:bg-yellow-950 text-status-warning'
        }
      `}
      role="alert"
    >
      <Clock size={14} className="flex-shrink-0 mt-0.5" />
      <div className="space-y-0.5">
        {/* Primary message */}
        {expired ? (
          <p className="font-medium">Cookies have expired</p>
        ) : (
          <p className="font-medium">
            Cookies expire in {daysUntilExpiry} day{daysUntilExpiry !== 1 ? 's' : ''}
          </p>
        )}

        {/* Guidance text */}
        <p className="opacity-80">
          {expired
            ? 'Please export a fresh cookies file from your browser to continue downloading.'
            : isCritical
              ? 'Consider re-exporting cookies soon to avoid authentication failures.'
              : 'Your cookies are approaching their expiry date.'}
        </p>
      </div>
    </div>
  );
}

// ============================================================
// Main Component
// ============================================================

/**
 * Renders the Cookies settings tab with:
 * - A status badge indicating the current cookie state.
 * - An introductory info box about Apple Music cookie requirements.
 * - Expandable/collapsible browser-specific export instructions.
 * - A file picker for selecting the cookies.txt file.
 * - A "Copy Cookie Path" button when a file is selected.
 * - A "Validate Cookies" button with loading state.
 * - Validation results panel showing:
 *   - Pass/fail header with icon.
 *   - Cookie counts (total and Apple Music specific).
 *   - Detected domains list with visual highlighting for Apple domains.
 *   - Expiry warning with estimated days remaining.
 *   - Any additional warnings from the backend.
 */
export function CookiesTab() {
  /* ---- Store bindings ---- */
  /** Read the current application settings from the Zustand store */
  const settings = useSettingsStore((s) => s.settings);

  /** Obtain the settings mutation function to persist changes */
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  /* ---- Local state ---- */
  /**
   * Holds the result of the most recent cookie validation call.
   * null means validation has not been attempted (or was cleared
   * after selecting a new file).
   */
  const [validation, setValidation] = useState<CookieValidation | null>(null);

  /**
   * Tracks whether a validation request is currently in-flight
   * so we can show a loading spinner on the validate button.
   */
  const [isValidating, setIsValidating] = useState(false);

  /**
   * Tracks whether the "Copy Cookie Path" operation succeeded
   * so we can briefly flash a confirmation to the user.
   */
  const [copySuccess, setCopySuccess] = useState(false);

  /* ---- Auto-import state ---- */
  /** List of detected browsers for auto-import */
  const [browsers, setBrowsers] = useState<DetectedBrowser[]>([]);
  /** Whether browser detection is in progress */
  const [isDetecting, setIsDetecting] = useState(true);
  /** Whether a cookie import is in progress */
  const [isImporting, setIsImporting] = useState(false);
  /** Which browser is currently selected in the dropdown */
  const [selectedBrowser, setSelectedBrowser] = useState<string>('');
  /** Result of the most recent auto-import */
  const [importResult, setImportResult] = useState<CookieImportResult | null>(null);
  /** Error message from a failed import */
  const [importError, setImportError] = useState<string | null>(null);

  /* ---- Embedded login window state ---- */
  /** Whether the embedded login browser window is currently open */
  const [isLoginWindowOpen, setIsLoginWindowOpen] = useState(false);
  /** Whether cookie extraction from the login window is in progress */
  const [isExtractingFromLogin, setIsExtractingFromLogin] = useState(false);

  /* ---- Platform detection ---- */
  const { platform } = usePlatform();

  /* ---- Derived values ---- */
  /**
   * Compute the overall cookie status from the current path and
   * validation result. This drives the status badge display.
   */
  const cookieStatus = useMemo(
    () => deriveCookieStatus(settings.cookies_path, validation),
    [settings.cookies_path, validation],
  );

  /* ---- Effects ---- */

  /**
   * Detect installed browsers on mount for the auto-import dropdown.
   */
  useEffect(() => {
    let cancelled = false;
    async function detect() {
      try {
        const detected = await commands.detectBrowsers();
        if (!cancelled) {
          setBrowsers(detected);
          // Pre-select the first browser if available
          if (detected.length > 0) {
            setSelectedBrowser(detected[0].id);
          }
        }
      } catch {
        // Browser detection failed -- auto-import section will be hidden
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
   * Listen for the "login-cookies-extracted" event from the Rust backend.
   * Emitted when the embedded login window detects successful Apple Music
   * authentication and extracts cookies automatically.
   */
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    async function setup() {
      try {
        unlisten = await listen<CookieImportResult>(
          'login-cookies-extracted',
          (event) => {
            try {
              setImportResult(event.payload);
              setIsLoginWindowOpen(false);
              setIsExtractingFromLogin(false);

              // Update settings if cookies were saved successfully
              if (event.payload.success && event.payload.path) {
                updateSettings({ cookies_path: event.payload.path });
                setValidation(null); // Clear manual validation for fresh cookies
              }
            } catch (err) {
              console.error('Error handling login-cookies-extracted:', err);
            }
          },
        );
      } catch (err) {
        console.warn('[CookiesTab] Failed to listen for login events:', err);
      }
    }

    setup();
    return () => {
      unlisten?.();
    };
  }, [updateSettings]);

  /**
   * Listen for the "login-window-closed" event from the Rust backend.
   * Emitted when the login window is closed (by user or programmatically).
   * Resets the login window state so the UI shows the correct buttons.
   */
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    async function setup() {
      try {
        unlisten = await listen('login-window-closed', () => {
          setIsLoginWindowOpen(false);
          setIsExtractingFromLogin(false);
        });
      } catch (err) {
        console.warn('[CookiesTab] Failed to listen for login-window-closed:', err);
      }
    }

    setup();
    return () => {
      unlisten?.();
    };
  }, []);

  /* ---- Handlers ---- */

  /**
   * Handles the auto-import button click. Extracts cookies from the
   * selected browser and updates settings on success.
   */
  const handleBrowserImport = useCallback(async () => {
    if (!selectedBrowser) return;

    // Safari on macOS: check FDA first
    if (selectedBrowser === 'safari' && platform === 'macos') {
      try {
        const hasFda = await commands.checkFullDiskAccess();
        if (!hasFda) {
          setImportError(
            'Safari requires Full Disk Access. Go to System Settings > Privacy & Security > Full Disk Access and add GAMDL.',
          );
          return;
        }
      } catch {
        setImportError('Unable to check Full Disk Access status.');
        return;
      }
    }

    setIsImporting(true);
    setImportError(null);
    setImportResult(null);

    try {
      const result = await commands.importCookiesFromBrowser(selectedBrowser);
      setImportResult(result);

      // Update settings store if import was successful
      if (result.success && result.path) {
        updateSettings({ cookies_path: result.path });
        setValidation(null); // Clear manual validation since we have fresh cookies
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setImportError(message);
    }

    setIsImporting(false);
  }, [selectedBrowser, platform, updateSettings]);

  /**
   * Opens the embedded Apple Music login browser window.
   * Calls the Rust backend to create a secondary webview that loads
   * music.apple.com, allowing the user to sign in directly.
   */
  const handleOpenLoginWindow = useCallback(async () => {
    setImportError(null);
    setImportResult(null);

    try {
      await commands.openAppleLogin();
      setIsLoginWindowOpen(true);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setImportError(message);
    }
  }, []);

  /**
   * Manually triggers cookie extraction from the login window.
   * Fallback for when automatic detection doesn't fire.
   */
  const handleManualExtract = useCallback(async () => {
    setIsExtractingFromLogin(true);
    setImportError(null);

    try {
      const result = await commands.extractLoginCookies();
      setImportResult(result);
      setIsLoginWindowOpen(false);
      setIsExtractingFromLogin(false);

      if (result.success && result.path) {
        updateSettings({ cookies_path: result.path });
        setValidation(null);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setImportError(message);
      setIsExtractingFromLogin(false);
    }
  }, [updateSettings]);

  /**
   * Cancels the embedded login flow by closing the login window
   * and resetting login-related state.
   */
  const handleCancelLogin = useCallback(async () => {
    try {
      await commands.closeAppleLogin();
    } catch {
      // Silent failure -- window may already be closed
    }
    setIsLoginWindowOpen(false);
    setIsExtractingFromLogin(false);
  }, []);

  /**
   * Validates the currently selected cookies file by invoking the
   * Rust backend command. Updates both the validation state and
   * the loading indicator. If the call throws (e.g., file not
   * found), the validation state is cleared to null.
   */
  const handleValidate = useCallback(async () => {
    /* Guard: nothing to validate if no file is selected */
    if (!settings.cookies_path) return;

    setIsValidating(true);
    try {
      const result = await commands.validateCookiesFile(settings.cookies_path);
      setValidation(result);
    } catch {
      /* On error, reset validation so the UI does not show stale results */
      setValidation(null);
    }
    setIsValidating(false);
  }, [settings.cookies_path]);

  /**
   * Copies the currently selected cookies file path to the system
   * clipboard using the Clipboard API. Shows a brief "Copied!"
   * confirmation for 2 seconds before reverting the button text.
   */
  const handleCopyPath = useCallback(async () => {
    if (!settings.cookies_path) return;

    try {
      await navigator.clipboard.writeText(settings.cookies_path);
      setCopySuccess(true);

      /* Reset the success indicator after 2 seconds */
      setTimeout(() => setCopySuccess(false), 2000);
    } catch {
      /* Clipboard write can fail if the document is not focused or
         the permission was denied -- silently ignore. */
    }
  }, [settings.cookies_path]);

  /**
   * Handles changes from the file picker. When a new file is
   * selected, persists the path to settings and clears any
   * previous validation result so stale data is not displayed.
   */
  const handleFileChange = useCallback(
    (path: string | null) => {
      updateSettings({ cookies_path: path });
      setValidation(null);
      setCopySuccess(false);
    },
    [updateSettings],
  );

  /* ---- Render ---- */
  return (
    <div className="space-y-6 max-w-xl">
      {/* ============================================================
          Section 1: Header with Status Badge
          Shows the tab title alongside the current cookie status
          so the user can see the state at a glance.
          ============================================================ */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2.5">
          <Cookie size={18} className="text-accent" />
          <h3 className="text-sm font-medium text-content-primary">
            Authentication Cookies
          </h3>
        </div>
        <StatusBadge status={cookieStatus} />
      </div>

      {/* ============================================================
          Section 2: Introductory Information Box
          Brief explanation of why cookies are needed and what format
          is expected. Rendered with a shield icon for visual emphasis.
          ============================================================ */}
      <div className="p-4 rounded-platform border border-border-light bg-surface-elevated">
        <div className="flex items-start gap-3">
          <Shield size={18} className="text-accent flex-shrink-0 mt-0.5" />
          <div className="text-sm text-content-secondary space-y-2">
            <p>
              GAMDL requires Apple Music cookies for authentication. You need a
              Netscape-format cookies file exported from your browser.
            </p>
            <p>
              Use a browser extension like{' '}
              <span className="font-medium text-content-primary">
                cookies.txt
              </span>{' '}
              to export cookies from{' '}
              <span className="font-medium text-content-primary">
                music.apple.com
              </span>{' '}
              after logging in.
            </p>
          </div>
        </div>
      </div>

      {/* ============================================================
          Section 2.5: Auto-Import from Browser
          Quick re-import option for when cookies expire. Shows a
          browser dropdown and an import button.
          ============================================================ */}
      {!isDetecting && browsers.length > 0 && (
        <div className="p-4 rounded-platform border border-border-light bg-surface-elevated space-y-3">
          <div className="flex items-center gap-2.5 mb-2">
            <Globe size={16} className="text-accent flex-shrink-0" />
            <h4 className="text-xs font-medium text-content-primary">
              Import from Browser
            </h4>
          </div>

          {/* Browser selector and import button */}
          <div className="flex items-center gap-2">
            <select
              aria-label="Select browser for cookie import"
              className="
                flex-1 px-3 py-1.5 text-sm rounded-platform
                border border-border-light bg-surface-primary
                text-content-primary
                focus:outline-none focus:ring-2 focus:ring-accent/50
              "
              value={selectedBrowser}
              onChange={(e) => {
                setSelectedBrowser(e.target.value);
                setImportResult(null);
                setImportError(null);
              }}
              disabled={isImporting}
            >
              {browsers.map((b) => (
                <option key={b.id} value={b.id}>
                  {b.name}
                  {b.requires_fda ? ' (requires Full Disk Access)' : ''}
                </option>
              ))}
            </select>
            <Button
              variant="secondary"
              size="sm"
              loading={isImporting}
              onClick={handleBrowserImport}
              disabled={!selectedBrowser}
            >
              Import Cookies
            </Button>
          </div>

          {/* Privacy notice */}
          <p className="text-xs text-content-tertiary">
            Reads only Apple Music cookies (apple.com, mzstatic.com) from your browser. No other data is accessed.
          </p>

          {/* Import result -- three states: success with cookies, empty import, or failure */}
          {importResult && (() => {
            const hasAppleCookies = importResult.success && importResult.apple_music_cookies > 0;
            const isEmptyImport = importResult.success && importResult.apple_music_cookies === 0;
            return (
              <div
                className={`p-3 rounded-platform border text-xs ${
                  hasAppleCookies
                    ? 'border-status-success bg-green-50 dark:bg-green-950'
                    : isEmptyImport
                      ? 'border-status-warning bg-yellow-50 dark:bg-yellow-950'
                      : 'border-status-error bg-red-50 dark:bg-red-950'
                }`}
              >
                <div className="flex items-center gap-1.5 mb-1">
                  {hasAppleCookies ? (
                    <CheckCircle size={14} className="text-status-success" />
                  ) : isEmptyImport ? (
                    <AlertTriangle size={14} className="text-status-warning" />
                  ) : (
                    <XCircle size={14} className="text-status-error" />
                  )}
                  <span className="font-medium text-content-primary">
                    {hasAppleCookies
                      ? 'Cookies Imported'
                      : isEmptyImport
                        ? 'No Apple Music Cookies Found'
                        : 'Import Failed'}
                  </span>
                </div>
                {hasAppleCookies && (
                  <p className="text-content-secondary ml-5">
                    {importResult.apple_music_cookies} Apple Music cookies imported
                  </p>
                )}
                {isEmptyImport && (
                  <p className="text-content-secondary ml-5">
                    Log in to music.apple.com in your browser first, then try importing again.
                  </p>
                )}
                {importResult.warnings.map((w, i) => (
                  <p key={i} className="text-status-warning ml-5">{w}</p>
                ))}
              </div>
            );
          })()}

          {/* Import error */}
          {importError && (
            <div className="p-3 rounded-platform border border-status-error bg-red-50 dark:bg-red-950 text-xs">
              <div className="flex items-center gap-1.5">
                <XCircle size={14} className="text-status-error flex-shrink-0" />
                <p className="text-status-error">{importError}</p>
              </div>
            </div>
          )}

          {/* Sign in with Apple Music (embedded login browser) */}
          <div className="pt-2 border-t border-border-light">
            {!isLoginWindowOpen ? (
              /* Show the "Sign in" option when login window is not open */
              <>
                <p className="text-xs text-content-tertiary mb-2">
                  Or sign in directly:
                </p>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={handleOpenLoginWindow}
                  disabled={isImporting}
                >
                  <LogIn size={14} className="mr-1.5" />
                  Sign in with Apple Music
                </Button>
              </>
            ) : (
              /* Show the "Waiting for login..." state when login window is open */
              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <Loader2 size={14} className="animate-spin text-accent" />
                  <span className="text-xs font-medium text-content-primary">
                    Signing in...
                  </span>
                </div>
                <p className="text-xs text-content-tertiary">
                  Sign in with your Apple ID in the browser window, then return here.
                </p>
                <div className="flex gap-2">
                  <Button
                    variant="secondary"
                    size="sm"
                    loading={isExtractingFromLogin}
                    onClick={handleManualExtract}
                  >
                    I&apos;ve signed in
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={handleCancelLogin}
                    disabled={isExtractingFromLogin}
                  >
                    Cancel
                  </Button>
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Loading indicator for browser detection */}
      {isDetecting && (
        <div className="flex items-center gap-2 py-2 text-xs text-content-tertiary">
          <Loader2 size={14} className="animate-spin" />
          Detecting browsers...
        </div>
      )}

      {/* ============================================================
          Section 3: Expandable Browser Instructions (Manual Import)
          Collapsible accordion with per-browser step-by-step guides
          for exporting cookies in Netscape format.
          ============================================================ */}
      <BrowserInstructions />

      {/* ============================================================
          Section 4: Cookie File Picker
          Lets the user browse for and select their cookies.txt file.
          Clearing the selection also resets validation state.
          ============================================================ */}
      <FilePickerButton
        label="Cookies File"
        description="Path to the Netscape-format cookies.txt file"
        value={settings.cookies_path}
        onChange={handleFileChange}
        placeholder="No cookies file selected"
        filters={[{ name: 'Text Files', extensions: ['txt'] }]}
      />

      {/* ============================================================
          Section 5: Action Buttons Row
          Contains the "Validate Cookies" button and, when a file is
          selected, a "Copy Cookie Path" button. Buttons are laid out
          horizontally with a gap.
          ============================================================ */}
      {settings.cookies_path && (
        <div className="flex items-center gap-2">
          {/* Validate button -- triggers backend validation */}
          <Button
            variant="secondary"
            size="sm"
            loading={isValidating}
            onClick={handleValidate}
          >
            Validate Cookies
          </Button>

          {/* Copy path button -- copies the file path to clipboard */}
          <Tooltip
            content={copySuccess ? 'Copied!' : 'Copy file path to clipboard'}
            position="top"
          >
            <Button
              variant="ghost"
              size="sm"
              icon={<Copy size={14} />}
              onClick={handleCopyPath}
            >
              {copySuccess ? 'Copied!' : 'Copy Cookie Path'}
            </Button>
          </Tooltip>
        </div>
      )}

      {/* ============================================================
          Section 6: Validation Results Panel
          Shown only after a successful validation call. Contains:
          - Pass/fail header with icon.
          - Cookie count summary.
          - Detected domains list.
          - Expiry warning (if applicable).
          - Additional backend warnings.
          ============================================================ */}
      {validation && (
        <div
          className={`p-4 rounded-platform border space-y-4 ${
            validation.valid && !validation.expired
              ? 'border-status-success bg-green-50 dark:bg-green-950'
              : validation.expired
                ? 'border-status-warning bg-yellow-50 dark:bg-yellow-950'
                : 'border-status-error bg-red-50 dark:bg-red-950'
          }`}
        >
          {/* ---- Result header (pass / fail / expired icon + label) ---- */}
          <div className="flex items-center gap-2">
            {validation.valid && !validation.expired ? (
              <CheckCircle size={16} className="text-status-success" />
            ) : validation.expired ? (
              <AlertTriangle size={16} className="text-status-warning" />
            ) : (
              <XCircle size={16} className="text-status-error" />
            )}
            <span className="text-sm font-medium text-content-primary">
              {validation.valid && !validation.expired
                ? 'Cookies Valid'
                : validation.expired
                  ? 'Cookies Expired'
                  : 'Cookies Invalid'}
            </span>
          </div>

          {/* ---- Cookie count summary ---- */}
          <div className="text-xs text-content-secondary space-y-1 ml-6">
            <p>
              <span className="font-medium">{validation.cookie_count}</span>{' '}
              total cookies found
            </p>
            <p>
              <span className="font-medium">{validation.apple_music_cookies}</span>{' '}
              Apple Music cookies
            </p>
          </div>

          {/* ---- Detected domains list ---- */}
          {validation.domains.length > 0 && (
            <div className="ml-6">
              <DetectedDomains domains={validation.domains} />
            </div>
          )}

          {/* ---- Expiry warning with estimated days ---- */}
          {(validation.expired || validation.warnings.length > 0) && (
            <div className="ml-6">
              <ExpiryWarning
                expired={validation.expired}
                warnings={validation.warnings}
              />
            </div>
          )}

          {/* ---- Additional backend warnings ---- */}
          {validation.warnings.length > 0 && (
            <div className="text-xs text-content-secondary space-y-1 ml-6">
              {validation.warnings.map((warning, i) => (
                <div key={i} className="flex items-start gap-1.5">
                  <AlertTriangle
                    size={12}
                    className="text-status-warning flex-shrink-0 mt-0.5"
                  />
                  <p className="text-status-warning">{warning}</p>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
