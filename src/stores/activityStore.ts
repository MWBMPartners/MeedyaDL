// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Zustand store for the Activity Log page. Accumulates raw subprocess
// output lines emitted by the Rust backend via the "activity-log" Tauri
// event. Capped at MAX_ENTRIES to prevent unbounded memory growth during
// long download sessions. When the cap is reached, the oldest 10% of
// entries are trimmed (amortised cost avoids per-add trimming).

import { create } from 'zustand';
import type { ActivityLogEntry } from '@/types';

/**
 * Maximum number of activity log entries retained in memory.
 * When exceeded, the oldest 10% are trimmed in a single operation.
 */
const MAX_ENTRIES = 10_000;

/** Auto-incrementing counter for stable React keys. */
let _nextId = 0;

/** Assign a stable `_id` to an entry. */
function tagEntry(entry: ActivityLogEntry): ActivityLogEntry {
  return { ...entry, _id: _nextId++ };
}

interface ActivityState {
  /** All log entries, newest last. */
  entries: ActivityLogEntry[];
  /** Whether auto-scrolling is paused (user scrolled up or clicked Pause). */
  paused: boolean;
  /** Append a single log entry. */
  addEntry: (entry: ActivityLogEntry) => void;
  /** Append a batch of log entries in a single state update. */
  addEntries: (entries: ActivityLogEntry[]) => void;
  /** Set the paused state (controls auto-scroll in the Activity Log view). */
  setPaused: (paused: boolean) => void;
  /** Clear all accumulated entries. */
  clearEntries: () => void;
}

export const useActivityStore = create<ActivityState>((set) => ({
  entries: [],
  paused: false,

  addEntry: (entry) =>
    set((state) => {
      const tagged = tagEntry(entry);
      if (state.entries.length >= MAX_ENTRIES) {
        // Trim oldest 10% to amortise trimming cost
        const trimmed = state.entries.slice(Math.floor(MAX_ENTRIES * 0.1));
        trimmed.push(tagged);
        return { entries: trimmed };
      }
      return { entries: [...state.entries, tagged] };
    }),

  addEntries: (newEntries) =>
    set((state) => {
      if (newEntries.length === 0) return state;
      const tagged = newEntries.map(tagEntry);
      const all = [...state.entries, ...tagged];
      if (all.length > MAX_ENTRIES) {
        // Trim oldest entries to stay at cap
        return { entries: all.slice(all.length - MAX_ENTRIES) };
      }
      return { entries: all };
    }),

  setPaused: (paused) => set({ paused }),

  clearEntries: () => set({ entries: [] }),
}));

/** Exported for testing. */
export { MAX_ENTRIES };
