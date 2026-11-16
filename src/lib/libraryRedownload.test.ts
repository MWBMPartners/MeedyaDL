// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

import { describe, expect, it } from 'vitest';
import { buildLibraryRedownloadRequest } from './libraryRedownload';

describe('buildLibraryRedownloadRequest', () => {
  it('never asks to replace files already there', () => {
    const request = buildLibraryRedownloadRequest('https://music.apple.com/us/album/x/1', null);
    expect(request.options?.overwrite).toBe(false);
    expect(request.urls).toEqual(['https://music.apple.com/us/album/x/1']);
    expect(request.mv_companion_override).toBeNull();
  });

  it('keeps the music-video choice', () => {
    expect(buildLibraryRedownloadRequest('u', true).mv_companion_override).toBe(true);
    expect(buildLibraryRedownloadRequest('u', false).mv_companion_override).toBe(false);
  });
});
