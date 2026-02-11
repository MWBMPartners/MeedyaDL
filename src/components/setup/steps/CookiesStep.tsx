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
 * ## Behaviour
 *
 * 1. Displays instructions on how to export cookies from a browser.
 * 2. Provides a FilePickerButton to select the exported cookies.txt file.
 * 3. Offers a "Validate Cookies" button that sends the file to the Rust
 *    backend for validation via the `validate_cookies_file` Tauri command.
 * 4. Shows validation results (pass/fail, cookie count, warnings).
 * 5. Provides a "Skip for Now" button that completes the step without
 *    cookies, allowing the user to configure them later in Settings.
 *
 * ## Completion Detection
 *
 * The step can be completed in two ways:
 *   - **Validation success**: When `validation.valid` is true, the step
 *     auto-completes via a useEffect hook.
 *   - **Skip**: When the user clicks "Skip for Now", `completeStep('cookies')`
 *     is called directly.
 *
 * ## Differences from CookiesTab
 *
 * This component is a simplified version of the settings-page CookiesTab.
 * It omits the status badge, detected domains, expiry warning, and per-browser
 * accordion instructions in favour of a compact inline instruction list.
 * The full-featured version is available in Settings after setup completes.
 *
 * ## Store Connections
 *
 * - **settingsStore**: Reads/writes `settings.cookies_path`.
 * - **setupStore**: `completeStep('cookies')`.
 * - **Tauri commands**: `validateCookiesFile` for backend validation.
 *
 * @see {@link ../SetupWizard.tsx}                             -- Parent wizard container
 * @see {@link ../../settings/tabs/CookiesTab.tsx}             -- Full-featured cookies settings
 * @see {@link @/stores/settingsStore.ts}                      -- Zustand store for settings
 * @see {@link @/stores/setupStore.ts}                         -- Zustand store for wizard state
 * @see {@link @/lib/tauri-commands.ts}                        -- Tauri IPC command wrappers
 */

// React hooks for local state and auto-completion effects.
import { useState, useEffect } from 'react';

// Lucide icons for the instruction card and validation results.
import {
  Shield,         // Authentication context icon
  CheckCircle,    // Valid cookies indicator
  AlertTriangle,  // Invalid cookies indicator
} from 'lucide-react';

// Zustand stores for reading/writing cookies path and managing wizard state.
import { useSettingsStore } from '@/stores/settingsStore';
import { useSetupStore } from '@/stores/setupStore';

// Tauri IPC command wrappers for cookie file validation.
import * as commands from '@/lib/tauri-commands';

// Shared UI components for the file picker and action buttons.
import { FilePickerButton, Button } from '@/components/common';

// TypeScript type for the cookie validation result.
import type { CookieValidation } from '@/types';

/**
 * CookiesStep -- Renders the cookie import step.
 *
 * Layout:
 *   1. Heading and description
 *   2. Instructions card with numbered steps
 *   3. File picker for the cookies.txt file
 *   4. "Validate Cookies" and "Skip for Now" buttons
 *   5. Validation results panel (if validation has been run)
 */
export function CookiesStep() {
  // --- Store bindings ---
  /** Current settings (for reading cookies_path) */
  const settings = useSettingsStore((s) => s.settings);
  /** Partial-update function for persisting the cookies path */
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  /** Marks the 'cookies' wizard step as completed */
  const completeStep = useSetupStore((s) => s.completeStep);

  // --- Local state ---
  /** Result of the most recent cookie validation (null = not yet validated) */
  const [validation, setValidation] = useState<CookieValidation | null>(null);
  /** Whether a validation request is currently in-flight */
  const [isValidating, setIsValidating] = useState(false);

  /**
   * Auto-complete this step when validation succeeds.
   * Watches the `validation` state -- when the backend reports the cookies
   * are valid, the step is automatically marked as completed, enabling
   * the "Continue" button in the wizard footer.
   */
  useEffect(() => {
    if (validation?.valid) {
      completeStep('cookies');
    }
  }, [validation, completeStep]);

  /**
   * Validates the selected cookies file via the Rust backend.
   * Sends the file path to the `validate_cookies_file` Tauri IPC command
   * and stores the result in local state. On error, clears the validation
   * state so stale results are not displayed.
   */
  const handleValidate = async () => {
    if (!settings.cookies_path) return; // Guard: no file selected
    setIsValidating(true);
    try {
      const result = await commands.validateCookiesFile(settings.cookies_path);
      setValidation(result);
    } catch {
      setValidation(null); // Clear stale results on error
    }
    setIsValidating(false);
  };

  /**
   * Skips the cookie import step.
   * The user can configure cookies later from Settings > Cookies.
   * Calling completeStep directly enables the "Continue" button
   * without requiring valid cookies.
   */
  const handleSkip = () => {
    completeStep('cookies');
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-content-primary">
          Apple Music Cookies
        </h2>
        <p className="text-sm text-content-secondary mt-1">
          GAMDL needs your Apple Music cookies to authenticate downloads. You
          can skip this step and configure cookies later in Settings.
        </p>
      </div>

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

      {/* Validate and Skip buttons */}
      <div className="flex gap-3">
        {settings.cookies_path && (
          <Button
            variant="primary"
            loading={isValidating}
            onClick={handleValidate}
          >
            Validate Cookies
          </Button>
        )}
        <Button variant="ghost" onClick={handleSkip}>
          Skip for Now
        </Button>
      </div>

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
            <p>{validation.apple_music_cookies} Apple Music cookies found</p>
            {validation.warnings.map((w, i) => (
              <p key={i} className="text-status-warning">{w}</p>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
