/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * UI state store.
 * Manages transient UI state: current page, sidebar collapse, toasts,
 * and modal visibility. This state is not persisted across sessions.
 */

import { create } from 'zustand';
import type { AppPage, Toast, ToastType } from '@/types';

interface UiState {
  /** Currently active navigation page */
  currentPage: AppPage;
  /** Whether the sidebar is collapsed (narrow mode) */
  sidebarCollapsed: boolean;
  /** Active toast notifications displayed at the top of the screen */
  toasts: Toast[];
  /** Whether the setup wizard should be shown */
  showSetupWizard: boolean;

  /** Navigate to a different page */
  setPage: (page: AppPage) => void;
  /** Toggle sidebar collapsed state */
  toggleSidebar: () => void;
  /** Set sidebar collapsed state explicitly */
  setSidebarCollapsed: (collapsed: boolean) => void;
  /** Show/hide the setup wizard */
  setShowSetupWizard: (show: boolean) => void;
  /** Add a toast notification */
  addToast: (message: string, type: ToastType, duration?: number) => void;
  /** Remove a specific toast by ID */
  removeToast: (id: string) => void;
}

export const useUiStore = create<UiState>((set) => ({
  currentPage: 'download',
  sidebarCollapsed: false,
  toasts: [],
  showSetupWizard: false,

  setPage: (page) => set({ currentPage: page }),

  toggleSidebar: () =>
    set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),

  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),

  setShowSetupWizard: (show) => set({ showSetupWizard: show }),

  addToast: (message, type, duration = 5000) => {
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
    set((state) => ({
      toasts: [...state.toasts, { id, message, type, duration }],
    }));

    // Auto-dismiss after the specified duration
    if (duration > 0) {
      setTimeout(() => {
        set((state) => ({
          toasts: state.toasts.filter((t) => t.id !== id),
        }));
      }, duration);
    }
  },

  removeToast: (id) =>
    set((state) => ({
      toasts: state.toasts.filter((t) => t.id !== id),
    })),
}));
