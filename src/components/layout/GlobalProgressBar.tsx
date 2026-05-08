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
import { formatActiveItemCaption } from '@/lib/progress-caption';

/**
 * Platform detection config — loaded once from engines.toml via IPC.
 * Each entry has URL patterns, an icon path, and a display name.
 * Populated by loadPlatformConfig() on first use; empty until then.
 */
interface PlatformEntry {
  id: string;
  name: string;
  icon: string;
  patterns: string[];
}

let platformConfig: PlatformEntry[] = [];
let configLoaded = false;
/** Subscribers notified when platformConfig loads, so components re-render. */
let configSubscribers: Array<() => void> = [];

/**
 * Loads platform config from engines.toml via IPC (one-time).
 * Called lazily on first render of GlobalProgressBar.
 * Notifies subscribers when the config is ready so React components re-render.
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
        patterns: p.url_patterns,
      }));
    // Notify all subscribers that config is ready
    for (const cb of configSubscribers) cb();
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
 * Falls back to a local <img> tag if the inline SVG can't be loaded.
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

  // Fallback: render the local icon as an <img> (no currentColor theming,
  // but works under img-src 'self' without external network requests).
  if (useFallback && platform.icon) {
    return (
      <img
        src={platform.icon}
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

  // Load platform config from engines.toml via IPC (one-time).
  // Uses a subscriber pattern so the component re-renders when the
  // async IPC resolves — without this, the module-level platformConfig
  // would be empty on first render and the icon would never appear.
  const [, setConfigReady] = useState(false);
  useEffect(() => {
    const cb = () => setConfigReady(true);
    configSubscribers.push(cb);
    loadPlatformConfig();
    // If config already loaded (e.g., hot reload), trigger immediately
    if (platformConfig.length > 0) setConfigReady(true);
    return () => {
      configSubscribers = configSubscribers.filter((s) => s !== cb);
    };
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
   * Per-item progress value.
   * During processing: use actual progress if available (companion downloads
   * update progress while item stays in 'processing' state), otherwise null
   * for indeterminate animation (enrichment stages without progress data).
   */
  const itemProgress =
    activeItem?.state === 'processing'
      ? (activeItem?.speed ? (activeItem?.progress ?? null) : null)
      : (activeItem?.progress ?? 0);

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
