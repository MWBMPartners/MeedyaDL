/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Dependencies installation step of the setup wizard.
 * Shows the status of all external tool dependencies (FFmpeg, mp4decrypt, etc.)
 * and provides install buttons for each. Auto-completes when all required
 * dependencies are installed.
 */

import { useEffect } from 'react';
import {
  CheckCircle,
  XCircle,
  Download,
  AlertCircle,
} from 'lucide-react';
import { useDependencyStore } from '@/stores/dependencyStore';
import { useSetupStore } from '@/stores/setupStore';
import { Button, LoadingSpinner } from '@/components/common';

/**
 * Renders the dependencies step showing all tool statuses with
 * individual install buttons.
 */
export function DependenciesStep() {
  const tools = useDependencyStore((s) => s.tools);
  const isChecking = useDependencyStore((s) => s.isChecking);
  const isInstalling = useDependencyStore((s) => s.isInstalling);
  const installingName = useDependencyStore((s) => s.installingName);
  const checkAll = useDependencyStore((s) => s.checkAll);
  const installTool = useDependencyStore((s) => s.installTool);
  const error = useDependencyStore((s) => s.error);
  const completeStep = useSetupStore((s) => s.completeStep);

  /* Check all dependencies on mount */
  useEffect(() => {
    checkAll();
  }, [checkAll]);

  /* Auto-complete when all required tools are installed */
  useEffect(() => {
    const requiredTools = tools.filter((t) => t.required);
    const allInstalled = requiredTools.length > 0 && requiredTools.every((t) => t.installed);
    if (allInstalled) {
      completeStep('dependencies');
    }
  }, [tools, completeStep]);

  /** Install all missing tools sequentially */
  const handleInstallAll = async () => {
    const missing = tools.filter((t) => !t.installed);
    for (const tool of missing) {
      try {
        await installTool(tool.name);
      } catch {
        /* Continue with next tool even if one fails */
      }
    }
  };

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
