/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/lib/url-parser.test.ts - Unit tests for the Apple Music URL parser
 *
 * This test suite validates the three exported functions from url-parser.ts:
 * 1. `isAppleMusicUrl()` - Domain validation (music.apple.com / itunes.apple.com)
 * 2. `parseAppleMusicUrl()` - Full URL parsing with content type detection
 * 3. `getContentTypeLabel()` - Content type to display label mapping
 *
 * Test strategy:
 * - Tests cover the happy path for every supported content type
 * - Edge cases include invalid URLs, non-Apple domains, empty strings,
 *   and whitespace-padded input
 * - No mocking is needed because url-parser.ts is pure logic with no
 *   dependencies on Tauri APIs or external services
 *
 * Run with: `npx vitest run src/lib/url-parser.test.ts`
 *
 * @see {@link https://vitest.dev/api/} - Vitest API reference
 * @see {@link https://vitest.dev/api/expect.html} - Vitest expect matchers
 */

/**
 * Vitest test primitives:
 * - `describe`: groups related tests into a named suite
 * - `it`: defines a single test case (alias for `test`)
 * - `expect`: creates an assertion chain
 *
 * @see {@link https://vitest.dev/api/#describe} - describe API
 * @see {@link https://vitest.dev/api/#test} - it/test API
 * @see {@link https://vitest.dev/api/expect.html} - expect API
 */
import { describe, it, expect } from 'vitest';

/**
 * Import the functions under test from the url-parser module.
 * These are the public API of the parser that components consume.
 */
import { parseAppleMusicUrl, isAppleMusicUrl, getContentTypeLabel, detectService, parseMediaUrl, isSupportedUrl } from './url-parser';

/**
 * Test suite for `isAppleMusicUrl()` - domain validation.
 *
 * This function is the first gate in URL processing. It must:
 * - Accept both current (music.apple.com) and legacy (itunes.apple.com) domains
 * - Reject all other domains (even music-related ones like spotify.com)
 * - Handle malformed input gracefully (no throws)
 */
describe('isAppleMusicUrl', () => {
  /** Validates that the current Apple Music domain is accepted */
  it('accepts music.apple.com URLs', () => {
    expect(isAppleMusicUrl('https://music.apple.com/us/album/test/123')).toBe(true);
  });

  /** Validates that the legacy iTunes domain is also accepted (backward compatibility) */
  it('accepts itunes.apple.com URLs', () => {
    expect(isAppleMusicUrl('https://itunes.apple.com/us/album/test/123')).toBe(true);
  });

  /** Accepts legacy Apple Music Classical domain */
  it('accepts classical.apple.com URLs', () => {
    expect(isAppleMusicUrl('https://classical.apple.com/us/album/test/123')).toBe(true);
  });

  /** Accepts current Apple Music Classical domain (post-2026 migration) */
  it('accepts classical.music.apple.com URLs', () => {
    expect(isAppleMusicUrl('https://classical.music.apple.com/gb/album/1844602145')).toBe(true);
  });

  /** Accepts slug-less Classical URL with locale query — the real live shape */
  it('accepts classical.music.apple.com URLs with slug-less path + locale query', () => {
    expect(
      isAppleMusicUrl('https://classical.music.apple.com/gb/album/1844602145?l=en-GB')
    ).toBe(true);
  });

  /** Ensures non-Apple domains are rejected, even if the path looks valid */
  it('rejects non-Apple Music URLs', () => {
    expect(isAppleMusicUrl('https://example.com/music')).toBe(false);
    expect(isAppleMusicUrl('https://spotify.com/track/123')).toBe(false);
  });

  /** Ensures malformed input (not a URL, empty string) returns false without throwing */
  it('rejects invalid URLs', () => {
    expect(isAppleMusicUrl('not a url')).toBe(false);
    expect(isAppleMusicUrl('')).toBe(false);
  });
});

/**
 * Test suite for `parseAppleMusicUrl()` - full URL parsing and classification.
 *
 * This is the main parser function. Tests cover:
 * - All five supported content types (song, album, playlist, music-video, artist)
 * - The critical song vs. album distinction (songs are album URLs with `?i=` param)
 * - Error cases (invalid URLs, non-Apple domains)
 * - Input normalization (whitespace trimming)
 */
describe('parseAppleMusicUrl', () => {
  /**
   * Songs are a special case: they use the /album/ path segment but include
   * a `?i={trackId}` query parameter. This test verifies the parser correctly
   * identifies the `i` parameter and classifies as 'song' rather than 'album'.
   */
  it('detects songs (album URL with ?i= parameter)', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/us/album/some-song/123456?i=789');
    expect(result.contentType).toBe('song');
    expect(result.isValid).toBe(true);
  });

  /** Albums use the /album/ path segment without any `i` query parameter */
  it('detects albums', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/us/album/some-album/123456');
    expect(result.contentType).toBe('album');
    expect(result.isValid).toBe(true);
  });

  /** Playlists use the /playlist/ path segment (IDs often prefixed with `pl.`) */
  it('detects playlists', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/us/playlist/my-playlist/pl.abc123');
    expect(result.contentType).toBe('playlist');
    expect(result.isValid).toBe(true);
  });

  /** Music videos use the /music-video/ path segment (hyphenated) */
  it('detects music videos', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/us/music-video/some-video/123456');
    expect(result.contentType).toBe('music-video');
    expect(result.isValid).toBe(true);
  });

  /** Artists use the /artist/ path segment */
  it('detects artists', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/us/artist/some-artist/123456');
    expect(result.contentType).toBe('artist');
    expect(result.isValid).toBe(true);
  });

  /** Malformed input (not a URL) should return 'unknown' type and isValid: false */
  it('returns unknown for invalid URLs', () => {
    const result = parseAppleMusicUrl('not a url');
    expect(result.contentType).toBe('unknown');
    expect(result.isValid).toBe(false);
  });

  /** Valid URLs on non-Apple domains should return 'unknown' type and isValid: false */
  it('returns unknown for non-Apple Music URLs', () => {
    const result = parseAppleMusicUrl('https://example.com/music');
    expect(result.contentType).toBe('unknown');
    expect(result.isValid).toBe(false);
  });

  /**
   * Verifies whitespace trimming: users often copy URLs with leading/trailing
   * spaces from browsers, chat apps, or text editors. The parser should
   * handle this gracefully.
   */
  it('trims whitespace from input', () => {
    const result = parseAppleMusicUrl('  https://music.apple.com/us/album/test/123  ');
    expect(result.contentType).toBe('album');
    expect(result.isValid).toBe(true);
  });
});

/**
 * Test suite for non-geographic URLs (no storefront code in the path).
 *
 * Apple Music URLs can sometimes lack a storefront code, for example when
 * shared via messaging apps that strip the region segment, or from APIs
 * that return geo-non-specific URLs. The frontend parser should still
 * detect the content type correctly from the path keywords alone.
 *
 * Note: The backend handles injecting a storefront code before passing
 * to GAMDL; the frontend only needs to classify the content type.
 */
describe('non-geographic URLs (no storefront code)', () => {
  it('detects album without storefront', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/album/midnights/1649434004');
    expect(result.contentType).toBe('album');
    expect(result.isValid).toBe(true);
  });

  it('detects song without storefront (album URL with ?i= param)', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/album/midnights/1649434004?i=1649434280'
    );
    expect(result.contentType).toBe('song');
    expect(result.isValid).toBe(true);
  });

  it('detects playlist without storefront', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb'
    );
    expect(result.contentType).toBe('playlist');
    expect(result.isValid).toBe(true);
  });

  it('detects music video without storefront', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/music-video/some-video/1234567890');
    expect(result.contentType).toBe('music-video');
    expect(result.isValid).toBe(true);
  });

  it('detects artist without storefront', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/artist/taylor-swift/159260351');
    expect(result.contentType).toBe('artist');
    expect(result.isValid).toBe(true);
  });

  it('detects classical album without storefront', () => {
    const result = parseAppleMusicUrl(
      'https://classical.apple.com/album/beethoven-symphony-no-9/1234567890'
    );
    expect(result.contentType).toBe('album');
    expect(result.isValid).toBe(true);
  });

  // Apple Music Classical domain migration (2026) — Apple moved Classical
  // to `classical.music.apple.com` and dropped the slug segment from
  // Share-link URLs. The frontend parser must classify both the new
  // hostname and the slug-less path shape.

  it('classifies new classical.music.apple.com album URLs (slug-less)', () => {
    const result = parseAppleMusicUrl('https://classical.music.apple.com/gb/album/1844602145');
    expect(result.isValid).toBe(true);
    expect(result.contentType).toBe('album');
  });

  it('classifies new classical.music.apple.com album URL with ?l= locale query', () => {
    // The real-world shape captured from the Apple Music Classical app
    // Share → Copy Link, 2026-04-23.
    const result = parseAppleMusicUrl(
      'https://classical.music.apple.com/gb/album/1844602145?l=en-GB'
    );
    expect(result.isValid).toBe(true);
    expect(result.contentType).toBe('album');
  });

  it('classifies new classical.music.apple.com song URL with ?i= track id', () => {
    const result = parseAppleMusicUrl(
      'https://classical.music.apple.com/gb/album/1844602145?i=1844602150'
    );
    expect(result.isValid).toBe(true);
    // contentType is 'song' when `?i=` is present on an album URL (per detectContentType).
    expect(result.contentType).toBe('song');
  });

  it('detects song URL path without storefront', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/song/anti-hero/1649434280');
    // /song/ path is not currently a recognized content type in the frontend
    // (songs use /album/ with ?i= param), so this should be 'unknown'
    expect(result.contentType).toBe('unknown');
    expect(result.isValid).toBe(false);
  });
});

/**
 * Test suite for `getContentTypeLabel()` - content type to display label mapping.
 *
 * Ensures every possible `AppleMusicContentType` value maps to the correct
 * human-readable label string. This is a simple mapping function, but the
 * test serves as a safety net against accidental typos in the labels.
 */
describe('getContentTypeLabel', () => {
  /** Exhaustively tests all content type variants */
  it('returns correct labels for all content types', () => {
    expect(getContentTypeLabel('song')).toBe('Song');
    expect(getContentTypeLabel('album')).toBe('Album');
    expect(getContentTypeLabel('playlist')).toBe('Playlist');
    expect(getContentTypeLabel('music-video')).toBe('Music Video');
    expect(getContentTypeLabel('artist')).toBe('Artist');
    expect(getContentTypeLabel('library')).toBe('Library');
    expect(getContentTypeLabel('recording')).toBe('Classical Recording');
    expect(getContentTypeLabel('unknown')).toBe('Unknown');
  });
});

/*
 * Apple Music Classical recording URLs — see url-parser.ts for the path
 * shape documentation. The parser must:
 *   1. Recognise the content type as `recording`.
 *   2. Mark `isValid: true` so the URL can be submitted like any other
 *      Apple Music URL. Earlier behaviour (#573) rejected recording
 *      URLs on the grounds that GAMDL can't handle them, but that was
 *      reversed 2026-04-23 after reviewer feedback: the correct UX is
 *      to accept what the user pasted and let the pipeline give a
 *      clear outcome. With #567's broadened empty-output guard, a URL
 *      GAMDL can't parse fails cleanly rather than cascading into
 *      false lyrics-companion "success" lines.
 */
describe('parseAppleMusicUrl - classical recording URLs', () => {
  it('classifies recording URLs as `recording` content type', () => {
    const result = parseAppleMusicUrl(
      'https://classical.music.apple.com/gb/recording/gustav-mahler-1860-pp1-1452377808'
    );
    expect(result.contentType).toBe('recording');
  });

  it('marks recording URLs as submittable', () => {
    // Reversed from the #573 behaviour: recording URLs now pass the
    // validator so the user isn't asked to re-navigate for a different
    // link. GAMDL may still fail on the URL — that failure is handled
    // cleanly by the #567 empty-output enrichment guard.
    const result = parseAppleMusicUrl(
      'https://classical.music.apple.com/gb/recording/gustav-mahler-1860-pp1-1452377808'
    );
    expect(result.isValid).toBe(true);
  });

  it('classifies recording URL with locale query param as submittable', () => {
    // Real-world URL shape from Apple Music Classical Share → Copy Link,
    // captured 2026-04-23.
    const result = parseAppleMusicUrl(
      'https://classical.music.apple.com/gb/recording/gustav-mahler-1860-pp1-1452377808?l=en-GB'
    );
    expect(result.contentType).toBe('recording');
    expect(result.isValid).toBe(true);
  });

  it('does not misclassify album URLs as recordings', () => {
    // Regression canary — `/album/` URLs must NOT accidentally hit the
    // `recording` branch if someone refactors the detection order.
    const result = parseAppleMusicUrl(
      'https://classical.music.apple.com/gb/album/1844602145'
    );
    expect(result.contentType).toBe('album');
    expect(result.isValid).toBe(true);
  });
});

// ============================================================
// Library URL Tests
// ============================================================

describe('parseAppleMusicUrl - library URLs', () => {
  it('parses a library album URL', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/library/albums/l.8zPXbAv');
    expect(result.isValid).toBe(true);
    expect(result.contentType).toBe('library');
  });

  it('parses a library songs URL', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/library/songs');
    expect(result.isValid).toBe(true);
    expect(result.contentType).toBe('library');
  });

  it('parses a library playlists URL', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/library/playlists/p.abc123');
    expect(result.isValid).toBe(true);
    expect(result.contentType).toBe('library');
  });

  it('parses a library recently-added URL', () => {
    const result = parseAppleMusicUrl('https://music.apple.com/library/recently-added');
    expect(result.isValid).toBe(true);
    expect(result.contentType).toBe('library');
  });
});

// ============================================================
// Multi-service URL detection (#232)
// ============================================================

describe('detectService', () => {
  it('detects Apple Music URLs', () => {
    expect(detectService('https://music.apple.com/us/album/test/123')).toBe('apple-music');
  });

  it('detects Apple Music Classical URLs', () => {
    expect(detectService('https://classical.apple.com/us/album/test/123')).toBe('apple-music');
  });

  it('detects iTunes URLs', () => {
    expect(detectService('https://itunes.apple.com/lookup?id=123')).toBe('apple-music');
  });

  it('detects YouTube Music URLs', () => {
    expect(detectService('https://music.youtube.com/watch?v=abc123')).toBe('youtube-music');
  });

  it('detects YouTube URLs', () => {
    expect(detectService('https://www.youtube.com/watch?v=abc123')).toBe('youtube');
  });

  it('detects Spotify URLs', () => {
    expect(detectService('https://open.spotify.com/album/abc123')).toBe('spotify');
  });

  it('detects BBC iPlayer URLs', () => {
    expect(detectService('https://www.bbc.co.uk/iplayer/episode/abc123')).toBe('bbc-iplayer');
  });

  it('returns null for unknown URLs', () => {
    expect(detectService('https://example.com/music')).toBeNull();
  });

  it('is case-insensitive', () => {
    expect(detectService('https://MUSIC.APPLE.COM/us/album/test/123')).toBe('apple-music');
  });
});

describe('isSupportedUrl', () => {
  it('returns true for Apple Music', () => {
    expect(isSupportedUrl('https://music.apple.com/us/album/test/123')).toBe(true);
  });

  it('returns false for unknown domains', () => {
    expect(isSupportedUrl('https://example.com')).toBe(false);
  });
});

describe('parseMediaUrl', () => {
  it('parses Apple Music album URL with content type', () => {
    const result = parseMediaUrl('https://music.apple.com/us/album/test/123');
    expect(result.service).toBe('apple-music');
    expect(result.contentType).toBe('album');
    expect(result.isValid).toBe(true);
  });

  it('parses Spotify URL without content type', () => {
    const result = parseMediaUrl('https://open.spotify.com/album/abc123');
    expect(result.service).toBe('spotify');
    expect(result.contentType).toBeNull();
    expect(result.isValid).toBe(true);
  });

  it('marks unknown URLs as invalid', () => {
    const result = parseMediaUrl('https://example.com/music');
    expect(result.service).toBeNull();
    expect(result.isValid).toBe(false);
  });

  it('trims whitespace', () => {
    const result = parseMediaUrl('  https://music.apple.com/us/album/test/123  ');
    expect(result.service).toBe('apple-music');
    expect(result.isValid).toBe(true);
  });
});
