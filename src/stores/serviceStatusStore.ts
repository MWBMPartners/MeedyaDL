// Copyright (c) 2026 MeedyaDL

/**
 * @file Zustand store for the remote service status (kill-switch) system.
 *
 * Manages the state of the remote service status configuration that controls
 * per-service enable/disable flags. The config is fetched from GitHub on
 * app launch and refreshed every 4 hours.
 *
 * @see src-tauri/src/services/service_status.rs -- Rust service
 * @see src/lib/tauri-commands.ts -- checkServiceStatus() IPC wrapper
 */

import { create } from 'zustand';
import { checkServiceStatus } from '@/lib/tauri-commands';
import type { ServiceStatusConfig, MediaServiceId } from '@/types';

/**
 * Maps the frontend kebab-case MediaServiceId to the PascalCase key
 * used in the remote JSON config.
 */
const SERVICE_KEY_MAP: Record<MediaServiceId, string> = {
  'apple-music': 'AppleMusic',
  youtube: 'YouTube',
  'bbc-iplayer': 'BBCiPlayer',
  spotify: 'Spotify',
};

interface ServiceStatusState {
  /** The fetched service status configuration, or null if not yet loaded */
  config: ServiceStatusConfig | null;
  /** Whether a fetch is currently in progress */
  isLoading: boolean;
  /** Error message from the last failed fetch, or null */
  error: string | null;
  /** ISO timestamp of the last successful check */
  lastChecked: string | null;

  /** Fetch the service status from the backend (remote → cache → default) */
  checkStatus: () => Promise<void>;
  /** Returns true if the given service is disabled by the remote config */
  isServiceDisabled: (serviceId: MediaServiceId) => boolean;
  /** Returns the message for a disabled service, or null */
  getServiceMessage: (serviceId: MediaServiceId) => string | null;
  /** Returns the global announcement message, or null */
  getGlobalMessage: () => string | null;
  /** Returns a list of all disabled service IDs */
  getDisabledServices: () => MediaServiceId[];
}

export const useServiceStatusStore = create<ServiceStatusState>((set, get) => ({
  config: null,
  isLoading: false,
  error: null,
  lastChecked: null,

  checkStatus: async () => {
    set({ isLoading: true, error: null });
    try {
      const config = await checkServiceStatus();
      set({
        config,
        isLoading: false,
        lastChecked: new Date().toISOString(),
      });
    } catch (err) {
      set({
        isLoading: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  isServiceDisabled: (serviceId: MediaServiceId): boolean => {
    const { config } = get();
    if (!config) return false; // Not loaded yet = not disabled (fail-open)
    const key = SERVICE_KEY_MAP[serviceId];
    if (!key) return false;
    const entry = config.services[key];
    if (!entry) return false; // Missing key = not disabled (fail-open)
    return !entry.enabled;
  },

  getServiceMessage: (serviceId: MediaServiceId): string | null => {
    const { config } = get();
    if (!config) return null;
    const key = SERVICE_KEY_MAP[serviceId];
    if (!key) return null;
    const entry = config.services[key];
    return entry?.message ?? null;
  },

  getGlobalMessage: (): string | null => {
    const { config } = get();
    return config?.global_message ?? null;
  },

  getDisabledServices: (): MediaServiceId[] => {
    const { config } = get();
    if (!config) return [];
    const disabled: MediaServiceId[] = [];
    for (const [serviceId, key] of Object.entries(SERVICE_KEY_MAP)) {
      const entry = config.services[key];
      if (entry && !entry.enabled) {
        disabled.push(serviceId as MediaServiceId);
      }
    }
    return disabled;
  },
}));
