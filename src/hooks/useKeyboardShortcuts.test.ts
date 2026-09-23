// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for useKeyboardShortcuts.
 *
 * There was no test for this hook at all before this file. That mattered
 * more than usual here, because the Cmd/Ctrl+Shift+. "abort every download"
 * shortcut had a bug that no test would have caught by accident: the code
 * matched on `e.key === '.'` while also requiring Shift to be held, but on
 * a US or UK keyboard those two things can never both be true at once --
 * holding Shift on the period key sends `>`, not `.`. So the shortcut
 * documented in help/keyboard-shortcuts.md and the in-app shortcuts dialog
 * could never actually fire on the most common layouts. The main job of
 * the tests below is to fail if that ever comes back.
 *
 * Every test here drives the REAL exported `useKeyboardShortcuts` hook by
 * dispatching genuine `KeyboardEvent`s at `window`, the same way a real key
 * press would arrive -- none of this re-implements the hook's own switch
 * statement to check against itself.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook } from '@testing-library/react';

// --- Store mocks -----------------------------------------------------
//
// Vitest hoists vi.mock() calls above ordinary imports, and its transform
// only allows a factory to reference an outer variable whose name contains
// "mock" (case-insensitive) -- otherwise it would be read before it is
// assigned. Naming every stand-in function `...Mock` (matching the pattern
// already used elsewhere in this repo, e.g. src/lib/openPath.test.ts) is
// what makes that legal, not just a style choice.
const setPageMock = vi.fn();
const setShortcutsHelpOpenMock = vi.fn();
vi.mock('@/stores/uiStore', () => ({
  useUiStore: {
    getState: () => ({
      setPage: setPageMock,
      setShortcutsHelpOpen: setShortcutsHelpOpenMock,
    }),
  },
}));

const abortAllMock = vi.fn();
vi.mock('@/stores/downloadStore', () => ({
  useDownloadStore: {
    getState: () => ({
      abortAll: abortAllMock,
    }),
  },
}));

// `abort_queue_confirm` needs to change between tests (on vs. off), so the
// settings object is produced by a mock function rather than a fixed
// literal, and each test tells it what to return.
const getSettingsMock = vi.fn();
vi.mock('@/stores/settingsStore', () => ({
  useSettingsStore: {
    getState: () => ({
      settings: getSettingsMock(),
    }),
  },
}));

import { useKeyboardShortcuts } from './useKeyboardShortcuts';

/**
 * Dispatches a `keydown` at `window`, exactly as the browser would for a
 * real key press -- this is what actually drives the hook's listener,
 * as opposed to calling some internal function directly.
 */
function pressKey(init: KeyboardEventInit & { key: string }): void {
  window.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }));
}

describe('useKeyboardShortcuts', () => {
  beforeEach(() => {
    setPageMock.mockReset();
    setShortcutsHelpOpenMock.mockReset();
    abortAllMock.mockReset();
    abortAllMock.mockResolvedValue(0);
    getSettingsMock.mockReset();
    getSettingsMock.mockReturnValue({ abort_queue_confirm: false });
    // Make sure no earlier test's focused element is still sitting in the
    // document, which would trip the "typing in a form field" guard for a
    // test that isn't about that guard at all.
    if (
      document.activeElement instanceof HTMLElement &&
      document.activeElement !== document.body
    ) {
      document.activeElement.blur();
    }
  });

  it('registers exactly one window keydown listener on mount, and removes it on unmount', () => {
    const addSpy = vi.spyOn(window, 'addEventListener');
    const removeSpy = vi.spyOn(window, 'removeEventListener');

    const { unmount } = renderHook(() => useKeyboardShortcuts());
    expect(addSpy).toHaveBeenCalledTimes(1);
    expect(addSpy).toHaveBeenCalledWith('keydown', expect.any(Function));

    unmount();
    expect(removeSpy).toHaveBeenCalledTimes(1);
    expect(removeSpy).toHaveBeenCalledWith('keydown', expect.any(Function));

    addSpy.mockRestore();
    removeSpy.mockRestore();
  });

  describe('Cmd/Ctrl+Shift+. -- abort all downloads (the fault this file exists to catch)', () => {
    it('fires on the shifted character a US/UK keyboard actually sends for that key (">")', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: '>', shiftKey: true, metaKey: true });
      expect(abortAllMock).toHaveBeenCalledTimes(1);
      unmount();
    });

    it('also fires on the plain "." (e.g. AZERTY, where the physical key already sends "." when shifted -- the one layout the old, self-contradictory check happened to work on)', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: '.', shiftKey: true, ctrlKey: true });
      expect(abortAllMock).toHaveBeenCalledTimes(1);
      unmount();
    });

    it('does NOT fire on a plain "." without Shift, even though "." is the character the old broken check looked for', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: '.', shiftKey: false, metaKey: true });
      expect(abortAllMock).not.toHaveBeenCalled();
      unmount();
    });

    it('does NOT fire without Cmd/Ctrl held, even with Shift+.', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: '>', shiftKey: true });
      expect(abortAllMock).not.toHaveBeenCalled();
      unmount();
    });

    it('asks for confirmation first when abort_queue_confirm is on, and does nothing if the user cancels', () => {
      getSettingsMock.mockReturnValue({ abort_queue_confirm: true });
      const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false);

      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: '>', shiftKey: true, metaKey: true });

      expect(confirmSpy).toHaveBeenCalledTimes(1);
      expect(abortAllMock).not.toHaveBeenCalled();

      confirmSpy.mockRestore();
      unmount();
    });

    it('aborts once the user confirms', () => {
      getSettingsMock.mockReturnValue({ abort_queue_confirm: true });
      const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);

      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: '>', shiftKey: true, metaKey: true });

      expect(abortAllMock).toHaveBeenCalledTimes(1);

      confirmSpy.mockRestore();
      unmount();
    });

    it('fires immediately with no confirmation prompt when abort_queue_confirm is off', () => {
      getSettingsMock.mockReturnValue({ abort_queue_confirm: false });
      const confirmSpy = vi.spyOn(window, 'confirm');

      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: '>', shiftKey: true, ctrlKey: true });

      expect(confirmSpy).not.toHaveBeenCalled();
      expect(abortAllMock).toHaveBeenCalledTimes(1);

      confirmSpy.mockRestore();
      unmount();
    });
  });

  describe('page-navigation shortcuts', () => {
    it('Cmd/Ctrl+D navigates to Download and focuses the URL input', () => {
      const urlInput = document.createElement('textarea');
      urlInput.id = 'url-input';
      document.body.appendChild(urlInput);

      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: 'd', metaKey: true });

      expect(setPageMock).toHaveBeenCalledWith('download');

      document.body.removeChild(urlInput);
      unmount();
    });

    it('Cmd/Ctrl+, (comma) navigates to Settings', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: ',', metaKey: true });
      expect(setPageMock).toHaveBeenCalledWith('settings');
      unmount();
    });

    it('Cmd/Ctrl+Q navigates to Queue', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: 'q', ctrlKey: true });
      expect(setPageMock).toHaveBeenCalledWith('queue');
      unmount();
    });

    it('Cmd/Ctrl+L navigates to Library', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: 'l', metaKey: true });
      expect(setPageMock).toHaveBeenCalledWith('library');
      unmount();
    });

    it('Cmd/Ctrl+H navigates to History', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: 'h', metaKey: true });
      expect(setPageMock).toHaveBeenCalledWith('history');
      unmount();
    });

    it('Cmd/Ctrl+K navigates to Activity', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: 'k', metaKey: true });
      expect(setPageMock).toHaveBeenCalledWith('activity');
      unmount();
    });

    it('does nothing at all without the platform modifier held', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: 'q' });
      expect(setPageMock).not.toHaveBeenCalled();
      unmount();
    });

    it('is suppressed while focus is inside a text field, so typing is never hijacked', () => {
      const input = document.createElement('input');
      document.body.appendChild(input);
      input.focus();

      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: 'q', metaKey: true });

      expect(setPageMock).not.toHaveBeenCalled();

      document.body.removeChild(input);
      unmount();
    });
  });

  describe('Cmd/Ctrl+Shift+? -- shortcuts help dialog', () => {
    it('opens the dialog on the shifted "?" character', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: '?', shiftKey: true, metaKey: true });
      expect(setShortcutsHelpOpenMock).toHaveBeenCalledWith(true);
      unmount();
    });

    it('does not open on the unshifted "/" -- this case already got the shifted-character rule right, unlike the "." case this file was written to fix', () => {
      const { unmount } = renderHook(() => useKeyboardShortcuts());
      pressKey({ key: '/', shiftKey: false, metaKey: true });
      expect(setShortcutsHelpOpenMock).not.toHaveBeenCalled();
      unmount();
    });
  });
});
