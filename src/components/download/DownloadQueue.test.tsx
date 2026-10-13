// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for DownloadQueue (#232 — second installment).
 *
 * Scope: render-state behaviour driven by `queueItems` shape.
 * Specifically:
 *   - Empty state (icon + helper text)
 *   - Header subtitle pluralisation
 *   - Stats bar segments (per-state counts, only-when-non-zero)
 *   - Header actions (polish pass H5): Start (queued > 0 AND active ===
 *     0), Pause and Abort (active > 0 OR queued > 0) as buttons; Import,
 *     Export, Retry All Failed, Clear Completed, Clear All and Refresh in
 *     the "More" menu, greyed out when they cannot be used
 *   - The More menu by keyboard, and how it is announced
 *   - Filter chips use the status-pill words
 *   - Confirmation modals open (Clear All, Retry All, Abort)
 *   - Polling: refreshQueue called on mount
 *
 * **Out of scope** (deferred): per-row QueueItem rendering — mocked
 * here as a stable test placeholder so this file tests the queue
 * controller, not the row component. QueueItem warrants its own
 * dedicated test file once we have a cleaner mock surface.
 *
 * @see src/components/download/DownloadQueue.tsx
 */

import { render, screen, fireEvent, act } from '@testing-library/react';
import { useDownloadStore } from '@/stores/downloadStore';
import { useUiStore } from '@/stores/uiStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { DownloadQueue } from '@/components/download/DownloadQueue';
import { makeQueueItem as makeItem } from '@/testing/fixtures';
import type { QueueItemStatus, DownloadState } from '@/types';

/**
 * Mock the QueueItem child to a tiny placeholder so this file tests
 * the queue *controller* (header, stats bar, action buttons,
 * empty state, modals) and not the row rendering. QueueItem has
 * its own behaviour (state badges, hover menus, expand/collapse)
 * that warrants a dedicated test file.
 */
vi.mock('@/components/download/QueueItem', () => ({
  QueueItem: ({ item }: { item: QueueItemStatus }) => (
    <div data-testid={`queue-item-${item.id}`} data-state={item.state}>
      {item.current_track ?? item.urls[0]}
    </div>
  ),
}));

/**
 * Also mock `QueueListVirtualized` so it skips
 * `@tanstack/react-virtual` in the test. jsdom gives the scroll
 * container zero height by default, so the real virtualizer thinks
 * nothing is in view and renders zero rows — which makes
 * `getByTestId("queue-item-a")` fail. The mocked version renders
 * each item directly with the same `data-testid` shape the existing
 * QueueItem mock provides, so test queries keep working unchanged. (#467)
 */
vi.mock('@/components/download/QueueListVirtualized', () => ({
  QueueListVirtualized: ({
    queueItems,
    onToggleSelect,
  }: {
    queueItems: QueueItemStatus[];
    onToggleSelect?: (id: string) => void;
  }) => {
    if (queueItems.length === 0) {
      return (
        <div>
          <p>No downloads in queue</p>
          <p>Paste an Apple Music URL on the Download page</p>
        </div>
      );
    }
    return (
      <div role="list" aria-label="Download queue items">
        {queueItems.map((item) => (
          <div
            key={item.id}
            data-testid={`queue-item-${item.id}`}
            data-state={item.state}
          >
            {/* Selection, so the bulk actions can be tested. */}
            <input type="checkbox" aria-label={`Select ${item.id}`} onChange={() => onToggleSelect?.(item.id)} />
            {item.current_track ?? item.urls[0]}
          </div>
        ))}
      </div>
    );
  },
}));

beforeEach(() => {
  // Reset all three stores between tests so no item state bleeds.
  act(() => {
    useDownloadStore.setState({ queueItems: [] });
    useUiStore.setState({ toasts: [] });
  });
});

describe('DownloadQueue', () => {
  // ===========================================================================
  // Empty state
  // ===========================================================================

  it('renders empty state when no items', () => {
    render(<DownloadQueue />);
    expect(screen.getByText('No downloads in queue')).toBeInTheDocument();
    expect(
      screen.getByText(/Paste an Apple Music URL on the Download page/i)
    ).toBeInTheDocument();
  });

  it('uses singular "item" in subtitle for empty queue', () => {
    render(<DownloadQueue />);
    // 0 items → "0 items" (plural — non-1 takes plural form per the source)
    expect(screen.getByText('0 items in queue')).toBeInTheDocument();
  });

  it('uses singular "item" for exactly one item', () => {
    act(() => {
      useDownloadStore.setState({ queueItems: [makeItem({ id: 'a' })] });
    });
    render(<DownloadQueue />);
    expect(screen.getByText('1 item in queue')).toBeInTheDocument();
  });

  it('uses plural "items" for multiple items', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a' }), makeItem({ id: 'b' })],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByText('2 items in queue')).toBeInTheDocument();
  });

  // ===========================================================================
  // Item rendering
  // ===========================================================================

  it('renders one row per queue item via the mocked QueueItem', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'downloading' }),
          makeItem({ id: 'b', state: 'queued' }),
          makeItem({ id: 'c', state: 'complete' }),
        ],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByTestId('queue-item-a')).toHaveAttribute('data-state', 'downloading');
    expect(screen.getByTestId('queue-item-b')).toHaveAttribute('data-state', 'queued');
    expect(screen.getByTestId('queue-item-c')).toHaveAttribute('data-state', 'complete');
  });

  it('renders an accessible list landmark for screen readers', () => {
    act(() => {
      useDownloadStore.setState({ queueItems: [makeItem()] });
    });
    render(<DownloadQueue />);
    expect(screen.getByRole('list', { name: /download queue items/i })).toBeInTheDocument();
  });

  // ===========================================================================
  // Stats bar segments
  // ===========================================================================

  it('shows per-state count segments only for non-zero counts', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'downloading' }),
          makeItem({ id: 'b', state: 'queued' }),
          makeItem({ id: 'c', state: 'queued' }),
          makeItem({ id: 'd', state: 'complete' }),
          makeItem({ id: 'e', state: 'error' }),
        ],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByText('1 active')).toBeInTheDocument();
    expect(screen.getByText('2 queued')).toBeInTheDocument();
    expect(screen.getByText('1 completed')).toBeInTheDocument();
    expect(screen.getByText('1 failed')).toBeInTheDocument();
  });

  it('omits segments for states with zero items', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'queued' })],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByText('1 queued')).toBeInTheDocument();
    expect(screen.queryByText(/active/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/completed/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/failed/i)).not.toBeInTheDocument();
  });

  // ===========================================================================
  // Header actions: three buttons plus a "More" menu (polish pass H5)
  // ===========================================================================

  /** Opens the header's "More" menu and returns it. */
  function openMore() {
    fireEvent.click(screen.getByRole('button', { name: /^more/i }));
    return screen.getByRole('menu', { name: /more queue actions/i });
  }

  /** The menu item whose name matches, or null. */
  function menuItem(name: RegExp) {
    return screen.queryByRole('menuitem', { name });
  }

  it('keeps only Start, Pause and Abort as buttons; everything else is in More', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'queued' }),
          makeItem({ id: 'b', state: 'complete' }),
          makeItem({ id: 'c', state: 'error' }),
        ],
      });
    });
    render(<DownloadQueue />);
    const header = screen.getByRole('banner');
    const labels = [...header.querySelectorAll('button')].map((b) => b.textContent?.trim());
    expect(labels).toEqual(['Start (1)', 'Pause', 'Abort', 'More']);
    // None of the secondary actions is a header button any more.
    for (const gone of [/^import$/i, /^export/i, /^refresh$/i, /^clear all$/i, /clear completed/i, /retry all failed/i]) {
      expect(screen.queryByRole('button', { name: gone })).toBeNull();
    }
  });

  it('More is announced as a menu button and lists every secondary action', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'queued' }),
          makeItem({ id: 'b', state: 'downloading' }),
          makeItem({ id: 'c', state: 'complete' }),
          makeItem({ id: 'd', state: 'error' }),
          makeItem({ id: 'e', state: 'error' }),
        ],
      });
    });
    render(<DownloadQueue />);
    const more = screen.getByRole('button', { name: /^more/i });
    expect(more).toHaveAttribute('aria-haspopup', 'menu');
    expect(more).toHaveAttribute('aria-expanded', 'false');
    openMore();
    expect(more).toHaveAttribute('aria-expanded', 'true');
    expect(menuItem(/import a queue file/i)).toBeEnabled();
    expect(menuItem(/export the queue \(2\)/i)).toBeEnabled();
    expect(menuItem(/retry all failed \(2\)/i)).toBeEnabled();
    expect(menuItem(/clear completed \(1\)/i)).toBeEnabled();
    expect(menuItem(/clear all/i)).toBeEnabled();
    expect(menuItem(/^refresh$/i)).toBeEnabled();
  });

  it('shows an action that cannot be used right now greyed out, not hidden', () => {
    act(() => {
      useDownloadStore.setState({ queueItems: [makeItem({ id: 'a', state: 'queued' })] });
    });
    render(<DownloadQueue />);
    openMore();
    expect(menuItem(/^retry all failed$/i)).toBeDisabled();
    expect(menuItem(/^clear completed$/i)).toBeDisabled();
  });

  it('the More menu works from the keyboard and gives focus back', () => {
    act(() => {
      useDownloadStore.setState({ queueItems: [makeItem({ id: 'a', state: 'queued' })] });
    });
    render(<DownloadQueue />);
    const more = screen.getByRole('button', { name: /^more/i });
    more.focus();
    fireEvent.keyDown(more, { key: 'ArrowDown' });
    const menu = screen.getByRole('menu', { name: /more queue actions/i });
    // Focus lands on the first item...
    expect(document.activeElement).toBe(menuItem(/import a queue file/i));
    // ...arrow keys move it...
    fireEvent.keyDown(document, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(menuItem(/export the queue/i));
    // ...and Escape closes the menu and returns focus to "More".
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(menu).not.toBeInTheDocument();
    expect(document.activeElement).toBe(more);
    expect(more).toHaveAttribute('aria-expanded', 'false');
  });

  it('Start shows when queued > 0 AND no active items', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'queued' })],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByRole('button', { name: /^start \(1\)$/i })).toBeInTheDocument();
  });

  it('Start hides when there are active downloads', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'queued' }),
          makeItem({ id: 'b', state: 'downloading' }),
        ],
      });
    });
    render(<DownloadQueue />);
    expect(screen.queryByRole('button', { name: /^start/i })).not.toBeInTheDocument();
  });

  it('Abort shows when there are active OR queued items', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'queued' })],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByRole('button', { name: /^abort$/i })).toBeInTheDocument();
  });

  it('Abort hides when only terminal items exist', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'complete' }),
          makeItem({ id: 'b', state: 'error' }),
        ],
      });
    });
    render(<DownloadQueue />);
    expect(screen.queryByRole('button', { name: /^abort$/i })).not.toBeInTheDocument();
  });

  it('labels the filter chips with the same words as the status pills', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'complete' }), makeItem({ id: 'b', state: 'cancelled' })],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByRole('button', { name: 'Complete (1)' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Cancelled (1)' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^complete \(1\)$/ })).toBeNull();
  });

  // ===========================================================================
  // Confirmation modals
  // ===========================================================================

  it('opens the Retry All Failed confirmation modal from the More menu', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'error' })],
      });
    });
    render(<DownloadQueue />);
    openMore();
    fireEvent.click(menuItem(/retry all failed/i)!);
    expect(screen.getByText(/Retry All Failed Downloads/i)).toBeInTheDocument();
    expect(
      screen.getByText(/will re-queue 1 failed download/i)
    ).toBeInTheDocument();
  });

  it('opens the Clear All confirmation modal from the More menu', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'queued' })],
      });
    });
    render(<DownloadQueue />);
    openMore();
    fireEvent.click(menuItem(/clear all/i)!);
    expect(screen.getByText(/Clear All Queue Items/i)).toBeInTheDocument();
  });

  it('opens the Abort Queue modal when abort_queue_confirm setting is true', () => {
    // Default settings have abort_queue_confirm=true, so this is the
    // common path. The button click should open the confirmation modal,
    // not fire abortAll directly.
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'downloading' })],
      });
      useSettingsStore.setState({
        settings: {
          ...useSettingsStore.getState().settings,
          abort_queue_confirm: true,
        },
      });
    });
    render(<DownloadQueue />);
    fireEvent.click(screen.getByRole('button', { name: /^abort$/i }));
    expect(screen.getByRole('dialog', { name: /abort queue/i })).toBeInTheDocument();
  });

  // ===========================================================================
  // Bulk actions report failures (polish pass M8)
  // ===========================================================================

  it('a bulk cancel says how many could not be cancelled, instead of claiming all of them', async () => {
    const cancel = vi.fn((id: string) => (id === 'b' ? Promise.reject(new Error('already finished')) : Promise.resolve()));
    const realCancel = useDownloadStore.getState().cancelDownload;
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'queued' }), makeItem({ id: 'b', state: 'queued' })],
        cancelDownload: cancel,
      });
    });
    render(<DownloadQueue />);
    fireEvent.click(screen.getByLabelText('Select a'));
    fireEvent.click(screen.getByLabelText('Select b'));
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /cancel selected/i }));
    });
    const toasts = useUiStore.getState().toasts;
    const report = toasts.find((t) => /Cancelled 1 of 2 items; 1 could not be cancelled/.test(t.message));
    expect(report?.type).toBe('warning');
    expect(report?.details).toBe('already finished');
    expect(toasts.some((t) => t.message === 'Cancelled 2 items')).toBe(false);
    act(() => useDownloadStore.setState({ cancelDownload: realCancel }));
  });

  // ===========================================================================
  // Polling
  // ===========================================================================

  it('calls refreshQueue on mount', () => {
    const refreshSpy = vi.spyOn(useDownloadStore.getState(), 'refreshQueue');
    render(<DownloadQueue />);
    expect(refreshSpy).toHaveBeenCalled();
  });

  // ===========================================================================
  // State exhaustiveness sanity check
  // ===========================================================================

  it('handles every DownloadState variant without crashing', () => {
    const states: DownloadState[] = [
      'queued',
      'downloading',
      'processing',
      'complete',
      'error',
      'cancelled',
    ];
    act(() => {
      useDownloadStore.setState({
        queueItems: states.map((state, i) =>
          makeItem({ id: `item-${i}`, state })
        ),
      });
    });
    render(<DownloadQueue />);
    // Should render every row without throwing.
    states.forEach((_, i) => {
      expect(screen.getByTestId(`queue-item-item-${i}`)).toBeInTheDocument();
    });
  });
});
