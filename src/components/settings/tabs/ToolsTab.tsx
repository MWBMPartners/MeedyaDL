/**
 * Copyright (c) 2026 MeedyaSuite
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
 *   MP4Box) with:
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

import { useCallback, useEffect } from 'react';

import {
  CheckCircle,
  XCircle,
  Download,
  AlertCircle,
  RefreshCw,
  ChevronDown,
  ChevronRight,
} from 'lucide-react';

import {
  installGamdlVersion,
  getGamdlSupportWindow,
  type GamdlSupportWindow,
  createBackup,
  listBackups,
  restoreFromBackup,
  deleteBackup,
  type BackupEntry,
} from '@/lib/tauri-commands';
import { useDependencyStore } from '@/stores/dependencyStore';
import { useUiStore } from '@/stores/uiStore';
// `useSettingsStore` retained — the per-tool path map (TOOL_PATH_KEYS)
// uses RUNTIME-keyed lookups (`settings[key as keyof ...]`), which
// can't be expressed via `useSettingsField`'s compile-time-keyed
// signature. `useSettingsField` is used below for the one statically-
// keyed field (`temp_path`).
import { useSettingsStore } from '@/stores/settingsStore';
import { useSettingsField } from '@/hooks/useSettingsField';

import { Button, LoadingSpinner, FilePickerButton, SettingsSection } from '@/components/common';

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
  MediaInfo: 'mediainfo_path',
};

/**
 * User-friendly descriptions for each tool's custom path override.
 */
const TOOL_PATH_DESCRIPTIONS: Record<string, string> = {
  FFmpeg: 'Audio/video processing and remuxing. Required for most operations.',
  mp4decrypt: 'Decrypting DRM-protected streams (Bento4 toolkit).',
  'N_m3u8DL-RE': 'HLS/DASH stream downloader. Used when download mode is set to N_m3u8DL-RE.',
  MP4Box: 'MP4 muxing and remuxing (GPAC toolkit). Used when remux mode is set to MP4Box.',
  MediaInfo: 'Accurate codec detection for the enrichment pipeline. Identifies Dolby Atmos, AC-3, ALAC, and AAC variants.',
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
  // Full settings snapshot retained for the dynamic-keyed tool path
  // map (getToolPath/setToolPath below).
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  // Statically-keyed Directories field.
  const tempPath = useSettingsField('temp_path');

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

  /** Install all missing required tools sequentially. */
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
    <div className="space-y-3">
      {/* ============================================================ */}
      {/* Section: Core Dependencies                                    */}
      {/* ============================================================ */}
      <SettingsSection
        title="Core Dependencies"
        description="Python and GAMDL are required for the app to function. They are automatically installed during first-time setup."
      >

        {isChecking && !python && !gamdl ? (
          <LoadingSpinner label="Checking dependencies..." />
        ) : (
          <div className="space-y-2">
            {/* Python status */}
            {python && (
              <div className="flex items-center gap-3 p-3 rounded-platform border border-border-light bg-surface-elevated">
                {python.installed ? (
                  <CheckCircle size={18} className="text-status-success flex-shrink-0" />
                ) : (
                  <XCircle size={18} className="text-status-error flex-shrink-0" />
                )}
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-content-primary">Python</span>
                  {python.version && (
                    <p className="text-xs text-content-secondary">v{python.version}</p>
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
                  <CheckCircle size={18} className="text-status-success flex-shrink-0" />
                ) : (
                  <XCircle size={18} className="text-status-error flex-shrink-0" />
                )}
                <div className="flex-1 min-w-0">
                  <span className="text-sm font-medium text-content-primary">GAMDL</span>
                  {gamdl.version && (
                    <p className="text-xs text-content-secondary">v{gamdl.version}</p>
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
      </SettingsSection>

      {/* ============================================================ */}
      {/* Section: GAMDL Version Management (#522)                      */}
      {/* ============================================================ */}
      {gamdl?.installed && <GamdlVersionManagement installedVersion={gamdl.version ?? null} />}

      {/* ============================================================ */}
      {/* Section: External Tools                                       */}
      {/* ============================================================ */}
      <SettingsSection
        title="External Tools"
        description="Required tools must be installed for downloads to work. Optional tools provide additional features. Click a tool to configure a custom binary path."
      >

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
              const placeholder = 'Using managed version';

              return (
                <div
                  key={tool.name}
                  className="rounded-platform border border-border-light bg-surface-elevated overflow-hidden"
                >
                  {/* Tool row */}
                  <div className="flex items-center gap-3 p-3">
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
                        <p className="text-xs text-content-secondary">v{tool.version}</p>
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
                        {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                      </button>
                    )}
                  </div>

                  {/* Custom path override (expandable) */}
                  {isExpanded && pathKey && (
                    <div className="px-3 pb-3 pt-0 border-t border-border-light">
                      <p className="text-xs text-content-secondary mt-2 mb-2">{description}</p>
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
      </SettingsSection>

      {/* ============================================================ */}
      {/* Section: Backups (#466)                                       */}
      {/* ============================================================ */}
      <BackupManagement />

      {/* ============================================================ */}
      {/* Section: Directories                                          */}
      {/* ============================================================ */}
      <SettingsSection title="Directories">
        <FilePickerButton
          label="Temp Directory"
          description="Directory for intermediate files during download and processing. Leave empty to use a MeedyaDL subdirectory within the OS default temp directory."
          value={tempPath.value || null}
          onChange={(path) => tempPath.set(path || '')}
          directory
          placeholder="Default: {OS temp}/MeedyaDL"
        />
      </SettingsSection>

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

/**
 * GAMDL version management section (#522). Renders below Core
 * Dependencies when GAMDL is installed. Lets the user:
 *
 *   - See the support classification for the installed version
 *     (Supported / Untested / Unsupported) compared against the
 *     MeedyaDL-tested window from `tool-versions.toml`.
 *   - Click "Install recommended (vX.Y.Z)" to force-reinstall the
 *     recommended version (works for both upgrades AND downgrades —
 *     uses `pip install --force-reinstall` under the hood, unlike the
 *     setup-wizard install button which uses `--upgrade` and can't
 *     downgrade).
 *   - Type a specific version (e.g., "2.9.3") and force-install it.
 *     Bypasses the support window — for power-user rollback /
 *     manual-validation scenarios.
 *
 * The backend (`install_gamdl_version`) classifies the requested
 * version and emits an activity-log advisory when it falls outside
 * the supported range, but does NOT refuse — the user has opted in.
 */
function GamdlVersionManagement({ installedVersion }: { installedVersion: string | null }) {
  const fetchAll = useDependencyStore((s) => s.checkAll);
  const addToast = useUiStore((s) => s.addToast);
  const [supportWindow, setSupportWindow] = useState<GamdlSupportWindow | null>(null);
  const [versionInput, setVersionInput] = useState('');
  const [isInstallingRecommended, setIsInstallingRecommended] = useState(false);
  const [isInstallingSpecific, setIsInstallingSpecific] = useState(false);

  // Fetch the support window once on mount.
  useEffect(() => {
    getGamdlSupportWindow()
      .then(setSupportWindow)
      .catch((e) => console.warn('Failed to load GAMDL support window', e));
  }, []);

  if (!supportWindow) {
    return null;
  }

  const { minimum, maximum_tested, recommended } = supportWindow;

  // Cheap client-side classification mirroring `gamdl_capabilities::classify`
  // — used for the badge only; the authoritative classification runs in
  // Rust on every install.
  const classify = (
    v: string | null,
  ): { label: string; tone: 'success' | 'warning' | 'error' | 'neutral' } => {
    if (!v) return { label: 'Unknown', tone: 'neutral' };
    if (compareVersions(v, minimum) < 0) return { label: 'Unsupported', tone: 'error' };
    if (compareVersions(v, maximum_tested) > 0) return { label: 'Untested', tone: 'warning' };
    return { label: 'Supported', tone: 'success' };
  };

  const status = classify(installedVersion);

  const handleInstallRecommended = async () => {
    setIsInstallingRecommended(true);
    try {
      const installed = await installGamdlVersion(recommended);
      addToast(`GAMDL v${installed} (recommended) installed successfully.`, 'success');
      await fetchAll();
    } catch (e) {
      addToast(
        `GAMDL install failed: ${e instanceof Error ? e.message : String(e)}`,
        'error',
      );
    } finally {
      setIsInstallingRecommended(false);
    }
  };

  const handleInstallSpecific = async () => {
    const trimmed = versionInput.trim();
    if (!trimmed) return;
    setIsInstallingSpecific(true);
    try {
      const installed = await installGamdlVersion(trimmed);
      addToast(`GAMDL v${installed} installed successfully.`, 'success');
      setVersionInput('');
      await fetchAll();
    } catch (e) {
      addToast(
        `GAMDL install failed: ${e instanceof Error ? e.message : String(e)}`,
        'error',
      );
    } finally {
      setIsInstallingSpecific(false);
    }
  };

  const badgeClass =
    status.tone === 'success'
      ? 'bg-status-success-bg text-status-success border-status-success'
      : status.tone === 'warning'
        ? 'bg-status-warning-bg text-status-warning border-status-warning'
        : status.tone === 'error'
          ? 'bg-status-error-bg text-status-error border-status-error'
          : 'bg-surface-elevated text-content-secondary border-border-light';

  const anyBusy = isInstallingRecommended || isInstallingSpecific;

  return (
    <SettingsSection
      title="GAMDL Version Management"
      description="Roll back to a previous GAMDL version or pin to a specific release. Useful when an upstream upgrade introduces a regression or for manual validation."
    >
      <div className="space-y-3">
        {/* Installed version + support badge */}
        <div className="flex items-center gap-3 p-3 rounded-platform border border-border-light bg-surface-elevated">
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-content-primary">
              Installed: v{installedVersion ?? 'unknown'}
            </p>
            <p className="text-xs text-content-secondary mt-0.5">
              Tested range: v{minimum} – v{maximum_tested} &middot; Recommended: v{recommended}
            </p>
          </div>
          <span className={`text-xs px-2 py-0.5 rounded-full border ${badgeClass}`}>
            {status.label}
          </span>
        </div>

        {/* Install recommended button */}
        <div className="flex items-center gap-2">
          <Button
            variant="primary"
            size="sm"
            icon={<Download size={14} />}
            loading={isInstallingRecommended}
            disabled={anyBusy}
            onClick={handleInstallRecommended}
          >
            Install recommended (v{recommended})
          </Button>
          {installedVersion === recommended && (
            <span className="text-xs text-content-secondary">
              Already on the recommended version.
            </span>
          )}
        </div>

        {/* Install specific version input */}
        <div>
          <label htmlFor="gamdl-version-input" className="block text-xs font-medium text-content-secondary mb-1">
            Advanced: install specific version
          </label>
          <div className="flex gap-2">
            <input
              id="gamdl-version-input"
              type="text"
              value={versionInput}
              onChange={(e) => setVersionInput(e.target.value)}
              placeholder="e.g. 2.9.3"
              disabled={anyBusy}
              className="flex-1 px-3 py-1.5 text-sm rounded-platform border border-border-light bg-surface-elevated text-content-primary placeholder:text-content-tertiary focus:outline-none focus:ring-2 focus:ring-accent-primary"
              aria-describedby="gamdl-version-help"
            />
            <Button
              variant="secondary"
              size="sm"
              loading={isInstallingSpecific}
              disabled={anyBusy || !versionInput.trim()}
              onClick={handleInstallSpecific}
            >
              Install
            </Button>
          </div>
          <p id="gamdl-version-help" className="mt-1 text-xs text-content-tertiary">
            Any PyPI-compatible GAMDL version. Versions outside the tested range install with a warning.
          </p>
        </div>
      </div>
    </SettingsSection>
  );
}

/**
 * Tiny semver-ish comparator for the support-window badge classification.
 * Returns negative / zero / positive — like `String.localeCompare` but
 * numerically aware so "v3.10" > "v3.9". Mirrors the Rust
 * `gamdl_capabilities::classify` semantics for the visual badge only;
 * the authoritative classification runs server-side on every install.
 */
function compareVersions(a: string, b: string): number {
  const parts = (s: string): number[] =>
    s
      .replace(/^v/i, '')
      .split(/[.\-+]/)
      .map((p) => parseInt(p, 10))
      .filter((n) => !Number.isNaN(n));
  const pa = parts(a);
  const pb = parts(b);
  const len = Math.max(pa.length, pb.length);
  for (let i = 0; i < len; i++) {
    const da = pa[i] ?? 0;
    const db = pb[i] ?? 0;
    if (da !== db) return da - db;
  }
  return 0;
}

/**
 * Backups section (#466). Auto-backup on app exit runs in the
 * background (see `lib.rs::on_window_event`); this UI surfaces the
 * resulting snapshots and lets the user create / restore / delete
 * snapshots on demand.
 *
 * UX rules:
 *   - "Create snapshot" button always available.
 *   - Snapshot list shows newest first, with timestamp / file count /
 *     size and per-entry Restore / Delete buttons.
 *   - Restore prompts a confirmation modal (destructive — overwrites
 *     live state) AND surfaces a "restart MeedyaDL to apply" toast
 *     after success because the in-memory SettingsCache + active
 *     queue lock would otherwise diverge from disk.
 */
function BackupManagement() {
  const addToast = useUiStore((s) => s.addToast);
  const [snapshots, setSnapshots] = useState<BackupEntry[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [restoreTarget, setRestoreTarget] = useState<BackupEntry | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await listBackups();
      setSnapshots(list);
    } catch (e) {
      addToast(`Failed to list backups: ${e instanceof Error ? e.message : String(e)}`, 'error');
      setSnapshots([]);
    }
  }, [addToast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleCreate = async () => {
    setBusy(true);
    try {
      const summary = await createBackup();
      addToast(
        `Snapshot created with ${summary.files.length} file${summary.files.length === 1 ? '' : 's'}`,
        'success',
      );
      await refresh();
    } catch (e) {
      addToast(`Snapshot failed: ${e instanceof Error ? e.message : String(e)}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  const handleRestoreConfirmed = async () => {
    if (!restoreTarget) return;
    const target = restoreTarget;
    setRestoreTarget(null);
    setBusy(true);
    try {
      const summary = await restoreFromBackup(target.path);
      addToast(
        `Restored ${summary.restored.length} file${summary.restored.length === 1 ? '' : 's'} — please restart MeedyaDL to apply.`,
        'warning',
      );
    } catch (e) {
      addToast(`Restore failed: ${e instanceof Error ? e.message : String(e)}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (entry: BackupEntry) => {
    setBusy(true);
    try {
      await deleteBackup(entry.path);
      addToast('Snapshot deleted.', 'info');
      await refresh();
    } catch (e) {
      addToast(`Delete failed: ${e instanceof Error ? e.message : String(e)}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  return (
    <SettingsSection
      title="Backups"
      description="Snapshots include settings, queue, and history. A snapshot is taken automatically when MeedyaDL exits; only the most recent 10 are kept on disk."
    >
      <div className="space-y-3">
        <div className="flex items-center gap-2">
          <Button
            variant="secondary"
            size="sm"
            icon={<Download size={14} />}
            loading={busy && snapshots !== null}
            disabled={busy}
            onClick={handleCreate}
          >
            Create snapshot now
          </Button>
          <Button variant="ghost" size="sm" icon={<RefreshCw size={14} />} onClick={() => refresh()}>
            Refresh
          </Button>
        </div>

        {snapshots === null ? (
          <LoadingSpinner label="Loading snapshots..." />
        ) : snapshots.length === 0 ? (
          <p className="text-sm text-content-tertiary">
            No snapshots yet. Click "Create snapshot now" or quit MeedyaDL to take one
            automatically.
          </p>
        ) : (
          <div className="space-y-1.5 max-h-72 overflow-y-auto">
            {snapshots.map((s) => (
              <div
                key={s.path}
                className="flex items-center gap-3 p-2 rounded-platform border border-border-light bg-surface-elevated"
              >
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-content-primary">
                    {formatSnapshotName(s.name)}
                  </p>
                  <p className="text-xs text-content-tertiary mt-0.5">
                    {s.file_count} file{s.file_count === 1 ? '' : 's'} &middot;{' '}
                    {formatBytes(s.size_bytes)}
                  </p>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={busy}
                  onClick={() => setRestoreTarget(s)}
                >
                  Restore
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  icon={<XCircle size={14} />}
                  disabled={busy}
                  onClick={() => handleDelete(s)}
                  aria-label={`Delete snapshot ${s.name}`}
                />
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Restore confirmation modal */}
      {restoreTarget && (
        <div
          className="fixed inset-0 z-50 bg-black/40 flex items-center justify-center"
          onClick={() => setRestoreTarget(null)}
          role="dialog"
          aria-modal="true"
        >
          <div
            className="bg-surface-primary border border-border-light rounded-platform p-5 max-w-md mx-4"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-base font-semibold text-content-primary mb-2">
              Restore from snapshot?
            </h3>
            <p className="text-sm text-content-secondary mb-4">
              This will overwrite your current settings, queue, and history with the contents of{' '}
              <span className="font-mono">{formatSnapshotName(restoreTarget.name)}</span>. Your
              current state will be lost (unless you create a snapshot first). You'll need to
              restart MeedyaDL for the restore to take effect.
            </p>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={() => setRestoreTarget(null)}>
                Cancel
              </Button>
              <Button variant="primary" onClick={handleRestoreConfirmed}>
                Restore
              </Button>
            </div>
          </div>
        </div>
      )}
    </SettingsSection>
  );
}

/** Pretty-print a snapshot directory name (YYYYMMDD-HHMMSS) as a
 *  human-friendly local timestamp. Falls back to the raw name on
 *  any parse failure. */
function formatSnapshotName(name: string): string {
  // Expect "YYYYMMDD-HHMMSS"
  const m = /^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$/.exec(name);
  if (!m) return name;
  const [, y, mo, d, h, mi, s] = m;
  // Construct as UTC (matches backup_service::snapshot_dir_name).
  const dt = new Date(Date.UTC(+y, +mo - 1, +d, +h, +mi, +s));
  if (Number.isNaN(dt.getTime())) return name;
  return dt.toLocaleString();
}

/** Format byte count as KB / MB. */
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
