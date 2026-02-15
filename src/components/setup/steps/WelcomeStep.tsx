/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file WelcomeStep.tsx -- Welcome screen step of the setup wizard.
 *
 * Renders the introductory screen that the user sees when the setup wizard
 * first appears. This step serves a purely informational purpose: it
 * introduces the application, explains what the wizard will do, and lists
 * the steps ahead.
 *
 * ## Auto-completion
 *
 * Since no user action is required on this step, it automatically calls
 * `completeStep('welcome')` in a `useEffect` on mount. This enables the
 * "Continue" button in the parent {@link SetupWizard} immediately.
 *
 * ## Store Connection
 *
 * - **setupStore**: Calls `completeStep('welcome')` to unblock navigation.
 *
 * @see {@link ../SetupWizard.tsx}         -- Parent wizard container
 * @see {@link @/stores/setupStore.ts}     -- Zustand store for wizard state
 * @see {@link https://react.dev/reference/react/useEffect} -- useEffect hook
 */

// React useEffect for auto-completing the step on mount.
import { useEffect } from 'react';

// Icons used in the welcome step layout.
import { Download, AlertTriangle } from 'lucide-react';

// Zustand store providing the completeStep action.
import { useSetupStore } from '@/stores/setupStore';

/**
 * WelcomeStep -- Renders the welcome screen.
 *
 * Layout:
 *   1. Large app icon (Download icon in an accent-coloured rounded square)
 *   2. Welcome heading and subtitle
 *   3. Numbered list of setup steps the wizard will perform
 *   4. Footnote about sandboxed installation
 *
 * The component calls `completeStep('welcome')` on mount to signal the
 * parent wizard that this step's requirements are satisfied (there are
 * none), enabling the "Continue" button.
 */
export function WelcomeStep() {
  /** Marks a wizard step as completed in the setupStore */
  const completeStep = useSetupStore((s) => s.completeStep);

  /**
   * Auto-complete this step on mount.
   * The Welcome step has no requirements -- it is purely informational.
   * Calling completeStep('welcome') adds 'welcome' to the completedSteps
   * set, which enables the "Continue" button in the wizard footer.
   * @see https://react.dev/reference/react/useEffect
   */
  useEffect(() => {
    completeStep('welcome');
  }, [completeStep]);

  return (
    <div className="text-center space-y-6">
      {/* App icon */}
      <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-accent">
        <Download size={40} className="text-content-inverse" />
      </div>

      {/* Welcome heading */}
      <div>
        <h2 className="text-2xl font-bold text-content-primary">
          Welcome to GAMDL
        </h2>
        <p className="text-base text-content-secondary mt-2">
          Apple Music Downloader
        </p>
      </div>

      {/* Setup description */}
      <div className="text-left max-w-md mx-auto space-y-3 text-sm text-content-secondary">
        <p>
          This wizard will help you set up everything needed to download music
          and videos from Apple Music. Here's what we'll do:
        </p>

        <ol className="list-decimal list-inside space-y-2 ml-2">
          <li>
            <strong className="text-content-primary">Install Python</strong> - A
            portable Python runtime will be downloaded (no system changes)
          </li>
          <li>
            <strong className="text-content-primary">Install GAMDL</strong> -
            The Apple Music download tool will be installed into the portable
            Python
          </li>
          <li>
            <strong className="text-content-primary">Install Tools</strong> -
            Required tools like FFmpeg and mp4decrypt will be downloaded
          </li>
          <li>
            <strong className="text-content-primary">Import Cookies</strong> -
            You'll provide your Apple Music authentication cookies
          </li>
        </ol>

        <p className="text-xs text-content-tertiary">
          All files are installed to the application data directory. Nothing is
          installed system-wide.
        </p>

        {/* Disclaimer notice */}
        <div className="mt-4 p-3 rounded-platform border border-status-warning/30 bg-status-warning/5">
          <div className="flex items-start gap-2">
            <AlertTriangle
              size={14}
              className="text-status-warning flex-shrink-0 mt-0.5"
            />
            <div className="text-xs text-content-tertiary leading-relaxed">
              <strong className="text-content-secondary">Disclaimer:</strong>{' '}
              MeedyaDL relies on third-party libraries and services. Quality of
              service, features, and performance are not guaranteed. While we
              endeavour to provide updates and fixes, no liability is accepted
              for loss of functionality. By continuing, you acknowledge and
              accept these terms.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
