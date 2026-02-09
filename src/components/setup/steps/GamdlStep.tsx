/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * GAMDL installation step of the setup wizard.
 * Checks if GAMDL is installed in the portable Python, and if not,
 * installs it via pip.
 */

import { useEffect } from 'react';
import { CheckCircle, Download } from 'lucide-react';
import { useDependencyStore } from '@/stores/dependencyStore';
import { useSetupStore } from '@/stores/setupStore';
import { Button, LoadingSpinner } from '@/components/common';

/**
 * Renders the GAMDL installation step. Shows current status and
 * provides an install button if GAMDL is not yet available.
 */
export function GamdlStep() {
  const gamdl = useDependencyStore((s) => s.gamdl);
  const isChecking = useDependencyStore((s) => s.isChecking);
  const isInstalling = useDependencyStore((s) => s.isInstalling);
  const checkGamdl = useDependencyStore((s) => s.checkGamdl);
  const installGamdl = useDependencyStore((s) => s.installGamdl);
  const error = useDependencyStore((s) => s.error);
  const completeStep = useSetupStore((s) => s.completeStep);
  const setStepError = useSetupStore((s) => s.setStepError);

  /* Check GAMDL status on mount */
  useEffect(() => {
    checkGamdl();
  }, [checkGamdl]);

  /* Mark step as complete when GAMDL is installed */
  useEffect(() => {
    if (gamdl?.installed) {
      completeStep('gamdl');
    }
  }, [gamdl, completeStep]);

  /** Handle GAMDL installation */
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
