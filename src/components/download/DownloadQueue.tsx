// Copyright (c) 2026 MeedyaDL
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
 * React hooks:
 * - `useEffect`  -- polling interval on mount.
 * - `useMemo`    -- derived queue statistics (counts, overall progress).
 * @see https://react.dev/reference/react/useEffect
 * @see https://react.dev/reference/react/useMemo
 */
import { useEffect, useMemo, useState } from 'react';

/**
 * Lucide icons for the page header action buttons.
 * - `RefreshCw` -- manual refresh button (@see https://lucide.dev/icons/refresh-cw)
 * - `Trash2`    -- "Clear Finished" button (@see https://lucide.dev/icons/trash-2)
 */
import { Download, Play, RefreshCw, Trash2, Upload } from 'lucide-react';

/**
 * Zustand store hooks.
 * @see useDownloadStore in @/stores/downloadStore.ts -- queue state & operations.
 * @see useUiStore in @/stores/uiStore.ts            -- toast notifications.
 */
import { useDownloadStore } from '@/stores/downloadStore';
import { useUiStore } from '@/stores/uiStore';

/** Reusable UI components from the common library. */
import { Button, Modal } from '@/components/common';

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
   * Retries a failed download with wrapper disabled, falling back to
   * cookie-based authentication. Only for downloads that used wrapper.
   */
  const retryWithoutWrapper = useDownloadStore((s) => s.retryWithoutWrapper);

  /**
   * Removes all finished items (complete, error, cancelled) from the
   * backend queue and returns the number of items removed.
   */
  const clearFinished = useDownloadStore((s) => s.clearFinished);

  /** Clear ALL non-active items from the queue. */
  const clearAll = useDownloadStore((s) => s.clearAll);

  /**
   * Exports the current queue to a `.meedyadl` file via a native save dialog.
   * Only non-terminal items (queued/active) are included in the export.
   */
  const exportQueue = useDownloadStore((s) => s.exportQueue);

  /**
   * Imports queue items from a `.meedyadl` file via a native file picker.
   * Imported items are enqueued and processing starts automatically.
   */
  const importQueue = useDownloadStore((s) => s.importQueue);

  /**
   * Manually triggers queue processing. Used when auto_start_queue is
   * disabled and the user clicks "Start Queue".
   */
  const processQueue = useDownloadStore((s) => s.processQueue);

  /** Shows a toast notification for action feedback. */
  const addToast = useUiStore((s) => s.addToast);

  /** Confirmation modal state for "Clear All". */
  const [showClearAllConfirm, setShowClearAllConfirm] = useState(false);

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
   * Retry a failed download without the wrapper system.
   * Wraps `retryWithoutWrapper()` with toast feedback.
   * @param id - The unique download ID from the backend.
   */
  const handleRetryWithoutWrapper = async (id: string) => {
    try {
      await retryWithoutWrapper(id);
      addToast('Download requeued without wrapper', 'info');
    } catch {
      addToast('Failed to retry download without wrapper', 'error');
    }
  };

  /**
   * Export the current queue to a `.meedyadl` file.
   * Wraps `exportQueue()` with toast feedback showing the count exported.
   */
  const handleExport = async () => {
    try {
      const count = await exportQueue();
      if (count > 0) {
        addToast(`Exported ${count} item${count !== 1 ? 's' : ''}`, 'success');
      }
    } catch {
      addToast('Failed to export queue', 'error');
    }
  };

  /**
   * Import queue items from a `.meedyadl` file.
   * Wraps `importQueue()` with toast feedback showing the count imported.
   */
  const handleImport = async () => {
    try {
      const count = await importQueue();
      if (count > 0) {
        addToast(`Imported ${count} item${count !== 1 ? 's' : ''}`, 'success');
      }
    } catch {
      addToast('Failed to import queue', 'error');
    }
  };

  /**
   * Manually start processing the download queue.
   * Wraps `processQueue()` with toast feedback.
   */
  const handleStartQueue = async () => {
    try {
      await processQueue();
      addToast('Queue processing started', 'info');
    } catch {
      addToast('Failed to start queue processing', 'error');
    }
  };

  /**
   * Clear all finished items (complete, error, cancelled) from the queue.
   * Wraps `clearFinished()` with toast feedback showing the count removed.
   */
  const handleClearFinished = async () => {
    try {
      const removed = await clearFinished();
      addToast(`Cleared ${removed} item${removed !== 1 ? 's' : ''}`, 'info');
    } catch {
      addToast('Failed to clear queue', 'error');
    }
  };

  /** Clear ALL non-active items after user confirms. */
  const handleClearAllConfirmed = async () => {
    setShowClearAllConfirm(false);
    try {
      const removed = await clearAll();
      addToast(`Cleared all ${removed} item${removed !== 1 ? 's' : ''}`, 'info');
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
    (i) => i.state === 'complete' || i.state === 'cancelled'
  ).length;

  /**
   * Count of items eligible for export: non-terminal items that are
   * queued, downloading, or processing. The Export button is only
   * shown when this count is greater than zero.
   */
  const exportableCount = queueItems.filter(
    (i) => i.state === 'queued' || i.state === 'downloading' || i.state === 'processing'
  ).length;

  /**
   * Count of items waiting to be processed (in 'queued' state).
   * Used to conditionally show the "Start Queue" button.
   */
  const queuedCount = queueItems.filter((i) => i.state === 'queued').length;

  /**
   * Count of items currently being processed (downloading or processing).
   * The "Start Queue" button is shown when items are queued but none active.
   */
  const activeCount = queueItems.filter(
    (i) => i.state === 'downloading' || i.state === 'processing'
  ).length;

  /**
   * Aggregate queue statistics derived from the current queue items.
   * Computes per-state counts and an overall progress ratio (completed / total).
   * Only non-zero counts are included in the display segments array.
   *
   * Memoised to avoid recalculating on every render when the queue items
   * array reference has not changed.
   *
   * @see https://react.dev/reference/react/useMemo
   */
  const queueStats = useMemo(() => {
    const total = queueItems.length;
    const completed = queueItems.filter((i) => i.state === 'complete').length;
    const failed = queueItems.filter((i) => i.state === 'error').length;
    const active = queueItems.filter(
      (i) => i.state === 'downloading' || i.state === 'processing'
    ).length;
    const queued = queueItems.filter((i) => i.state === 'queued').length;

    /**
     * Overall progress percentage: ratio of completed items to total items.
     * Returns 0 when the queue is empty to avoid division by zero.
     */
    const overallProgress = total > 0 ? Math.round((completed / total) * 100) : 0;

    /**
     * Build an array of display segments, only including non-zero counts.
     * Each segment has a label and a Tailwind colour class matching the
     * state icon colours from QueueItem's STATE_CONFIG.
     */
    const segments: { label: string; colorClass: string }[] = [];
    if (active > 0) segments.push({ label: `${active} active`, colorClass: 'text-status-info' });
    if (queued > 0) segments.push({ label: `${queued} queued`, colorClass: 'text-content-tertiary' });
    if (completed > 0)
      segments.push({ label: `${completed} completed`, colorClass: 'text-status-success' });
    if (failed > 0) segments.push({ label: `${failed} failed`, colorClass: 'text-status-error' });

    return { total, completed, overallProgress, segments };
  }, [queueItems]);

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
             * "Start Queue" button -- shown when there are queued items
             * waiting to be processed and no downloads are currently active.
             * Always visible in manual mode; also shown in auto mode as a
             * fallback if processing stalled.
             */}
            {queuedCount > 0 && activeCount === 0 && (
              <Button
                variant="primary"
                size="sm"
                icon={<Play size={14} />}
                onClick={handleStartQueue}
              >
                Start Queue ({queuedCount})
              </Button>
            )}

            {/*
             * "Import" button -- always shown, opens a native file picker
             * to import queue items from a .meedyadl file.
             */}
            <Button variant="ghost" size="sm" icon={<Download size={14} />} onClick={handleImport}>
              Import
            </Button>

            {/*
             * "Export" button -- always visible when queue has items.
             * Disabled when there are no non-terminal items to export.
             */}
            {queueItems.length > 0 && (
              <Button
                variant="ghost"
                size="sm"
                icon={<Upload size={14} />}
                onClick={handleExport}
                disabled={exportableCount === 0}
              >
                Export{exportableCount > 0 ? ` (${exportableCount})` : ''}
              </Button>
            )}

            {/*
             * "Clear Completed" button -- only rendered when there are
             * completed or cancelled items to clear. Errored items are
             * kept so the user can review and retry them.
             */}
            {finishedCount > 0 && (
              <Button
                variant="ghost"
                size="sm"
                icon={<Trash2 size={14} />}
                onClick={handleClearFinished}
              >
                Clear Completed ({finishedCount})
              </Button>
            )}

            {queueItems.length > 0 && (
              <Button
                variant="ghost"
                size="sm"
                icon={<Trash2 size={14} />}
                onClick={() => setShowClearAllConfirm(true)}
              >
                Clear All
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
       * Queue statistics bar -- shown when the queue has items.
       * Displays per-state counts (only non-zero) and an overall
       * progress bar showing the completed/total ratio.
       */}
      {queueItems.length > 0 && (
        <div className="px-4 py-2 border-b border-border-light bg-surface-secondary">
          {/*
           * Top row: per-state count segments separated by middot characters.
           * Each segment is coloured to match its state icon (e.g., blue for
           * active, green for completed) for visual consistency with QueueItem.
           */}
          <div className="flex items-center gap-1 text-xs">
            {queueStats.segments.map((seg, idx) => (
              <span key={seg.label} className="flex items-center gap-1">
                {idx > 0 && <span className="text-content-tertiary">&middot;</span>}
                <span className={seg.colorClass}>{seg.label}</span>
              </span>
            ))}
          </div>

          {/* Overall progress bar removed — now rendered globally
           * in MainLayout via GlobalProgressBar, visible on all pages. */}
        </div>
      )}

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
            <p className="text-xs mt-1">Add a download from the Download page to get started</p>
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
                onRetryWithoutWrapper={handleRetryWithoutWrapper}
                onCopyUrl={() => addToast('Link copied to clipboard', 'success')}
              />
            ))}
          </div>
        )}
      </div>

      {/* Confirmation modal for "Clear All" */}
      <Modal
        open={showClearAllConfirm}
        onClose={() => setShowClearAllConfirm(false)}
        title="Clear All Queue Items"
      >
        <p className="text-sm text-content-secondary mb-4">
          This will remove all queued, completed, failed, and cancelled items
          from the download queue. Active downloads will not be interrupted.
        </p>
        <p className="text-sm text-content-secondary mb-6">
          This action cannot be undone.
        </p>
        <div className="flex justify-end gap-2">
          <Button
            variant="ghost"
            onClick={() => setShowClearAllConfirm(false)}
          >
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={handleClearAllConfirmed}
          >
            Clear All
          </Button>
        </div>
      </Modal>
    </div>
  );
}
