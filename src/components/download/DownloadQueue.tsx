// Copyright (c) 2024-2026 MWBM Partners Ltd
/**
 * @file Download queue page component.
 *
 * Displays all queued, active, completed, and failed downloads in a
 * scrollable list. Provides controls to cancel, retry, and clear items.
 *
 * ## Real-time progress updates
 *
 * Download progress is updated via two complementary mechanisms:
 *
 *  1. **Tauri event listeners** -- The app-level event setup (typically in
 *     `App.tsx` or a dedicated hook) listens for `gamdl://progress`,
 *     `gamdl://complete`, `gamdl://error`, and `gamdl://cancelled` events
 *     emitted by the Rust backend. These events call the corresponding
 *     handler methods on the download store (`handleProgressEvent`,
 *     `handleDownloadComplete`, etc.), which update `queueItems[]` in
 *     real-time and trigger re-renders of this component and its children.
 *
 *  2. **Polling fallback** -- A `setInterval` in this component's
 *     `useEffect` calls `refreshQueue()` every 3 seconds as a safety net.
 *     This catches any events that might have been missed (e.g., if the
 *     component mounted after an event was emitted) and keeps the UI
 *     consistent with the backend's ground truth.
 *
 * ## Store connections
 *
 *  - {@link useDownloadStore} -- reads `queueItems[]` and calls
 *    `refreshQueue()`, `cancelDownload()`, `retryDownload()`, and
 *    `clearFinished()`.
 *  - {@link useUiStore} -- `addToast()` for success/error feedback.
 *
 * @see https://react.dev/reference/react/useEffect   -- polling setup.
 * @see https://v2.tauri.app/develop/calling-rust/#events
 *      Tauri events documentation.
 * @see https://lucide.dev/icons/refresh-cw           -- refresh icon.
 * @see https://lucide.dev/icons/trash-2              -- clear icon.
 */

/**
 * React `useEffect` hook for the polling interval on mount.
 * @see https://react.dev/reference/react/useEffect
 */
import { useEffect } from 'react';

/**
 * Lucide icons for the page header action buttons.
 * - `RefreshCw` -- manual refresh button (@see https://lucide.dev/icons/refresh-cw)
 * - `Trash2`    -- "Clear Finished" button (@see https://lucide.dev/icons/trash-2)
 */
import { RefreshCw, Trash2 } from 'lucide-react';

/**
 * Zustand store hooks.
 * @see useDownloadStore in @/stores/downloadStore.ts -- queue state & operations.
 * @see useUiStore in @/stores/uiStore.ts            -- toast notifications.
 */
import { useDownloadStore } from '@/stores/downloadStore';
import { useUiStore } from '@/stores/uiStore';

/** Reusable button component from the common library. */
import { Button } from '@/components/common';

/** Page header component for consistent page-level headings. */
import { PageHeader } from '@/components/layout';

/**
 * Individual queue item row component.
 * @see QueueItem in ./QueueItem.tsx
 */
import { QueueItem } from './QueueItem';

/**
 * Renders the download queue page showing all download items with their
 * current status, real-time progress, and available actions.
 *
 * Supports:
 *  - **Cancel** -- stops an active or queued download.
 *  - **Retry**  -- re-queues a failed or cancelled download.
 *  - **Clear Finished** -- removes all completed/failed/cancelled items.
 *  - **Refresh** -- manual queue refresh from the Rust backend.
 *
 * The page fills the full height of the content area (`h-full`) with
 * the PageHeader pinned at the top and a scrollable item list below.
 *
 * @see https://react.dev/reference/react/useEffect  -- polling setup
 * @see https://react.dev/learn/rendering-lists       -- .map() rendering
 */
export function DownloadQueue() {
  // ---------------------------------------------------------------
  // Store selectors (Zustand)
  // ---------------------------------------------------------------

  /**
   * The complete array of queue items, each with a `state`, `progress`,
   * `speed`, `eta`, `error`, etc. This is the primary data source for
   * the entire queue UI.
   * @see QueueItemStatus in @/types/index.ts
   */
  const queueItems = useDownloadStore((s) => s.queueItems);

  /**
   * Fetches the latest queue state from the Rust backend via the
   * `getQueueStatus` Tauri command and replaces `queueItems` in the store.
   * Called on mount, every 3 seconds, and on manual refresh.
   */
  const refreshQueue = useDownloadStore((s) => s.refreshQueue);

  /**
   * Cancels an active or queued download by ID via the `cancelDownload`
   * Tauri command. The backend will emit a `gamdl://cancelled` event
   * which updates the item state to 'cancelled'.
   */
  const cancelDownload = useDownloadStore((s) => s.cancelDownload);

  /**
   * Retries a failed or cancelled download by ID via the `retryDownload`
   * Tauri command. The backend re-queues the download and the item
   * transitions back to 'queued' state.
   */
  const retryDownload = useDownloadStore((s) => s.retryDownload);

  /**
   * Removes all finished items (complete, error, cancelled) from the
   * backend queue and returns the number of items removed.
   */
  const clearFinished = useDownloadStore((s) => s.clearFinished);

  /** Shows a toast notification for action feedback. */
  const addToast = useUiStore((s) => s.addToast);

  // ---------------------------------------------------------------
  // Polling effect
  // ---------------------------------------------------------------

  /**
   * On mount: immediately refresh the queue, then set up a 3-second
   * polling interval as a fallback for any missed Tauri events.
   *
   * The cleanup function (`return () => clearInterval(...)`) stops the
   * interval when the component unmounts (e.g., user navigates away
   * from the Queue page), preventing memory leaks and unnecessary
   * backend calls.
   *
   * `refreshQueue` is listed as a dependency. Because it is a stable
   * function reference from Zustand (created once in the store), this
   * effect only runs once on mount.
   *
   * @see https://react.dev/reference/react/useEffect#connecting-to-an-external-system
   */
  useEffect(() => {
    refreshQueue();
    const interval = setInterval(refreshQueue, 3000);
    return () => clearInterval(interval);
  }, [refreshQueue]);

  // ---------------------------------------------------------------
  // Event handlers (passed down to QueueItem children)
  // ---------------------------------------------------------------

  /**
   * Cancel an active or queued download.
   * Wraps `cancelDownload()` with toast feedback.
   * @param id - The unique download ID from the backend.
   */
  const handleCancel = async (id: string) => {
    try {
      await cancelDownload(id);
      addToast('Download cancelled', 'info');
    } catch {
      addToast('Failed to cancel download', 'error');
    }
  };

  /**
   * Retry a failed or cancelled download.
   * Wraps `retryDownload()` with toast feedback.
   * @param id - The unique download ID from the backend.
   */
  const handleRetry = async (id: string) => {
    try {
      await retryDownload(id);
      addToast('Download requeued', 'info');
    } catch {
      addToast('Failed to retry download', 'error');
    }
  };

  /**
   * Clear all finished items (complete, error, cancelled) from the queue.
   * Wraps `clearFinished()` with toast feedback showing the count removed.
   */
  const handleClearAll = async () => {
    try {
      const removed = await clearFinished();
      addToast(`Cleared ${removed} item${removed !== 1 ? 's' : ''}`, 'info');
    } catch {
      addToast('Failed to clear queue', 'error');
    }
  };

  // ---------------------------------------------------------------
  // Derived values
  // ---------------------------------------------------------------

  /**
   * Count of items eligible for the "Clear Finished" action: items in
   * 'complete', 'error', or 'cancelled' state. The button is only
   * shown when this count is greater than zero.
   */
  const finishedCount = queueItems.filter(
    (i) => i.state === 'complete' || i.state === 'error' || i.state === 'cancelled',
  ).length;

  // ---------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------
  return (
    /**
     * Outer container: `flex flex-col h-full` ensures the PageHeader
     * stays pinned at the top and the queue list scrolls independently.
     */
    <div className="flex flex-col h-full">
      {/*
       * Page header with dynamic subtitle showing the total item count.
       * The `actions` slot contains "Clear Finished" and "Refresh" buttons.
       * @see PageHeader in @/components/layout/PageHeader.tsx
       */}
      <PageHeader
        title="Queue"
        subtitle={`${queueItems.length} item${queueItems.length !== 1 ? 's' : ''} in queue`}
        actions={
          <div className="flex gap-2">
            {/*
             * "Clear Finished" button -- only rendered when there are
             * completed, errored, or cancelled items to clear.
             * Shows the count in parentheses for quick visibility.
             */}
            {finishedCount > 0 && (
              <Button
                variant="ghost"
                size="sm"
                icon={<Trash2 size={14} />}
                onClick={handleClearAll}
              >
                Clear Finished ({finishedCount})
              </Button>
            )}

            {/*
             * Manual refresh button -- fetches the latest queue state
             * from the backend. Useful if real-time events are delayed
             * or if the user wants an instant update.
             */}
            <Button
              variant="ghost"
              size="sm"
              icon={<RefreshCw size={14} />}
              onClick={() => refreshQueue()}
            >
              Refresh
            </Button>
          </div>
        }
      />

      {/*
       * Scrollable queue item list.
       * `flex-1` makes it grow to fill remaining space below the header.
       * `overflow-y-auto` enables vertical scrolling when items overflow.
       */}
      <div className="flex-1 overflow-y-auto">
        {queueItems.length === 0 ? (
          /*
           * Empty state -- shown when the queue has no items at all.
           * Centered vertically and horizontally with flex utilities.
           */
          <div className="flex flex-col items-center justify-center h-full text-content-tertiary">
            <p className="text-sm">No downloads in queue</p>
            <p className="text-xs mt-1">
              Add a download from the Download page to get started
            </p>
          </div>
        ) : (
          /*
           * Queue item list -- maps each `QueueItemStatus` to a
           * `<QueueItem>` component. The `key` prop uses the unique
           * download ID from the backend for efficient React reconciliation.
           *
           * `onCancel` and `onRetry` callbacks are passed down so the
           * child component can trigger queue operations without directly
           * accessing the store (prop drilling for explicit data flow).
           *
           * @see QueueItem in ./QueueItem.tsx
           * @see https://react.dev/learn/rendering-lists
           */
          <div>
            {queueItems.map((item) => (
              <QueueItem
                key={item.id}
                item={item}
                onCancel={handleCancel}
                onRetry={handleRetry}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
