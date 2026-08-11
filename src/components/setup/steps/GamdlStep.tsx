/**
 * Copyright (c) 2026 MeedyaSuite
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

// React hooks for checking status on mount, auto-completing, and holding
// the read-only external-GAMDL detection result.
import { useEffect, useState } from 'react';

// Lucide icons for status display and the install button.
import { CheckCircle, Download, Info } from 'lucide-react';

// Zustand stores for dependency tracking and wizard step management.
import { useDependencyStore } from '@/stores/dependencyStore';
import { useSetupStore } from '@/stores/setupStore';

// Shared UI components.
import { Button, LoadingSpinner } from '@/components/common';

// Read-only detection of a `gamdl` installed outside MeedyaDL's managed venv.
import { detectExternalGamdl } from '@/lib/tauri-commands';
import type { ExternalGamdlInfo } from '@/types';

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
  /** Triggers installation of all bundled pip engines (GAMDL + votify + ...) */
  const installBundledEngines = useDependencyStore((s) => s.installBundledEngines);
  /** Error message from the most recent operation */
  const error = useDependencyStore((s) => s.error);

  // --- Setup store selectors ---
  /** Marks the 'gamdl' step as completed */
  const completeStep = useSetupStore((s) => s.completeStep);
  /** Records an error for the current step */
  const setStepError = useSetupStore((s) => s.setStepError);

  /**
   * A `gamdl` installed outside MeedyaDL's managed venv (e.g. via `pipx`),
   * if any. Purely informational — MeedyaDL keeps and uses its own tested
   * copy and never touches this one.
   */
  const [externalGamdl, setExternalGamdl] = useState<ExternalGamdlInfo | null>(null);

  /** Check GAMDL status on mount */
  useEffect(() => {
    checkGamdl();
  }, [checkGamdl]);

  /**
   * Detect (read-only) any system/pipx GAMDL so we can explain why MeedyaDL
   * still keeps its own copy, rather than looking oblivious to a duplicate.
   * Never blocks the step; failures are silently ignored.
   */
  useEffect(() => {
    let cancelled = false;
    detectExternalGamdl()
      .then((info) => {
        if (!cancelled) setExternalGamdl(info);
      })
      .catch(() => {
        /* Detection is best-effort; ignore failures. */
      });
    return () => {
      cancelled = true;
    };
  }, []);

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
      await installBundledEngines();
    } catch (e) {
      setStepError(String(e));
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-content-primary">Download Engines</h2>
        <p className="text-sm text-content-secondary mt-1">
          Installing core download engines (GAMDL, votify) into the portable Python environment.
          These power Apple Music and Spotify downloads.
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
              <p className="text-sm font-medium text-content-primary">GAMDL Installed</p>
              <p className="text-xs text-content-secondary">Version: {gamdl.version}</p>
            </div>
          </div>
        ) : (
          /* GAMDL is not installed */
          <div className="space-y-4">
            <div>
              <p className="text-sm font-medium text-content-primary">GAMDL Not Found</p>
              <p className="text-xs text-content-secondary mt-1">
                Click below to install GAMDL from PyPI. This uses the portable Python installed in
                the previous step.
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

      {/* Informational note when a GAMDL is also installed outside our venv
          (e.g. via pipx). We deliberately keep our own tested copy and never
          touch the external one, so explain why the "duplicate" exists. */}
      {externalGamdl && (
        <div className="flex items-start gap-2 p-3 rounded-platform border border-border-light bg-surface-secondary text-xs text-content-secondary">
          <Info size={16} className="text-accent flex-shrink-0 mt-0.5" />
          <p>
            GAMDL v{externalGamdl.version} is also installed on your system
            {externalGamdl.source.startsWith('pipx') ? ' via pipx' : ''}. MeedyaDL keeps its own
            tested copy so downloads stay on a supported version — your other installation isn&apos;t
            changed.
          </p>
        </div>
      )}

      {/* Error display */}
      {error && (
        <div className="p-3 rounded-platform border border-status-error bg-status-error-bg text-sm text-status-error">
          {error}
        </div>
      )}
    </div>
  );
}
