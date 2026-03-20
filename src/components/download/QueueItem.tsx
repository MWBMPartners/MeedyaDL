// Copyright (c) 2026 MeedyaDL
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
 *  - `FileOutput`    -> open file     (@see https://lucide.dev/icons/file-output)
 *  - `AlertTriangle` -> fallback warn (@see https://lucide.dev/icons/alert-triangle)
 */
import { useState } from 'react';
import {
  Clock,
  Copy,
  Download,
  CheckCircle,
  XCircle,
  Loader2,
  X,
  RotateCcw,
  FolderOpen,
  FileOutput,
  AlertTriangle,
} from 'lucide-react';

/**
 * ProgressBar: a horizontal bar component that visualises a 0-100
 * percentage value. Accepts `null` to display an indeterminate state.
 * @see ProgressBar in @/components/common
 */
import { ProgressBar, ContextMenu } from '@/components/common';
import type { ContextMenuItem } from '@/components/common';

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

  /**
   * Callback invoked when the user clicks "Retry without Wrapper" on
   * a failed download that was attempted with wrapper enabled.
   * Receives the download ID. The parent uses this to call
   * `downloadStore.retryWithoutWrapper(id)`.
   */
  onRetryWithoutWrapper: (id: string) => void;

  /**
   * Callback invoked after the source URL is copied to the clipboard
   * via the context menu. The parent uses this to show a toast
   * notification confirming the copy.
   */
  onCopyUrl: (url: string) => void;
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
export function QueueItem({
  item,
  onCancel,
  onRetry,
  onRetryWithoutWrapper,
  onCopyUrl,
}: QueueItemProps) {
  /**
   * Look up the visual configuration (icon, colour, label) for the
   * current download state from the static `STATE_CONFIG` record.
   */
  const config = STATE_CONFIG[item.state];

  /** Right-click context menu position and visibility state. */
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    visible: boolean;
  }>({ x: 0, y: 0, visible: false });

  /**
   * Whether this completed item has non-fatal warnings. When true, the
   * status icon switches from green CheckCircle to amber AlertTriangle
   * to signal that the download succeeded but encountered issues.
   */
  const hasWarnings = item.state === 'complete' && item.warnings && item.warnings.length > 0;

  /** The Lucide icon component for the current state, overridden for warnings. */
  const StateIcon = hasWarnings ? AlertTriangle : config.icon;

  /** The color class, overridden to amber for completed-with-warnings. */
  const stateColorClass = hasWarnings ? 'text-status-warning' : config.colorClass;

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
      if (item.output_is_directory) {
        // Path is already a directory (album/playlist) — open it directly
        await open(item.output_path);
      } else {
        /*
         * Extract the parent directory path from the full file path.
         * Example: '/Users/me/Music/Artist/Album/01 Track.m4a'
         *       -> '/Users/me/Music/Artist/Album'
         */
        const sep = item.output_path.includes('\\') ? '\\' : '/';
        const parentDir = item.output_path.substring(0, item.output_path.lastIndexOf(sep));
        await open(parentDir);
      }
    } catch {
      /* Shell API unavailable (running outside Tauri) -- silently ignore */
    }
  };

  /**
   * Opens the output file in its default application (e.g., Music.app,
   * VLC, or the system default media player).
   *
   * Uses the same `open()` function from the shell plugin, which
   * delegates to the OS default handler for the file type.
   */
  const handleOpenFile = async () => {
    if (!item.output_path) return;
    try {
      const { open } = await import('@tauri-apps/plugin-shell');
      await open(item.output_path);
    } catch {
      /* Shell API unavailable (running outside Tauri) -- silently ignore */
    }
  };

  /**
   * Opens the right-click context menu at the cursor position.
   * Suppresses the native browser context menu.
   */
  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
  };

  /**
   * Build context menu items dynamically based on the item's current
   * state and available data. Items are conditionally included so the
   * menu only shows actions that are relevant.
   */
  const contextMenuItems: ContextMenuItem[] = [
    // Always available: copy the source URL to clipboard
    {
      label: 'Copy Source Link',
      icon: <Copy size={14} />,
      onClick: () => {
        navigator.clipboard
          .writeText(item.urls[0])
          .then(() => {
            onCopyUrl(item.urls[0]);
          })
          .catch(() => {});
      },
    },
    // Available when output files exist (complete, or error with output)
    ...(item.output_path
      ? [
          {
            label: 'Open Folder',
            icon: <FolderOpen size={14} />,
            onClick: handleOpenFolder,
            separator: true,
          },
        ]
      : []),
    // Available for failed or cancelled downloads
    ...(item.state === 'error' || item.state === 'cancelled'
      ? [
          {
            label: 'Retry Download',
            icon: <RotateCcw size={14} />,
            onClick: () => onRetry(item.id),
            separator: !item.used_wrapper && !item.output_path,
          },
          // "Retry without Wrapper" -- only for downloads that used wrapper auth
          ...(item.used_wrapper
            ? [
                {
                  label: 'Retry without Wrapper',
                  icon: <RotateCcw size={14} />,
                  onClick: () => onRetryWithoutWrapper(item.id),
                  separator: !item.output_path,
                },
              ]
            : []),
        ]
      : []),
  ];

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
    <div
      className="px-4 py-3 border-b border-border-light last:border-b-0 hover:bg-surface-secondary transition-colors"
      onContextMenu={handleContextMenu}
    >
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
        <div className={`mt-0.5 flex-shrink-0 ${stateColorClass}`}>
          <StateIcon size={18} className={item.state === 'processing' ? 'animate-spin' : ''} />
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
          <p className="text-sm text-content-primary truncate">{item.urls[0]}</p>

          {/*
           * Current track name and track counter -- shown when the backend
           * reports which track is currently being downloaded/processed
           * (for album and playlist downloads that contain multiple tracks).
           *
           * When `total_tracks` and `completed_tracks` are both available
           * and the item is in an active state, a "Track N of M" counter
           * is appended after the track name for aggregate progress context.
           */}
          {item.current_track && (
            <p className="text-xs text-content-secondary mt-0.5 truncate">
              {item.current_track}
              {isActive &&
                item.total_tracks != null &&
                item.completed_tracks != null && (
                  <span className="text-content-tertiary ml-1.5">
                    (Track {item.completed_tracks} of {item.total_tracks})
                  </span>
                )}
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
              aria-label="Cancel download"
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
              aria-label="Retry download"
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
          <ProgressBar value={item.state === 'downloading' ? item.progress : null} />
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

      {/*
       * "Retry without Wrapper" pill button -- shown below the error message
       * for failed downloads that were attempted with wrapper authentication.
       * Allows users to fall back to cookie-based auth without navigating
       * to settings. Styled as a subtle pill to avoid visual clutter.
       */}
      {item.state === 'error' && item.used_wrapper && (
        <div className="mt-1.5 pl-7">
          <button
            type="button"
            onClick={() => onRetryWithoutWrapper(item.id)}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-platform text-content-secondary hover:text-content-primary bg-surface-secondary hover:bg-surface-elevated transition-colors"
            title="Disable wrapper and retry with cookie-based authentication"
          >
            <RotateCcw size={12} />
            Retry without Wrapper
          </button>
        </div>
      )}

      {/*
       * Warning messages -- shown for completed items that encountered
       * non-fatal issues during the download (e.g., GAMDL logged error
       * lines but still exited successfully). Displayed in amber below
       * the progress section or error message.
       */}
      {hasWarnings && (
        <div className="mt-1.5 pl-7 space-y-0.5">
          {item.warnings.map((warning, i) => (
            <p key={i} className="text-xs text-status-warning">
              {warning}
            </p>
          ))}
        </div>
      )}

      {/*
       * File action row -- shown for completed downloads that have an
       * output path. Provides labeled "Open File" and "Open Folder"
       * buttons that are clearly associated with the queue item above.
       *
       * `pl-7` aligns with the URL text (matches icon width + gap-3).
       * Uses compact pill-style buttons with bg-surface-secondary for
       * visual distinction from the inline icon buttons.
       */}
      {item.state === 'complete' && item.output_path && (
        <div className="mt-2 pl-7 flex gap-2">
          {/* Hide "Open File" for directories (albums/playlists) -- not meaningful for multiple files */}
          {!item.output_is_directory && (
            <button
              type="button"
              onClick={handleOpenFile}
              className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-platform text-content-secondary hover:text-content-primary bg-surface-secondary hover:bg-surface-elevated transition-colors"
              title="Open in default application"
            >
              <FileOutput size={12} />
              Open File
            </button>
          )}
          <button
            type="button"
            onClick={handleOpenFolder}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-platform text-content-secondary hover:text-content-primary bg-surface-secondary hover:bg-surface-elevated transition-colors"
            title="Reveal in file manager"
          >
            <FolderOpen size={12} />
            Open Folder
          </button>
        </div>
      )}

      {/* Right-click context menu (rendered via portal) */}
      {contextMenu.visible && (
        <ContextMenu
          items={contextMenuItems}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu((prev) => ({ ...prev, visible: false }))}
        />
      )}
    </div>
  );
}
