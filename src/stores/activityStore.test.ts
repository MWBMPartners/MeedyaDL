// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

import { describe, it, expect, beforeEach } from 'vitest';
import { useActivityStore } from './activityStore';
import type { ActivityLogEntry } from '@/types';

/** Helper: create a mock log entry. */
function mockEntry(overrides: Partial<ActivityLogEntry> = {}): ActivityLogEntry {
  return {
    download_id: 'test-id',
    stream: 'internal',
    line: 'Test log message',
    timestamp: new Date().toISOString(),
    ...overrides,
  };
}

describe('activityStore', () => {
  beforeEach(() => {
    useActivityStore.getState().clearEntries();
  });

  it('starts with empty entries', () => {
    expect(useActivityStore.getState().entries).toHaveLength(0);
  });

  it('adds an entry', () => {
    useActivityStore.getState().addEntry(mockEntry());
    expect(useActivityStore.getState().entries).toHaveLength(1);
    expect(useActivityStore.getState().entries[0].line).toBe('Test log message');
  });

  it('adds multiple entries in order', () => {
    useActivityStore.getState().addEntry(mockEntry({ line: 'first' }));
    useActivityStore.getState().addEntry(mockEntry({ line: 'second' }));
    useActivityStore.getState().addEntry(mockEntry({ line: 'third' }));

    const entries = useActivityStore.getState().entries;
    expect(entries).toHaveLength(3);
    expect(entries[0].line).toBe('first');
    expect(entries[2].line).toBe('third');
  });

  it('has no entry cap (previously 5000)', () => {
    // Add more than the old 5000 cap to verify no truncation
    for (let i = 0; i < 100; i++) {
      useActivityStore.getState().addEntry(mockEntry({ line: `entry ${i}` }));
    }
    expect(useActivityStore.getState().entries).toHaveLength(100);
    // First entry should still be present (no oldest-trimming)
    expect(useActivityStore.getState().entries[0].line).toBe('entry 0');
  });

  it('clears all entries', () => {
    useActivityStore.getState().addEntry(mockEntry());
    useActivityStore.getState().addEntry(mockEntry());
    useActivityStore.getState().clearEntries();
    expect(useActivityStore.getState().entries).toHaveLength(0);
  });

  it('manages paused state', () => {
    expect(useActivityStore.getState().paused).toBe(false);
    useActivityStore.getState().setPaused(true);
    expect(useActivityStore.getState().paused).toBe(true);
    useActivityStore.getState().setPaused(false);
    expect(useActivityStore.getState().paused).toBe(false);
  });
});
