// Copyright (c) 2026 MeedyaDL
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
//
// Virtualized rendering via @tanstack/react-virtual to keep DOM node count
// low (~150 nodes) regardless of total entry count, preventing memory bloat.

import { useEffect, useRef, useCallback, useState, useMemo } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

import { useActivityStore } from '@/stores/activityStore';
import { Button, Input } from '@/components/common';
import { PageHeader } from '@/components/layout/PageHeader';
import { StatisticsPanel } from '@/components/download/StatisticsPanel';
import { Download, Trash2, Search, X, Copy, ScrollText, HardDrive, FolderOpen } from 'lucide-react';
import { exportActivityLog, exportDiskActivityLog, getLogsFolderPath } from '@/lib/tauri-commands';
import { useUiStore } from '@/stores/uiStore';

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
 * - Virtualized rendering -- only visible rows are in the DOM
 * - Auto-scrolls to bottom as new lines arrive
 * - Pauses auto-scroll when user scrolls up or clicks Pause
 * - Resumes catching up (no lines lost) on Resume click
 * - Stderr lines highlighted in warning colour
 * - Text is selectable and copyable
 * - Capped at 10,000 entries -- oldest trimmed when exceeded
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

  /*
   * Stable height-measurement callback. Wrapped in `useCallback` so the
   * virtualizer's internal config doesn't thrash its reference identity
   * on every render — rapid log bursts at ~60 flushes/sec (per
   * App.tsx's RAF batching) produce a lot of renders, and a fresh
   * `measureElement` closure on each one was observed to interact
   * with the measurement cache in ways that produce the overlapping-
   * text regression reported in #575.
   */
  const measureElement = useCallback(
    (element: Element | null | undefined) =>
      element?.getBoundingClientRect().height ?? 26,
    [],
  );

  /*
   * Stable per-item key. This is the critical fix for #575: without
   * `getItemKey`, TanStack virtual keys its measurement cache by
   * positional index. When the entry list shifts (10,000-entry
   * trimming cap triggering, filter toggles changing `filteredEntries.length`,
   * RAF-batched bursts prepending new entries), cached row heights
   * attach to the wrong entries and the resulting `translateY()`
   * offsets overlap adjacent rows.
   *
   * Keying by the entry's stable `_id` means a measurement made for
   * entry id=1234 stays attached to entry id=1234 regardless of what
   * position it currently occupies in the filtered list. Entries
   * without an `_id` fall back to the index — that path is only
   * reached if an upstream emitter skips the auto-increment, which
   * shouldn't happen in normal flow.
   *
   * #442 (closed 2026-04-12) was the original fix for the same
   * symptom but only added `measureElement`; it didn't add the
   * stable-key layer, which is why this class of bug regressed
   * under real workloads (200-track box set on external USB, per #575
   * repro). This commit is the belt-and-braces completion of #442's
   * fix.
   */
  const getItemKey = useCallback(
    (index: number) => filteredEntries[index]?._id ?? index,
    [filteredEntries],
  );

  /** Virtualizer for efficient rendering of large entry lists.
   * Uses dynamic height measurement so wrapped multi-line entries
   * don't overlap with subsequent rows. */
  const virtualizer = useVirtualizer({
    count: filteredEntries.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 26, // text-xs + leading-relaxed + py-0.5 + border (single-line estimate)
    overscan: 50, // render 50 extra rows above/below viewport
    measureElement,
    getItemKey,
  });

  /**
   * Auto-scroll to bottom when new filtered entries arrive, unless paused.
   * Uses the virtualizer's scrollToIndex for accurate positioning with
   * dynamic row heights.
   *
   * Pre-fix this also gated on `userScrolledRef.current`, but that
   * created a UX trap reported on 2026-05-11: scrolling up set the ref
   * but NOT `paused`, so the checkbox kept showing "Auto-scroll: ON"
   * while auto-scroll was effectively off. The handleScroll handler
   * below now keeps `paused` in sync with the scroll position so the
   * checkbox reflects reality and `paused` is the single source of
   * truth.
   */
  useEffect(() => {
    if (paused) return;
    if (filteredEntries.length > 0) {
      virtualizer.scrollToIndex(filteredEntries.length - 1, { align: 'end' });
    }
  }, [filteredEntries.length, paused, virtualizer]);

  /**
   * Detect user scroll position and sync the `paused` store flag with
   * it. Scrolling up past the threshold sets `paused = true` (so the
   * Auto-scroll checkbox visibly unchecks); scrolling back to the
   * bottom sets `paused = false` (the existing intended behaviour).
   * `userScrolledRef` is kept for the moment-of-scroll detection but
   * is no longer the gate — the auto-scroll effect now reads `paused`
   * exclusively.
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
      if (!paused) setPaused(true);
    }
  }, [paused, setPaused]);

  /**
   * Resume auto-scroll: scroll to the latest entry and clear the
   * pause flag. Used by both the "Jump to latest" pill button (which
   * appears whenever the user is scrolled away) and the Auto-scroll
   * checkbox toggle.
   */
  const resumeAutoScroll = useCallback(() => {
    userScrolledRef.current = false;
    setPaused(false);
    if (filteredEntries.length > 0) {
      virtualizer.scrollToIndex(filteredEntries.length - 1, { align: 'end' });
    }
  }, [filteredEntries.length, setPaused, virtualizer]);

  /**
   * Export all log entries to a .log file via native save dialog.
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
   * Export the persistent on-disk activity log (#541). Concatenates the
   * most recent daily log files, which captures the complete forensic
   * record from the moment the app started — including entries that
   * were trimmed from the in-memory view when the 10,000 cap was
   * exceeded, and every verbose line regardless of whether the
   * Verbose filter is on.
   */
  const handleExportDisk = async () => {
    const addToast = useUiStore.getState().addToast;
    try {
      const bytes = await exportDiskActivityLog(3);
      const kb = (bytes / 1024).toFixed(1);
      addToast(`Exported on-disk activity log (${kb} KB, last 3 days)`, 'success');
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (!msg.toLowerCase().includes('cancel')) {
        addToast(`Failed to export on-disk log: ${msg}`, 'error');
      }
    }
  };

  /**
   * Open the logs folder in the OS file manager so users can browse,
   * archive, or attach files to bug reports directly. Uses the shell
   * plugin (same pattern as QueueItem's "Open Folder" action).
   */
  const handleRevealLogsFolder = async () => {
    const addToast = useUiStore.getState().addToast;
    try {
      const path = await getLogsFolderPath();
      const { open } = await import('@tauri-apps/plugin-shell');
      await open(path);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      addToast(`Failed to open logs folder: ${msg}`, 'error');
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
          <div className="flex items-center gap-2">
            <label
              className="flex items-center gap-1.5 text-xs text-content-secondary cursor-pointer select-none"
              title={
                paused
                  ? 'Auto-scroll is paused (you scrolled up). Tick to scroll to the latest line and resume.'
                  : 'Auto-scroll is following the latest line. Untick to pause, or scroll up.'
              }
            >
              <input
                type="checkbox"
                checked={!paused}
                onChange={(e) => {
                  if (e.target.checked) {
                    resumeAutoScroll();
                  } else {
                    setPaused(true);
                  }
                }}
                className="accent-accent w-3.5 h-3.5 cursor-pointer"
              />
              Auto-scroll
            </label>
            <Button
              variant="secondary"
              size="sm"
              icon={<Download size={14} />}
              onClick={handleExport}
              disabled={entries.length === 0}
              title="Export the entries currently visible in this view"
            >
              Export
            </Button>
            <Button
              variant="secondary"
              size="sm"
              icon={<HardDrive size={14} />}
              onClick={handleExportDisk}
              title="Export the full on-disk activity log (last 3 days) — includes entries trimmed from the 10,000-line view"
            >
              Export Disk
            </Button>
            <Button
              variant="ghost"
              size="sm"
              icon={<FolderOpen size={14} />}
              onClick={handleRevealLogsFolder}
              title="Open the logs folder in the OS file manager"
            >
              Reveal
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

      {/* Scrollable log container -- virtualized for performance.
          Wrapped in a `relative` flex container so the floating
          "Jump to latest" pill can be absolute-positioned over the
          bottom-right corner when auto-scroll is paused. */}
      <div className="relative flex-1 m-4 mt-0 min-h-0">
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="h-full overflow-y-auto bg-surface-secondary rounded-platform p-3 font-mono text-xs leading-relaxed select-text"
        role="log"
        aria-live="polite"
        aria-label="Activity log"
      >
        {entries.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-12 text-content-tertiary">
            <ScrollText size={32} className="mb-3 opacity-40" />
            <p className="text-sm font-medium">No activity yet</p>
            <p className="text-xs mt-1">Start a download to see live output here. The log resets on app restart.</p>
          </div>
        ) : filteredEntries.length === 0 ? (
          <p className="text-content-tertiary text-center py-8">
            No entries match the current search or filter criteria.
          </p>
        ) : (
          <div
            style={{
              height: `${virtualizer.getTotalSize()}px`,
              width: '100%',
              position: 'relative',
            }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const entry = filteredEntries[virtualRow.index];
              return (
                <div
                  key={entry._id ?? virtualRow.index}
                  ref={virtualizer.measureElement}
                  data-index={virtualRow.index}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                  className={`group relative whitespace-pre-wrap break-words pr-6 py-0.5 px-1 font-mono text-xs leading-relaxed border-b border-border/20 ${
                    virtualRow.index % 2 === 0 ? '' : 'bg-surface-primary/30'
                  } ${
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
                  <button
                    type="button"
                    className="absolute right-0 top-0 opacity-0 group-hover:opacity-60 hover:!opacity-100 transition-opacity p-0.5"
                    title="Copy to clipboard"
                    aria-label="Copy log entry"
                    onClick={() => {
                      navigator.clipboard.writeText(entry.line);
                    }}
                  >
                    <Copy size={10} />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

        {/* Floating "Jump to latest" pill — visible only when the user
            has scrolled away from the bottom (auto-scroll paused).
            Gives a one-click resume that's more discoverable than
            unticking and re-ticking the Auto-scroll checkbox. */}
        {paused && entries.length > 0 && (
          <button
            type="button"
            onClick={resumeAutoScroll}
            className="absolute bottom-3 right-3 z-10 flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-accent text-content-inverse text-xs font-medium shadow-md hover:bg-accent-hover transition-colors cursor-pointer"
            title="Scroll to the latest line and resume auto-scroll"
            aria-label="Jump to latest activity log line and resume auto-scroll"
          >
            ↓ Jump to latest
          </button>
        )}
      </div>
    </div>
  );
}
