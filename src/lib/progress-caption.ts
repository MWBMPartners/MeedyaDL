// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Per-item progress-bar caption formatter (Phase 3.5f / 3.5g).
 *
 * Extracted from `GlobalProgressBar.tsx` so the React component file
 * exports only its component (Fast Refresh works best when each file
 * has a single component export) and the caption rules can be unit-
 * tested in isolation without rendering the full DOM.
 */

import type { QueueItemStatus } from '@/types';

/**
 * Builds the per-item progress-bar caption from a queue item's state.
 *
 * Three resolution paths in priority order:
 *
 *   1. **`processing_label`** wins outright. The backend pre-assembles
 *      this as `{stage label} — {Artist}: {Album}` via
 *      `progress_stages::set_stage_with_label` so the format and
 *      Artist/Album suffix are already correct; the frontend just
 *      displays it. This covers every enrichment + companion stage.
 *
 *   2. **`current_track`** present (download underway, GAMDL is
 *      iterating tracks): assemble a `DOWNLOADING…Artist — Album —
 *      "Track"` caption inline. Track in quotes because it changes
 *      per-track and the quotes signal "this is what's transferring
 *      right now"; Artist/Album are stable context.
 *
 *   3. **Fallback**: just the album name (or first URL if even the
 *      album is unknown — typically pre-metadata-fetch state).
 */
export function formatActiveItemCaption(
  item: QueueItemStatus | null | undefined
): string {
  if (!item) return 'Waiting…';

  // Path 1: backend-assembled processing label.
  if (item.processing_label) return item.processing_label;

  const track = item.current_track;
  const album = item.album_name;
  const artist = item.artist_name;
  const isDownloading = item.state === 'downloading';

  // Path 2: assemble from per-track context.
  if (track) {
    const parts: string[] = [];
    if (artist) parts.push(artist);
    if (album) parts.push(album);
    parts.push(`"${track}"`);
    const label = parts.join(' — ');
    return isDownloading ? `DOWNLOADING...${label}` : label;
  }

  // Path 3: fallback to album / URL.
  const fallback = album ?? item.urls?.[0] ?? '';
  return isDownloading ? `DOWNLOADING...${fallback}` : fallback;
}
