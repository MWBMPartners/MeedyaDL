/**
 * Copyright (c) 2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/lib/url-parser.ts - Apple Music URL parser and content type detector
 *
 * This module provides client-side URL parsing for Apple Music URLs.
 * It determines the content type (song, album, playlist, music video, artist)
 * by analyzing the URL path structure without making any network requests.
 *
 * Apple Music URL structure:
 * ```
 * https://music.apple.com/{region}/{type}/{slug}/{id}[?i={trackId}]
 *   │                       │       │      │      │    └─ Track ID (songs only)
 *   │                       │       │      │      └──── Numeric ID
 *   │                       │       │      └─────────── URL-safe slug name
 *   │                       │       └────────────────── Content type path segment
 *   │                       └────────────────────────── 2-letter region code (us, gb, jp...)
 *   └────────────────────────────────────────────────── Base domain
 * ```
 *
 * Also accepts `classical.apple.com` (Apple Music Classical) and the legacy
 * `itunes.apple.com` domain (auto-redirects to music.apple.com).
 *
 * This parser is purely frontend logic (no IPC calls to Rust). It uses the
 * built-in `URL` Web API for reliable URL parsing instead of regex-based
 * extraction, which avoids edge cases with URL encoding, ports, and fragments.
 *
 * Used by: DownloadForm component (content-type badges, per-type option defaults)
 *
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/API/URL} - URL Web API
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/JavaScript/Guide/Regular_expressions} - Regex reference
 */

/**
 * Type imports for the return types of parser functions.
 * @see src/types/index.ts for AppleMusicContentType and ParsedUrl definitions
 */
import type { AppleMusicContentType, MediaServiceId, ParsedUrl } from '@/types';

/**
 * Parses an Apple Music URL and detects its content type.
 *
 * This is the primary entry point for URL parsing. It performs three steps:
 * 1. Trims whitespace from the input (handles copy-paste artifacts)
 * 2. Validates that the URL belongs to an Apple Music domain
 * 3. Detects the specific content type from the URL path structure
 *
 * Supported URL formats:
 * - Songs:        https://music.apple.com/{region}/album/{name}/{id}?i={trackId}
 * - Albums:       https://music.apple.com/{region}/album/{name}/{id}
 * - Playlists:    https://music.apple.com/{region}/playlist/{name}/{id}
 * - Music Videos: https://music.apple.com/{region}/music-video/{name}/{id}
 * - Artists:      https://music.apple.com/{region}/artist/{name}/{id}
 *
 * @param url - The URL string to parse (may contain leading/trailing whitespace)
 * @returns ParsedUrl with the trimmed URL, detected content type, and validity flag
 *
 * @example
 * ```ts
 * const result = parseAppleMusicUrl('https://music.apple.com/us/album/fearless/1440935016');
 * // result = { url: '...', contentType: 'album', isValid: true }
 *
 * const song = parseAppleMusicUrl('https://music.apple.com/us/album/love-story/1440935016?i=1440935018');
 * // song = { url: '...', contentType: 'song', isValid: true }
 *
 * const classical = parseAppleMusicUrl('https://classical.apple.com/us/album/beethoven-symphony-no-9/1234567890');
 * // classical = { url: '...', contentType: 'album', isValid: true }
 * ```
 */
export function parseAppleMusicUrl(url: string): ParsedUrl {
  /* Step 1: Trim whitespace to handle copy-paste from browsers/messages */
  const trimmed = url.trim();

  /* Step 2: Validate the domain -- reject non-Apple Music URLs early */
  if (!isAppleMusicUrl(trimmed)) {
    return { url: trimmed, contentType: 'unknown', isValid: false };
  }

  /* Step 3: Analyze the URL path to determine content type */
  const contentType = detectContentType(trimmed);

  /*
   * A URL is valid (submittable) if it classifies into a known content
   * type — including `recording`. An earlier iteration (#573) rejected
   * `recording` URLs on the grounds that GAMDL's URL vocabulary doesn't
   * include `/recording/` paths, but that was reverted 2026-04-23: the
   * correct UX is to accept what the user pasted and let the pipeline
   * give a clear outcome. With #567's broadened empty-output guard now
   * on main, a URL GAMDL can't handle fails cleanly (single
   * "Enrichment skipped — primary download produced no output files"
   * line instead of cascading false-success noise), so the rationale
   * for pre-emptive rejection no longer holds.
   */
  return {
    url: trimmed,
    contentType,
    isValid: contentType !== 'unknown',
  };
}

/**
 * Checks whether a string is a valid Apple Music URL.
 *
 * Uses the built-in `URL` constructor for parsing. If the string is not
 * a valid URL at all, the constructor throws a `TypeError`, which we
 * catch and return `false`. This approach is more reliable than regex
 * for handling edge cases like URL encoding, ports, and fragments.
 *
 * Accepted domains:
 * - `music.apple.com` - Current Apple Music web player domain
 * - `classical.apple.com` - Legacy Apple Music Classical domain
 * - `classical.music.apple.com` - Current Apple Music Classical domain
 *   (Apple moved Classical under music.apple.com's subdomain hierarchy
 *   in 2026; the app now shares Share-link format with `music.apple.com`
 *   but drops the slug segment: `/album/{id}` instead of
 *   `/album/{slug}/{id}`)
 * - `itunes.apple.com` - Legacy iTunes Store domain (still used in some links)
 *
 * @param url - The URL string to validate
 * @returns true if the URL belongs to an Apple Music domain, false otherwise
 *
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/API/URL/URL} - URL constructor
 */
export function isAppleMusicUrl(url: string): boolean {
  try {
    /* Use the URL constructor for standards-compliant URL parsing */
    const parsed = new URL(url);
    /* Check hostname against the current and legacy Apple Music domains */
    return (
      parsed.hostname === 'music.apple.com' ||
      parsed.hostname === 'classical.apple.com' ||
      parsed.hostname === 'classical.music.apple.com' ||
      parsed.hostname === 'itunes.apple.com'
    );
  } catch {
    /* URL constructor throws TypeError for malformed URLs (not a valid URL) */
    return false;
  }
}

/**
 * Detects the content type from an Apple Music URL path.
 *
 * This is a private helper function that performs the actual content type
 * classification. It uses simple `String.includes()` checks on the URL
 * path rather than complex regex patterns, prioritizing readability and
 * maintainability.
 *
 * Apple Music URL path structure: `/{region}/{type}/{slug}/{id}`
 *
 * Detection logic (order matters!):
 * 1. Songs MUST be checked before albums because song URLs use the
 *    `/album/` path segment with an additional `?i={trackId}` query
 *    parameter. If we checked for `/album/` first, songs would be
 *    misclassified as albums.
 * 2. All other content types use unique path segments.
 *
 * @param url - A valid Apple Music URL string
 * @returns The detected content type, or 'unknown' if the path doesn't match
 */
function detectContentType(url: string): AppleMusicContentType {
  try {
    const parsed = new URL(url);
    /* Lowercase the path for case-insensitive matching */
    const path = parsed.pathname.toLowerCase();

    /*
     * IMPORTANT: Song detection MUST come before album detection.
     * Songs are a subset of album URLs -- they share the /album/ path
     * but additionally include an `i` query parameter containing the
     * individual track ID within the album.
     * Example: /us/album/fearless/1440935016?i=1440935018
     */
    if (path.includes('/album/') && parsed.searchParams.has('i')) {
      return 'song';
    }

    /* Albums: /album/ path without the `i` query parameter */
    if (path.includes('/album/')) {
      return 'album';
    }

    /* Playlists: /playlist/ path segment (may use `pl.` prefixed IDs) */
    if (path.includes('/playlist/')) {
      return 'playlist';
    }

    /* Music Videos: /music-video/ path segment (hyphenated) */
    if (path.includes('/music-video/')) {
      return 'music-video';
    }

    /* Artists: /artist/ path segment */
    if (path.includes('/artist/')) {
      return 'artist';
    }

    /* Library URLs: /library/ path segment (personal library items) */
    /* e.g., /library/albums/l.8zPXbAv, /library/songs/..., /library/playlists/... */
    if (path.includes('/library/')) {
      return 'library';
    }

    /*
     * Apple Music Classical recording URLs (`classical.music.apple.com`).
     * A "recording" is a specific performance of a classical work — e.g.
     * Beethoven's 5th as performed by Karajan / Berliner Philharmoniker
     * in 1977, distinct from the album release that contains it. The
     * path shape is `/recording/{composer-year-piece}-{id}` (example:
     * `/gb/recording/gustav-mahler-1860-pp1-1452377808?l=en-GB`).
     *
     * Recognised here so the UI can surface a specific "not yet
     * supported — use the album URL instead" message rather than the
     * generic "Please enter a valid Apple Music URL". Marked
     * non-submittable in `parseAppleMusicUrl()` to block downloads —
     * GAMDL's URL vocabulary does not include `/recording/` paths.
     */
    if (path.includes('/recording/')) {
      return 'recording';
    }

    /* No recognized content type path segment found */
    return 'unknown';
  } catch {
    /* Defensive: catch any unexpected URL parsing errors */
    return 'unknown';
  }
}

/**
 * Returns a human-readable label for a content type.
 *
 * Maps the machine-readable `AppleMusicContentType` string literal to
 * a user-friendly display string. Used in the DownloadForm to show
 * content-type badges next to parsed URLs (e.g., a blue "Album" pill).
 *
 * The `default` case handles the 'unknown' variant and any future
 * additions to the type that haven't been mapped yet.
 *
 * @param contentType - The detected content type from parseAppleMusicUrl()
 * @returns A capitalized display string (e.g., "Album", "Music Video", "Unknown")
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
    case 'library':
      return 'Library';
    case 'recording':
      return 'Classical Recording';
    default:
      return 'Unknown';
  }
}

// ============================================================
// Multi-Service URL Detection
// ============================================================

/**
 * Domain-to-service mapping for client-side URL detection.
 * Order matters: more specific domains must come before broader ones
 * (e.g., music.youtube.com before youtube.com).
 */
const SERVICE_DOMAINS: Array<{ service: MediaServiceId; domains: string[] }> = [
  { service: 'apple-music', domains: ['classical.music.apple.com', 'music.apple.com', 'classical.apple.com', 'itunes.apple.com'] },
  { service: 'youtube-music', domains: ['music.youtube.com'] },
  { service: 'youtube', domains: ['youtube.com', 'youtu.be'] },
  { service: 'spotify', domains: ['open.spotify.com'] },
  { service: 'bbc-iplayer', domains: ['bbc.co.uk/iplayer', 'bbc.co.uk/sounds'] },
];

/**
 * Detects which media service a URL belongs to.
 *
 * Checks the URL against known service domains. Returns the service ID
 * if recognised, or `null` if the URL doesn't match any supported service.
 * This is a pure client-side check — no IPC call needed.
 *
 * @param url - The URL string to check
 * @returns The detected MediaServiceId, or null if unrecognised
 */
export function detectService(url: string): MediaServiceId | null {
  const lower = url.toLowerCase();
  for (const { service, domains } of SERVICE_DOMAINS) {
    for (const domain of domains) {
      if (lower.includes(domain)) {
        return service;
      }
    }
  }
  return null;
}

/**
 * Checks whether a URL belongs to any supported media service.
 *
 * @param url - The URL string to validate
 * @returns true if the URL matches a known service domain
 */
export function isSupportedUrl(url: string): boolean {
  return detectService(url) !== null;
}

/**
 * Result type for the generic multi-service URL parser.
 */
export interface ParsedMediaUrl {
  /** The trimmed URL string */
  url: string;
  /** The detected service, or null if unrecognised */
  service: MediaServiceId | null;
  /** Content type (Apple Music only; null for other services) */
  contentType: AppleMusicContentType | null;
  /** Whether the URL is a valid supported service URL */
  isValid: boolean;
}

/**
 * Generic multi-service URL parser.
 *
 * Detects which service a URL belongs to and, for Apple Music URLs,
 * also detects the content type (song, album, playlist, etc.).
 * For other services, content type detection will be added as those
 * services are implemented.
 *
 * @param url - The URL string to parse
 * @returns ParsedMediaUrl with service detection and validity
 */
export function parseMediaUrl(url: string): ParsedMediaUrl {
  const trimmed = url.trim();
  const service = detectService(trimmed);

  if (!service) {
    return { url: trimmed, service: null, contentType: null, isValid: false };
  }

  // For Apple Music, also detect content type
  if (service === 'apple-music') {
    const contentType = detectContentType(trimmed);
    return {
      url: trimmed,
      service,
      contentType,
      isValid: contentType !== 'unknown',
    };
  }

  // For other services, URL is valid if the domain matches
  return { url: trimmed, service, contentType: null, isValid: true };
}
