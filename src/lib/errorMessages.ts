// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file How MeedyaDL tells people something went wrong (polish pass M8).
 *
 * The rule: every error a person sees says, in plain English, what went
 * wrong and what to do next -- in the style of the newer messages ("MeedyaDL
 * could not save that answer, so it will ask again next time you open it").
 * The technical text the error came with (a Rust error string, a Python
 * traceback, a file path) is never the whole message. It goes into a folded
 * "Details" part of the message, where somebody reporting a problem can
 * still find and copy it.
 *
 * Before this file, about 60 of the app's 95 error messages were "Failed to
 * X: <raw error>" or a bare "Failed to X", and two global handlers showed
 * "Unexpected error: ..." and "Async error: ...".
 *
 * What this file cannot do: make the text the backend sends any clearer.
 * Where the backend already writes for people (a paused service, a link
 * that is not supported yet), callers show that text as the message.
 */

import { useUiStore } from '@/stores/uiStore';
import type { ToastType } from '@/types';

/**
 * The technical text of anything that was thrown or rejected: an Error's
 * message, or the value as text. Never shown on its own; it goes in
 * "Details".
 */
export function rawError(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === 'string') return err;
  try {
    return JSON.stringify(err) ?? String(err);
  } catch {
    return String(err);
  }
}

/**
 * True when a rejection only means the person closed a file or folder
 * picker. Those are not errors and get no message at all. The backend says
 * "cancelled" in several wordings ("Folder selection cancelled", "Export
 * cancelled", "Import cancelled").
 */
export function isCancellation(err: unknown): boolean {
  return /\bcancel(l)?ed\b|\bcancel\b/i.test(rawError(err));
}

interface ShowErrorOptions {
  /** 'error' unless the problem is a warning (nothing was lost). */
  type?: Extract<ToastType, 'error' | 'warning'>;
  /** Replaces any earlier message with the same key (see addToast). */
  key?: string;
  /** An optional button on the message, for example "Retry". */
  action?: { label: string; onClick: () => void };
}

/**
 * Show an error the house way.
 *
 * @param message What went wrong and what to do next, in plain English.
 *   Write it so it makes sense on its own -- most people never open the
 *   details.
 * @param err What was thrown, if anything. Its technical text is folded
 *   into "Details" under the message; it is left out when it would only
 *   repeat the message.
 */
export function showError(message: string, err?: unknown, opts: ShowErrorOptions = {}): void {
  const details = err === undefined ? undefined : rawError(err).trim();
  useUiStore
    .getState()
    .addToast(
      message,
      opts.type ?? 'error',
      undefined,
      opts.key,
      opts.action,
      details && details !== message ? details : undefined
    );
}

/**
 * Show an error whose cause the backend explains: the backend's own words
 * when they are already a plain sentence written for people (a refused
 * retry, an item that is still downloading, a file that is not a queue
 * file), otherwise `fallback` with the technical text under "Details".
 *
 * "A plain sentence" means: not technical (see summariseDownloadError),
 * ends with a full stop, question mark or exclamation mark, and fits in a
 * toast. Rust error strings ("No such file or directory (os error 2)")
 * fail that test and get the fallback.
 */
export function showBackendError(fallback: string, err: unknown, opts: ShowErrorOptions = {}): void {
  const { message, details } = explainError(fallback, err);
  useUiStore
    .getState()
    .addToast(message, opts.type ?? 'error', undefined, opts.key, opts.action, details ?? undefined);
}

/**
 * The same choice as showBackendError, without showing anything: for an
 * error drawn on the page (see InlineError). Returns the backend's own
 * words when they are already a plain sentence, otherwise `fallback` with
 * the technical text as `details`.
 */
export function explainError(fallback: string, err: unknown): { message: string; details: string | null } {
  const raw = rawError(err).trim();
  if (raw && /[.!?]$/.test(raw) && summariseDownloadError(raw).details === null) {
    return { message: raw, details: null };
  }
  return { message: fallback, details: raw && raw !== fallback ? raw : null };
}

// ---------------------------------------------------------------------------
// Download errors: a short summary for a queue or history row
// ---------------------------------------------------------------------------

/**
 * What a failed download's error looks like to a person: a short summary
 * with a next step, plus the full technical text when the summary is not
 * the original message.
 */
export interface DownloadErrorSummary {
  /** One or two plain sentences: what went wrong and what to do next. */
  summary: string;
  /** The original text, to fold away under "Details"; null when the
   *  original was already a plain sentence and is shown as the summary. */
  details: string | null;
}

/**
 * Signs that an error is technical text rather than a sentence for people:
 * a Python traceback or exception name, an operating-system error number,
 * the download tool's exit report, or a file path.
 */
const TECHNICAL = [
  /Traceback \(most recent call last\)/,
  /File "[^"]+", line \d+/,
  /\[Errno -?\d+\]/,
  /\b[A-Za-z_][\w.]*(?:Error|Exception)\b:?/,
  /\b(?:stdout|stderr):/,
  /Exited with code -?\d+/,
  /\(os error \d+\)/,
  /(?:^|[\s"'(])(?:\/Users\/|\/home\/|\/Volumes\/|\/private\/|[A-Za-z]:\\)/,
];

/**
 * Messages longer than this are summarised even if they look like prose.
 * Set well above the longest message the backend writes for people (its
 * "GAMDL bug — ..." explanations run to about 400 characters, and are
 * worth reading in full); a raw report that long is nearly always a
 * traceback, which the patterns above catch anyway.
 */
const LONG_MESSAGE = 500;

/** The prefix the backend puts on messages it recognises as a fault in
 *  GAMDL itself (utils/process.rs GAMDL_BUG_MARKER). */
const GAMDL_BUG_MARKER = 'GAMDL bug — ';

/**
 * Plain summaries, checked in order; the first that matches wins. Disk
 * problems come before network ones on purpose: macOS reports a
 * disconnected cloud drive as "[Errno 60] Operation timed out", the same
 * words as a network timeout, so only an error that also mentions
 * connecting counts as a network problem.
 */
const SUMMARIES: Array<{ test: RegExp; summary: string }> = [
  {
    test: /no space left|ENOSPC|disk (?:is )?full|read-only file system|EROFS|permission denied|\[Errno 13\]|EACCES|stale file handle|ESTALE/i,
    summary:
      'MeedyaDL could not save the files to your download folder. Check that the folder still exists, has free space and can be written to, then retry.',
  },
  {
    test: /not streamable|resource not found|\b404\b|not available in your (?:country|region|storefront)|no longer available/i,
    summary:
      'Apple Music does not offer this item here, or it has been removed. Check that the link still opens in Apple Music.',
  },
  {
    test: /\b401\b|\b403\b|unauthori[sz]ed|forbidden|cookie|not signed in|sign in again|media-user-token|login required/i,
    summary:
      'Apple Music did not accept your sign-in. Sign in again in Settings > Cookies, then retry.',
  },
  {
    test: /\b429\b|too many requests|rate.?limit/i,
    summary: 'Apple Music asked MeedyaDL to slow down. Wait a few minutes, then retry.',
  },
  {
    test: /connect(?:ing|ion)?\b.*(?:timed? ?out|refused|reset|failed)|timed out while connecting|ConnectError|ConnectTimeout|ReadTimeout|network is unreachable|name or service not known|nodename nor servname|getaddrinfo|\bdns\b/i,
    summary:
      'MeedyaDL could not reach Apple Music. Check your internet connection, then retry.',
  },
  {
    test: /format is not available|no compatible format|codec.*not available|not available in (?:the )?(?:requested|selected) (?:codec|format|quality)/i,
    summary:
      'This item is not available in the audio quality you chose. Pick another in Settings > Codec Fallback Order, then retry.',
  },
  {
    test: /ffmpeg|mp4decrypt|mp4box|n_m3u8dl|mediainfo/i,
    summary:
      'A tool MeedyaDL needs to finish the download did not work. Check it in Settings > Tools, then retry.',
  },
];

/** Used when nothing above matches. */
const GENERIC_SUMMARY =
  'The download did not finish. Retry it; if it fails again, the details say what went wrong.';

/** Used for a fault MeedyaDL recognises as GAMDL's own. */
const GAMDL_BUG_SUMMARY =
  'The download tool MeedyaDL uses (GAMDL) ran into a fault of its own. Retry the download; if it fails again, report it to GAMDL from the ⋮ menu.';

/**
 * Turn a failed download's error into a short summary for its row.
 *
 * A message that is already a plain sentence (most of the backend's own
 * messages, including its explanations of known GAMDL faults) is shown as it
 * is. A technical one -- a traceback, an exception name, an error number, a
 * file path, or anything over 500 characters --
 * is replaced by a plain summary of its likely cause, and the original goes
 * into `details`.
 *
 * What it cannot do: be sure of the cause. The summaries match on words in
 * the error -- the last exception first, then the whole text -- so a
 * message that mentions two problems gets whichever is checked first (disk
 * before network -- see SUMMARIES). The original is always kept in
 * `details`, so nothing is lost when the guess is wrong.
 */
export function summariseDownloadError(raw: string): DownloadErrorSummary {
  const text = raw.trim();
  if (!text) return { summary: '', details: null };
  // The "GAMDL bug — " marker alone does not make a message technical:
  // the backend's own explanations of known GAMDL faults carry it, and they
  // are written for people. It only decides which summary a technical one
  // falls back to.
  const isGamdlBug = text.startsWith(GAMDL_BUG_MARKER);
  const technical = text.length > LONG_MESSAGE || TECHNICAL.some((re) => re.test(text));
  if (!technical) return { summary: text, details: null };
  // A Python traceback names the code it passed through before the error
  // that actually happened, and that code can mention other problems (a
  // "raise ...NotStreamableError(...)" line above a network time-out, for
  // example). The error that happened is the LAST "SomethingError: ..."
  // in the text, so that part is matched first, and the whole text only if
  // the last part says nothing recognisable.
  const lastException = [...text.matchAll(/[A-Za-z_][\w.]*(?:Error|Exception):/g)].pop();
  const tail = lastException?.index !== undefined ? text.slice(lastException.index) : text;
  const match = SUMMARIES.find((s) => s.test.test(tail)) ?? SUMMARIES.find((s) => s.test.test(text));
  const summary = match?.summary ?? (isGamdlBug ? GAMDL_BUG_SUMMARY : GENERIC_SUMMARY);
  return { summary, details: text };
}

// ---------------------------------------------------------------------------
// Messages used in more than one place, written once so they cannot drift
// ---------------------------------------------------------------------------

/** The app update could not be downloaded or installed. */
export const UPDATE_FAILED =
  'MeedyaDL could not download or install the update. Check your internet connection, then try again from the Updates page.';

/** MeedyaDL could not restart itself after an update. */
export const RESTART_FAILED = 'MeedyaDL could not restart itself. Quit MeedyaDL and open it again to finish the update.';

/** GAMDL, the download tool, could not be updated. */
export const GAMDL_UPDATE_FAILED =
  'MeedyaDL could not update GAMDL, the tool it downloads with. Check your internet connection, then try again; the details say what the installer reported.';

/** A helper tool (FFmpeg, MP4Box, ...) could not be updated. */
export function componentUpdateFailed(name: string): string {
  return `MeedyaDL could not update ${name}. Check your internet connection, then try again; your current copy was kept if it was working.`;
}

/** A chosen GAMDL version could not be installed (Settings > Tools). */
export const GAMDL_INSTALL_FAILED =
  'MeedyaDL could not install that version of GAMDL. Check your internet connection and the version number, then try again; the details say what the installer reported.';

/** The logs folder could not be opened. */
export const LOGS_FOLDER_FAILED =
  'MeedyaDL could not open the logs folder. Check its location in Settings > Advanced > Diagnostics, then try again.';

/** Links could not be added to the queue (Download page). */
export const ADD_FAILED_MANY = 'MeedyaDL could not add those links to the queue. Check the links, then try again.';
/** A link could not be added to the queue (Download page). */
export const ADD_FAILED_ONE = 'MeedyaDL could not add that link to the queue. Check the link, then try again.';

/** A queue item could not be moved. */
export const MOVE_FAILED =
  'MeedyaDL could not move that item. Choose Refresh in the More menu, then try again.';

/** A download could not be retried, and the backend gave no plain reason. */
export const RETRY_FAILED = 'MeedyaDL could not retry that download. Try again in a moment.';

/** A dropped file is not a MeedyaDL download record. */
export const NOT_A_RECORD =
  'That file is not a MeedyaDL download record it can read. Drop a .meedyadl file that MeedyaDL saved.';

/** The downloads could not be stopped (Abort). */
export const ABORT_FAILED =
  'MeedyaDL could not stop the downloads. Press Abort again; if they keep running, quit MeedyaDL.';

/** A folder could not be scanned (Library). */
export const FOLDER_SCAN_FAILED =
  'MeedyaDL could not scan that folder. Check that it still exists and you can open it, then try again.';

/** Items removed by Clear or Abort could not be put back by Undo. */
export function undoFailed(count: number): string {
  return `MeedyaDL could not put back the ${count === 1 ? 'item' : `${count} items`} you removed. Add ${count === 1 ? 'its link' : 'their links'} again on the Download page.`;
}

/** Signing in to the wrapper did not work, and the backend gave no plain reason. */
export const WRAPPER_SIGN_IN_FAILED =
  'MeedyaDL could not sign in to the wrapper. Check that the wrapper is running and that your Apple ID details are right, then try again.';

/** Cookies could not be imported from a browser or the sign-in window. */
export const COOKIES_IMPORT_FAILED =
  'MeedyaDL could not get your Apple Music sign-in. Make sure you are signed in to Apple Music, then try again — or use Sign In instead.';
