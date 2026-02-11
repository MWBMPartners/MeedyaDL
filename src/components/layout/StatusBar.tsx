// Copyright (c) 2024-2026 MWBM Partners Ltd
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

/**
 * Zustand store hook for the download queue.
 * Provides `queueItems` -- an array of `QueueItemStatus` objects whose
 * `.state` field is one of: 'queued' | 'downloading' | 'processing' |
 * 'complete' | 'error' | 'cancelled'.
 * @see useDownloadStore in @/stores/downloadStore.ts
 * @see QueueItemStatus in @/types/index.ts
 */
import { useDownloadStore } from '@/stores/downloadStore';

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
    (i) => i.state === 'downloading' || i.state === 'processing',
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

      {/* Right section: application version string */}
      <span>gamdl-GUI v0.1.0</span>
    </div>
  );
}
