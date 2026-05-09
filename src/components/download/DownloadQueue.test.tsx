// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for DownloadQueue (#232 — second installment).
 *
 * Scope: render-state behaviour driven by `queueItems` shape.
 * Specifically:
 *   - Empty state (icon + helper text)
 *   - Header subtitle pluralisation
 *   - Stats bar segments (per-state counts, only-when-non-zero)
 *   - Conditional action buttons:
 *     - Start Queue: queued > 0 AND active === 0
 *     - Export: items > 0; disabled when exportable === 0
 *     - Clear Completed: finished > 0
 *     - Retry All Failed: failed > 0
 *     - Clear All: items > 0
 *     - Abort Queue: active > 0 OR queued > 0
 *   - Refresh button always present
 *   - Confirmation modals open on click (Clear All, Retry All, Abort)
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
  // Conditional action buttons
  // ===========================================================================

  it('Refresh button is always present', () => {
    render(<DownloadQueue />);
    expect(screen.getByRole('button', { name: /refresh/i })).toBeInTheDocument();
  });

  it('Import button is always present', () => {
    render(<DownloadQueue />);
    expect(screen.getByRole('button', { name: /^import$/i })).toBeInTheDocument();
  });

  it('Start Queue button shows when queued > 0 AND no active items', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'queued' })],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByRole('button', { name: /start queue \(1\)/i })).toBeInTheDocument();
  });

  it('Start Queue button hides when there are active downloads', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'queued' }),
          makeItem({ id: 'b', state: 'downloading' }),
        ],
      });
    });
    render(<DownloadQueue />);
    expect(screen.queryByRole('button', { name: /start queue/i })).not.toBeInTheDocument();
  });

  it('Clear Completed button shows ONLY when finished items exist (complete/cancelled)', () => {
    // No completed items → button hidden
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'queued' })],
      });
    });
    const { rerender } = render(<DownloadQueue />);
    expect(screen.queryByRole('button', { name: /clear completed/i })).not.toBeInTheDocument();

    // Add a completed item → button appears with count
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'queued' }),
          makeItem({ id: 'b', state: 'complete' }),
        ],
      });
    });
    rerender(<DownloadQueue />);
    expect(screen.getByRole('button', { name: /clear completed \(1\)/i })).toBeInTheDocument();
  });

  it('Retry All Failed button shows ONLY when failed > 0', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'error' }),
          makeItem({ id: 'b', state: 'error' }),
        ],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByRole('button', { name: /retry all failed \(2\)/i })).toBeInTheDocument();
  });

  it('Abort Queue button shows when there are active OR queued items', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'queued' })],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByRole('button', { name: /abort queue/i })).toBeInTheDocument();
  });

  it('Abort Queue button hides when only terminal items exist', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'complete' }),
          makeItem({ id: 'b', state: 'error' }),
        ],
      });
    });
    render(<DownloadQueue />);
    expect(screen.queryByRole('button', { name: /abort queue/i })).not.toBeInTheDocument();
  });

  it('Export button shows when queue has items, with active count in label', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [
          makeItem({ id: 'a', state: 'queued' }),
          makeItem({ id: 'b', state: 'downloading' }),
        ],
      });
    });
    render(<DownloadQueue />);
    expect(screen.getByRole('button', { name: /export \(2\)/i })).toBeInTheDocument();
  });

  // ===========================================================================
  // Confirmation modals
  // ===========================================================================

  it('opens the Retry All Failed confirmation modal on click', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'error' })],
      });
    });
    render(<DownloadQueue />);
    fireEvent.click(screen.getByRole('button', { name: /retry all failed/i }));
    expect(screen.getByText(/Retry All Failed Downloads/i)).toBeInTheDocument();
    expect(
      screen.getByText(/will re-queue 1 failed download/i)
    ).toBeInTheDocument();
  });

  it('opens the Clear All confirmation modal on click', () => {
    act(() => {
      useDownloadStore.setState({
        queueItems: [makeItem({ id: 'a', state: 'queued' })],
      });
    });
    render(<DownloadQueue />);
    fireEvent.click(screen.getByRole('button', { name: /^clear all$/i }));
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
    fireEvent.click(screen.getByRole('button', { name: /abort queue/i }));
    // Modal title from the source: "Abort Queue?" inside the modal body
    expect(screen.getAllByText(/abort/i).length).toBeGreaterThan(1);
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
