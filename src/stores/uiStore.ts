// Copyright (c) 2026 MeedyaSuite
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

/**
 * Centralised toast-dismissal worker (#894).
 *
 * Pre-#894, `addToast` scheduled a `setTimeout` PER toast that
 * captured `id` + the `set` callback in a closure. Over a multi-hour
 * session with frequent toasts (e.g. clipboard-monitor URL detections
 * every 2s + download-progress event toasts), hundreds-to-thousands
 * of in-flight timers accumulated. Each closure retained a slice of
 * the React tree until the timer fired.
 *
 * The replacement is a single `setInterval` that polls every
 * `WORKER_TICK_MS` and removes any toast whose `expiresAt` deadline
 * has passed. The worker is started lazily on the first non-
 * persistent toast and stopped automatically when no non-persistent
 * toasts remain. No per-toast closures.
 *
 * Tick rate: 250 ms — fast enough that the user perceives auto-
 * dismiss as immediate, slow enough that idle CPU is unmeasurable.
 */
const WORKER_TICK_MS = 250;
let dismissalWorkerHandle: ReturnType<typeof setInterval> | null = null;

function ensureDismissalWorker(): void {
  if (dismissalWorkerHandle !== null) return;
  dismissalWorkerHandle = setInterval(() => {
    // Read the store directly to avoid stale-closure issues. Filter
    // out any toast whose deadline has passed. If nothing remains
    // that the worker can act on, stop the worker.
    const now = Date.now();
    const store = useUiStore.getState();
    const before = store.toasts.length;
    const filtered = store.toasts.filter(
      (t) => t.expiresAt == null || t.expiresAt > now,
    );
    if (filtered.length !== before) {
      useUiStore.setState({ toasts: filtered });
    }
    // Stop the worker when no toast has a non-null expiresAt.
    const hasExpiring = useUiStore
      .getState()
      .toasts.some((t) => t.expiresAt != null);
    if (!hasExpiring && dismissalWorkerHandle !== null) {
      clearInterval(dismissalWorkerHandle);
      dismissalWorkerHandle = null;
    }
  }, WORKER_TICK_MS);
}

/**
 * Test-only: stop the dismissal worker and clear the handle. Lets
 * unit tests start each test with a clean worker state.
 *
 * @internal
 */
export function __resetToastWorkerForTests(): void {
  if (dismissalWorkerHandle !== null) {
    clearInterval(dismissalWorkerHandle);
    dismissalWorkerHandle = null;
  }
}

// AppPage -- union literal type for sidebar navigation targets ('download' | 'queue' | ...)
// Toast    -- shape of an individual toast notification (id, message, type, duration)
// ToastType -- severity level union ('success' | 'error' | 'warning' | 'info')
import type { AppPage, Toast, ToastType } from '@/types';
import { useSettingsStore } from '@/stores/settingsStore';

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

  /**
   * Controls visibility of the pre-release first-load notice modal.
   * Shown when a new pre-release version (v0.x.x) is launched for the first
   * time. Dismissed by the user, after which the modal won't appear again
   * until the next version change.
   */
  showPrereleaseNotice: boolean;

  /** Whether the crash report opt-in prompt is shown (first launch only). */
  showCrashReportPrompt: boolean;

  /** Set the crash report prompt visibility. */
  setShowCrashReportPrompt: (show: boolean) => void;

  /**
   * Whether the Spotify first-run consent modal is visible (M9-UI).
   * Flipped by `DownloadForm.runPreflightAndSubmit` when it detects
   * a Spotify URL and `spotify_consent_acknowledged` is still false.
   */
  showSpotifyConsent: boolean;

  /** Set the Spotify consent modal visibility. */
  setShowSpotifyConsent: (show: boolean) => void;

  /**
   * Callback the consent modal invokes on user-accept (M9-UI).
   * Captured at the point the modal opens so the form's submit flow
   * can resume after consent without rebuilding request state from
   * stale refs. Cleared on dismiss or after the callback fires.
   */
  pendingSpotifyConsentCallback: (() => Promise<void>) | null;

  /** Set the pending consent callback. */
  setPendingSpotifyConsentCallback: (cb: (() => Promise<void>) | null) => void;

  /**
   * When set, the Help page should auto-navigate to this topic ID on mount.
   * Used by the `<HelpButton>` component for contextual deep-linking from
   * settings fields to their corresponding help documentation.
   * Reset to `null` after the HelpViewer consumes it.
   */
  helpActiveTopic: string | null;

  /**
   * Whether the global Keyboard Shortcuts help dialog (#465) is
   * currently visible. Flipped by the Cmd/Ctrl+Shift+? handler in
   * `useKeyboardShortcuts.ts` and consumed by `ShortcutsHelpDialog`
   * which is mounted in MainLayout so it can appear over any page.
   */
  shortcutsHelpOpen: boolean;

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
   * Show or hide the pre-release first-load notice modal.
   * @param show -- `true` to display the notice, `false` to dismiss it
   */
  setShowPrereleaseNotice: (show: boolean) => void;

  /**
   * Navigate to the Help page and auto-select a specific topic.
   * Sets both `currentPage` to 'help' and `helpActiveTopic` to the given ID.
   * Called by `<HelpButton>` components embedded in settings fields.
   * @param topic -- Help topic ID (e.g. 'cookies-help', 'audio-codecs')
   */
  navigateToHelp: (topic: string) => void;

  /**
   * Clear the help topic deep-link after it has been consumed by HelpViewer.
   * Prevents stale topic selections on subsequent visits to the Help page.
   */
  clearHelpActiveTopic: () => void;

  /** Open or close the global Keyboard Shortcuts help dialog (#465). */
  setShortcutsHelpOpen: (open: boolean) => void;

  /**
   * Create and display a new toast notification.
   * Generates a unique ID, appends the toast to the stack, and schedules
   * auto-removal after `duration` ms (default 5000). Permanent toasts can
   * be created by passing `duration = 0`.
   *
   * **Deduplication**: If a toast with the same `message` is already on screen,
   * the new toast is silently dropped. If a `key` is provided and a toast with
   * the same key exists, the old one is replaced with the new one.
   *
   * @param message  -- Human-readable notification text
   * @param type     -- Severity level ('success' | 'error' | 'warning' | 'info')
   * @param duration -- Auto-dismiss delay in ms; 0 means persistent (default: 5000)
   * @param key      -- Optional deduplication key (e.g. 'preflight:Wrapper')
   * @param action   -- Optional action button (label + onClick callback)
   */
  addToast: (message: string, type: ToastType, duration?: number, key?: string, action?: { label: string; onClick: () => void }) => void;

  /**
   * Remove a specific toast from the stack by its unique ID.
   * Called by the toast's dismiss button or by the auto-dismiss timer.
   * @param id -- The unique toast identifier (generated by `addToast`)
   */
  removeToast: (id: string) => void;

  /**
   * Remove all toasts with a given key from the stack.
   * Used to dismiss category-specific toasts when their condition resolves
   * (e.g., remove wrapper warnings when the wrapper becomes reachable).
   * @param key -- The category key to match (e.g. 'preflight:Wrapper')
   */
  removeToastsByKey: (key: string) => void;
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
export const useUiStore = create<UiState>((set, get) => ({
  // -------------------------------------------------------------------------
  // Initial state values
  // -------------------------------------------------------------------------
  currentPage: 'download', // Start on the Download page by default
  sidebarCollapsed: false, // Sidebar starts fully expanded; overridden by settings on startup
  toasts: [], // No active notifications on launch
  showSetupWizard: false, // Setup wizard hidden until <App> decides to show it
  showPrereleaseNotice: false, // Pre-release notice hidden until version change detected
  showCrashReportPrompt: false, // Crash report opt-in prompt (first launch only)
  showSpotifyConsent: false, // M9-UI: Spotify first-run consent modal
  pendingSpotifyConsentCallback: null, // M9-UI: callback to invoke on consent accept
  helpActiveTopic: null, // No deep-link target until a HelpButton is clicked
  shortcutsHelpOpen: false, // Shortcuts help dialog closed until Cmd/Ctrl+Shift+? opens it (#465)

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
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),

  /**
   * Directly set sidebar collapsed state. Called on app startup from the
   * persisted `sidebar_collapsed` setting loaded by `settingsStore`.
   */
  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),

  /** Show or hide the setup wizard modal overlay. */
  setShowSetupWizard: (show) => set({ showSetupWizard: show }),

  /** Show or hide the pre-release first-load notice modal. */
  setShowPrereleaseNotice: (show) => set({ showPrereleaseNotice: show }),
  setShowCrashReportPrompt: (show: boolean) => set({ showCrashReportPrompt: show }),
  setShowSpotifyConsent: (show: boolean) => set({ showSpotifyConsent: show }),
  setPendingSpotifyConsentCallback: (cb: (() => Promise<void>) | null) =>
    set({ pendingSpotifyConsentCallback: cb }),

  /**
   * Navigate to the Help page and deep-link to a specific topic.
   * Sets both the page and the topic in a single state update.
   */
  navigateToHelp: (topic) => set({ currentPage: 'help', helpActiveTopic: topic }),

  /** Clear the help deep-link topic after HelpViewer has consumed it. */
  clearHelpActiveTopic: () => set({ helpActiveTopic: null }),

  setShortcutsHelpOpen: (open) => set({ shortcutsHelpOpen: open }),

  /**
   * Create a new toast notification with a unique ID and optional auto-dismiss.
   *
   * **Deduplication rules:**
   * 1. If a toast with the exact same `message` already exists → skip (no duplicate).
   * 2. If a `key` is provided and a toast with that key exists → replace the old one.
   *
   * ID generation combines a high-resolution timestamp with a random suffix to
   * ensure uniqueness even if multiple toasts are created in the same millisecond.
   *
   * The `setTimeout` auto-dismiss runs outside React's render cycle; when it fires,
   * it uses the updater-function form of `set()` to safely read the latest toast
   * array (avoiding stale closures over the `toasts` array).
   */
  addToast: (message, type, duration?, key?, action?) => {
    const { settings } = useSettingsStore.getState();
    const style = settings.notification_style ?? 'native_and_in_app';

    // Resolve duration: use explicit value if provided, otherwise read from settings.
    // Error and warning toasts are persistent (0 = no auto-dismiss) unless explicitly timed.
    if (duration === undefined) {
      if (type === 'error' || type === 'warning') {
        duration = 0; // Persistent — requires manual dismissal
      } else {
        duration = (settings.notification_auto_dismiss_seconds ?? 5) * 1000;
      }
    }

    // #993: Determine up front whether this message is already showing as an
    // in-app toast. The in-app `set()` updater below (see the `state.toasts.some(...)`
    // check) already deduplicates identical messages, but that check runs AFTER
    // the native notification would have fired -- so a repeated warning/error that
    // gets correctly deduped in-app was still stacking N identical OS banners.
    const isDuplicateInApp = get().toasts.some((t) => t.message === message);

    // Send native OS notification when notification_style includes native.
    // Errors are surfaced to the WebView console so future regressions can
    // be diagnosed instead of failing silently (#658).
    //
    // Gate on in-app dedup EXCEPT in 'native_only' mode: in that mode the
    // `toasts` array is intentionally always empty (see the early return
    // below), so `isDuplicateInApp` can never be true and state-based dedup
    // is impossible -- gating on it there would silently suppress every
    // native notification after the first.
    if (
      (style === 'native_and_in_app' && !isDuplicateInApp) ||
      style === 'native_only'
    ) {
      const typeLabel = type === 'error' ? 'Error' : type === 'warning' ? 'Warning' : type === 'success' ? 'Success' : 'Info';
      import('@tauri-apps/plugin-notification').then(({ isPermissionGranted, requestPermission, sendNotification }) => {
        isPermissionGranted()
          .then((granted) => {
            if (!granted) return requestPermission();
            return 'granted' as const;
          })
          .then((result) => {
            if (result === 'granted') {
              sendNotification({ title: `MeedyaDL — ${typeLabel}`, body: message });
            } else {
              console.warn(
                `[notification] permission not granted (status: ${result}). ` +
                'Open System Settings > Notifications > MeedyaDL to enable.',
              );
            }
          })
          .catch((err: unknown) => {
            console.warn('[notification] sendNotification failed:', err);
          });
      }).catch((err: unknown) => {
        console.warn('[notification] plugin module failed to load:', err);
      });
    }

    // Skip in-app toast when user prefers native-only notifications.
    if (style === 'native_only') return;

    // Generate a collision-resistant unique ID for this toast.
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;

    // Compute the absolute auto-dismiss deadline. `null` means
    // "never auto-dismiss" — error and warning toasts default to
    // duration 0 (persistent) per the resolution above.
    // #894: the deadline is stored on the toast itself + a single
    // centralised worker polls it; no per-toast setTimeout closures.
    const expiresAt = duration > 0 ? Date.now() + duration : null;

    set((state) => {
      // Dedup: skip if a toast with the same message is already on screen.
      if (state.toasts.some((t) => t.message === message)) {
        return state;
      }

      // If a key is provided, remove any existing toast with the same key
      // (replacement behaviour — only one toast per key at a time).
      const filtered = key ? state.toasts.filter((t) => t.key !== key) : state.toasts;

      return {
        toasts: [
          ...filtered,
          { id, message, type, duration, expiresAt, key, action },
        ],
      };
    });

    // Kick the centralised dismissal worker if this toast has a
    // deadline. The worker is a no-op until a non-persistent toast
    // exists; once started it stops itself when none remain.
    if (expiresAt !== null) {
      ensureDismissalWorker();
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

  /**
   * Remove all toasts matching a given key. Used to dismiss toasts when
   * the condition that caused them resolves (e.g., wrapper becomes reachable).
   */
  removeToastsByKey: (key) =>
    set((state) => ({
      toasts: state.toasts.filter((t) => t.key !== key),
    })),
}));
