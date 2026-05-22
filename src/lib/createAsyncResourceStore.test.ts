// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for createAsyncResourceStore (#716 finding #5).
 *
 * Pins the lifecycle behaviour the factory promises so the
 * eventual settingsStore/dependencyStore/etc. migrations have a
 * stable target. Covers:
 *   - Initial state defaults
 *   - load: happy path + error path + isLoading transitions
 *   - save: happy path + error path + isDirty clearing + re-throw
 *   - debouncedSave: batches rapid edits + respects debounceMs
 *   - update: shallow merge + sets isDirty
 *   - reset: restores defaults + sets isDirty
 *   - read-only resources (no save config): save/debouncedSave no-op
 *
 * @see src/lib/createAsyncResourceStore.ts
 */

import { createAsyncResourceStore } from '@/lib/createAsyncResourceStore';

interface TestData {
  count: number;
  label: string;
  nested?: { flag: boolean };
}

const DEFAULTS: TestData = { count: 0, label: 'init' };

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('createAsyncResourceStore', () => {
  // ===========================================================================
  // Initial state
  // ===========================================================================

  it('initialises with defaults and clean state', () => {
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn(),
    });
    const s = useStore.getState();
    expect(s.data).toEqual(DEFAULTS);
    expect(s.isLoading).toBe(false);
    expect(s.isDirty).toBe(false);
    expect(s.error).toBeNull();
  });

  // ===========================================================================
  // load
  // ===========================================================================

  it('load: replaces data + clears isLoading + clears isDirty on success', async () => {
    const fresh: TestData = { count: 42, label: 'loaded' };
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn().mockResolvedValue(fresh),
    });
    // Pre-load: dirty the store so we can prove load clears it.
    useStore.getState().update({ count: 99 });
    expect(useStore.getState().isDirty).toBe(true);

    await useStore.getState().load();

    const s = useStore.getState();
    expect(s.data).toEqual(fresh);
    expect(s.isLoading).toBe(false);
    expect(s.isDirty).toBe(false);
    expect(s.error).toBeNull();
  });

  it('load: surfaces error string + clears isLoading on rejection', async () => {
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn().mockRejectedValue(new Error('IPC failed')),
    });
    await useStore.getState().load();
    const s = useStore.getState();
    expect(s.error).toBe('IPC failed');
    expect(s.isLoading).toBe(false);
    // Data preserved on failure (last-known-good).
    expect(s.data).toEqual(DEFAULTS);
  });

  it('load: stringifies non-Error rejections', async () => {
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn().mockRejectedValue('plain string failure'),
    });
    await useStore.getState().load();
    expect(useStore.getState().error).toBe('plain string failure');
  });

  // ===========================================================================
  // save
  // ===========================================================================

  it('save: invokes save fn with current data + clears isDirty on success', async () => {
    const saveFn = vi.fn().mockResolvedValue(undefined);
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn(),
      save: saveFn,
    });
    useStore.getState().update({ count: 7 });
    expect(useStore.getState().isDirty).toBe(true);

    await useStore.getState().save();

    expect(saveFn).toHaveBeenCalledWith({ count: 7, label: 'init' });
    expect(useStore.getState().isDirty).toBe(false);
    expect(useStore.getState().error).toBeNull();
  });

  it('save: re-throws on failure and surfaces error to state', async () => {
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn(),
      save: vi.fn().mockRejectedValue(new Error('disk full')),
    });
    useStore.getState().update({ count: 1 });

    await expect(useStore.getState().save()).rejects.toThrow('disk full');
    expect(useStore.getState().error).toBe('disk full');
    // isDirty preserved on save failure — the user still has unsaved changes.
    expect(useStore.getState().isDirty).toBe(true);
  });

  it('save: read-only stores (no save config) are silent no-ops', async () => {
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn(),
      // No save fn supplied
    });
    // Should not throw, should not mutate state.
    await expect(useStore.getState().save()).resolves.toBeUndefined();
    expect(useStore.getState().error).toBeNull();
  });

  // ===========================================================================
  // debouncedSave
  // ===========================================================================

  it('debouncedSave: batches rapid calls into a single save', async () => {
    const saveFn = vi.fn().mockResolvedValue(undefined);
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn(),
      save: saveFn,
      debounceMs: 200,
    });
    useStore.getState().update({ count: 1 });
    useStore.getState().debouncedSave();
    useStore.getState().update({ count: 2 });
    useStore.getState().debouncedSave();
    useStore.getState().update({ count: 3 });
    useStore.getState().debouncedSave();

    // Nothing yet — the debounce window hasn't elapsed.
    expect(saveFn).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(200);

    expect(saveFn).toHaveBeenCalledTimes(1);
    // Should have used the LATEST data, not the first call's snapshot.
    expect(saveFn).toHaveBeenCalledWith({ count: 3, label: 'init' });
    expect(useStore.getState().isDirty).toBe(false);
  });

  it('debouncedSave: defaults to 300ms when debounceMs not specified', async () => {
    const saveFn = vi.fn().mockResolvedValue(undefined);
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn(),
      save: saveFn,
    });
    useStore.getState().update({ count: 1 });
    useStore.getState().debouncedSave();

    await vi.advanceTimersByTimeAsync(299);
    expect(saveFn).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(saveFn).toHaveBeenCalledTimes(1);
  });

  it('debouncedSave: read-only stores are no-ops', () => {
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn(),
    });
    expect(() => useStore.getState().debouncedSave()).not.toThrow();
  });

  it('debouncedSave: surfaces save errors to state but does not throw', async () => {
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn(),
      save: vi.fn().mockRejectedValue(new Error('debounce save failed')),
      debounceMs: 50,
    });
    useStore.getState().update({ count: 5 });
    useStore.getState().debouncedSave();

    await vi.advanceTimersByTimeAsync(50);

    expect(useStore.getState().error).toBe('debounce save failed');
    // isDirty stays true because the save didn't succeed.
    expect(useStore.getState().isDirty).toBe(true);
  });

  // ===========================================================================
  // update
  // ===========================================================================

  it('update: shallow-merges partial + sets isDirty', () => {
    const useStore = createAsyncResourceStore<TestData>({
      defaults: { count: 0, label: 'a', nested: { flag: false } },
      load: vi.fn(),
    });
    useStore.getState().update({ count: 5 });
    const s = useStore.getState();
    expect(s.data).toEqual({ count: 5, label: 'a', nested: { flag: false } });
    expect(s.isDirty).toBe(true);
  });

  it('update: produces a new object reference (Zustand reference equality)', () => {
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn(),
    });
    const before = useStore.getState().data;
    useStore.getState().update({ count: 99 });
    const after = useStore.getState().data;
    expect(after).not.toBe(before);
  });

  // ===========================================================================
  // reset
  // ===========================================================================

  it('reset: restores defaults + sets isDirty (so user must save explicitly)', async () => {
    const useStore = createAsyncResourceStore<TestData>({
      defaults: DEFAULTS,
      load: vi.fn().mockResolvedValue({ count: 100, label: 'loaded' }),
      save: vi.fn().mockResolvedValue(undefined),
    });
    await useStore.getState().load();
    expect(useStore.getState().data.count).toBe(100);

    useStore.getState().reset();
    const s = useStore.getState();
    expect(s.data).toEqual(DEFAULTS);
    expect(s.isDirty).toBe(true);
  });

  it('reset: produces a copy of defaults (no shared reference)', () => {
    const defaults = { count: 0, label: 'init' };
    const useStore = createAsyncResourceStore<TestData>({
      defaults,
      load: vi.fn(),
    });
    useStore.getState().reset();
    expect(useStore.getState().data).not.toBe(defaults);
  });
});
