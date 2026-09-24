/**
 * Copyright (c) 2024-2026 MeedyaSuite
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

import {
  useUiStore,
  __resetToastWorkerForTests,
  __resetSidebarSaveForTests,
  MAX_TOASTS,
} from '@/stores/uiStore';
import { useSettingsStore } from '@/stores/settingsStore';
import * as commands from '@/lib/tauri-commands';
import * as notificationPlugin from '@tauri-apps/plugin-notification';

/**
 * Mock the backend command wrappers (#1175).
 *
 * Toggling the sidebar now writes the new position to disk. There is no
 * backend in a unit test, so every wrapper the store might reach for is
 * replaced with a spy. `saveSettings` is mocked as well as the one the
 * sidebar actually uses, precisely so a test can assert it was NEVER
 * called -- see "saves only the new boolean" below.
 */
vi.mock('@/lib/tauri-commands', () => ({
  getSettings: vi.fn(),
  saveSettings: vi.fn().mockResolvedValue(undefined),
  saveSidebarCollapsed: vi.fn().mockResolvedValue(undefined),
}));

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
  // #1175: the pending sidebar write is module-scoped in the same way.
  // Throw it away so a toggle in one test cannot fire during the next.
  __resetSidebarSaveForTests();
  // #1175: forget which backend calls the previous test made, so the
  // "was never called" assertions mean this test and not the whole file.
  vi.mocked(commands.saveSidebarCollapsed).mockClear();
  vi.mocked(commands.saveSettings).mockClear();
  // #993: clear notification-plugin spy call history (but keep the
  // mockResolvedValue/mockReturnValue implementations above intact --
  // mockClear() only resets .mock.calls/.mock.results, not the implementation).
  vi.mocked(notificationPlugin.isPermissionGranted).mockClear();
  vi.mocked(notificationPlugin.requestPermission).mockClear();
  vi.mocked(notificationPlugin.sendNotification).mockClear();
  // Reset settings to a known baseline (master switch on, style native +
  // in-app) so notification tests aren't affected by a prior test's
  // `useSettingsStore.setState()`. Both fields are reset here, not just
  // `notification_style` -- a test that turns `desktop_notifications` off
  // and forgets to put it back would otherwise leak that into every test
  // that runs after it, since `setState` here only spreads over whatever
  // the previous test left behind.
  useSettingsStore.setState((state) => ({
    settings: {
      ...state.settings,
      desktop_notifications: true,
      notification_style: 'native_and_in_app',
    },
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

    /*
     * The four tests below exist because remembering the sidebar has been
     * tried twice before and backed out twice, both times for the same
     * reason: the save was wider than the click. They are written to go
     * red against those two specific mistakes, not just against "the
     * feature is missing".
     */

    it('never marks the settings screen as having unsaved edits', () => {
      // The first backed-out attempt called updateSettings(), which sets
      // isDirty. That tells the person they have unsaved work and arms
      // the "Save Changes" button -- over a change they did not make on
      // that screen, and which is already saved by another route.
      vi.useFakeTimers();
      try {
        useSettingsStore.setState({ isDirty: false });

        useUiStore.getState().toggleSidebar();
        vi.advanceTimersByTime(1000);

        expect(useSettingsStore.getState().isDirty).toBe(false);
      } finally {
        vi.useRealTimers();
      }
    });

    it('changes no setting other than the sidebar position', () => {
      vi.useFakeTimers();
      try {
        const before = { ...useSettingsStore.getState().settings };

        useUiStore.getState().toggleSidebar();
        vi.advanceTimersByTime(1000);

        const after = useSettingsStore.getState().settings;

        // The sidebar's own field is expected to move; nothing else is.
        expect(after.sidebar_collapsed).toBe(true);
        const strip = (s: Record<string, unknown>) => {
          const copy = { ...s };
          delete copy.sidebar_collapsed;
          return copy;
        };
        expect(strip(after as unknown as Record<string, unknown>)).toEqual(
          strip(before as unknown as Record<string, unknown>),
        );
      } finally {
        vi.useRealTimers();
      }
    });

    it('saves only the new boolean, never the whole settings object', () => {
      // The second backed-out attempt called the settings store's
      // debouncedSave(), which writes ALL of the settings from memory --
      // so one click on the arrow could commit half-finished Settings
      // edits over the file. That method has since been deleted; this
      // checks the write that replaced it stays narrow.
      vi.useFakeTimers();
      try {
        useUiStore.getState().toggleSidebar();
        vi.advanceTimersByTime(1000);

        expect(commands.saveSidebarCollapsed).toHaveBeenCalledTimes(1);
        expect(commands.saveSidebarCollapsed).toHaveBeenCalledWith(true);
        expect(commands.saveSettings).not.toHaveBeenCalled();
      } finally {
        vi.useRealTimers();
      }
    });

    it('turns a burst of clicks into one save carrying the final position', () => {
      // Five clicks from expanded ends collapsed: true, false, true,
      // false, true. One write, and it must carry the value from the
      // last click -- not whatever some store held when the timer fired.
      vi.useFakeTimers();
      try {
        for (let i = 0; i < 5; i += 1) {
          useUiStore.getState().toggleSidebar();
        }
        vi.advanceTimersByTime(1000);

        expect(commands.saveSidebarCollapsed).toHaveBeenCalledTimes(1);
        expect(commands.saveSidebarCollapsed).toHaveBeenCalledWith(true);
        expect(useUiStore.getState().sidebarCollapsed).toBe(true);
      } finally {
        vi.useRealTimers();
      }
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
      //
      // The MAX_TOASTS ceiling added later means the array itself
      // holds at most MAX_TOASTS entries at any one time (the oldest
      // are dropped as new ones arrive) rather than growing to 50 --
      // that is the OTHER fix's job and is covered by its own tests
      // above. What THIS test still needs to prove is unchanged: that
      // firing many toasts uses one shared worker rather than 50
      // individual timers, and that the worker still clears everything
      // once it all expires.
      for (let i = 0; i < 50; i += 1) {
        useUiStore.getState().addToast(`msg ${i}`, 'info', 1000);
      }
      expect(useUiStore.getState().toasts).toHaveLength(MAX_TOASTS);

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

  // =========================================================================
  // Desktop Notifications master switch
  //
  // `desktop_notifications` is the on/off toggle shown in Settings >
  // General; `notification_style` is the dropdown underneath it that only
  // appears -- and can only be changed -- while the toggle is on. Before
  // this fix, `addToast` looked only at `notification_style` and never at
  // `desktop_notifications`, so turning the toggle off did not stop a
  // single OS banner. These tests go red against that specific mistake,
  // not just against "the switch is missing" -- each one turns the switch
  // off and checks the native path stayed off, the same way the sidebar
  // tests above go red against the two ways remembering the sidebar broke
  // before.
  // =========================================================================
  describe('addToast — desktop notifications master switch', () => {
    it('sends no native notification when the switch is off, even though the style is "native + in-app"', async () => {
      useSettingsStore.setState((state) => ({
        settings: {
          ...state.settings,
          desktop_notifications: false,
          notification_style: 'native_and_in_app',
        },
      }));

      useUiStore.getState().addToast('Switch is off', 'warning');

      // Nothing to wait FOR here (the assertion is that nothing ever
      // fires), so give stray microtasks a moment to run instead of
      // asserting synchronously.
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(notificationPlugin.sendNotification).not.toHaveBeenCalled();

      // The switch only concerns the native OS banner. The in-app toast
      // is unaffected and must still appear.
      const { toasts } = useUiStore.getState();
      expect(toasts).toHaveLength(1);
      expect(toasts[0].message).toBe('Switch is off');
    });

    it('still shows an in-app toast when the switch is off and the saved style is "native only" -- otherwise the message reaches the user nowhere at all', async () => {
      useSettingsStore.setState((state) => ({
        settings: {
          ...state.settings,
          desktop_notifications: false,
          notification_style: 'native_only',
        },
      }));

      useUiStore.getState().addToast('Would otherwise vanish', 'error');

      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(notificationPlugin.sendNotification).not.toHaveBeenCalled();

      // Before the fix, "native only" always returned early and skipped
      // the in-app toast. Combined with the switch being off (so no OS
      // banner either), that message reached the user nowhere -- silently.
      // This is the exact case the fix exists to prevent.
      const { toasts } = useUiStore.getState();
      expect(toasts).toHaveLength(1);
      expect(toasts[0].message).toBe('Would otherwise vanish');
    });

    it('leaves the saved notification_style untouched -- the switch-off behaviour only changes what this one call does', () => {
      useSettingsStore.setState((state) => ({
        settings: {
          ...state.settings,
          desktop_notifications: false,
          notification_style: 'native_only',
        },
      }));

      useUiStore.getState().addToast('Does not rewrite settings', 'info');

      expect(useSettingsStore.getState().settings.notification_style).toBe('native_only');
    });

    it('still sends the native notification when the switch is on and the style is "native only" (unaffected by this fix)', async () => {
      useSettingsStore.setState((state) => ({
        settings: {
          ...state.settings,
          desktop_notifications: true,
          notification_style: 'native_only',
        },
      }));

      useUiStore.getState().addToast('Switch is on', 'warning');

      await vi.waitFor(() => {
        expect(notificationPlugin.sendNotification).toHaveBeenCalledTimes(1);
      });
      // "Native only" with the switch on keeps its pre-existing meaning:
      // no in-app toast.
      expect(useUiStore.getState().toasts).toHaveLength(0);
    });
  });

  // =========================================================================
  // Toast list ceiling
  // =========================================================================
  describe('addToast — list ceiling (MAX_TOASTS)', () => {
    it('keeps only the newest MAX_TOASTS entries once the list overflows, dropping the oldest first', () => {
      // Every message here is different, so the message-based dedup a few
      // lines up in `addToast` does nothing -- and these are persistent
      // (duration 0) error toasts, so nothing ages them out either. This
      // is exactly the run-of-differently-worded-failures shape the cap
      // exists for.
      const total = MAX_TOASTS + 1;
      for (let i = 0; i < total; i += 1) {
        useUiStore.getState().addToast(`Failure ${i}`, 'error', 0);
      }

      const { toasts } = useUiStore.getState();
      expect(toasts).toHaveLength(MAX_TOASTS);

      // "Failure 0" was the oldest and should have been dropped; the
      // newest MAX_TOASTS entries remain, still in the order they arrived.
      const messages = toasts.map((t) => t.message);
      expect(messages).not.toContain('Failure 0');
      expect(messages).toEqual(
        Array.from({ length: MAX_TOASTS }, (_, i) => `Failure ${i + 1}`),
      );
    });

    it('drops a message that would go away by itself before a lasting one', () => {
      // A lasting error, then passing notes, then one more to overflow.
      // The lasting error stays: it is there because the person has to act
      // on it. The oldest PASSING note goes instead (stand-in review, 24
      // Sept 2026 -- the plain oldest used to be dropped, whatever it was).
      useUiStore.getState().addToast('Nothing will happen after the queue', 'error', 0);
      for (let i = 0; i < MAX_TOASTS; i += 1) {
        useUiStore.getState().addToast(`Note ${i}`, 'info', 5000);
      }

      const messages = useUiStore.getState().toasts.map((t) => t.message);
      expect(messages).toHaveLength(MAX_TOASTS);
      expect(messages).toContain('Nothing will happen after the queue');
      expect(messages).not.toContain('Note 0');
      expect(messages[messages.length - 1]).toBe(`Note ${MAX_TOASTS - 1}`);
    });

    it('does not trim the list while it is at or under the ceiling', () => {
      for (let i = 0; i < MAX_TOASTS; i += 1) {
        useUiStore.getState().addToast(`Item ${i}`, 'info', 0);
      }

      expect(useUiStore.getState().toasts).toHaveLength(MAX_TOASTS);
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
