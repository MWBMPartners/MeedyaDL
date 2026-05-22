// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Per-item progress-bar percentage computation (#790).
 *
 * Extracted from `GlobalProgressBar.tsx` so the multi-signal
 * resolution rule can be unit-tested in isolation without
 * rendering the full DOM. Sibling to `progress-caption.ts` which
 * does the analogous extraction for the caption text.
 *
 * ## Why this exists
 *
 * Pre-fix the bar fell through to a CSS-keyframe animated
 * indeterminate stripe (sliding left-to-right) whenever the
 * `progress` field was unset — which fired routinely between
 * enrichment stages and during companion-supervisor handoffs.
 * User feedback (2026-05-17): the indeterminate animation
 * should ONLY appear when it's genuinely impossible to derive a
 * percentage — try every other signal first.
 *
 * ## What stays per-file
 *
 * For multi-file downloads (album with many tracks, multi-codec
 * companion tiers), the bar SHOULD show the current file's
 * byte-level progress (cycles 0 → 100 per file) — this is what
 * the user explicitly asked for in the clarification: "we should
 * update the progress bar label for each so that we can also
 * accurately represent progress for each individual item
 * progress." The label (handled by `formatActiveItemCaption`)
 * tells you WHICH file is in flight; the bar tells you HOW MUCH
 * of THAT file is done. Resetting between files is the correct
 * behaviour, not a bug.
 *
 * ## Resolution order
 *
 *   1. **Active GAMDL stream** (`speed` is set, which means the
 *      subprocess is actively reporting `[download] X%`): use
 *      the raw `progress` — per-file byte percentage, the most
 *      accurate signal we have.
 *
 *   2. **Enrichment stage weight set** (`processing_progress`):
 *      scale that 0-1 fraction to 0-100. Each stage transition
 *      moves the bar forward.
 *
 *   3. **Non-active states** (queued / complete / error): the
 *      backend-provided `progress` value (typically 0 or 100).
 *
 *   4. **Last resort** — no signal at all: return `null`. The
 *      caller renders an indeterminate animation. Only fires
 *      when the item is in `processing` state with neither
 *      `speed` nor `processing_progress` set (e.g. brief gaps
 *      between enrichment stages before the next one starts
 *      reporting).
 */

import type { QueueItemStatus } from '@/types';

/** Clamp a number into the 0-100 range so out-of-bound backend
 * values can never push the bar visually off-screen. */
function clamp01(percent: number): number {
  if (!Number.isFinite(percent)) return 0;
  return Math.min(100, Math.max(0, percent));
}

/**
 * Compute the per-item progress-bar percentage (0-100), or `null`
 * to request an indeterminate-animation render.
 *
 * `null` is the LAST resort — only when no other signal is
 * available. The previous "always determinate" version of this
 * helper was rolled back per user clarification: per-file
 * accuracy beats a synthesised determinate number that doesn't
 * reflect any real measurement.
 */
export function computeItemProgress(
  item: QueueItemStatus | null | undefined
): number | null {
  if (!item) return null;

  // Path 1: GAMDL is actively reporting bytes for the current
  // file. `speed` is the marker — it gets cleared between files /
  // between stages. When set, `progress` is the most accurate
  // signal we have: per-file byte percentage.
  if (item.speed) {
    return clamp01(item.progress ?? 0);
  }

  // Path 2: no active byte stream, but enrichment has emitted a
  // stage weight. Each stage transition moves the bar forward.
  if (item.processing_progress != null) {
    return clamp01(item.processing_progress * 100);
  }

  // Path 3: non-active states fall through to the backend value
  // (typically 0 for queued, 100 for complete, last-known for
  // error).
  if (item.state !== 'processing' && item.state !== 'downloading') {
    return clamp01(item.progress ?? 0);
  }

  // Path 4: TRULY no signal. Fire the indeterminate animation —
  // the only honest answer when we can't measure. Should be brief
  // (between enrichment stages); if it persists, that's a
  // separate bug to fix at the source (the missing emit), not
  // here.
  return null;
}
