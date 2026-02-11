// Copyright (c) 2024-2026 MWBM Partners Ltd
/**
 * @file Barrel export for download-related components.
 *
 * Re-exports all download UI components from a single entry point:
 *
 *   import { DownloadForm, DownloadQueue, QueueItem } from '@/components/download';
 *
 * This barrel pattern keeps import statements concise and decouples
 * consumers from the internal file structure. If a component is renamed
 * or split into a new file, only this barrel needs to change.
 *
 * Exported components and their roles:
 *  - {@link DownloadForm}  - URL input page with validation, content-type
 *    detection badge, and per-download quality overrides. Connected to
 *    downloadStore (URL state + submission) and settingsStore (default codec).
 *  - {@link DownloadQueue} - Queue list page showing all download items
 *    with real-time progress, status icons, and cancel/retry/clear actions.
 *    Connected to downloadStore (queue state + operations) and uiStore (toasts).
 *  - {@link QueueItem}     - Individual row within the queue list displaying
 *    a single download's URL, status icon, progress bar, speed/ETA, fallback
 *    indicator, and context-sensitive action buttons.
 *
 * @see https://www.typescriptlang.org/docs/handbook/modules.html#re-exports
 *      TypeScript re-export documentation.
 */

/** URL input page with Apple Music URL validation and quality overrides. */
export { DownloadForm } from './DownloadForm';

/** Queue list page displaying all download items with progress and actions. */
export { DownloadQueue } from './DownloadQueue';

/** Single queue item row with status, progress bar, and action buttons. */
export { QueueItem } from './QueueItem';
