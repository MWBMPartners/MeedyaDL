// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for the error-message helpers (polish pass M8).
 */

import { beforeEach, describe, expect, it } from 'vitest';
import { isCancellation, rawError, showError, summariseDownloadError } from './errorMessages';
import { useUiStore } from '@/stores/uiStore';

const TRACEBACK =
  'GAMDL bug — Traceback (most recent call last): File "/Users/demo/Library/Application Support/com.meedyasuite.meedyadl/python/lib/python3.12/site-packages/gamdl/interface/song.py", line 412, in get_stream_info raise GamdlInterfaceMediaNotStreamableError("Media is not streamable: 1646756971") httpx.ConnectError: [Errno 60] Operation timed out while connecting to amp-api.music.apple.com:443';

beforeEach(() => {
  useUiStore.setState({ toasts: [] });
});

describe('showError', () => {
  it('shows the plain message and folds the technical text under Details', () => {
    showError('MeedyaDL could not open the logs folder. Check it still exists, then try again.', new Error('No such file or directory (os error 2)'));
    const [toast] = useUiStore.getState().toasts;
    expect(toast.type).toBe('error');
    expect(toast.message).toBe('MeedyaDL could not open the logs folder. Check it still exists, then try again.');
    expect(toast.details).toBe('No such file or directory (os error 2)');
  });

  it('leaves Details out when there is nothing technical to add', () => {
    showError('That link is not one MeedyaDL can download. Copy the link from Apple Music and try again.');
    expect(useUiStore.getState().toasts[0].details).toBeUndefined();
  });

  it('can show a warning instead of an error', () => {
    showError('Some links were skipped. Check them, then add them again.', undefined, { type: 'warning' });
    expect(useUiStore.getState().toasts[0].type).toBe('warning');
  });
});

describe('rawError and isCancellation', () => {
  it('reads an Error, a string, or anything else', () => {
    expect(rawError(new Error('boom'))).toBe('boom');
    expect(rawError('plain')).toBe('plain');
    expect(rawError({ code: 5 })).toBe('{"code":5}');
  });

  it('recognises a closed file picker in the wordings the backend uses', () => {
    for (const text of ['Folder selection cancelled', 'Export cancelled', 'Import cancelled', 'User canceled']) {
      expect(isCancellation(new Error(text))).toBe(true);
    }
    expect(isCancellation(new Error('Permission denied'))).toBe(false);
  });
});

describe('summariseDownloadError', () => {
  it('keeps a message that is already a plain sentence, with nothing folded away', () => {
    const plain = 'Every track in this album is already on disk, so there is nothing to retry.';
    expect(summariseDownloadError(plain)).toEqual({ summary: plain, details: null });
  });

  it('replaces a traceback with a plain summary and keeps the original as details', () => {
    const { summary, details } = summariseDownloadError(TRACEBACK);
    expect(summary).toBe('MeedyaDL could not reach Apple Music. Check your internet connection, then retry.');
    expect(details).toBe(TRACEBACK);
  });

  it('treats a disk problem as a disk problem, even though macOS calls it a time-out', () => {
    expect(summariseDownloadError('OSError: [Errno 28] No space left on device').summary).toMatch(/^MeedyaDL could not save the files/);
  });

  it('summarises sign-in, region and slow-down problems', () => {
    expect(summariseDownloadError('httpx.HTTPStatusError: 401 Unauthorized').summary).toMatch(/did not accept your sign-in/);
    expect(summariseDownloadError('GamdlInterfaceMediaNotStreamableError: Media is not streamable').summary).toMatch(/does not offer this item/);
    expect(summariseDownloadError('httpx.HTTPStatusError: 429 Too Many Requests').summary).toMatch(/slow down/);
  });

  it("keeps the backend's own explanation of a known GAMDL fault, marker and all", () => {
    const curated = 'GAMDL bug — all 3 probed file(s) appear corrupted or truncated — a known upstream truncated-write defect (gamdl#328). Try re-downloading.';
    expect(summariseDownloadError(curated)).toEqual({ summary: curated, details: null });
  });

  it('a GAMDL fault with no recognisable cause points at the report menu', () => {
    const { summary } = summariseDownloadError('GAMDL bug — KeyError: playParams');
    expect(summary).toMatch(/fault of its own/);
    expect(summary).toMatch(/report it/);
  });

  it('a long message is summarised even when it reads like prose', () => {
    const long = 'Something unusual happened while the album was being put together. '.repeat(9);
    expect(summariseDownloadError(long).details).toBe(long.trim());
  });

  it('every summary ends with something to do', () => {
    for (const text of [TRACEBACK, 'OSError: [Errno 28] No space left', 'GAMDL bug — KeyError: x', 'RuntimeError: weird']) {
      expect(summariseDownloadError(text).summary).toMatch(/(retry|Retry|report it|Check that the link)/);
    }
  });
});
