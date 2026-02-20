/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file YouTubeTab.tsx -- YouTube settings placeholder tab.
 *
 * This tab will contain YouTube-specific download settings once the
 * yt-dlp integration is implemented. Currently displays a "Coming Soon"
 * message with a brief description of planned capabilities.
 */

/**
 * YouTubeTab -- Placeholder settings tab for YouTube downloads.
 *
 * Planned capabilities:
 * - Video resolution selection (up to 8K)
 * - Audio-only extraction with format selection
 * - Subtitle embedding and language selection
 * - SponsorBlock integration for ad segment removal
 * - Concurrent fragment downloads
 * - Cookie import for age-restricted content
 */
export function YouTubeTab() {
  return (
    <div className="space-y-4">
      <div className="text-center py-12">
        <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-[#ff0000]/10 mb-4">
          <span className="text-2xl">&#9654;</span>
        </div>
        <h3 className="text-lg font-semibold text-content-primary">
          YouTube Downloads
        </h3>
        <p className="text-sm text-content-secondary mt-2 max-w-md mx-auto">
          Download videos, playlists, channels, and live streams from YouTube
          in up to 8K resolution with audio-only extraction, subtitle embedding,
          and SponsorBlock integration.
        </p>
        <div className="mt-4 inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-status-warning/10 text-status-warning text-xs font-medium">
          Coming Soon
        </div>
      </div>
    </div>
  );
}
