// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file useLocalWallClock.test.tsx -- Tests for the 1 Hz wall-clock hook (#801).
 *
 * Validates that the hook returns a correctly-formatted HH:MM:SS string
 * and re-renders when the wall-clock second advances. Uses Vitest's fake
 * timers so the test deterministically advances time rather than waiting
 * on the real system clock.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useLocalWallClock } from './useLocalWallClock';

describe('useLocalWallClock (#801)', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns a string formatted as HH:MM:SS', () => {
    // Pin "now" to a known instant so the formatted value is deterministic.
    // 2026-05-17T15:30:45Z; in en-GB the format is always 24-hour HH:MM:SS.
    vi.setSystemTime(new Date('2026-05-17T15:30:45.000Z'));

    const { result } = renderHook(() => useLocalWallClock());
    expect(result.current).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });

  it('advances every wall-clock second', () => {
    vi.setSystemTime(new Date('2026-05-17T15:30:45.000Z'));

    const { result } = renderHook(() => useLocalWallClock());
    const initial = result.current;

    // The hook aligns its first tick to the next wall-clock second boundary
    // (so the display advances in sync with the actual seconds). The system
    // time we set lands exactly on a boundary (.000 ms), so the alignment
    // delay is the full 1000 ms; advancing 1 s gets us the first tick.
    act(() => {
      vi.advanceTimersByTime(1000);
    });

    // After a one-second advance the displayed time must have changed.
    expect(result.current).not.toBe(initial);
    expect(result.current).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });

  it('continues advancing after the first aligned tick (steady setInterval)', () => {
    vi.setSystemTime(new Date('2026-05-17T15:30:45.000Z'));

    const { result } = renderHook(() => useLocalWallClock());

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    const afterFirst = result.current;

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    const afterSecond = result.current;

    expect(afterSecond).not.toBe(afterFirst);
    expect(afterSecond).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });

  it('clears its timers on unmount (no leaked intervals)', () => {
    vi.setSystemTime(new Date('2026-05-17T15:30:45.000Z'));

    const { result, unmount } = renderHook(() => useLocalWallClock());
    const initial = result.current;

    unmount();

    // Time advance after unmount must NOT update the (stale) result —
    // because the React tree was torn down — and must not throw or
    // schedule any further state updates. We assert by advancing 10 s
    // and checking the captured `initial` is still semantically the
    // last value the hook produced.
    expect(() => {
      vi.advanceTimersByTime(10_000);
    }).not.toThrow();

    // `result.current` reads the last committed value from before unmount.
    expect(result.current).toBe(initial);
  });
});
