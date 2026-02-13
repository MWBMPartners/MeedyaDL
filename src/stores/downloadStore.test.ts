/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/stores/downloadStore.test.ts - Unit tests for the download store
 *
 * Tests URL input validation, download submission, queue event handlers,
 * and queue management actions (cancel, retry, clear).
 *
 * The Tauri IPC commands are mocked to run without a Rust backend.
 * The url-parser module is NOT mocked -- it is a pure function that
 * runs fine in the test environment.
 *
 * @see src/stores/downloadStore.ts - The store under test
 */

import { useDownloadStore } from '@/stores/downloadStore';
import * as commands from '@/lib/tauri-commands';

import type { QueueItemStatus, GamdlProgress } from '@/types';

/**
 * Mock the tauri-commands module so we can control IPC responses.
 */
vi.mock('@/lib/tauri-commands', () => ({
  startDownload: vi.fn(),
  cancelDownload: vi.fn(),
  retryDownload: vi.fn(),
  clearQueue: vi.fn(),
  getQueueStatus: vi.fn(),
}));

/**
 * Factory to create a mock QueueItemStatus with sensible defaults.
 * Individual fields can be overridden via the `overrides` parameter.
 */
function createMockQueueItem(overrides: Partial<QueueItemStatus> = {}): QueueItemStatus {
  return {
    id: 'test-id-1',
    urls: ['https://music.apple.com/us/album/test/1234567890'],
    state: 'queued',
    progress: 0,
    current_track: null,
    total_tracks: null,
    completed_tracks: null,
    speed: null,
    eta: null,
    error: null,
    output_path: null,
    codec_used: null,
    fallback_occurred: false,
    created_at: '2026-02-09T12:00:00Z',
    ...overrides,
  };
}

/**
 * Reset the store and mocks before each test.
 */
beforeEach(() => {
  vi.clearAllMocks();
  useDownloadStore.setState({
    urlInput: '',
    urlIsValid: false,
    urlContentType: 'unknown',
    overrideOptions: null,
    queueItems: [],
    isSubmitting: false,
    error: null,
  });
});

describe('downloadStore', () => {
  // =========================================================================
  // URL Input Validation
  // =========================================================================
  describe('setUrlInput', () => {
    it('validates a valid album URL', () => {
      useDownloadStore.getState().setUrlInput(
        'https://music.apple.com/us/album/midnights/1649434004',
      );

      const state = useDownloadStore.getState();
      expect(state.urlInput).toBe('https://music.apple.com/us/album/midnights/1649434004');
      expect(state.urlIsValid).toBe(true);
      expect(state.urlContentType).toBe('album');
    });

    it('validates a valid song URL', () => {
      useDownloadStore.getState().setUrlInput(
        'https://music.apple.com/us/album/anti-hero/1649434004?i=1649434038',
      );

      expect(useDownloadStore.getState().urlIsValid).toBe(true);
      expect(useDownloadStore.getState().urlContentType).toBe('song');
    });

    it('validates a valid playlist URL', () => {
      useDownloadStore.getState().setUrlInput(
        'https://music.apple.com/us/playlist/todays-hits/pl.f4d106fed2bd41149aaacabb233eb5eb',
      );

      expect(useDownloadStore.getState().urlIsValid).toBe(true);
      expect(useDownloadStore.getState().urlContentType).toBe('playlist');
    });

    it('validates a valid music video URL', () => {
      useDownloadStore.getState().setUrlInput(
        'https://music.apple.com/us/music-video/bad-blood/1445927585',
      );

      expect(useDownloadStore.getState().urlIsValid).toBe(true);
      expect(useDownloadStore.getState().urlContentType).toBe('music-video');
    });

    it('rejects an invalid URL', () => {
      useDownloadStore.getState().setUrlInput('not-a-url');

      expect(useDownloadStore.getState().urlIsValid).toBe(false);
      expect(useDownloadStore.getState().urlContentType).toBe('unknown');
    });

    it('rejects a non-Apple Music URL', () => {
      useDownloadStore.getState().setUrlInput('https://open.spotify.com/track/12345');

      expect(useDownloadStore.getState().urlIsValid).toBe(false);
    });

    it('handles empty input', () => {
      useDownloadStore.getState().setUrlInput('');

      expect(useDownloadStore.getState().urlIsValid).toBe(false);
      expect(useDownloadStore.getState().urlContentType).toBe('unknown');
    });
  });

  // =========================================================================
  // Download Submission
  // =========================================================================
  describe('submitDownload', () => {
    it('submits a valid URL and clears input on success', async () => {
      /* Set up a valid URL */
      useDownloadStore.getState().setUrlInput(
        'https://music.apple.com/us/album/midnights/1649434004',
      );

      vi.mocked(commands.startDownload).mockResolvedValueOnce('dl-id-123');

      const downloadId = await useDownloadStore.getState().submitDownload();

      expect(downloadId).toBe('dl-id-123');
      expect(commands.startDownload).toHaveBeenCalledWith({
        urls: ['https://music.apple.com/us/album/midnights/1649434004'],
        options: undefined,
      });
      /* Input should be cleared after successful submission */
      expect(useDownloadStore.getState().urlInput).toBe('');
      expect(useDownloadStore.getState().urlIsValid).toBe(false);
      expect(useDownloadStore.getState().isSubmitting).toBe(false);
    });

    it('rejects submission when URL is invalid', async () => {
      useDownloadStore.getState().setUrlInput('invalid-url');

      await expect(
        useDownloadStore.getState().submitDownload(),
      ).rejects.toThrow('Invalid Apple Music URL');

      expect(useDownloadStore.getState().error).toBe('Invalid Apple Music URL');
    });

    it('passes override options when set', async () => {
      useDownloadStore.getState().setUrlInput(
        'https://music.apple.com/us/album/midnights/1649434004',
      );
      useDownloadStore.getState().setOverrideOptions({ song_codec: 'atmos' });

      vi.mocked(commands.startDownload).mockResolvedValueOnce('dl-id-456');

      await useDownloadStore.getState().submitDownload();

      expect(commands.startDownload).toHaveBeenCalledWith({
        urls: ['https://music.apple.com/us/album/midnights/1649434004'],
        options: { song_codec: 'atmos' },
      });
    });

    it('sets error on backend failure', async () => {
      useDownloadStore.getState().setUrlInput(
        'https://music.apple.com/us/album/midnights/1649434004',
      );
      vi.mocked(commands.startDownload).mockRejectedValueOnce('Network timeout');

      await expect(
        useDownloadStore.getState().submitDownload(),
      ).rejects.toThrow('Network timeout');

      expect(useDownloadStore.getState().error).toBe('Network timeout');
      expect(useDownloadStore.getState().isSubmitting).toBe(false);
    });
  });

  // =========================================================================
  // Override Options
  // =========================================================================
  describe('setOverrideOptions', () => {
    it('sets override options', () => {
      useDownloadStore.getState().setOverrideOptions({ song_codec: 'aac' });
      expect(useDownloadStore.getState().overrideOptions).toEqual({ song_codec: 'aac' });
    });

    it('clears override options with null', () => {
      useDownloadStore.getState().setOverrideOptions({ song_codec: 'aac' });
      useDownloadStore.getState().setOverrideOptions(null);
      expect(useDownloadStore.getState().overrideOptions).toBeNull();
    });
  });

  // =========================================================================
  // clearInput
  // =========================================================================
  describe('clearInput', () => {
    it('resets all input-related state', () => {
      useDownloadStore.getState().setUrlInput(
        'https://music.apple.com/us/album/midnights/1649434004',
      );
      useDownloadStore.getState().setOverrideOptions({ song_codec: 'aac' });

      useDownloadStore.getState().clearInput();

      const state = useDownloadStore.getState();
      expect(state.urlInput).toBe('');
      expect(state.urlIsValid).toBe(false);
      expect(state.urlContentType).toBe('unknown');
      expect(state.overrideOptions).toBeNull();
    });
  });

  // =========================================================================
  // Progress Event Handlers
  // =========================================================================
  describe('handleProgressEvent', () => {
    it('updates progress on download_progress event', () => {
      const item = createMockQueueItem({ id: 'dl-1', state: 'queued' });
      useDownloadStore.setState({ queueItems: [item] });

      const progress: GamdlProgress = {
        download_id: 'dl-1',
        event: { type: 'download_progress', percent: 45, speed: '2.5 MB/s', eta: '01:30' },
      };
      useDownloadStore.getState().handleProgressEvent(progress);

      const updated = useDownloadStore.getState().queueItems[0];
      expect(updated.progress).toBe(45);
      expect(updated.speed).toBe('2.5 MB/s');
      expect(updated.eta).toBe('01:30');
      expect(updated.state).toBe('downloading');
    });

    it('updates track info on track_info event', () => {
      const item = createMockQueueItem({ id: 'dl-1', state: 'downloading' });
      useDownloadStore.setState({ queueItems: [item] });

      const progress: GamdlProgress = {
        download_id: 'dl-1',
        event: { type: 'track_info', title: 'Anti-Hero', artist: 'Taylor Swift', album: 'Midnights' },
      };
      useDownloadStore.getState().handleProgressEvent(progress);

      expect(useDownloadStore.getState().queueItems[0].current_track).toBe('Anti-Hero');
    });

    it('transitions to processing state on processing_step event', () => {
      const item = createMockQueueItem({ id: 'dl-1', state: 'downloading' });
      useDownloadStore.setState({ queueItems: [item] });

      const progress: GamdlProgress = {
        download_id: 'dl-1',
        event: { type: 'processing_step', step: 'Remuxing' },
      };
      useDownloadStore.getState().handleProgressEvent(progress);

      expect(useDownloadStore.getState().queueItems[0].state).toBe('processing');
    });

    it('handles complete event in progress handler', () => {
      const item = createMockQueueItem({ id: 'dl-1', state: 'processing' });
      useDownloadStore.setState({ queueItems: [item] });

      const progress: GamdlProgress = {
        download_id: 'dl-1',
        event: { type: 'complete', path: '/output/song.m4a' },
      };
      useDownloadStore.getState().handleProgressEvent(progress);

      const updated = useDownloadStore.getState().queueItems[0];
      expect(updated.state).toBe('complete');
      expect(updated.progress).toBe(100);
      expect(updated.output_path).toBe('/output/song.m4a');
    });

    it('handles error event in progress handler', () => {
      const item = createMockQueueItem({ id: 'dl-1', state: 'downloading' });
      useDownloadStore.setState({ queueItems: [item] });

      const progress: GamdlProgress = {
        download_id: 'dl-1',
        event: { type: 'error', message: 'Codec not available' },
      };
      useDownloadStore.getState().handleProgressEvent(progress);

      const updated = useDownloadStore.getState().queueItems[0];
      expect(updated.state).toBe('error');
      expect(updated.error).toBe('Codec not available');
    });

    it('ignores events for non-existent queue items', () => {
      const item = createMockQueueItem({ id: 'dl-1' });
      useDownloadStore.setState({ queueItems: [item] });

      const progress: GamdlProgress = {
        download_id: 'non-existent',
        event: { type: 'download_progress', percent: 50, speed: '1 MB/s', eta: '00:30' },
      };
      useDownloadStore.getState().handleProgressEvent(progress);

      /* Original item should be unchanged */
      expect(useDownloadStore.getState().queueItems[0].progress).toBe(0);
    });

    it('only updates the matching item in a multi-item queue', () => {
      const items = [
        createMockQueueItem({ id: 'dl-1', state: 'queued' }),
        createMockQueueItem({ id: 'dl-2', state: 'queued' }),
        createMockQueueItem({ id: 'dl-3', state: 'queued' }),
      ];
      useDownloadStore.setState({ queueItems: items });

      const progress: GamdlProgress = {
        download_id: 'dl-2',
        event: { type: 'download_progress', percent: 75, speed: '3 MB/s', eta: '00:10' },
      };
      useDownloadStore.getState().handleProgressEvent(progress);

      const updated = useDownloadStore.getState().queueItems;
      expect(updated[0].progress).toBe(0);    // dl-1 unchanged
      expect(updated[1].progress).toBe(75);   // dl-2 updated
      expect(updated[2].progress).toBe(0);    // dl-3 unchanged
    });
  });

  // =========================================================================
  // Terminal Event Handlers
  // =========================================================================
  describe('handleDownloadComplete', () => {
    it('marks a download as complete with 100% progress', () => {
      const item = createMockQueueItem({ id: 'dl-1', state: 'processing', progress: 95 });
      useDownloadStore.setState({ queueItems: [item] });

      useDownloadStore.getState().handleDownloadComplete('dl-1');

      const updated = useDownloadStore.getState().queueItems[0];
      expect(updated.state).toBe('complete');
      expect(updated.progress).toBe(100);
    });

    it('does not affect other queue items', () => {
      const items = [
        createMockQueueItem({ id: 'dl-1', state: 'downloading', progress: 50 }),
        createMockQueueItem({ id: 'dl-2', state: 'downloading', progress: 30 }),
      ];
      useDownloadStore.setState({ queueItems: items });

      useDownloadStore.getState().handleDownloadComplete('dl-1');

      expect(useDownloadStore.getState().queueItems[1].state).toBe('downloading');
      expect(useDownloadStore.getState().queueItems[1].progress).toBe(30);
    });
  });

  describe('handleDownloadError', () => {
    it('marks a download as error with the error message', () => {
      const item = createMockQueueItem({ id: 'dl-1', state: 'downloading' });
      useDownloadStore.setState({ queueItems: [item] });

      useDownloadStore.getState().handleDownloadError('dl-1', 'Authentication failed');

      const updated = useDownloadStore.getState().queueItems[0];
      expect(updated.state).toBe('error');
      expect(updated.error).toBe('Authentication failed');
    });
  });

  describe('handleDownloadCancelled', () => {
    it('marks a download as cancelled', () => {
      const item = createMockQueueItem({ id: 'dl-1', state: 'downloading' });
      useDownloadStore.setState({ queueItems: [item] });

      useDownloadStore.getState().handleDownloadCancelled('dl-1');

      expect(useDownloadStore.getState().queueItems[0].state).toBe('cancelled');
    });
  });

  // =========================================================================
  // Queue Management
  // =========================================================================
  describe('cancelDownload', () => {
    it('calls the cancel command', async () => {
      vi.mocked(commands.cancelDownload).mockResolvedValueOnce(undefined);

      await useDownloadStore.getState().cancelDownload('dl-1');

      expect(commands.cancelDownload).toHaveBeenCalledWith('dl-1');
    });

    it('sets error on failure', async () => {
      vi.mocked(commands.cancelDownload).mockRejectedValueOnce('Cancel failed');

      await useDownloadStore.getState().cancelDownload('dl-1');

      expect(useDownloadStore.getState().error).toBe('Cancel failed');
    });
  });

  describe('retryDownload', () => {
    it('retries and refreshes the queue', async () => {
      const refreshedItems = [createMockQueueItem({ id: 'dl-1', state: 'queued' })];
      vi.mocked(commands.retryDownload).mockResolvedValueOnce(undefined);
      vi.mocked(commands.getQueueStatus).mockResolvedValueOnce({
        total: 1, active: 0, queued: 1, completed: 0, failed: 0,
        items: refreshedItems,
      });

      await useDownloadStore.getState().retryDownload('dl-1');

      expect(commands.retryDownload).toHaveBeenCalledWith('dl-1');
      expect(useDownloadStore.getState().queueItems).toEqual(refreshedItems);
    });
  });

  describe('clearFinished', () => {
    it('clears finished items and returns the count', async () => {
      vi.mocked(commands.clearQueue).mockResolvedValueOnce(3);
      vi.mocked(commands.getQueueStatus).mockResolvedValueOnce({
        total: 1, active: 1, queued: 0, completed: 0, failed: 0,
        items: [createMockQueueItem({ id: 'dl-active', state: 'downloading' })],
      });

      const removed = await useDownloadStore.getState().clearFinished();

      expect(removed).toBe(3);
      expect(useDownloadStore.getState().queueItems).toHaveLength(1);
      expect(useDownloadStore.getState().queueItems[0].id).toBe('dl-active');
    });

    it('returns 0 on failure', async () => {
      vi.mocked(commands.clearQueue).mockRejectedValueOnce('Clear failed');

      const removed = await useDownloadStore.getState().clearFinished();

      expect(removed).toBe(0);
    });
  });

  describe('refreshQueue', () => {
    it('fetches queue status from the backend', async () => {
      const items = [
        createMockQueueItem({ id: 'dl-1', state: 'complete', progress: 100 }),
        createMockQueueItem({ id: 'dl-2', state: 'downloading', progress: 50 }),
      ];
      vi.mocked(commands.getQueueStatus).mockResolvedValueOnce({
        total: 2, active: 1, queued: 0, completed: 1, failed: 0,
        items,
      });

      await useDownloadStore.getState().refreshQueue();

      expect(useDownloadStore.getState().queueItems).toEqual(items);
    });

    it('sets error on failure', async () => {
      vi.mocked(commands.getQueueStatus).mockRejectedValueOnce('Fetch failed');

      await useDownloadStore.getState().refreshQueue();

      expect(useDownloadStore.getState().error).toBe('Fetch failed');
    });
  });
});
