/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Completion step of the setup wizard.
 * Shows a summary of what was installed and a button to start using the app.
 * Auto-completes on mount.
 */

import { useEffect } from 'react';
import { CheckCircle } from 'lucide-react';
import { useSetupStore } from '@/stores/setupStore';
import { useDependencyStore } from '@/stores/dependencyStore';

/**
 * Renders the completion step with installation summary.
 * Auto-marks itself as completed on mount.
 */
export function CompleteStep() {
  const completeStep = useSetupStore((s) => s.completeStep);
  const python = useDependencyStore((s) => s.python);
  const gamdl = useDependencyStore((s) => s.gamdl);
  const tools = useDependencyStore((s) => s.tools);

  /* Auto-complete this step on mount */
  useEffect(() => {
    completeStep('complete');
  }, [completeStep]);

  const installedTools = tools.filter((t) => t.installed);

  return (
    <div className="text-center space-y-6">
      {/* Success icon */}
      <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-status-success">
        <CheckCircle size={40} className="text-white" />
      </div>

      {/* Heading */}
      <div>
        <h2 className="text-2xl font-bold text-content-primary">
          Setup Complete!
        </h2>
        <p className="text-base text-content-secondary mt-2">
          Everything is ready. You can now start downloading music.
        </p>
      </div>

      {/* Installation summary */}
      <div className="text-left max-w-md mx-auto p-4 rounded-platform-lg border border-border-light bg-surface-elevated">
        <h3 className="text-sm font-semibold text-content-primary mb-3">
          Installation Summary
        </h3>
        <div className="space-y-2 text-sm">
          {/* Python */}
          {python?.installed && (
            <div className="flex items-center gap-2 text-content-secondary">
              <CheckCircle size={14} className="text-status-success flex-shrink-0" />
              Python {python.version}
            </div>
          )}

          {/* GAMDL */}
          {gamdl?.installed && (
            <div className="flex items-center gap-2 text-content-secondary">
              <CheckCircle size={14} className="text-status-success flex-shrink-0" />
              GAMDL {gamdl.version}
            </div>
          )}

          {/* Tools */}
          {installedTools.map((tool) => (
            <div
              key={tool.name}
              className="flex items-center gap-2 text-content-secondary"
            >
              <CheckCircle size={14} className="text-status-success flex-shrink-0" />
              {tool.name} {tool.version && `v${tool.version}`}
            </div>
          ))}
        </div>
      </div>

      <p className="text-xs text-content-tertiary">
        Click "Get Started" below to begin using GAMDL
      </p>
    </div>
  );
}
