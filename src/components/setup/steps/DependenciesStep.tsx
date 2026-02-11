/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file DependenciesStep.tsx -- External tool dependencies installation step.
 *
 * Renders the "Tools" step within the {@link SetupWizard}. This step shows
 * a list of all external tool dependencies that GAMDL requires or can
 * optionally use, along with their installation status and individual
 * install buttons.
 *
 * ## Tool List
 *
 * The dependency store provides a `tools` array of `DependencyStatus`
 * objects, each containing:
 *   - `name`: Tool name (e.g., "FFmpeg", "mp4decrypt")
 *   - `installed`: Boolean indicating whether the tool is available
 *   - `version`: Detected version string (if installed)
 *   - `required`: Whether the tool is required (vs. optional)
 *
 * ## Completion Detection
 *
 * This step auto-completes when **all required tools** are installed.
 * Optional tools do not block progression. The `useEffect` hook checks
 * this condition whenever the `tools` array changes (e.g., after an
 * installation finishes and triggers a status re-check).
 *
 * ## "Install All" Behaviour
 *
 * The "Install All" button iterates over missing tools **sequentially**
 * (not in parallel) using a `for...of` loop with `await`. If one tool
 * fails to install, the loop catches the error and continues with the
 * next tool. This ensures partial progress is preserved.
 *
 * ## Visual States
 *
 * Each tool row shows one of three status icons:
 *   - Green checkmark: Installed
 *   - Red X circle: Required but not installed
 *   - Grey alert circle: Optional and not installed
 *
 * ## Store Connections
 *
 * - **dependencyStore**: `tools` array, `checkAll`, `installTool`,
 *   `isChecking`, `isInstalling`, `installingName`, `error`.
 * - **setupStore**: `completeStep('dependencies')`.
 *
 * @see {@link ../SetupWizard.tsx}             -- Parent wizard container
 * @see {@link @/stores/dependencyStore.ts}    -- Manages dependency status
 * @see {@link @/stores/setupStore.ts}         -- Manages wizard step state
 */

// React useEffect for checking dependencies on mount and auto-completing.
import { useEffect } from 'react';

// Lucide icons for the three tool status states and install button.
import {
  CheckCircle,   // Installed (green)
  XCircle,       // Required but missing (red)
  Download,      // Install button icon
  AlertCircle,   // Optional and missing (grey)
} from 'lucide-react';

// Zustand stores for dependency tracking and wizard step management.
import { useDependencyStore } from '@/stores/dependencyStore';
import { useSetupStore } from '@/stores/setupStore';

// Shared UI components.
import { Button, LoadingSpinner } from '@/components/common';

/**
 * DependenciesStep -- Renders the external tools installation step.
 *
 * Shows all tools in a vertical list with status indicators and install
 * buttons. An "Install All" button at the top installs all missing tools
 * sequentially. Auto-completes when all required tools are installed.
 */
export function DependenciesStep() {
  // --- Dependency store selectors ---
  /** Array of tool dependency statuses */
  const tools = useDependencyStore((s) => s.tools);
  /** True while the backend is checking tool availability */
  const isChecking = useDependencyStore((s) => s.isChecking);
  /** True while any tool installation is in progress */
  const isInstalling = useDependencyStore((s) => s.isInstalling);
  /** Name of the tool currently being installed (for button label) */
  const installingName = useDependencyStore((s) => s.installingName);
  /** Triggers status checks for all tools at once */
  const checkAll = useDependencyStore((s) => s.checkAll);
  /** Triggers installation of a specific tool by name */
  const installTool = useDependencyStore((s) => s.installTool);
  /** Error message from the most recent operation */
  const error = useDependencyStore((s) => s.error);

  // --- Setup store selectors ---
  /** Marks the 'dependencies' step as completed */
  const completeStep = useSetupStore((s) => s.completeStep);

  /** Check all dependency statuses on mount */
  useEffect(() => {
    checkAll();
  }, [checkAll]);

  /**
   * Auto-complete the step when all REQUIRED tools are installed.
   * Filters the tools array to only required tools, then checks that
   * the array is non-empty (tools have been loaded) and every required
   * tool has `installed: true`.
   */
  useEffect(() => {
    const requiredTools = tools.filter((t) => t.required);
    const allInstalled = requiredTools.length > 0 && requiredTools.every((t) => t.installed);
    if (allInstalled) {
      completeStep('dependencies');
    }
  }, [tools, completeStep]);

  /**
   * Installs all missing tools sequentially.
   * Uses a for...of loop with await to ensure tools are installed one
   * at a time (some may depend on others, and sequential installation
   * avoids resource contention). Errors are caught per-tool so that a
   * failure in one tool does not prevent the rest from being installed.
   */
  const handleInstallAll = async () => {
    const missing = tools.filter((t) => !t.installed);
    for (const tool of missing) {
      try {
        await installTool(tool.name);
      } catch {
        /* Continue with the next tool even if this one fails */
      }
    }
  };

  /** Count of tools that are not yet installed (drives "Install All" button label) */
  const missingCount = tools.filter((t) => !t.installed).length;

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-content-primary">
          External Tools
        </h2>
        <p className="text-sm text-content-secondary mt-1">
          GAMDL uses several external tools for processing downloads. Required
          tools must be installed; optional tools provide additional features.
        </p>
      </div>

      {/* Loading state */}
      {isChecking ? (
        <LoadingSpinner label="Checking dependencies..." />
      ) : (
        <>
          {/* Install all button */}
          {missingCount > 0 && (
            <Button
              variant="primary"
              icon={<Download size={16} />}
              loading={isInstalling}
              onClick={handleInstallAll}
            >
              {isInstalling
                ? `Installing ${installingName}...`
                : `Install All (${missingCount} missing)`}
            </Button>
          )}

          {/* Tool list */}
          <div className="space-y-2">
            {tools.map((tool) => (
              <div
                key={tool.name}
                className="flex items-center gap-3 p-3 rounded-platform border border-border-light bg-surface-elevated"
              >
                {/* Status icon */}
                {tool.installed ? (
                  <CheckCircle size={18} className="text-status-success flex-shrink-0" />
                ) : tool.required ? (
                  <XCircle size={18} className="text-status-error flex-shrink-0" />
                ) : (
                  <AlertCircle size={18} className="text-content-tertiary flex-shrink-0" />
                )}

                {/* Tool info */}
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-content-primary">
                      {tool.name}
                    </span>
                    {tool.required && (
                      <span className="text-[10px] font-medium px-1.5 py-0.5 rounded-full bg-status-error/10 text-status-error">
                        Required
                      </span>
                    )}
                    {!tool.required && (
                      <span className="text-[10px] font-medium px-1.5 py-0.5 rounded-full bg-surface-secondary text-content-tertiary">
                        Optional
                      </span>
                    )}
                  </div>
                  {tool.version && (
                    <p className="text-xs text-content-secondary">
                      v{tool.version}
                    </p>
                  )}
                </div>

                {/* Install button for missing tools */}
                {!tool.installed && (
                  <Button
                    variant="secondary"
                    size="sm"
                    icon={<Download size={14} />}
                    loading={isInstalling && installingName === tool.name}
                    disabled={isInstalling}
                    onClick={() => installTool(tool.name)}
                  >
                    Install
                  </Button>
                )}
              </div>
            ))}
          </div>
        </>
      )}

      {/* Error display */}
      {error && (
        <div className="p-3 rounded-platform border border-status-error bg-red-50 dark:bg-red-950 text-sm text-status-error">
          {error}
        </div>
      )}
    </div>
  );
}
