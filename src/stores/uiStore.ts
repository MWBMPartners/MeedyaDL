// Copyright (c) 2024-2026 MWBM Partners Ltd
/**
 * @file uiStore.ts -- UI State Management Store
 * @license MIT -- See LICENSE file in the project root.
 *
 * Manages transient (non-persisted) UI state for the application shell, including:
 *   - **Page navigation**: Tracks which page the user is viewing (download, queue,
 *     settings, help). Consumed by the `<Sidebar>` component for active-link
 *     highlighting and by `<MainContent>` for conditional page rendering.
 *   - **Sidebar collapse**: Controls whether the navigation sidebar is in its
 *     narrow (icon-only) mode. Consumed by `<Sidebar>` and `<AppLayout>`.
 *   - **Toast notifications**: A FIFO queue of ephemeral messages displayed by
 *     `<ToastContainer>` at the top of the viewport. Toasts auto-dismiss after
 *     a configurable duration.
 *   - **Setup wizard visibility**: Controls whether the first-run setup overlay
 *     is displayed, consumed by `<SetupWizard>` and `<App>`.
 *
 * This store is intentionally **not** persisted -- UI layout state resets on each
 * application launch. Sidebar collapse preference, however, is persisted separately
 * in `settingsStore.ts` (as `sidebar_collapsed`) and applied on startup.
 *
 * @see {@link https://zustand.docs.pmnd.rs/getting-started/introduction} -- Zustand overview
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state} -- How `set()` merges state
 */

// Zustand `create` builds a React hook (`useUiStore`) backed by a single store instance.
// Components that call `useUiStore(selector)` will re-render only when the selected
// slice of state changes -- Zustand handles this via strict-equality comparison.
import { create } from 'zustand';

// AppPage -- union literal type for sidebar navigation targets ('download' | 'queue' | ...)
// Toast    -- shape of an individual toast notification (id, message, type, duration)
// ToastType -- severity level union ('success' | 'error' | 'warning' | 'info')
import type { AppPage, Toast, ToastType } from '@/types';

/**
 * Shape of the UI state slice and its actions.
 *
 * Zustand stores combine state fields and action methods in a single interface.
 * Actions are functions that call `set()` to produce the next state; they are
 * **not** reducers -- they run imperatively and can be async.
 *
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state} -- `set()` merging semantics
 */
interface UiState {
  // ---------------------------------------------------------------------------
  // State fields
  // ---------------------------------------------------------------------------

  /**
   * Currently active navigation page (e.g., 'download', 'queue', 'settings', 'help').
   * Read by `<Sidebar>` to highlight the active nav link and by `<MainContent>`
   * to conditionally render the corresponding page component.
   * Defaults to 'download' so the user lands on the download page on launch.
   */
  currentPage: AppPage;

  /**
   * Whether the sidebar is collapsed into narrow (icon-only) mode.
   * Toggled via the sidebar's collapse button. The `<AppLayout>` component
   * reads this to adjust its CSS grid column widths.
   */
  sidebarCollapsed: boolean;

  /**
   * Stack of active toast notifications, rendered by `<ToastContainer>`.
   * Each toast has a unique `id` (for removal), a `message`, a severity `type`,
   * and an optional `duration` (ms) after which it auto-dismisses.
   * New toasts are appended to the end; the UI renders them top-to-bottom.
   */
  toasts: Toast[];

  /**
   * Controls visibility of the first-run setup wizard overlay.
   * Set to `true` by `<App>` on initial launch when dependencies are missing,
   * and set to `false` when the user completes or dismisses the wizard.
   */
  showSetupWizard: boolean;

  // ---------------------------------------------------------------------------
  // Actions -- each calls `set()` to produce the next immutable state snapshot
  // ---------------------------------------------------------------------------

  /**
   * Navigate to a different page. Replaces `currentPage` with the given value.
   * Called by `<Sidebar>` nav-link click handlers.
   * @param page -- The page to navigate to (one of AppPage union members)
   */
  setPage: (page: AppPage) => void;

  /**
   * Toggle the sidebar between expanded and collapsed modes.
   * Uses the updater-function form of `set()` to read the previous value.
   * Called by the sidebar's chevron toggle button.
   */
  toggleSidebar: () => void;

  /**
   * Explicitly set sidebar collapsed state (rather than toggling).
   * Used on startup to restore the user's saved preference from `settingsStore`.
   * @param collapsed -- `true` for icon-only mode, `false` for full sidebar
   */
  setSidebarCollapsed: (collapsed: boolean) => void;

  /**
   * Show or hide the first-run setup wizard overlay.
   * @param show -- `true` to display the wizard, `false` to dismiss it
   */
  setShowSetupWizard: (show: boolean) => void;

  /**
   * Create and display a new toast notification.
   * Generates a unique ID, appends the toast to the stack, and schedules
   * auto-removal after `duration` ms (default 5000). Permanent toasts can
   * be created by passing `duration = 0`.
   * @param message  -- Human-readable notification text
   * @param type     -- Severity level ('success' | 'error' | 'warning' | 'info')
   * @param duration -- Auto-dismiss delay in ms; 0 means persistent (default: 5000)
   */
  addToast: (message: string, type: ToastType, duration?: number) => void;

  /**
   * Remove a specific toast from the stack by its unique ID.
   * Called by the toast's dismiss button or by the auto-dismiss timer.
   * @param id -- The unique toast identifier (generated by `addToast`)
   */
  removeToast: (id: string) => void;
}

/**
 * Zustand store hook for transient UI state.
 *
 * Usage in components:
 *   const page = useUiStore((s) => s.currentPage);  // subscribe to one field
 *   const { addToast } = useUiStore();               // destructure actions
 *
 * `create<UiState>` returns a React hook that triggers re-renders only when
 * the selected slice of state changes (shallow equality by default).
 *
 * The `set` callback provided by Zustand performs a **shallow merge** -- only
 * the keys you return are overwritten; all other state fields are preserved.
 * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state}
 */
export const useUiStore = create<UiState>((set) => ({
  // -------------------------------------------------------------------------
  // Initial state values
  // -------------------------------------------------------------------------
  currentPage: 'download', // Start on the Download page by default
  sidebarCollapsed: false, // Sidebar starts fully expanded; overridden by settings on startup
  toasts: [],              // No active notifications on launch
  showSetupWizard: false,  // Setup wizard hidden until <App> decides to show it

  // -------------------------------------------------------------------------
  // Actions
  // -------------------------------------------------------------------------

  /**
   * Replace the active page. Uses the object-shorthand form of `set()` which
   * shallow-merges the returned object into the current state.
   */
  setPage: (page) => set({ currentPage: page }),

  /**
   * Toggle sidebar collapse. Uses the **updater-function** form of `set()`
   * (`set((prev) => next)`) to derive the new value from the previous state,
   * avoiding stale-closure issues.
   * @see {@link https://zustand.docs.pmnd.rs/guides/updating-state#using-updater-function}
   */
  toggleSidebar: () =>
    set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),

  /**
   * Directly set sidebar collapsed state. Called on app startup from the
   * persisted `sidebar_collapsed` setting loaded by `settingsStore`.
   */
  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),

  /** Show or hide the setup wizard modal overlay. */
  setShowSetupWizard: (show) => set({ showSetupWizard: show }),

  /**
   * Create a new toast notification with a unique ID and optional auto-dismiss.
   *
   * ID generation combines a high-resolution timestamp with a random suffix to
   * ensure uniqueness even if multiple toasts are created in the same millisecond.
   *
   * The `setTimeout` auto-dismiss runs outside React's render cycle; when it fires,
   * it uses the updater-function form of `set()` to safely read the latest toast
   * array (avoiding stale closures over the `toasts` array).
   */
  addToast: (message, type, duration = 5000) => {
    // Generate a collision-resistant unique ID for this toast.
    // Format: "toast-<epoch_ms>-<random_5_char>"
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;

    // Append the new toast to the end of the array (immutable spread pattern).
    set((state) => ({
      toasts: [...state.toasts, { id, message, type, duration }],
    }));

    // Schedule automatic removal after `duration` ms.
    // A duration of 0 means the toast persists until manually dismissed.
    if (duration > 0) {
      setTimeout(() => {
        // Use updater form to read the latest toasts array at dismissal time.
        set((state) => ({
          toasts: state.toasts.filter((t) => t.id !== id),
        }));
      }, duration);
    }
  },

  /**
   * Remove a specific toast by its unique ID. Produces a new array with
   * the target toast filtered out (immutable update pattern).
   */
  removeToast: (id) =>
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    })),
}));
