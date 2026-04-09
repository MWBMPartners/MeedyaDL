// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

import { describe, it, expect, beforeEach } from 'vitest';
import { useActivityStore, MAX_ENTRIES } from './activityStore';
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

  it('assigns stable _id to each entry via addEntry', () => {
    useActivityStore.getState().addEntry(mockEntry({ line: 'a' }));
    useActivityStore.getState().addEntry(mockEntry({ line: 'b' }));

    const entries = useActivityStore.getState().entries;
    expect(entries[0]._id).toBeDefined();
    expect(entries[1]._id).toBeDefined();
    // IDs should be unique and sequential
    expect(entries[1]._id!).toBeGreaterThan(entries[0]._id!);
  });

  it('caps entries at MAX_ENTRIES, trimming oldest', () => {
    // Fill to capacity
    const batch = Array.from({ length: MAX_ENTRIES + 500 }, (_, i) =>
      mockEntry({ line: `entry ${i}` })
    );
    useActivityStore.getState().addEntries(batch);

    const entries = useActivityStore.getState().entries;
    expect(entries).toHaveLength(MAX_ENTRIES);
    // Oldest entries should be trimmed — first entry should be entry 500
    expect(entries[0].line).toBe('entry 500');
    // Last entry should be the final one added
    expect(entries[entries.length - 1].line).toBe(`entry ${MAX_ENTRIES + 499}`);
  });

  it('addEntries appends a batch in a single update', () => {
    const batch = [
      mockEntry({ line: 'batch-1' }),
      mockEntry({ line: 'batch-2' }),
      mockEntry({ line: 'batch-3' }),
    ];
    useActivityStore.getState().addEntries(batch);

    const entries = useActivityStore.getState().entries;
    expect(entries).toHaveLength(3);
    expect(entries[0].line).toBe('batch-1');
    expect(entries[2].line).toBe('batch-3');
  });

  it('addEntries assigns stable _ids', () => {
    const batch = [mockEntry({ line: 'a' }), mockEntry({ line: 'b' })];
    useActivityStore.getState().addEntries(batch);

    const entries = useActivityStore.getState().entries;
    expect(entries[0]._id).toBeDefined();
    expect(entries[1]._id).toBeDefined();
    expect(entries[1]._id!).toBeGreaterThan(entries[0]._id!);
  });

  it('addEntries with empty array is a no-op', () => {
    useActivityStore.getState().addEntry(mockEntry({ line: 'existing' }));
    const before = useActivityStore.getState().entries;
    useActivityStore.getState().addEntries([]);
    const after = useActivityStore.getState().entries;
    // Should return the same state reference (no-op)
    expect(after).toBe(before);
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
