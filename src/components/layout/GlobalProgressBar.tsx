// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Global progress bar component.
 *
 * Renders two thin progress bars at the bottom of the main content area,
 * visible on ALL pages (not just the Queue page). This lets users monitor
 * download progress while configuring settings, reading help, etc.
 *
 * Layout (stacked vertically, each 4px tall):
 *   Upper bar: per-item progress (the currently downloading item)
 *   Lower bar: queue-level progress (completed / total items)
 *
 * Both bars auto-hide when no downloads are active or queued.
 *
 * @see MainLayout -- parent component that renders this above the StatusBar
 * @see ProgressBar -- the shared progress bar primitive
 * @see useDownloadStore -- source of queue item state
 */

import { useEffect, useMemo, useState } from 'react';

import { useDownloadStore } from '@/stores/downloadStore';
import { formatActiveItemCaption } from '@/lib/progress-caption';
import { computeItemProgress } from '@/lib/progress-percent';
import {
  detectPlatform,
  loadPlatformConfig,
  subscribeToPlatformConfig,
} from '@/lib/platform-config';
import { PlatformIcon } from '@/lib/PlatformIcon';

/**
 * Renders two stacked progress bars that are always visible at the bottom
 * of the window, regardless of which page the user is on.
 *
 * - **Upper bar (per-item)**: Shows the progress of the currently active
 *   download. Displays the track name and speed/ETA when available.
 *   Indeterminate (animated) when in processing state.
 *
 * - **Lower bar (queue-level)**: Shows overall queue completion as a
 *   ratio of completed items to total items.
 *
 * Both bars are hidden when the queue is empty or all items are complete.
 */
export function GlobalProgressBar() {
  const queueItems = useDownloadStore((s) => s.queueItems);

  // Load platform config from engines.toml via IPC (one-time).
  // Uses the subscribe helper so the component re-renders when the
  // async IPC resolves — without this, the platform cache would be
  // empty on first render and the icon would never appear.
  const [, setConfigReady] = useState(false);
  useEffect(() => {
    const unsubscribe = subscribeToPlatformConfig(() => setConfigReady(true));
    loadPlatformConfig();
    return unsubscribe;
  }, []);

  /**
   * Derive the active item (currently downloading/processing) and
   * queue-level stats from the queue items array.
   */
  const { activeItem, queueProgress, totalItems, completedItems, hasWork } =
    useMemo(() => {
      const active = queueItems.find(
        (i) => i.state === 'downloading' || i.state === 'processing'
      );
      const total = queueItems.filter(
        (i) =>
          i.state === 'downloading' ||
          i.state === 'processing' ||
          i.state === 'queued' ||
          i.state === 'complete' ||
          i.state === 'error' ||
          i.state === 'cancelled'
      ).length;
      // Integer completed-count for the numeric "N of M complete" caption.
      // A `processing` item has produced its primary files (audio on disk)
      // but is still mid-enrichment — counted as "done" in the integer
      // display because the user's expectation of "download complete"
      // tracks the audio landing, not every last ReplayGain / AcoustID /
      // manifest-write tick. This preserves the pre-#576 integer caption
      // behaviour.
      const completed = queueItems.filter(
        (i) =>
          i.state === 'complete' ||
          i.state === 'processing' ||
          i.state === 'error' ||
          i.state === 'cancelled'
      ).length;

      // Weighted progress fraction (#576). Unlike the integer caption
      // above, this gives `processing` items credit based on where they
      // are INSIDE the enrichment pipeline, not just binary 0/1. The
      // backend emits `processing_progress` at every enrichment stage
      // start (see ENRICHMENT_STAGE_WEIGHTS in download_queue.rs) so the
      // bar ticks up 0.05 → 0.15 → 0.25 → 0.40 → 0.55 → 0.75 → 1.0 as
      // metadata / lyrics / artwork / AcoustID / ReplayGain land.
      //
      // Without this, large box sets (200-track Beethoven etc.) showed
      // the queue bar pinned at a single partial-credit value for the
      // entire 15–40 min enrichment phase. Progress updates via the
      // `queue-updated` event listener in App.tsx refresh the store,
      // and the derived aggregate here picks up the new fraction on
      // the next render.
      const weightedDone = queueItems.reduce((acc, i) => {
        if (
          i.state === 'complete' ||
          i.state === 'error' ||
          i.state === 'cancelled'
        ) {
          return acc + 1.0;
        }
        if (i.state === 'processing') {
          // processing_progress is null until the first enrichment stage
          // ticks over; default to 0.5 so the bar at least reflects the
          // audio-landed milestone. Matches the pre-#576 flat partial
          // credit but upgrades over time.
          const p = i.processing_progress ?? 0.5;
          // Clamp defensively — a mis-computed weight > 1 would falsely
          // inflate queue progress past its integer denominator.
          return acc + Math.min(Math.max(p, 0), 1);
        }
        return acc;
      }, 0);
      const progress =
        total > 0 ? Math.round((weightedDone / total) * 100) : 0;
      const working =
        total > 0 && (active !== undefined || completed < total);

      return {
        activeItem: active ?? null,
        queueProgress: progress,
        totalItems: total,
        completedItems: completed,
        hasWork: working,
      };
    }, [queueItems]);

  /* Hide entirely when nothing is happening */
  if (!hasWork) return null;

  /**
   * Per-item progress value — see `computeItemProgress` for the
   * aggregation rule (#790). For multi-track items the value now
   * spans the WHOLE item (completed_tracks + current_track%), not
   * just the current track's GAMDL `[download] X%` — so the bar
   * ticks monotonically forward instead of "scrolling" left-to-
   * right as each track resets to 0.
   */
  const itemProgress = computeItemProgress(activeItem ?? null);

  /** Per-item caption — see `formatActiveItemCaption` for rules. */
  const itemLabel = formatActiveItemCaption(activeItem);

  /** Speed and ETA suffix */
  const speedEta = [activeItem?.speed, activeItem?.eta]
    .filter(Boolean)
    .join(' · ');

  /** Platform detection for the icon */
  const platform = detectPlatform(activeItem?.urls);

  return (
    <div
      className="flex-shrink-0 border-t border-border-light bg-surface-secondary px-4 py-1.5"
      role="region"
      aria-label="Download progress"
    >
      {/* Upper bar: per-item progress */}
      <div className="flex items-center gap-2 mb-1">
        {/* Platform icon + track info (left) */}
        <PlatformIcon platform={platform} />
        <span className="text-[12px] text-content-secondary truncate min-w-0 flex-1">
          {activeItem ? itemLabel : 'Waiting…'}
        </span>
        {/* Speed + ETA + percentage (right).
            #790: `computeItemProgress` now exhausts every signal
            (active byte stream → enrichment stage weight) before
            falling back to `null`, so the "Processing…" placeholder
            only fires in the brief gaps where no signal exists at
            all. When it does fire, that's the honest answer — we
            don't fake a percentage we can't measure. */}
        <span className="text-[12px] text-content-tertiary whitespace-nowrap flex-shrink-0">
          {speedEta ? `${speedEta} · ` : ''}
          {itemProgress !== null ? `${Math.round(itemProgress)}%` : 'Processing…'}
        </span>
      </div>
      <div
        className="h-1.5 w-full rounded-full bg-surface-elevated overflow-hidden mb-1.5"
        role="progressbar"
        aria-valuenow={itemProgress ?? undefined}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label="Current download progress"
      >
        {itemProgress === null ? (
          /* Indeterminate — LAST resort per #790. Only fires when
             we have neither an active GAMDL byte stream nor an
             enrichment stage weight. Should be brief; persistent
             indeterminate state would indicate a missing emit at
             the source, not a problem in this render. */
          <div
            className="h-full w-1/3 rounded-full bg-accent animate-[indeterminate_1.5s_ease-in-out_infinite]"
            style={{ animation: 'indeterminate 1.5s ease-in-out infinite' }}
          />
        ) : (
          /* Determinate — per-file byte progress (GAMDL active),
             enrichment stage weight, or terminal-state value.
             Each file's bar cycles 0 → 100 as expected; the
             caption changes per file so the user can see which
             file is in flight. */
          <div
            className="h-full rounded-full bg-accent transition-all duration-300 ease-out"
            style={{ width: `${itemProgress}%` }}
          />
        )}
      </div>

      {/* Lower bar: queue-level progress */}
      <div className="flex items-center gap-2 mb-0.5">
        <PlatformIcon platform={platform} />
        <span className="text-[12px] text-content-tertiary truncate min-w-0 flex-1">
          {completedItems} of {totalItems} complete
        </span>
        <span className="text-[12px] text-content-tertiary whitespace-nowrap flex-shrink-0">
          {queueProgress}%
        </span>
      </div>
      <div
        className="h-1.5 w-full rounded-full bg-surface-elevated overflow-hidden"
        role="progressbar"
        aria-valuenow={queueProgress}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label="Overall queue progress"
      >
        <div
          className="h-full rounded-full bg-status-success transition-all duration-300 ease-out"
          style={{ width: `${queueProgress}%` }}
        />
      </div>
    </div>
  );
}
