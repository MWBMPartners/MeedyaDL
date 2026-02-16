// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Zustand store for the Activity Log page. Accumulates raw subprocess
// output lines emitted by the Rust backend via the "activity-log" Tauri
// event. Entries are capped at MAX_ENTRIES to prevent unbounded memory
// growth during long download sessions.

import { create } from 'zustand';
import type { ActivityLogEntry } from '@/types';

/** Maximum number of log entries retained in memory. */
const MAX_ENTRIES = 5000;

interface ActivityState {
  /** All log entries, newest last. */
  entries: ActivityLogEntry[];
  /** Whether auto-scrolling is paused (user scrolled up or clicked Pause). */
  paused: boolean;
  /** Append a new log entry; trims oldest entries if over MAX_ENTRIES. */
  addEntry: (entry: ActivityLogEntry) => void;
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
      const next = [...state.entries, entry];
      if (next.length > MAX_ENTRIES) {
        return { entries: next.slice(next.length - MAX_ENTRIES) };
      }
      return { entries: next };
    }),

  setPaused: (paused) => set({ paused }),

  clearEntries: () => set({ entries: [] }),
}));
