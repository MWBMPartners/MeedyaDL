// Copyright (c) 2026 MeedyaSuite
/**
 * @file Individual download queue item component (#911 Phase 1).
 *
 * Displays the status, progress, and controls for a single download as
 * a responsive multi-column row. Visibility tiers (per
 * `.claude/memory/project_multi_service_ui_direction.md` anti-pattern 1):
 *
 *  - **Tier 1** (always visible):
 *     - **Album-art thumbnail** — lazy-loaded 48×48 (#911-2)
 *     - **Primary identifier** — `Artist — Album — Track` from the
 *        early-metadata fetch; falls back to the URL when metadata
 *        hasn't loaded yet (#911-3)
 *     - **Status pill** — coloured rounded-full with icon + label,
 *        rendered via the shared `<StatusPill>` (#911-4)
 *     - **Inline actions** — Cancel / Retry buttons; hidden at 40%
 *        opacity by default, full-opacity on row hover or keyboard
 *        focus (#911-5)
 *  - **Tier 2 (≥ md / 768px):** Platform / service icon (#911-1)
 *  - **Tier 3 (≥ lg / 1024px):** Inline speed / ETA badge for active rows
 *  - **Tier 4 (≥ xl / 1280px):** Codec / quality badge
 *  - **Tier 5 (≥ 2xl / 1536px):** Relative submitted-at time
 *
 * Secondary rows (progress bar, error message, "Retry without Wrapper"
 * pill, warnings list, file-action buttons) sit below the primary row
 * and render full-width when their state predicate fires.
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
 * @see StatusPill in @/components/common -- owns the state-→-pill mapping.
 * @see PlatformIcon in @/lib/PlatformIcon -- service-platform icon renderer.
 */
import { memo, useEffect, useId, useState } from 'react';
import {
  Copy,
  X,
  RotateCcw,
  FolderOpen,
  FileOutput,
  AlertTriangle,
  Music,
  Trash2,
  ChevronUp,
  ChevronDown,
  ChevronsUp,
  ChevronsDown,
} from 'lucide-react';

import { QueueItemExpandPanel } from './QueueItemExpandPanel';

import {
  detectPlatform,
  loadPlatformConfig,
  subscribeToPlatformConfig,
} from '@/lib/platform-config';
import { PlatformIcon } from '@/lib/PlatformIcon';

/**
 * ProgressBar: a horizontal bar component that visualises a 0-100
 * percentage value. Accepts `null` to display an indeterminate state.
 * @see ProgressBar in @/components/common
 */
import { ProgressBar, ContextMenu, ErrorMessageDisplay, StatusPill } from '@/components/common';
import type { ContextMenuItem } from '@/components/common';

/**
 * Type imports for queue item data and download state.
 * @see QueueItemStatus in @/types/index.ts  -- full shape of a queue item.
 * @see DownloadState in @/types/index.ts    -- 'queued' | 'downloading' | ... union.
 */
import type { QueueItemStatus } from '@/types';

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

  /**
   * Whether this row is currently selected for bulk actions (#463).
   * When `undefined` (the legacy path) the checkbox is not rendered
   * and the row behaves exactly as before. When defined, a checkbox
   * is prepended to the row's left edge.
   */
  isSelected?: boolean;

  /**
   * Callback invoked when the user toggles the selection checkbox
   * (#463). Receives the item id. Required when `isSelected` is
   * defined; ignored otherwise.
   */
  onToggleSelect?: (id: string) => void;

  /**
   * Callback invoked when the user picks "Delete" from the right-click
   * menu (#685). The parent is responsible for showing a confirmation
   * modal before invoking the IPC. Receives the unique download ID.
   *
   * The Delete entry is hidden for active rows (`downloading` /
   * `processing`) — the user must Cancel first.
   */
  onDelete: (id: string) => void;

  /**
   * Callbacks for the four queue-reorder context-menu entries (#782).
   * Only fire for `Queued` items; the parent is responsible for
   * invoking the matching IPC (`moveQueueItemToTop` etc.) and showing
   * any toast feedback.
   */
  onMoveToTop: (id: string) => void;
  onMoveUp: (id: string) => void;
  onMoveDown: (id: string) => void;
  onMoveToBottom: (id: string) => void;

  /**
   * Whether each move action is currently meaningful — drives the
   * `disabled` flag on the corresponding context-menu entry so the
   * user sees the action greyed-out when it would be a no-op
   * (e.g. "Move up" on the topmost queued item). Computed by the
   * parent from the item's position within the queued sub-sequence.
   */
  canMoveUp: boolean;
  canMoveDown: boolean;
}

// Per-state icon / colour / label mapping moved to `<StatusPill>`
// (`src/components/common/StatusPill.tsx`) as part of #911-4 so every
// per-row context (queue, history, library scan, future Cmd-K results)
// renders the same status vocabulary. The legacy `STATE_CONFIG` map
// + `StateIcon` / `stateColorClass` helpers were removed in the same
// commit — the pill owns all of it now.

/**
 * Builds the primary identifier line for a queue row (#911-3 — replaces
 * the raw URL render with human-readable metadata when available).
 *
 * Falls back gracefully:
 *  - Artist + Album + current track → `Artist — Album — Track Title`
 *  - Artist + Album only            → `Artist — Album`
 *  - Just one of artist / album     → that one
 *  - Nothing                        → the original URL
 *
 * Returns `null` when called with neither metadata nor URLs (caller
 * should never hit this — every queue item has at least one URL).
 */
function formatPrimaryIdentifier(item: QueueItemStatus): string {
  const artist = item.artist_name?.trim() || null;
  const album = item.album_name?.trim() || null;
  const segments: string[] = [];
  if (artist) segments.push(artist);
  if (album) segments.push(album);
  if (segments.length === 0) return item.urls?.[0] ?? '';
  return segments.join(' — ');
}

/**
 * Formats an ISO 8601 timestamp into a compact relative-time badge
 * for the Tier 5 (≥ 2xl) "submitted" column on queue rows (#911-0).
 *
 * Returns `"just now"`, `"5m"`, `"2h"`, `"3d"`, `"4w"` depending on age.
 * Returns the empty string if the input doesn't parse.
 *
 * Deliberately compact — this is shown alongside other Tier 5 badges
 * and shouldn't dominate the row. For verbose timestamps users can
 * read `created_at` via the row tooltip or expand the row.
 */
function formatRelativeTime(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return '';
  const diffMs = Date.now() - t;
  const diffSec = Math.floor(diffMs / 1000);
  if (diffSec < 30) return 'just now';
  if (diffSec < 60) return `${diffSec}s`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}m`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h`;
  const diffDay = Math.floor(diffHr / 24);
  if (diffDay < 7) return `${diffDay}d`;
  const diffWk = Math.floor(diffDay / 7);
  return `${diffWk}w`;
}

/**
 * Renders the Tier 1 album-art thumbnail (#911-2). Shows a lazy-loaded
 * `<img>` when the backend supplied an `artwork_url`; otherwise falls
 * through to a generic music-note placeholder (Lucide `Music`).
 *
 * The `onError` handler swaps to the placeholder when the URL fails to
 * load (network error, Apple Music CDN hiccup, malformed substitution).
 * The placeholder uses the same dimensions + rounding as the loaded
 * image so the row height stays stable.
 */
function AlbumArtThumbnail({ artworkUrl }: { artworkUrl: string | null | undefined }) {
  const [errored, setErrored] = useState(false);
  const showImage = artworkUrl && !errored;
  return (
    <div
      className="w-12 h-12 flex-shrink-0 rounded-platform overflow-hidden bg-surface-elevated flex items-center justify-center text-content-tertiary"
      aria-hidden="true"
    >
      {showImage ? (
        <img
          src={artworkUrl}
          alt=""
          loading="lazy"
          className="w-full h-full object-cover"
          onError={() => setErrored(true)}
        />
      ) : (
        <Music size={20} />
      )}
    </div>
  );
}

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
function QueueItemComponent({
  item,
  onCancel,
  onRetry,
  onRetryWithoutWrapper,
  onCopyUrl,
  onDelete,
  onMoveToTop,
  onMoveUp,
  onMoveDown,
  onMoveToBottom,
  canMoveUp,
  canMoveDown,
  isSelected,
  onToggleSelect,
}: QueueItemProps) {
  // Kick off the platform-config IPC load on first mount so the Tier 2
  // platform icon resolves. The helper is idempotent across rows — only
  // the first row triggers the actual IPC call; subsequent rows reuse
  // the module-level cache. The subscriber re-renders this row once
  // the config arrives (typically <100ms after mount).
  const [, setPlatformConfigReady] = useState(false);
  useEffect(() => {
    const unsubscribe = subscribeToPlatformConfig(() => setPlatformConfigReady(true));
    loadPlatformConfig();
    return unsubscribe;
  }, []);

  const platform = detectPlatform(item.urls);
  const primaryIdentifier = formatPrimaryIdentifier(item);
  const submittedRelative = formatRelativeTime(item.created_at);

  /** Right-click context menu position and visibility state. */
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    visible: boolean;
  }>({ x: 0, y: 0, visible: false });

  /**
   * Click-to-expand affordance (#911 Phase 1 item 0 sub-item).
   *
   * Per-row useState matches the existing `contextMenu` precedent.
   * Lifting to a parent `Set<id>` is unnecessary because rows are
   * memoised + virtualised and the expansion state is intentionally
   * ephemeral (a virtualiser-recycled DOM node mounts fresh; if the
   * user wanted persistence across re-virtualisation they'd use the
   * `data-index` / `row id` keying that already exists in
   * `QueueListVirtualized.tsx`'s `getItemKey`).
   *
   * Each row gets a stable `useId()` so the chevron's
   * `aria-controls` matches the panel's `id` — required by the
   * WAI-ARIA Disclosure Pattern. The id is also unique across
   * multiple simultaneously-expanded rows.
   */
  const [expanded, setExpanded] = useState(false);
  const detailsPanelId = useId();

  /**
   * Whether this completed item has non-fatal warnings. Forwarded to
   * `<StatusPill>` so the pill flips from green ("Complete") to amber
   * ("Warnings") for the affected row.
   */
  const hasWarnings =
    item.state === 'complete' && item.warnings && item.warnings.length > 0;

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
    // Queue reorder actions (#782) — visible only for `Queued` items
    // since active and terminal rows can't be reordered. The four
    // entries form one logical group with a trailing separator so
    // they sit visually apart from the Retry / Delete cluster below.
    // Each entry is `disabled` when the move would be a no-op (e.g.
    // "Move up" on the topmost queued item).
    ...(item.state === 'queued'
      ? [
          {
            label: 'Move to Top',
            icon: <ChevronsUp size={14} />,
            onClick: () => onMoveToTop(item.id),
            disabled: !canMoveUp,
          },
          {
            label: 'Move Up',
            icon: <ChevronUp size={14} />,
            onClick: () => onMoveUp(item.id),
            disabled: !canMoveUp,
          },
          {
            label: 'Move Down',
            icon: <ChevronDown size={14} />,
            onClick: () => onMoveDown(item.id),
            disabled: !canMoveDown,
          },
          {
            label: 'Move to Bottom',
            icon: <ChevronsDown size={14} />,
            onClick: () => onMoveToBottom(item.id),
            disabled: !canMoveDown,
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
    // "Delete" — non-active rows only (#685). Active items must be
    // cancelled first; the Rust guard rejects deletion of active rows
    // for defense-in-depth, but hiding the entry keeps the UI honest.
    ...(item.state !== 'downloading' && item.state !== 'processing'
      ? [
          {
            label: 'Delete',
            icon: <Trash2 size={14} />,
            onClick: () => onDelete(item.id),
          },
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
      className={`queue-row-enter group px-4 py-3 border-b border-border-light last:border-b-0 transition-colors ${
        isSelected ? 'bg-accent/5 hover:bg-accent/10' : 'hover:bg-surface-secondary'
      }`}
      role="listitem"
      aria-label={`Download: ${item.urls?.[0] ?? 'unknown'}`}
      onContextMenu={handleContextMenu}
    >
      {/*
       * Top row: responsive flex columns (#911-0). Tier-1 columns
       * (checkbox / album art / primary identifier / status icon /
       * actions) are always visible. Tier 2+ columns reveal at
       * Tailwind breakpoints (`md` 768, `lg` 1024, `xl` 1280, `2xl`
       * 1536) so narrow windows stay scannable while wide windows
       * surface monitoring data inline. Anti-pattern 1 in
       * `.claude/memory/project_multi_service_ui_direction.md`:
       * column visibility is responsive — never fixed density at
       * all widths.
       */}
      <div className="flex items-center gap-3">
        {/* Tier 1 — bulk-select checkbox (#463). Conditional on the
         * parent passing isSelected + onToggleSelect; otherwise the
         * row renders without a checkbox column. */}
        {isSelected !== undefined && onToggleSelect && (
          <input
            type="checkbox"
            className="accent-accent cursor-pointer flex-shrink-0"
            checked={isSelected}
            onChange={() => onToggleSelect(item.id)}
            aria-label={isSelected ? 'Deselect queue item' : 'Select queue item'}
          />
        )}

        {/* Tier 1 — album-art thumbnail (#911-2). Lazy-loaded; falls
         * back to a music-note placeholder when artwork_url is null
         * or the image fails to load. */}
        <AlbumArtThumbnail artworkUrl={item.artwork_url} />

        {/* Tier 2 (≥ md / 768px) — platform / service icon (#911-1).
         * Hidden on narrow windows to keep the primary identifier
         * readable; the album art alone already encodes content
         * identity at that width. Click-to-expand affordance
         * (future PR) will surface this column inline at any width. */}
        <div className="hidden md:flex flex-shrink-0">
          <PlatformIcon platform={platform} size={20} />
        </div>

        {/* Tier 1 — primary identifier (Artist — Album — Track) +
         * current-track caption + fallback indicator. `flex-1` claims
         * the remaining horizontal space; `min-w-0` lets `truncate`
         * work inside a flex container. */}
        <div className="flex-1 min-w-0">
          <p
            className="text-sm text-content-primary truncate"
            title={item.urls?.[0]}
          >
            {primaryIdentifier}
          </p>

          {/* Current-track + Track N/M caption — shown when the backend
           * has reported per-track progress. Suppressed for
           * `total_tracks === 1` per #609 (GAMDL v3.1 reports
           * `[Track 1/1]` for single-song URLs). */}
          {item.current_track && (
            <p className="text-xs text-content-secondary mt-0.5 truncate">
              {item.current_track}
              {isActive &&
                item.total_tracks != null &&
                item.total_tracks > 1 &&
                item.completed_tracks != null && (
                  <span className="text-content-tertiary ml-1.5">
                    (Track {item.completed_tracks} of {item.total_tracks})
                  </span>
                )}
            </p>
          )}

          {/* Fallback-chain indicator — when GAMDL couldn't get the
           * preferred codec and fell back to something else. */}
          {item.fallback_occurred && (
            <div className="flex items-center gap-1 mt-1 text-xs text-status-warning">
              <AlertTriangle size={12} />
              <span>Fallback used (codec: {item.codec_used})</span>
            </div>
          )}
        </div>

        {/* Tier 3 (≥ lg / 1024px) — inline speed/ETA badge for active
         * downloads. The full-width progress bar still renders below
         * the row; this column surfaces the headline number compactly
         * for at-a-glance monitoring on wide windows. */}
        {isActive && item.speed && (
          <div
            className="hidden lg:flex flex-shrink-0 text-xs text-content-tertiary whitespace-nowrap tabular-nums"
            aria-label={item.eta ? `${item.speed}, ETA ${item.eta}` : item.speed}
          >
            {item.speed}
            {item.eta ? ` · ${item.eta}` : ''}
          </div>
        )}

        {/* Tier 4 (≥ xl / 1280px) — codec / quality badge. Reflects
         * the codec the backend actually wrote (may differ from the
         * requested codec when fallback occurred). */}
        {item.codec_used && (
          <div className="hidden xl:flex flex-shrink-0 px-2 py-0.5 rounded-full bg-surface-elevated text-[11px] text-content-secondary whitespace-nowrap uppercase tracking-wide">
            {item.codec_used}
          </div>
        )}

        {/* Tier 5 (≥ 2xl / 1536px) — relative submitted-at time.
         * Compact format (just-now / 5m / 2h / 3d / 4w). Full
         * timestamp is surfaced via the row's tooltip / context
         * menu. */}
        {submittedRelative && (
          <div
            className="hidden 2xl:flex flex-shrink-0 text-[11px] text-content-tertiary whitespace-nowrap tabular-nums"
            title={`Queued ${item.created_at}`}
          >
            {submittedRelative}
          </div>
        )}

        {/* Tier 1 — status pill (#911-4). Coloured rounded-full with
         * icon + label. Replaces the previous icon-only indicator
         * with a more scan-able pill that pairs colour, icon, and
         * label into a single visual unit. */}
        <StatusPill
          state={item.state}
          hasWarnings={hasWarnings}
          className="flex-shrink-0"
        />

        {/* Tier 1 — inline action buttons (Cancel / Retry). Hover-
         * revealed at 40% opacity by default; full opacity on row
         * hover OR when any button within receives keyboard focus
         * (`focus-within:opacity-100`) so Tab-navigation never loses
         * track of the active button. #911-5. */}
        <div className="flex items-center gap-1 flex-shrink-0 opacity-40 group-hover:opacity-100 focus-within:opacity-100 transition-opacity">
          {(isActive || item.state === 'queued') && (
            <button
              onClick={() => onCancel(item.id)}
              className="p-1.5 rounded-platform text-content-tertiary hover:text-status-error hover:bg-surface-elevated transition-colors"
              // a11y: title matches aria-label so sighted-hover tooltip
              // and screen-reader announcement say the same thing (#950)
              title="Cancel download"
              aria-label="Cancel download"
            >
              <X size={14} />
            </button>
          )}
          {(item.state === 'error' || item.state === 'cancelled') && (
            <button
              onClick={() => onRetry(item.id)}
              className="p-1.5 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-surface-elevated transition-colors"
              // a11y: title matches aria-label so sighted-hover tooltip
              // and screen-reader announcement say the same thing (#950)
              title="Retry download"
              aria-label="Retry download"
            >
              <RotateCcw size={14} />
            </button>
          )}
        </div>

        {/*
         * #911 Phase 1 item 0 sub-item — click-to-expand disclosure
         * chevron. Placed AFTER the actions cluster (not inside) so
         * its own `opacity-100` isn't subject to the cluster's
         * `opacity-40 group-hover:opacity-100` cascade. Disclosure
         * controls must be persistently visible per the WAI-ARIA
         * Disclosure Pattern — non-pointer users (keyboard, switch,
         * voice control) need to see the affordance exists before
         * they've already focused into the row.
         *
         * Tab order intent: Cancel/Retry come first (inside the
         * cluster), Expand last. Convention is "safest action last
         * in tab order" so an accidental Enter doesn't trigger a
         * destructive Cancel.
         *
         * Reduced-motion compliance: the `transition-transform`
         * snaps cleanly via globals.css's `*` reduced-motion guard
         * (zeroes transition-duration to 0.01ms). No explicit
         * `motion-safe:` prefix needed.
         */}
        <button
          type="button"
          onClick={() => setExpanded((prev) => !prev)}
          aria-expanded={expanded}
          aria-controls={detailsPanelId}
          aria-label={
            expanded
              ? 'Hide additional download details'
              : 'Show additional download details'
          }
          className="flex-shrink-0 ml-1 p-1.5 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-surface-elevated transition-colors"
          title={expanded ? 'Hide details' : 'Show details'}
        >
          <ChevronDown
            size={14}
            className={`transition-transform duration-200 ${
              expanded ? 'rotate-180' : ''
            }`}
            aria-hidden="true"
          />
        </button>
      </div>

      {/*
       * Click-to-expand row detail panel — only mounts when
       * `expanded` is true. The panel mounts BEFORE the file-action
       * row so the visual order reads:
       *   Primary row → Progress → Error → Retry-no-wrapper → Warnings
       *   → Expand panel → File actions → Context menu portal.
       * Putting it ahead of the file-action row groups the read-only
       * inspection content together; the action buttons stay at the
       * bottom where users expect them.
       */}
      {expanded && (
        <QueueItemExpandPanel
          item={item}
          panelId={detailsPanelId}
          primaryIdentifier={primaryIdentifier}
          platformLabel={platform?.name ?? null}
          submittedRelative={submittedRelative}
        />
      )}

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
        <div className="mt-2 pl-[60px]">
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
        <div className="mt-1.5 pl-[60px]">
          <ErrorMessageDisplay
            message={item.error}
            sourceUrl={item.urls?.[0]}
            // Queue rows are full-width so we don't aggressively
            // truncate, but tooltip + right-click (copy / report
            // upstream when applicable) still apply.
            truncateLines={null}
          />
        </div>
      )}

      {/*
       * "Retry without Wrapper" pill -- below the error for failed
       * wrapper-auth downloads. Visual prominence bumped per #890
       * (was text-xs / bg-surface-secondary → text-sm / accent
       * border) because users were missing the original passive-pill
       * styling. Context-menu entry is the secondary path.
       */}
      {item.state === 'error' && item.used_wrapper && (
        <div className="mt-2 pl-[60px]" data-testid="retry-without-wrapper-pill">
          <button
            type="button"
            onClick={() => onRetryWithoutWrapper(item.id)}
            className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-platform border border-accent/40 text-accent bg-accent/5 hover:bg-accent/10 hover:border-accent/60 transition-colors"
            title="Disable wrapper and retry this download with cookie-based authentication"
            // #945: long-form aria-label carries the cookie-vs-wrapper
            // distinction for screen reader users.
            aria-label="Retry without wrapper (uses cookie-based authentication)"
          >
            <RotateCcw size={14} />
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
        <div className="mt-1.5 pl-[60px] space-y-0.5">
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
        <div className="mt-2 pl-[60px] flex gap-2">
          {/* Hide "Open File" for directories (albums/playlists) -- not meaningful for multiple files */}
          {!item.output_is_directory && (
            <button
              type="button"
              onClick={handleOpenFile}
              className="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs rounded-platform text-content-secondary hover:text-content-primary bg-surface-secondary hover:bg-surface-elevated transition-colors"
              title="Open in default application"
              // #945: screen readers announce what this icon-plus-label
              // button actually does.
              aria-label="Open downloaded file in default application"
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
            // #945: screen readers announce what this icon-plus-label
            // button actually does.
            aria-label="Reveal downloaded file's folder in file manager"
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

/**
 * Field-aware equality check for `React.memo` (#689).
 *
 * `getQueueStatus` IPC polling at ~10×/sec on active downloads
 * deserialises fresh JSON, producing new object references for every
 * `item` even when the content is unchanged. So a shallow `prev.item
 * !== next.item` check would treat every poll as a change and defeat
 * the memo. Instead, we compare only the fields that actually drive
 * rendering of this row — when none have changed, React skips the
 * re-render even if the parent passed a fresh object.
 *
 * Callbacks are deliberately *not* compared: they are invoke-only
 * (clicked never read), so an identity change doesn't require a
 * re-render. The parent already wraps every callback in
 * `useCallback` (#689 sibling change in DownloadQueue.tsx) so they
 * stay stable across renders for unrelated rows.
 *
 * `canMoveUp` / `canMoveDown` ARE compared because they affect the
 * `disabled` state of the right-click menu entries the row renders.
 *
 * Returning `true` here means "props are equal — skip the re-render."
 * Returning `false` means "render again."
 */
function arePropsEqual(prev: QueueItemProps, next: QueueItemProps): boolean {
  if (prev.item.id !== next.item.id) return false;
  if (prev.canMoveUp !== next.canMoveUp) return false;
  if (prev.canMoveDown !== next.canMoveDown) return false;
  if (prev.isSelected !== next.isSelected) return false;

  const a = prev.item;
  const b = next.item;
  return (
    a.state === b.state &&
    a.progress === b.progress &&
    a.processing_progress === b.processing_progress &&
    a.processing_label === b.processing_label &&
    a.current_track === b.current_track &&
    a.codec_used === b.codec_used &&
    a.fallback_occurred === b.fallback_occurred &&
    a.used_wrapper === b.used_wrapper &&
    a.output_path === b.output_path &&
    a.output_is_directory === b.output_is_directory &&
    a.error === b.error &&
    a.speed === b.speed &&
    a.eta === b.eta &&
    a.total_tracks === b.total_tracks &&
    a.completed_tracks === b.completed_tracks &&
    a.album_name === b.album_name &&
    a.artist_name === b.artist_name &&
    // Arrays: shallow ref check is sufficient. The store replaces
    // these in-place via setState, so ref-equal means content-equal
    // for our purposes; if the array is rebuilt, the content has
    // genuinely changed.
    a.urls === b.urls &&
    a.warnings === b.warnings
  );
}

/**
 * Memoised `QueueItem` export — see {@link arePropsEqual} for the
 * rationale and field list. Without this wrap, every progress event
 * re-renders every row in the queue.
 */
export const QueueItem = memo(QueueItemComponent, arePropsEqual);
