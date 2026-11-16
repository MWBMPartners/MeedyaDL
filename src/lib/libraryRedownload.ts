// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file libraryRedownload — the download request the Library page sends
 * when the person asks to fill an album's gaps.
 *
 * A Library re-download only fills in what is missing. It must never
 * replace the files already there — they carry everything MeedyaDL added
 * after the first download (tags, lyrics, artwork) — so it always asks for
 * overwrite OFF, whatever the "Overwrite" setting says. Without this the
 * backend took the setting, and with it on a "2 of 12 missing"
 * re-download replaced all twelve.
 */

import type { DownloadRequest } from '@/types';

/**
 * Builds the request for re-downloading one album from the Library page.
 *
 * @param url - The album's link from its download record.
 * @param mvCompanionOverride - The person's music-video choice for this
 *   re-download (`null` follows the setting).
 */
export function buildLibraryRedownloadRequest(
  url: string,
  mvCompanionOverride: boolean | null
): DownloadRequest {
  return {
    urls: [url],
    options: { overwrite: false },
    mv_companion_override: mvCompanionOverride,
  };
}
