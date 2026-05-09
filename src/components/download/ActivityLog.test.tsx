// Copyright (c) 2026 MeedyaDL
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
 *   - Category toggles (System / Download / Verbose) — aria-checked
 *     state + visible/hidden filter outcome
 *   - Filtered-to-empty state ("No entries match…")
 *   - Entry rendering: [System] badge, [shortId] badge, [MeedyaDL]
 *     badge for internal-stream entries
 *   - Action buttons: Export / Export Disk / Reveal / Clear with
 *     enabled/disabled gating on entry count
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
 *   - `@tauri-apps/plugin-shell` — `open()` stub so the Reveal
 *     button doesn't throw on click.
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

vi.mock('@tauri-apps/plugin-shell', () => ({
  open: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@/lib/tauri-commands', () => ({
  exportActivityLog: vi.fn().mockResolvedValue(undefined),
  exportDiskActivityLog: vi.fn().mockResolvedValue(1024),
  getLogsFolderPath: vi.fn().mockResolvedValue('/var/log/meedyadl'),
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
    const { rerender } = render(<ActivityLog />);
    expect(screen.getByText('1 line')).toBeInTheDocument();

    act(() => {
      useActivityStore.setState({
        entries: [makeEntry({ _id: 1 }), makeEntry({ _id: 2 })],
      });
    });
    rerender(<ActivityLog />);
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
    expect(screen.getByText(/2 of 3 lines/i)).toBeInTheDocument();
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

  it('System / Download / Verbose toggles render with correct initial aria-checked', () => {
    render(<ActivityLog />);
    expect(screen.getByRole('checkbox', { name: /filter system entries/i })).toHaveAttribute(
      'aria-checked',
      'true'
    );
    expect(
      screen.getByRole('checkbox', { name: /filter download entries/i })
    ).toHaveAttribute('aria-checked', 'true');
    // Verbose starts off.
    expect(
      screen.getByRole('checkbox', { name: /filter verbose entries/i })
    ).toHaveAttribute('aria-checked', 'false');
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
    fireEvent.click(screen.getByRole('checkbox', { name: /filter system entries/i }));
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
    fireEvent.click(screen.getByRole('checkbox', { name: /filter download entries/i }));
    expect(screen.queryByText(/\[VERBOSE\] Trace dump line/)).not.toBeInTheDocument();
    expect(screen.queryByText('Normal line')).not.toBeInTheDocument();

    // Turn Verbose ON — only the verbose line comes back.
    fireEvent.click(screen.getByRole('checkbox', { name: /filter verbose entries/i }));
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

  it('Export and Clear buttons are disabled when entries are empty', () => {
    render(<ActivityLog />);
    expect(screen.getByRole('button', { name: /^export$/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /^clear$/i })).toBeDisabled();
  });

  it('Export and Clear buttons are enabled once entries exist', () => {
    act(() => {
      useActivityStore.setState({
        entries: [makeEntry({ _id: 1 })],
      });
    });
    render(<ActivityLog />);
    expect(screen.getByRole('button', { name: /^export$/i })).toBeEnabled();
    expect(screen.getByRole('button', { name: /^clear$/i })).toBeEnabled();
  });

  it('Export Disk and Reveal buttons are always present (not gated on entry count)', () => {
    render(<ActivityLog />);
    expect(screen.getByRole('button', { name: /export disk/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^reveal$/i })).toBeInTheDocument();
  });

  it('Auto-scroll checkbox renders + reflects paused state', () => {
    act(() => {
      useActivityStore.setState({ paused: true });
    });
    render(<ActivityLog />);
    const checkbox = screen.getByLabelText(/auto-scroll/i) as HTMLInputElement;
    expect(checkbox.checked).toBe(false); // paused = !auto-scroll
  });
});
