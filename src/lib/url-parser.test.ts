/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Tests for the Apple Music URL parser.
 * Verifies URL validation, content type detection, and label generation
 * for all supported Apple Music URL formats.
 */

import { describe, it, expect } from 'vitest';
import {
  parseAppleMusicUrl,
  isAppleMusicUrl,
  getContentTypeLabel,
} from './url-parser';

describe('isAppleMusicUrl', () => {
  it('accepts music.apple.com URLs', () => {
    expect(isAppleMusicUrl('https://music.apple.com/us/album/test/123')).toBe(
      true,
    );
  });

  it('accepts itunes.apple.com URLs', () => {
    expect(
      isAppleMusicUrl('https://itunes.apple.com/us/album/test/123'),
    ).toBe(true);
  });

  it('rejects non-Apple Music URLs', () => {
    expect(isAppleMusicUrl('https://example.com/music')).toBe(false);
    expect(isAppleMusicUrl('https://spotify.com/track/123')).toBe(false);
  });

  it('rejects invalid URLs', () => {
    expect(isAppleMusicUrl('not a url')).toBe(false);
    expect(isAppleMusicUrl('')).toBe(false);
  });
});

describe('parseAppleMusicUrl', () => {
  it('detects songs (album URL with ?i= parameter)', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/us/album/some-song/123456?i=789',
    );
    expect(result.contentType).toBe('song');
    expect(result.isValid).toBe(true);
  });

  it('detects albums', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/us/album/some-album/123456',
    );
    expect(result.contentType).toBe('album');
    expect(result.isValid).toBe(true);
  });

  it('detects playlists', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/us/playlist/my-playlist/pl.abc123',
    );
    expect(result.contentType).toBe('playlist');
    expect(result.isValid).toBe(true);
  });

  it('detects music videos', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/us/music-video/some-video/123456',
    );
    expect(result.contentType).toBe('music-video');
    expect(result.isValid).toBe(true);
  });

  it('detects artists', () => {
    const result = parseAppleMusicUrl(
      'https://music.apple.com/us/artist/some-artist/123456',
    );
    expect(result.contentType).toBe('artist');
    expect(result.isValid).toBe(true);
  });

  it('returns unknown for invalid URLs', () => {
    const result = parseAppleMusicUrl('not a url');
    expect(result.contentType).toBe('unknown');
    expect(result.isValid).toBe(false);
  });

  it('returns unknown for non-Apple Music URLs', () => {
    const result = parseAppleMusicUrl('https://example.com/music');
    expect(result.contentType).toBe('unknown');
    expect(result.isValid).toBe(false);
  });

  it('trims whitespace from input', () => {
    const result = parseAppleMusicUrl(
      '  https://music.apple.com/us/album/test/123  ',
    );
    expect(result.contentType).toBe('album');
    expect(result.isValid).toBe(true);
  });
});

describe('getContentTypeLabel', () => {
  it('returns correct labels for all content types', () => {
    expect(getContentTypeLabel('song')).toBe('Song');
    expect(getContentTypeLabel('album')).toBe('Album');
    expect(getContentTypeLabel('playlist')).toBe('Playlist');
    expect(getContentTypeLabel('music-video')).toBe('Music Video');
    expect(getContentTypeLabel('artist')).toBe('Artist');
    expect(getContentTypeLabel('unknown')).toBe('Unknown');
  });
});
