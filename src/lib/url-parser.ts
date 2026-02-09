/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Apple Music URL parser.
 * Detects the content type (song, album, playlist, music video, artist)
 * from an Apple Music URL by analyzing the URL path structure.
 * Used by the download form to show content-type indicators and
 * apply appropriate default options per content type.
 */

import type { AppleMusicContentType, ParsedUrl } from '@/types';

/**
 * Parses an Apple Music URL and detects its content type.
 *
 * Supported URL formats:
 * - Songs:        https://music.apple.com/{region}/album/{name}/{id}?i={trackId}
 * - Albums:       https://music.apple.com/{region}/album/{name}/{id}
 * - Playlists:    https://music.apple.com/{region}/playlist/{name}/{id}
 * - Music Videos: https://music.apple.com/{region}/music-video/{name}/{id}
 * - Artists:      https://music.apple.com/{region}/artist/{name}/{id}
 *
 * @param url - The URL string to parse
 * @returns Parsed URL with content type and validity information
 */
export function parseAppleMusicUrl(url: string): ParsedUrl {
  const trimmed = url.trim();

  // Check if the URL is a valid Apple Music URL
  if (!isAppleMusicUrl(trimmed)) {
    return { url: trimmed, contentType: 'unknown', isValid: false };
  }

  // Detect the content type from the URL path
  const contentType = detectContentType(trimmed);

  return {
    url: trimmed,
    contentType,
    isValid: contentType !== 'unknown',
  };
}

/**
 * Checks whether a string is a valid Apple Music URL.
 *
 * Accepts both https://music.apple.com/ and https://itunes.apple.com/ formats.
 */
export function isAppleMusicUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return (
      parsed.hostname === 'music.apple.com' ||
      parsed.hostname === 'itunes.apple.com'
    );
  } catch {
    return false;
  }
}

/**
 * Detects the content type from an Apple Music URL path.
 *
 * The path structure is: /{region}/{type}/{name}/{id}
 * Songs are albums with a `?i=` query parameter for the track ID.
 */
function detectContentType(url: string): AppleMusicContentType {
  try {
    const parsed = new URL(url);
    const path = parsed.pathname.toLowerCase();

    // Songs: album URL with an `i` query parameter (track ID)
    if (path.includes('/album/') && parsed.searchParams.has('i')) {
      return 'song';
    }

    // Albums: /album/ without a track ID query parameter
    if (path.includes('/album/')) {
      return 'album';
    }

    // Playlists: /playlist/ path segment
    if (path.includes('/playlist/')) {
      return 'playlist';
    }

    // Music Videos: /music-video/ path segment
    if (path.includes('/music-video/')) {
      return 'music-video';
    }

    // Artists: /artist/ path segment
    if (path.includes('/artist/')) {
      return 'artist';
    }

    return 'unknown';
  } catch {
    return 'unknown';
  }
}

/**
 * Returns a human-readable label for a content type.
 *
 * @param contentType - The detected content type
 * @returns A display string (e.g., "Album", "Music Video")
 */
export function getContentTypeLabel(contentType: AppleMusicContentType): string {
  switch (contentType) {
    case 'song':
      return 'Song';
    case 'album':
      return 'Album';
    case 'playlist':
      return 'Playlist';
    case 'music-video':
      return 'Music Video';
    case 'artist':
      return 'Artist';
    default:
      return 'Unknown';
  }
}
