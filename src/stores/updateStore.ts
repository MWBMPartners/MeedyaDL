/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Update state store.
 * Manages the application update checking lifecycle: tracking available
 * updates for GAMDL, Python, and the app itself. Supports auto-check
 * on startup, manual checks, GAMDL upgrade execution, and dismissal
 * of update notifications.
 */

import { create } from 'zustand';
import type { ComponentUpdate, UpdateCheckResult } from '@/types';
import * as commands from '@/lib/tauri-commands';

interface UpdateState {
  /** The most recent update check result (null if never checked) */
  lastResult: UpdateCheckResult | null;
  /** Whether an update check is currently in progress */
  isChecking: boolean;
  /** Whether a GAMDL upgrade is currently in progress */
  isUpgrading: boolean;
  /** IDs of dismissed update notifications (component names) */
  dismissed: string[];
  /** Error from the last check or upgrade operation */
  error: string | null;

  /** Check for updates to all components (GAMDL, app, Python) */
  checkForUpdates: () => Promise<UpdateCheckResult>;
  /** Upgrade GAMDL to the latest compatible version */
  upgradeGamdl: () => Promise<string>;
  /** Dismiss an update notification for a specific component */
  dismissUpdate: (componentName: string) => void;
  /** Clear all dismissed notifications (e.g., after a new check) */
  clearDismissed: () => void;
  /** Returns components with available, non-dismissed updates */
  getActiveUpdates: () => ComponentUpdate[];
  /** Whether there are any non-dismissed updates available */
  hasActiveUpdates: () => boolean;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  lastResult: null,
  isChecking: false,
  isUpgrading: false,
  dismissed: [],
  error: null,

  checkForUpdates: async () => {
    set({ isChecking: true, error: null });
    try {
      const result = await commands.checkAllUpdates();
      // Clear previous dismissals on a fresh check so new updates are visible
      set({ lastResult: result, isChecking: false, dismissed: [] });
      return result;
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      set({ error: message, isChecking: false });
      throw new Error(message);
    }
  },

  upgradeGamdl: async () => {
    set({ isUpgrading: true, error: null });
    try {
      const version = await commands.upgradeGamdl();
      // After upgrade, re-check to refresh the update status
      const result = await commands.checkAllUpdates();
      set({ lastResult: result, isUpgrading: false });
      return version;
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      set({ error: message, isUpgrading: false });
      throw new Error(message);
    }
  },

  dismissUpdate: (componentName) =>
    set((state) => ({
      dismissed: [...state.dismissed, componentName],
    })),

  clearDismissed: () => set({ dismissed: [] }),

  getActiveUpdates: () => {
    const { lastResult, dismissed } = get();
    if (!lastResult) return [];
    return lastResult.components.filter(
      (c) =>
        c.update_available &&
        c.is_compatible &&
        !dismissed.includes(c.name),
    );
  },

  hasActiveUpdates: () => {
    return get().getActiveUpdates().length > 0;
  },
}));
