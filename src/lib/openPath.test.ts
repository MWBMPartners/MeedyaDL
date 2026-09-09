// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for openPath.
 *
 * The behaviour worth protecting is the failure path. Three buttons used
 * to swallow every error and do nothing visible, so a folder the user had
 * moved or deleted produced a click that appeared to break the app. These
 * tests fail if that silence ever comes back.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';

const openMock = vi.fn();
vi.mock('@tauri-apps/plugin-shell', () => ({ open: (p: string) => openMock(p) }));

const addToastMock = vi.fn();
vi.mock('@/stores/uiStore', () => ({
  useUiStore: { getState: () => ({ addToast: addToastMock }) },
}));

import { openContainingFolder, openDownloadedFile } from './openPath';

describe('openPath', () => {
  beforeEach(() => {
    openMock.mockReset();
    addToastMock.mockReset();
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  describe('when the path opens', () => {
    it('opens an album folder as given, without stripping anything', async () => {
      openMock.mockResolvedValue(undefined);
      const ok = await openContainingFolder('/Users/me/Music/Artist/Album', true);
      expect(ok).toBe(true);
      expect(openMock).toHaveBeenCalledWith('/Users/me/Music/Artist/Album');
      expect(addToastMock).not.toHaveBeenCalled();
    });

    it('opens the containing folder when given a file', async () => {
      openMock.mockResolvedValue(undefined);
      await openContainingFolder('/Users/me/Music/Artist/Album/01 Track.m4a', false);
      expect(openMock).toHaveBeenCalledWith('/Users/me/Music/Artist/Album');
    });

    it('handles Windows paths by their own separator, not the host machine\'s', async () => {
      // A queue file exported on Windows and imported on a Mac still has
      // backslashes in it, so the separator has to come from the path.
      openMock.mockResolvedValue(undefined);
      await openContainingFolder('C:\\Users\\me\\Music\\Album\\01 Track.m4a', false);
      expect(openMock).toHaveBeenCalledWith('C:\\Users\\me\\Music\\Album');
    });

    it('opens a file as given', async () => {
      openMock.mockResolvedValue(undefined);
      const ok = await openDownloadedFile('/Users/me/Music/Artist/Album/01 Track.m4a');
      expect(ok).toBe(true);
      expect(openMock).toHaveBeenCalledWith('/Users/me/Music/Artist/Album/01 Track.m4a');
    });
  });

  describe('when the path cannot be opened — the behaviour this file exists for', () => {
    it('tells the user, rather than failing silently', async () => {
      openMock.mockRejectedValue(new Error('No such file or directory'));
      const ok = await openContainingFolder('/Users/me/Music/Gone', true);

      expect(ok).toBe(false);
      expect(addToastMock).toHaveBeenCalledTimes(1);
      const [message, variant] = addToastMock.mock.calls[0];
      expect(variant).toBe('error');
      expect(message).toContain('could not be opened');
      // The path is in the message because it is the thing the person can act on.
      expect(message).toContain('/Users/me/Music/Gone');
    });

    it('says "folder" for a folder and "file" for a file', async () => {
      openMock.mockRejectedValue(new Error('nope'));

      await openContainingFolder('/some/folder', true);
      expect(addToastMock.mock.calls[0][0]).toContain('That folder');

      addToastMock.mockReset();
      await openDownloadedFile('/some/file.m4a');
      expect(addToastMock.mock.calls[0][0]).toContain('That file');
    });

    it('does not claim to know why it failed', async () => {
      // The likely causes are not reliably distinguishable from the error,
      // and a confident wrong guess is worse than an honest general one.
      openMock.mockRejectedValue(new Error('EACCES: permission denied'));
      await openDownloadedFile('/some/file.m4a');
      const message = addToastMock.mock.calls[0][0] as string;
      expect(message).toContain('may have been');
      expect(message).not.toContain('permission denied');
    });

    it('replaces its own message instead of stacking copies', async () => {
      // Clicking a dead button five times should leave one message, not five.
      openMock.mockRejectedValue(new Error('nope'));
      await openDownloadedFile('/a.m4a');
      await openDownloadedFile('/a.m4a');
      const firstKey = addToastMock.mock.calls[0][3];
      const secondKey = addToastMock.mock.calls[1][3];
      expect(firstKey).toBeTruthy();
      expect(secondKey).toBe(firstKey);
    });
  });
});
