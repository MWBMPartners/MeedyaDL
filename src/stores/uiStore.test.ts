/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/stores/uiStore.test.ts - Unit tests for the UI state store
 *
 * Tests the uiStore's page navigation, sidebar collapse toggle,
 * toast notification lifecycle, and setup wizard visibility management.
 *
 * These tests exercise Zustand stores directly (no React rendering needed)
 * by calling `useUiStore.getState()` to read state and invoking actions
 * via `useUiStore.getState().actionName()`.
 *
 * @see src/stores/uiStore.ts - The store under test
 * @see {@link https://zustand.docs.pmnd.rs/guides/testing} - Zustand testing patterns
 */

import { useUiStore, __resetToastWorkerForTests } from '@/stores/uiStore';
import { useSettingsStore } from '@/stores/settingsStore';
import * as notificationPlugin from '@tauri-apps/plugin-notification';

/**
 * Mock the Tauri notification plugin (#993).
 *
 * `addToast` dynamically imports this module (`import('@tauri-apps/plugin-notification')`)
 * to send native OS notifications when `notification_style` includes 'native'.
 * `vi.mock()` calls are hoisted by Vitest to the top of the module, so this
 * mock is already registered by the time that dynamic import executes --
 * hoisting applies to dynamic `import()` the same way it does to static
 * `import` declarations.
 *
 * Default: permission already granted, so `sendNotification` is reachable.
 */
vi.mock('@tauri-apps/plugin-notification', () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(true),
  requestPermission: vi.fn().mockResolvedValue('granted'),
  sendNotification: vi.fn(),
}));

/**
 * Reset the store to its initial state before each test.
 * Zustand stores are singletons, so mutations from one test would
 * leak into the next without this reset.
 */
beforeEach(() => {
  useUiStore.setState({
    currentPage: 'download',
    sidebarCollapsed: false,
    toasts: [],
    showSetupWizard: false,
  });
  // #894: the centralised dismissal worker is module-scoped state;
  // reset it between tests so each starts from a clean baseline.
  __resetToastWorkerForTests();
  // #993: clear notification-plugin spy call history (but keep the
  // mockResolvedValue/mockReturnValue implementations above intact --
  // mockClear() only resets .mock.calls/.mock.results, not the implementation).
  vi.mocked(notificationPlugin.isPermissionGranted).mockClear();
  vi.mocked(notificationPlugin.requestPermission).mockClear();
  vi.mocked(notificationPlugin.sendNotification).mockClear();
  // Reset settings to a known baseline (native + in-app) so notification
  // tests aren't affected by a prior test's `useSettingsStore.setState()`.
  useSettingsStore.setState((state) => ({
    settings: { ...state.settings, notification_style: 'native_and_in_app' },
  }));
});

describe('uiStore', () => {
  // =========================================================================
  // Initial State
  // =========================================================================
  describe('initial state', () => {
    it('starts on the download page', () => {
      expect(useUiStore.getState().currentPage).toBe('download');
    });

    it('starts with sidebar expanded', () => {
      expect(useUiStore.getState().sidebarCollapsed).toBe(false);
    });

    it('starts with no toasts', () => {
      expect(useUiStore.getState().toasts).toEqual([]);
    });

    it('starts with setup wizard hidden', () => {
      expect(useUiStore.getState().showSetupWizard).toBe(false);
    });
  });

  // =========================================================================
  // Page Navigation
  // =========================================================================
  describe('setPage', () => {
    it('navigates to the queue page', () => {
      useUiStore.getState().setPage('queue');
      expect(useUiStore.getState().currentPage).toBe('queue');
    });

    it('navigates to the settings page', () => {
      useUiStore.getState().setPage('settings');
      expect(useUiStore.getState().currentPage).toBe('settings');
    });

    it('navigates to the help page', () => {
      useUiStore.getState().setPage('help');
      expect(useUiStore.getState().currentPage).toBe('help');
    });

    it('navigates back to the download page', () => {
      useUiStore.getState().setPage('settings');
      useUiStore.getState().setPage('download');
      expect(useUiStore.getState().currentPage).toBe('download');
    });
  });

  // =========================================================================
  // Sidebar Collapse
  // =========================================================================
  describe('toggleSidebar', () => {
    it('collapses the sidebar on first toggle', () => {
      useUiStore.getState().toggleSidebar();
      expect(useUiStore.getState().sidebarCollapsed).toBe(true);
    });

    it('expands the sidebar on second toggle', () => {
      useUiStore.getState().toggleSidebar();
      useUiStore.getState().toggleSidebar();
      expect(useUiStore.getState().sidebarCollapsed).toBe(false);
    });
  });

  describe('setSidebarCollapsed', () => {
    it('explicitly collapses the sidebar', () => {
      useUiStore.getState().setSidebarCollapsed(true);
      expect(useUiStore.getState().sidebarCollapsed).toBe(true);
    });

    it('explicitly expands the sidebar', () => {
      useUiStore.getState().setSidebarCollapsed(true);
      useUiStore.getState().setSidebarCollapsed(false);
      expect(useUiStore.getState().sidebarCollapsed).toBe(false);
    });
  });

  // =========================================================================
  // Setup Wizard
  // =========================================================================
  describe('setShowSetupWizard', () => {
    it('shows the setup wizard', () => {
      useUiStore.getState().setShowSetupWizard(true);
      expect(useUiStore.getState().showSetupWizard).toBe(true);
    });

    it('hides the setup wizard', () => {
      useUiStore.getState().setShowSetupWizard(true);
      useUiStore.getState().setShowSetupWizard(false);
      expect(useUiStore.getState().showSetupWizard).toBe(false);
    });
  });

  // =========================================================================
  // Toast Notifications
  // =========================================================================
  describe('addToast', () => {
    it('adds a success toast to the queue', () => {
      useUiStore.getState().addToast('Download complete', 'success');

      const { toasts } = useUiStore.getState();
      expect(toasts).toHaveLength(1);
      expect(toasts[0].message).toBe('Download complete');
      expect(toasts[0].type).toBe('success');
    });

    it('adds an error toast to the queue', () => {
      useUiStore.getState().addToast('Network error', 'error');

      const { toasts } = useUiStore.getState();
      expect(toasts).toHaveLength(1);
      expect(toasts[0].type).toBe('error');
    });

    it('generates a unique ID for each toast', () => {
      useUiStore.getState().addToast('First', 'info');
      useUiStore.getState().addToast('Second', 'info');

      const { toasts } = useUiStore.getState();
      expect(toasts).toHaveLength(2);
      expect(toasts[0].id).not.toBe(toasts[1].id);
    });

    it('appends toasts in order', () => {
      useUiStore.getState().addToast('First', 'info');
      useUiStore.getState().addToast('Second', 'warning');
      useUiStore.getState().addToast('Third', 'error');

      const messages = useUiStore.getState().toasts.map((t) => t.message);
      expect(messages).toEqual(['First', 'Second', 'Third']);
    });

    it('uses the specified duration', () => {
      useUiStore.getState().addToast('Custom duration', 'info', 10000);

      const { toasts } = useUiStore.getState();
      expect(toasts[0].duration).toBe(10000);
    });

    it('defaults duration to 5000ms', () => {
      useUiStore.getState().addToast('Default duration', 'info');

      const { toasts } = useUiStore.getState();
      expect(toasts[0].duration).toBe(5000);
    });

    it('auto-dismisses after the specified duration', () => {
      vi.useFakeTimers();

      useUiStore.getState().addToast('Temporary', 'info', 3000);
      expect(useUiStore.getState().toasts).toHaveLength(1);

      // Advance past the deadline + one worker tick (250 ms) so the
      // centralised dismissal worker has had a chance to scan.
      vi.advanceTimersByTime(3500);
      expect(useUiStore.getState().toasts).toHaveLength(0);

      vi.useRealTimers();
    });

    it('does not auto-dismiss when duration is 0 (persistent toast)', () => {
      vi.useFakeTimers();

      useUiStore.getState().addToast('Persistent', 'error', 0);
      vi.advanceTimersByTime(60000); // Wait a full minute

      /* Toast should still be present */
      expect(useUiStore.getState().toasts).toHaveLength(1);

      vi.useRealTimers();
    });

    // #894: centralised dismissal-worker tests
    it('records expiresAt deadline on auto-dismissable toasts', () => {
      const before = Date.now();
      useUiStore.getState().addToast('Tracked', 'info', 5000);
      const after = Date.now();
      const t = useUiStore.getState().toasts[0];
      expect(t.expiresAt).not.toBeNull();
      expect(t.expiresAt!).toBeGreaterThanOrEqual(before + 5000);
      expect(t.expiresAt!).toBeLessThanOrEqual(after + 5000);
    });

    it('records expiresAt=null on persistent toasts', () => {
      useUiStore.getState().addToast('Sticky', 'error', 0);
      const t = useUiStore.getState().toasts[0];
      expect(t.expiresAt).toBeNull();
    });

    it('shares a single dismissal worker across many toasts', () => {
      vi.useFakeTimers();

      // Fire 50 toasts in rapid succession; pre-#894 this would
      // schedule 50 individual setTimeouts. Post-#894 the worker
      // is a single setInterval — the count of pending timers stays
      // bounded regardless of how many toasts are queued.
      for (let i = 0; i < 50; i += 1) {
        useUiStore.getState().addToast(`msg ${i}`, 'info', 1000);
      }
      expect(useUiStore.getState().toasts).toHaveLength(50);

      // Advance past expiry + worker tick; all should be cleared.
      vi.advanceTimersByTime(1500);
      expect(useUiStore.getState().toasts).toHaveLength(0);

      vi.useRealTimers();
    });

    it('worker stops once no expiring toasts remain', () => {
      vi.useFakeTimers();

      useUiStore.getState().addToast('temp', 'info', 1000);
      vi.advanceTimersByTime(1500); // toast expires, worker scans, list empty
      expect(useUiStore.getState().toasts).toHaveLength(0);

      // Add a persistent toast. The worker should NOT restart for
      // it (persistent toasts have no deadline; the worker would
      // only spin idly).
      useUiStore.getState().addToast('forever', 'error', 0);

      // Now add a fresh expiring toast — the worker should pick
      // this up on the next tick.
      useUiStore.getState().addToast('temp2', 'info', 500);
      vi.advanceTimersByTime(800); // past temp2's deadline + tick
      const messages = useUiStore.getState().toasts.map((t) => t.message);
      expect(messages).toEqual(['forever']);

      vi.useRealTimers();
    });
  });

  // =========================================================================
  // Native OS Notification Dedup (#993)
  // =========================================================================
  describe('addToast — native notification dedup', () => {
    it('sends exactly one native notification for a duplicate in-app message', async () => {
      useSettingsStore.setState((state) => ({
        settings: { ...state.settings, notification_style: 'native_and_in_app' },
      }));

      useUiStore.getState().addToast('Same message', 'warning');
      useUiStore.getState().addToast('Same message', 'warning');

      // The native send is fired-and-forget inside async promise chains
      // (dynamic import -> isPermissionGranted -> requestPermission ->
      // sendNotification), so poll until they settle instead of asserting
      // synchronously.
      await vi.waitFor(() => {
        expect(notificationPlugin.sendNotification).toHaveBeenCalledTimes(1);
      });

      // The in-app dedup (pre-existing behaviour, `state.toasts.some(...)`
      // in the `set()` updater) should still leave exactly one toast.
      const { toasts } = useUiStore.getState();
      expect(toasts).toHaveLength(1);
      expect(toasts[0].message).toBe('Same message');
    });

    it('still sends a native notification for two distinct messages', async () => {
      useSettingsStore.setState((state) => ({
        settings: { ...state.settings, notification_style: 'native_and_in_app' },
      }));

      useUiStore.getState().addToast('First message', 'info');
      // Let the first call's fire-and-forget dynamic import
      // (`import('@tauri-apps/plugin-notification')`) settle before firing
      // the second. Two same-specifier dynamic imports issued in the same
      // tick race in Vitest's mock resolution (a test-harness quirk, not a
      // real-app concern -- the browser/Tauri runtime has no such race);
      // sequencing them here keeps the assertion meaningful instead of flaky.
      await vi.waitFor(() => {
        expect(notificationPlugin.sendNotification).toHaveBeenCalledTimes(1);
      });

      useUiStore.getState().addToast('Second message', 'info');
      await vi.waitFor(() => {
        expect(notificationPlugin.sendNotification).toHaveBeenCalledTimes(2);
      });

      expect(useUiStore.getState().toasts).toHaveLength(2);
    });

    it('sends a native notification for every repeat in native_only mode (state-based dedup is impossible)', async () => {
      useSettingsStore.setState((state) => ({
        settings: { ...state.settings, notification_style: 'native_only' },
      }));

      useUiStore.getState().addToast('Repeated native-only', 'warning');
      // See the sequencing note above -- avoid firing two same-specifier
      // dynamic imports in the same tick.
      await vi.waitFor(() => {
        expect(notificationPlugin.sendNotification).toHaveBeenCalledTimes(1);
      });

      useUiStore.getState().addToast('Repeated native-only', 'warning');

      // native_only never populates the in-app `toasts` array, so
      // `isDuplicateInApp` can never be true -- the carve-out in the
      // gate must let both sends through.
      await vi.waitFor(() => {
        expect(notificationPlugin.sendNotification).toHaveBeenCalledTimes(2);
      });

      expect(useUiStore.getState().toasts).toHaveLength(0);
    });

    it('does not send a native notification in in_app_only mode', async () => {
      useSettingsStore.setState((state) => ({
        settings: { ...state.settings, notification_style: 'in_app_only' },
      }));

      useUiStore.getState().addToast('In-app only', 'info');

      // Give any stray microtasks a chance to run, then assert the spy
      // was never invoked (nothing to wait FOR here, so a short real-timer
      // flush is used instead of vi.waitFor).
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(notificationPlugin.sendNotification).not.toHaveBeenCalled();
      expect(useUiStore.getState().toasts).toHaveLength(1);
    });
  });

  describe('removeToast', () => {
    it('removes a specific toast by ID', () => {
      useUiStore.getState().addToast('Keep me', 'info', 0);
      useUiStore.getState().addToast('Remove me', 'error', 0);

      const toastToRemove = useUiStore.getState().toasts[1];
      useUiStore.getState().removeToast(toastToRemove.id);

      const { toasts } = useUiStore.getState();
      expect(toasts).toHaveLength(1);
      expect(toasts[0].message).toBe('Keep me');
    });

    it('does nothing when removing a non-existent ID', () => {
      useUiStore.getState().addToast('Keep me', 'info', 0);
      useUiStore.getState().removeToast('non-existent-id');

      expect(useUiStore.getState().toasts).toHaveLength(1);
    });
  });
});
