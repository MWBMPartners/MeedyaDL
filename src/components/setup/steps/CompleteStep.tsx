/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file CompleteStep.tsx -- Setup complete step of the setup wizard.
 *
 * Renders the final "Done" step within the {@link SetupWizard}. This step
 * serves as a confirmation screen, showing a summary of everything that was
 * installed during the setup process.
 *
 * ## Auto-completion
 *
 * Like the {@link WelcomeStep}, this step auto-completes on mount since no
 * user action is required. This enables the "Get Started" button in the
 * wizard footer, which calls `handleFinish()` to dismiss the wizard.
 *
 * ## Installation Summary
 *
 * The summary reads from the dependency store to show:
 *   - Python version (if installed)
 *   - GAMDL version (if installed)
 *   - All external tools that were installed, with their versions
 *
 * Each installed item is shown with a green checkmark icon.
 *
 * ## Store Connections
 *
 * - **setupStore**: `completeStep('complete')` for auto-completion.
 * - **dependencyStore**: Reads `python`, `gamdl`, and `tools` for the summary.
 *
 * @see {@link ../SetupWizard.tsx}             -- Parent wizard container
 * @see {@link @/stores/setupStore.ts}         -- Zustand store for wizard state
 * @see {@link @/stores/dependencyStore.ts}    -- Zustand store for dependency status
 */

// React useEffect for auto-completing on mount.
import { useEffect } from 'react';

// CheckCircle icon used for each installed item in the summary list.
import { CheckCircle } from 'lucide-react';

// Zustand stores for wizard state and dependency information.
import { useSetupStore } from '@/stores/setupStore';
import { useDependencyStore } from '@/stores/dependencyStore';

/**
 * CompleteStep -- Renders the setup completion screen.
 *
 * Layout:
 *   1. Large success icon (green rounded square with checkmark)
 *   2. "Setup Complete!" heading and subtitle
 *   3. Installation summary card listing all installed components
 *   4. Footer text prompting the user to click "Get Started"
 *
 * Auto-completes on mount to enable the "Get Started" button.
 */
export function CompleteStep() {
  /** Marks the 'complete' wizard step as done */
  const completeStep = useSetupStore((s) => s.completeStep);
  /** Python installation status for the summary */
  const python = useDependencyStore((s) => s.python);
  /** GAMDL installation status for the summary */
  const gamdl = useDependencyStore((s) => s.gamdl);
  /** All external tool statuses for the summary */
  const tools = useDependencyStore((s) => s.tools);

  /** Auto-complete this step on mount (no action required from the user) */
  useEffect(() => {
    completeStep('complete');
  }, [completeStep]);

  /** Filter to only installed tools for the summary display */
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
