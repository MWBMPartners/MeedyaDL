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
import { useCallback, useEffect, useMemo, useState } from 'react';

/**
 * Lucide icons for the page header action buttons.
 * - `RefreshCw` -- manual refresh button (@see https://lucide.dev/icons/refresh-cw)
 * - `Trash2`    -- "Clear Finished" button (@see https://lucide.dev/icons/trash-2)
 */
import { Download, Play, RefreshCw, RotateCcw, Square, Trash2, Upload } from 'lucide-react';

/**
 * Zustand store hooks.
 * @see useDownloadStore in @/stores/downloadStore.ts -- queue state & operations.
 * @see useUiStore in @/stores/uiStore.ts            -- toast notifications.
 */
import { useDownloadStore } from '@/stores/downloadStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { useUiStore } from '@/stores/uiStore';
import { withErrorToast } from '@/lib/withErrorToast';
import { useConfirmation } from '@/lib/useConfirmation';
import {
  moveQueueItemToTop,
  moveQueueItemToBottom,
  moveQueueItemUp,
  moveQueueItemDown,
} from '@/lib/tauri-commands';

/** Reusable UI components from the common library. */
import { Button, Modal } from '@/components/common';

/** Page header component for consistent page-level headings. */
import { PageHeader } from '@/components/layout';

/**
 * Individual queue item row component.
 * @see QueueItem in ./QueueItem.tsx
 */
import { QueueItem } from './QueueItem';

/** Per-item snapshot type used by the Delete confirmation modal (#685). */
import type { QueueItemStatus } from '@/types';

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

  /** Delete a single non-active item from the queue (#685). */
  const deleteItem = useDownloadStore((s) => s.deleteItem);

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

  /**
   * Confirmation modal target for per-item Delete (#685). Holds the
   * full queue item snapshot so the modal can render a meaningful label
   * (URL, current track) without re-querying the store. `null` means the
   * modal is closed. Per-item delete needs the full target snapshot in
   * its description so it stays as plain Modal state, not useConfirmation.
   */
  const [deleteTarget, setDeleteTarget] = useState<QueueItemStatus | null>(null);

  /**
   * Confirmation modal state for "Abort Queue" (#620). The destructive
   * nature of this action (cancels every active + queued item at once)
   * warrants a confirmation gate rather than a bare click.
   */
  const [showAbortConfirm, setShowAbortConfirm] = useState(false);

  /**
   * Aborts every active + queued download in one IPC call. The store's
   * `abortAll` surfaces its own summary toast; this wrapper dispatches
   * and closes the confirmation modal.
   */
  const abortAll = useDownloadStore((s) => s.abortAll);

  /**
   * Per-session snapshot of the "Don't ask again" checkbox state inside
   * the modal. When the user confirms with the box ticked, we flip
   * `abort_queue_confirm` off in settings so subsequent aborts fire
   * immediately from the button / keyboard shortcut (#620).
   */
  const [abortDontAskAgain, setAbortDontAskAgain] = useState(false);

  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  const handleAbortAll = useCallback(async () => {
    setShowAbortConfirm(false);
    if (abortDontAskAgain) {
      // User opted to skip the confirmation next time. Persist before
      // kicking off the abort so a race between the IPC and the
      // settings save can't lose the preference.
      await updateSettings({ abort_queue_confirm: false });
      setAbortDontAskAgain(false);
    }
    await abortAll();
  }, [abortAll, abortDontAskAgain, updateSettings]);

  /**
   * Entry point for the "Abort Queue" action. Honours the
   * `abort_queue_confirm` setting: shows the modal on `true` (the
   * default), fires directly on `false` (user has opted in to
   * single-click aborts).
   */
  const triggerAbort = useCallback(() => {
    if (settings.abort_queue_confirm) {
      setShowAbortConfirm(true);
    } else {
      void abortAll();
    }
  }, [abortAll, settings.abort_queue_confirm]);

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
   * Queue reorder handlers (#782). Each calls the matching IPC, lets
   * the backend's `queue-updated` event refresh the store
   * automatically, and shows a toast only on hard failure (the IPC
   * never throws on no-op moves — those silently `return false`).
   *
   * No optimistic UI update needed: the backend persists the new
   * order to `queue.json` AND emits `queue-updated`, and the App-
   * level event listener calls `refreshQueue()` which pulls the
   * authoritative state from disk on the next tick. This keeps the
   * client always in sync with the persisted order, including
   * across an app restart.
   */
  const handleMoveToTop = async (id: string) => {
    try {
      await moveQueueItemToTop(id);
    } catch {
      addToast('Failed to move item', 'error');
    }
  };
  const handleMoveUp = async (id: string) => {
    try {
      await moveQueueItemUp(id);
    } catch {
      addToast('Failed to move item', 'error');
    }
  };
  const handleMoveDown = async (id: string) => {
    try {
      await moveQueueItemDown(id);
    } catch {
      addToast('Failed to move item', 'error');
    }
  };
  const handleMoveToBottom = async (id: string) => {
    try {
      await moveQueueItemToBottom(id);
    } catch {
      addToast('Failed to move item', 'error');
    }
  };

  /**
   * Export the current queue to a `.meedyadl` file.
   * Wraps `exportQueue()` with toast feedback showing the count exported.
   */
  const handleExport = async () => {
    const count = await withErrorToast(() => exportQueue(), {
      errorMsg: 'Failed to export queue',
    });
    if (count !== undefined && count > 0) {
      addToast(`Exported ${count} item${count !== 1 ? 's' : ''}`, 'success');
    }
  };

  /**
   * Import queue items from a `.meedyadl` file.
   * Wraps `importQueue()` with toast feedback showing the count imported.
   */
  const handleImport = async () => {
    const count = await withErrorToast(() => importQueue(), {
      errorMsg: 'Failed to import queue',
    });
    if (count !== undefined && count > 0) {
      addToast(`Imported ${count} item${count !== 1 ? 's' : ''}`, 'success');
    }
  };

  /**
   * Manually start processing the download queue.
   * Wraps `processQueue()` with toast feedback.
   */
  const handleStartQueue = async () => {
    await withErrorToast(() => processQueue(), {
      successMsg: 'Queue processing started',
      successVariant: 'info',
      errorMsg: 'Failed to start queue processing',
    });
  };

  /**
   * Clear all finished items (complete, error, cancelled) from the queue.
   * Wraps `clearFinished()` with toast feedback showing the count removed.
   */
  const handleClearFinished = async () => {
    const removed = await withErrorToast(() => clearFinished(), {
      errorMsg: 'Failed to clear queue',
    });
    if (removed !== undefined) {
      addToast(`Cleared ${removed} item${removed !== 1 ? 's' : ''}`, 'info');
    }
  };

  /** Clear ALL non-active items. The useConfirmation hook auto-closes
   *  on resolve, so this just runs the action and surfaces the toast. */
  const handleClearAllConfirmed = async () => {
    const removed = await withErrorToast(() => clearAll(), {
      errorMsg: 'Failed to clear queue',
    });
    if (removed !== undefined) {
      addToast(`Cleared all ${removed} item${removed !== 1 ? 's' : ''}`, 'info');
    }
  };

  /**
   * Per-item Delete (#685). Opens the confirmation modal with the
   * targeted item — actual deletion happens in
   * `handleDeleteItemConfirmed` only after the user confirms.
   */
  const handleDeleteItem = (id: string) => {
    const target = queueItems.find((i) => i.id === id);
    if (!target) return;
    setDeleteTarget(target);
  };

  /**
   * Confirms the Delete action for the currently-targeted item. Errors
   * surface as a toast (e.g. backend race where the item became active
   * after the menu opened — Rust guard would reject).
   */
  const handleDeleteItemConfirmed = async () => {
    const target = deleteTarget;
    if (!target) return;
    setDeleteTarget(null);
    try {
      await deleteItem(target.id);
      addToast('Item removed from queue', 'info');
    } catch (e) {
      addToast(
        e instanceof Error ? e.message : 'Failed to delete item',
        'error',
      );
    }
  };

  /**
   * Bulk retry: iterate every errored item and call the existing
   * retry IPC for each (#665). Each call goes through `retry_download`
   * which now consults the smart manifest planner (#667), so already-
   * downloaded tracks are skipped at the planner layer — bulk retry
   * of 12 items only re-fetches the actually-failed tracks across them.
   *
   * `Promise.allSettled` lets us count successes and failures
   * independently; one bad item shouldn't abort the whole batch.
   */
  const handleRetryAllFailedConfirmed = async () => {
    const failedItems = queueItems.filter((i) => i.state === 'error');
    if (failedItems.length === 0) {
      addToast('No failed downloads to retry', 'info');
      return;
    }
    const results = await Promise.allSettled(
      failedItems.map((i) => retryDownload(i.id)),
    );
    const succeeded = results.filter((r) => r.status === 'fulfilled').length;
    const failed = results.length - succeeded;
    if (failed === 0) {
      addToast(
        `Re-queued ${succeeded} failed download${succeeded !== 1 ? 's' : ''}`,
        'success',
      );
    } else {
      addToast(
        `Re-queued ${succeeded} of ${results.length} (${failed} could not be retried — check items individually)`,
        'warning',
      );
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
   * Count of failed items eligible for the "Retry All Failed" action
   * (#665). Only `error`-state items qualify; cancelled items were
   * stopped intentionally and shouldn't be auto-resumed by a bulk action.
   */
  const failedCount = queueItems.filter((i) => i.state === 'error').length;

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
  // Confirmation modal hooks (audit v2 #3)
  // ---------------------------------------------------------------
  // Each `useConfirmation` returns { open, modal }: `open` wires to a
  // trigger button's onClick; `modal` renders once in the JSX. The
  // hook owns its own open/close state and auto-closes on a
  // successful onConfirm. Stays open if onConfirm throws so the user
  // can retry — onConfirm surfaces its own error toast.
  //
  // The Abort + per-item Delete modals stay as inline <Modal>s for now:
  // Abort has a "don't ask again" checkbox that mutates separate
  // settings state; Delete needs the full target snapshot in its
  // description. Both could migrate but are slightly off the happy path
  // of the hook.

  const retryAllConfirm = useConfirmation({
    title: 'Retry All Failed Downloads',
    description: (
      <>
        <p className="mb-4">
          This will re-queue {failedCount} failed download
          {failedCount !== 1 ? 's' : ''}. The smart retry path skips
          already-downloaded tracks via the manifest, so only the
          actually-failed tracks will be re-fetched.
        </p>
        <p>
          For wrapper-related failures, use the per-item &quot;Retry
          without Wrapper&quot; option instead.
        </p>
      </>
    ),
    confirmLabel: 'Retry All',
    onConfirm: handleRetryAllFailedConfirmed,
  });

  const clearAllConfirm = useConfirmation({
    title: 'Clear All Queue Items',
    description: (
      <>
        <p className="mb-4">
          This will remove all queued, completed, failed, and cancelled
          items from the download queue. Active downloads will not be
          interrupted.
        </p>
        <p>This action cannot be undone.</p>
      </>
    ),
    confirmLabel: 'Clear All',
    onConfirm: handleClearAllConfirmed,
  });

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

            {/*
             * "Retry All Failed" button (#665) — appears when at least one
             * failed item exists. Each retry goes through the smart manifest
             * planner (#667), so already-downloaded tracks are skipped at
             * the planner layer. Confirmation-gated to prevent a misclick
             * from re-queueing twelve items at once.
             */}
            {failedCount > 0 && (
              <Button
                variant="ghost"
                size="sm"
                icon={<RotateCcw size={14} />}
                onClick={retryAllConfirm.open}
              >
                Retry All Failed ({failedCount})
              </Button>
            )}

            {queueItems.length > 0 && (
              <Button
                variant="ghost"
                size="sm"
                icon={<Trash2 size={14} />}
                onClick={clearAllConfirm.open}
              >
                Clear All
              </Button>
            )}

            {/*
             * "Abort Queue" button (#620). Shown whenever there is at least
             * one item in a non-terminal state that could be stopped.
             * Destructive-styled + confirmation-gated — a misclick here
             * would cancel an entire batch download.
             */}
            {(activeCount > 0 || queuedCount > 0) && (
              <Button
                variant="ghost"
                size="sm"
                icon={<Square size={14} />}
                onClick={triggerAbort}
                className="text-status-error hover:bg-status-error/10"
                title="Stop every active and queued download immediately (Cmd/Ctrl+Shift+.)"
              >
                Abort Queue
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
            <Download size={40} className="mb-4 opacity-30" />
            <p className="text-sm font-medium">No downloads in queue</p>
            <p className="text-xs mt-1 text-center max-w-xs">Paste an Apple Music URL on the Download page to get started, or copy a URL to your clipboard and MeedyaDL will detect it automatically.</p>
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
          <div role="list" aria-label="Download queue items">
            {(() => {
              // Compute the queued-only sub-sequence's id list once per
              // render so each row knows its position within it (#782).
              // canMoveUp / canMoveDown disable the matching context-menu
              // entries when the move would be a no-op (topmost queued
              // can't move up; bottommost can't move down). Non-Queued
              // rows simply don't show the move actions, so their
              // can-flags are unused — set both to `false` defensively.
              const queuedIds = queueItems
                .filter((i) => i.state === 'queued')
                .map((i) => i.id);
              return queueItems.map((item) => {
                const queuedPos =
                  item.state === 'queued' ? queuedIds.indexOf(item.id) : -1;
                const canMoveUp = queuedPos > 0;
                const canMoveDown =
                  queuedPos >= 0 && queuedPos < queuedIds.length - 1;
                return (
                  <QueueItem
                    key={item.id}
                    item={item}
                    onCancel={handleCancel}
                    onRetry={handleRetry}
                    onRetryWithoutWrapper={handleRetryWithoutWrapper}
                    onCopyUrl={() => addToast('Link copied to clipboard', 'success')}
                    onDelete={handleDeleteItem}
                    onMoveToTop={handleMoveToTop}
                    onMoveUp={handleMoveUp}
                    onMoveDown={handleMoveDown}
                    onMoveToBottom={handleMoveToBottom}
                    canMoveUp={canMoveUp}
                    canMoveDown={canMoveDown}
                  />
                );
              });
            })()}
          </div>
        )}
      </div>

      {/*
        Retry All Failed (#665) + Clear All confirmation modals — both
        owned by useConfirmation hooks declared above (audit v2 #3).
        The hooks render their own <Modal> here; the trigger buttons
        in the actions row call their `.open` methods.
      */}
      {retryAllConfirm.modal}
      {clearAllConfirm.modal}

      {/* Confirmation modal for per-item Delete (#685) */}
      <Modal
        open={deleteTarget !== null}
        onClose={() => setDeleteTarget(null)}
        title="Remove queue item"
      >
        <p className="text-sm text-content-secondary mb-4">
          Remove this item from the download queue? Already-downloaded
          files on disk are not deleted — only the queue entry.
        </p>
        {deleteTarget && (
          <p className="text-xs text-content-tertiary mb-6 break-all">
            {deleteTarget.current_track || deleteTarget.urls?.[0] || deleteTarget.id}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={() => setDeleteTarget(null)}>
            Cancel
          </Button>
          <Button variant="primary" onClick={handleDeleteItemConfirmed}>
            Remove
          </Button>
        </div>
      </Modal>

      {/* Confirmation modal for "Abort Queue" (#620) */}
      <Modal
        open={showAbortConfirm}
        onClose={() => setShowAbortConfirm(false)}
        title="Abort Queue"
      >
        <p className="text-sm text-content-secondary mb-4">
          This will stop the current download <em>immediately</em> and cancel
          every item that is still queued or processing. Already-completed
          downloads are kept so you don&apos;t lose your history.
        </p>
        <p className="text-sm text-content-secondary mb-4">
          This action cannot be undone. Cancelled items can be retried
          individually from the queue.
        </p>
        <label className="flex items-center gap-2 text-sm text-content-secondary mb-6 cursor-pointer select-none">
          <input
            type="checkbox"
            checked={abortDontAskAgain}
            onChange={(e) => setAbortDontAskAgain(e.target.checked)}
            className="h-4 w-4 cursor-pointer"
          />
          <span>
            Don&apos;t ask again — single-click abort from now on. Re-enable
            in Settings &gt; General &gt; Preferences.
          </span>
        </label>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={() => setShowAbortConfirm(false)}>
            Cancel
          </Button>
          <Button variant="primary" onClick={handleAbortAll}>
            Abort Queue
          </Button>
        </div>
      </Modal>
    </div>
  );
}
