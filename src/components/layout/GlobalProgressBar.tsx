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

import { useMemo, useState, useEffect } from 'react';

import { useDownloadStore } from '@/stores/downloadStore';

/**
 * Platform detection config — loaded once from engines.toml via IPC.
 * Each entry has URL patterns, an icon path, and a display name.
 * Populated by loadPlatformConfig() on first use; empty until then.
 */
interface PlatformEntry {
  id: string;
  name: string;
  icon: string;
  faviconHost: string;
  patterns: string[];
}

let platformConfig: PlatformEntry[] = [];
let configLoaded = false;

/**
 * Loads platform config from engines.toml via IPC (one-time).
 * Called lazily on first render of GlobalProgressBar.
 */
async function loadPlatformConfig() {
  if (configLoaded) return;
  configLoaded = true;
  try {
    const { getPlatformConfig } = await import('@/lib/tauri-commands');
    const platforms = await getPlatformConfig();
    platformConfig = platforms
      .filter((p) => p.enabled)
      .map((p) => ({
        id: p.id,
        name: p.name,
        icon: p.icon ? `/${p.icon}` : '',
        faviconHost: p.url_patterns[0]?.replace(/\/.*$/, '') ?? '',
        patterns: p.url_patterns,
      }));
  } catch {
    // IPC not ready yet — will retry on next render
    configLoaded = false;
  }
}

/**
 * Detects the download platform from the first URL of a queue item.
 * Matches against URL patterns from engines.toml (loaded via IPC).
 */
function detectPlatform(urls?: string[]): PlatformEntry | undefined {
  const raw = urls?.[0] ?? '';
  try {
    const parsed = new URL(raw);
    const urlStr = parsed.hostname + parsed.pathname;
    return platformConfig.find((p) =>
      p.patterns.some((pattern) => urlStr.includes(pattern))
    );
  } catch {
    return undefined;
  }
}

/**
 * Renders a platform icon for the progress bar. Fetches the local SVG and
 * renders it inline so `currentColor` inherits from the parent CSS context,
 * automatically adapting to light, dark, and colour-blind themes.
 *
 * Falls back to Google Favicon API (PNG) if the local SVG can't be loaded.
 * SVG content is cached in a module-level Map to avoid re-fetching.
 */
const svgCache = new Map<string, string>();

function PlatformIcon({ platform }: { platform: PlatformEntry | undefined }) {
  const [svgHtml, setSvgHtml] = useState<string | null>(
    () => (platform?.icon ? svgCache.get(platform.icon) ?? null : null)
  );
  const [useFallback, setUseFallback] = useState(false);

  useEffect(() => {
    if (!platform || !platform.icon) {
      if (platform) setUseFallback(true);
      return;
    }
    if (svgCache.has(platform.icon)) {
      setSvgHtml(svgCache.get(platform.icon)!);
      return;
    }
    // Ensure absolute path for Tauri production (base URL is tauri://localhost/)
    const iconUrl = platform.icon.startsWith('/') ? platform.icon : `/${platform.icon}`;
    fetch(iconUrl)
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.text();
      })
      .then((text) => {
        // Only accept SVG content (not HTML error pages)
        if (text.includes('<svg')) {
          // Defence-in-depth: parse the SVG via DOMParser and strip
          // <script> elements + event handler attributes before inline
          // rendering. Tauri's CSP already blocks inline scripts, but
          // this prevents any bypass via onload/onerror/xlink:href.
          const parser = new DOMParser();
          const doc = parser.parseFromString(text, 'image/svg+xml');
          const svgEl = doc.querySelector('svg');
          if (!svgEl) {
            setUseFallback(true);
            return;
          }
          // Remove script elements and event handler attributes
          doc.querySelectorAll('script').forEach((s) => s.remove());
          doc.querySelectorAll('*').forEach((el) => {
            for (const attr of [...el.attributes]) {
              if (attr.name.startsWith('on')) {
                el.removeAttribute(attr.name);
              }
            }
          });
          // Inject sizing attributes for container fill
          svgEl.setAttribute('width', '100%');
          svgEl.setAttribute('height', '100%');
          svgEl.setAttribute('style', 'display:block');
          const sanitized = svgEl.outerHTML;
          svgCache.set(platform.icon, sanitized);
          setSvgHtml(sanitized);
        } else {
          setUseFallback(true);
        }
      })
      .catch(() => setUseFallback(true));
  }, [platform]);

  if (!platform) return null;

  // Inline SVG: inherits currentColor from parent for theme adaptability.
  // The SVG has no fixed width/height — it expands to fill the container.
  if (svgHtml) {
    return (
      <span
        className="flex-shrink-0 inline-flex items-center justify-center text-content-secondary [&>svg]:w-full [&>svg]:h-full"
        style={{ width: 16, height: 16 }}
        aria-label={platform.name}
        dangerouslySetInnerHTML={{ __html: svgHtml }}
      />
    );
  }

  // Fallback: Google Favicon API (PNG, doesn't adapt to theme but works)
  if (useFallback) {
    return (
      <img
        src={`https://www.google.com/s2/favicons?domain=${platform.faviconHost}&sz=32`}
        alt={platform.name}
        width={16}
        height={16}
        className="flex-shrink-0"
      />
    );
  }

  return null; // Loading
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

  // Load platform config from engines.toml via IPC (one-time)
  useEffect(() => {
    loadPlatformConfig();
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
      // Count items that are done for queue-level progress.
      // 'processing' counts as done — the user's files are downloaded,
      // enrichment/companions are background bonus processing.
      const completed = queueItems.filter(
        (i) =>
          i.state === 'complete' ||
          i.state === 'processing' ||
          i.state === 'error' ||
          i.state === 'cancelled'
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

  /**
   * Per-item progress value.
   * During processing: use actual progress if available (companion downloads
   * update progress while item stays in 'processing' state), otherwise null
   * for indeterminate animation (enrichment stages without progress data).
   */
  const itemProgress =
    activeItem?.state === 'processing'
      ? (activeItem?.speed ? (activeItem?.progress ?? null) : null)
      : (activeItem?.progress ?? 0);

  /** Build contextual label: "Artist — Album — "Track"" for multi-queue clarity */
  const itemLabel = (() => {
    // Processing labels (enrichment/companions) take priority
    if (activeItem?.processing_label) return activeItem.processing_label;

    const track = activeItem?.current_track;
    const album = activeItem?.album_name;
    const artist = activeItem?.artist_name;

    if (track) {
      const parts: string[] = [];
      if (artist) parts.push(artist);
      if (album) parts.push(album);
      parts.push(`"${track}"`);
      return parts.join(' — ');
    }

    return album ?? activeItem?.urls?.[0] ?? '';
  })();

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
