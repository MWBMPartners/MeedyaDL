// Copyright (c) 2024-2026 MWBM Partners Ltd
/**
 * @file Individual download queue item component.
 *
 * Displays the status, progress, and controls for a single download.
 * Rendered as a row within the {@link DownloadQueue} list. Each row shows:
 *
 *  - **Status icon** -- coloured Lucide icon reflecting the current state
 *    (queued, downloading, processing, complete, error, cancelled).
 *  - **URL** -- the Apple Music URL being downloaded (truncated with ellipsis).
 *  - **Current track** -- the track name currently being processed (if known).
 *  - **Fallback indicator** -- warning badge shown when the backend fell back
 *    to an alternative codec because the preferred codec was unavailable.
 *  - **Progress bar** -- horizontal bar shown for active downloads, with
 *    percentage driven by `item.progress` (0-100).
 *  - **Speed / ETA** -- transfer speed and estimated time remaining.
 *  - **Action buttons** -- context-sensitive: Open Folder (complete),
 *    Cancel (active/queued), Retry (error/cancelled).
 *
 * ## Fallback chain indicator
 *
 * When GAMDL cannot obtain the preferred codec (e.g., ALAC is unavailable
 * for a particular track), it falls back through the configured fallback
 * chain (`settings.music_fallback_chain`). If a fallback occurs, the
 * backend sets `fallback_occurred: true` and `codec_used` on the queue
 * item. This component displays a yellow warning message showing the
 * codec that was actually used.
 *
 * ## Props
 *
 * This component receives its data and callbacks as props from the
 * parent {@link DownloadQueue} component. It does **not** access the
 * Zustand stores directly -- this keeps it a presentational component
 * with explicit data flow via props.
 *
 * @see https://react.dev/learn/passing-props-to-a-component
 *      React docs -- passing props to components.
 * @see https://lucide.dev/icons/  -- all icons used in state mapping.
 * @see https://tailwindcss.com/docs/animation#spin  -- spinner animation.
 */

/**
 * Lucide React icons mapped to download states and action buttons.
 *
 * State icons:
 *  - `Clock`         -> queued        (@see https://lucide.dev/icons/clock)
 *  - `Download`      -> downloading   (@see https://lucide.dev/icons/download)
 *  - `Loader2`       -> processing    (@see https://lucide.dev/icons/loader-2)
 *  - `CheckCircle`   -> complete      (@see https://lucide.dev/icons/check-circle)
 *  - `XCircle`       -> error/cancel  (@see https://lucide.dev/icons/x-circle)
 *
 * Action icons:
 *  - `X`             -> cancel button (@see https://lucide.dev/icons/x)
 *  - `RotateCcw`     -> retry button  (@see https://lucide.dev/icons/rotate-ccw)
 *  - `FolderOpen`    -> open folder   (@see https://lucide.dev/icons/folder-open)
 *  - `AlertTriangle` -> fallback warn (@see https://lucide.dev/icons/alert-triangle)
 */
import {
  Clock,
  Download,
  CheckCircle,
  XCircle,
  Loader2,
  X,
  RotateCcw,
  FolderOpen,
  AlertTriangle,
} from 'lucide-react';

/**
 * ProgressBar: a horizontal bar component that visualises a 0-100
 * percentage value. Accepts `null` to display an indeterminate state.
 * @see ProgressBar in @/components/common
 */
import { ProgressBar } from '@/components/common';

/**
 * Type imports for queue item data and download state.
 * @see QueueItemStatus in @/types/index.ts  -- full shape of a queue item.
 * @see DownloadState in @/types/index.ts    -- 'queued' | 'downloading' | ... union.
 */
import type { QueueItemStatus, DownloadState } from '@/types';

/**
 * Props for the {@link QueueItem} component.
 *
 * This component is a **presentational** (or "dumb") component: it receives
 * all data and callbacks via props and does not access Zustand stores
 * directly. This makes it easy to test and reason about.
 *
 * @see https://react.dev/learn/passing-props-to-a-component
 */
interface QueueItemProps {
  /**
   * The queue item data object containing state, progress, URLs,
   * track info, speed, ETA, error message, output path, etc.
   * @see QueueItemStatus in @/types/index.ts
   */
  item: QueueItemStatus;

  /**
   * Callback invoked when the user clicks the "Cancel" button.
   * Receives the unique download ID. The parent (DownloadQueue)
   * uses this to call `downloadStore.cancelDownload(id)`.
   */
  onCancel: (id: string) => void;

  /**
   * Callback invoked when the user clicks the "Retry" button on
   * a failed or cancelled download. Receives the download ID.
   * The parent uses this to call `downloadStore.retryDownload(id)`.
   */
  onRetry: (id: string) => void;
}

/**
 * Static configuration mapping each {@link DownloadState} to its
 * visual representation: a Lucide icon component, a Tailwind CSS
 * colour class, and a human-readable label.
 *
 * Colour semantics follow the app's design-token system:
 *  - `text-content-tertiary` -- neutral / inactive (queued, cancelled)
 *  - `text-status-info`      -- blue / active (downloading)
 *  - `text-status-warning`   -- yellow / processing
 *  - `text-status-success`   -- green / complete
 *  - `text-status-error`     -- red / error
 *
 * The `Loader2` icon is used for 'processing' because it supports
 * the `animate-spin` class for a spinner effect.
 *
 * @see DownloadState in @/types/index.ts
 * @see https://tailwindcss.com/docs/animation#spin  -- animate-spin
 */
const STATE_CONFIG: Record<
  DownloadState,
  { icon: typeof Clock; colorClass: string; label: string }
> = {
  queued: { icon: Clock, colorClass: 'text-content-tertiary', label: 'Queued' },
  downloading: {
    icon: Download,
    colorClass: 'text-status-info',
    label: 'Downloading',
  },
  processing: {
    icon: Loader2,
    colorClass: 'text-status-warning',
    label: 'Processing',
  },
  complete: {
    icon: CheckCircle,
    colorClass: 'text-status-success',
    label: 'Complete',
  },
  error: { icon: XCircle, colorClass: 'text-status-error', label: 'Error' },
  cancelled: {
    icon: XCircle,
    colorClass: 'text-content-tertiary',
    label: 'Cancelled',
  },
};

/**
 * Renders a single item in the download queue with status icon,
 * progress tracking, fallback indicator, and context-sensitive
 * action buttons.
 *
 * Visual layout:
 * ```
 * ┌────────────────────────────────────────────────────────────┐
 * │ [Icon]  https://music.apple.com/...         [Cancel/Retry] │
 * │         Current Track Name                                 │
 * │         ⚠ Fallback used (codec: aac)                      │
 * │         ████████████░░░░░░░░░░░░░░░░ 45%                  │
 * │         1.2 MB/s   ETA: 2:30                              │
 * │         Error: some error message (if error state)         │
 * └────────────────────────────────────────────────────────────┘
 * ```
 *
 * @param item     - The queue item data (status, progress, URLs, etc.).
 * @param onCancel - Callback to cancel an active/queued download.
 * @param onRetry  - Callback to retry a failed/cancelled download.
 *
 * @see https://react.dev/learn/conditional-rendering
 *      React docs -- conditional rendering of sections.
 * @see https://tailwindcss.com/docs/animation#spin
 *      Tailwind animate-spin for the processing spinner.
 */
export function QueueItem({ item, onCancel, onRetry }: QueueItemProps) {
  /**
   * Look up the visual configuration (icon, colour, label) for the
   * current download state from the static `STATE_CONFIG` record.
   */
  const config = STATE_CONFIG[item.state];

  /** The Lucide icon component for the current state (e.g., Clock, Download). */
  const StateIcon = config.icon;

  /**
   * Whether this item is currently in an "active" state (downloading or
   * processing). Used to conditionally render the progress bar and to
   * determine which action buttons are available.
   */
  const isActive = item.state === 'downloading' || item.state === 'processing';

  /**
   * Opens the output folder in the native file manager (Finder on macOS,
   * Explorer on Windows, or the default file manager on Linux).
   *
   * Uses a dynamic import of `@tauri-apps/plugin-shell` to:
   *  1. Avoid hard failures when running outside the Tauri shell.
   *  2. Keep the shell plugin tree-shaken when not needed.
   *
   * The `open()` function from the shell plugin opens a path in the
   * OS default handler. We extract the parent directory of the output
   * file by stripping the last path segment (substring up to the last '/').
   *
   * @see https://v2.tauri.app/plugin/shell/#open
   */
  const handleOpenFolder = async () => {
    if (!item.output_path) return;
    try {
      const { open } = await import('@tauri-apps/plugin-shell');
      /*
       * Extract the parent directory path from the full file path.
       * Example: '/Users/me/Music/Artist/Album/01 Track.m4a'
       *       -> '/Users/me/Music/Artist/Album'
       */
      const parentDir = item.output_path.substring(
        0,
        item.output_path.lastIndexOf('/'),
      );
      await open(parentDir);
    } catch {
      /* Shell API unavailable (running outside Tauri) -- silently ignore */
    }
  };

  return (
    /**
     * Queue item row container.
     *
     * `border-b border-border-light` draws a separator between items.
     * `last:border-b-0` removes the bottom border from the last item
     * to avoid a double-border with the container edge.
     * `hover:bg-surface-secondary` provides a subtle highlight on hover.
     * `transition-colors` smoothly animates the background change.
     */
    <div className="px-4 py-3 border-b border-border-light last:border-b-0 hover:bg-surface-secondary transition-colors">
      {/*
       * Top row: three-column flex layout.
       * Left: status icon | Center: URL + track info | Right: action buttons.
       * `items-start` aligns all columns to the top edge.
       */}
      <div className="flex items-start gap-3">
        {/*
         * Status icon column.
         *
         * The icon is coloured according to `config.colorClass` from
         * `STATE_CONFIG`. For the 'processing' state, `animate-spin`
         * is applied to the `Loader2` icon to create a spinner effect.
         *
         * `mt-0.5` nudges the icon down slightly to align with the
         * first line of text. `flex-shrink-0` prevents the icon from
         * being compressed when the URL text is long.
         *
         * @see https://tailwindcss.com/docs/animation#spin
         */}
        <div className={`mt-0.5 flex-shrink-0 ${config.colorClass}`}>
          <StateIcon
            size={18}
            className={item.state === 'processing' ? 'animate-spin' : ''}
          />
        </div>

        {/*
         * Center column: URL, current track name, and fallback indicator.
         *
         * `flex-1` absorbs remaining space between the icon and buttons.
         * `min-w-0` is critical for `truncate` to work inside a flex
         * container -- without it, the text would overflow instead of
         * being truncated with an ellipsis.
         *
         * @see https://tailwindcss.com/docs/text-overflow#truncate
         */}
        <div className="flex-1 min-w-0">
          {/*
           * Primary URL text.
           * Shows the first URL from the `item.urls` array (there is
           * typically only one URL per download request).
           * `truncate` clips long URLs with an ellipsis.
           */}
          <p className="text-sm text-content-primary truncate">
            {item.urls[0]}
          </p>

          {/*
           * Current track name -- shown when the backend reports which
           * track is currently being downloaded/processed (for album and
           * playlist downloads that contain multiple tracks).
           */}
          {item.current_track && (
            <p className="text-xs text-content-secondary mt-0.5 truncate">
              {item.current_track}
            </p>
          )}

          {/*
           * Fallback chain indicator.
           *
           * Displayed when `item.fallback_occurred` is true, meaning
           * the backend could not obtain the user's preferred codec
           * and fell back to an alternative from the configured fallback
           * chain (e.g., ALAC -> AAC).
           *
           * Shows a yellow warning icon + the codec that was actually
           * used (`item.codec_used`).
           *
           * @see settings.music_fallback_chain in @/stores/settingsStore.ts
           */}
          {item.fallback_occurred && (
            <div className="flex items-center gap-1 mt-1 text-xs text-status-warning">
              <AlertTriangle size={12} />
              <span>Fallback used (codec: {item.codec_used})</span>
            </div>
          )}
        </div>

        {/*
         * Right column: context-sensitive action buttons.
         *
         * Which buttons are shown depends on the item's state:
         *  - Complete + output_path: "Open folder" button.
         *  - Active or queued:       "Cancel" button.
         *  - Error or cancelled:     "Retry" button.
         *
         * `flex-shrink-0` prevents buttons from being compressed.
         */}
        <div className="flex items-center gap-1 flex-shrink-0">
          {/*
           * "Open folder" button -- shown for completed downloads
           * that have an output path. Calls `handleOpenFolder()` to
           * open the parent directory in the native file manager.
           */}
          {item.state === 'complete' && item.output_path && (
            <button
              onClick={handleOpenFolder}
              className="p-1.5 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-surface-elevated transition-colors"
              title="Open folder"
            >
              <FolderOpen size={14} />
            </button>
          )}

          {/*
           * "Cancel" button -- shown for active (downloading/processing)
           * and queued downloads. Calls `onCancel(item.id)` which
           * propagates up to the parent DownloadQueue and ultimately
           * calls `downloadStore.cancelDownload()`.
           *
           * On hover, the icon turns red (`hover:text-status-error`)
           * to signal the destructive nature of the action.
           */}
          {(isActive || item.state === 'queued') && (
            <button
              onClick={() => onCancel(item.id)}
              className="p-1.5 rounded-platform text-content-tertiary hover:text-status-error hover:bg-surface-elevated transition-colors"
              title="Cancel"
            >
              <X size={14} />
            </button>
          )}

          {/*
           * "Retry" button -- shown for failed or cancelled downloads.
           * Calls `onRetry(item.id)` which propagates up to the parent
           * DownloadQueue and ultimately calls `downloadStore.retryDownload()`.
           */}
          {(item.state === 'error' || item.state === 'cancelled') && (
            <button
              onClick={() => onRetry(item.id)}
              className="p-1.5 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-surface-elevated transition-colors"
              title="Retry"
            >
              <RotateCcw size={14} />
            </button>
          )}
        </div>
      </div>

      {/*
       * Progress section -- only rendered for active downloads.
       *
       * `pl-7` (28px left padding) aligns the progress bar with the
       * URL text, accounting for the 18px icon + 12px gap (gap-3).
       *
       * The ProgressBar receives:
       *  - A numeric value (0-100) when state is 'downloading'.
       *  - `null` when state is 'processing', which renders an
       *    indeterminate/pulsing bar (exact behaviour depends on the
       *    ProgressBar component implementation).
       *
       * @see ProgressBar in @/components/common
       */}
      {isActive && (
        <div className="mt-2 pl-7">
          <ProgressBar
            value={item.state === 'downloading' ? item.progress : null}
          />
          {/*
           * Speed and ETA information -- shown when `item.speed` is
           * available (set by `downloadStore.handleProgressEvent()`
           * when the backend emits a `download_progress` event).
           *
           * `text-[11px]` uses an arbitrary value for a compact font.
           */}
          {item.speed && (
            <div className="flex gap-3 mt-1 text-[11px] text-content-tertiary">
              {/* Download speed (e.g., "1.2 MB/s") */}
              {item.speed && <span>{item.speed}</span>}
              {/* Estimated time remaining (e.g., "ETA: 2:30") */}
              {item.eta && <span>ETA: {item.eta}</span>}
            </div>
          )}
        </div>
      )}

      {/*
       * Error message -- shown only for items in the 'error' state
       * that have a non-null `item.error` string. Displayed in red
       * below the progress section (or directly below the URL if no
       * progress bar is shown).
       *
       * `pl-7` aligns with the URL text above.
       */}
      {item.state === 'error' && item.error && (
        <p className="mt-1.5 pl-7 text-xs text-status-error">{item.error}</p>
      )}
    </div>
  );
}
