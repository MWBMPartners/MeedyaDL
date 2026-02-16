// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file Updates page component.
 *
 * Displays a detailed view of available component updates with full release
 * notes rendered as markdown. When no updates are available, shows a
 * "You're up to date" state with the current app version and a manual
 * check button.
 *
 * Release notes are stripped of the "Choose your download" section and
 * everything below it, since the in-app updater handles downloads
 * automatically.
 */

import { useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import {
  RefreshCw,
  ExternalLink,
  Download,
  RotateCcw,
  CheckCircle,
  ArrowUpCircle,
} from 'lucide-react';
import { relaunch } from '@tauri-apps/plugin-process';

import { useUpdateStore } from '@/stores/updateStore';
import { useUiStore } from '@/stores/uiStore';
import { PageHeader } from '@/components/layout';
import { Button } from '@/components/common';

/**
 * Strips the "Choose your download" section and everything after it from
 * release notes markdown. This section is irrelevant for in-app updates
 * since the updater handles downloads automatically.
 */
function stripDownloadSection(body: string): string {
  // Match "## Choose your download" with optional emoji prefix
  const pattern = /^##\s+(?:🖥️\s*)?choose\s+your\s+download/im;
  const match = body.search(pattern);
  if (match === -1) return body;
  return body.slice(0, match).trimEnd();
}

export function UpdatesPage() {
  const lastResult = useUpdateStore((s) => s.lastResult);
  const dismissed = useUpdateStore((s) => s.dismissed);
  const dismissUpdate = useUpdateStore((s) => s.dismissUpdate);
  const upgradeGamdl = useUpdateStore((s) => s.upgradeGamdl);
  const isUpgrading = useUpdateStore((s) => s.isUpgrading);
  const isChecking = useUpdateStore((s) => s.isChecking);
  const checkForUpdates = useUpdateStore((s) => s.checkForUpdates);
  const isDownloadingUpdate = useUpdateStore((s) => s.isDownloadingUpdate);
  const downloadProgress = useUpdateStore((s) => s.downloadProgress);
  const updateInstalled = useUpdateStore((s) => s.updateInstalled);
  const downloadAndInstallAppUpdate = useUpdateStore(
    (s) => s.downloadAndInstallAppUpdate,
  );
  const addToast = useUiStore((s) => s.addToast);

  const activeUpdates = useMemo(() => {
    if (!lastResult) return [];
    return lastResult.components.filter(
      (c) =>
        c.update_available &&
        c.is_compatible &&
        !dismissed.includes(c.name),
    );
  }, [lastResult, dismissed]);

  // Current app version from the last check result
  const currentVersion = useMemo(() => {
    if (!lastResult) return null;
    const app = lastResult.components.find((c) => c.name === 'MeedyaDL');
    return app?.current_version ?? null;
  }, [lastResult]);

  const handleUpgradeGamdl = async () => {
    try {
      const version = await upgradeGamdl();
      addToast(`GAMDL upgraded to v${version}`, 'success');
    } catch {
      addToast('Failed to upgrade GAMDL', 'error');
    }
  };

  const handleDownloadAndInstall = async (tag: string) => {
    try {
      await downloadAndInstallAppUpdate(tag);
      addToast('Update installed! Restart to apply.', 'success');
    } catch {
      addToast('Failed to download and install update', 'error');
    }
  };

  const handleRestart = async () => {
    try {
      await relaunch();
    } catch {
      addToast('Failed to restart. Please restart manually.', 'error');
    }
  };

  const handleCheck = () => {
    checkForUpdates().catch(() => {});
  };

  const handleViewRelease = (url: string) => {
    window.open(url, '_blank');
  };

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Updates"
        actions={
          <Button
            variant="secondary"
            size="sm"
            icon={<RefreshCw size={14} />}
            loading={isChecking}
            onClick={handleCheck}
          >
            {isChecking ? 'Checking...' : 'Check for Updates'}
          </Button>
        }
      />

      <div className="flex-1 overflow-y-auto p-6">
        {activeUpdates.length === 0 ? (
          /* No updates available state */
          <div className="flex flex-col items-center justify-center py-16 text-center">
            <CheckCircle size={48} className="text-status-success mb-4" />
            <h3 className="text-lg font-semibold text-content-primary mb-1">
              You&apos;re up to date!
            </h3>
            {currentVersion && (
              <p className="text-sm text-content-secondary">
                Current version: v{currentVersion}
              </p>
            )}
            {lastResult && (
              <p className="text-xs text-content-tertiary mt-2">
                Last checked: {new Date(lastResult.checked_at).toLocaleString()}
              </p>
            )}
          </div>
        ) : (
          /* Updates available */
          <div className="space-y-6 max-w-3xl">
            <div className="flex items-center gap-2 text-sm font-medium text-content-primary">
              <ArrowUpCircle size={18} className="text-accent" />
              {activeUpdates.length === 1
                ? 'Update Available'
                : `${activeUpdates.length} Updates Available`}
            </div>

            {activeUpdates.map((update) => (
              <div
                key={update.name}
                className="rounded-platform border border-border-light bg-surface-secondary p-5"
              >
                {/* Header row: component name, versions, actions */}
                <div className="flex items-center justify-between gap-4 mb-3">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold text-content-primary">
                      {update.name}
                    </span>
                    {update.current_version && (
                      <span className="text-sm text-content-secondary">
                        v{update.current_version}
                      </span>
                    )}
                    {update.latest_version && (
                      <span className="text-sm text-accent font-medium">
                        &rarr; v{update.latest_version}
                      </span>
                    )}
                    {update.is_prerelease && (
                      <span className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold bg-status-warning-bg text-status-warning">
                        Pre-Release
                      </span>
                    )}
                  </div>

                  <div className="flex items-center gap-2">
                    {/* GAMDL upgrade */}
                    {update.name === 'GAMDL' && (
                      <Button
                        variant="primary"
                        size="sm"
                        icon={<RefreshCw size={12} />}
                        loading={isUpgrading}
                        onClick={handleUpgradeGamdl}
                      >
                        Upgrade
                      </Button>
                    )}

                    {/* MeedyaDL app update */}
                    {update.name === 'MeedyaDL' && (
                      <>
                        {updateInstalled ? (
                          <Button
                            variant="primary"
                            size="sm"
                            icon={<RotateCcw size={12} />}
                            onClick={handleRestart}
                          >
                            Restart Now
                          </Button>
                        ) : isDownloadingUpdate ? (
                          <div className="flex items-center gap-2 min-w-[120px]">
                            <div className="flex-1 h-1.5 bg-surface-secondary rounded-full overflow-hidden">
                              <div
                                className="h-full bg-accent rounded-full transition-all duration-300"
                                style={{
                                  width: `${downloadProgress ?? 0}%`,
                                }}
                              />
                            </div>
                            <span className="text-[10px] text-content-tertiary tabular-nums">
                              {downloadProgress != null
                                ? `${downloadProgress}%`
                                : '...'}
                            </span>
                          </div>
                        ) : (
                          update.tag_name && (
                            <Button
                              variant="primary"
                              size="sm"
                              icon={<Download size={12} />}
                              onClick={() =>
                                handleDownloadAndInstall(update.tag_name!)
                              }
                            >
                              Download &amp; Install
                            </Button>
                          )
                        )}
                      </>
                    )}

                    {/* View release link */}
                    {update.release_url && (
                      <button
                        type="button"
                        onClick={() => handleViewRelease(update.release_url!)}
                        className="p-1.5 rounded hover:bg-surface-secondary transition-colors"
                        title="View release on GitHub"
                      >
                        <ExternalLink
                          size={14}
                          className="text-content-tertiary"
                        />
                      </button>
                    )}

                    {/* Dismiss */}
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => dismissUpdate(update.name)}
                    >
                      Dismiss
                    </Button>
                  </div>
                </div>

                {/* Pre-release warning */}
                {update.is_prerelease && (
                  <p className="text-[11px] text-status-warning mb-3">
                    This is a pre-release version and may contain bugs or
                    incomplete features. Not recommended for production use.
                  </p>
                )}

                {/* Full release notes (markdown) for MeedyaDL */}
                {update.name === 'MeedyaDL' && update.release_body && (
                  <div className="mt-4 pt-4 border-t border-border-light">
                    <h4 className="text-xs font-semibold text-content-secondary uppercase tracking-wider mb-3">
                      Release Notes
                    </h4>
                    <div className="prose prose-sm max-w-none text-content-primary">
                      <ReactMarkdown>
                        {stripDownloadSection(update.release_body)}
                      </ReactMarkdown>
                    </div>
                  </div>
                )}

                {/* Description for non-MeedyaDL components */}
                {update.name !== 'MeedyaDL' && update.description && (
                  <p className="text-sm text-content-secondary">
                    {update.description}
                  </p>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
