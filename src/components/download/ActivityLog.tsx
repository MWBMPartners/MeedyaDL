// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Activity Log page component.
// Displays a live, terminal-style view of all application activity.
// Shows both GAMDL subprocess output (download-specific) and system-level
// events (updates, dependency installs, settings, queue operations).
// Each line arrives in real-time from the backend via the "activity-log"
// Tauri event. The log auto-scrolls to the bottom unless the user scrolls
// up or clicks "Pause".
//
// Text is fully selectable and copyable for bug reporting purposes.
//
// The StatisticsPanel is rendered at the top of the page (before the log
// entries) to give a quick overview of session download activity.
//
// Search and filter toolbar:
// - A search input filters entries by case-insensitive substring match on
//   the message text.
// - Three category filter toggles (System, Download, Verbose) control which
//   entry types are visible. System entries have download_id === "system",
//   Download entries have download_id !== "system", and Verbose entries
//   contain "[VERBOSE]" in the message text.
// - Filters are combined: an entry must pass BOTH the search query AND have
//   at least one of its matching categories enabled.
// - The filtered count is shown in the subtitle alongside the total count.

import { useEffect, useRef, useCallback, useState, useMemo } from 'react';

import { useActivityStore } from '@/stores/activityStore';
import { Button, Input } from '@/components/common';
import { PageHeader } from '@/components/layout/PageHeader';
import { StatisticsPanel } from '@/components/download/StatisticsPanel';
import { Download, Pause, Play, Trash2, Search, X } from 'lucide-react';
import { exportActivityLog } from '@/lib/tauri-commands';

/**
 * Formats an ISO 8601 timestamp to a short HH:MM:SS format.
 */
function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString('en-GB', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  } catch {
    return '';
  }
}

/**
 * Truncates a download ID to its first 8 characters for compact display.
 */
function shortId(id: string): string {
  return id.length > 8 ? id.slice(0, 8) : id;
}

/**
 * Determines the category of an activity log entry.
 * - "system": app-wide events (download_id === "system")
 * - "verbose": entries containing the [VERBOSE] tag in the message line
 * - "download": per-download events (everything else)
 *
 * Note: a verbose entry may also be a system or download entry. The category
 * check is non-exclusive -- an entry can match multiple categories. The
 * filter logic treats an entry as visible if ANY of its matching categories
 * are enabled.
 */
function getEntryCategories(entry: { download_id: string; line: string }): {
  isSystem: boolean;
  isDownload: boolean;
  isVerbose: boolean;
} {
  const isVerbose = entry.line.includes('[VERBOSE]');
  const isSystem = entry.download_id === 'system';
  const isDownload = entry.download_id !== 'system';
  return { isSystem, isDownload, isVerbose };
}

/**
 * ActivityLog -- Live terminal-style view of all application activity.
 *
 * Shows two kinds of events:
 * - **Download events** -- GAMDL subprocess stdout/stderr and per-download
 *   internal messages (enrichment, companions, fallback decisions).
 *   Identified by a truncated download ID prefix, e.g. `[abc123de]`.
 * - **System events** -- App-wide actions (update checks, dependency installs,
 *   settings saves, queue operations, startup). Identified by `download_id`
 *   === `"system"` and rendered with a `[System]` badge.
 *
 * Features:
 * - Search bar for case-insensitive text filtering on message content
 * - Category filter toggles: System, Download, Verbose
 * - Auto-scrolls to bottom as new lines arrive
 * - Pauses auto-scroll when user scrolls up or clicks Pause
 * - Resumes catching up (no lines lost) on Resume click
 * - Stderr lines highlighted in warning colour
 * - Text is selectable and copyable
 * - Capped at 5000 entries (oldest trimmed) via activityStore
 */
export function ActivityLog() {
  const entries = useActivityStore((s) => s.entries);
  const paused = useActivityStore((s) => s.paused);
  const setPaused = useActivityStore((s) => s.setPaused);
  const clearEntries = useActivityStore((s) => s.clearEntries);

  /** Search query for filtering log entries by message text. */
  const [searchQuery, setSearchQuery] = useState('');

  /** Category filter toggles. All enabled by default except Verbose. */
  const [showSystem, setShowSystem] = useState(true);
  const [showDownload, setShowDownload] = useState(true);
  const [showVerbose, setShowVerbose] = useState(false);

  /** Ref to the scrollable container for auto-scroll management. */
  const scrollRef = useRef<HTMLDivElement>(null);

  /** Whether the user has manually scrolled up (auto-pause detection). */
  const userScrolledRef = useRef(false);

  /**
   * Derive filtered entries based on search query and category filters.
   * An entry is included if:
   *   1. Its `line` contains the search query (case-insensitive), AND
   *   2. At least one of the entry's matching categories is enabled.
   *
   * Category logic (non-exclusive):
   * - Verbose entries (line contains "[VERBOSE]") are shown only when the
   *   Verbose filter is on.
   * - Non-verbose system entries are shown when System is on.
   * - Non-verbose download entries are shown when Download is on.
   * - A verbose system entry is shown when Verbose OR System is on.
   * - A verbose download entry is shown when Verbose OR Download is on.
   */
  const filteredEntries = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();

    return entries.filter((entry) => {
      // Step 1: Search query filter -- case-insensitive substring match on message text
      if (query && !entry.line.toLowerCase().includes(query)) {
        return false;
      }

      // Step 2: Category filter -- entry must have at least one enabled category
      const { isSystem, isDownload, isVerbose } = getEntryCategories(entry);

      // If the entry is verbose, it passes if the verbose toggle is on
      // OR if its base category (system/download) toggle is on.
      if (isVerbose) {
        if (showVerbose) return true;
        if (isSystem && showSystem) return true;
        if (isDownload && showDownload) return true;
        return false;
      }

      // Non-verbose entries: check their base category
      if (isSystem && showSystem) return true;
      if (isDownload && showDownload) return true;

      return false;
    });
  }, [entries, searchQuery, showSystem, showDownload, showVerbose]);

  /**
   * Auto-scroll to bottom when new filtered entries arrive, unless paused.
   * Uses requestAnimationFrame for smooth scroll behaviour.
   */
  useEffect(() => {
    if (paused || userScrolledRef.current) return;
    const el = scrollRef.current;
    if (el) {
      requestAnimationFrame(() => {
        el.scrollTop = el.scrollHeight;
      });
    }
  }, [filteredEntries.length, paused]);

  /**
   * Detect user scroll-up to auto-pause. If the user scrolls away from
   * the bottom (more than 50px threshold), we pause auto-scroll. If they
   * scroll back to the bottom, we resume.
   */
  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const threshold = 50;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < threshold;
    if (atBottom) {
      userScrolledRef.current = false;
      if (paused) setPaused(false);
    } else {
      userScrolledRef.current = true;
    }
  }, [paused, setPaused]);

  /**
   * Export all log entries to a .txt file via native save dialog.
   * Exports the full unfiltered entries, not just the filtered view.
   * Silently catches cancellation (user closed the dialog).
   */
  const handleExport = async () => {
    try {
      await exportActivityLog(entries);
    } catch {
      // User cancelled or error -- no action needed
    }
  };

  /**
   * Toggle pause/resume. When resuming, reset the scroll flag so
   * auto-scroll kicks in on the next entry.
   */
  const handleTogglePause = () => {
    if (paused) {
      userScrolledRef.current = false;
      setPaused(false);
      // Immediately scroll to bottom on resume
      const el = scrollRef.current;
      if (el) {
        requestAnimationFrame(() => {
          el.scrollTop = el.scrollHeight;
        });
      }
    } else {
      setPaused(true);
    }
  };

  /**
   * Handle search input changes.
   */
  const handleSearchChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setSearchQuery(e.target.value);
  }, []);

  /**
   * Clear the search query.
   */
  const handleClearSearch = useCallback(() => {
    setSearchQuery('');
  }, []);

  /** Whether any filters are actively reducing the entry count. */
  const isFiltered = searchQuery.trim() !== '' || !showSystem || !showDownload || showVerbose;

  /** Build the subtitle string showing entry counts and filter/pause state. */
  const subtitle = isFiltered
    ? `${filteredEntries.length} of ${entries.length} line${entries.length !== 1 ? 's' : ''} (filtered)${paused ? ' (paused)' : ''}`
    : `${entries.length} line${entries.length !== 1 ? 's' : ''}${paused ? ' (paused)' : ''}`;

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Activity Log"
        subtitle={subtitle}
        actions={
          <div className="flex gap-2">
            <Button
              variant="secondary"
              size="sm"
              icon={paused ? <Play size={14} /> : <Pause size={14} />}
              onClick={handleTogglePause}
            >
              {paused ? 'Resume' : 'Pause'}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              icon={<Download size={14} />}
              onClick={handleExport}
              disabled={entries.length === 0}
            >
              Export
            </Button>
            <Button
              variant="ghost"
              size="sm"
              icon={<Trash2 size={14} />}
              onClick={clearEntries}
              disabled={entries.length === 0}
            >
              Clear
            </Button>
          </div>
        }
      />

      {/* Session statistics panel (collapsible, hidden when queue is empty) */}
      <StatisticsPanel />

      {/* Search and filter toolbar */}
      <div className="mx-4 mb-2 space-y-2">
        {/* Search input */}
        <Input
          placeholder="Search activity log..."
          value={searchQuery}
          onChange={handleSearchChange}
          icon={<Search size={16} />}
          suffix={
            searchQuery ? (
              <button
                onClick={handleClearSearch}
                className="text-content-tertiary hover:text-content-primary transition-colors cursor-pointer"
                aria-label="Clear search"
              >
                <X size={14} />
              </button>
            ) : undefined
          }
          aria-label="Search activity log"
        />

        {/* Category filter toggles */}
        <div className="flex gap-2">
          <button
            onClick={() => setShowSystem(!showSystem)}
            className={`
              px-2.5 py-1 text-xs font-medium rounded-platform border transition-colors cursor-pointer
              ${showSystem
                ? 'bg-status-info/15 text-status-info border-status-info/30'
                : 'bg-transparent text-content-tertiary border-border hover:text-content-secondary'}
            `}
            role="checkbox"
            aria-checked={showSystem ? 'true' : 'false'}
            aria-label="Filter system entries"
          >
            System
          </button>
          <button
            onClick={() => setShowDownload(!showDownload)}
            className={`
              px-2.5 py-1 text-xs font-medium rounded-platform border transition-colors cursor-pointer
              ${showDownload
                ? 'bg-accent/15 text-accent border-accent/30'
                : 'bg-transparent text-content-tertiary border-border hover:text-content-secondary'}
            `}
            role="checkbox"
            aria-checked={showDownload ? 'true' : 'false'}
            aria-label="Filter download entries"
          >
            Download
          </button>
          <button
            onClick={() => setShowVerbose(!showVerbose)}
            className={`
              px-2.5 py-1 text-xs font-medium rounded-platform border transition-colors cursor-pointer
              ${showVerbose
                ? 'bg-status-warning/15 text-status-warning border-status-warning/30'
                : 'bg-transparent text-content-tertiary border-border hover:text-content-secondary'}
            `}
            role="checkbox"
            aria-checked={showVerbose ? 'true' : 'false'}
            aria-label="Filter verbose entries"
          >
            Verbose
          </button>
        </div>
      </div>

      {/* Scrollable log container */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto bg-surface-secondary rounded-platform m-4 mt-0 p-3 font-mono text-xs leading-relaxed select-text"
        role="log"
        aria-live="polite"
        aria-label="Activity log"
      >
        {entries.length === 0 ? (
          <p className="text-content-tertiary text-center py-8">
            No activity yet. Start a download or perform an action to see live output here.
          </p>
        ) : filteredEntries.length === 0 ? (
          <p className="text-content-tertiary text-center py-8">
            No entries match the current search or filter criteria.
          </p>
        ) : (
          filteredEntries.map((entry, i) => (
            <div
              key={i}
              className={`whitespace-pre-wrap break-all ${
                entry.stream === 'internal'
                  ? 'text-accent-primary'
                  : entry.stream === 'stderr'
                    ? 'text-status-warning'
                    : 'text-content-primary'
              }`}
            >
              <span className="text-content-tertiary">{formatTime(entry.timestamp)} </span>
              {entry.download_id === 'system' ? (
                <span className="text-status-info font-medium">[System] </span>
              ) : (
                <>
                  <span className="text-accent">[{shortId(entry.download_id)}] </span>
                  {entry.stream === 'internal' && (
                    <span className="text-accent-primary font-medium">[MeedyaDL] </span>
                  )}
                </>
              )}
              {entry.line}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
