/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file PythonStep.tsx -- Python installation step of the setup wizard.
 *
 * Renders the "Python" step within the {@link SetupWizard}. This step is
 * responsible for ensuring a Python runtime is available for GAMDL to use.
 *
 * ## Behaviour
 *
 * 1. **On mount**, the component calls `checkPython()` (via the dependency
 *    store) which invokes a Tauri IPC command to detect whether Python is
 *    already installed in the application's data directory.
 *
 * 2. **If Python is found**, the component displays a success card with
 *    the detected version and path, and automatically calls
 *    `completeStep('python')` to enable the "Continue" button.
 *
 * 3. **If Python is not found**, the component displays an "Install Python"
 *    button. Clicking it calls `installPython()`, which downloads and
 *    installs a portable Python runtime via the Rust backend. The button
 *    shows a loading state during installation.
 *
 * 4. **On error**, the error message is displayed in a styled error banner.
 *
 * ## Key Design Decisions
 *
 * - The portable Python is self-contained and does not modify the user's
 *   system-wide Python installation.
 * - The `checkPython` call on mount uses the dependency store (not a local
 *   fetch), so the result is shared across the application.
 *
 * ## Store Connections
 *
 * - **dependencyStore**: Provides `python` status, `checkPython`,
 *   `installPython`, `isChecking`, `isInstalling`, and `error`.
 * - **setupStore**: Provides `completeStep` and `setStepError`.
 *
 * @see {@link ../SetupWizard.tsx}             -- Parent wizard container
 * @see {@link @/stores/dependencyStore.ts}    -- Manages dependency status
 * @see {@link @/stores/setupStore.ts}         -- Manages wizard step state
 * @see {@link https://react.dev/reference/react/useEffect} -- useEffect hook
 * @see {@link https://v2.tauri.app/develop/calling-rust/}  -- Tauri IPC commands
 */

// React useEffect for checking status on mount and auto-completing.
import { useEffect } from 'react';

// Lucide icons: CheckCircle for installed state, Download for install button.
import { CheckCircle, Download } from 'lucide-react';

// Zustand stores: dependencyStore tracks Python installation status;
// setupStore manages the wizard step lifecycle.
import { useDependencyStore } from '@/stores/dependencyStore';
import { useSetupStore } from '@/stores/setupStore';

// Shared UI components for the install button and loading indicator.
import { Button, LoadingSpinner } from '@/components/common';

/**
 * PythonStep -- Renders the Python installation step.
 *
 * Displays one of three states:
 *   1. **Checking**: A loading spinner while the backend detects Python.
 *   2. **Installed**: A success card showing version and path.
 *   3. **Not installed**: An install button to download portable Python.
 */
export function PythonStep() {
  // --- Dependency store selectors ---
  /** Python installation status (null until checked) */
  const python = useDependencyStore((s) => s.python);
  /** True while the backend is checking Python availability */
  const isChecking = useDependencyStore((s) => s.isChecking);
  /** True while the Python download/install is in progress */
  const isInstalling = useDependencyStore((s) => s.isInstalling);
  /** Triggers the backend check for Python */
  const checkPython = useDependencyStore((s) => s.checkPython);
  /** Triggers the Python download and installation */
  const installPython = useDependencyStore((s) => s.installPython);
  /** Error message from the most recent operation */
  const error = useDependencyStore((s) => s.error);

  // --- Setup store selectors ---
  /** Marks the 'python' step as completed in the wizard */
  const completeStep = useSetupStore((s) => s.completeStep);
  /** Records an error message for the current wizard step */
  const setStepError = useSetupStore((s) => s.setStepError);

  /**
   * Check Python status on mount by invoking the backend detection command.
   * Runs once (stable function reference from Zustand).
   */
  useEffect(() => {
    checkPython();
  }, [checkPython]);

  /**
   * Auto-complete this wizard step when Python is detected as installed.
   * The `python?.installed` guard handles both null (not yet checked) and
   * false (checked but not installed) cases.
   */
  useEffect(() => {
    if (python?.installed) {
      completeStep('python');
    }
  }, [python, completeStep]);

  /**
   * Handles the "Install Python" button click.
   * Calls the dependency store's installPython action, which invokes the
   * Tauri IPC command to download and extract the portable Python runtime.
   * On failure, the error is recorded in the setup store.
   */
  const handleInstall = async () => {
    try {
      await installPython();
    } catch (e) {
      setStepError(String(e));
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-content-primary">
          Python Runtime
        </h2>
        <p className="text-sm text-content-secondary mt-1">
          GAMDL requires Python to run. A portable Python runtime will be
          downloaded and installed to the application data directory.
        </p>
      </div>

      {/* Status display */}
      <div className="p-4 rounded-platform-lg border border-border-light bg-surface-elevated">
        {isChecking ? (
          <LoadingSpinner size="sm" label="Checking Python status..." />
        ) : python?.installed ? (
          /* Python is installed */
          <div className="flex items-center gap-3">
            <CheckCircle size={20} className="text-status-success" />
            <div>
              <p className="text-sm font-medium text-content-primary">
                Python Installed
              </p>
              <p className="text-xs text-content-secondary">
                Version: {python.version}
              </p>
              {python.path && (
                <p className="text-xs text-content-tertiary font-mono mt-0.5">
                  {python.path}
                </p>
              )}
            </div>
          </div>
        ) : (
          /* Python is not installed */
          <div className="space-y-4">
            <div>
              <p className="text-sm font-medium text-content-primary">
                Python Not Found
              </p>
              <p className="text-xs text-content-secondary mt-1">
                Click below to download and install a portable Python runtime.
                This is a self-contained installation that won't affect any
                system-wide Python installations.
              </p>
            </div>

            <Button
              variant="primary"
              icon={<Download size={16} />}
              loading={isInstalling}
              onClick={handleInstall}
            >
              {isInstalling ? 'Installing Python...' : 'Install Python'}
            </Button>
          </div>
        )}
      </div>

      {/* Error display */}
      {error && (
        <div className="p-3 rounded-platform border border-status-error bg-red-50 dark:bg-red-950 text-sm text-status-error">
          {error}
        </div>
      )}
    </div>
  );
}
