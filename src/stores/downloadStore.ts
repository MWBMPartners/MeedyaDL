// Copyright (c) 2024-2026 MeedyaDL
/**
 * @file downloadStore.ts -- Download Queue State Management Store
 * @license MIT -- See LICENSE file in the project root.
 *
 * Manages the entire download lifecycle:
 *
 *   **URL input & validation**:
 *   - `setUrlInput()` validates the URL in real-time via `parseAppleMusicUrl()`.
 *   - The `<DownloadPage>` reads `urlIsValid` and `urlContentType` to show
 *     validation feedback and a content-type badge (song, album, playlist, etc.).
 *
 *   **Download submission**:
 *   - `submitDownload()` sends the validated URL (plus optional quality overrides)
 *     to the Rust backend via the `start_download` Tauri command.
 *   - The Rust `download_queue.rs` service enqueues the job and returns a unique
 *     download ID. The frontend clears the input and resets overrides on success.
 *
 *   **Real-time progress updates** (Tauri event system):
 *   - The Rust backend emits events as the GAMDL subprocess produces output:
 *       - `gamdl://progress`          -> `handleProgressEvent()`
 *       - `gamdl://download-complete` -> `handleDownloadComplete()`
 *       - `gamdl://download-error`    -> `handleDownloadError()`
 *       - `gamdl://download-cancelled`-> `handleDownloadCancelled()`
 *   - These events are subscribed to in the `<App>` component (or a dedicated
 *     `useTauriEventListeners` hook) using `@tauri-apps/api/event.listen()`.
 *   - The event handler callbacks call the corresponding store actions, which
 *     perform immutable updates on the `queueItems` array.
 *
 *   **Queue management**:
 *   - `cancelDownload()`, `retryDownload()`, `clearFinished()` delegate to Rust
 *     commands and then refresh the queue snapshot.
 *   - `refreshQueue()` fetches the full queue state from the backend, used on
 *     app startup and after mutations to ensure consistency.
 *
 * Consumed by: `<DownloadPage>`, `<QueuePage>`, `<QueueItem>`, `<App>`.
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state} -- Zustand state updates
 * @see {@link https://v2.tauri.app/develop/calling-frontend/} -- Tauri events from Rust to JS
 * @see {@link https://v2.tauri.app/develop/calling-rust/} -- Tauri `invoke()` IPC bridge
 */

// Zustand store factory. Returns a React hook with automatic re-render on state change.
import { create } from 'zustand';

// GamdlOptions    -- per-download quality/format overrides (all fields optional)
// GamdlProgress   -- payload shape for `gamdl://progress` Tauri events
// QueueItemStatus -- detailed status of a single download queue item
import type { GamdlOptions, GamdlProgress, QueueItemStatus } from '@/types';

// Pure function that validates and classifies an Apple Music URL (song, album, etc.).
// Returns { url, isValid, contentType }.
import { parseAppleMusicUrl } from '@/lib/url-parser';

// Type-safe wrappers for Tauri IPC commands. Each function maps to a
// `#[tauri::command]` handler in the Rust backend.
import * as commands from '@/lib/tauri-commands';

/**
 * Combined state + actions interface for the download store.
 *
 * State is divided into two logical groups:
 *   1. **URL input state** -- controlled input for the URL text field plus
 *      validation results, consumed by `<DownloadPage>`.
 *   2. **Queue state** -- the list of download items with their real-time status,
 *      consumed by `<QueuePage>` and `<QueueItem>`.
 *
 * Actions fall into three categories:
 *   - **User-initiated**: `setUrlInput`, `submitDownload`, `cancelDownload`, etc.
 *   - **Event handlers**: `handleProgressEvent`, `handleDownloadComplete`, etc.
 *     These are called by Tauri event listeners, NOT directly by components.
 *   - **Refresh**: `refreshQueue` re-fetches the full queue snapshot from Rust.
 */
interface DownloadState {
  // ---------------------------------------------------------------------------
  // URL input state (controlled by <DownloadPage>)
  // ---------------------------------------------------------------------------

  /**
   * The raw text content of the URL input field.
   * Updated on every keystroke via `setUrlInput()`.
   */
  urlInput: string;

  /**
   * Whether `urlInput` is a structurally valid Apple Music URL.
   * Derived synchronously by `parseAppleMusicUrl()` inside `setUrlInput()`.
   * The submit button is disabled when this is `false`.
   */
  urlIsValid: boolean;

  /**
   * The detected Apple Music content type ('song', 'album', 'playlist',
   * 'music-video', 'artist', or 'unknown'). Shown as a badge next to the
   * URL input for user feedback.
   */
  urlContentType: string;

  /**
   * Optional per-download quality/format overrides. When `null`, the download
   * uses the global defaults from `settingsStore`. When set, these values
   * override the corresponding global settings for this single download only.
   * Managed by the quality-override panel in `<DownloadPage>`.
   */
  overrideOptions: GamdlOptions | null;

  // ---------------------------------------------------------------------------
  // Queue state (mirrors the Rust download_queue.rs service state)
  // ---------------------------------------------------------------------------

  /**
   * Snapshot of all download queue items with their current status.
   * Each item tracks: id, URLs, state, progress %, current track name,
   * speed, ETA, error message, output path, etc.
   *
   * This array is updated in three ways:
   *   1. Full refresh via `refreshQueue()` (fetches from Rust backend)
   *   2. Incremental updates via `handleProgressEvent()` (Tauri events)
   *   3. Terminal-state updates via `handleDownloadComplete/Error/Cancelled()`
   */
  queueItems: QueueItemStatus[];

  /**
   * `true` while `submitDownload()` is awaiting the Rust `start_download`
   * response. The UI disables the submit button during this period.
   */
  isSubmitting: boolean;

  /**
   * Human-readable error message from the last failed operation.
   * `null` when there is no error. Displayed by the `<DownloadPage>` error banner.
   */
  error: string | null;

  // ---------------------------------------------------------------------------
  // User-initiated actions
  // ---------------------------------------------------------------------------

  /**
   * Update the URL input and synchronously validate it.
   * Called on every keystroke in the URL text field.
   * Internally calls `parseAppleMusicUrl(url)` which returns `{ isValid, contentType }`.
   * @param url -- The raw URL string from the input field
   */
  setUrlInput: (url: string) => void;

  /**
   * Set or clear per-download quality overrides.
   * Pass `null` to revert to global settings.
   * @param options -- A partial `GamdlOptions` object or `null`
   */
  setOverrideOptions: (options: GamdlOptions | null) => void;

  /**
   * Submit the current URL for download to the Rust backend.
   * Validates the URL, calls `commands.startDownload()`, clears the input on
   * success, and returns the new download ID.
   * @returns The unique download ID assigned by the backend
   * @throws If the URL is invalid or the Rust command fails
   */
  submitDownload: () => Promise<string>;

  /**
   * Cancel an active or queued download by its ID.
   * IPC call: `commands.cancelDownload(downloadId)` -> Rust `cancel_download`
   * The backend sends a cancellation signal to the GAMDL subprocess.
   * @param downloadId -- The unique ID of the download to cancel
   */
  cancelDownload: (downloadId: string) => Promise<void>;

  /**
   * Retry a failed or cancelled download. The backend resets the item to
   * 'queued' state with fresh options and re-enqueues it.
   * IPC call: `commands.retryDownload(downloadId)` -> Rust `retry_download`
   * After retrying, the queue is refreshed to reflect the new state.
   * @param downloadId -- The unique ID of the download to retry
   */
  retryDownload: (downloadId: string) => Promise<void>;

  /**
   * Remove all completed, failed, and cancelled items from the queue.
   * IPC call: `commands.clearQueue()` -> Rust `clear_queue`
   * Returns the number of items removed. Refreshes the queue afterward.
   * @returns The count of items that were cleared
   */
  clearFinished: () => Promise<number>;

  /**
   * Fetch the full queue snapshot from the Rust backend.
   * IPC call: `commands.getQueueStatus()` -> Rust `get_queue_status`
   * Called on app startup, after mutations, and periodically for consistency.
   */
  refreshQueue: () => Promise<void>;

  /**
   * Export the current queue to a `.meedyadl` file via a native save dialog.
   * IPC call: `commands.exportQueue()` -> Rust `export_queue`
   * Only non-terminal items (queued/active) are exported.
   * @returns The count of items exported
   */
  exportQueue: () => Promise<number>;

  /**
   * Import queue items from a `.meedyadl` file via a native file picker.
   * IPC call: `commands.importQueue()` -> Rust `import_queue`
   * Imported items are enqueued and processing starts automatically.
   * @returns The count of items imported
   */
  importQueue: () => Promise<number>;

  // ---------------------------------------------------------------------------
  // Tauri event handlers (called by event listeners, NOT by components directly)
  // ---------------------------------------------------------------------------

  /**
   * Handle a structured progress event emitted by the Rust backend.
   * The Rust `download_queue.rs` service parses GAMDL subprocess stdout/stderr
   * and emits `gamdl://progress` events with a `GamdlProgress` payload.
   *
   * This handler performs an immutable update on the matching queue item based
   * on the event type: `download_progress`, `track_info`, `processing_step`,
   * `complete`, or `error`.
   *
   * @param progress -- The structured progress event payload
   * @see {@link https://v2.tauri.app/develop/calling-frontend/} -- Tauri event system
   */
  handleProgressEvent: (progress: GamdlProgress) => void;

  /**
   * Handle a download-complete event. Sets the item's state to 'complete'
   * and progress to 100%. Called when the backend emits `gamdl://download-complete`.
   * @param downloadId -- The ID of the completed download
   */
  handleDownloadComplete: (downloadId: string) => void;

  /**
   * Handle a download-error event. Sets the item's state to 'error' and
   * stores the error message. Called when `gamdl://download-error` is emitted.
   * @param downloadId -- The ID of the failed download
   * @param error -- Human-readable error description from the backend
   */
  handleDownloadError: (downloadId: string, error: string) => void;

  /**
   * Handle a download-cancelled event. Sets the item's state to 'cancelled'.
   * Called when `gamdl://download-cancelled` is emitted after a user cancellation.
   * @param downloadId -- The ID of the cancelled download
   */
  handleDownloadCancelled: (downloadId: string) => void;

  /**
   * Reset the URL input field and associated state to their initial values.
   * Called after successful submission or when the user clicks "Clear".
   */
  clearInput: () => void;
}

/**
 * Zustand store hook for download queue state and URL input management.
 *
 * Usage in components:
 *   const queueItems = useDownloadStore((s) => s.queueItems);
 *   const { submitDownload, cancelDownload } = useDownloadStore();
 *
 * The store creator receives `set` (for immutable state updates) and `get`
 * (for reading current state inside async actions without stale closures).
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state}
 */
export const useDownloadStore = create<DownloadState>((set, get) => ({
  // -------------------------------------------------------------------------
  // Initial state -- empty input, no queue items, no errors
  // -------------------------------------------------------------------------
  urlInput: '',             // URL input field starts empty
  urlIsValid: false,        // No valid URL until user types one
  urlContentType: 'unknown',// Content type unknown until URL is parsed
  overrideOptions: null,    // No per-download overrides; use global settings
  queueItems: [],           // Empty queue until refreshQueue() or events arrive
  isSubmitting: false,      // No submission in progress
  error: null,              // No error

  // -------------------------------------------------------------------------
  // URL input actions
  // -------------------------------------------------------------------------

  /**
   * Update the URL input text and validate it synchronously.
   * `parseAppleMusicUrl()` uses regex matching to detect valid Apple Music
   * URL patterns and classify the content type (song, album, playlist, etc.).
   * All three fields are updated atomically in a single `set()` call.
   */
  setUrlInput: (url) => {
    const parsed = parseAppleMusicUrl(url);
    set({
      urlInput: url,
      urlIsValid: parsed.isValid,       // true if URL matches Apple Music patterns
      urlContentType: parsed.contentType, // 'song', 'album', 'playlist', etc.
    });
  },

  /** Replace the per-download quality overrides. Pass `null` to clear. */
  setOverrideOptions: (options) => set({ overrideOptions: options }),

  // -------------------------------------------------------------------------
  // Download submission
  // -------------------------------------------------------------------------

  /**
   * Submit the current URL for download to the Rust backend.
   *
   * Flow:
   *   1. Guard: reject if URL is invalid (early return with error).
   *   2. Set `isSubmitting = true` to disable the UI submit button.
   *   3. Call `commands.startDownload()` -> Rust `start_download` command.
   *      The request contains the URL array and optional quality overrides.
   *      `overrideOptions ?? undefined` converts null to undefined so the
   *      Rust handler receives `None` for the options field.
   *   4. On success: clear the URL input and overrides, return the download ID.
   *   5. On failure: store the error and re-throw for the component to handle.
   */
  submitDownload: async () => {
    // Read the latest state at call time via `get()` to avoid stale closures.
    const { urlInput, urlIsValid, overrideOptions } = get();

    // Guard: do not proceed if the URL has not passed validation.
    if (!urlIsValid) {
      set({ error: 'Invalid Apple Music URL' });
      throw new Error('Invalid Apple Music URL');
    }

    // Signal submission in progress and clear any stale errors.
    set({ isSubmitting: true, error: null });
    try {
      // IPC call: send the download request to the Rust download_queue service.
      // The backend enqueues the job and immediately returns a unique download ID.
      const downloadId = await commands.startDownload({
        urls: [urlInput],
        // Convert null -> undefined so Tauri serializes as Option::None in Rust.
        options: overrideOptions ?? undefined,
      });

      // Clear the input form after successful submission, readying it for
      // the next URL. All input-related fields are reset atomically.
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

  // -------------------------------------------------------------------------
  // Queue management actions (delegate to Rust then refresh)
  // -------------------------------------------------------------------------

  /**
   * Cancel an active or queued download.
   * IPC call: `commands.cancelDownload(downloadId)` -> Rust `cancel_download`
   * The actual state transition happens via the `gamdl://download-cancelled`
   * event that the backend emits after cancellation succeeds.
   */
  cancelDownload: async (downloadId) => {
    try {
      await commands.cancelDownload(downloadId);
    } catch (e) {
      set({ error: String(e) });
    }
  },

  /**
   * Retry a failed or cancelled download.
   * IPC call: `commands.retryDownload(downloadId)` -> Rust `retry_download`
   * After the retry command succeeds, we explicitly refresh the full queue
   * to pick up the item's reset state (back to 'queued').
   */
  retryDownload: async (downloadId) => {
    try {
      await commands.retryDownload(downloadId);
      // Fetch the updated queue snapshot to reflect the retried item's new state.
      const status = await commands.getQueueStatus();
      set({ queueItems: status.items });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  /**
   * Clear all terminal-state items (completed, failed, cancelled) from the queue.
   * IPC call: `commands.clearQueue()` -> Rust `clear_queue`
   * Returns the number of items that were removed.
   * Refreshes the queue afterward to ensure frontend and backend are in sync.
   */
  clearFinished: async () => {
    try {
      const removed = await commands.clearQueue();
      // Refresh the queue to remove cleared items from the UI.
      const status = await commands.getQueueStatus();
      set({ queueItems: status.items });
      return removed;
    } catch (e) {
      set({ error: String(e) });
      return 0; // Return 0 on failure -- no items were cleared
    }
  },

  /**
   * Fetch the full queue snapshot from the Rust backend.
   * IPC call: `commands.getQueueStatus()` -> Rust `get_queue_status`
   * This replaces the entire `queueItems` array, which is useful on startup
   * (to restore in-progress downloads) and after mutations.
   */
  refreshQueue: async () => {
    try {
      const status = await commands.getQueueStatus();
      set({ queueItems: status.items });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  /**
   * Export the current queue to a `.meedyadl` file.
   * IPC call: `commands.exportQueue()` -> Rust `export_queue`
   * Opens a native save dialog. Returns the count of exported items.
   */
  exportQueue: async () => {
    try {
      const count = await commands.exportQueue();
      return count;
    } catch (e) {
      set({ error: String(e) });
      return 0;
    }
  },

  /**
   * Import queue items from a `.meedyadl` file.
   * IPC call: `commands.importQueue()` -> Rust `import_queue`
   * Opens a native file picker. Refreshes the queue after import.
   */
  importQueue: async () => {
    try {
      const count = await commands.importQueue();
      // Refresh the queue to include the newly imported items
      const status = await commands.getQueueStatus();
      set({ queueItems: status.items });
      return count;
    } catch (e) {
      set({ error: String(e) });
      return 0;
    }
  },

  // -------------------------------------------------------------------------
  // Tauri event handlers -- called by event listeners set up in <App>
  // These perform immutable updates on individual queue items.
  // -------------------------------------------------------------------------

  /**
   * Handle a structured `gamdl://progress` event from the Rust backend.
   *
   * The Rust `download_queue.rs` service parses GAMDL subprocess output lines
   * into structured `GamdlOutputEvent` variants and emits them as Tauri events.
   * This handler finds the matching queue item by `download_id` and updates
   * its fields based on the event type.
   *
   * Immutable update pattern:
   *   1. Shallow-copy the `queueItems` array.
   *   2. Find the target item by ID.
   *   3. Shallow-copy the target item.
   *   4. Mutate the copy based on the event type.
   *   5. Replace the item in the array copy.
   *   6. Return the new array as the next state.
   *
   * @see {@link https://v2.tauri.app/develop/calling-frontend/} -- Tauri events
   */
  handleProgressEvent: (progress) => {
    set((state) => {
      // Step 1: Shallow-copy the array to avoid mutating existing state.
      const items = [...state.queueItems];
      // Step 2: Find the queue item matching this event's download ID.
      const idx = items.findIndex((i) => i.id === progress.download_id);

      if (idx >= 0) {
        // Step 3: Shallow-copy the target item for immutable update.
        const item = { ...items[idx] };

        // Step 4: Update fields based on the event discriminator.
        switch (progress.event.type) {
          case 'download_progress':
            // Update progress percentage, download speed, and ETA.
            item.progress = progress.event.percent;
            item.speed = progress.event.speed || null;
            item.eta = progress.event.eta || null;
            item.state = 'downloading'; // Transition to 'downloading' state
            break;
          case 'track_info':
            // Update the currently-downloading track name for display.
            item.current_track = progress.event.title || null;
            break;
          case 'processing_step':
            // Transition to 'processing' (post-download remuxing/tagging).
            item.state = 'processing';
            break;
          case 'complete':
            // Mark as complete with 100% progress and record output path.
            item.state = 'complete';
            item.progress = 100;
            item.output_path = progress.event.path || null;
            break;
          case 'error':
            // Mark as error and store the error message for display.
            item.state = 'error';
            item.error = progress.event.message || null;
            break;
        }

        // Step 5: Replace the old item with the updated copy.
        items[idx] = item;
      }
      // If no matching item found (idx < 0), return unchanged array.
      // This can happen if the event arrives after the item was cleared.

      // Step 6: Return the new array as the next state.
      return { queueItems: items };
    });
  },

  /**
   * Handle a `gamdl://download-complete` terminal event.
   * Uses `map()` to produce a new array with the target item updated.
   * `as const` assertion ensures TypeScript narrows the string literal type.
   */
  handleDownloadComplete: (downloadId) => {
    set((state) => {
      const items = state.queueItems.map((item) =>
        item.id === downloadId
          ? { ...item, state: 'complete' as const, progress: 100 }
          : item, // Leave non-matching items unchanged
      );
      return { queueItems: items };
    });
  },

  /**
   * Handle a `gamdl://download-error` terminal event.
   * Stores the error message alongside the state transition.
   */
  handleDownloadError: (downloadId, error) => {
    set((state) => {
      const items = state.queueItems.map((item) =>
        item.id === downloadId
          ? { ...item, state: 'error' as const, error }
          : item, // Leave non-matching items unchanged
      );
      return { queueItems: items };
    });
  },

  /**
   * Handle a `gamdl://download-cancelled` terminal event.
   * Transitions the item to 'cancelled' state.
   */
  handleDownloadCancelled: (downloadId) => {
    set((state) => {
      const items = state.queueItems.map((item) =>
        item.id === downloadId
          ? { ...item, state: 'cancelled' as const }
          : item, // Leave non-matching items unchanged
      );
      return { queueItems: items };
    });
  },

  // -------------------------------------------------------------------------
  // Input reset
  // -------------------------------------------------------------------------

  /**
   * Reset the URL input field and all related state to initial values.
   * Called after successful submission or when the user clicks a "Clear" button.
   */
  clearInput: () =>
    set({
      urlInput: '',
      urlIsValid: false,
      urlContentType: 'unknown',
      overrideOptions: null,
    }),
}));
