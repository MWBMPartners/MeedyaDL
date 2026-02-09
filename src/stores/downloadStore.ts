/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Download state store.
 * Manages the download queue, URL input state, and per-download
 * quality overrides. Listens for Tauri events from the backend
 * to update progress in real-time.
 */

import { create } from 'zustand';
import type { GamdlOptions, GamdlProgress, QueueItemStatus } from '@/types';
import { parseAppleMusicUrl } from '@/lib/url-parser';
import * as commands from '@/lib/tauri-commands';

interface DownloadState {
  /** Current URL input value */
  urlInput: string;
  /** Whether the current URL input is a valid Apple Music URL */
  urlIsValid: boolean;
  /** Detected content type of the current URL */
  urlContentType: string;
  /** Per-download quality overrides (null = use global settings) */
  overrideOptions: GamdlOptions | null;
  /** Download queue items with their current status */
  queueItems: QueueItemStatus[];
  /** Whether a download submission is in progress */
  isSubmitting: boolean;
  /** Error from the last operation */
  error: string | null;

  /** Update the URL input and validate it */
  setUrlInput: (url: string) => void;
  /** Set per-download quality overrides */
  setOverrideOptions: (options: GamdlOptions | null) => void;
  /** Submit the current URL for download */
  submitDownload: () => Promise<string>;
  /** Refresh the queue status from the backend */
  refreshQueue: () => Promise<void>;
  /** Handle a GAMDL progress event from the backend */
  handleProgressEvent: (progress: GamdlProgress) => void;
  /** Clear the URL input */
  clearInput: () => void;
}

export const useDownloadStore = create<DownloadState>((set, get) => ({
  urlInput: '',
  urlIsValid: false,
  urlContentType: 'unknown',
  overrideOptions: null,
  queueItems: [],
  isSubmitting: false,
  error: null,

  setUrlInput: (url) => {
    const parsed = parseAppleMusicUrl(url);
    set({
      urlInput: url,
      urlIsValid: parsed.isValid,
      urlContentType: parsed.contentType,
    });
  },

  setOverrideOptions: (options) => set({ overrideOptions: options }),

  submitDownload: async () => {
    const { urlInput, urlIsValid, overrideOptions } = get();

    if (!urlIsValid) {
      set({ error: 'Invalid Apple Music URL' });
      throw new Error('Invalid Apple Music URL');
    }

    set({ isSubmitting: true, error: null });
    try {
      const downloadId = await commands.startDownload({
        urls: [urlInput],
        options: overrideOptions ?? undefined,
      });

      // Clear the input after successful submission
      set({
        urlInput: '',
        urlIsValid: false,
        urlContentType: 'unknown',
        overrideOptions: null,
        isSubmitting: false,
      });

      return downloadId;
    } catch (e) {
      const msg = String(e);
      set({ error: msg, isSubmitting: false });
      throw new Error(msg);
    }
  },

  refreshQueue: async () => {
    try {
      const status = await commands.getQueueStatus();
      set({ queueItems: status.items });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  handleProgressEvent: (progress) => {
    set((state) => {
      const items = [...state.queueItems];
      const idx = items.findIndex((i) => i.id === progress.download_id);

      if (idx >= 0) {
        const item = { ...items[idx] };

        // Update the item based on the event type
        switch (progress.event.type) {
          case 'download_progress':
            item.progress = progress.event.percent;
            item.speed = progress.event.speed || null;
            item.eta = progress.event.eta || null;
            item.state = 'downloading';
            break;
          case 'track_info':
            item.current_track = progress.event.title || null;
            break;
          case 'processing_step':
            item.state = 'processing';
            break;
          case 'complete':
            item.state = 'complete';
            item.progress = 100;
            item.output_path = progress.event.path || null;
            break;
          case 'error':
            item.state = 'error';
            item.error = progress.event.message || null;
            break;
        }

        items[idx] = item;
      }

      return { queueItems: items };
    });
  },

  clearInput: () =>
    set({
      urlInput: '',
      urlIsValid: false,
      urlContentType: 'unknown',
      overrideOptions: null,
    }),
}));
