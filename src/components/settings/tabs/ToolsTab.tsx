/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file ToolsTab.tsx -- External tool management settings tab.
 *
 * Renders the "Tools" tab within the {@link SettingsPage} component.
 * Provides a unified view for managing all external dependencies:
 *
 * ## Section 1: Core Dependencies
 *
 *   Shows the installation status of **Python** (portable runtime) and
 *   **GAMDL** (the Apple Music downloader package). Each has an Install
 *   or Update button. These are the minimum requirements for the app
 *   to function — the download page is gated on both being installed.
 *
 * ## Section 2: External Tools
 *
 *   Shows all external tool dependencies (FFmpeg, mp4decrypt, N_m3u8DL-RE,
 *   MP4Box, AMDecrypt) with:
 *     - Status icon (green check / red X / grey alert)
 *     - Tool name + Required/Optional badge + System badge
 *     - Version string (if installed)
 *     - Install button (if missing)
 *     - Custom path override (FilePickerButton)
 *
 *   "Check All" refreshes statuses; "Install All Missing" installs
 *   missing tools sequentially (same pattern as the setup wizard's
 *   DependenciesStep).
 *
 * ## Section 3: Directories
 *
 *   Temp directory configuration (moved from the former PathsTab).
 *
 * ## Store Connections
 *
 * - **dependencyStore**: `python`, `gamdl`, `tools`, `checkAll`,
 *   `installPython`, `installGamdl`, `installTool`, operation flags.
 * - **settingsStore**: `settings.*_path` fields for tool path overrides,
 *   `settings.temp_path` for the temp directory.
 *
 * @see {@link ../SettingsPage.tsx}            -- Parent container
 * @see {@link @/stores/dependencyStore.ts}    -- Dependency status store
 * @see {@link @/stores/settingsStore.ts}      -- Settings store
 * @see {@link ../../setup/steps/DependenciesStep.tsx} -- Setup wizard equivalent
 */

import { useEffect } from 'react';

import {
  CheckCircle,
  XCircle,
  Download,
  AlertCircle,
  RefreshCw,
  ChevronDown,
  ChevronRight,
} from 'lucide-react';

import { useDependencyStore } from '@/stores/dependencyStore';
import { useSettingsStore } from '@/stores/settingsStore';


import { Button, LoadingSpinner, FilePickerButton } from '@/components/common';

import { useState } from 'react';

/**
 * Maps tool names from the dependency store to their corresponding
 * settings path keys. Used to connect each tool's custom path override
 * FilePickerButton to the correct settings field.
 */
const TOOL_PATH_KEYS: Record<string, string> = {
  FFmpeg: 'ffmpeg_path',
  mp4decrypt: 'mp4decrypt_path',
  'N_m3u8DL-RE': 'nm3u8dlre_path',
  MP4Box: 'mp4box_path',
  AMDecrypt: 'amdecrypt_path',
};

/**
 * User-friendly descriptions for each tool's custom path override.
 */
const TOOL_PATH_DESCRIPTIONS: Record<string, string> = {
  FFmpeg: 'Audio/video processing and remuxing. Required for most operations.',
  mp4decrypt: 'Decrypting DRM-protected streams (Bento4 toolkit).',
  'N_m3u8DL-RE':
    'HLS/DASH stream downloader. Used when download mode is set to N_m3u8DL-RE.',
  MP4Box: 'MP4 muxing and remuxing (GPAC toolkit). Used when remux mode is set to MP4Box.',
  AMDecrypt:
    'Optional Apple Music DRM decryption tool. Used with the wrapper authentication system. See Help > Wrapper / AMdecrypt.',
};

/**
 * ToolsTab -- Renders the Tools settings tab.
 *
 * Merges tool installation management (from DependenciesStep) with custom
 * path overrides (from the former PathsTab) into a single unified view.
 */
export function ToolsTab() {
  // --- Dependency store ---
  const python = useDependencyStore((s) => s.python);
  const gamdl = useDependencyStore((s) => s.gamdl);
  const tools = useDependencyStore((s) => s.tools);
  const isChecking = useDependencyStore((s) => s.isChecking);
  const isInstalling = useDependencyStore((s) => s.isInstalling);
  const installingName = useDependencyStore((s) => s.installingName);
  const checkAll = useDependencyStore((s) => s.checkAll);
  const installPython = useDependencyStore((s) => s.installPython);
  const installGamdl = useDependencyStore((s) => s.installGamdl);
  const installTool = useDependencyStore((s) => s.installTool);
  const depError = useDependencyStore((s) => s.error);

  // --- Settings store ---
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  // --- Local state: which tools have their path override expanded ---
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());

  /** Toggle the custom path override section for a tool */
  const togglePathExpanded = (toolName: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(toolName)) {
        next.delete(toolName);
      } else {
        next.add(toolName);
      }
      return next;
    });
  };

  /** Check all dependency statuses on mount */
  useEffect(() => {
    checkAll();
  }, [checkAll]);

  /** Install all missing required tools sequentially.
   *  Optional tools (like AMDecrypt) are excluded because they
   *  have no auto-install source — users configure them via Set Path. */
  const handleInstallAll = async () => {
    const missing = tools.filter((t) => !t.installed && t.required);
    for (const tool of missing) {
      try {
        await installTool(tool.name);
      } catch {
        /* Continue with next tool */
      }
    }
  };

  const missingCount = tools.filter((t) => !t.installed && t.required).length;

  /**
   * Gets the current custom path value for a tool from settings.
   * Returns the settings value or null if not configured.
   */
  const getToolPath = (toolName: string): string | null => {
    const key = TOOL_PATH_KEYS[toolName];
    if (!key) return null;
    const value = settings[key as keyof typeof settings];
    if (typeof value === 'string' && value.length > 0) return value;
    return null;
  };

  /**
   * Updates the custom path for a tool in settings.
   */
  const setToolPath = (toolName: string, path: string | null) => {
    const key = TOOL_PATH_KEYS[toolName];
    if (!key) return;
    updateSettings({ [key]: path });
  };

  return (
    <div className="space-y-6 max-w-xl">
      {/* ============================================================ */}
      {/* Section: Core Dependencies                                    */}
      {/* ============================================================ */}
      <div>
        <h3 className="text-sm font-semibold text-content-primary mb-1">
          Core Dependencies
        </h3>
        <p className="text-xs text-content-secondary mb-4">
          Python and GAMDL are required for the app to function. They are
          automatically installed during first-time setup.
        </p>

        {isChecking && !python && !gamdl ? (
          <LoadingSpinner label="Checking dependencies..." />
        ) : (
          <div className="space-y-2">
            {/* Python status */}
            {python && (
              <div className="flex items-center gap-3 p-3 rounded-platform border border-border-light bg-surface-elevated">
                {python.installed ? (
                  <CheckCircle
                    size={18}
                    className="text-status-success flex-shrink-0"
                  />
                ) : (
                  <XCircle
                    size={18}
                    className="text-status-error flex-shrink-0"
                  />
                )}
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-content-primary">
                    Python
                  </span>
                  {python.version && (
                    <p className="text-xs text-content-secondary">
                      v{python.version}
                    </p>
                  )}
                </div>
                {!python.installed && (
                  <Button
                    variant="secondary"
                    size="sm"
                    icon={<Download size={14} />}
                    loading={isInstalling && installingName === 'Python'}
                    disabled={isInstalling}
                    onClick={() => installPython()}
                  >
                    Install
                  </Button>
                )}
              </div>
            )}

            {/* GAMDL status */}
            {gamdl && (
              <div className="flex items-center gap-3 p-3 rounded-platform border border-border-light bg-surface-elevated">
                {gamdl.installed ? (
                  <CheckCircle
                    size={18}
                    className="text-status-success flex-shrink-0"
                  />
                ) : (
                  <XCircle
                    size={18}
                    className="text-status-error flex-shrink-0"
                  />
                )}
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-content-primary">
                    GAMDL
                  </span>
                  {gamdl.version && (
                    <p className="text-xs text-content-secondary">
                      v{gamdl.version}
                    </p>
                  )}
                </div>
                {!gamdl.installed && (
                  <Button
                    variant="secondary"
                    size="sm"
                    icon={<Download size={14} />}
                    loading={isInstalling && installingName === 'GAMDL'}
                    disabled={isInstalling}
                    onClick={() => installGamdl()}
                  >
                    Install
                  </Button>
                )}
              </div>
            )}
          </div>
        )}
      </div>

      {/* ============================================================ */}
      {/* Section: External Tools                                       */}
      {/* ============================================================ */}
      <div>
        <h3 className="text-sm font-semibold text-content-primary mb-1">
          External Tools
        </h3>
        <p className="text-xs text-content-secondary mb-4">
          Required tools must be installed for downloads to work. Optional
          tools provide additional features. Click a tool to configure a
          custom binary path.
        </p>

        {/* Action buttons */}
        <div className="flex gap-2 mb-3">
          <Button
            variant="secondary"
            size="sm"
            icon={<RefreshCw size={14} />}
            loading={isChecking}
            onClick={checkAll}
          >
            Check All
          </Button>
          {missingCount > 0 && (
            <Button
              variant="primary"
              size="sm"
              icon={<Download size={14} />}
              loading={isInstalling}
              onClick={handleInstallAll}
            >
              {isInstalling
                ? `Installing ${installingName}...`
                : `Install All (${missingCount} missing)`}
            </Button>
          )}
        </div>

        {/* Tool list */}
        {isChecking && tools.length === 0 ? (
          <LoadingSpinner label="Checking tools..." />
        ) : (
          <div className="space-y-2">
            {tools.map((tool) => {
              const pathKey = TOOL_PATH_KEYS[tool.name];
              const isExpanded = expandedPaths.has(tool.name);
              const customPath = getToolPath(tool.name);
              const description = TOOL_PATH_DESCRIPTIONS[tool.name] || '';
              const placeholder =
                tool.name === 'AMDecrypt'
                  ? 'Not configured'
                  : 'Using managed version';

              return (
                <div
                  key={tool.name}
                  className="rounded-platform border border-border-light bg-surface-elevated overflow-hidden"
                >
                  {/* Tool row */}
                  <div className="flex items-center gap-3 p-3">
                    {/* Status icon */}
                    {tool.installed ? (
                      <CheckCircle
                        size={18}
                        className="text-status-success flex-shrink-0"
                      />
                    ) : tool.required ? (
                      <XCircle
                        size={18}
                        className="text-status-error flex-shrink-0"
                      />
                    ) : (
                      <AlertCircle
                        size={18}
                        className="text-content-tertiary flex-shrink-0"
                      />
                    )}

                    {/* Tool info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-content-primary">
                          {tool.name}
                        </span>
                        {tool.required ? (
                          <span className="text-[10px] font-medium px-1.5 py-0.5 rounded-full bg-status-error/10 text-status-error">
                            Required
                          </span>
                        ) : (
                          <span className="text-[10px] font-medium px-1.5 py-0.5 rounded-full bg-surface-secondary text-content-tertiary">
                            Optional
                          </span>
                        )}
                        {tool.source === 'system' && (
                          <span className="text-[10px] font-medium px-1.5 py-0.5 rounded-full bg-accent-primary/10 text-accent-primary">
                            System
                          </span>
                        )}
                        {customPath && (
                          <span className="text-[10px] font-medium px-1.5 py-0.5 rounded-full bg-amber-500/10 text-amber-600 dark:text-amber-400">
                            Custom
                          </span>
                        )}
                      </div>
                      {tool.version && (
                        <p className="text-xs text-content-secondary">
                          v{tool.version}
                        </p>
                      )}
                    </div>

                    {/* Install button (missing required tools) or Set Path
                        (missing optional tools that have no auto-install) */}
                    {!tool.installed && tool.required && (
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
                    {!tool.installed && !tool.required && pathKey && (
                      <Button
                        variant="secondary"
                        size="sm"
                        icon={<ChevronRight size={14} />}
                        onClick={() => {
                          if (!expandedPaths.has(tool.name)) {
                            togglePathExpanded(tool.name);
                          }
                        }}
                      >
                        Set Path
                      </Button>
                    )}

                    {/* Expand/collapse for custom path */}
                    {pathKey && (
                      <button
                        type="button"
                        className="p-1 rounded text-content-tertiary hover:text-content-secondary transition-colors"
                        onClick={() => togglePathExpanded(tool.name)}
                        title="Configure custom binary path"
                      >
                        {isExpanded ? (
                          <ChevronDown size={16} />
                        ) : (
                          <ChevronRight size={16} />
                        )}
                      </button>
                    )}
                  </div>

                  {/* Custom path override (expandable) */}
                  {isExpanded && pathKey && (
                    <div className="px-3 pb-3 pt-0 border-t border-border-light">
                      <p className="text-xs text-content-secondary mt-2 mb-2">
                        {description}
                      </p>
                      <FilePickerButton
                        label="Custom Path"
                        description="Override the managed version with a custom binary. Leave empty to use the auto-installed version."
                        value={customPath}
                        onChange={(path) => setToolPath(tool.name, path)}
                        placeholder={placeholder}
                      />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* ============================================================ */}
      {/* Section: Directories                                          */}
      {/* ============================================================ */}
      <div>
        <h3 className="text-sm font-semibold text-content-primary mb-4">
          Directories
        </h3>
        <FilePickerButton
          label="Temp Directory"
          description="Directory for intermediate files during download and processing. Leave empty to use a MeedyaDL subdirectory within the OS default temp directory."
          value={settings.temp_path || null}
          onChange={(path) => updateSettings({ temp_path: path || '' })}
          directory
          placeholder="Default: {OS temp}/MeedyaDL"
        />
      </div>

      {/* ============================================================ */}
      {/* Error display                                                 */}
      {/* ============================================================ */}
      {depError && (
        <div className="p-3 rounded-platform border border-status-error bg-status-error-bg text-sm text-status-error">
          {depError}
        </div>
      )}
    </div>
  );
}
