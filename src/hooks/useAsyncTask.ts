// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file useAsyncTask — component-local one-shot async lifecycle hook.
 *
 * Audit v2 finding #2: ~10 components hand-roll the same shape:
 *
 *   const [isLoading, setIsLoading] = useState(false);
 *   const [error, setError] = useState<string | null>(null);
 *
 *   const run = async () => {
 *     setIsLoading(true);
 *     setError(null);
 *     try {
 *       await ipc();
 *     } catch (err) {
 *       setError(err instanceof Error ? err.message : String(err));
 *     } finally {
 *       setIsLoading(false);
 *     }
 *   };
 *
 * `createAsyncResourceStore` (v1.0.7) covers the *store-level*
 * shape — full data + load + save + dirty tracking. This hook is
 * the **component-local** sibling: a single async function with
 * `isRunning` + `error` state, no shared store, no debounce.
 *
 *   const submit = useAsyncTask(runPreflight);
 *   <Button loading={submit.isRunning} onClick={() => submit.run()}>
 *     Submit
 *   </Button>
 *   {submit.error && <p className="text-status-error">{submit.error}</p>}
 *
 * Pairs naturally with `withErrorToast` when the call site wants a
 * toast in addition to local error state:
 *
 *   const submit = useAsyncTask(() =>
 *     withErrorToast(runPreflight, { errorMsg: 'Submit failed' }),
 *   );
 *
 * The wrapped fn can take arbitrary arguments, forwarded through
 * `run(...args)`. Captured fresh on each render via a ref so
 * stale-closure bugs don't bite — `run` itself is referentially
 * stable so it can be passed directly to onClick handlers.
 *
 * @see createAsyncResourceStore — store-level sibling (full data + load + save)
 * @see withErrorToast — natural pairing for toast-on-error
 * @see audit doc finding #2 in
 *      [.github/audits/codebase-unification-audit-v2.md]
 */

import { useCallback, useRef, useState } from 'react';

export interface UseAsyncTaskReturn<TArgs extends unknown[], TResult> {
  /**
   * Invoke the wrapped function with `args`. Returns the resolved
   * value on success, `undefined` on rejection. Errors are NOT
   * re-thrown — they're surfaced via the `error` state.
   *
   * Referentially stable across renders, so it's safe to pass
   * directly to a button's `onClick` without `useCallback` wrapping.
   */
  run: (...args: TArgs) => Promise<TResult | undefined>;
  /** True while the most recent `run` invocation is in flight. */
  isRunning: boolean;
  /**
   * Error message from the most recent rejection, or `null` if the
   * last run succeeded (or has not been called yet). Cleared at the
   * start of each new run.
   */
  error: string | null;
}

/**
 * Wrap a single async function with `isRunning` + `error` state.
 *
 * The wrapped function is captured by ref so the *latest* closure
 * always runs — pass it inline without worrying about stale state.
 *
 * @param fn — the async function to wrap. Re-captured on every render.
 *
 * @example No-args
 *   const submit = useAsyncTask(runPreflight);
 *   <Button onClick={() => submit.run()}>Submit</Button>
 *
 * @example With args
 *   const del = useAsyncTask((id: string) => deleteCrashReport(id));
 *   <Button onClick={() => del.run(report.id)}>Delete</Button>
 *
 * @example Composed with withErrorToast
 *   const save = useAsyncTask(() =>
 *     withErrorToast(saveSettings, {
 *       successMsg: 'Saved',
 *       errorMsg: 'Save failed',
 *     }),
 *   );
 */
export function useAsyncTask<TArgs extends unknown[], TResult>(
  fn: (...args: TArgs) => Promise<TResult>,
): UseAsyncTaskReturn<TArgs, TResult> {
  // Capture the latest closure on every render so `run` always
  // calls the current version. Without this, a button wired to a
  // stable `run` would close over the *first* render's `fn`.
  const fnRef = useRef(fn);
  fnRef.current = fn;

  const [isRunning, setIsRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // `run` is intentionally stable (empty dep array). Captured-by-ref
  // pattern above keeps the closure fresh without dep churn.
  const run = useCallback(
    async (...args: TArgs): Promise<TResult | undefined> => {
      setIsRunning(true);
      setError(null);
      try {
        return await fnRef.current(...args);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setError(msg);
        return undefined;
      } finally {
        setIsRunning(false);
      }
    },
    [],
  );

  return { run, isRunning, error };
}
