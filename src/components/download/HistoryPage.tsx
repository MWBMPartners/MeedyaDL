// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download History page component.
// Displays a persistent record of all completed and failed downloads.
// Each entry shows the date, URL/title, codec badge, and status icon.
// A search input filters entries by title, artist, album, or URL.
// The "Clear History" button deletes all entries from disk.
// Failed entries can be retried individually (button or right-click) or
// in bulk via the "Retry All Failed" header action (#665).

import { useEffect, useMemo, useState, useCallback } from 'react';

import { PageHeader } from '@/components/layout/PageHeader';
import { Button, ContextMenu, ErrorMessageDisplay, Modal } from '@/components/common';
import type { ContextMenuItem } from '@/components/common';
import {
  Trash2,
  Search,
  CheckCircle,
  XCircle,
  X,
  FolderOpen,
  Clock,
  RotateCcw,
  Copy,
} from 'lucide-react';
// Trash2 used for both "Clear History" header button and the per-row Delete
// context menu entry (#685) — single icon import covers both call sites.
import {
  listHistory,
  clearHistory,
  searchHistory,
  startDownload,
  deleteHistoryEntry,
} from '@/lib/tauri-commands';
import { useUiStore } from '@/stores/uiStore';

import type { HistoryEntry } from '@/types';

/**
 * Formats an ISO 8601 timestamp to a short locale-appropriate date/time string.
 */
function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return iso;
  }
}

/**
 * Extracts a display label from a history entry.
 * Prefers title, falls back to a truncated URL.
 */
function getDisplayLabel(entry: HistoryEntry): string {
  if (entry.title) return entry.title;
  // Truncate long URLs for display
  const url = entry.url;
  if (url.length > 80) {
    return url.slice(0, 77) + '...';
  }
  return url;
}

/**
 * History entries are recorded as `success` or anything else; this helper
 * centralises the "is this a candidate for retry?" predicate so the
 * per-item buttons, the right-click menu, and the bulk action all agree.
 *
 * Includes both hard failures and partial-success entries (the dominant
 * `GAMDL reported N per-track error(s) even though the process exited 0`
 * shape lands here as `status: "failed"`).
 */
function isFailed(entry: HistoryEntry): boolean {
  return entry.status !== 'success';
}

/**
 * Renders the download history page with search, entry list, retry
 * affordances per row, and bulk retry / clear actions.
 */
export function HistoryPage() {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [contextMenu, setContextMenu] = useState<{
    entry: HistoryEntry;
    x: number;
    y: number;
  } | null>(null);
  const [showRetryAllConfirm, setShowRetryAllConfirm] = useState(false);
  /**
   * Per-item Delete confirmation modal target (#685). Holds the entry
   * being deleted so the modal can show a meaningful label. `null` =
   * modal closed.
   */
  const [deleteTarget, setDeleteTarget] = useState<HistoryEntry | null>(null);
  const addToast = useUiStore((s) => s.addToast);

  /** Loads history entries from the backend. */
  const loadEntries = useCallback(async () => {
    try {
      setIsLoading(true);
      const result = searchQuery.trim()
        ? await searchHistory(searchQuery.trim())
        : await listHistory();
      setEntries(result);
    } catch (err) {
      console.error('Failed to load history:', err);
    } finally {
      setIsLoading(false);
    }
  }, [searchQuery]);

  // Load entries on mount and when search query changes
  useEffect(() => {
    // Debounce search queries by 300ms
    const timer = setTimeout(() => {
      loadEntries();
    }, searchQuery ? 300 : 0);
    return () => clearTimeout(timer);
  }, [loadEntries, searchQuery]);

  /** Handles the Clear History button click. */
  const handleClear = useCallback(async () => {
    try {
      await clearHistory();
      setEntries([]);
      addToast('Download history cleared', 'info');
    } catch (err) {
      console.error('Failed to clear history:', err);
      addToast('Failed to clear history', 'error');
    }
  }, [addToast]);

  /** Opens the output folder for a history entry via the shell. */
  const handleOpenFolder = useCallback(async (filePath: string) => {
    try {
      const { open } = await import('@tauri-apps/plugin-shell');
      // Open the parent directory if filePath is a file
      const path = filePath.replace(/[/\\][^/\\]+$/, '');
      await open(path);
    } catch (err) {
      console.error('Failed to open folder:', err);
    }
  }, []);

  /**
   * Re-enqueue a failed history entry via `start_download`. The backend
   * removes the old history row for the same URL, and the new attempt writes
   * one replacement row on completion.
   */
  const handleRetryOne = useCallback(
    async (entry: HistoryEntry) => {
      try {
        await startDownload({ urls: [entry.url] });
        setEntries((current) => current.filter((item) => item.id !== entry.id));
        addToast(`Re-queued: ${getDisplayLabel(entry)}`, 'success');
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        addToast(`Retry failed: ${message}`, 'error');
      }
    },
    [addToast],
  );

  /**
   * Copy the entry's URL to the clipboard. Surface as a toast so the user
   * gets confirmation — the failure mode (clipboard API blocked) is
   * silent otherwise on some browsers.
   */
  const handleCopyUrl = useCallback(
    async (entry: HistoryEntry) => {
      try {
        await navigator.clipboard.writeText(entry.url);
        addToast('URL copied to clipboard', 'info');
      } catch {
        addToast('Could not copy to clipboard', 'error');
      }
    },
    [addToast],
  );

  /**
   * Bulk retry: dedupe URLs from every failed entry and submit them in
   * a single `start_download` call. Showing one summary toast with the
   * count beats N individual ones for a 12-item retry.
   */
  const failedEntries = useMemo(() => entries.filter(isFailed), [entries]);
  const failedCount = failedEntries.length;

  const handleRetryAllConfirmed = useCallback(async () => {
    setShowRetryAllConfirm(false);
    // Dedupe URLs — multiple failed history rows can point at the same URL
    // (a user retried, it failed again, both attempts are recorded).
    const uniqueUrls = Array.from(new Set(failedEntries.map((e) => e.url)));
    if (uniqueUrls.length === 0) {
      addToast('No failed downloads to retry', 'info');
      return;
    }
    try {
      await startDownload({ urls: uniqueUrls });
      setEntries((current) => current.filter((entry) => !uniqueUrls.includes(entry.url)));
      addToast(
        `Re-queued ${uniqueUrls.length} download${uniqueUrls.length !== 1 ? 's' : ''}` +
          (uniqueUrls.length < failedCount
            ? ` (${failedCount - uniqueUrls.length} duplicate URL${
                failedCount - uniqueUrls.length !== 1 ? 's' : ''
              } skipped)`
            : ''),
        'success',
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      addToast(`Bulk retry failed: ${message}`, 'error');
    }
  }, [failedEntries, failedCount, addToast]);

  /**
   * Per-item Delete (#685). Confirms via modal, then removes the entry
   * from disk via the IPC. Updates local state on success so the UI
   * doesn't flicker during the round-trip.
   */
  const handleDeleteConfirmed = useCallback(async () => {
    const target = deleteTarget;
    if (!target) return;
    setDeleteTarget(null);
    try {
      await deleteHistoryEntry(target.id);
      setEntries((current) => current.filter((entry) => entry.id !== target.id));
      addToast('History entry removed', 'info');
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      addToast(`Failed to remove: ${message}`, 'error');
    }
  }, [deleteTarget, addToast]);

  /** Right-click handler — opens the per-row context menu at the cursor. */
  const handleContextMenu = useCallback(
    (event: React.MouseEvent, entry: HistoryEntry) => {
      event.preventDefault();
      setContextMenu({ entry, x: event.clientX, y: event.clientY });
    },
    [],
  );

  /**
   * Build the menu items for the right-click context menu. The Retry
   * entry only appears for failed rows; Open Folder appears only when
   * the entry has a recorded path. Copy URL is always available so the
   * menu is never empty.
   */
  const buildContextMenuItems = useCallback(
    (entry: HistoryEntry): ContextMenuItem[] => {
      const items: ContextMenuItem[] = [
        {
          label: 'Copy URL',
          icon: <Copy size={14} />,
          onClick: () => handleCopyUrl(entry),
        },
      ];
      if (isFailed(entry)) {
        items.push({
          label: 'Retry Download',
          icon: <RotateCcw size={14} />,
          onClick: () => handleRetryOne(entry),
        });
      }
      if (entry.file_path) {
        items.push({
          label: 'Open Folder',
          icon: <FolderOpen size={14} />,
          onClick: () => handleOpenFolder(entry.file_path!),
        });
      }
      // Delete entry (#685). Always available — history rows are
      // metadata-only, so deletion never affects on-disk audio.
      items.push({
        label: 'Delete',
        icon: <Trash2 size={14} />,
        onClick: () => setDeleteTarget(entry),
        separator: true,
      });
      return items;
    },
    [handleCopyUrl, handleRetryOne, handleOpenFolder],
  );

  const subtitle = searchQuery
    ? `${entries.length} result${entries.length !== 1 ? 's' : ''}`
    : `${entries.length} download${entries.length !== 1 ? 's' : ''}`;

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="History"
        subtitle={subtitle}
        actions={
          <div className="flex gap-2">
            {/* Retry All Failed (#665) — shown only when failed entries exist
                and we're not in a search context (avoids confusion about
                "all failed" meaning "all matching the search"). */}
            {failedCount > 0 && !searchQuery && (
              <Button
                variant="ghost"
                size="sm"
                icon={<RotateCcw size={14} />}
                onClick={() => setShowRetryAllConfirm(true)}
              >
                Retry All Failed ({failedCount})
              </Button>
            )}
            {entries.length > 0 && !searchQuery && (
              <Button variant="ghost" size="sm" icon={<Trash2 size={14} />} onClick={handleClear}>
                Clear History
              </Button>
            )}
          </div>
        }
      />

      {/* Search bar */}
      <div className="px-6 py-3 border-b border-border-light">
        <div className="relative">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-content-tertiary" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search by title, artist, album, or URL..."
            className="w-full pl-9 pr-8 py-2 text-sm rounded-platform bg-input-bg border border-input-border text-content-primary placeholder:text-content-tertiary focus:outline-none focus:ring-1 focus:ring-accent"
          />
          {searchQuery && (
            <button
              type="button"
              onClick={() => setSearchQuery('')}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-content-tertiary hover:text-content-primary"
              aria-label="Clear search"
            >
              <X size={14} />
            </button>
          )}
        </div>
      </div>

      {/* Entry list */}
      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex items-center justify-center h-32 text-content-tertiary text-sm">
            Loading history...
          </div>
        ) : entries.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-64 text-content-tertiary">
            {!searchQuery && <Clock size={32} className="mb-3 opacity-40" />}
            <p className="text-sm font-medium">
              {searchQuery ? 'No matching downloads found.' : 'No download history yet.'}
            </p>
            {!searchQuery && (
              <p className="text-xs mt-1">
                Completed and failed downloads will appear here.
              </p>
            )}
          </div>
        ) : (
          <div className="divide-y divide-border-light">
            {entries.map((entry) => (
              <div
                key={entry.id}
                onContextMenu={(e) => handleContextMenu(e, entry)}
                className="px-6 py-3 hover:bg-surface-secondary transition-colors"
              >
                <div className="flex items-start gap-3">
                  {/* Status icon */}
                  <div className="flex-shrink-0 mt-0.5">
                    {entry.status === 'success' ? (
                      <CheckCircle size={16} className="text-status-success" />
                    ) : (
                      <XCircle size={16} className="text-status-error" />
                    )}
                  </div>

                  {/* Content */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-content-primary truncate">
                        {getDisplayLabel(entry)}
                      </span>
                      {entry.codec && (
                        <span className="flex-shrink-0 px-1.5 py-0.5 text-[10px] font-medium rounded bg-accent/10 text-accent uppercase">
                          {entry.codec}
                        </span>
                      )}
                    </div>

                    {/* Secondary info */}
                    <div className="flex items-center gap-2 mt-0.5">
                      <span className="text-xs text-content-tertiary">
                        {formatDate(entry.completed_at)}
                      </span>
                      {entry.artist && (
                        <>
                          <span className="text-xs text-content-tertiary">·</span>
                          <span className="text-xs text-content-secondary truncate">
                            {entry.artist}
                          </span>
                        </>
                      )}
                      {entry.album && (
                        <>
                          <span className="text-xs text-content-tertiary">·</span>
                          <span className="text-xs text-content-secondary truncate">
                            {entry.album}
                          </span>
                        </>
                      )}
                    </div>

                    {/* Error message for failed downloads.
                        Wrapped in ErrorMessageDisplay so long messages
                        (especially upstream "GAMDL bug — …" classifier
                        outputs) get a hover tooltip with the full text
                        + a right-click menu to copy or report
                        upstream. The visible text still truncates to
                        2 lines so the row layout doesn't blow out. */}
                    {entry.error_message && (
                      <div className="mt-1">
                        <ErrorMessageDisplay
                          message={entry.error_message}
                          sourceUrl={entry.url}
                          truncateLines={2}
                        />
                      </div>
                    )}

                    {/* URL (shown small below the title). `title` adds a
                        native browser tooltip so long Apple Music URLs
                        — which often run past the visible row width — are
                        readable on hover without resizing the window. */}
                    <p
                      className="text-[11px] text-content-tertiary mt-0.5 truncate"
                      title={entry.url}
                    >
                      {entry.url}
                    </p>
                  </div>

                  {/* Per-row actions: Retry (failed) and Open Folder (has path).
                      Both render inline so the user doesn't have to right-click,
                      but the right-click menu has the same actions for keyboard /
                      power-user workflows (#665). */}
                  <div className="flex items-center gap-1 flex-shrink-0">
                    {isFailed(entry) && (
                      <button
                        type="button"
                        onClick={() => handleRetryOne(entry)}
                        className="p-1 text-content-tertiary hover:text-content-primary rounded-platform hover:bg-surface-tertiary transition-colors"
                        aria-label="Retry download"
                        title="Retry download"
                      >
                        <RotateCcw size={14} />
                      </button>
                    )}
                    {entry.file_path && (
                      <button
                        type="button"
                        onClick={() => handleOpenFolder(entry.file_path!)}
                        className="p-1 text-content-tertiary hover:text-content-primary rounded-platform hover:bg-surface-tertiary transition-colors"
                        aria-label="Open folder"
                        title="Open folder"
                      >
                        <FolderOpen size={14} />
                      </button>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Right-click context menu (#665) */}
      {contextMenu && (
        <ContextMenu
          items={buildContextMenuItems(contextMenu.entry)}
          x={contextMenu.x}
          y={contextMenu.y}
          onClose={() => setContextMenu(null)}
        />
      )}

      {/* Per-item Delete confirmation modal (#685) */}
      <Modal
        open={deleteTarget !== null}
        onClose={() => setDeleteTarget(null)}
        title="Remove history entry"
      >
        <p className="text-sm text-content-secondary mb-4">
          Remove this entry from your download history? Already-downloaded
          files on disk are not deleted — only the history record.
        </p>
        {deleteTarget && (
          <p className="text-xs text-content-tertiary mb-6 break-all">
            {getDisplayLabel(deleteTarget)}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={() => setDeleteTarget(null)}>
            Cancel
          </Button>
          <Button variant="primary" onClick={handleDeleteConfirmed}>
            Remove
          </Button>
        </div>
      </Modal>

      {/* Bulk retry confirmation modal (#665) */}
      <Modal
        open={showRetryAllConfirm}
        onClose={() => setShowRetryAllConfirm(false)}
        title="Retry All Failed Downloads"
      >
        <p className="text-sm text-content-secondary mb-4">
          This will re-enqueue {failedCount} failed download
          {failedCount !== 1 ? 's' : ''} from your history. Duplicate URLs are
          deduplicated automatically — each unique URL is queued once.
        </p>
        <p className="text-sm text-content-secondary mb-6">
          The history entries themselves are kept. Successful retries will write
          new history entries on completion.
        </p>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={() => setShowRetryAllConfirm(false)}>
            Cancel
          </Button>
          <Button variant="primary" onClick={handleRetryAllConfirmed}>
            Retry All
          </Button>
        </div>
      </Modal>
    </div>
  );
}
