// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file windowTitle.ts -- set the title the operating system shows for the
 * MeedyaDL window.
 *
 * Setting `document.title` alone is not enough in the desktop app: Tauri
 * does not copy the page's title onto the native window (its page-title
 * hook does nothing unless the app asks it to), so the window switcher,
 * the taskbar, the Dock's window list and screen readers' window lists all
 * kept saying "MeedyaDL" whichever screen was open. Each screen used to
 * set only `document.title`, which nobody outside the page ever sees.
 *
 * This sets both. Changing the native title needs the
 * `core:window:allow-set-title` permission in
 * `src-tauri/capabilities/default.json`.
 *
 * What it cannot do: report a failure. Outside the desktop app (tests, a
 * plain browser) there is no native window, and a refused permission only
 * means the native title stays as it was; neither is worth interrupting
 * anyone over, so both are ignored.
 */

import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * Sets the page title and the native window title to `title`.
 *
 * @param title -- The whole title, for example "Queue — MeedyaDL".
 */
export function setWindowTitle(title: string): void {
  document.title = title;
  try {
    void getCurrentWindow()
      .setTitle(title)
      .catch(() => {
        // Permission refused or no window: the native title stays as it was.
      });
  } catch {
    // Not inside the desktop app (tests, a plain browser): there is no
    // native window to name.
  }
}
