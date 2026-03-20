// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download statistics panel component.
// Derives session-based stats from current queue items and displays them
// in a collapsible section with colour-coded stat cards. Since there is
// no download history database yet (see #196), all figures are computed
// from the in-memory queue snapshot held in the download store.

import { useMemo } from 'react';

import { useDownloadStore } from '@/stores/downloadStore';
import { SettingsSection } from '@/components/common';
import { SONG_CODEC_LABELS, type SongCodec } from '@/types';

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
    <div className="px-4 pb-2">
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
      </SettingsSection>
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
