// Copyright (c) 2024-2026 MeedyaSuite
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

// `openContainingFolder` reveals (selects the folder in its parent) and
// `openDownloadedFile` opens (launches the file in its own app) -- see
// the file header for why these went from the shell plugin to the
// opener plugin, and why they take two different routes.
//
// Revealing still calls the opener plugin directly: it only selects the
// item in the file manager, and runs nothing.
//
// Opening goes to the backend instead. It used to call the plugin's own
// "open this path", with permission to open ANY path -- and on Windows,
// opening a program runs it, so anything that could run code in this
// page could run any file on the computer. The test below pins the
// route, so putting the direct call back fails here rather than quietly
// needing that permission again.
const revealItemInDirMock = vi.fn();
const openPathMock = vi.fn();
vi.mock('@tauri-apps/plugin-opener', () => ({
  revealItemInDir: (p: string) => revealItemInDirMock(p),
  openPath: (p: string) => openPathMock(p),
}));

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

const addToastMock = vi.fn();
vi.mock('@/stores/uiStore', () => ({
  useUiStore: { getState: () => ({ addToast: addToastMock }) },
}));

import { openContainingFolder, openDownloadedFile } from './openPath';

describe('openPath', () => {
  beforeEach(() => {
    revealItemInDirMock.mockReset();
    openPathMock.mockReset();
    invokeMock.mockReset();
    addToastMock.mockReset();
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  describe('when the path opens', () => {
    it('reveals an album folder as given, without stripping anything', async () => {
      revealItemInDirMock.mockResolvedValue(undefined);
      const ok = await openContainingFolder('/Users/me/Music/Artist/Album', true);
      expect(ok).toBe(true);
      expect(revealItemInDirMock).toHaveBeenCalledWith('/Users/me/Music/Artist/Album');
      expect(addToastMock).not.toHaveBeenCalled();
    });

    it('reveals the containing folder when given a file', async () => {
      revealItemInDirMock.mockResolvedValue(undefined);
      await openContainingFolder('/Users/me/Music/Artist/Album/01 Track.m4a', false);
      expect(revealItemInDirMock).toHaveBeenCalledWith('/Users/me/Music/Artist/Album');
    });

    it('handles Windows paths by their own separator, not the host machine\'s', async () => {
      // A queue file exported on Windows and imported on a Mac still has
      // backslashes in it, so the separator has to come from the path.
      revealItemInDirMock.mockResolvedValue(undefined);
      await openContainingFolder('C:\\Users\\me\\Music\\Album\\01 Track.m4a', false);
      expect(revealItemInDirMock).toHaveBeenCalledWith('C:\\Users\\me\\Music\\Album');
    });

    it('opens a file as given, through the backend rather than the plugin', async () => {
      invokeMock.mockResolvedValue(undefined);
      const ok = await openDownloadedFile('/Users/me/Music/Artist/Album/01 Track.m4a');
      expect(ok).toBe(true);
      expect(invokeMock).toHaveBeenCalledWith('open_downloaded_file', {
        filePath: '/Users/me/Music/Artist/Album/01 Track.m4a',
      });
      // The whole point of the change: the page no longer opens paths
      // itself, so the app no longer needs permission to open any path.
      expect(openPathMock).not.toHaveBeenCalled();
    });
  });

  describe('when the path cannot be opened — the behaviour this file exists for', () => {
    it('tells the user, rather than failing silently', async () => {
      revealItemInDirMock.mockRejectedValue(new Error('No such file or directory'));
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
      revealItemInDirMock.mockRejectedValue(new Error('nope'));
      invokeMock.mockRejectedValue(new Error('nope'));

      await openContainingFolder('/some/folder', true);
      expect(addToastMock.mock.calls[0][0]).toContain('That folder');

      addToastMock.mockReset();
      await openDownloadedFile('/some/file.m4a');
      expect(addToastMock.mock.calls[0][0]).toContain('That file');
    });

    it('does not claim to know why it failed', async () => {
      // The likely causes are not reliably distinguishable from the error,
      // and a confident wrong guess is worse than an honest general one.
      invokeMock.mockRejectedValue(new Error('EACCES: permission denied'));
      await openDownloadedFile('/some/file.m4a');
      const message = addToastMock.mock.calls[0][0] as string;
      expect(message).toContain('may have been');
      expect(message).not.toContain('permission denied');
    });

    it('replaces its own message instead of stacking copies', async () => {
      // Clicking a dead button five times should leave one message, not five.
      invokeMock.mockRejectedValue(new Error('nope'));
      await openDownloadedFile('/a.m4a');
      await openDownloadedFile('/a.m4a');
      const firstKey = addToastMock.mock.calls[0][3];
      const secondKey = addToastMock.mock.calls[1][3];
      expect(firstKey).toBeTruthy();
      expect(secondKey).toBe(firstKey);
    });
  });
});
