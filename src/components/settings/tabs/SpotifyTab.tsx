/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file SpotifyTab.tsx -- Spotify settings placeholder tab.
 *
 * This tab will contain Spotify-specific download settings once the
 * votify integration is implemented. Currently displays a "Coming Soon"
 * message with a brief description of planned capabilities.
 */

/**
 * SpotifyTab -- Placeholder settings tab for Spotify downloads.
 *
 * Planned capabilities:
 * - Track, album, playlist, and artist downloads
 * - Ogg Vorbis quality selection
 * - Cover art saving
 * - Lyrics embedding
 * - Output template configuration
 */
export function SpotifyTab() {
  return (
    <div className="space-y-4">
      <div className="text-center py-12">
        <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-[#1db954]/10 mb-4">
          <span className="text-2xl">&#127925;</span>
        </div>
        <h3 className="text-lg font-semibold text-content-primary">Spotify Downloads</h3>
        <p className="text-sm text-content-secondary mt-2 max-w-md mx-auto">
          Download tracks, albums, playlists, and artist discographies from Spotify using votify.
          Supports Ogg Vorbis quality selection, cover art saving, and lyrics embedding.
        </p>
        <div className="mt-4 inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-status-warning/10 text-status-warning text-xs font-medium">
          Coming Soon
        </div>
      </div>
    </div>
  );
}
