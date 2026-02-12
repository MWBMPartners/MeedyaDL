// Copyright (c) 2024-2026 MWBM Partners Ltd

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
import { ArrowUpCircle, X, ExternalLink, RefreshCw } from 'lucide-react';

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

/** Zustand store for update state (available updates, dismiss, upgrade actions) */
import { useUpdateStore } from '@/stores/updateStore';

/** Zustand store for general UI state (toast notifications) */
import { useUiStore } from '@/stores/uiStore';

/** Common Button component -- used for the "Upgrade" CTA */
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
  const addToast = useUiStore((s) => s.addToast);

  /**
   * Derive the filtered active-updates list from raw store data.
   * Only recomputes when `lastResult` or `dismissed` changes.
   * Filters to only show updates that are available, compatible, and
   * not dismissed by the user.
   */
  const activeUpdates = useMemo(() => {
    if (!lastResult) return [];
    return lastResult.components.filter(
      (c) =>
        c.update_available &&
        c.is_compatible &&
        !dismissed.includes(c.name),
    );
  }, [lastResult, dismissed]);

  /* Early return -- render nothing when there are no active updates */
  if (activeUpdates.length === 0) return null;

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
   * Opens a release URL in the user's default browser.
   * Uses `window.open` with `_blank` target so the Tauri webview
   * delegates to the system browser rather than navigating in-app.
   *
   * @param url - Full URL to the GitHub release page
   */
  const handleViewRelease = (url: string) => {
    window.open(url, '_blank');
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
        <ArrowUpCircle
          size={18}
          className="text-accent flex-shrink-0 mt-0.5"
        />

        {/* Update details column */}
        <div className="flex-1 space-y-2">
          {/*
           * Header text -- singular or plural depending on the number
           * of available updates (e.g. "Update Available" vs "3 Updates Available").
           */}
          <p className="text-sm font-medium text-content-primary">
            {activeUpdates.length === 1
              ? 'Update Available'
              : `${activeUpdates.length} Updates Available`}
          </p>

          {/*
           * Per-component update rows.
           * Each row shows the component name, current version, latest
           * version (with a right-arrow), and action buttons.
           */}
          {activeUpdates.map((update) => (
            <div
              key={update.name}
              className="flex items-center justify-between gap-2"
            >
              {/* Component name and version info (left side) */}
              <span className="text-xs text-content-secondary">
                <span className="font-medium">{update.name}</span>
                {/* Current version -- only shown when known */}
                {update.current_version && (
                  <span> v{update.current_version}</span>
                )}
                {/* Latest version -- highlighted in accent colour with arrow */}
                {update.latest_version && (
                  <span className="text-accent">
                    {' '}
                    &rarr; v{update.latest_version}
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
                 * External release link -- opens the GitHub release page
                 * in the system browser. Only rendered when the update
                 * object includes a release_url.
                 * The non-null assertion (!) is safe because of the
                 * conditional rendering guard.
                 */}
                {update.release_url && (
                  <button
                    onClick={() => handleViewRelease(update.release_url!)}
                    className="p-1 rounded hover:bg-surface-secondary transition-colors"
                    title="View release"
                  >
                    <ExternalLink
                      size={14}
                      className="text-content-tertiary"
                    />
                  </button>
                )}

                {/*
                 * Dismiss button -- hides this specific update notification
                 * until the next update check cycle. Calls dismissUpdate
                 * with the component name as the key.
                 */}
                <button
                  onClick={() => dismissUpdate(update.name)}
                  className="p-1 rounded hover:bg-surface-secondary transition-colors"
                  title="Dismiss"
                >
                  <X size={14} className="text-content-tertiary" />
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
