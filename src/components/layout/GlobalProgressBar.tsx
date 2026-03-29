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
 * Platform detection and icon configuration.
 *
 * Each entry maps URL hostnames to a platform ID, display name, and icon source.
 * The icon path points to a local SVG/PNG in public/icons/platforms/. If the file
 * doesn't exist, the component falls back to the service's favicon.
 *
 * To add a new platform: add an entry here with the URL patterns and icon path.
 * This mirrors the platforms section of engines.toml but is frontend-only for
 * performance (no IPC needed for progress bar rendering).
 */
const PLATFORM_CONFIG: {
  id: string;
  name: string;
  icon: string;
  faviconHost: string;
  hostnames: string[];
}[] = [
  {
    id: 'apple-music',
    name: 'Apple Music',
    icon: '/icons/platforms/apple-music.svg',
    faviconHost: 'music.apple.com',
    hostnames: ['music.apple.com', 'classical.apple.com', 'itunes.apple.com'],
  },
  {
    id: 'spotify',
    name: 'Spotify',
    icon: '/icons/platforms/spotify.svg',
    faviconHost: 'open.spotify.com',
    hostnames: ['open.spotify.com'],
  },
  {
    id: 'youtube',
    name: 'YouTube',
    icon: '/icons/platforms/youtube.svg',
    faviconHost: 'youtube.com',
    hostnames: ['youtube.com', 'youtu.be', 'www.youtube.com', 'm.youtube.com'],
  },
  {
    id: 'youtube-music',
    name: 'YouTube Music',
    icon: '/icons/platforms/youtube-music.svg',
    faviconHost: 'music.youtube.com',
    hostnames: ['music.youtube.com'],
  },
  {
    id: 'bbc-iplayer',
    name: 'BBC iPlayer',
    icon: '/icons/platforms/bbc-iplayer.svg',
    faviconHost: 'bbc.co.uk',
    hostnames: ['bbc.co.uk', 'www.bbc.co.uk'],
  },
];

/**
 * Detects the download platform from the first URL of a queue item.
 * Returns the platform config entry, or undefined for unrecognised URLs.
 */
function detectPlatform(urls?: string[]) {
  const raw = urls?.[0] ?? '';
  try {
    const { hostname } = new URL(raw);
    return PLATFORM_CONFIG.find((p) =>
      p.hostnames.some((h) => hostname === h || hostname.endsWith('.' + h))
    );
  } catch {
    return undefined;
  }
}

/**
 * Renders a platform icon for the progress bar. Tries the local SVG/PNG
 * first (from public/icons/platforms/), then falls back to Google's favicon
 * service which returns PNG favicons for any domain.
 */
function PlatformIcon({ platform }: { platform: ReturnType<typeof detectPlatform> }) {
  if (!platform) return null;
  return (
    <img
      src={platform.icon}
      alt={platform.name}
      width={14}
      height={14}
      className="flex-shrink-0"
      onError={(e) => {
        // Fallback: Google favicon service (returns PNG, better than raw ICO)
        const img = e.currentTarget;
        if (!img.dataset.fallback) {
          img.dataset.fallback = '1';
          img.src = `https://www.google.com/s2/favicons?domain=${platform.faviconHost}&sz=32`;
        }
      }}
    />
  );
}

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
