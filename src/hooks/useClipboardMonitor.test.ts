// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for useClipboardMonitor.
 *
 * Three faults are pinned here, all in the real exported
 * `useClipboardMonitor` hook -- none of this re-implements the hook's own
 * logic in the test file to check it against itself:
 *
 * 1. The toast's "Download" button used to do nothing at all when the
 *    backend answered with an empty `download_id`. The backend sends that
 *    back on purpose in ordinary situations -- most commonly "this link
 *    is already in the queue" -- with the reason in `duplicate_warning`.
 *    That is not an error, so it never reached the `.catch()` either:
 *    clicking Download just produced silence. Fixed to fall back to
 *    `duplicate_warning`, then a plain sentence, matching the identical
 *    shape already handled correctly in downloadStore.ts's undo handler.
 *
 * 2. `seenUrls` used to grow for the entire life of the session with no
 *    limit. Fixed with a bounded cap (`MAX_SEEN_URLS`, oldest dropped
 *    first).
 *
 * 3. The hook used to send a second, direct system notification that
 *    ignored the notification style and said "click to download" (clicking
 *    did nothing). Now there is one prompt, by one path (#1431): the
 *    in-app toast, tagged `clipboard_link`, and the backend decides about
 *    the system copy. The plugin mock stays so the tests can prove it is
 *    never called.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

// --- Mocks -------------------------------------------------------------
//
// Every stand-in below is named with a "Mock" suffix so Vitest's hoisting
// rule lets these factories reference them (see the identical note in
// useKeyboardShortcuts.test.ts and src/lib/openPath.test.ts, which use
// the same pattern).
const addToastMock = vi.fn();
vi.mock('@/stores/uiStore', () => ({
  useUiStore: { getState: () => ({ addToast: addToastMock }) },
}));

// clipboard_monitoring and desktop_notifications need to change between
// tests, so they come from a mock function's return value rather than a
// fixed object.
const getSettingsMock = vi.fn();
vi.mock('@/stores/settingsStore', () => ({
  useSettingsStore: { getState: () => ({ settings: getSettingsMock() }) },
}));

const readClipboardMock = vi.fn();
const startDownloadMock = vi.fn();
vi.mock('@/lib/tauri-commands', () => ({
  readClipboard: () => readClipboardMock(),
  startDownload: (req: unknown) => startDownloadMock(req),
}));

const isPermissionGrantedMock = vi.fn();
const requestPermissionMock = vi.fn();
const sendNotificationMock = vi.fn();
vi.mock('@tauri-apps/plugin-notification', () => ({
  isPermissionGranted: () => isPermissionGrantedMock(),
  requestPermission: () => requestPermissionMock(),
  sendNotification: (opts: unknown) => sendNotificationMock(opts),
}));

import { useClipboardMonitor } from './useClipboardMonitor';

/** Matches the hook's own private polling interval (2 seconds). */
const POLL_INTERVAL_MS = 2000;

/** A syntactically valid Apple Music album URL, as real clipboard text would be. */
const ALBUM_URL = 'https://music.apple.com/us/album/test/1234567890';

/** The toast key the hook uses for its clipboard-detection toast. */
const CLIPBOARD_TOAST_KEY = 'clipboard-url';

/**
 * Advances the one pending clipboard-poll timer and lets every promise it
 * kicks off (readClipboard().then(...) and everything chained after it)
 * actually settle before the test looks at the result. Plain
 * `vi.advanceTimersByTime` only fires the timer callback; it does not wait
 * for the microtasks that callback schedules.
 */
async function poll(): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
  });
}

/**
 * Finds the "Download" button's onClick handler from the most recent
 * clipboard-detection toast. This is the real function the hook wired up,
 * not a stand-in -- calling it is how these tests exercise fault #1.
 */
function getDownloadAction(): () => void {
  const call = [...addToastMock.mock.calls].reverse().find((c) => c[3] === CLIPBOARD_TOAST_KEY);
  if (!call) {
    throw new Error('No clipboard-detection toast was shown -- cannot find its Download action');
  }
  const action = (call[4] as { onClick?: () => void } | undefined)?.onClick;
  if (typeof action !== 'function') {
    throw new Error('Clipboard-detection toast had no onClick action');
  }
  return action;
}

describe('useClipboardMonitor', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    addToastMock.mockReset();
    readClipboardMock.mockReset();
    startDownloadMock.mockReset();
    isPermissionGrantedMock.mockReset();
    requestPermissionMock.mockReset();
    sendNotificationMock.mockReset();

    getSettingsMock.mockReset();
    getSettingsMock.mockReturnValue({
      clipboard_monitoring: true,
      desktop_notifications: false,
    });

    // Defaults for the notification path so tests that don't care about it
    // don't accidentally exercise sendNotification().
    isPermissionGrantedMock.mockResolvedValue(false);
    requestPermissionMock.mockResolvedValue('default');
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('the Download button (fault #1 -- used to do nothing for an already-queued link)', () => {
    it('shows the backend\'s own reason when download_id is empty, instead of nothing', async () => {
      readClipboardMock.mockResolvedValue(ALBUM_URL);
      const { unmount } = renderHook(() => useClipboardMonitor(true));
      await poll();

      const downloadAction = getDownloadAction();
      addToastMock.mockClear();

      startDownloadMock.mockResolvedValue({
        download_id: '',
        duplicate_warning: 'This link is already in the queue.',
      });

      await act(async () => {
        downloadAction();
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(addToastMock).toHaveBeenCalledTimes(1);
      expect(addToastMock).toHaveBeenCalledWith('This link is already in the queue.', 'info');

      unmount();
    });

    it('falls back to a plain sentence when the backend gives no id and no reason either', async () => {
      readClipboardMock.mockResolvedValue(ALBUM_URL);
      const { unmount } = renderHook(() => useClipboardMonitor(true));
      await poll();

      const downloadAction = getDownloadAction();
      addToastMock.mockClear();

      startDownloadMock.mockResolvedValue({ download_id: '', duplicate_warning: null });

      await act(async () => {
        downloadAction();
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(addToastMock).toHaveBeenCalledWith('Already in the queue — nothing added', 'info');

      unmount();
    });

    it('still shows the success toast for a genuinely new download', async () => {
      readClipboardMock.mockResolvedValue(ALBUM_URL);
      const { unmount } = renderHook(() => useClipboardMonitor(true));
      await poll();

      const downloadAction = getDownloadAction();
      addToastMock.mockClear();

      startDownloadMock.mockResolvedValue({ download_id: 'abc-123', duplicate_warning: null });

      await act(async () => {
        downloadAction();
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(addToastMock).toHaveBeenCalledWith('Added to queue from clipboard', 'success');

      unmount();
    });

    it('still shows an error toast when the IPC call itself rejects', async () => {
      readClipboardMock.mockResolvedValue(ALBUM_URL);
      const { unmount } = renderHook(() => useClipboardMonitor(true));
      await poll();

      const downloadAction = getDownloadAction();
      addToastMock.mockClear();

      startDownloadMock.mockRejectedValue(new Error('network down'));

      await act(async () => {
        downloadAction();
        await Promise.resolve();
        await Promise.resolve();
      });

      // Polish pass (M8): a plain message, with what was thrown folded
      // under "Details" (the last argument) -- never the raw text alone.
      expect(addToastMock).toHaveBeenCalledWith(
        'MeedyaDL could not add the copied link to the queue. Paste it on the Download page instead.',
        'error',
        undefined,
        undefined,
        undefined,
        'network down',
      );

      unmount();
    });
  });

  describe('seenUrls cap (fault #2 -- used to grow without limit)', () => {
    it('does not re-prompt for the same URL while it is still on the clipboard', async () => {
      readClipboardMock.mockResolvedValue(ALBUM_URL);
      const { unmount } = renderHook(() => useClipboardMonitor(true));

      await poll();
      await poll();
      await poll();

      const clipboardToasts = addToastMock.mock.calls.filter((c) => c[3] === CLIPBOARD_TOAST_KEY);
      expect(clipboardToasts).toHaveLength(1);

      unmount();
    });

    it('drops the oldest URL once the cap is exceeded, so an early one can be re-detected', async () => {
      const { unmount } = renderHook(() => useClipboardMonitor(true));
      const firstUrl = 'https://music.apple.com/us/album/test/1000000000';

      // Fill the set to exactly MAX_SEEN_URLS (200) distinct entries,
      // starting with firstUrl. At exactly 200, nothing has been evicted
      // yet -- the eviction check only runs when a 201st distinct URL
      // actually needs room.
      for (let i = 0; i < 200; i++) {
        readClipboardMock.mockResolvedValueOnce(
          `https://music.apple.com/us/album/test/${1000000000 + i}`,
        );
        await poll();
      }

      // One more distinct URL -- this is the one that pushes the set over
      // the cap, which is what should evict firstUrl (the oldest entry).
      readClipboardMock.mockResolvedValueOnce('https://music.apple.com/us/album/test/1000000200');
      await poll();

      addToastMock.mockClear();

      // If the cap and the "drop the oldest" rule are both working,
      // firstUrl was evicted by the step above, so re-presenting it now
      // counts as new again and produces a fresh toast. If the set were
      // still unbounded (the bug), firstUrl would still be remembered
      // from the very first poll and nothing would show here.
      readClipboardMock.mockResolvedValueOnce(firstUrl);
      await poll();

      const clipboardToasts = addToastMock.mock.calls.filter((c) => c[3] === CLIPBOARD_TOAST_KEY);
      expect(clipboardToasts).toHaveLength(1);

      unmount();
    });
  });

  describe('one prompt, one path (fault #3 -- a second, direct system notification, #1431)', () => {
    // There used to be a second system notification sent straight from
    // this hook. It ignored the notification style and the "when"
    // setting, and said "click to download" although clicking it did
    // nothing. Now the hook raises exactly one prompt -- the in-app toast
    // with its Download button -- tagged `clipboard_link`, and the
    // backend's one notification rule decides about the system copy.
    beforeEach(() => {
      getSettingsMock.mockReturnValue({
        clipboard_monitoring: true,
        desktop_notifications: true,
      });
      vi.spyOn(document, 'hasFocus').mockReturnValue(false);
      isPermissionGrantedMock.mockResolvedValue(true);
      requestPermissionMock.mockResolvedValue('granted');
    });

    it('raises exactly one prompt and never sends a system notification itself', async () => {
      readClipboardMock.mockResolvedValue(ALBUM_URL);
      const { unmount } = renderHook(() => useClipboardMonitor(true));
      await poll();

      const prompts = addToastMock.mock.calls.filter((c) => c[3] === CLIPBOARD_TOAST_KEY);
      expect(prompts).toHaveLength(1);
      expect(sendNotificationMock).not.toHaveBeenCalled();
      expect(isPermissionGrantedMock).not.toHaveBeenCalled();
      expect(requestPermissionMock).not.toHaveBeenCalled();

      unmount();
    });

    it('tags the prompt as a copied link and gives the system copy honest wording', async () => {
      readClipboardMock.mockResolvedValue(ALBUM_URL);
      const { unmount } = renderHook(() => useClipboardMonitor(true));
      await poll();

      const call = addToastMock.mock.calls.find((c) => c[3] === CLIPBOARD_TOAST_KEY);
      const options = call?.[6] as { kind?: string; systemBody?: string } | undefined;
      expect(options?.kind).toBe('clipboard_link');
      expect(options?.systemBody).not.toMatch(/click/i);
      // The system text names the very message the person will find.
      expect(options?.systemBody).toContain(`"${call?.[0] as string}"`);
      expect(options).toMatchObject({ keepUntilDismissedIfSent: true });
      // MeedyaDL is not in front here, so the prompt stays until dismissed.
      expect(call?.[2]).toBe(0);

      unmount();
    });
  });

  it('lets the prompt time out after 10 seconds while MeedyaDL is in front', async () => {
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    readClipboardMock.mockResolvedValue(ALBUM_URL);
    const { unmount } = renderHook(() => useClipboardMonitor(true));
    await poll();
    const call = addToastMock.mock.calls.find((c) => c[3] === CLIPBOARD_TOAST_KEY);
    expect(call?.[2]).toBe(10000);
    unmount();
  });

  describe('basic gating (pre-existing behaviour, protected so future edits don\'t regress it)', () => {
    it('does nothing while isReady is false', async () => {
      readClipboardMock.mockResolvedValue(ALBUM_URL);
      const { unmount } = renderHook(() => useClipboardMonitor(false));
      await poll();
      expect(readClipboardMock).not.toHaveBeenCalled();
      unmount();
    });

    it('does nothing when clipboard_monitoring is off', async () => {
      getSettingsMock.mockReturnValue({
        clipboard_monitoring: false,
        desktop_notifications: false,
      });
      readClipboardMock.mockResolvedValue(ALBUM_URL);

      const { unmount } = renderHook(() => useClipboardMonitor(true));
      await poll();

      expect(readClipboardMock).not.toHaveBeenCalled();
      unmount();
    });

    it('ignores clipboard text that is not a supported URL', async () => {
      readClipboardMock.mockResolvedValue('just some ordinary text');
      const { unmount } = renderHook(() => useClipboardMonitor(true));
      await poll();

      expect(addToastMock).not.toHaveBeenCalled();
      unmount();
    });
  });
});
