// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download statistics panel component.
// Derives session-based stats from current queue items and displays them
// in a collapsible section with colour-coded stat cards. Since there is
// no download history database yet (see #196), all figures are computed
// from the in-memory queue snapshot held in the download store.

import { useEffect, useMemo, useState } from 'react';

import { useDownloadStore } from '@/stores/downloadStore';
import { SettingsSection } from '@/components/common';
import { SONG_CODEC_LABELS, type SongCodec } from '@/types';
import { getLifetimeStats, type LifetimeStats } from '@/lib/tauri-commands';

/**
 * Maps a raw `codec_used` string (e.g., "alac", "aac-legacy") to a
 * human-readable short label for compact display in the stat card.
 * Falls back to uppercasing the raw value if it is not a known codec.
 */
function codecDisplayName(raw: string): string {
  const label = SONG_CODEC_LABELS[raw as SongCodec];
  if (label) {
    // Extract the short portion before the first parenthesis or "Legacy"
    // e.g., "Lossless (ALAC) (Experimental)" -> "ALAC"
    //        "AAC Legacy (256kbps at up to 44.1kHz)" -> "AAC Legacy"
    const parenMatch = label.match(/\(([^)]+)\)/);
    if (parenMatch && parenMatch[1] !== 'Experimental') {
      return parenMatch[1];
    }
    // No useful parenthetical — use the text before the first '('
    const beforeParen = label.split('(')[0].trim();
    return beforeParen || raw.toUpperCase();
  }
  return raw.toUpperCase();
}

/**
 * StatisticsPanel -- Collapsible panel showing session download statistics.
 *
 * Derives all numbers from `useDownloadStore().queueItems` via `useMemo`,
 * so no backend calls are needed. The panel re-renders automatically when
 * queue state changes (new items, completions, errors, etc.).
 *
 * Layout (two rows of stat cards):
 *   Row 1: Total downloads | Success rate | Top codec
 *   Row 2: Active | Queued | Completed | Failed
 */
export function StatisticsPanel() {
  const queueItems = useDownloadStore((s) => s.queueItems);

  /**
   * Compute all statistics from the current queue snapshot.
   * Wrapped in useMemo so we only recompute when queueItems changes.
   */
  const stats = useMemo(() => {
    const total = queueItems.length;
    const active = queueItems.filter(
      (i) => i.state === 'downloading' || i.state === 'processing'
    ).length;
    const queued = queueItems.filter((i) => i.state === 'queued').length;
    const completed = queueItems.filter((i) => i.state === 'complete').length;
    const failed = queueItems.filter((i) => i.state === 'error').length;

    // Success rate: completed / (completed + failed). Avoid division by zero.
    const terminal = completed + failed;
    const successRate = terminal > 0 ? Math.round((completed / terminal) * 100) : null;

    // Most common codec: tally codec_used across all items that have one.
    const codecCounts = new Map<string, number>();
    for (const item of queueItems) {
      if (item.codec_used) {
        codecCounts.set(
          item.codec_used,
          (codecCounts.get(item.codec_used) ?? 0) + 1
        );
      }
    }
    let topCodec: string | null = null;
    let topCodecCount = 0;
    for (const [codec, count] of codecCounts) {
      if (count > topCodecCount) {
        topCodecCount = count;
        topCodec = codec;
      }
    }

    return { total, active, queued, completed, failed, successRate, topCodec };
  }, [queueItems]);

  // Do not render the panel at all if there are no queue items.
  if (stats.total === 0) return null;

  return (
    <div className="px-4 pb-2 space-y-2">
      <SettingsSection title="Session Statistics" defaultOpen={false}>
        {/* Row 1: Summary cards */}
        <div className="grid grid-cols-3 gap-3">
          <StatCard
            value={String(stats.total)}
            label="Total"
            colour="text-content-primary"
          />
          <StatCard
            value={stats.successRate !== null ? `${stats.successRate}%` : '--'}
            label="Success"
            colour={
              stats.successRate === null
                ? 'text-content-tertiary'
                : stats.successRate >= 90
                  ? 'text-status-success'
                  : stats.successRate >= 50
                    ? 'text-status-warning'
                    : 'text-status-error'
            }
          />
          <StatCard
            value={stats.topCodec ? codecDisplayName(stats.topCodec) : '--'}
            label="Top Codec"
            colour="text-accent-primary"
          />
        </div>

        {/* Row 2: Per-state breakdown */}
        <div className="grid grid-cols-4 gap-3">
          <StatCard
            value={String(stats.active)}
            label="Active"
            colour="text-status-info"
          />
          <StatCard
            value={String(stats.queued)}
            label="Queued"
            colour="text-content-secondary"
          />
          <StatCard
            value={String(stats.completed)}
            label="Done"
            colour="text-status-success"
          />
          <StatCard
            value={String(stats.failed)}
            label="Failed"
            colour="text-status-error"
          />
        </div>

        {/* Download speed sparkline (last 60 samples) */}
        <SpeedSparkline />
      </SettingsSection>

      {/* Lifetime stats (#464) — aggregated from persistent history. */}
      <LifetimeStatsSection />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Lifetime stats section (#464)
// ---------------------------------------------------------------------------

/**
 * Renders aggregated lifetime download stats fetched from the backend
 * via `getLifetimeStats()`. Computed on each open (panel uses
 * `defaultOpen={false}`, so this only fetches when the user expands
 * it — but we fetch unconditionally on mount to drive the
 * "X completed" badge in the header without needing a separate ping).
 *
 * Refreshes whenever `queueItems.length` changes (a new download
 * landing usually means the history file has just been updated).
 *
 * Hidden when there's no persistent history yet (`total === 0`) so
 * fresh installs don't see an empty grid of dashes.
 */
function LifetimeStatsSection() {
  const queueLen = useDownloadStore((s) => s.queueItems.length);
  const [stats, setStats] = useState<LifetimeStats | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getLifetimeStats()
      .then((s) => {
        if (!cancelled) {
          setStats(s);
          setError(null);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e));
        }
      });
    return () => {
      cancelled = true;
    };
    // Re-fetch when the queue length changes (signals that a new
    // download likely just landed in history).
  }, [queueLen]);

  if (error) {
    return (
      <SettingsSection title="Lifetime Statistics" defaultOpen={false}>
        <p className="text-xs text-status-error">{error}</p>
      </SettingsSection>
    );
  }
  if (!stats || stats.total === 0) {
    return null;
  }

  const lastDay = stats.last_7_days[stats.last_7_days.length - 1];
  return (
    <SettingsSection
      title={`Lifetime Statistics (${stats.total})`}
      defaultOpen={false}
    >
      <div className="grid grid-cols-3 gap-3">
        <StatCard
          value={String(stats.total)}
          label="All-time downloads"
          colour="text-content-primary"
        />
        <StatCard
          value={`${Math.round(stats.success_rate)}%`}
          label="Success rate"
          colour={
            stats.success_rate >= 90
              ? 'text-status-success'
              : stats.success_rate >= 50
                ? 'text-status-warning'
                : 'text-status-error'
          }
        />
        <StatCard
          value={lastDay ? String(lastDay.count) : '0'}
          label="Today"
          colour="text-accent-primary"
        />
      </div>

      {/* Codec distribution — bar list, hidden when empty. */}
      {stats.codec_distribution.length > 0 && (
        <div className="mt-3">
          <h4 className="text-xs font-medium text-content-secondary mb-1.5">
            By codec
          </h4>
          <div className="space-y-1">
            {stats.codec_distribution.slice(0, 6).map((c) => {
              const pct = (c.count / stats.success) * 100;
              return (
                <div key={c.codec} className="flex items-center gap-2 text-xs">
                  <span className="w-24 truncate text-content-secondary">
                    {codecDisplayName(c.codec)}
                  </span>
                  <div className="flex-1 h-2 rounded-full bg-surface-secondary/60 overflow-hidden">
                    <div
                      className="h-full bg-accent rounded-full"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                  <span className="w-10 text-right text-content-tertiary tabular-nums">
                    {c.count}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Top artist + album + window — single-line summary. */}
      <div className="mt-3 text-xs text-content-tertiary space-y-0.5">
        {stats.top_artist && (
          <p>
            <span className="text-content-secondary">Top artist:</span> {stats.top_artist.name} ({stats.top_artist.count})
          </p>
        )}
        {stats.top_album && (
          <p>
            <span className="text-content-secondary">Top album:</span> {stats.top_album.name} ({stats.top_album.count})
          </p>
        )}
        <p>
          <span className="text-content-secondary">Last 7 days:</span>{' '}
          {stats.last_7_days.reduce((sum, d) => sum + d.count, 0)} downloads
        </p>
      </div>
    </SettingsSection>
  );
}

// ---------------------------------------------------------------------------
// Speed sparkline sub-component
// ---------------------------------------------------------------------------

/** Renders an SVG sparkline of recent download speeds. */
function SpeedSparkline() {
  const speedHistory = useDownloadStore((s) => s.speedHistory);
  if (speedHistory.length < 2) return null;

  const width = 280;
  const height = 40;
  const max = Math.max(...speedHistory, 0.1); // Avoid division by zero
  const stepX = width / (speedHistory.length - 1);

  const points = speedHistory
    .map((v, i) => `${i * stepX},${height - (v / max) * (height - 4)}`)
    .join(' ');

  const latest = speedHistory[speedHistory.length - 1];
  const label = latest >= 1 ? `${latest.toFixed(1)} MB/s` : `${(latest * 1024).toFixed(0)} KB/s`;

  return (
    <div className="flex items-center gap-2 mt-2">
      <svg width={width} height={height} className="flex-shrink-0" aria-label="Download speed history">
        <polyline
          points={points}
          fill="none"
          stroke="var(--color-accent)"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          opacity="0.7"
        />
      </svg>
      <span className="text-xs text-content-tertiary whitespace-nowrap">{label}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Internal stat card sub-component
// ---------------------------------------------------------------------------

interface StatCardProps {
  /** The numeric or text value displayed prominently. */
  value: string;
  /** Short label rendered below the value. */
  label: string;
  /** Tailwind text colour class for the value. */
  colour: string;
}

/**
 * StatCard -- A compact card showing a single stat with a value and label.
 *
 * Renders a centred value in a large, semi-bold font above a smaller muted
 * label. The value colour is configurable to provide at-a-glance status cues.
 */
function StatCard({ value, label, colour }: StatCardProps) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg bg-surface-secondary/60 px-2 py-3 min-w-0">
      <span className={`text-lg font-semibold leading-tight truncate max-w-full ${colour}`}>
        {value}
      </span>
      <span className="text-xs text-content-tertiary mt-0.5">{label}</span>
    </div>
  );
}
