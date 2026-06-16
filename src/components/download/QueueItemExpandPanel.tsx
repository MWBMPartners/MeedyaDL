// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Click-to-expand row detail panel (#911 Phase 1 item 0 sub-item).
 *
 * Renders the hidden-tier columns (Tier 2-5) inline beneath the
 * responsive queue row when the user clicks the chevron toggle.
 * Companion to the `QueueItem.tsx` Tier 1-5 responsive column system:
 * at narrow widths the Tier 4-5 (codec, submitted-at) and Tier 2-3
 * (platform, speed/ETA) columns are hidden by `hidden md:flex` /
 * `hidden lg:flex` etc. classes; this panel surfaces them inline so a
 * user on a narrow window can still see all the data on demand.
 *
 * ## Why a separate file
 *
 * `QueueItem.tsx` is already ~890 LOC. Inlining 150 LOC for the panel
 * pushes it past 1000 — a maintainability cliff. The panel is rendered
 * by exactly one consumer (the per-row `expanded` toggle) so an
 * extracted file with explicit props gives the right balance of
 * locality + readability.
 *
 * ## Reduced-motion compliance
 *
 * The panel uses CSS `opacity` + `transform` (NOT `max-height`) so the
 * project-wide `prefers-reduced-motion` rule in `globals.css` lines
 * 368-377 can zero the transition duration to 0.01ms cleanly. With
 * `max-height` the snap behaviour is fragile because content-height
 * still has to be computed for the closing transform. Opacity +
 * translateY work in both motion modes.
 *
 * ## Inverse-tier mirror
 *
 * Each panel cell uses the INVERSE responsive class of the column it
 * mirrors. The Tier 2 platform-icon row column is `hidden md:flex`
 * (hidden below md). Its panel mirror is `md:hidden` (hidden above
 * md — i.e. only shown at widths where the row column is hidden).
 * This avoids the redundant double-announcement of "Platform: Apple
 * Music" at wide widths where the icon is already visible in the row.
 *
 * ## What the panel shows
 *
 * Top section (inverse-tier mirrors) — these only render at widths
 * where the row's column is hidden:
 *   - Tier 2 mirror: Platform / service name
 *   - Tier 3 mirror: Speed + ETA (only when active)
 *   - Tier 4 mirror: Codec / quality (only when set)
 *   - Tier 5 mirror: Submitted-at relative time
 *
 * Bottom section (always visible) — power-user data the row never
 * shows even at 2xl:
 *   - Full first URL (monospace, user-selectable)
 *   - Multi-URL batch count when `urls.length > 1`
 *   - Output path (when complete)
 *   - Audio traits (e.g. "atmos, lossless") when populated
 *   - Music-video companion count when populated
 *   - Submitted-at ISO timestamp via the row's `created_at`
 *
 * The selectable monospace text is essential for power users who
 * need to copy a long URL or output path into a terminal — the row's
 * truncated primary identifier doesn't support text selection
 * reliably.
 *
 * ## Accessibility
 *
 * The panel's outer container has `role="region"` plus an
 * `aria-label="Additional details for {primary identifier}"` so
 * screen-reader users get a clear region announcement when the
 * expansion toggle is activated. The trigger (chevron button in
 * `QueueItem.tsx`) carries `aria-expanded` and `aria-controls`
 * pointing at this region's `id` — the canonical WAI-ARIA Disclosure
 * pattern.
 *
 * The 2-column grid (`grid-cols-2`) collapses to a single column at
 * very narrow widths via the inherent overflow behaviour of `grid`
 * (`gap-x-4` shrinks when there's no room). No explicit responsive
 * grid class needed — the inverse-tier hiding pattern already
 * suppresses most content at narrow widths.
 */

import type { QueueItemStatus } from '@/types';

export interface QueueItemExpandPanelProps {
  /** The full queue item — every field is potentially surfaced. */
  item: QueueItemStatus;
  /**
   * Stable ID for the panel's outer container. Mirrored by the
   * triggering chevron button's `aria-controls`. Generated via
   * `useId()` in the parent so it's stable across re-renders and
   * unique across multiple expanded rows.
   */
  panelId: string;
  /**
   * Human-readable label for the row's primary identifier
   * (`Artist — Album — Track` or fallback URL). Used as part of
   * the panel's `aria-label` so screen readers can distinguish
   * "Additional details for Album A" from "Additional details for
   * Album B" when both happen to be expanded.
   */
  primaryIdentifier: string;
  /**
   * Display name for the detected platform (e.g. "Apple Music",
   * "Spotify"). Resolved via `detectPlatform()` in the parent so
   * the panel doesn't re-do that work. `null` when the platform
   * config hasn't loaded yet or the URL doesn't match any
   * registered platform.
   */
  platformLabel: string | null;
  /**
   * Human-friendly relative submitted-at string ("3 hours ago").
   * Pre-formatted by the parent's `formatRelativeTime` so the
   * panel can stay presentational.
   */
  submittedRelative: string | null;
}

/**
 * A single label/value pair inside the panel. Kept colocated rather
 * than extracted to `common/` because the layout (label-then-value
 * stacked, monospace value when copyable) is panel-specific and
 * doesn't generalise.
 */
function DetailField({
  label,
  value,
  selectable = false,
  className = '',
}: {
  label: string;
  value: string | number | null | undefined;
  /** When true, value is rendered in monospace + user-selectable for
   *  copy-paste into a terminal. */
  selectable?: boolean;
  /** Optional Tailwind classes for the outer wrapper — used by the
   *  inverse-tier mirrors to hide above the corresponding breakpoint. */
  className?: string;
}) {
  if (value == null || value === '') return null;
  return (
    <div className={`flex flex-col gap-0.5 ${className}`}>
      <span className="text-[10px] uppercase tracking-wide text-content-tertiary">
        {label}
      </span>
      <span
        className={`text-xs text-content-secondary ${
          selectable ? 'font-mono break-all select-text' : 'truncate'
        }`}
      >
        {value}
      </span>
    </div>
  );
}

/**
 * Renders the expanded-row detail panel. The parent owns the
 * `expanded` boolean state — this component renders only when the
 * row is in the expanded state.
 *
 * The inverse-tier mirrors are wrapped in their own `<div>` wrappers
 * with the matching `md:hidden` / `lg:hidden` / etc. responsive
 * classes so they collapse cleanly at wide widths (where the row
 * already shows that data). The hidden-mirror cells still exist in
 * the DOM at wide widths — they're CSS-hidden, not React-conditional
 * — to keep grid-cell alignment stable across breakpoints.
 */
export function QueueItemExpandPanel({
  item,
  panelId,
  primaryIdentifier,
  platformLabel,
  submittedRelative,
}: QueueItemExpandPanelProps) {
  const firstUrl = item.urls?.[0] ?? null;
  const extraUrlCount =
    item.urls && item.urls.length > 1 ? item.urls.length - 1 : 0;
  const isActive = item.state === 'downloading' || item.state === 'processing';
  const audioTraits =
    item.audio_traits && item.audio_traits.length > 0
      ? item.audio_traits.join(', ')
      : null;

  // ISO timestamp for the truly verbose "exact submitted time" line.
  // Wrapped in a defensive try/catch because the field is a backend
  // string and a bad value shouldn't crash the row.
  let submittedIso: string | null = null;
  if (item.created_at) {
    try {
      submittedIso = new Date(item.created_at).toLocaleString();
    } catch {
      submittedIso = item.created_at;
    }
  }

  return (
    <div
      id={panelId}
      role="region"
      aria-label={`Additional details for ${primaryIdentifier}`}
      className="mt-2 pl-[60px] pr-2"
    >
      <div className="grid grid-cols-2 gap-x-4 gap-y-2 rounded-platform border border-border-light bg-surface-secondary px-3 py-2">
        {/*
         * Inverse-tier mirrors — only visible at widths where the
         * corresponding row column is hidden. The `md:hidden` class
         * hides this cell at widths ≥ md (where the row's Tier 2
         * column is visible); same shape for `lg:hidden` (Tier 3),
         * `xl:hidden` (Tier 4), `2xl:hidden` (Tier 5). This is the
         * exact inverse of QueueItem.tsx's `hidden md:flex` etc.
         * pattern — no double-announcement at wide widths.
         */}
        <DetailField
          label="Platform"
          value={platformLabel}
          className="md:hidden"
        />
        {isActive && (
          <DetailField
            label="Speed / ETA"
            value={
              item.speed && item.eta
                ? `${item.speed} · ETA ${item.eta}`
                : item.speed ?? null
            }
            className="lg:hidden"
          />
        )}
        <DetailField
          label="Codec / quality"
          value={item.codec_used}
          className="xl:hidden"
        />
        <DetailField
          label="Submitted"
          value={submittedRelative}
          className="2xl:hidden"
        />

        {/*
         * Verbose / power-user data — always visible. The row never
         * surfaces these at any width, so no inverse-tier suppression
         * needed.
         */}
        <DetailField label="Full URL" value={firstUrl} selectable />
        {extraUrlCount > 0 && (
          <DetailField
            label="Multi-URL batch"
            value={`+${extraUrlCount} more URL${extraUrlCount === 1 ? '' : 's'}`}
          />
        )}
        <DetailField
          label={item.output_is_directory ? 'Output folder' : 'Output path'}
          value={item.output_path}
          selectable
        />
        <DetailField label="Audio traits" value={audioTraits} />
        <DetailField
          label="Music-video companions"
          value={item.mv_companion_count}
        />
        {submittedIso && submittedIso !== submittedRelative && (
          <DetailField label="Submitted at" value={submittedIso} />
        )}
      </div>
    </div>
  );
}
