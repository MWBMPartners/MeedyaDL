// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for ActivityLog (#232 — third installment).
 *
 * Scope: render-state behaviour driven by activityStore entries +
 * the search/filter toolbar. Specifically:
 *   - Empty state ("No activity yet")
 *   - Header subtitle: line counts + (filtered) / (paused) suffixes
 *   - Search input: case-insensitive substring filtering
 *   - Search clear (X) button
 *   - Category toggles (System / Download / Verbose) — toggle buttons
 *     (aria-pressed) + visible/hidden filter outcome
 *   - Filtered-to-empty state ("No entries match…")
 *   - Entry rendering: [System] badge, [shortId] badge, [MeedyaDL]
 *     badge for internal-stream entries
 *   - Header (polish pass M5, L4): the Auto-scroll switch, Clear, and
 *     the More menu (export the lines shown / export the full log / open
 *     the logs folder), with gating on entry count
 *
 * **Mocks:**
 *   - `@tanstack/react-virtual` — replaced with a synchronous
 *     pass-through that renders every entry. The real virtualizer
 *     depends on browser layout (jsdom returns zero-width rects),
 *     so under jsdom it would render no entries at all and every
 *     content assertion would fail. The mock preserves the
 *     `getItemKey` contract by simply rendering the full list.
 *   - `StatisticsPanel` — replaced with an empty placeholder so
 *     this file tests the log component, not the panel.
 *   - `@tauri-apps/plugin-opener` — `revealItemInDir()` stub so the
 *     "Open the logs folder" item doesn't throw on click.
 *
 * @see src/components/download/ActivityLog.tsx
 */

import { render, screen, fireEvent, act } from '@testing-library/react';
import { useActivityStore } from '@/stores/activityStore';
import { ActivityLog } from '@/components/download/ActivityLog';
import { makeActivityEntry as makeEntry } from '@/testing/fixtures';

// Mock the virtualizer to render every entry synchronously. The real
// useVirtualizer measures DOM elements via getBoundingClientRect()
// which jsdom returns as zero-rect — leading to zero rows rendered
// and every content-based assertion missing. We mimic the virtualizer's
// surface (count, getVirtualItems, getTotalSize, scrollToIndex,
// measureElement) just enough that the component renders cleanly.
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        index,
        start: index * 26,
        end: (index + 1) * 26,
        key: index,
        size: 26,
        lane: 0,
      })),
    getTotalSize: () => count * 26,
    scrollToIndex: vi.fn(),
    measureElement: vi.fn(),
  }),
}));

vi.mock('@/components/download/StatisticsPanel', () => ({
  StatisticsPanel: () => <div data-testid="stats-panel-mock" />,
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
  revealItemInDir: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@/lib/tauri-commands', () => ({
  exportActivityLog: vi.fn().mockResolvedValue(undefined),
  exportDiskActivityLog: vi.fn().mockResolvedValue(1024),
  getLogsFolderPath: vi.fn().mockResolvedValue('/var/log/meedyadl'),
  // The store writes the lines it is about to drop to a file before
  // discarding them. That only happens once the log is full, which no test
  // reached until the "keeps announcing" test below — so this was missing
  // and the store threw the moment trimming started.
  saveSessionLog: vi.fn().mockResolvedValue(undefined),
}));

beforeEach(() => {
  // Reset the activityStore between tests so entries don't bleed.
  // setEntries is the public store action for direct manipulation;
  // if absent, fall back to setState.
  act(() => {
    useActivityStore.setState({ entries: [], paused: false });
  });
});

describe('ActivityLog', () => {
  // ===========================================================================
  // Empty state
  // ===========================================================================

  // ===========================================================================
  // The spoken summary for screen readers
  // ===========================================================================

  it('keeps announcing new lines when the list stays the same size because old ones are being dropped', () => {
    // The log holds at most 10,000 lines. Once it is full, every new line
    // pushes an old one off the front, so the NUMBER OF LINES stops changing
    // while lines keep pouring in. A first version of this decided whether
    // anything had happened by comparing that number, so from the moment the
    // log filled up it concluded nothing was happening and went quiet for the
    // rest of the session — and because the log's own running commentary was
    // removed at the same time, somebody using a screen reader would have
    // heard nothing at all from then on.
    //
    // This reproduces that exact situation — same number of lines, different
    // lines — without building ten thousand of them, which is slow enough to
    // time out when the whole suite runs at once.
    vi.useFakeTimers();
    try {
      act(() => {
        useActivityStore.getState().addEntries([
          makeEntry({ line: 'one' }),
          makeEntry({ line: 'two' }),
          makeEntry({ line: 'three' }),
        ]);
      });

      render(<ActivityLog />);

      // Let a tick pass so the starting point is recorded.
      act(() => {
        vi.advanceTimersByTime(8000);
      });

      const sizeBefore = useActivityStore.getState().entries.length;

      // Two new lines arrive and two old ones fall off the front, exactly as
      // they would once the log is full.
      act(() => {
        useActivityStore.getState().addEntries([
          makeEntry({ line: 'four' }),
          makeEntry({ line: 'five', severity: 'error' }),
        ]);
        const all = useActivityStore.getState().entries;
        useActivityStore.setState({ entries: all.slice(all.length - sizeBefore) });
      });

      // The list is the same size as before — that is the whole point.
      expect(useActivityStore.getState().entries.length).toBe(sizeBefore);

      act(() => {
        vi.advanceTimersByTime(8000);
      });

      const announcement = screen.getByTestId('activity-log-announcement');
      expect(announcement.textContent).toMatch(/2 new lines/);
      expect(announcement.textContent).toMatch(/1 error/);
    } finally {
      vi.useRealTimers();
    }
  });

  it('renders empty state when no entries', () => {
    render(<ActivityLog />);
    expect(screen.getByText('No activity yet')).toBeInTheDocument();
    expect(screen.getByText(/Start a download to see live output/i)).toBeInTheDocument();
  });

  it('subtitle reads "0 lines" when empty', () => {
    render(<ActivityLog />);
    expect(screen.getByText('0 lines')).toBeInTheDocument();
  });

  // ===========================================================================
  // Subtitle counts + state suffixes
  // ===========================================================================

  it('subtitle pluralises lines correctly', () => {
    act(() => {
      useActivityStore.setState({
        entries: [makeEntry({ _id: 1 })],
      });
    });
    render(<ActivityLog />);
    expect(screen.getByText('1 line')).toBeInTheDocument();

    // Drive the second assertion through a Zustand state update — the
    // component subscribes to the store, so React re-renders
    // automatically. The previous implementation called
    // `rerender(<ActivityLog />)` which forced @tanstack/react-virtual
    // to re-measure under jsdom, causing intermittent 5s timeouts on
    // the Windows GitHub Actions runner specifically (jsdom DOM
    // measurement is slower on win32 due to Node test harness
    // differences). Single-render + store-driven re-render keeps the
    // virtualiser instance stable across the two assertions.
    act(() => {
      useActivityStore.setState({
        entries: [makeEntry({ _id: 1 }), makeEntry({ _id: 2 })],
      });
    });
    expect(screen.getByText('2 lines')).toBeInTheDocument();
  });

  it('subtitle adds (paused) suffix when paused', () => {
    act(() => {
      useActivityStore.setState({
        entries: [makeEntry({ _id: 1 })],
        paused: true,
      });
    });
    render(<ActivityLog />);
    expect(screen.getByText(/1 line.*\(paused\)/)).toBeInTheDocument();
  });

  it('subtitle shows "X of Y lines (filtered)" when a filter is active', () => {
    act(() => {
      useActivityStore.setState({
        entries: [
          makeEntry({ _id: 1, line: 'foo apple' }),
          makeEntry({ _id: 2, line: 'bar pear' }),
          makeEntry({ _id: 3, line: 'baz apple pie' }),
        ],
      });
    });
    render(<ActivityLog />);
    fireEvent.change(screen.getByLabelText(/search activity log/i), {
      target: { value: 'apple' },
    });
    expect(screen.getByText('2 of 3 lines (filtered)')).toBeInTheDocument();
  });

  // ===========================================================================
  // Search filter
  // ===========================================================================

  it('search input filters entries by case-insensitive substring', () => {
    act(() => {
      useActivityStore.setState({
        entries: [
          makeEntry({ _id: 1, line: 'Connecting to Apple Music' }),
          makeEntry({ _id: 2, line: 'Downloaded track' }),
          makeEntry({ _id: 3, line: 'Apple replied 200 OK' }),
        ],
      });
    });
    render(<ActivityLog />);
    fireEvent.change(screen.getByLabelText(/search activity log/i), {
      target: { value: 'apple' }, // lowercase — should still match
    });
    // Exact string, not a loose /2 of 3 lines/i regex: the log now also
    // carries its own "Showing 2 of 3 lines in total (filtered)..."
    // line inside the scrollable region (a11y audit Fix 12 -- so a
    // screen reader user reaching the end of the ~150 rendered rows
    // is told there's more, and how many, rather than the count only
    // ever existing in the page header). A loose substring match hits
    // both that new line and the header subtitle this test actually
    // means to check.
    expect(screen.getByText('2 of 3 lines (filtered)')).toBeInTheDocument();
  });

  it('Clear search (X) button empties the input', () => {
    act(() => {
      useActivityStore.setState({
        entries: [makeEntry({ _id: 1, line: 'hello world' })],
      });
    });
    render(<ActivityLog />);
    const search = screen.getByLabelText(/search activity log/i) as HTMLInputElement;
    fireEvent.change(search, { target: { value: 'hello' } });
    expect(search.value).toBe('hello');
    fireEvent.click(screen.getByLabelText(/clear search/i));
    expect(search.value).toBe('');
  });

  it('shows "No entries match" when search filters everything out', () => {
    act(() => {
      useActivityStore.setState({
        entries: [makeEntry({ _id: 1, line: 'hello world' })],
      });
    });
    render(<ActivityLog />);
    fireEvent.change(screen.getByLabelText(/search activity log/i), {
      target: { value: 'no-match' },
    });
    expect(
      screen.getByText('No entries match the current search or filter criteria.')
    ).toBeInTheDocument();
  });

  // ===========================================================================
  // Category toggles (System / Download / Verbose)
  // ===========================================================================

  it('System / Download / Verbose toggles render with correct initial aria-pressed', () => {
    render(<ActivityLog />);
    expect(screen.getByRole('button', { name: /filter system entries/i })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
    expect(
      screen.getByRole('button', { name: /filter download entries/i })
    ).toHaveAttribute('aria-pressed', 'true');
    // Verbose starts off.
    expect(
      screen.getByRole('button', { name: /filter verbose entries/i })
    ).toHaveAttribute('aria-pressed', 'false');
  });

  it('toggling System off hides system entries', () => {
    act(() => {
      useActivityStore.setState({
        entries: [
          makeEntry({ _id: 1, download_id: 'system', line: 'Update check started' }),
          makeEntry({ _id: 2, download_id: 'abcd', line: 'Track 1 of 5' }),
        ],
      });
    });
    render(<ActivityLog />);
    fireEvent.click(screen.getByRole('button', { name: /filter system entries/i }));
    expect(screen.getByText('1 of 2 lines (filtered)')).toBeInTheDocument();
    expect(screen.queryByText('Update check started')).not.toBeInTheDocument();
    expect(screen.getByText('Track 1 of 5')).toBeInTheDocument();
  });

  it('Verbose toggle reveals [VERBOSE] entries when their base category is off', () => {
    // Verbose semantics per ActivityLog.tsx:
    // A `[VERBOSE]` download line is visible if EITHER `showVerbose`
    // OR `showDownload` is on. To prove the Verbose toggle does
    // anything, we first turn OFF Download (so the verbose download
    // line goes hidden), then turn ON Verbose (which restores it).
    act(() => {
      useActivityStore.setState({
        entries: [
          makeEntry({ _id: 1, line: 'Normal line' }),
          makeEntry({ _id: 2, line: '[VERBOSE] Trace dump line' }),
        ],
      });
    });
    render(<ActivityLog />);
    // Both visible initially — Download is on, which catches both
    // the normal line AND the verbose download line.
    expect(screen.getByText(/\[VERBOSE\] Trace dump line/)).toBeInTheDocument();

    // Turn Download OFF — both download lines should now hide.
    fireEvent.click(screen.getByRole('button', { name: /filter download entries/i }));
    expect(screen.queryByText(/\[VERBOSE\] Trace dump line/)).not.toBeInTheDocument();
    expect(screen.queryByText('Normal line')).not.toBeInTheDocument();

    // Turn Verbose ON — only the verbose line comes back.
    fireEvent.click(screen.getByRole('button', { name: /filter verbose entries/i }));
    expect(screen.getByText(/\[VERBOSE\] Trace dump line/)).toBeInTheDocument();
    expect(screen.queryByText('Normal line')).not.toBeInTheDocument();
  });

  // ===========================================================================
  // Entry rendering: badges
  // ===========================================================================

  it('renders [System] badge for system entries', () => {
    act(() => {
      useActivityStore.setState({
        entries: [
          makeEntry({ _id: 1, download_id: 'system', line: 'App started' }),
        ],
      });
    });
    render(<ActivityLog />);
    expect(screen.getByText(/\[System\]/)).toBeInTheDocument();
  });

  it('renders [shortId] badge (truncated to 8 chars) for download entries', () => {
    act(() => {
      useActivityStore.setState({
        entries: [
          makeEntry({
            _id: 1,
            download_id: 'abcd1234-de00-4000-9000-000000000000',
            line: 'Started',
          }),
        ],
      });
    });
    render(<ActivityLog />);
    // shortId truncates to first 8 chars of the UUID
    expect(screen.getByText(/\[abcd1234\]/)).toBeInTheDocument();
  });

  it('renders [MeedyaDL] badge for internal-stream entries', () => {
    act(() => {
      useActivityStore.setState({
        entries: [
          makeEntry({
            _id: 1,
            download_id: 'abcd1234',
            stream: 'internal',
            line: 'Enrichment stage 1/8',
          }),
        ],
      });
    });
    render(<ActivityLog />);
    expect(screen.getByText(/\[MeedyaDL\]/)).toBeInTheDocument();
  });

  // ===========================================================================
  // Action buttons + gating
  // ===========================================================================

  /** Opens the header's More menu and returns its items by name. */
  function openMore() {
    fireEvent.click(screen.getByRole('button', { name: /^more/i }));
    return screen.getByRole('menu', { name: /more activity log actions/i });
  }

  it('the header holds Auto-scroll, Clear and More, and nothing else', () => {
    render(<ActivityLog />);
    const header = screen.getByRole('banner');
    const buttons = [...header.querySelectorAll('button')].map((b) => b.textContent?.trim() || b.getAttribute('role'));
    // The switch (role="switch", no text of its own), Clear, More.
    expect(buttons).toEqual(['switch', 'Clear', 'More']);
    expect(screen.getByRole('switch', { name: /auto-scroll/i })).toBeInTheDocument();
  });

  it('Export the lines shown and Clear are unavailable when there are no entries', () => {
    render(<ActivityLog />);
    expect(screen.getByRole('button', { name: /^clear$/i })).toBeDisabled();
    openMore();
    expect(screen.getByRole('menuitem', { name: /export the lines shown/i })).toBeDisabled();
  });

  it('Export the lines shown and Clear are available once entries exist', () => {
    act(() => {
      useActivityStore.setState({
        entries: [makeEntry({ _id: 1 })],
      });
    });
    render(<ActivityLog />);
    expect(screen.getByRole('button', { name: /^clear$/i })).toBeEnabled();
    openMore();
    expect(screen.getByRole('menuitem', { name: /export the lines shown/i })).toBeEnabled();
  });

  it('the full log export and the logs folder are always offered (not gated on entry count)', () => {
    render(<ActivityLog />);
    openMore();
    expect(screen.getByRole('menuitem', { name: /export the full log/i })).toBeEnabled();
    expect(screen.getByRole('menuitem', { name: /open the logs folder/i })).toBeEnabled();
  });

  it('Auto-scroll is the shared switch and reflects the paused state', () => {
    act(() => {
      useActivityStore.setState({ paused: true });
    });
    render(<ActivityLog />);
    const autoScroll = screen.getByRole('switch', { name: /auto-scroll/i });
    expect(autoScroll).toHaveAttribute('aria-checked', 'false'); // paused = !auto-scroll
    fireEvent.click(autoScroll);
    expect(useActivityStore.getState().paused).toBe(false);
  });

  it('shows the empty state in ordinary text, not the log\'s monospace', () => {
    render(<ActivityLog />);
    const empty = screen.getByText('No activity yet').closest('div');
    expect(empty?.className).toContain('font-sans');
  });
});
