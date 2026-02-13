/**
 * Copyright (c) 2024-2026 MeedyaDL
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
import {
  parseAppleMusicUrl,
  isAppleMusicUrl,
  getContentTypeLabel,
} from './url-parser';

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
    expect(isAppleMusicUrl('https://music.apple.com/us/album/test/123')).toBe(
      true,
    );
  });

  /** Validates that the legacy iTunes domain is also accepted (backward compatibility) */
  it('accepts itunes.apple.com URLs', () => {
    expect(
      isAppleMusicUrl('https://itunes.apple.com/us/album/test/123'),
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
    const result = parseAppleMusicUrl(
      'https://music.apple.com/us/album/some-song/123456?i=789',
    );
    expect(result.contentType).toBe('song');
    expect(result.isValid).toBe(true);
  });

  /** Albums use the /album/ path segment without any `i` query parameter */
  it('detects albums', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/us/album/some-album/123456',
    );
    expect(result.contentType).toBe('album');
    expect(result.isValid).toBe(true);
  });

  /** Playlists use the /playlist/ path segment (IDs often prefixed with `pl.`) */
  it('detects playlists', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/us/playlist/my-playlist/pl.abc123',
    );
    expect(result.contentType).toBe('playlist');
    expect(result.isValid).toBe(true);
  });

  /** Music videos use the /music-video/ path segment (hyphenated) */
  it('detects music videos', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/us/music-video/some-video/123456',
    );
    expect(result.contentType).toBe('music-video');
    expect(result.isValid).toBe(true);
  });

  /** Artists use the /artist/ path segment */
  it('detects artists', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/us/artist/some-artist/123456',
    );
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
    const result = parseAppleMusicUrl(
      '  https://music.apple.com/us/album/test/123  ',
    );
    expect(result.contentType).toBe('album');
    expect(result.isValid).toBe(true);
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
  /** Exhaustively tests all six content type variants */
  it('returns correct labels for all content types', () => {
    expect(getContentTypeLabel('song')).toBe('Song');
    expect(getContentTypeLabel('album')).toBe('Album');
    expect(getContentTypeLabel('playlist')).toBe('Playlist');
    expect(getContentTypeLabel('music-video')).toBe('Music Video');
    expect(getContentTypeLabel('artist')).toBe('Artist');
    expect(getContentTypeLabel('unknown')).toBe('Unknown');
  });
});
