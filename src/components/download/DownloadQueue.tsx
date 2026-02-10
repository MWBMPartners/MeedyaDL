/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Download queue page component.
 * Displays all queued, active, completed, and failed downloads in a
 * scrollable list. Provides controls to cancel, remove, and clear items.
 */

import { useEffect } from 'react';
import { RefreshCw, Trash2 } from 'lucide-react';
import { useDownloadStore } from '@/stores/downloadStore';
import { useUiStore } from '@/stores/uiStore';
import { Button } from '@/components/common';
import { PageHeader } from '@/components/layout';
import { QueueItem } from './QueueItem';

/**
 * Renders the download queue page showing all download items
 * with their current status, progress, and available actions.
 * Supports cancel, retry, remove, and clear-all operations.
 */
export function DownloadQueue() {
  const queueItems = useDownloadStore((s) => s.queueItems);
  const refreshQueue = useDownloadStore((s) => s.refreshQueue);
  const cancelDownload = useDownloadStore((s) => s.cancelDownload);
  const retryDownload = useDownloadStore((s) => s.retryDownload);
  const clearFinished = useDownloadStore((s) => s.clearFinished);
  const addToast = useUiStore((s) => s.addToast);

  /* Refresh queue status on mount and periodically */
  useEffect(() => {
    refreshQueue();
    const interval = setInterval(refreshQueue, 3000);
    return () => clearInterval(interval);
  }, [refreshQueue]);

  /** Cancel an active download */
  const handleCancel = async (id: string) => {
    try {
      await cancelDownload(id);
      addToast('Download cancelled', 'info');
    } catch {
      addToast('Failed to cancel download', 'error');
    }
  };

  /** Retry a failed or cancelled download */
  const handleRetry = async (id: string) => {
    try {
      await retryDownload(id);
      addToast('Download requeued', 'info');
    } catch {
      addToast('Failed to retry download', 'error');
    }
  };

  /** Clear all completed, failed, and cancelled items from the queue */
  const handleClearAll = async () => {
    try {
      const removed = await clearFinished();
      addToast(`Cleared ${removed} item${removed !== 1 ? 's' : ''}`, 'info');
    } catch {
      addToast('Failed to clear queue', 'error');
    }
  };

  /** Count of finished items (eligible for clear) */
  const finishedCount = queueItems.filter(
    (i) => i.state === 'complete' || i.state === 'error' || i.state === 'cancelled',
  ).length;

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Queue"
        subtitle={`${queueItems.length} item${queueItems.length !== 1 ? 's' : ''} in queue`}
        actions={
          <div className="flex gap-2">
            {/* Clear completed/failed/cancelled items */}
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

            {/* Manual refresh */}
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

      {/* Queue item list */}
      <div className="flex-1 overflow-y-auto">
        {queueItems.length === 0 ? (
          /* Empty state */
          <div className="flex flex-col items-center justify-center h-full text-content-tertiary">
            <p className="text-sm">No downloads in queue</p>
            <p className="text-xs mt-1">
              Add a download from the Download page to get started
            </p>
          </div>
        ) : (
          /* Queue items */
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
