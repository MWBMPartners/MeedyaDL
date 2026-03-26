// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Download History page component.
// Displays a persistent record of all completed and failed downloads.
// Each entry shows the date, URL/title, codec badge, and status icon.
// A search input filters entries by title, artist, album, or URL.
// The "Clear History" button deletes all entries from disk.

import { useEffect, useState, useCallback } from 'react';

import { PageHeader } from '@/components/layout/PageHeader';
import { Button } from '@/components/common';
import { Trash2, Search, CheckCircle, XCircle, X, FolderOpen, Clock } from 'lucide-react';
import { listHistory, clearHistory, searchHistory } from '@/lib/tauri-commands';
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
 * Renders the download history page with search, entry list, and clear action.
 */
export function HistoryPage() {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);
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

  const subtitle = searchQuery
    ? `${entries.length} result${entries.length !== 1 ? 's' : ''}`
    : `${entries.length} download${entries.length !== 1 ? 's' : ''}`;

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="History"
        subtitle={subtitle}
        actions={
          entries.length > 0 && !searchQuery ? (
            <Button variant="ghost" size="sm" onClick={handleClear}>
              <Trash2 size={14} className="mr-1.5" />
              Clear History
            </Button>
          ) : undefined
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

                    {/* Error message for failed downloads */}
                    {entry.error_message && (
                      <p className="text-xs text-status-error mt-1 line-clamp-2">
                        {entry.error_message}
                      </p>
                    )}

                    {/* URL (shown small below the title) */}
                    <p className="text-[11px] text-content-tertiary mt-0.5 truncate">
                      {entry.url}
                    </p>
                  </div>

                  {/* Open folder action */}
                  {entry.file_path && entry.status === 'success' && (
                    <button
                      type="button"
                      onClick={() => handleOpenFolder(entry.file_path!)}
                      className="flex-shrink-0 p-1 text-content-tertiary hover:text-content-primary rounded-platform hover:bg-surface-tertiary transition-colors"
                      aria-label="Open folder"
                      title="Open folder"
                    >
                      <FolderOpen size={14} />
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
