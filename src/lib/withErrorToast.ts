// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file withErrorToast — async wrapper that emits success/error toasts.
 *
 * Audit v2 finding #1: ~30+ component handlers across the codebase
 * followed the shape "try the call; on success toast; on failure toast".
 * This helper collapses it to a single call:
 *
 *   await withErrorToast(() => deleteCrashReport(id), {
 *     successMsg: 'Report deleted',
 *     errorMsg: 'MeedyaDL could not delete that report. Try again; if it keeps happening, restart MeedyaDL.',
 *   });
 *
 * Polish pass (M8): the error message says what went wrong and what to do
 * next, in plain English, and the thrown error's own technical text is
 * folded under "Details" automatically -- it is never the whole message.
 * That is why `errorMsg` is required: the old default showed the raw
 * error by itself, and the usual call shape was "Failed to X: ${err}".
 *
 * The function reads `addToast` from `useUiStore.getState()` so it
 * works from any context (component handlers, store actions, async
 * effects) without needing to be a React hook.
 *
 * @see uiStore.addToast — the underlying toast emitter
 * @see audit doc finding #1 in
 *      [.github/audits/codebase-unification-audit-v2.md]
 */

import { useUiStore } from '@/stores/uiStore';
import { rawError } from '@/lib/errorMessages';

/**
 * Options controlling toast emission for a single async operation.
 *
 * - No `successMsg` → no toast on success
 * - `errorMsg` is required (see above)
 * - No `suppressOn` → every error fires a toast
 */
export interface WithErrorToastOptions {
  /** Toast text shown after successful resolution. Omit to skip. */
  successMsg?: string;
  /** Toast variant for success. Default: `'success'`. */
  successVariant?: 'success' | 'info';
  /**
   * What went wrong and what to do next, in plain English. Required: the
   * thrown error's technical text goes under "Details" by itself, so this
   * should never repeat it.
   * - `string`: the message
   * - `(err: unknown) => string`: for the rare message that depends on
   *   what was thrown (keep the technical text out of it)
   */
  errorMsg: string | ((err: unknown) => string);
  /**
   * Case-insensitive substrings that, when present in the thrown
   * error's text OR in the message, suppress the error toast. Useful
   * for paths where rejection is expected — e.g. `['cancel']` for file
   * picker dismissals where the user has already signalled intent to
   * abort the action. (The thrown text is checked as well because the
   * message is now a plain sentence that rarely repeats it.)
   */
  suppressOn?: string[];
}

/**
 * Run an async function and emit toasts based on its outcome.
 *
 * On success: returns the resolved value, optionally fires a
 * success/info toast.
 *
 * On failure: returns `undefined`, fires an error toast (unless the
 * normalised error message matches a `suppressOn` pattern). Errors
 * are NOT re-thrown — callers that need the rejection to propagate
 * (rare) should not use this helper.
 *
 * @example Simple use
 *   await withErrorToast(() => deleteCrashReport(id), {
 *     successMsg: 'Report deleted',
 *     successVariant: 'info',
 *     errorMsg: 'MeedyaDL could not delete that report. Try again.',
 *   });
 *
 * @example Suppressed cancellation
 *   await withErrorToast(() => exportQueue(), {
 *     successMsg: 'Queue exported',
 *     errorMsg: 'MeedyaDL could not save the queue file. Choose another folder and try again.',
 *     suppressOn: ['cancel'], // user closed the file picker
 *   });
 */
export async function withErrorToast<T>(
  fn: () => Promise<T>,
  opts: WithErrorToastOptions,
): Promise<T | undefined> {
  const addToast = useUiStore.getState().addToast;
  try {
    const result = await fn();
    if (opts.successMsg) {
      addToast(opts.successMsg, opts.successVariant ?? 'success');
    }
    return result;
  } catch (err) {
    const rawMsg = rawError(err);
    const display = typeof opts.errorMsg === 'function' ? opts.errorMsg(err) : opts.errorMsg;
    if (opts.suppressOn?.length) {
      const lower = `${rawMsg}\n${display}`.toLowerCase();
      if (opts.suppressOn.some((p) => lower.includes(p.toLowerCase()))) {
        return undefined;
      }
    }
    // The technical text goes under "Details", unless it would only
    // repeat the message.
    addToast(display, 'error', undefined, undefined, undefined, rawMsg && rawMsg !== display ? rawMsg : undefined);
    return undefined;
  }
}
