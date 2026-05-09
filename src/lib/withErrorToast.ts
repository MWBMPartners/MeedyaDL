// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file withErrorToast — async wrapper that emits success/error toasts.
 *
 * Audit v2 finding #1: ~30+ component handlers across the codebase
 * follow the shape:
 *
 *   try {
 *     await ipc();
 *     addToast('Success!', 'success');
 *   } catch (err) {
 *     addToast(`Failed to do X: ${err}`, 'error');
 *   }
 *
 * Each repetition is 4-6 lines that say nothing the call site cares
 * about. This helper collapses the shape to a single call:
 *
 *   await withErrorToast(() => ipc(), {
 *     successMsg: 'Success!',
 *     errorMsg: (err) => `Failed to do X: ${err}`,
 *   });
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

/**
 * Options controlling toast emission for a single async operation.
 *
 * Omit any field for the corresponding default behaviour:
 * - No `successMsg` → no toast on success
 * - No `errorMsg` → use the raw error message (`String(err)`)
 * - No `suppressOn` → every error fires a toast
 */
export interface WithErrorToastOptions {
  /** Toast text shown after successful resolution. Omit to skip. */
  successMsg?: string;
  /** Toast variant for success. Default: `'success'`. */
  successVariant?: 'success' | 'info';
  /**
   * Error message strategy:
   * - `string`: static text shown verbatim
   * - `(err: unknown) => string`: derives the toast text from the
   *   thrown value — typically `(err) => \`Failed to X: ${err}\``
   * - omitted: uses `String(err)` (or `err.message` for Error
   *   instances) as the toast text
   */
  errorMsg?: string | ((err: unknown) => string);
  /**
   * Case-insensitive substrings that, when present in the
   * normalised error message, suppress the error toast. Useful for
   * paths where rejection is expected — e.g. `['cancel']` for file
   * picker dismissals where the user has already signalled intent
   * to abort the action.
   *
   * The normalisation matches what `withErrorToast` would have
   * displayed (the post-`errorMsg` string), so a custom `errorMsg`
   * function can rewrite the comparison surface if needed.
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
 *     successMsg: 'Crash report deleted',
 *     successVariant: 'info',
 *     errorMsg: (err) => `Failed to delete crash report: ${err}`,
 *   });
 *
 * @example Suppressed cancellation
 *   await withErrorToast(() => exportQueue(), {
 *     successMsg: 'Queue exported',
 *     errorMsg: 'Failed to export queue',
 *     suppressOn: ['cancel'], // user closed the file picker
 *   });
 *
 * @example Conditional success toast (gate inside the wrapped fn)
 *   const count = await withErrorToast(async () => {
 *     const n = await exportQueue();
 *     if (n > 0) useUiStore.getState().addToast(`Exported ${n}`, 'success');
 *     return n;
 *   }, { errorMsg: 'Failed to export queue' });
 */
export async function withErrorToast<T>(
  fn: () => Promise<T>,
  opts: WithErrorToastOptions = {},
): Promise<T | undefined> {
  const addToast = useUiStore.getState().addToast;
  try {
    const result = await fn();
    if (opts.successMsg) {
      addToast(opts.successMsg, opts.successVariant ?? 'success');
    }
    return result;
  } catch (err) {
    const rawMsg = err instanceof Error ? err.message : String(err);
    const display =
      typeof opts.errorMsg === 'function'
        ? opts.errorMsg(err)
        : (opts.errorMsg ?? rawMsg);
    if (opts.suppressOn?.length) {
      const lower = display.toLowerCase();
      if (opts.suppressOn.some((p) => lower.includes(p.toLowerCase()))) {
        return undefined;
      }
    }
    addToast(display, 'error');
    return undefined;
  }
}
