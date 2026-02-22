/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/lib/url-parser.ts - Multi-service URL parser and content type detector
 *
 * This module provides client-side URL parsing for all supported media services:
 * Apple Music, YouTube, BBC iPlayer, and Spotify. It determines the content type
 * and service by analyzing the URL domain and path structure without making any
 * network requests.
 *
 * The main entry point is `parseUrl()`, which auto-detects the service from the
 * URL domain and classifies the content type. The legacy `parseAppleMusicUrl()`
 * and `isAppleMusicUrl()` functions are preserved for backward compatibility.
 *
 * Used by: DownloadForm component (content-type/service badges, per-type defaults)
 *
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/API/URL} - URL Web API
 */

import type { AppleMusicContentType, MediaContentType, MediaServiceId, ParsedUrl } from '@/types';

// ============================================================
// Main entry point: multi-service URL parser
// ============================================================

/**
 * Parses any supported media URL and detects both service and content type.
 *
 * This is the primary entry point for URL parsing. It performs:
 * 1. Trims whitespace from the input
 * 2. Detects which media service the URL belongs to via domain matching
 * 3. Detects the specific content type from the URL path structure
 *
 * @param url - The URL string to parse (may contain leading/trailing whitespace)
 * @returns ParsedUrl with the trimmed URL, detected service, content type, and validity
 *
 * @example
 * ```ts
 * parseUrl('https://music.apple.com/us/album/fearless/1440935016')
 * // { url: '...', serviceId: 'apple-music', contentType: 'album', isValid: true }
 *
 * parseUrl('https://www.youtube.com/watch?v=dQw4w9WgXcQ')
 * // { url: '...', serviceId: 'youtube', contentType: 'video', isValid: true }
 *
 * parseUrl('https://open.spotify.com/album/1234')
 * // { url: '...', serviceId: 'spotify', contentType: 'album', isValid: true }
 *
 * parseUrl('https://www.bbc.co.uk/iplayer/episode/b006q2x0/doctor-who')
 * // { url: '...', serviceId: 'bbc-iplayer', contentType: 'episode', isValid: true }
 * ```
 */
export function parseUrl(url: string): ParsedUrl {
  const trimmed = url.trim();
  const serviceId = detectService(trimmed);

  if (!serviceId) {
    return { url: trimmed, serviceId: null, contentType: 'unknown', isValid: false };
  }

  let contentType: MediaContentType;
  switch (serviceId) {
    case 'apple-music':
      contentType = detectAppleMusicContentType(trimmed);
      break;
    case 'youtube':
      contentType = detectYouTubeContentType(trimmed);
      break;
    case 'bbc-iplayer':
      contentType = detectBBCiPlayerContentType(trimmed);
      break;
    case 'spotify':
      contentType = detectSpotifyContentType(trimmed);
      break;
    default:
      contentType = 'unknown';
  }

  return {
    url: trimmed,
    serviceId,
    contentType,
    isValid: contentType !== 'unknown',
  };
}

// ============================================================
// Service detection
// ============================================================

/**
 * Detects which media service a URL belongs to based on its domain.
 *
 * @param url - A URL string to check
 * @returns The detected service ID, or null if no known service matches
 */
export function detectService(url: string): MediaServiceId | null {
  try {
    const parsed = new URL(url);
    const hostname = parsed.hostname.toLowerCase();

    // Apple Music
    if (hostname === 'music.apple.com' || hostname === 'itunes.apple.com') {
      return 'apple-music';
    }

    // BBC iPlayer / Sounds (check before YouTube since bbc.co.uk is more specific)
    if (hostname === 'www.bbc.co.uk' || hostname === 'bbc.co.uk') {
      const path = parsed.pathname.toLowerCase();
      if (path.startsWith('/iplayer') || path.startsWith('/sounds')) {
        return 'bbc-iplayer';
      }
    }

    // YouTube (including YouTube Music)
    if (
      hostname === 'www.youtube.com' ||
      hostname === 'youtube.com' ||
      hostname === 'youtu.be' ||
      hostname === 'music.youtube.com' ||
      hostname === 'm.youtube.com'
    ) {
      return 'youtube';
    }

    // Spotify
    if (hostname === 'open.spotify.com') {
      return 'spotify';
    }

    return null;
  } catch {
    return null;
  }
}

// ============================================================
// Service-specific content type detectors
// ============================================================

/**
 * Detects the content type from an Apple Music URL path.
 * Songs are checked before albums (songs are album URLs with `?i=` param).
 */
function detectAppleMusicContentType(url: string): AppleMusicContentType {
  try {
    const parsed = new URL(url);
    const path = parsed.pathname.toLowerCase();

    if (path.includes('/album/') && parsed.searchParams.has('i')) {
      return 'song';
    }
    if (path.includes('/album/')) return 'album';
    if (path.includes('/playlist/')) return 'playlist';
    if (path.includes('/music-video/')) return 'music-video';
    if (path.includes('/artist/')) return 'artist';
    return 'unknown';
  } catch {
    return 'unknown';
  }
}

/**
 * Detects the content type from a YouTube URL path.
 *
 * Supported URL patterns:
 * - Videos:     youtube.com/watch?v=ID, youtu.be/ID
 * - Playlists:  youtube.com/playlist?list=ID
 * - Channels:   youtube.com/channel/ID, youtube.com/@handle
 * - Live:       youtube.com/watch?v=ID (with /live path or live indicator)
 */
function detectYouTubeContentType(url: string): MediaContentType {
  try {
    const parsed = new URL(url);
    const path = parsed.pathname.toLowerCase();
    const hostname = parsed.hostname.toLowerCase();

    // Live streams (explicit /live path)
    if (path.includes('/live')) {
      return 'live-stream';
    }

    // Playlists
    if (path === '/playlist' && parsed.searchParams.has('list')) {
      return 'playlist';
    }

    // Channels
    if (path.startsWith('/channel/') || path.startsWith('/@')) {
      return 'channel';
    }

    // Regular videos (youtube.com/watch?v= or youtu.be/ID)
    if (parsed.searchParams.has('v') || hostname === 'youtu.be') {
      return 'video';
    }

    return 'unknown';
  } catch {
    return 'unknown';
  }
}

/**
 * Detects the content type from a BBC iPlayer/Sounds URL path.
 *
 * Supported URL patterns:
 * - Episodes:    bbc.co.uk/iplayer/episode/ID
 * - Series:      bbc.co.uk/iplayer/episodes/ID
 * - Clips:       bbc.co.uk/programmes/ID (short clip)
 * - Programmes:  bbc.co.uk/sounds/play/ID, bbc.co.uk/iplayer/group/ID
 */
function detectBBCiPlayerContentType(url: string): MediaContentType {
  try {
    const parsed = new URL(url);
    const path = parsed.pathname.toLowerCase();

    if (path.includes('/iplayer/episode/')) return 'episode';
    if (path.includes('/iplayer/episodes/')) return 'series';
    if (path.includes('/iplayer/group/')) return 'series';
    if (path.includes('/sounds/play/')) return 'episode';
    if (path.includes('/sounds/series/')) return 'series';
    if (path.includes('/programmes/')) return 'programme';
    return 'unknown';
  } catch {
    return 'unknown';
  }
}

/**
 * Detects the content type from a Spotify URL path.
 *
 * Supported URL patterns:
 * - Tracks:     open.spotify.com/track/ID
 * - Albums:     open.spotify.com/album/ID
 * - Playlists:  open.spotify.com/playlist/ID
 * - Artists:    open.spotify.com/artist/ID
 */
function detectSpotifyContentType(url: string): MediaContentType {
  try {
    const parsed = new URL(url);
    const path = parsed.pathname.toLowerCase();

    if (path.startsWith('/track/')) return 'track';
    if (path.startsWith('/album/')) return 'album';
    if (path.startsWith('/playlist/')) return 'playlist';
    if (path.startsWith('/artist/')) return 'artist';
    return 'unknown';
  } catch {
    return 'unknown';
  }
}

// ============================================================
// UI helpers
// ============================================================

/**
 * Returns a human-readable label for the detected service.
 *
 * @param serviceId - The detected service ID
 * @returns A display string (e.g., "Apple Music", "YouTube")
 */
export function getServiceLabel(serviceId: MediaServiceId): string {
  const labels: Record<MediaServiceId, string> = {
    'apple-music': 'Apple Music',
    youtube: 'YouTube',
    'bbc-iplayer': 'BBC iPlayer',
    spotify: 'Spotify',
  };
  return labels[serviceId];
}

/**
 * Returns a CSS colour string for a service badge.
 *
 * @param serviceId - The service to get the colour for
 * @returns A hex colour code
 */
export function getServiceColor(serviceId: MediaServiceId): string {
  const colors: Record<MediaServiceId, string> = {
    'apple-music': '#fc3c44',
    youtube: '#ff0000',
    'bbc-iplayer': '#006def',
    spotify: '#1db954',
  };
  return colors[serviceId];
}

/**
 * Returns a human-readable label for a content type.
 *
 * Maps the machine-readable content type string literal to a user-friendly
 * display string. Used in the DownloadForm to show content-type badges
 * next to parsed URLs.
 *
 * @param contentType - The detected content type
 * @returns A capitalized display string (e.g., "Album", "Music Video", "Episode")
 */
export function getContentTypeLabel(contentType: MediaContentType): string {
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
    case 'video':
      return 'Video';
    case 'channel':
      return 'Channel';
    case 'live-stream':
      return 'Live Stream';
    case 'programme':
      return 'Programme';
    case 'episode':
      return 'Episode';
    case 'clip':
      return 'Clip';
    case 'series':
      return 'Series';
    case 'track':
      return 'Track';
    default:
      return 'Unknown';
  }
}

// ============================================================
// Backward-compatible exports
// ============================================================

/**
 * Parses an Apple Music URL and detects its content type.
 *
 * @deprecated Use `parseUrl()` instead, which auto-detects the service.
 *
 * This function is preserved for backward compatibility with existing
 * code that specifically targets Apple Music URLs.
 *
 * @param url - The URL string to parse
 * @returns ParsedUrl with the trimmed URL, detected content type, and validity flag
 */
export function parseAppleMusicUrl(url: string): ParsedUrl {
  const trimmed = url.trim();
  if (!isAppleMusicUrl(trimmed)) {
    return { url: trimmed, serviceId: null, contentType: 'unknown', isValid: false };
  }
  const contentType = detectAppleMusicContentType(trimmed);
  return {
    url: trimmed,
    serviceId: 'apple-music',
    contentType,
    isValid: contentType !== 'unknown',
  };
}

/**
 * Checks whether a string is a valid Apple Music URL.
 *
 * @param url - The URL string to validate
 * @returns true if the URL belongs to an Apple Music domain, false otherwise
 */
export function isAppleMusicUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    return parsed.hostname === 'music.apple.com' || parsed.hostname === 'itunes.apple.com';
  } catch {
    return false;
  }
}
