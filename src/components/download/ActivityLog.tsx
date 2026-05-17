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

import type { ActivityLogEntry } from '@/types';
import { useActivityStore } from '@/stores/activityStore';
import { Button, Input } from '@/components/common';
import { PageHeader } from '@/components/layout/PageHeader';
import { StatisticsPanel } from '@/components/download/StatisticsPanel';
import { Download, Trash2, Search, X, Copy, ScrollText, HardDrive, FolderOpen } from 'lucide-react';
import { exportActivityLog, exportDiskActivityLog, getLogsFolderPath } from '@/lib/tauri-commands';
import { useUiStore } from '@/stores/uiStore';
import { useLocalWallClock } from '@/hooks/useLocalWallClock';

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
 * Picks the Tailwind text-colour class for an activity-log entry.
 *
 * Severity wins (#793): an entry explicitly tagged
 * `error` / `warning` always uses the matching status token —
 * regardless of which stream produced it — so the user can scan
 * the log for red/amber to find the actual problems.
 *
 * When severity is `info` or unset (the common case + the
 * backwards-compat fallback for older Rust builds), fall through to
 * the pre-#793 stream-based colour scheme so existing
 * informational lines stay visually consistent:
 *   - `internal`: accent (MeedyaDL-emitted messages)
 *   - `stderr`: amber (subprocess stderr — most Python tooling
 *     routes warnings there even without a level prefix)
 *   - `stdout`: default content colour
 *
 * All four candidate classes (`text-status-error`,
 * `text-status-warning`, `text-status-info`, `text-content-primary`)
 * are defined in the project's design-token CSS and adapt across
 * light / dark / high-contrast / colour-blind themes — so the
 * coloured rendering stays readable in every accessibility mode
 * without per-theme overrides here.
 */
function textColourForEntry(entry: ActivityLogEntry): string {
  if (entry.severity === 'error') return 'text-status-error';
  if (entry.severity === 'warning') return 'text-status-warning';
  if (entry.stream === 'internal') return 'text-accent-primary';
  if (entry.stream === 'stderr') return 'text-status-warning';
  return 'text-content-primary';
}

/**
 * Returns the local-time calendar date of an ISO-8601 timestamp as
 * `YYYY-MM-DD`. Used by the cross-day banner logic to decide when two
 * consecutive entries straddle a date boundary in the user's local TZ.
 *
 * Returns the empty string on parse failure — the banner generator
 * treats that as "same as previous" and skips banner insertion so a
 * malformed timestamp can never produce a stray banner row.
 */
function localDateKey(iso: string): string {
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return '';
    // toLocaleDateString('en-CA') yields YYYY-MM-DD reliably across
    // browsers, which is what we want for stable date comparison.
    return d.toLocaleDateString('en-CA');
  } catch {
    return '';
  }
}

/**
 * Discriminated-union row item rendered by the activity-log virtualiser.
 *
 * The virtualiser used to iterate `filteredEntries` directly; #801
 * interleaves cross-day banner rows between entries that fall on
 * different local-time dates. The banners are UI-only: they don't
 * count toward the line counter, don't appear in any export, and
 * carry only the date string they're labelling.
 */
type DisplayItem =
  | { kind: 'entry'; entry: ActivityLogEntry; key: string | number }
  | { kind: 'banner'; date: string; key: string };

/**
 * Maps `filteredEntries` into the virtualiser's input array, inserting
 * a `─── YYYY-MM-DD ───` banner row whenever two consecutive entries
 * straddle a local-date boundary.
 *
 * Single-pass; banner-row keys are namespaced (`banner:DATE`) so they
 * never collide with entry `_id`s. No banner is inserted before the
 * first entry — the user already knows what day they're looking at
 * when the log loads; the banner is specifically for the
 * "this entry happened on a new day" callout.
 */
function buildDisplayItems(entries: ActivityLogEntry[]): DisplayItem[] {
  const items: DisplayItem[] = [];
  let lastDate = '';
  for (let i = 0; i < entries.length; i++) {
    const entry = entries[i];
    const date = localDateKey(entry.timestamp);
    // Insert a banner whenever the calendar date advances. Skip the
    // first iteration (lastDate === '' triggers the banner only on
    // genuine date changes, never at i === 0).
    if (date && lastDate && date !== lastDate) {
      items.push({ kind: 'banner', date, key: `banner:${date}` });
    }
    items.push({ kind: 'entry', entry, key: entry._id ?? `entry:${i}` });
    if (date) lastDate = date;
  }
  return items;
}

/**
 * Live local-system wall clock rendered as a small chip in the page
 * subtitle. Lives in its own component so the 1 Hz re-render is
 * isolated to one `<span>` rather than the whole `ActivityLog` tree.
 *
 * Format matches the per-entry `formatTime()` timestamps below
 * (24-hour HH:MM:SS), so the user can compare visually at a glance.
 */
function WallClockChip(): React.JSX.Element {
  const now = useLocalWallClock();
  return (
    <span
      className="font-mono tabular-nums text-content-secondary"
      title="Local system time (updates every second)"
      aria-label={`Local time ${now}`}
    >
      {now}
    </span>
  );
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
  /*
   * Incremental filter (#691) — O(new-entries) per tick rather than
   * O(all-entries). The Activity Log store guarantees two invariants
   * that make this safe:
   *   1. Entries are append-only at the back (newest last).
   *   2. Trimming (when the 10 000-entry cap is exceeded) removes
   *      ONLY from the front (oldest first).
   * Combined with the monotonically-increasing `_id` field, the cache
   * needs only:
   *   - prepend trim: drop entries whose `_id` < `entries[0]._id`
   *   - tail append: filter the slice strictly after the cached
   *     `lastSeenId` and concatenate.
   * The predicate key (search query + category toggles) is a stable
   * string; when it changes, we fall back to a full re-filter and
   * reseed the cache. That happens rarely — typing in the search box,
   * toggling a filter chip — and the incremental path covers the
   * common case of "60 RAF-batched entries/sec arriving from the
   * download pipeline".
   *
   * Pre-#691 implementation was `entries.filter(predicate)` inside
   * `useMemo`. On a heavy 50-item batch with verbose logging, the
   * old code did 60 events/sec × 10 000 retained entries = 600 000
   * predicate evaluations per second, dominating the React render
   * budget. The incremental path does 60 events/sec × ~tail-size
   * (often 1–10) = ~600/sec, three orders of magnitude less work.
   */
  const predicateKey = useMemo(
    () => `${searchQuery.trim().toLowerCase()}|${showSystem ? 1 : 0}|${showDownload ? 1 : 0}|${showVerbose ? 1 : 0}`,
    [searchQuery, showSystem, showDownload, showVerbose],
  );

  // Cache spans renders without triggering them. Stored as a ref so
  // we can mutate it from inside the memo (the memo runs once per
  // render and the ref isn't exposed externally, so this is safe).
  const filterCacheRef = useRef<{
    predicateKey: string;
    filtered: ActivityLogEntry[];
    lastSeenId: number;
    firstStoreId: number;
  }>({ predicateKey: '', filtered: [], lastSeenId: -1, firstStoreId: -1 });

  const filteredEntries = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();

    // Inline predicate — kept as a closure rather than a top-level
    // function so the search query + category toggles close over by
    // reference. Hot path: called once per *new* entry per tick,
    // never the full 10 000.
    const predicate = (entry: ActivityLogEntry): boolean => {
      if (query && !entry.line.toLowerCase().includes(query)) return false;
      const { isSystem, isDownload, isVerbose } = getEntryCategories(entry);
      if (isVerbose) {
        if (showVerbose) return true;
        if (isSystem && showSystem) return true;
        if (isDownload && showDownload) return true;
        return false;
      }
      if (isSystem && showSystem) return true;
      if (isDownload && showDownload) return true;
      return false;
    };

    const cache = filterCacheRef.current;
    const last = entries.length > 0 ? entries[entries.length - 1] : null;
    const first = entries.length > 0 ? entries[0] : null;
    const lastId = last?._id ?? -1;
    const firstStoreId = first?._id ?? -1;

    // Branch A — predicate changed (or first render). Full re-filter
    // and reseed the cache. Rare: only when the user edits the
    // search box or toggles a category chip.
    if (cache.predicateKey !== predicateKey) {
      const filtered = entries.filter(predicate);
      filterCacheRef.current = {
        predicateKey,
        filtered,
        lastSeenId: lastId,
        firstStoreId,
      };
      return filtered;
    }

    // Branch B — same predicate. Apply the two structural deltas
    // (front-trim, tail-append) against the cached array. Both are
    // O(1) detection + O(delta) work.
    let next = cache.filtered;

    // B1: front-trim. The store removes the oldest 10% when the
    // 10 000-entry cap is exceeded. Drop any cached entries whose
    // `_id` predates the new `entries[0]._id`. `_id` is monotonic
    // so we can stop as soon as we hit an in-range one.
    if (firstStoreId > cache.firstStoreId && firstStoreId !== -1) {
      // Binary search would be O(log n), but the trim batch is
      // ~1 000 entries (10% of 10 000) which is a one-shot delta;
      // a linear scan with early-return is simpler and not measurably
      // slower in practice.
      let dropCount = 0;
      while (dropCount < next.length && (next[dropCount]._id ?? -1) < firstStoreId) {
        dropCount++;
      }
      if (dropCount > 0) {
        next = next.slice(dropCount);
      }
    }

    // B2: tail-append. Filter only entries strictly after the cached
    // `lastSeenId`. `_id` is monotonic so a from-the-end scan finds
    // the boundary in O(new-entries) without scanning the whole array.
    if (lastId > cache.lastSeenId) {
      // Find the index of the first new entry. From-end is the right
      // direction because we expect the delta to be small (handful
      // of entries) and clustered at the tail.
      let firstNewIdx = entries.length;
      while (firstNewIdx > 0 && (entries[firstNewIdx - 1]._id ?? -1) > cache.lastSeenId) {
        firstNewIdx--;
      }
      if (firstNewIdx < entries.length) {
        const tail = entries.slice(firstNewIdx).filter(predicate);
        if (tail.length > 0) {
          next = next === cache.filtered ? [...next, ...tail] : [...next, ...tail];
        }
      }
    }

    // Commit the new cache snapshot for the next render. Only mutate
    // if anything actually changed — keeps the React DevTools
    // "renders" panel readable.
    if (
      next !== cache.filtered ||
      lastId !== cache.lastSeenId ||
      firstStoreId !== cache.firstStoreId
    ) {
      filterCacheRef.current = {
        predicateKey,
        filtered: next,
        lastSeenId: lastId,
        firstStoreId,
      };
    }
    return next;
  }, [entries, predicateKey, searchQuery, showSystem, showDownload, showVerbose]);

  /*
   * Cross-day banner injection (#801): wrap `filteredEntries` in a
   * discriminated-union array (`DisplayItem[]`) where every entry-row is
   * interleaved with a `─── YYYY-MM-DD ───` banner row whenever two
   * consecutive entries straddle a local-date boundary. The virtualiser
   * iterates this combined list so banners count as virtualised rows and
   * scroll/measure consistently with the existing entry rows.
   *
   * Banner rows are pure UI — they aren't real entries, don't appear in
   * any export, and don't count toward the user-facing line counter
   * shown in the page subtitle (which still uses `filteredEntries.length`).
   */
  const displayItems = useMemo(() => buildDisplayItems(filteredEntries), [filteredEntries]);

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
    (index: number) => displayItems[index]?.key ?? index,
    [displayItems],
  );

  /** Virtualizer for efficient rendering of large entry lists.
   * Uses dynamic height measurement so wrapped multi-line entries
   * don't overlap with subsequent rows. Iterates `displayItems` (entries
   * + cross-day banners, #801) rather than `filteredEntries` directly. */
  const virtualizer = useVirtualizer({
    count: displayItems.length,
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
    if (displayItems.length > 0) {
      virtualizer.scrollToIndex(displayItems.length - 1, { align: 'end' });
    }
  }, [displayItems.length, paused, virtualizer]);

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
    if (displayItems.length > 0) {
      virtualizer.scrollToIndex(displayItems.length - 1, { align: 'end' });
    }
  }, [displayItems.length, setPaused, virtualizer]);

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
        subtitle={
          <span className="text-sm text-content-secondary flex items-center gap-2">
            <span>{subtitle}</span>
            <span aria-hidden="true">·</span>
            <WallClockChip />
          </span>
        }
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
              const item = displayItems[virtualRow.index];

              // Cross-day banner row (#801) — purely visual marker
              // between two entries that fall on different local
              // calendar dates. Not a real log entry; not exported.
              if (item.kind === 'banner') {
                return (
                  <div
                    key={item.key}
                    ref={virtualizer.measureElement}
                    data-index={virtualRow.index}
                    data-banner-date={item.date}
                    style={{
                      position: 'absolute',
                      top: 0,
                      left: 0,
                      width: '100%',
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                    className="flex items-center gap-2 py-1 px-1 font-mono text-[11px] text-content-tertiary select-none"
                    role="separator"
                    aria-label={`Date changed to ${item.date}`}
                  >
                    <span className="flex-1 border-t border-border/40" aria-hidden="true" />
                    <span className="tabular-nums tracking-wide">{item.date}</span>
                    <span className="flex-1 border-t border-border/40" aria-hidden="true" />
                  </div>
                );
              }

              const entry = item.entry;
              return (
                <div
                  key={item.key}
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
                  } ${textColourForEntry(entry)}`}
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
