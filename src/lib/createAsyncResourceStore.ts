// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file createAsyncResourceStore — Zustand factory for IPC-backed
 * load/save/edit resources (#716 finding #5).
 *
 * Six of the existing Zustand stores in this codebase
 * (`settingsStore`, `dependencyStore`, `updateStore`, `serviceStatusStore`,
 * `setupStore`, `downloadStore`) implement variants of the same shape:
 *
 *   1. Hold a typed `data: T` value, hydrated from a Tauri IPC.
 *   2. Track `isLoading: boolean` while the load IPC is in flight.
 *   3. Track `error: string | null` from the most recent load/save.
 *   4. (load+save stores only) Track `isDirty: boolean` between
 *      `update()` and the next `save()`.
 *   5. Provide a `debouncedSave()` that batches rapid edits.
 *
 * Each store currently re-implements that shape inline — the
 * settingsStore version is ~140 lines of state + actions, the
 * serviceStatusStore version is ~60. The boilerplate is mostly
 * verbatim; the shape ratchets up further when M8/M9/M10 land
 * per-service settings stores (BBC iPlayer / Spotify / YouTube)
 * that all need the same load+save lifecycle.
 *
 * This factory captures that shape once. Callsites pass:
 *   - `defaults: T` — initial in-memory value (used pre-load + by `reset`)
 *   - `load: () => Promise<T>` — IPC wrapper to fetch from backend
 *   - `save?: (data: T) => Promise<void>` — IPC wrapper to persist; omit
 *     for read-only resources (e.g., `serviceStatusStore`)
 *   - `debounceMs?: number` — debounce window; defaults to 300ms when
 *     `save` is provided, 0 otherwise
 *
 * Returns a Zustand `useStore` hook with the standard React hook +
 * `getState()` / `setState()` / `subscribe()` surface — drop-in
 * compatible with the rest of the codebase's store conventions.
 *
 * **Migration is opt-in.** None of the existing stores is migrated by
 * this commit; the factory exists so that NEW stores (the M8-M10
 * per-service settings pages, primarily) can use it from day one. A
 * follow-up cycle can migrate the existing stores incrementally if
 * the per-store API rename (`settings` → `data`, `loadSettings` →
 * `load`, etc.) is judged worth the consumer-side refactor across
 * 30+ files. See [codebase audit finding #5](../../.github/audits/codebase-unification-audit-v1.md#5-frontend-zustand-store-loadsavepersist-pattern--issue-tbd).
 *
 * @example Read-only resource (load only):
 *   export const useFooStore = createAsyncResourceStore<FooConfig>({
 *     defaults: { items: [] },
 *     load: () => commands.getFoo(),
 *   });
 *   // Component:  const items = useFooStore(s => s.data.items);
 *   // Startup:    useFooStore.getState().load();
 *
 * @example Read-write resource (load + save + dirty tracking):
 *   export const useBarStore = createAsyncResourceStore<BarSettings>({
 *     defaults: DEFAULT_BAR,
 *     load: () => commands.getBar(),
 *     save: (data) => commands.saveBar(data),
 *   });
 *   // Component:  useBarStore(s => s.update({ enabled: true }));
 *   //             useBarStore.getState().save();
 */

import { create, type StoreApi, type UseBoundStore } from 'zustand';

/**
 * The reactive state shape exposed by every async resource store.
 *
 * Generic `T` is the resource type (e.g. `AppSettings`,
 * `ServiceStatusConfig`, `UpdateInfo`). All four state fields are
 * always present — `isDirty` is meaningful only for stores
 * configured with a `save` function, but it's still typed so
 * consumers get a uniform API.
 */
export interface AsyncResourceState<T> {
  /** Current in-memory value. Initialized to `defaults`, replaced by `load`. */
  data: T;
  /** True while the most recent `load()` call is in flight. */
  isLoading: boolean;
  /**
   * True after `update()` is called, false after a successful `save()` or
   * `load()`. Always false for read-only resources (no `save` provided).
   */
  isDirty: boolean;
  /** Last error message from `load`/`save`, or null if the last call succeeded. */
  error: string | null;
}

/**
 * The action surface every async resource store provides. Read-only
 * stores still expose `save` / `debouncedSave` / `update` — they're
 * no-ops in that case so consumers can write defensive code without
 * narrowing on the config.
 */
export interface AsyncResourceActions<T> {
  /** Fetch from backend, replace `data`, clear `isDirty`/`error`. */
  load: () => Promise<void>;
  /**
   * Persist current `data` to backend. No-op when no `save` config was
   * supplied. Re-throws on failure so callers can surface UI feedback.
   */
  save: () => Promise<void>;
  /**
   * Debounced variant of `save` — batches rapid edits within the
   * configured window. No-op for read-only stores.
   */
  debouncedSave: () => void;
  /**
   * Shallow-merge a partial update into `data`, mark `isDirty: true`.
   * Spread semantics mean `update({ a: 1 })` doesn't touch other
   * fields, matching the per-store conventions this replaces.
   */
  update: (partial: Partial<T>) => void;
  /** Restore `data` to `defaults` and mark `isDirty: true`. */
  reset: () => void;
}

/**
 * Combined reactive state + actions surface — the type returned by
 * `createAsyncResourceStore<T>()`. Exported separately so consumers
 * can write component selectors like
 * `useFooStore((s: AsyncResourceStore<FooConfig>) => s.data.items)`
 * if they need the explicit annotation.
 */
export type AsyncResourceStore<T> = AsyncResourceState<T> &
  AsyncResourceActions<T>;

/**
 * Configuration for an async resource store. `save` is optional; when
 * omitted, the resource is treated as read-only (load-only stores
 * like `serviceStatusStore` fit here).
 */
export interface AsyncResourceConfig<T> {
  /** Initial in-memory value. Used pre-load and by `reset()`. */
  defaults: T;
  /** IPC wrapper to fetch the latest value from the backend. */
  load: () => Promise<T>;
  /** IPC wrapper to persist `data`. Omit for read-only resources. */
  save?: (data: T) => Promise<void>;
  /**
   * Debounce window for `debouncedSave`, in milliseconds. Defaults to
   * 300 (matches `settingsStore`'s historical value) when `save` is
   * supplied; ignored otherwise.
   */
  debounceMs?: number;
}

/**
 * Build a Zustand store hook that follows the standard async-resource
 * lifecycle described in this file's header. See the `@example` blocks
 * in the file-level docstring for typical wiring.
 */
export function createAsyncResourceStore<T extends object>(
  config: AsyncResourceConfig<T>
): UseBoundStore<StoreApi<AsyncResourceStore<T>>> {
  const debounceMs = config.debounceMs ?? 300;
  // Capture once so the async closure inside debouncedSave doesn't
  // need a non-null assertion or re-narrow across an await boundary.
  const saveFn = config.save;

  // The debounce timer is stored in module-local closure (one per
  // store instance — this whole closure runs once per
  // `createAsyncResourceStore` call). Same shape as the historical
  // `_saveDebounceTimer` in settingsStore.
  let saveDebounceTimer: ReturnType<typeof setTimeout> | null = null;

  return create<AsyncResourceStore<T>>((set, get) => ({
    data: config.defaults,
    isLoading: false,
    isDirty: false,
    error: null,

    load: async () => {
      set({ isLoading: true, error: null });
      try {
        const data = await config.load();
        set({ data, isLoading: false, isDirty: false });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        set({ error: message, isLoading: false });
      }
    },

    save: async () => {
      if (!saveFn) {
        // Read-only resource — defensive no-op so consumers don't
        // need to narrow on the config shape before calling save().
        return;
      }
      set({ error: null });
      try {
        await saveFn(get().data);
        set({ isDirty: false });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        set({ error: message });
        throw new Error(message, { cause: err });
      }
    },

    debouncedSave: () => {
      if (!saveFn) return;
      if (saveDebounceTimer) clearTimeout(saveDebounceTimer);
      saveDebounceTimer = setTimeout(async () => {
        saveDebounceTimer = null;
        try {
          await saveFn(get().data);
          set({ isDirty: false });
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err);
          set({ error: message });
        }
      }, debounceMs);
    },

    update: (partial) =>
      set((state) => ({
        data: { ...state.data, ...partial },
        isDirty: true,
      })),

    reset: () => set({ data: { ...config.defaults }, isDirty: true }),
  }));
}
