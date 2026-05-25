/* eslint-disable @typescript-eslint/ban-ts-comment */
// @ts-nocheck — Staged for future multi-service integration.
//
/**
 * Copyright (c) 2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file BBCiPlayerTab.tsx -- BBC iPlayer settings placeholder tab.
 *
 * This tab will contain BBC iPlayer-specific download settings once the
 * get_iplayer + yt-dlp integration is implemented. Currently displays a
 * "Coming Soon" message with a brief description of planned capabilities.
 */

/**
 * BBCiPlayerTab -- Placeholder settings tab for BBC iPlayer downloads.
 *
 * Planned capabilities:
 * - TV programme and episode downloads
 * - BBC Sounds audio content
 * - Quality selection (SD, HD, Full HD)
 * - Subtitle and thumbnail downloads
 * - Series batch downloads
 */
export function BBCiPlayerTab() {
  return (
    <div className="space-y-4">
      <div className="text-center py-12">
        <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-[#006def]/10 mb-4">
          <span className="text-2xl">&#128250;</span>
        </div>
        <h3 className="text-lg font-semibold text-content-primary">BBC iPlayer Downloads</h3>
        <p className="text-sm text-content-secondary mt-2 max-w-md mx-auto">
          Download TV programmes, episodes, series, and BBC Sounds audio content using get_iplayer
          with yt-dlp fallback. Supports quality selection, subtitles, and thumbnails.
        </p>
        <p className="text-xs text-content-tertiary mt-2 max-w-md mx-auto">
          Note: BBC iPlayer content is region-restricted to the UK. A UK VPN may be required.
        </p>
        <div className="mt-4 inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-status-warning/10 text-status-warning text-xs font-medium">
          Coming Soon
        </div>
      </div>
    </div>
  );
}
