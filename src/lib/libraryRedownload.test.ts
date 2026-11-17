// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

import { describe, expect, it } from 'vitest';
import {
  GAPS_ONLY_NOTE,
  UPDATE_NEEDS_OVERWRITE,
  shouldWarnAboutSecondCopy,
  buildLibraryRedownloadRequest,
  decideLibraryRedownload,
} from './libraryRedownload';

describe('buildLibraryRedownloadRequest', () => {
  it('never asks to replace files when filling gaps', () => {
    const request = buildLibraryRedownloadRequest('https://music.apple.com/us/album/x/1', null);
    expect(request.options?.overwrite).toBe(false);
    expect(request.urls).toEqual(['https://music.apple.com/us/album/x/1']);
    expect(request.mv_companion_override).toBeNull();
  });

  it('leaves overwrite to the setting for an update', () => {
    const request = buildLibraryRedownloadRequest('u', true, 'refresh');
    expect(request.options).toBeUndefined();
    expect(request.mv_companion_override).toBe(true);
  });

  it('keeps the music-video choice', () => {
    expect(buildLibraryRedownloadRequest('u', false).mv_companion_override).toBe(false);
  });
});

describe('decideLibraryRedownload', () => {
  it('fills gaps without overwriting, whatever the setting', () => {
    expect(decideLibraryRedownload({ hasGaps: true, updated: false }, true)).toEqual({
      kind: 'fill_gaps',
    });
    expect(decideLibraryRedownload({ hasGaps: true, updated: false }, false)).toEqual({
      kind: 'fill_gaps',
    });
  });

  it('says updated songs are not replaced when a row has gaps and an update', () => {
    expect(decideLibraryRedownload({ hasGaps: true, updated: true }, true)).toEqual({
      kind: 'fill_gaps',
      note: GAPS_ONLY_NOTE,
    });
  });

  it('follows the Overwrite setting for an update only', () => {
    expect(decideLibraryRedownload({ hasGaps: false, updated: true }, true)).toEqual({
      kind: 'refresh',
    });
    expect(decideLibraryRedownload({ hasGaps: false, updated: true }, false)).toEqual({
      kind: 'explain',
      message: UPDATE_NEEDS_OVERWRITE,
    });
  });
});

describe('shouldWarnAboutSecondCopy', () => {
  it('warns only when names were changed and something would be queued', () => {
    expect(shouldWarnAboutSecondCopy({ kind: 'fill_gaps' }, true)).toBe(true);
    expect(shouldWarnAboutSecondCopy({ kind: 'refresh' }, true)).toBe(true);
    expect(shouldWarnAboutSecondCopy({ kind: 'fill_gaps' }, false)).toBe(false);
    expect(
      shouldWarnAboutSecondCopy({ kind: 'explain', message: UPDATE_NEEDS_OVERWRITE }, true)
    ).toBe(false);
  });

  it('never promises that updated songs are replaced when filling gaps', () => {
    expect(GAPS_ONLY_NOTE).toMatch(/kept, not replaced/);
    expect(UPDATE_NEEDS_OVERWRITE).toMatch(/Overwrite Existing Files/);
  });
});
