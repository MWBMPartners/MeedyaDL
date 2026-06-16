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

import { useCallback, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Download } from 'lucide-react';

import type { QueueItemStatus } from '@/types';

import { QueueItem } from './QueueItem';

/**
 * #911 Phase 1 item 0 sub-item — sticky column header strip.
 *
 * Inlined inside this file (rather than extracted to a separate
 * component) because it's used by exactly one consumer and weighs
 * ~50 LOC. Promotion to a standalone file is cheap if Phase 2 adds
 * service-filter chips or sortable columns that grow the surface.
 *
 * The strip mirrors the responsive-tier vocabulary of `QueueItem.tsx`:
 * each label uses the SAME `hidden md:flex` / `hidden lg:flex` /
 * `hidden xl:flex` / `hidden 2xl:flex` class as the row column it
 * labels, so labels appear/disappear in lockstep with their columns.
 *
 * Column alignment is preserved via shared structural rules: same
 * outer `flex items-center gap-3 px-4`, same `flex-shrink-0` discipline
 * on every non-flex-1 cell, same album-art spacer width (`w-12 h-12`)
 * even though the strip doesn't render an actual thumbnail. The
 * bulk-select checkbox column reserves a fixed spacer when
 * `hasSelection` is true so column alignment doesn't shift when
 * selection mode toggles.
 *
 * A11y note: the strip uses `role="row"` + `role="columnheader"`
 * children. The labels themselves are visible-only chrome — the row's
 * status pill / metadata cells carry their own accessible names — but
 * the `role` annotations let assistive tech announce "Status column,
 * sortable" once we add sort, and make the strip linear-scrollable
 * separately from the data rows.
 */
function QueueColumnHeader({ hasSelection }: { hasSelection: boolean }) {
  return (
    <div
      role="row"
      className="sticky top-0 z-10 flex items-center gap-3 border-b border-border-light bg-surface-secondary px-4 py-2 text-[10px] font-medium uppercase tracking-wide text-content-tertiary"
    >
      {/* Optional bulk-select column spacer — w-4 matches the
       * checkbox + accent-accent footprint in QueueItem.tsx so the
       * downstream columns stay byte-for-byte aligned. */}
      {hasSelection && <div className="w-4 flex-shrink-0" aria-hidden="true" />}

      {/* Album-art column spacer — no label (the thumbnail itself is
       * the column's content; a label would be redundant). w-12 h-12
       * matches AlbumArtThumbnail's footprint. */}
      <div className="w-12 h-12 flex-shrink-0" aria-hidden="true" />

      {/* Tier 2 — Platform label (hidden below md, matching the
       * row's `hidden md:flex` platform icon column). */}
      <div
        role="columnheader"
        className="hidden md:flex flex-shrink-0 w-5 justify-center"
      >
        <span aria-hidden="true">Svc</span>
      </div>

      {/* Tier 1 — primary identifier column header. The row's
       * primary-identifier cell uses `flex-1 min-w-0`; the header
       * matches exactly so its label tracks the cell's left edge
       * at every breakpoint. */}
      <div role="columnheader" className="flex-1 min-w-0">
        Artist — Album — Track
      </div>

      {/* Tier 3 — Speed / ETA (≥ lg). */}
      <div role="columnheader" className="hidden lg:flex flex-shrink-0">
        Speed / ETA
      </div>

      {/* Tier 4 — Codec / quality (≥ xl). */}
      <div role="columnheader" className="hidden xl:flex flex-shrink-0">
        Codec
      </div>

      {/* Tier 5 — Submitted (≥ 2xl). */}
      <div role="columnheader" className="hidden 2xl:flex flex-shrink-0">
        Submitted
      </div>

      {/* Tier 1 — Status pill column header. Must sit between the
       * Tier 5 timestamp and the actions reservation so it aligns
       * with the row's StatusPill placement. */}
      <div role="columnheader" className="flex-shrink-0">
        Status
      </div>

      {/* Actions + chevron column — no label (icons in the row are
       * self-explanatory via `title` / `aria-label`). Width reserved
       * so the last data column doesn't drift right when actions
       * appear/disappear per row state. The narrow `w-20` is a
       * reasonable approximation of the Cancel + Retry + Chevron
       * cluster at hover-revealed full-opacity. */}
      <div className="w-20 flex-shrink-0" aria-hidden="true" />
    </div>
  );
}

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
  /** Optional bulk-select state (#463). When provided, each row
   *  renders a checkbox bound to this Set; absent means no checkboxes. */
  selectedIds?: ReadonlySet<string>;
  /** Toggle a single row's selection. Required when `selectedIds`
   *  is supplied. */
  onToggleSelect?: (id: string) => void;
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
  selectedIds,
  onToggleSelect,
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
   * Defensive measurement closure (#911 Phase 1 sub-item — virtualiser
   * hardening). Direct mirror of `ActivityLog.tsx::measureElement`
   * (the post-#442 / #575 fix). The `useCallback` empty-dep
   * stabilisation matters now that the click-to-expand affordance
   * makes Queue rows dynamic-height for the first time: a fresh
   * closure on every render was observed elsewhere (ActivityLog) to
   * interact with TanStack's measurement cache in ways that produce
   * row-overlap bugs under fast-update workloads.
   *
   * Fallback to `80` (the legacy estimate) when the element ref is
   * null — happens only momentarily during initial mount.
   */
  const measureElement = useCallback(
    (element: Element | null | undefined) =>
      element?.getBoundingClientRect().height ?? 80,
    [],
  );

  /**
   * Stable per-item key (#911 Phase 1 sub-item — virtualiser
   * hardening). Without this, TanStack keys its measurement cache
   * by positional index, so when the queue reorders (#782) cached
   * row heights attach to the wrong items and the resulting
   * `translateY()` offsets overlap adjacent rows. Same root cause
   * as #575 in ActivityLog.
   *
   * Wrapped in `useCallback([queueItems])` so the identity changes
   * only when the queue array itself changes — minor optimisation
   * for the memoised render path.
   */
  const getItemKey = useCallback(
    (index: number) => queueItems[index]?.id ?? index,
    [queueItems],
  );

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
    measureElement,
    getItemKey,
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
      // `scroll-padding-top: 44px` keeps the focused button visible
      // when Tab focus enters a row whose top edge is under the
      // sticky column header (44px = header's py-2 + text-[10px]
      // line-height + border-b — verified during the #911 sticky
      // header build).
      className="flex-1 overflow-y-auto [scroll-padding-top:44px]"
      role="list"
      aria-label="Download queue items"
    >
      <QueueColumnHeader hasSelection={selectedIds !== undefined} />
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
                isSelected={
                  selectedIds === undefined ? undefined : selectedIds.has(item.id)
                }
                onToggleSelect={onToggleSelect}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
