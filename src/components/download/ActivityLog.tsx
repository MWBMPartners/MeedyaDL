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

import { useEffect, useRef, useCallback } from 'react';

import { useActivityStore } from '@/stores/activityStore';
import { Button } from '@/components/common';
import { PageHeader } from '@/components/layout/PageHeader';
import { Download, Pause, Play, Trash2 } from 'lucide-react';
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
 * ActivityLog -- Live terminal-style view of all application activity.
 *
 * Shows two kinds of events:
 * - **Download events** — GAMDL subprocess stdout/stderr and per-download
 *   internal messages (enrichment, companions, fallback decisions).
 *   Identified by a truncated download ID prefix, e.g. `[abc123de]`.
 * - **System events** — App-wide actions (update checks, dependency installs,
 *   settings saves, queue operations, startup). Identified by `download_id`
 *   === `"system"` and rendered with a `[System]` badge.
 *
 * Features:
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

  /** Ref to the scrollable container for auto-scroll management. */
  const scrollRef = useRef<HTMLDivElement>(null);

  /** Whether the user has manually scrolled up (auto-pause detection). */
  const userScrolledRef = useRef(false);

  /**
   * Auto-scroll to bottom when new entries arrive, unless paused.
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
  }, [entries.length, paused]);

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
   * Silently catches cancellation (user closed the dialog).
   */
  const handleExport = async () => {
    try {
      await exportActivityLog(entries);
    } catch {
      // User cancelled or error — no action needed
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

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="Activity Log"
        subtitle={`${entries.length} line${entries.length !== 1 ? 's' : ''}${paused ? ' (paused)' : ''}`}
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
        ) : (
          entries.map((entry, i) => (
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
