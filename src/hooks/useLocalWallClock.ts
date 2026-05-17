// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file useLocalWallClock.ts -- Tick a once-per-second wall clock.
 *
 * Tiny utility hook that triggers a 1 Hz re-render of the calling
 * component and returns the current local-system time formatted as
 * `HH:MM:SS` (24-hour). The format matches the existing per-entry
 * timestamps in `ActivityLog.tsx::formatTime`, so the rendered clock
 * lines up visually with the log entries underneath it.
 *
 * **Why:** the Activity Log already shows per-entry `HH:MM:SS` stamps,
 * but the user has no way to read "how long ago did the queue last
 * move?" or "how long has this pause been going on?" without checking
 * the OS clock off-screen. A live wall-clock chip in the page header
 * answers both questions at a glance (#801).
 *
 * **Render budget:** only the component that calls this hook re-renders
 * every second. The virtualised `filteredEntries.map` render path is
 * unaffected because it doesn't depend on this state. Calling the hook
 * inside a small dedicated subcomponent (e.g. `<WallClockChip />`) is
 * the cheapest pattern, since it isolates the per-second re-render to
 * a single span rather than the whole `ActivityLog` tree.
 *
 * **Locale:** uses `en-GB` to pin the format to `HH:MM:SS` regardless
 * of the user's OS locale (some locales render 12-hour with AM/PM by
 * default). Matches the choice already made in `formatTime`.
 */

import { useEffect, useState } from 'react';

/**
 * Returns the current local-system time as `HH:MM:SS`, updated once
 * per second.
 *
 * The hook stores the current second in component state and schedules
 * a `setInterval` tick at 1000 ms. On unmount the interval is cleared.
 *
 * The interval is scheduled to fire at the start of each wall-clock
 * second (best-effort) by computing the delay until the next second
 * boundary and using a one-shot `setTimeout` to align the very first
 * tick, then handing off to a steady `setInterval`. This avoids the
 * common "clock that increments visibly off-beat" feel of a naive
 * `setInterval(_, 1000)` that runs from `useEffect` mount time.
 */
export function useLocalWallClock(): string {
  const [now, setNow] = useState<string>(() => formatNow());

  useEffect(() => {
    // Align the first tick to the next wall-clock second boundary so
    // the display advances in sync with the actual seconds rather
    // than at mount-time + N seconds.
    const msUntilNextSecond = 1000 - (Date.now() % 1000);

    let intervalId: ReturnType<typeof setInterval> | null = null;
    const timeoutId = setTimeout(() => {
      setNow(formatNow());
      intervalId = setInterval(() => setNow(formatNow()), 1000);
    }, msUntilNextSecond);

    return () => {
      clearTimeout(timeoutId);
      if (intervalId !== null) clearInterval(intervalId);
    };
  }, []);

  return now;
}

/**
 * Formats `new Date()` as `HH:MM:SS` in the user's local timezone.
 * Pinned to `en-GB` so the format is always 24-hour `HH:MM:SS`
 * regardless of the OS locale's default time-format preference.
 */
function formatNow(): string {
  return new Date().toLocaleTimeString('en-GB', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}
