// Copyright (c) 2026 MeedyaDL
/**
 * @file Status bar component.
 *
 * Renders a thin bar at the bottom of the main content area showing
 * current application state: active downloads, queued count, completed
 * count, and the application version string.
 *
 * The status bar is always visible (pinned below the scrollable `<main>`
 * inside {@link MainLayout}) and updates reactively whenever the download
 * queue changes in the Zustand store.
 *
 * State connection:
 *  - {@link useDownloadStore} -- reads `queueItems[]` to derive counts
 *    by filtering on `QueueItemStatus.state`.
 *
 * The left section displays download activity counters:
 *  - "X downloading" (with animated pulse dot) -- items in 'downloading'
 *    or 'processing' state.
 *  - "X queued" -- items waiting to start.
 *  - "X completed" -- successfully finished items.
 *  - "No downloads" -- shown when the queue is completely empty.
 *
 * The right section displays the application version string.
 *
 * @see https://tailwindcss.com/docs/animation#pulse -- animate-pulse used for the activity dot.
 * @see https://react.dev/learn/rendering-lists       -- conditional rendering of count spans.
 */

import { useState, useEffect } from 'react';

// Tauri app API for reading the version from tauri.conf.json at runtime.
import { getVersion } from '@tauri-apps/api/app';

/**
 * Zustand store hook for the download queue.
 * Provides `queueItems` -- an array of `QueueItemStatus` objects whose
 * `.state` field is one of: 'queued' | 'downloading' | 'processing' |
 * 'complete' | 'error' | 'cancelled'.
 * @see useDownloadStore in @/stores/downloadStore.ts
 * @see QueueItemStatus in @/types/index.ts
 */
import { useDownloadStore } from '@/stores/downloadStore';
import { useSettingsStore } from '@/stores/settingsStore';

/** Human-readable labels for after-queue actions (status bar display). */
const AFTER_QUEUE_LABELS: Record<string, string> = {
  do_nothing: '',
  open_output_folder: 'Open folder',
  play_sound: 'Play sound',
  close_meedyadl: 'Close app',
  restart_computer: 'Restart',
  hibernate_computer: 'Hibernate',
  shutdown_computer: 'Shut down',
};

/**
 * Compact indicator shown in the status bar when an after-queue action is set.
 * Shows "(once)" suffix for one-shot actions vs the persistent action.
 */
function AfterQueueIndicator() {
  const afterOnce = useSettingsStore((s) => s.settings.after_queue_once);
  const afterAlways = useSettingsStore((s) => s.settings.after_queue_action);

  // One-shot overrides persistent; show nothing for do_nothing
  const action = afterOnce ?? afterAlways ?? 'do_nothing';
  if (action === 'do_nothing') return null;

  const label = AFTER_QUEUE_LABELS[action] ?? action;
  const suffix = afterOnce ? ' (once)' : '';

  return (
    <span className="text-status-warning" title={`After queue: ${label}${suffix}`}>
      After queue: {label}{suffix}
    </span>
  );
}

/**
 * Renders a status bar at the bottom of the main content area.
 *
 * Derives three counters from the download store's `queueItems` array
 * by filtering on the `state` field of each {@link QueueItemStatus}:
 *  - **activeCount**: items whose state is `'downloading'` or `'processing'`.
 *  - **queuedCount**: items whose state is `'queued'`.
 *  - **completedCount**: items whose state is `'complete'`.
 *
 * These filters run on every render triggered by a `queueItems` change.
 * Because the queue is typically small (< 100 items), the O(n) filter
 * cost is negligible and memoisation is not needed.
 *
 * @returns A thin horizontal bar with activity summary (left) and version (right).
 */
export function StatusBar() {
  /**
   * Application version string, fetched once on mount from `tauri.conf.json`
   * via the Tauri app API. Falls back to 'unknown' if the call fails
   * (e.g., in a test environment without Tauri runtime).
   */
  const [appVersion, setAppVersion] = useState('...');

  useEffect(() => {
    getVersion()
      .then((v) => setAppVersion(v))
      .catch(() => setAppVersion('unknown'));
  }, []);

  /**
   * Subscribe to the `queueItems` slice of the download store.
   * This component re-renders whenever the array reference changes
   * (e.g., items are added, removed, or their state is updated).
   */
  const queueItems = useDownloadStore((s) => s.queueItems);

  /*
   * Derive display counters by filtering the queue array.
   * Each `.filter()` call iterates the full array, but with typical
   * queue sizes this is efficient enough without memoisation.
   */

  /** Number of items currently downloading or being post-processed. */
  const activeCount = queueItems.filter(
    (i) => i.state === 'downloading' || i.state === 'processing'
  ).length;

  /** Number of items waiting in the queue that have not yet started. */
  const queuedCount = queueItems.filter((i) => i.state === 'queued').length;

  /** Number of items that have finished successfully. */
  const completedCount = queueItems.filter((i) => i.state === 'complete').length;

  return (
    /**
     * Status bar container.
     *
     * `px-4 py-1.5` -- compact padding (16px horizontal, 6px vertical).
     * `bg-surface-secondary` -- slightly elevated background colour.
     * `border-t border-border-light` -- thin top border separating it
     * from the scrollable content above.
     * `text-[11px]` -- 11px font size (below Tailwind's smallest preset)
     * for an unobtrusive footer feel.
     * `text-content-tertiary` -- muted text colour from the design tokens.
     *
     * @see https://tailwindcss.com/docs/font-size  -- arbitrary font size
     */
    <div className="flex items-center justify-between px-4 py-1.5 bg-surface-secondary border-t border-border-light text-[11px] text-content-tertiary">
      {/*
       * Left section: download activity summary.
       * Conditionally renders one or more count spans depending on
       * which states have items. If the queue is empty, a "No downloads"
       * placeholder is shown instead.
       */}
      <div className="flex items-center gap-3">
        {/*
         * Active downloads indicator.
         * The small dot (`w-1.5 h-1.5 rounded-full`) uses `bg-status-info`
         * (blue) and `animate-pulse` (Tailwind's built-in pulsing animation)
         * to draw attention to ongoing activity.
         * @see https://tailwindcss.com/docs/animation#pulse
         */}
        {activeCount > 0 && (
          <span className="flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full bg-status-info animate-pulse" />
            {activeCount} downloading
          </span>
        )}
        {/* Queued count -- items waiting to start */}
        {queuedCount > 0 && <span>{queuedCount} queued</span>}
        {/* Completed count -- successfully finished items */}
        {completedCount > 0 && <span>{completedCount} completed</span>}
        {/* Empty-queue fallback message */}
        {queueItems.length === 0 && <span>No downloads</span>}
      </div>

      {/* Centre: after-queue action indicator (if non-default) */}
      <AfterQueueIndicator />

      {/* Right section: application version string (fetched from tauri.conf.json) */}
      <span>MeedyaDL v{appVersion}</span>
    </div>
  );
}
