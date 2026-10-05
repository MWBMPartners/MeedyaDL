// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for the progress-bar caption (polish pass, L5).
 *
 * The caption used to start "DOWNLOADING..." in capitals with no space
 * before the name ("DOWNLOADING...Wolfgang Amadeus Mozart").
 */

import { describe, expect, it } from 'vitest';
import { formatActiveItemCaption } from './progress-caption';
import { makeQueueItem } from '@/testing/fixtures';

describe('formatActiveItemCaption', () => {
  it('says "Downloading:" in sentence case, with a space before the name', () => {
    const caption = formatActiveItemCaption(
      makeQueueItem({ state: 'downloading', artist_name: 'Mozart', album_name: 'Requiem', current_track: 'Lacrimosa' }),
    );
    expect(caption).toBe('Downloading: Mozart — Requiem — "Lacrimosa"');
    expect(caption).not.toMatch(/DOWNLOADING|\.\.\./);
  });

  it('uses the same wording when only the album is known', () => {
    const caption = formatActiveItemCaption(makeQueueItem({ state: 'downloading', album_name: 'Requiem', current_track: null }));
    expect(caption).toBe('Downloading: Requiem');
  });

  it('adds nothing when the item is not downloading', () => {
    const caption = formatActiveItemCaption(makeQueueItem({ state: 'queued', album_name: 'Requiem', current_track: null }));
    expect(caption).toBe('Requiem');
  });
});
