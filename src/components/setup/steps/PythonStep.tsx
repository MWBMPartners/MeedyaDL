/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file PythonStep.tsx -- Python installation step of the setup wizard.
 *
 * Renders the "Python" step within the {@link SetupWizard}. This step ensures a
 * Python runtime is available for GAMDL to use.
 *
 * ## Behaviour
 *
 * 1. **On mount**, the component checks whether MeedyaDL's managed Python is
 *    already provisioned (`checkPython()`) AND probes the machine for a
 *    compatible **system** Python (`detectSystemPythons()`, #1017).
 *
 * 2. **If the managed Python is present**, a success card is shown and the step
 *    auto-completes.
 *
 * 3. **If it is not**, the user is offered two paths:
 *    - **Reuse a system Python** (no download): builds an isolated venv from a
 *      detected interpreter — the fast, low-bandwidth option when the user
 *      already has Python 3.10+ installed. A "Browse…" button locates one
 *      manually (mirrors the tool Browse affordance, #278).
 *    - **Download the portable runtime**: the original self-contained flow,
 *      always available as the fallback.
 *
 *    A `diagnosePythonVenv()` check (run alongside `checkPython()` on mount)
 *    can additionally surface a **broken system-venv** notice above these
 *    options: if MeedyaDL previously built its venv from a system Python that
 *    has since moved or been upgraded (e.g. `brew upgrade python` relocating
 *    the Homebrew Cellar path), the notice explains what happened and offers
 *    a one-click **Rebuild** from the same recorded interpreter when it's
 *    still on disk — otherwise it points the user at the reuse/download
 *    options below, which remain the fallback.
 *
 * 4. **On error**, the message is displayed in a styled error banner.
 *
 * ## Key Design Decisions
 *
 * - Reusing a system Python creates a *venv* (required because Homebrew/distro
 *   Pythons are PEP 668 "externally-managed"); the user's system installation
 *   is never modified. The venv lands at `{app_data}/python/`, so every
 *   `python -m gamdl` call site keeps working unchanged.
 * - The floor is Python 3.10 (GAMDL's compiled decrypt module is a cp310-abi3
 *   wheel). Detected interpreters below the floor are surfaced as an
 *   explanation, not offered for reuse.
 *
 * ## Store Connections
 *
 * - **dependencyStore**: `python`, `systemPythons`, `pythonVenvHealth`,
 *   `checkPython`, `detectSystemPythons`, `diagnosePythonVenv`,
 *   `installPython`, `useSystemPython`, `isChecking`, `isDetectingPythons`,
 *   `isInstalling`, `error`.
 * - **setupStore**: `completeStep`, `setStepError`.
 *
 * @see {@link ../SetupWizard.tsx}             -- Parent wizard container
 * @see {@link @/stores/dependencyStore.ts}    -- Manages dependency status
 * @see {@link @/stores/setupStore.ts}         -- Manages wizard step state
 */

// React useEffect for checking status on mount and auto-completing.
import { useEffect } from 'react';

// Lucide icons: success, portable download, system-Python reuse, browse,
// broken-venv notice, rebuild action.
import { AlertTriangle, CheckCircle, Download, FolderOpen, RefreshCw, Terminal } from 'lucide-react';

// Zustand stores: dependencyStore tracks Python status; setupStore manages the
// wizard step lifecycle.
import { useDependencyStore } from '@/stores/dependencyStore';
import { useSetupStore } from '@/stores/setupStore';

// Shared UI components for the install buttons and loading indicator.
import { Button, LoadingSpinner } from '@/components/common';

/**
 * PythonStep -- Renders the Python installation step.
 */
export function PythonStep() {
  // --- Dependency store selectors ---
  /** Managed Python installation status (null until checked) */
  const python = useDependencyStore((s) => s.python);
  /** Compatible system interpreters detected on the machine (#1017) */
  const systemPythons = useDependencyStore((s) => s.systemPythons);
  /** Diagnosis of the managed Python's health (broken system-venv vs. not-installed) */
  const pythonVenvHealth = useDependencyStore((s) => s.pythonVenvHealth);
  /** True while the backend is checking the managed Python */
  const isChecking = useDependencyStore((s) => s.isChecking);
  /** True while the backend is probing for system interpreters */
  const isDetectingPythons = useDependencyStore((s) => s.isDetectingPythons);
  /** True while a download/provision is in progress */
  const isInstalling = useDependencyStore((s) => s.isInstalling);
  /** Triggers the managed-Python check */
  const checkPython = useDependencyStore((s) => s.checkPython);
  /** Triggers system-interpreter detection */
  const detectSystemPythons = useDependencyStore((s) => s.detectSystemPythons);
  /** Triggers the managed-Python venv-health diagnosis */
  const diagnosePythonVenv = useDependencyStore((s) => s.diagnosePythonVenv);
  /** Downloads and installs the portable Python runtime */
  const installPython = useDependencyStore((s) => s.installPython);
  /** Provisions a venv from a chosen system interpreter.
   *  Bound to a non-`use`-prefixed local name so eslint's
   *  react-hooks/rules-of-hooks doesn't mistake the store action
   *  (not a React hook) for one when called in a plain handler. */
  const applySystemPython = useDependencyStore((s) => s.useSystemPython);
  /** Error message from the most recent operation */
  const error = useDependencyStore((s) => s.error);

  // --- Setup store selectors ---
  /** Marks the 'python' step as completed in the wizard */
  const completeStep = useSetupStore((s) => s.completeStep);
  /** Records an error message for the current wizard step */
  const setStepError = useSetupStore((s) => s.setStepError);

  /**
   * On mount: check the managed Python and, in parallel, detect any compatible
   * system Python (so we can offer reuse instead of forcing a download) and
   * diagnose the managed Python's health (so a broken system-venv can be
   * explained instead of showing an undifferentiated "Python Not Found").
   */
  useEffect(() => {
    checkPython();
    detectSystemPythons();
    diagnosePythonVenv();
  }, [checkPython, detectSystemPythons, diagnosePythonVenv]);

  /**
   * Auto-complete this step once a managed Python is provisioned (by either a
   * portable download or a system-Python venv — both set `python.installed`).
   */
  useEffect(() => {
    if (python?.installed) {
      completeStep('python');
    }
  }, [python, completeStep]);

  /** Download + install the portable Python runtime. */
  const handleInstallPortable = async () => {
    try {
      await installPython();
    } catch (e) {
      setStepError(String(e));
    }
  };

  /** Provision a venv from a specific system interpreter. */
  const handleUseSystem = async (interpreterPath: string) => {
    try {
      await applySystemPython(interpreterPath);
    } catch (e) {
      setStepError(String(e));
    }
  };

  /** Open a file picker so the user can locate a Python interpreter manually. */
  const handleBrowse = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        title: 'Locate a Python 3.10+ interpreter',
      });
      if (typeof selected === 'string') {
        await handleUseSystem(selected);
      }
    } catch (e) {
      // A cancelled dialog throws nothing meaningful; a real failure surfaces
      // via the store error already. Record anything unexpected on the step.
      if (e) setStepError(String(e));
    }
  };

  // Interpreters that clear the Python 3.10 floor are offered for reuse; the
  // rest are shown only to explain why they can't be used.
  const reusable = systemPythons.filter((p) => p.meetsFloor);
  const tooOld = systemPythons.filter((p) => !p.meetsFloor);

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-content-primary">Python Runtime</h2>
        <p className="text-sm text-content-secondary mt-1">
          GAMDL needs Python 3.10 or newer. Reuse a Python you already have, or download a
          self-contained runtime — either way, your system stays untouched.
        </p>
      </div>

      {/* Status / action card */}
      <div className="p-4 rounded-platform-lg border border-border-light bg-surface-elevated">
        {isChecking ? (
          <LoadingSpinner size="sm" label="Checking Python status..." />
        ) : python?.installed ? (
          /* Managed Python is provisioned */
          <div className="flex items-center gap-3">
            <CheckCircle size={20} className="text-status-success" />
            <div>
              <p className="text-sm font-medium text-content-primary">
                Python Ready{python.source === 'system' ? ' (using your system Python)' : ''}
              </p>
              <p className="text-xs text-content-secondary">Version: {python.version}</p>
              {python.path && (
                <p className="text-xs text-content-tertiary font-mono mt-0.5">{python.path}</p>
              )}
            </div>
          </div>
        ) : (
          /* Not provisioned yet — offer reuse and/or download */
          <div className="space-y-4">
            {/* Broken system-venv notice (#1017 follow-up): the managed
                Python was previously built from a system interpreter that
                has since moved or been upgraded (e.g. `brew upgrade
                python` relocating the Homebrew Cellar path), so the venv's
                own interpreter link no longer runs. Rendered ABOVE the
                normal reuse/download options, which stay available as the
                fallback either way. */}
            {pythonVenvHealth?.status === 'broken_system_venv' && (
              <div className="p-3 rounded-platform border border-status-warning bg-status-warning-bg space-y-2.5">
                <div className="flex items-start gap-2.5">
                  <AlertTriangle size={16} className="text-status-warning shrink-0 mt-0.5" />
                  <div className="text-sm text-content-primary space-y-1.5">
                    <p className="font-medium">Your Python environment stopped working</p>
                    <p className="text-xs text-content-secondary">
                      {pythonVenvHealth.recordedInterpreter ? (
                        <>
                          MeedyaDL built its Python environment from{' '}
                          <span className="font-mono text-content-primary">
                            {pythonVenvHealth.recordedInterpreter}
                          </span>
                          {pythonVenvHealth.recordedVersion
                            ? ` (Python ${pythonVenvHealth.recordedVersion})`
                            : ''}
                          , but that interpreter no longer runs — for example after running{' '}
                          <span className="font-mono text-content-primary">
                            brew upgrade python
                          </span>
                          .
                        </>
                      ) : (
                        "MeedyaDL's Python environment was built from a system Python that no " +
                        'longer runs, most likely because it moved or was upgraded.'
                      )}
                    </p>
                  </div>
                </div>
                {pythonVenvHealth.recordedInterpreter && pythonVenvHealth.recordedInterpreterExists ? (
                  <Button
                    variant="primary"
                    size="sm"
                    icon={<RefreshCw size={16} />}
                    loading={isInstalling}
                    disabled={isInstalling}
                    onClick={() => handleUseSystem(pythonVenvHealth.recordedInterpreter as string)}
                  >
                    Rebuild from this Python
                  </Button>
                ) : (
                  <p className="text-xs text-content-tertiary pl-[26px]">
                    That interpreter is no longer available — choose a replacement below.
                  </p>
                )}
              </div>
            )}

            {isDetectingPythons ? (
              <LoadingSpinner size="sm" label="Looking for an existing Python..." />
            ) : reusable.length > 0 ? (
              <div className="space-y-3">
                <div>
                  <p className="text-sm font-medium text-content-primary">
                    Use a Python you already have
                  </p>
                  <p className="text-xs text-content-secondary mt-1">
                    No download needed. MeedyaDL creates an isolated environment from it — your
                    system Python is left completely untouched.
                  </p>
                </div>

                {/* One row per reusable interpreter (best first) */}
                <div className="space-y-2">
                  {reusable.map((p) => (
                    <div
                      key={p.path}
                      className="flex items-center justify-between gap-3 p-2.5 rounded-platform border border-border-light bg-surface-base"
                    >
                      <div className="flex items-center gap-2.5 min-w-0">
                        <Terminal size={16} className="text-content-secondary shrink-0" />
                        <div className="min-w-0">
                          <p className="text-sm text-content-primary">
                            Python {p.version}{' '}
                            <span className="text-content-tertiary">· {p.source}</span>
                          </p>
                          <p className="text-xs text-content-tertiary font-mono truncate">
                            {p.path}
                          </p>
                        </div>
                      </div>
                      <Button
                        variant="primary"
                        size="sm"
                        loading={isInstalling}
                        disabled={isInstalling}
                        onClick={() => handleUseSystem(p.path)}
                      >
                        Use this
                      </Button>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              /* No reusable interpreter found */
              <div>
                <p className="text-sm font-medium text-content-primary">
                  No compatible Python found
                </p>
                <p className="text-xs text-content-secondary mt-1">
                  {tooOld.length > 0
                    ? `Found Python ${tooOld[0].version}, but GAMDL needs 3.10 or newer. Download the portable runtime below, or Browse to a newer Python.`
                    : "Download a portable Python runtime below, or Browse to locate one you've already installed."}
                </p>
              </div>
            )}

            {/* Fallback + manual options — always available */}
            <div className="flex flex-wrap items-center gap-2 pt-1">
              <Button
                variant={reusable.length > 0 ? 'secondary' : 'primary'}
                icon={<Download size={16} />}
                loading={isInstalling}
                disabled={isInstalling}
                onClick={handleInstallPortable}
              >
                {isInstalling ? 'Setting up Python...' : 'Download portable Python'}
              </Button>
              <Button
                variant="ghost"
                icon={<FolderOpen size={16} />}
                disabled={isInstalling}
                title="Locate a Python 3.10+ interpreter on your system"
                onClick={handleBrowse}
              >
                Browse…
              </Button>
            </div>
          </div>
        )}
      </div>

      {/* Error display */}
      {error && (
        <div className="p-3 rounded-platform border border-status-error bg-status-error-bg text-sm text-status-error">
          {error}
        </div>
      )}
    </div>
  );
}
