// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file libraryRedownload — what the Library page does when the person
 * asks to re-download an album, and the request it sends.
 *
 * Two reasons enable the button, and they want different things:
 *
 * - **Something is missing** (songs, or copies in a format): the
 *   re-download fills the gaps only and never replaces a file already
 *   there — those carry everything MeedyaDL added after the first download
 *   (tags, lyrics, artwork). Overwrite is always OFF, whatever the setting.
 *   (Songs already in the folder are kept; if their names were changed
 *   after the first download, a second copy may be added — the page warns.)
 * - **Only "Content updated"** (Apple Music changed the album): nothing is
 *   missing, so getting the new version means replacing files. That
 *   follows the person's "Overwrite" setting. With it off, nothing is
 *   queued — the download would fetch nothing — and the page says why.
 *
 * Both at once: fill the gaps, and say updated songs are not replaced.
 */

import type { DownloadRequest } from '@/types';

/** Why the button is enabled for a row. */
export interface LibraryRowState {
  /** Songs or format copies are missing. */
  hasGaps: boolean;
  /** Apple Music reports a newer version of the album. */
  updated: boolean;
}

/** What the page does for a row. */
export type LibraryRedownloadDecision =
  /** Fill the gaps, never replacing a file. `note` is shown when set. */
  | { kind: 'fill_gaps'; note?: string }
  /** Fetch the updated version, following the Overwrite setting. */
  | { kind: 'refresh' }
  /** Queue nothing; show `message` instead. */
  | { kind: 'explain'; message: string };

/** Shown when only an update is available and Overwrite is off. Names the
 * setting exactly as Settings > General shows it. */
export const UPDATE_NEEDS_OVERWRITE =
  'To download the updated version, turn on "Overwrite Existing Files" in Settings. With it off, the songs already in the folder are kept as they are, so nothing was added to the queue.';

/** Shown when a row has gaps and an update. */
export const GAPS_ONLY_NOTE =
  'Only what is missing is asked for. Songs already in the folder are kept, not replaced, so songs Apple Music has updated are not downloaded again.';

/** Title, text and button of the warning shown before re-downloading an
 * album whose folder or files were renamed after the first download. */
export const SECOND_COPY_WARNING = {
  title: 'This may add a second copy',
  description:
    "This album's folder or files were renamed after the first download (for example with [Explicit] or [Lossless]). Re-downloading it may add a second copy beside the existing files, instead of filling in or replacing them.",
  confirmLabel: 'Re-download anyway',
} as const;

/**
 * Whether to warn, before queueing, that the re-download may add a second
 * copy: the album's folder or files carry labels MeedyaDL added after the
 * first download (the download writes the plain names), and something
 * would actually be queued.
 */
export function shouldWarnAboutSecondCopy(
  decision: LibraryRedownloadDecision,
  hasLabelledNames: boolean
): boolean {
  return hasLabelledNames && decision.kind !== 'explain';
}

/**
 * Decides what a Library re-download does for one row.
 *
 * @param row - Why the button is enabled.
 * @param overwriteSetting - The saved "Overwrite existing files" setting.
 */
export function decideLibraryRedownload(
  row: LibraryRowState,
  overwriteSetting: boolean
): LibraryRedownloadDecision {
  if (row.hasGaps) {
    return row.updated ? { kind: 'fill_gaps', note: GAPS_ONLY_NOTE } : { kind: 'fill_gaps' };
  }
  if (row.updated) {
    return overwriteSetting
      ? { kind: 'refresh' }
      : { kind: 'explain', message: UPDATE_NEEDS_OVERWRITE };
  }
  // Nothing to do; the button is not shown in this case.
  return { kind: 'fill_gaps' };
}

/**
 * Builds the request for re-downloading one album from the Library page.
 *
 * @param url - The album's link from its download record.
 * @param mvCompanionOverride - The person's music-video choice for this
 *   re-download (`null` follows the setting).
 * @param mode - `fill_gaps` sends overwrite OFF; `refresh` sends no
 *   override, so the "Overwrite" setting decides.
 */
export function buildLibraryRedownloadRequest(
  url: string,
  mvCompanionOverride: boolean | null,
  mode: 'fill_gaps' | 'refresh' = 'fill_gaps'
): DownloadRequest {
  return mode === 'fill_gaps'
    ? { urls: [url], options: { overwrite: false }, mv_companion_override: mvCompanionOverride }
    : { urls: [url], mv_companion_override: mvCompanionOverride };
}
