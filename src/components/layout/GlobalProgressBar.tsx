// Copyright (c) 2026 MeedyaDL
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

import { useMemo } from 'react';

import { useDownloadStore } from '@/stores/downloadStore';

/**
 * Detects the download platform from the first URL of a queue item.
 * Returns a platform key used to select the correct icon.
 * Extensible for future services (Spotify, YouTube, BBC iPlayer).
 */
function detectPlatform(urls?: string[]): 'apple-music' | 'unknown' {
  const raw = urls?.[0] ?? '';
  try {
    const { hostname } = new URL(raw);
    if (
      hostname === 'music.apple.com' ||
      hostname === 'classical.apple.com' ||
      hostname === 'itunes.apple.com'
    ) {
      return 'apple-music';
    }
  } catch {
    // Malformed URL — fall through to unknown
  }
  return 'unknown';
}

/**
 * Inline SVG icon for Apple Music (music note).
 * 12x12px to match the 10px text size of the progress bar labels.
 */
function AppleMusicIcon() {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className="flex-shrink-0"
      aria-label="Apple Music"
    >
      <path
        d="M19.5 3.5L8.5 6v11a3 3 0 1 1-2-2.83V5l11-2.5v10.5a3 3 0 1 1-2-2.83V3.5Z"
        fill="currentColor"
        fillOpacity="0.5"
      />
    </svg>
  );
}

/** Platform icon lookup — extensible for future services. */
const PLATFORM_ICONS: Record<string, (() => React.JSX.Element) | undefined> = {
  'apple-music': AppleMusicIcon,
};

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
      // Count items that are done (complete, error, or cancelled)
      const completed = queueItems.filter(
        (i) => i.state === 'complete' || i.state === 'error' || i.state === 'cancelled'
      ).length;
      const progress = total > 0 ? Math.round((completed / total) * 100) : 0;
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

  /** Per-item progress value: null = indeterminate (processing state) */
  const itemProgress =
    activeItem?.state === 'processing' ? null : (activeItem?.progress ?? 0);

  /** Label for the per-item bar */
  const itemLabel = activeItem?.current_track ?? activeItem?.urls?.[0] ?? '';

  /** Speed and ETA suffix */
  const speedEta = [activeItem?.speed, activeItem?.eta]
    .filter(Boolean)
    .join(' · ');

  /** Platform detection for the icon */
  const platform = detectPlatform(activeItem?.urls);
  const PlatformIcon = PLATFORM_ICONS[platform];

  return (
    <div
      className="flex-shrink-0 border-t border-border-light bg-surface-secondary px-4 py-1.5"
      role="region"
      aria-label="Download progress"
    >
      {/* Upper bar: per-item progress */}
      <div className="flex items-center gap-2 mb-1">
        {/* Platform icon + track info (left) */}
        {PlatformIcon && <PlatformIcon />}
        <span className="text-[12px] text-content-secondary truncate min-w-0 flex-1">
          {activeItem ? itemLabel : 'Waiting…'}
        </span>
        {/* Speed + ETA + percentage (right) */}
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
          /* Indeterminate (processing) */
          <div
            className="h-full w-1/3 rounded-full bg-accent animate-[indeterminate_1.5s_ease-in-out_infinite]"
            style={{ animation: 'indeterminate 1.5s ease-in-out infinite' }}
          />
        ) : (
          /* Determinate */
          <div
            className="h-full rounded-full bg-accent transition-all duration-300 ease-out"
            style={{ width: `${Math.max(0, Math.min(100, itemProgress))}%` }}
          />
        )}
      </div>

      {/* Lower bar: queue-level progress */}
      <div className="flex items-center gap-2 mb-0.5">
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
