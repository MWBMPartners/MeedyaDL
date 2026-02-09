/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Dependency state store.
 * Tracks the installation status of Python, GAMDL, and external tools.
 * Used by the setup wizard and status indicators throughout the UI.
 */

import { create } from 'zustand';
import type { DependencyStatus } from '@/types';
import * as commands from '@/lib/tauri-commands';

interface DependencyState {
  /** Python runtime status */
  python: DependencyStatus | null;
  /** GAMDL package status */
  gamdl: DependencyStatus | null;
  /** External tool statuses (FFmpeg, mp4decrypt, etc.) */
  tools: DependencyStatus[];
  /** Whether a status check is in progress */
  isChecking: boolean;
  /** Whether an installation is in progress */
  isInstalling: boolean;
  /** Name of the component currently being installed */
  installingName: string | null;
  /** Error from the last operation */
  error: string | null;

  /** Check all dependency statuses */
  checkAll: () => Promise<void>;
  /** Check only Python status */
  checkPython: () => Promise<void>;
  /** Check only GAMDL status */
  checkGamdl: () => Promise<void>;
  /** Install Python */
  installPython: () => Promise<string>;
  /** Install GAMDL */
  installGamdl: () => Promise<string>;
  /** Install a specific tool */
  installTool: (name: string) => Promise<string>;
  /** Whether all required dependencies are installed */
  isReady: () => boolean;
}

export const useDependencyStore = create<DependencyState>((set, get) => ({
  python: null,
  gamdl: null,
  tools: [],
  isChecking: false,
  isInstalling: false,
  installingName: null,
  error: null,

  checkAll: async () => {
    set({ isChecking: true, error: null });
    try {
      const [python, gamdl, tools] = await Promise.all([
        commands.checkPythonStatus(),
        commands.checkGamdlStatus(),
        commands.checkAllDependencies(),
      ]);
      set({ python, gamdl, tools, isChecking: false });
    } catch (e) {
      set({ error: String(e), isChecking: false });
    }
  },

  checkPython: async () => {
    try {
      const python = await commands.checkPythonStatus();
      set({ python });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  checkGamdl: async () => {
    try {
      const gamdl = await commands.checkGamdlStatus();
      set({ gamdl });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  installPython: async () => {
    set({ isInstalling: true, installingName: 'Python', error: null });
    try {
      const version = await commands.installPython();
      // Refresh Python status after installation
      const python = await commands.checkPythonStatus();
      set({ python, isInstalling: false, installingName: null });
      return version;
    } catch (e) {
      const msg = String(e);
      set({ error: msg, isInstalling: false, installingName: null });
      throw new Error(msg);
    }
  },

  installGamdl: async () => {
    set({ isInstalling: true, installingName: 'GAMDL', error: null });
    try {
      const version = await commands.installGamdl();
      const gamdl = await commands.checkGamdlStatus();
      set({ gamdl, isInstalling: false, installingName: null });
      return version;
    } catch (e) {
      const msg = String(e);
      set({ error: msg, isInstalling: false, installingName: null });
      throw new Error(msg);
    }
  },

  installTool: async (name: string) => {
    set({ isInstalling: true, installingName: name, error: null });
    try {
      const version = await commands.installDependency(name);
      const tools = await commands.checkAllDependencies();
      set({ tools, isInstalling: false, installingName: null });
      return version;
    } catch (e) {
      const msg = String(e);
      set({ error: msg, isInstalling: false, installingName: null });
      throw new Error(msg);
    }
  },

  isReady: () => {
    const { python, gamdl } = get();
    return !!(python?.installed && gamdl?.installed);
  },
}));
