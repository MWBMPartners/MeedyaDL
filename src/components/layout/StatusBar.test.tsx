/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/layout/StatusBar.test.tsx
 *
 * Unit tests for the StatusBar component. The StatusBar subscribes to the
 * download store's `queueItems` array and derives display counters from
 * the items' states (downloading, queued, completed). It also shows the
 * app version string.
 *
 * These tests manipulate the Zustand download store directly via
 * `useDownloadStore.setState()` to simulate various queue configurations.
 *
 * @see src/components/layout/StatusBar.tsx - The component under test
 * @see src/stores/downloadStore.ts - Source of queue data
 */

import { render, screen } from '@testing-library/react';

import { StatusBar } from '@/components/layout/StatusBar';
import { useDownloadStore } from '@/stores/downloadStore';

import type { QueueItemStatus } from '@/types';

/**
 * Factory helper to create a mock queue item with a given state.
 * Provides reasonable defaults for all required fields so tests
 * can focus on the state field that drives StatusBar rendering.
 */
function createItem(
  state: QueueItemStatus['state'],
  id?: string,
): QueueItemStatus {
  return {
    id: id || `item-${Math.random().toString(36).slice(2)}`,
    urls: ['https://music.apple.com/us/album/test/123'],
    state,
    progress: state === 'complete' ? 100 : 0,
    current_track: null,
    total_tracks: null,
    completed_tracks: null,
    speed: null,
    eta: null,
    error: state === 'error' ? 'Test error' : null,
    output_path: state === 'complete' ? '/tmp/output' : null,
    codec_used: 'alac',
    fallback_occurred: false,
    created_at: new Date().toISOString(),
  };
}

/** Reset the download store to a clean state before each test. */
beforeEach(() => {
  useDownloadStore.setState({
    queueItems: [],
  });
});

describe('StatusBar', () => {
  // =========================================================================
  // Empty queue
  // =========================================================================

  /** When no items are in the queue, "No downloads" should be displayed. */
  it('shows "No downloads" when queue is empty', () => {
    render(<StatusBar />);

    expect(screen.getByText('No downloads')).toBeInTheDocument();
  });

  // =========================================================================
  // Active downloads
  // =========================================================================

  /** Items in 'downloading' state should be counted as active. */
  it('shows active download count for downloading items', () => {
    useDownloadStore.setState({
      queueItems: [
        createItem('downloading'),
        createItem('downloading'),
      ],
    });

    render(<StatusBar />);

    expect(screen.getByText(/2 downloading/)).toBeInTheDocument();
  });

  /** Items in 'processing' state should also count as active. */
  it('includes processing items in active count', () => {
    useDownloadStore.setState({
      queueItems: [
        createItem('downloading'),
        createItem('processing'),
      ],
    });

    render(<StatusBar />);

    expect(screen.getByText(/2 downloading/)).toBeInTheDocument();
  });

  // =========================================================================
  // Queued count
  // =========================================================================

  /** Items waiting in the 'queued' state should be displayed. */
  it('shows queued count', () => {
    useDownloadStore.setState({
      queueItems: [
        createItem('queued'),
        createItem('queued'),
        createItem('queued'),
      ],
    });

    render(<StatusBar />);

    expect(screen.getByText(/3 queued/)).toBeInTheDocument();
  });

  // =========================================================================
  // Completed count
  // =========================================================================

  /** Successfully finished items should show the completed count. */
  it('shows completed count', () => {
    useDownloadStore.setState({
      queueItems: [
        createItem('complete'),
      ],
    });

    render(<StatusBar />);

    expect(screen.getByText(/1 completed/)).toBeInTheDocument();
  });

  // =========================================================================
  // Mixed states
  // =========================================================================

  /** When the queue has items in multiple states, all counters appear. */
  it('shows multiple counters for mixed queue states', () => {
    useDownloadStore.setState({
      queueItems: [
        createItem('downloading'),
        createItem('queued'),
        createItem('queued'),
        createItem('complete'),
        createItem('complete'),
        createItem('complete'),
      ],
    });

    render(<StatusBar />);

    expect(screen.getByText(/1 downloading/)).toBeInTheDocument();
    expect(screen.getByText(/2 queued/)).toBeInTheDocument();
    expect(screen.getByText(/3 completed/)).toBeInTheDocument();
  });

  // =========================================================================
  // Non-counted states
  // =========================================================================

  /**
   * Error and cancelled items are in the queue but not shown as specific
   * counters — only downloading, queued, and completed have dedicated displays.
   * The "No downloads" placeholder should NOT appear since items exist.
   */
  it('does not show "No downloads" when only error/cancelled items exist', () => {
    useDownloadStore.setState({
      queueItems: [
        createItem('error'),
        createItem('cancelled'),
      ],
    });

    render(<StatusBar />);

    /* Items exist, so "No downloads" should not appear.
     * But no specific counters for error/cancelled are shown either. */
    expect(screen.queryByText('No downloads')).not.toBeInTheDocument();
  });

  // =========================================================================
  // Version string
  // =========================================================================

  /** The app version should always be displayed on the right side. */
  it('shows the app version string', () => {
    render(<StatusBar />);

    expect(screen.getByText(/MeedyaDL v/)).toBeInTheDocument();
  });
});
