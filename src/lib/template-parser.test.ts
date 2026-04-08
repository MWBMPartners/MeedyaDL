/**
 * Copyright (c) 2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file template-parser.test.ts -- Tests for the GAMDL template parser/serializer.
 */

import { describe, expect, it } from 'vitest';

import {
  getVariableLabel,
  parseTemplate,
  renderPreview,
  serializeTemplate,
} from './template-parser';

// ============================================================
// parseTemplate
// ============================================================

describe('parseTemplate', () => {
  it('parses a simple variable', () => {
    expect(parseTemplate('{artist}')).toEqual([{ type: 'variable', value: 'artist' }]);
  });

  it('parses a variable with format specifier', () => {
    expect(parseTemplate('{track:02d}')).toEqual([{ type: 'variable', value: 'track:02d' }]);
  });

  it('parses literal text and splits known separators', () => {
    expect(parseTemplate('Compilations/')).toEqual([
      { type: 'literal', value: 'Compilations' },
      { type: 'literal', value: '/' },
    ]);
  });

  it('parses mixed variable and literal', () => {
    expect(parseTemplate('{album_artist}/{album}')).toEqual([
      { type: 'variable', value: 'album_artist' },
      { type: 'literal', value: '/' },
      { type: 'variable', value: 'album' },
    ]);
  });

  it('parses complex multi-variable template', () => {
    expect(parseTemplate('{disc}-{track:02d} {title}')).toEqual([
      { type: 'variable', value: 'disc' },
      { type: 'literal', value: '-' },
      { type: 'variable', value: 'track:02d' },
      { type: 'literal', value: ' ' },
      { type: 'variable', value: 'title' },
    ]);
  });

  it('parses leading literal text with variable', () => {
    expect(parseTemplate('Compilations/{album}')).toEqual([
      { type: 'literal', value: 'Compilations' },
      { type: 'literal', value: '/' },
      { type: 'variable', value: 'album' },
    ]);
  });

  it('parses empty string', () => {
    expect(parseTemplate('')).toEqual([]);
  });

  it('parses pure literal and splits known separators', () => {
    expect(parseTemplate('Unknown Album')).toEqual([
      { type: 'literal', value: 'Unknown' },
      { type: 'literal', value: ' ' },
      { type: 'literal', value: 'Album' },
    ]);
  });

  it('handles unclosed brace gracefully', () => {
    const result = parseTemplate('{broken');
    // The unclosed brace is treated as literal text
    expect(result).toEqual([{ type: 'literal', value: '{broken' }]);
  });

  it('handles multiple path segments', () => {
    expect(parseTemplate('Playlists/{playlist_artist}/{playlist_title}')).toEqual([
      { type: 'literal', value: 'Playlists' },
      { type: 'literal', value: '/' },
      { type: 'variable', value: 'playlist_artist' },
      { type: 'literal', value: '/' },
      { type: 'variable', value: 'playlist_title' },
    ]);
  });

  it('splits dash separator into atomic chips', () => {
    // " - " is split into 3 atomic chips (space, hyphen, space) to prevent
    // roundtrip ambiguity when users build separators from individual chips
    expect(parseTemplate('{track:02d} - {title}')).toEqual([
      { type: 'variable', value: 'track:02d' },
      { type: 'literal', value: ' ' },
      { type: 'literal', value: '-' },
      { type: 'literal', value: ' ' },
      { type: 'variable', value: 'title' },
    ]);
  });

  it('preserves multiple adjacent separators through roundtrip', () => {
    // Space + Hyphen + Space must survive serialize→re-parse as 3 chips
    expect(parseTemplate('{disc} - {track:02d} - {title}')).toEqual([
      { type: 'variable', value: 'disc' },
      { type: 'literal', value: ' ' },
      { type: 'literal', value: '-' },
      { type: 'literal', value: ' ' },
      { type: 'variable', value: 'track:02d' },
      { type: 'literal', value: ' ' },
      { type: 'literal', value: '-' },
      { type: 'literal', value: ' ' },
      { type: 'variable', value: 'title' },
    ]);
  });

  it('parses adjacent variables without separator', () => {
    expect(parseTemplate('{artist}{title}')).toEqual([
      { type: 'variable', value: 'artist' },
      { type: 'variable', value: 'title' },
    ]);
  });
});

// ============================================================
// serializeTemplate
// ============================================================

describe('serializeTemplate', () => {
  it('serializes variable segments with braces', () => {
    expect(serializeTemplate([{ type: 'variable', value: 'artist' }])).toBe('{artist}');
  });

  it('serializes literal segments as-is', () => {
    expect(serializeTemplate([{ type: 'literal', value: 'Compilations/' }])).toBe(
      'Compilations/',
    );
  });

  it('serializes mixed segments', () => {
    expect(
      serializeTemplate([
        { type: 'variable', value: 'album_artist' },
        { type: 'literal', value: '/' },
        { type: 'variable', value: 'album' },
      ]),
    ).toBe('{album_artist}/{album}');
  });

  it('serializes empty array to empty string', () => {
    expect(serializeTemplate([])).toBe('');
  });
});

// ============================================================
// Roundtrip: parseTemplate -> serializeTemplate
// ============================================================

describe('roundtrip', () => {
  const defaultTemplates = [
    '{album_artist}/{album}',
    'Compilations/{album}',
    '{artist}/[Unknown]',
    '{track:02d} {title}',
    '{disc}-{track:02d} {title}',
    '{title}',
    'Playlists/{playlist_artist}/{playlist_title}',
  ];

  defaultTemplates.forEach((template) => {
    it(`roundtrips: ${template}`, () => {
      expect(serializeTemplate(parseTemplate(template))).toBe(template);
    });
  });

  it('roundtrips empty string', () => {
    expect(serializeTemplate(parseTemplate(''))).toBe('');
  });
});

// ============================================================
// getVariableLabel
// ============================================================

describe('getVariableLabel', () => {
  it('returns label for known variable', () => {
    expect(getVariableLabel('artist')).toBe('Artist');
    expect(getVariableLabel('track:02d')).toBe('Track #');
    expect(getVariableLabel('playlist_title')).toBe('Playlist Title');
  });

  it('returns raw value wrapped in braces for unknown variable', () => {
    expect(getVariableLabel('unknown_var')).toBe('{unknown_var}');
  });
});

// ============================================================
// renderPreview
// ============================================================

describe('renderPreview', () => {
  it('substitutes sample metadata for variables', () => {
    const segments = parseTemplate('{album_artist}/{album}');
    expect(renderPreview(segments)).toBe("Taylor Swift/1989 (Taylor's Version)");
  });

  it('preserves literal text', () => {
    const segments = parseTemplate('Compilations/{album}');
    expect(renderPreview(segments)).toBe("Compilations/1989 (Taylor's Version)");
  });

  it('falls back to braces for unknown variables', () => {
    const segments = parseTemplate('{unknown}');
    expect(renderPreview(segments)).toBe('{unknown}');
  });

  it('renders file template with track number', () => {
    const segments = parseTemplate('{track:02d} {title}');
    expect(renderPreview(segments)).toBe('03 Style');
  });

  it('renders empty segments as empty string', () => {
    expect(renderPreview([])).toBe('');
  });
});
