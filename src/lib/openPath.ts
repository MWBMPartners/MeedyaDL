// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file openPath — opens a folder or file, and says so when it cannot.
 *
 * Three buttons in the app opened a folder or a file, and all three did
 * nothing at all when it went wrong. Two of them caught the failure and
 * threw it away with a comment saying the shell plugin must be
 * unavailable because the app was running outside Tauri. That is one
 * reason it can fail, but the catch swallowed every other reason too —
 * most importantly the common one, where the folder has been moved,
 * renamed, or deleted since the download finished.
 *
 * From the outside those are identical: you click, and nothing happens.
 * No message, no cursor change, nothing to search for. The most likely
 * conclusion is that the app is broken.
 *
 * There was a second, bigger reason nothing happened, found later: these
 * calls used to go through `@tauri-apps/plugin-shell`'s `open()`, and
 * that function checks every path against an address pattern meant for
 * things like `https://example.com` before it asks the operating system
 * to do anything. A folder path such as `/Users/name/Music/Album` never
 * matches that pattern, so the plugin refused every single call here,
 * silently, before macOS/Windows/Linux ever saw the request. That
 * pattern is deliberately kept for the handful of places in the app that
 * really do open a web address — widening it so a folder path could pass
 * too would also let through a URL scheme nobody has reviewed. Instead,
 * opening and revealing a path on disk now goes through a separate
 * plugin, `@tauri-apps/plugin-opener`, whose own permission is granted
 * in `capabilities/default.json` and scoped to exactly that job.
 *
 * These helpers do the same job and explain themselves when they fail.
 * They read the toast function from the store directly rather than
 * through a React hook, so a presentational component can call them
 * without being given access to a store — the same approach
 * `withErrorToast` takes.
 *
 * @see withErrorToast — the same idea for IPC calls
 * @see uiStore.addToast — where the message ends up
 */

import { useUiStore } from '@/stores/uiStore';

/**
 * Toast key used for every failure raised here.
 *
 * Toasts sharing a key replace one another rather than stacking, so
 * clicking a dead button five times leaves one message on screen
 * instead of five identical ones.
 */
const OPEN_PATH_TOAST_KEY = 'open-path-failed';

/**
 * Strips the last segment from a path to get its containing folder.
 *
 * Picks the separator from the path itself rather than the current
 * platform, because a path can arrive from a queue file exported on a
 * different operating system.
 *
 * @param path -- A full path to a file.
 * @returns The containing folder, or the original path if there is no
 *          separator in it to strip.
 */
function parentFolderOf(path: string): string {
  const separator = path.includes('\\') ? '\\' : '/';
  const lastSeparator = path.lastIndexOf(separator);
  return lastSeparator > 0 ? path.substring(0, lastSeparator) : path;
}

/**
 * Opens a path with whatever the operating system uses for it, or shows
 * it selected inside its containing folder.
 *
 * @param path -- What to open or reveal.
 * @param whatFailed -- How to describe it if it does not work. Written
 *                      into the message the user reads, so it should be
 *                      ordinary words: "folder", "file".
 * @param mode -- `'open'` opens the path itself (a file with its usual
 *                app, or a folder as its own window showing what is
 *                inside it). `'reveal'` opens the path's *containing*
 *                folder with the path itself selected -- what "Reveal in
 *                Finder/Explorer" means everywhere else on the system.
 * @returns `true` if it opened, `false` if it did not (a message has
 *          already been shown to the user in that case).
 */
async function openOrExplain(
  path: string,
  whatFailed: 'folder' | 'file',
  mode: 'open' | 'reveal'
): Promise<boolean> {
  const addToast = useUiStore.getState().addToast;
  try {
    const { openPath, revealItemInDir } = await import('@tauri-apps/plugin-opener');
    if (mode === 'reveal') {
      await revealItemInDir(path);
    } else {
      await openPath(path);
    }
    return true;
  } catch (err) {
    // Deliberately does not try to guess WHY it failed. The likely
    // causes — the folder was moved, renamed or deleted, a drive or
    // network share is disconnected, or permission was refused — are
    // not reliably distinguishable from the error we get back, and a
    // confident wrong guess is worse than an honest general one. The
    // path is included because it is the thing the person can act on.
    const article = whatFailed === 'folder' ? 'That folder' : 'That file';
    addToast(
      `${article} could not be opened. It may have been moved, renamed or deleted since the download finished.\n${path}`,
      'error',
      undefined,
      OPEN_PATH_TOAST_KEY
    );
    console.error(`Failed to open ${whatFailed}:`, path, err);
    return false;
  }
}

/**
 * Reveals the folder containing a downloaded item -- opens the file
 * manager on its parent with the folder itself selected, the same thing
 * "Show in Finder" / "Show in Explorer" means in every other app.
 *
 * @param path -- Either the folder itself, or a file inside it.
 * @param pathIsDirectory -- `true` when `path` is already the folder,
 *        which is how album and playlist downloads record it. When
 *        `false` the containing folder is used instead.
 * @returns `true` if it opened; `false` if it did not, with the user
 *          already told why.
 */
export async function openContainingFolder(
  path: string,
  pathIsDirectory: boolean
): Promise<boolean> {
  const target = pathIsDirectory ? path : parentFolderOf(path);
  return openOrExplain(target, 'folder', 'reveal');
}

/**
 * Opens a downloaded file in whatever application handles its type.
 *
 * @param path -- The file to open.
 * @returns `true` if it opened; `false` if it did not, with the user
 *          already told why.
 */
export async function openDownloadedFile(path: string): Promise<boolean> {
  return openOrExplain(path, 'file', 'open');
}
