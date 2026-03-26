// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Zustand store for the Activity Log page. Accumulates raw subprocess
// output lines emitted by the Rust backend via the "activity-log" Tauri
// event. No entry cap — the log grows unbounded within a session and
// resets on app restart.

import { create } from 'zustand';
import type { ActivityLogEntry } from '@/types';

interface ActivityState {
  /** All log entries, newest last. */
  entries: ActivityLogEntry[];
  /** Whether auto-scrolling is paused (user scrolled up or clicked Pause). */
  paused: boolean;
  /** Append a new log entry. */
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
    set((state) => ({ entries: [...state.entries, entry] })),

  setPaused: (paused) => set({ paused }),

  clearEntries: () => set({ entries: [] }),
}));
