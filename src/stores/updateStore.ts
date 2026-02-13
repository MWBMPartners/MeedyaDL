// Copyright (c) 2024-2026 MeedyaDL
/**
 * @file updateStore.ts -- Update Checking & Upgrade State Management Store
 * @license MIT -- See LICENSE file in the project root.
 *
 * Manages the lifecycle of checking for and applying updates to application
 * components (GAMDL, the desktop app itself, Python runtime, etc.).
 *
 *   **Auto-check flow** (on startup):
 *   1. `<App>` checks `settingsStore.settings.auto_check_updates`.
 *   2. If enabled, calls `checkForUpdates()` which invokes the Rust
 *      `check_all_updates` command.
 *   3. The backend queries PyPI (for GAMDL), GitHub Releases (for the app),
 *      and local version info to produce an `UpdateCheckResult`.
 *   4. The result contains a `components[]` array where each entry reports
 *      current version, latest version, whether an update is available,
 *      and whether it is compatible with the current setup.
 *
 *   **Manual check flow**:
 *   - User clicks "Check for Updates" in settings or the update banner.
 *   - Same as auto-check, but `dismissed` is cleared so all updates reappear.
 *
 *   **GAMDL upgrade flow**:
 *   1. User clicks "Upgrade GAMDL" on the update notification.
 *   2. `upgradeGamdl()` calls `commands.upgradeGamdl()` -> Rust `upgrade_gamdl`
 *      which runs `pip install --upgrade gamdl`.
 *   3. After upgrade, re-checks all components to refresh version info.
 *
 *   **Dismiss flow**:
 *   - User can dismiss individual component update notifications.
 *   - Dismissed component names are stored in `dismissed[]`.
 *   - `getActiveUpdates()` filters out dismissed updates.
 *   - Dismissals are cleared on the next fresh update check.
 *
 * Consumed by: `<UpdateBanner>`, `<SettingsPage>`, `<App>` (auto-check).
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state} -- Zustand state updates
 * @see {@link https://v2.tauri.app/develop/calling-rust/} -- Tauri IPC invoke()
 */

// Zustand store factory. Creates a React hook with automatic subscription management.
import { create } from 'zustand';

// ComponentUpdate    -- per-component update info: name, versions, update_available, is_compatible
// UpdateCheckResult  -- aggregated result: has_updates, components[], errors[], checked_at
import type { ComponentUpdate, UpdateCheckResult } from '@/types';

// Type-safe wrappers for Tauri IPC commands related to update management.
// `checkAllUpdates` -> Rust `check_all_updates`
// `upgradeGamdl`    -> Rust `upgrade_gamdl`
import * as commands from '@/lib/tauri-commands';

/**
 * Combined state + actions interface for the update store.
 *
 * State tracks:
 *   - The most recent check result (or `null` if never checked).
 *   - Loading flags for check and upgrade operations.
 *   - A list of dismissed component names (user chose to ignore the update).
 *
 * Actions include:
 *   - Async operations that call the Rust backend (check, upgrade).
 *   - Synchronous dismissal management.
 *   - Computed getters that derive filtered update lists.
 */
interface UpdateState {
  // ---------------------------------------------------------------------------
  // State fields
  // ---------------------------------------------------------------------------

  /**
   * The full result from the most recent update check.
   * Contains `components[]` with per-component version info, `has_updates`
   * aggregate flag, `checked_at` timestamp, and any `errors[]` encountered.
   * `null` if `checkForUpdates()` has never been called.
   */
  lastResult: UpdateCheckResult | null;

  /**
   * `true` while `checkForUpdates()` is awaiting the Rust backend response.
   * The `<UpdateBanner>` or settings page shows a spinner during this time.
   */
  isChecking: boolean;

  /**
   * `true` while `upgradeGamdl()` is running the pip upgrade command.
   * The upgrade button is disabled and shows a progress indicator.
   */
  isUpgrading: boolean;

  /**
   * Array of component `name` strings whose update notifications the user
   * has dismissed. Used by `getActiveUpdates()` to filter the displayed list.
   *
   * Cleared automatically when a new `checkForUpdates()` call completes,
   * ensuring that freshly-detected updates are always visible.
   *
   * Example: `['gamdl', 'app']` means updates for both GAMDL and the app
   * have been dismissed by the user.
   */
  dismissed: string[];

  /**
   * Error message from the last failed `checkForUpdates()` or `upgradeGamdl()`
   * operation. `null` when there is no error.
   */
  error: string | null;

  // ---------------------------------------------------------------------------
  // Async actions (communicate with Rust backend)
  // ---------------------------------------------------------------------------

  /**
   * Check for updates to all registered components.
   *
   * IPC call: `commands.checkAllUpdates()` -> Rust `check_all_updates`
   * The Rust handler queries:
   *   - PyPI for the latest GAMDL version
   *   - GitHub Releases for the latest desktop app version
   *   - Local version info for the Python runtime
   *
   * On success: stores the result and clears previous dismissals so
   * newly-detected updates are visible to the user.
   * On failure: stores the error and re-throws.
   *
   * @returns The full `UpdateCheckResult` for the calling component to inspect
   * @throws If the backend check fails (e.g., network error)
   */
  checkForUpdates: () => Promise<UpdateCheckResult>;

  /**
   * Upgrade GAMDL to the latest compatible version.
   *
   * IPC call: `commands.upgradeGamdl()` -> Rust `upgrade_gamdl`
   * The Rust handler runs `pip install --upgrade gamdl` in the portable
   * Python environment.
   *
   * After the upgrade completes, automatically re-checks all components
   * to refresh version information in the UI.
   *
   * @returns The newly-installed GAMDL version string
   * @throws If the pip upgrade fails
   */
  upgradeGamdl: () => Promise<string>;

  // ---------------------------------------------------------------------------
  // Synchronous actions (local state only)
  // ---------------------------------------------------------------------------

  /**
   * Dismiss an update notification for a specific component.
   * Adds the component name to the `dismissed` array. The update will
   * no longer appear in `getActiveUpdates()` until the next check.
   * @param componentName -- The component name (e.g., 'gamdl', 'app')
   */
  dismissUpdate: (componentName: string) => void;

  /**
   * Clear all dismissed update notifications.
   * Called internally by `checkForUpdates()` after a successful check,
   * or externally if the user wants to "un-dismiss" all updates.
   */
  clearDismissed: () => void;

  // ---------------------------------------------------------------------------
  // Computed getters (derive values from current state via `get()`)
  // ---------------------------------------------------------------------------

  /**
   * Returns an array of `ComponentUpdate` objects that:
   *   1. Have an available update (`update_available === true`).
   *   2. Are compatible with the current setup (`is_compatible === true`).
   *   3. Have NOT been dismissed by the user.
   *
   * This is the primary data source for the `<UpdateBanner>` component.
   *
   * @returns Filtered array of actionable updates (may be empty)
   */
  getActiveUpdates: () => ComponentUpdate[];

  /**
   * Convenience boolean: `true` if there is at least one active (non-dismissed,
   * compatible) update available. Used to conditionally render the update banner.
   *
   * @returns Whether there are any actionable updates
   */
  hasActiveUpdates: () => boolean;
}

/**
 * Zustand store hook for update checking and GAMDL upgrade state.
 *
 * Usage in components:
 *   const isChecking = useUpdateStore((s) => s.isChecking);
 *   const { checkForUpdates, upgradeGamdl } = useUpdateStore();
 *   const updates = useUpdateStore((s) => s.getActiveUpdates());
 *
 * The store creator receives `set` (for state updates) and `get` (for reading
 * current state in computed getters like `getActiveUpdates()`).
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state}
 */
export const useUpdateStore = create<UpdateState>((set, get) => ({
  // -------------------------------------------------------------------------
  // Initial state -- no results, nothing in progress
  // -------------------------------------------------------------------------
  lastResult: null,    // No check result until first checkForUpdates() call
  isChecking: false,   // No check in progress
  isUpgrading: false,  // No upgrade in progress
  dismissed: [],       // No dismissed update notifications
  error: null,         // No error

  // -------------------------------------------------------------------------
  // Async actions
  // -------------------------------------------------------------------------

  /**
   * Check for updates to all registered components.
   *
   * IPC call: `commands.checkAllUpdates()` -> Rust `check_all_updates`
   *
   * The `dismissed` array is cleared on a successful check so that
   * freshly-detected updates are always shown to the user, even if they
   * previously dismissed the same component. This ensures the user is
   * aware of new versions without needing to manually "un-dismiss".
   */
  checkForUpdates: async () => {
    // Signal check in progress and clear stale errors.
    set({ isChecking: true, error: null });
    try {
      // Invoke the Rust command that queries PyPI, GitHub Releases, etc.
      const result = await commands.checkAllUpdates();
      // Store the result and clear previous dismissals atomically.
      // Clearing dismissals ensures newly-detected updates are visible.
      set({ lastResult: result, isChecking: false, dismissed: [] });
      return result;
    } catch (e) {
      // Normalize error to string and store for UI display.
      const message = e instanceof Error ? e.message : String(e);
      set({ error: message, isChecking: false });
      // Re-throw so the calling component can show a toast or take action.
      throw new Error(message);
    }
  },

  /**
   * Upgrade GAMDL to the latest compatible version via pip.
   *
   * IPC call: `commands.upgradeGamdl()` -> Rust `upgrade_gamdl`
   *
   * After the pip upgrade completes, a full update check is performed
   * to refresh all component version information. This ensures the UI
   * immediately reflects the new GAMDL version and removes the update
   * notification for GAMDL (since `update_available` will now be `false`).
   */
  upgradeGamdl: async () => {
    // Signal upgrade in progress and clear stale errors.
    set({ isUpgrading: true, error: null });
    try {
      // Run `pip install --upgrade gamdl` in the portable Python environment.
      const version = await commands.upgradeGamdl();
      // Re-check all components to refresh version info post-upgrade.
      const result = await commands.checkAllUpdates();
      // Store the fresh result and clear the upgrading flag.
      set({ lastResult: result, isUpgrading: false });
      return version;
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      set({ error: message, isUpgrading: false });
      throw new Error(message);
    }
  },

  // -------------------------------------------------------------------------
  // Synchronous actions
  // -------------------------------------------------------------------------

  /**
   * Dismiss an update notification for a specific component.
   * Appends the component name to the `dismissed` array using an immutable
   * spread pattern. The `getActiveUpdates()` getter will exclude it.
   */
  dismissUpdate: (componentName) =>
    set((state) => ({
      dismissed: [...state.dismissed, componentName],
    })),

  /** Reset the dismissed list, making all available updates visible again. */
  clearDismissed: () => set({ dismissed: [] }),

  // -------------------------------------------------------------------------
  // Computed getters -- derive filtered data from current state
  // -------------------------------------------------------------------------

  /**
   * Filter the last check result to only include actionable updates:
   *   - `update_available` must be `true` (a newer version exists)
   *   - `is_compatible` must be `true` (the update works with current setup)
   *   - The component must NOT be in the `dismissed` list
   *
   * Returns an empty array if no check has been performed yet (`lastResult` is null).
   *
   * Uses `get()` for synchronous access to the latest state.
   * `dismissed.includes()` is O(n) but the list is always tiny (< 10 items).
   */
  getActiveUpdates: () => {
    const { lastResult, dismissed } = get();
    // Guard: no results yet means no active updates.
    if (!lastResult) return [];
    return lastResult.components.filter(
      (c) =>
        c.update_available &&       // A newer version exists on PyPI/GitHub
        c.is_compatible &&          // The update is safe to install
        !dismissed.includes(c.name), // The user hasn't dismissed this notification
    );
  },

  /**
   * Convenience boolean derived from `getActiveUpdates()`.
   * Returns `true` if at least one actionable update exists.
   * Used to conditionally render the `<UpdateBanner>` component.
   */
  hasActiveUpdates: () => {
    return get().getActiveUpdates().length > 0;
  },
}));
