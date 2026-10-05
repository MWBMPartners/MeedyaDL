// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file The window title reaches the operating system, not only the page.
 *
 * Tauri does not copy `document.title` onto the native window, so setting
 * only that left every window list saying "MeedyaDL" on every screen.
 */

import { setWindowTitle } from './windowTitle';

const setTitle = vi.fn().mockResolvedValue(undefined);
let windowAvailable = true;

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => {
    if (!windowAvailable) throw new Error('no native window');
    return { setTitle };
  },
}));

describe('setWindowTitle', () => {
  beforeEach(() => {
    setTitle.mockClear();
    windowAvailable = true;
  });

  it('sets the native window title as well as the page title', () => {
    setWindowTitle('Queue — MeedyaDL');
    expect(document.title).toBe('Queue — MeedyaDL');
    expect(setTitle).toHaveBeenCalledWith('Queue — MeedyaDL');
  });

  it('still sets the page title, and does not throw, when there is no native window', () => {
    windowAvailable = false;
    expect(() => setWindowTitle('History — MeedyaDL')).not.toThrow();
    expect(document.title).toBe('History — MeedyaDL');
  });
});
