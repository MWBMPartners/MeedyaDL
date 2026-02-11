/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file GamdlStep.tsx -- GAMDL package installation step of the setup wizard.
 *
 * Renders the "GAMDL" step within the {@link SetupWizard}. This step ensures
 * that the GAMDL Python package (the Apple Music download engine) is installed
 * into the portable Python environment set up in the previous step.
 *
 * ## Behaviour
 *
 * 1. **On mount**, calls `checkGamdl()` to detect whether GAMDL is already
 *    installed by running `pip show gamdl` in the portable Python.
 *
 * 2. **If GAMDL is found**, displays version info and auto-completes.
 *
 * 3. **If not found**, shows an "Install GAMDL" button that runs
 *    `pip install gamdl` in the portable Python via the Rust backend.
 *
 * ## Dependencies
 *
 * This step requires Python to be installed first (previous wizard step).
 * The dependency store's `installGamdl` action targets the portable Python
 * that was set up during the Python step.
 *
 * ## Store Connections
 *
 * - **dependencyStore**: `gamdl` status, `checkGamdl`, `installGamdl`,
 *   `isChecking`, `isInstalling`, `error`.
 * - **setupStore**: `completeStep('gamdl')`, `setStepError`.
 *
 * @see {@link ../SetupWizard.tsx}             -- Parent wizard container
 * @see {@link ./PythonStep.tsx}               -- Previous step (Python must be installed first)
 * @see {@link @/stores/dependencyStore.ts}    -- Manages dependency status
 * @see {@link @/stores/setupStore.ts}         -- Manages wizard step state
 */

// React useEffect for checking status on mount and auto-completing.
import { useEffect } from 'react';

// Lucide icons for status display and the install button.
import { CheckCircle, Download } from 'lucide-react';

// Zustand stores for dependency tracking and wizard step management.
import { useDependencyStore } from '@/stores/dependencyStore';
import { useSetupStore } from '@/stores/setupStore';

// Shared UI components.
import { Button, LoadingSpinner } from '@/components/common';

/**
 * GamdlStep -- Renders the GAMDL installation step.
 *
 * Structurally identical to {@link PythonStep} but targets the GAMDL
 * Python package instead of the Python runtime. Displays one of three
 * states: checking, installed (success), or not installed (install button).
 */
export function GamdlStep() {
  // --- Dependency store selectors ---
  /** GAMDL installation status (null until checked) */
  const gamdl = useDependencyStore((s) => s.gamdl);
  /** True while the backend is checking GAMDL availability */
  const isChecking = useDependencyStore((s) => s.isChecking);
  /** True while the GAMDL pip install is in progress */
  const isInstalling = useDependencyStore((s) => s.isInstalling);
  /** Triggers the backend check for GAMDL */
  const checkGamdl = useDependencyStore((s) => s.checkGamdl);
  /** Triggers the GAMDL pip installation */
  const installGamdl = useDependencyStore((s) => s.installGamdl);
  /** Error message from the most recent operation */
  const error = useDependencyStore((s) => s.error);

  // --- Setup store selectors ---
  /** Marks the 'gamdl' step as completed */
  const completeStep = useSetupStore((s) => s.completeStep);
  /** Records an error for the current step */
  const setStepError = useSetupStore((s) => s.setStepError);

  /** Check GAMDL status on mount */
  useEffect(() => {
    checkGamdl();
  }, [checkGamdl]);

  /** Auto-complete when GAMDL is detected as installed */
  useEffect(() => {
    if (gamdl?.installed) {
      completeStep('gamdl');
    }
  }, [gamdl, completeStep]);

  /**
   * Handles the "Install GAMDL" button click.
   * Calls the dependency store's installGamdl action, which invokes
   * `pip install gamdl` in the portable Python via Tauri IPC.
   */
  const handleInstall = async () => {
    try {
      await installGamdl();
    } catch (e) {
      setStepError(String(e));
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-content-primary">
          GAMDL Package
        </h2>
        <p className="text-sm text-content-secondary mt-1">
          GAMDL is the Apple Music download tool that powers this application.
          It will be installed into the portable Python environment.
        </p>
      </div>

      {/* Status display */}
      <div className="p-4 rounded-platform-lg border border-border-light bg-surface-elevated">
        {isChecking ? (
          <LoadingSpinner size="sm" label="Checking GAMDL status..." />
        ) : gamdl?.installed ? (
          /* GAMDL is installed */
          <div className="flex items-center gap-3">
            <CheckCircle size={20} className="text-status-success" />
            <div>
              <p className="text-sm font-medium text-content-primary">
                GAMDL Installed
              </p>
              <p className="text-xs text-content-secondary">
                Version: {gamdl.version}
              </p>
            </div>
          </div>
        ) : (
          /* GAMDL is not installed */
          <div className="space-y-4">
            <div>
              <p className="text-sm font-medium text-content-primary">
                GAMDL Not Found
              </p>
              <p className="text-xs text-content-secondary mt-1">
                Click below to install GAMDL from PyPI. This uses the portable
                Python installed in the previous step.
              </p>
            </div>

            <Button
              variant="primary"
              icon={<Download size={16} />}
              loading={isInstalling}
              onClick={handleInstall}
            >
              {isInstalling ? 'Installing GAMDL...' : 'Install GAMDL'}
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
