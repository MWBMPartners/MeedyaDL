// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Virtualized queue-item list (#467).
 *
 * Renders the `QueueItem` rows inside a `@tanstack/react-virtual`
 * scroll container so only the rows currently in the viewport (plus
 * a small overscan buffer) are kept in the React tree. Without
 * virtualization a 500+ item queue would mount 500+ `QueueItem`
 * components, each subscribing to its own progress events — which
 * was the documented pain point in #467 ("perf: virtualize queue
 * rendering for large queues").
 *
 * Mirrors the pattern already used in `ActivityLog.tsx` for the same
 * reason: dynamic-height row measurement so wrapped multi-line rows
 * (errors, fallback warnings, expanded captions) don't overlap. Both
 * components share the `useVirtualizer` hook + `measureElement` ref.
 *
 * The empty state and the queue-reorder `canMoveUp` / `canMoveDown`
 * computation moved here from `DownloadQueue.tsx` so that file stays
 * focused on the page-level shell (header, action buttons, modals).
 */

import { useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Download } from 'lucide-react';

import type { QueueItemStatus } from '@/types';

import { QueueItem } from './QueueItem';

/**
 * Props for the {@link QueueListVirtualized} component.
 *
 * The parent (`DownloadQueue`) owns all event handlers + toast
 * feedback; this component only handles the virtualized rendering
 * shell + per-row reorder-flag computation.
 */
interface QueueListVirtualizedProps {
  queueItems: QueueItemStatus[];
  onCancel: (id: string) => void;
  onRetry: (id: string) => void;
  onRetryWithoutWrapper: (id: string) => void;
  onCopyUrl: (url: string) => void;
  onDelete: (id: string) => void;
  onMoveToTop: (id: string) => void;
  onMoveUp: (id: string) => void;
  onMoveDown: (id: string) => void;
  onMoveToBottom: (id: string) => void;
}

export function QueueListVirtualized({
  queueItems,
  onCancel,
  onRetry,
  onRetryWithoutWrapper,
  onCopyUrl,
  onDelete,
  onMoveToTop,
  onMoveUp,
  onMoveDown,
  onMoveToBottom,
}: QueueListVirtualizedProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null);

  /**
   * Compute the queued-only sub-sequence's id list once per render so
   * each row knows its position within it for the reorder context-menu
   * disable flags (#782). Recomputed on every render because the queue
   * mutates frequently (progress events, state transitions, reorders).
   * Cheap: linear scan + linear filter over `queueItems`.
   */
  const queuedIds = queueItems
    .filter((i) => i.state === 'queued')
    .map((i) => i.id);

  /**
   * Virtualizer config mirrors `ActivityLog.tsx`:
   *   - `estimateSize` is a single-line guess; the dynamic
   *     `measureElement` ref on each row replaces it with the
   *     actual measured height as soon as the row mounts. That
   *     prevents the overlap bug that #442 / #575 fixed for the
   *     activity log.
   *   - `overscan: 10` keeps a small buffer above/below the
   *     viewport so fast scrolls don't reveal blank space.
   *   - The list is keyed by the stable backend `id` so React
   *     reconciles correctly when items are reordered (#782) or
   *     the queue mutates between renders.
   */
  const virtualizer = useVirtualizer({
    count: queueItems.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 80, // single-line item baseline; measureElement adjusts
    overscan: 10,
    getItemKey: (index) => queueItems[index]?.id ?? index,
  });

  // Empty state — bail out before wiring the virtualizer container so
  // the empty-state UI gets the full flex-1 height.
  if (queueItems.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto" ref={scrollRef}>
        <div className="flex flex-col items-center justify-center h-full text-content-tertiary">
          <Download size={40} className="mb-4 opacity-30" />
          <p className="text-sm font-medium">No downloads in queue</p>
          <p className="text-xs mt-1 text-center max-w-xs">
            Paste an Apple Music URL on the Download page to get started, or
            copy a URL to your clipboard and MeedyaDL will detect it
            automatically.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="flex-1 overflow-y-auto"
      role="list"
      aria-label="Download queue items"
    >
      <div
        style={{
          height: `${virtualizer.getTotalSize()}px`,
          width: '100%',
          position: 'relative',
        }}
      >
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const item = queueItems[virtualRow.index];
          if (!item) return null;
          const queuedPos =
            item.state === 'queued' ? queuedIds.indexOf(item.id) : -1;
          const canMoveUp = queuedPos > 0;
          const canMoveDown =
            queuedPos >= 0 && queuedPos < queuedIds.length - 1;
          return (
            <div
              key={item.id}
              ref={virtualizer.measureElement}
              data-index={virtualRow.index}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              <QueueItem
                item={item}
                onCancel={onCancel}
                onRetry={onRetry}
                onRetryWithoutWrapper={onRetryWithoutWrapper}
                onCopyUrl={() => onCopyUrl(item.urls?.[0] ?? '')}
                onDelete={onDelete}
                onMoveToTop={onMoveToTop}
                onMoveUp={onMoveUp}
                onMoveDown={onMoveDown}
                onMoveToBottom={onMoveToBottom}
                canMoveUp={canMoveUp}
                canMoveDown={canMoveDown}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
