/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Python installation step of the setup wizard.
 * Checks if Python is already installed, and if not, provides a button
 * to download and install the portable Python runtime.
 */

import { useEffect } from 'react';
import { CheckCircle, Download } from 'lucide-react';
import { useDependencyStore } from '@/stores/dependencyStore';
import { useSetupStore } from '@/stores/setupStore';
import { Button, LoadingSpinner } from '@/components/common';

/**
 * Renders the Python installation step. Shows current status and
 * provides an install button if Python is not yet available.
 */
export function PythonStep() {
  const python = useDependencyStore((s) => s.python);
  const isChecking = useDependencyStore((s) => s.isChecking);
  const isInstalling = useDependencyStore((s) => s.isInstalling);
  const checkPython = useDependencyStore((s) => s.checkPython);
  const installPython = useDependencyStore((s) => s.installPython);
  const error = useDependencyStore((s) => s.error);
  const completeStep = useSetupStore((s) => s.completeStep);
  const setStepError = useSetupStore((s) => s.setStepError);

  /* Check Python status on mount */
  useEffect(() => {
    checkPython();
  }, [checkPython]);

  /* Mark step as complete when Python is installed */
  useEffect(() => {
    if (python?.installed) {
      completeStep('python');
    }
  }, [python, completeStep]);

  /** Handle Python installation */
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
