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
  setToastDismissalPaused,
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
  // #1431: the backend's one notification rule. Default answer: not sent,
  // keep the in-app copy.
  notifySystem: vi.fn().mockResolvedValue({ sent: false, in_app: true }),
}));

/**
 * Mock the Tauri notification plugin.
 *
 * Since #1431 `addToast` never touches this plugin -- system notifications
 * are the backend's job. The mock stays so the tests can prove that.
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
  vi.mocked(commands.notifySystem).mockReset();
  vi.mocked(commands.notifySystem).mockResolvedValue({ sent: false, in_app: true });
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
          strip(before as unknown as Record<string, unknown>)
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
    // These tests are about the in-app toast list only, so they run with
    // the style set to in-app only. (The backend is mocked anyway, and
    // answers "not sent", so no toast is ever taken away here.)
    beforeEach(() => {
      useSettingsStore.setState((state) => ({
        settings: { ...state.settings, notification_style: 'in_app_only' },
      }));
    });

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
  // System notifications (#1431)
  //
  // `addToast` asks the backend's one notification rule (`notifySystem`),
  // which reads the SAVED settings and whether the person is looking at
  // MeedyaDL, and sends any system notification itself. The page never
  // calls the notification plugin. These tests replace ones that mocked
  // plugin behaviour that cannot happen on desktop (a permission refusal,
  // a throwing send): there the plugin always says "granted" and never
  // reports a failed send.
  // =========================================================================
  describe('addToast — system notifications', () => {
    const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

    it('never calls the notification plugin directly, whatever the style', async () => {
      for (const style of ['in_app_only', 'native_and_in_app', 'native_only'] as const) {
        useSettingsStore.setState((state) => ({
          settings: { ...state.settings, notification_style: style },
        }));
        useUiStore.getState().addToast(`Message for ${style}`, 'info');
        useUiStore.getState().addToast(`Error for ${style}`, 'error');
      }
      await flush();
      expect(notificationPlugin.sendNotification).not.toHaveBeenCalled();
      expect(notificationPlugin.isPermissionGranted).not.toHaveBeenCalled();
      expect(notificationPlugin.requestPermission).not.toHaveBeenCalled();
    });

    it('asks the backend with the right kind: general, error, or the one the caller names', async () => {
      useUiStore.getState().addToast('Saved', 'success');
      useUiStore.getState().addToast('Broke', 'error');
      useUiStore
        .getState()
        .addToast('Copied link', 'info', 10000, 'clipboard-url', undefined, undefined, {
          kind: 'clipboard_link',
          systemTitle: 'Copied link found',
          systemBody: 'Open MeedyaDL to add it.',
        });
      await flush();
      expect(commands.notifySystem).toHaveBeenCalledWith('general', 'MeedyaDL', 'Saved');
      expect(commands.notifySystem).toHaveBeenCalledWith(
        'error',
        'MeedyaDL: something went wrong',
        'Broke'
      );
      expect(commands.notifySystem).toHaveBeenCalledWith(
        'clipboard_link',
        'Copied link found',
        'Open MeedyaDL to add it.'
      );
    });

    it('does not ask again for a message already on screen (#993)', async () => {
      useUiStore.getState().addToast('Same message', 'warning');
      useUiStore.getState().addToast('Same message', 'warning');
      await flush();
      expect(commands.notifySystem).toHaveBeenCalledTimes(1);
      expect(useUiStore.getState().toasts).toHaveLength(1);
    });

    it('does not ask at all when the caller says the system half is already decided', async () => {
      useUiStore
        .getState()
        .addToast('From the backend', 'success', undefined, undefined, undefined, undefined, {
          kind: 'download_complete',
          system: false,
        });
      await flush();
      expect(commands.notifySystem).not.toHaveBeenCalled();
      expect(useUiStore.getState().toasts.map((t) => t.message)).toEqual(['From the backend']);
    });

    it('in "System only", removes a plain message once the system has shown it', async () => {
      vi.mocked(commands.notifySystem).mockResolvedValue({ sent: true, in_app: false });
      useUiStore.getState().addToast('Plain note', 'info');
      await vi.waitFor(() => {
        expect(useUiStore.getState().toasts).toHaveLength(0);
      });
    });

    it('in "System only", keeps anything a system notification cannot carry or that must stay', async () => {
      vi.mocked(commands.notifySystem).mockResolvedValue({ sent: true, in_app: false });
      const ui = useUiStore.getState();
      ui.addToast('Has a button', 'info', 5000, undefined, { label: 'Go', onClick: () => {} });
      ui.addToast('Has details', 'info', 5000, undefined, undefined, 'technical text');
      ui.addToast('Has a key', 'info', 5000, 'some-key');
      ui.addToast('Persistent', 'info', 0);
      ui.addToast('An error', 'error');
      ui.addToast('A warning', 'warning');
      await flush();
      expect(useUiStore.getState().toasts.map((t) => t.message)).toEqual([
        'Has a button',
        'Has details',
        'Has a key',
        'Persistent',
        'An error',
        'A warning',
      ]);
    });

    it('keeps a message the system notification points to until it is dismissed, once sent', async () => {
      vi.mocked(commands.notifySystem).mockResolvedValue({ sent: true, in_app: true });
      useUiStore
        .getState()
        .addToast('Copied link', 'info', 10000, 'clipboard-url', undefined, undefined, {
          kind: 'clipboard_link',
          keepUntilDismissedIfSent: true,
        });
      await vi.waitFor(() => {
        expect(useUiStore.getState().toasts[0].expiresAt).toBeNull();
      });
      expect(useUiStore.getState().toasts[0].duration).toBe(0);
    });

    it('lets such a message time out as usual when the system copy was not sent', async () => {
      vi.mocked(commands.notifySystem).mockResolvedValue({ sent: false, in_app: true });
      useUiStore
        .getState()
        .addToast('Copied link', 'info', 10000, 'clipboard-url', undefined, undefined, {
          kind: 'clipboard_link',
          keepUntilDismissedIfSent: true,
        });
      await flush();
      expect(useUiStore.getState().toasts[0].expiresAt).not.toBeNull();
    });

    it('keeps the message in the app when the backend did not send it', async () => {
      vi.mocked(commands.notifySystem).mockResolvedValue({ sent: false, in_app: true });
      useUiStore.getState().addToast('Window is in front', 'info');
      await flush();
      expect(useUiStore.getState().toasts.map((t) => t.message)).toEqual(['Window is in front']);
    });

    it('keeps the message, and adds nothing, when asking the backend fails', async () => {
      vi.mocked(commands.notifySystem).mockRejectedValue(new Error('no backend'));
      useUiStore.getState().addToast('Still here', 'success');
      await flush();
      // No second "could not notify" message: a failure is never turned
      // into another message (that is how a loop used to be possible).
      expect(useUiStore.getState().toasts.map((t) => t.message)).toEqual(['Still here']);
    });

    it('never changes a saved setting', () => {
      useSettingsStore.setState((state) => ({
        settings: {
          ...state.settings,
          desktop_notifications: false,
          notification_style: 'native_only',
        },
      }));
      useUiStore.getState().addToast('Does not rewrite settings', 'info');
      expect(useSettingsStore.getState().settings.notification_style).toBe('native_only');
      expect(useSettingsStore.getState().settings.desktop_notifications).toBe(false);
    });
  });

  // =========================================================================
  // Auto-dismiss waits while the person is reading (#1431)
  // =========================================================================
  describe('addToast — auto-dismiss pause', () => {
    it('extends a toast that arrived during the pause only from its arrival', () => {
      vi.useFakeTimers();
      try {
        setToastDismissalPaused(true);
        vi.advanceTimersByTime(5000);
        useUiStore.getState().addToast('Arrived late', 'info', 1000);
        const before = useUiStore.getState().toasts[0].expiresAt as number;
        vi.advanceTimersByTime(500);
        setToastDismissalPaused(false);
        // 500 ms paused since it arrived -- not the 5.5 s since the pause began.
        expect(useUiStore.getState().toasts[0].expiresAt).toBe(before + 500);
      } finally {
        setToastDismissalPaused(false);
        vi.useRealTimers();
      }
    });

    it('keeps a toast past its deadline while paused, then gives back the time it had left', () => {
      vi.useFakeTimers();
      try {
        useUiStore.getState().addToast('Reading this', 'info', 1000);
        vi.advanceTimersByTime(500);
        setToastDismissalPaused(true);
        vi.advanceTimersByTime(5000);
        expect(useUiStore.getState().toasts).toHaveLength(1);
        setToastDismissalPaused(false);
        vi.advanceTimersByTime(250);
        expect(useUiStore.getState().toasts).toHaveLength(1);
        vi.advanceTimersByTime(500);
        expect(useUiStore.getState().toasts).toHaveLength(0);
      } finally {
        setToastDismissalPaused(false);
        vi.useRealTimers();
      }
    });
  });

  // =========================================================================
  // Toast list ceiling
  // =========================================================================
  describe('addToast — list ceiling (MAX_TOASTS)', () => {
    // These tests are about the in-app toast list only. Run them with
    // system notifications off: firing dozens of toasts at once also fires
    // dozens of system-notification attempts, whose mocked dynamic imports
    // race in Vitest (see the sequencing note in the dedup tests below).
    // Those failed attempts used to vanish into a console warning; since a
    // failed system notification now shows a notice in the app, one would
    // otherwise land in whichever test happened to run next.
    beforeEach(() => {
      useSettingsStore.setState((state) => ({
        settings: { ...state.settings, notification_style: 'in_app_only' },
      }));
    });

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
      expect(messages).toEqual(Array.from({ length: MAX_TOASTS }, (_, i) => `Failure ${i + 1}`));
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
