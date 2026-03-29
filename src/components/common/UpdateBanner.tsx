// Copyright (c) 2026 MeedyaDL

/**
 * @file Update notification banner component.
 *
 * Displays a dismissible, accent-themed banner at the top of the main content
 * area when one or more component updates are available (GAMDL CLI, the
 * desktop app itself, or the Python runtime).
 *
 * **Banner behaviour:**
 * - Subscribes to the update store (`useUpdateStore`) for the list of
 *   available, non-dismissed, compatible updates.
 * - Shows the count of updates and, for each, the component name, current
 *   version, and latest version.
 * - **GAMDL** receives a direct "Upgrade" button that triggers an in-app
 *   upgrade (via `pip install --upgrade gamdl`) managed by the Rust backend.
 * - Other components display an external link to their GitHub release page.
 * - Each update can be individually dismissed (hidden until the next check).
 * - When all updates are dismissed or there are no updates, the banner is
 *   not rendered (returns null).
 *
 * **Usage across the application:**
 * Rendered once in App.tsx, immediately above the main content area.
 *
 * @see https://lucide.dev/icons/arrow-up-circle -- ArrowUpCircle icon (banner icon)
 * @see https://lucide.dev/icons/x              -- X icon (dismiss button)
 * @see https://lucide.dev/icons/external-link   -- ExternalLink icon (release link)
 * @see https://lucide.dev/icons/refresh-cw      -- RefreshCw icon (upgrade button)
 */

/**
 * Lucide icon imports for the banner UI.
 * @see https://lucide.dev/guide/packages/lucide-react
 */
import { ArrowUpCircle, X, ExternalLink, RefreshCw, Download, RotateCcw } from 'lucide-react';

/**
 * React `useMemo` hook for memoizing the derived active-updates list.
 * Without memoization, calling `getActiveUpdates()` inside a Zustand selector
 * creates a new array reference on every store change (via `.filter()`), which
 * causes Zustand's `Object.is()` equality check to fail and trigger unnecessary
 * re-renders of this component. By subscribing to the raw data (`lastResult`,
 * `dismissed`) and computing the filtered list with `useMemo`, re-renders only
 * happen when the underlying data actually changes.
 * @see https://react.dev/reference/react/useMemo
 */
import { useMemo } from 'react';

/**
 * Tauri process plugin for relaunching the app after an update is installed.
 * @see https://v2.tauri.app/plugin/process/
 */
import { relaunch } from '@tauri-apps/plugin-process';

/** Zustand store for update state (available updates, dismiss, upgrade actions) */
import { useUpdateStore } from '@/stores/updateStore';

/** Zustand store for general UI state (toast notifications) */
import { useUiStore } from '@/stores/uiStore';

/** Common Button component -- used for the "Upgrade" and "Download & Install" CTAs */
import { Button } from './Button';

/**
 * Renders an accent-themed banner showing available updates with
 * upgrade / dismiss / view-release actions.
 *
 * The component subscribes to four Zustand selectors from the update store
 * and one from the UI store (for toast notifications). All selectors use
 * the `(s) => s.property` shorthand for minimal re-renders.
 *
 * **Conditional rendering:**
 * Returns null when `activeUpdates` is empty, so the banner takes up zero
 * space in the layout when there is nothing to show.
 */
export function UpdateBanner() {
  /*
   * Zustand store subscriptions.
   * Each selector subscribes to a single slice of state to minimise
   * unnecessary re-renders when unrelated state changes.
   *
   * IMPORTANT: We subscribe to `lastResult` and `dismissed` separately
   * instead of calling `getActiveUpdates()` inside a selector. The old
   * pattern `(s) => s.getActiveUpdates()` called `.filter()` inside the
   * selector, creating a new array reference on every store change. Zustand's
   * `Object.is()` comparison always saw a new reference and triggered a
   * re-render, even when the underlying data hadn't changed. This caused
   * excessive re-renders that could cascade into React error #185 (maximum
   * update depth exceeded) during the initialization sequence.
   *
   * By subscribing to the raw data and deriving the filtered list via
   * `useMemo`, we only recompute when `lastResult` or `dismissed` actually
   * change — not on every unrelated store mutation.
   */
  const lastResult = useUpdateStore((s) => s.lastResult);
  const dismissed = useUpdateStore((s) => s.dismissed);
  const dismissUpdate = useUpdateStore((s) => s.dismissUpdate);
  const upgradeGamdl = useUpdateStore((s) => s.upgradeGamdl);
  const isUpgrading = useUpdateStore((s) => s.isUpgrading);
  const isDownloadingUpdate = useUpdateStore((s) => s.isDownloadingUpdate);
  const downloadProgress = useUpdateStore((s) => s.downloadProgress);
  const updateInstalled = useUpdateStore((s) => s.updateInstalled);
  const downloadAndInstallAppUpdate = useUpdateStore((s) => s.downloadAndInstallAppUpdate);
  const addToast = useUiStore((s) => s.addToast);
  const setPage = useUiStore((s) => s.setPage);

  /**
   * Derive the filtered active-updates list from raw store data.
   * Only recomputes when `lastResult` or `dismissed` changes.
   * Filters to only show updates that are available, compatible, and
   * not dismissed by the user.
   *
   * Note: When `updateInstalled` is true, we also include the MeedyaDL
   * component even if it would normally be filtered out, so the "Restart
   * Now" button remains visible.
   */
  // Core components shown with full detail; engine updates shown generically
  const CORE_COMPONENTS = ['MeedyaDL', 'GAMDL', 'Python Runtime'];

  const allUpdates = useMemo(() => {
    if (!lastResult) return [];
    return lastResult.components.filter(
      (c) => c.update_available && c.is_compatible && !dismissed.includes(c.name)
    );
  }, [lastResult, dismissed]);

  // Only show core updates individually in the banner
  const activeUpdates = useMemo(
    () => allUpdates.filter((c) => CORE_COMPONENTS.includes(c.name)),
    [allUpdates]
  );
  const hasEngineUpdates = useMemo(
    () => allUpdates.some((c) => !CORE_COMPONENTS.includes(c.name)),
    [allUpdates]
  );

  /* Early return -- render nothing when there are no active updates */
  if (allUpdates.length === 0) return null;

  /**
   * Handles the GAMDL in-app upgrade flow.
   * Calls the store's `upgradeGamdl` action (which invokes the Rust backend)
   * and shows a success or error toast based on the outcome.
   */
  const handleUpgradeGamdl = async () => {
    try {
      const version = await upgradeGamdl();
      addToast(`GAMDL upgraded to v${version}`, 'success');
    } catch {
      addToast('Failed to upgrade GAMDL', 'error');
    }
  };

  /**
   * Handles the MeedyaDL app download & install flow.
   * Calls the store's `downloadAndInstallAppUpdate` action which uses the
   * Tauri updater plugin to download, verify, and install the update.
   *
   * @param tag - Git tag of the release (e.g., "v0.3.7")
   */
  const handleDownloadAndInstall = async (tag: string) => {
    try {
      await downloadAndInstallAppUpdate(tag);
      addToast('Update installed! Restart to apply.', 'success');
    } catch {
      addToast('Failed to download and install update', 'error');
    }
  };

  /**
   * Handles restarting the application after an update is installed.
   * Uses the Tauri process plugin's `relaunch()` to restart the app.
   */
  const handleRestart = async () => {
    try {
      await relaunch();
    } catch {
      addToast('Failed to restart. Please restart manually.', 'error');
    }
  };

  /**
   * Opens a release URL in the user's default system browser.
   * Uses the Tauri shell plugin's `open()` to reliably delegate to the
   * OS default browser rather than navigating inside the WebView.
   *
   * @param url - Full URL to the GitHub release page
   */
  const handleViewRelease = (url: string) => {
    import('@tauri-apps/plugin-shell').then(({ open }) => open(url)).catch(() => {});
  };

  return (
    /*
     * Banner container.
     * - mx-4 mt-3 mb-1: outer margins to sit nicely below the title bar.
     * - border-accent/30: accent-coloured border at 30% opacity for a
     *   subtle highlight without being visually heavy.
     * - bg-accent-light: light tinted background matching the accent colour.
     */
    <div className="mx-4 mt-3 mb-1 p-3 rounded-platform border border-accent/30 bg-accent-light">
      <div className="flex items-start gap-3">
        {/*
         * Leading icon -- Lucide ArrowUpCircle indicating "update available".
         * flex-shrink-0 prevents it from compressing in narrow layouts.
         * mt-0.5 aligns it vertically with the first line of text.
         */}
        <ArrowUpCircle size={18} className="text-accent flex-shrink-0 mt-0.5" />

        {/* Update details column */}
        <div className="flex-1 space-y-2">
          {/*
           * Header text -- singular or plural depending on the number
           * of available updates (e.g. "Update Available" vs "3 Updates Available").
           */}
          <div className="flex items-center gap-3">
            <p className="text-sm font-medium text-content-primary">
              {activeUpdates.length === 1
                ? 'Update Available'
                : `${activeUpdates.length} Updates Available`}
            </p>
            <button
              type="button"
              onClick={() => setPage('updates')}
              className="text-xs text-accent hover:underline cursor-pointer"
            >
              View Details
            </button>
          </div>

          {/*
           * Per-component update rows.
           * Each row shows the component name, current version, latest
           * version (with a right-arrow), and action buttons.
           *
           * Component-specific behaviour:
           * - GAMDL: "Upgrade" button (in-app pip upgrade)
           * - MeedyaDL: "Download & Install" / progress bar / "Restart Now"
           * - Others (Python, etc.): "View Release" external link
           *
           * Pre-release updates show an amber badge and disclaimer.
           */}
          {activeUpdates.map((update) => (
            <div key={update.name} className="space-y-1.5">
              <div className="flex items-center justify-between gap-2">
                {/* Component name, version info, and pre-release badge (left side) */}
                <span className="text-xs text-content-secondary">
                  <span className="font-medium">{update.name}</span>
                  {/* Current version -- only shown when known */}
                  {update.current_version && <span> v{update.current_version}</span>}
                  {/* Latest version -- highlighted in accent colour with arrow */}
                  {update.latest_version && (
                    <span className="text-accent"> &rarr; v{update.latest_version}</span>
                  )}
                  {/* Pre-release badge -- amber indicator for beta/RC versions */}
                  {update.is_prerelease && (
                    <span className="ml-1.5 inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold bg-status-warning-bg text-status-warning">
                      Pre-Release
                    </span>
                  )}
                </span>

                {/* Action buttons (right side) */}
                <div className="flex items-center gap-1.5">
                  {/*
                   * GAMDL-specific upgrade button.
                   * Only shown when the update is for the GAMDL CLI tool,
                   * which can be upgraded in-app via pip. Uses the primary
                   * Button variant at sm size with a RefreshCw icon.
                   * The `loading` prop shows a spinner while upgrading.
                   */}
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

                  {/*
                   * MeedyaDL app update actions -- three states:
                   * 1. updateInstalled: "Restart Now" button to relaunch the app
                   * 2. isDownloadingUpdate: progress bar with percentage
                   * 3. Default: "Download & Install" button (requires tag_name)
                   */}
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
                        /* Download progress bar */
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
                            {downloadProgress != null ? `${downloadProgress}%` : '...'}
                          </span>
                        </div>
                      ) : (
                        update.tag_name && (
                          <Button
                            variant="primary"
                            size="sm"
                            icon={<Download size={12} />}
                            onClick={() => handleDownloadAndInstall(update.tag_name!)}
                          >
                            Download &amp; Install
                          </Button>
                        )
                      )}
                    </>
                  )}

                  {/*
                   * External release link -- opens the GitHub release page
                   * in the system browser. Only rendered when the update
                   * object includes a release_url.
                   * The non-null assertion (!) is safe because of the
                   * conditional rendering guard.
                   */}
                  {update.release_url && (
                    <button
                      type="button"
                      onClick={() => handleViewRelease(update.release_url!)}
                      className="p-1 rounded hover:bg-surface-secondary transition-colors"
                      title="View release"
                      aria-label={`View ${update.name} release on GitHub`}
                    >
                      <ExternalLink size={14} className="text-content-tertiary" />
                    </button>
                  )}

                  {/*
                   * Dismiss button -- hides this specific update notification
                   * until the next update check cycle. Calls dismissUpdate
                   * with the component name as the key.
                   */}
                  <button
                    type="button"
                    onClick={() => dismissUpdate(update.name)}
                    className="p-1 rounded hover:bg-surface-secondary transition-colors"
                    title="Dismiss"
                    aria-label={`Dismiss ${update.name} update notification`}
                  >
                    <X size={14} className="text-content-tertiary" />
                  </button>
                </div>
              </div>

              {/*
               * Pre-release disclaimer -- shown below the update row when
               * the release is marked as pre-release on GitHub. Uses an
               * amber/warning colour scheme to draw attention.
               */}
              {update.is_prerelease && (
                <p className="text-[11px] text-status-warning pl-0.5">
                  This is a pre-release version and may contain bugs or incomplete features. Not
                  recommended for production use.
                </p>
              )}
            </div>
          ))}

          {/* Generic message for engine/component updates (no individual names shown) */}
          {hasEngineUpdates && (
            <div className="text-xs text-content-secondary mt-1">
              Component updates are also available.{' '}
              <button
                type="button"
                onClick={() => setPage('updates')}
                className="text-accent hover:underline"
              >
                View Details
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
