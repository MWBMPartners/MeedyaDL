/**
 * Copyright (c) 2024-2026 MeedyaDL
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

import { useUiStore } from '@/stores/uiStore';

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

      /* Advance time past the auto-dismiss duration */
      vi.advanceTimersByTime(3000);
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
