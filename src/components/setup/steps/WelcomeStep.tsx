/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Welcome step of the setup wizard.
 * Introduces the application and explains what the setup process will do.
 * Auto-completes on mount since there's nothing to configure.
 */

import { useEffect } from 'react';
import { Download } from 'lucide-react';
import { useSetupStore } from '@/stores/setupStore';

/**
 * Renders the welcome step with app introduction and setup overview.
 * Automatically marks itself as completed so the user can proceed.
 */
export function WelcomeStep() {
  const completeStep = useSetupStore((s) => s.completeStep);

  /* Auto-complete this step on mount (no action required from the user) */
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
      </div>
    </div>
  );
}
