// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for useAsyncTask (audit v2 #2).
 *
 * Pins the hook contract:
 *   - Initial state: isRunning=false, error=null
 *   - run() flips isRunning during the call, back to false after
 *   - run() returns the resolved value on success
 *   - run() returns undefined + sets error on rejection
 *   - run() clears error at the start of each new invocation
 *   - run() is referentially stable across renders
 *   - Wrapped fn is captured FRESHLY on each render (no stale closure)
 *   - Args forwarded through run() to fn
 *   - Non-Error rejections stringify cleanly
 */

import { renderHook, act } from '@testing-library/react';
import { useAsyncTask } from '@/hooks/useAsyncTask';

describe('useAsyncTask', () => {
  // ===========================================================================
  // Initial state
  // ===========================================================================

  it('starts with isRunning=false, error=null', () => {
    const { result } = renderHook(() => useAsyncTask(async () => 'ok'));
    expect(result.current.isRunning).toBe(false);
    expect(result.current.error).toBeNull();
  });

  // ===========================================================================
  // Success path
  // ===========================================================================

  it('run() returns the resolved value on success', async () => {
    const { result } = renderHook(() => useAsyncTask(async () => 42));
    let returned: number | undefined;
    await act(async () => {
      returned = await result.current.run();
    });
    expect(returned).toBe(42);
    expect(result.current.error).toBeNull();
  });

  it('isRunning flips true during the call and false after', async () => {
    let resolveOuter: ((v: string) => void) | undefined;
    const fn = () =>
      new Promise<string>((resolve) => {
        resolveOuter = resolve;
      });
    const { result } = renderHook(() => useAsyncTask(fn));

    let runPromise: Promise<string | undefined> | undefined;
    act(() => {
      runPromise = result.current.run();
    });
    expect(result.current.isRunning).toBe(true);

    await act(async () => {
      resolveOuter!('done');
      await runPromise;
    });
    expect(result.current.isRunning).toBe(false);
  });

  // ===========================================================================
  // Error path
  // ===========================================================================

  it('run() returns undefined and surfaces error on rejection', async () => {
    const { result } = renderHook(() =>
      useAsyncTask(async () => {
        throw new Error('boom');
      }),
    );
    let returned: unknown;
    await act(async () => {
      returned = await result.current.run();
    });
    expect(returned).toBeUndefined();
    expect(result.current.error).toBe('boom');
    expect(result.current.isRunning).toBe(false);
  });

  it('non-Error rejections stringify cleanly', async () => {
    const { result } = renderHook(() =>
      useAsyncTask(async () => {
        throw 'plain string';
      }),
    );
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.error).toBe('plain string');
  });

  it('run() clears prior error at the start of a new invocation', async () => {
    let shouldFail = true;
    const fn = async () => {
      if (shouldFail) throw new Error('first failure');
      return 'ok';
    };
    const { result, rerender } = renderHook(({ f }) => useAsyncTask(f), {
      initialProps: { f: fn },
    });

    await act(async () => {
      await result.current.run();
    });
    expect(result.current.error).toBe('first failure');

    shouldFail = false;
    rerender({ f: fn });
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.error).toBeNull();
  });

  // ===========================================================================
  // Args forwarding
  // ===========================================================================

  it('forwards args to the wrapped fn', async () => {
    const fn = vi.fn(async (a: number, b: string) => `${a}-${b}`);
    const { result } = renderHook(() => useAsyncTask(fn));
    let returned: string | undefined;
    await act(async () => {
      returned = await result.current.run(7, 'x');
    });
    expect(fn).toHaveBeenCalledWith(7, 'x');
    expect(returned).toBe('7-x');
  });

  // ===========================================================================
  // Stable run reference + fresh closure
  // ===========================================================================

  it('run is referentially stable across renders', () => {
    const { result, rerender } = renderHook(() => useAsyncTask(async () => 'x'));
    const firstRun = result.current.run;
    rerender();
    expect(result.current.run).toBe(firstRun);
  });

  it('always invokes the LATEST wrapped closure (no stale-closure bug)', async () => {
    let callCount = 0;
    const makeFn = (label: string) =>
      vi.fn(async () => {
        callCount += 1;
        return label;
      });

    const fnA = makeFn('A');
    const fnB = makeFn('B');
    const { result, rerender } = renderHook(({ f }) => useAsyncTask(f), {
      initialProps: { f: fnA },
    });

    // Capture run from the first render (when fnA was the wrapped fn).
    const stableRun = result.current.run;

    // Re-render with a different fn — `run` should still point to the
    // same callback identity, but now invoke fnB.
    rerender({ f: fnB });

    let returned: string | undefined;
    await act(async () => {
      returned = await stableRun();
    });

    expect(returned).toBe('B');
    expect(fnA).not.toHaveBeenCalled();
    expect(fnB).toHaveBeenCalledTimes(1);
    expect(callCount).toBe(1);
  });
});
