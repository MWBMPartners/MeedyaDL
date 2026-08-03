/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file useGamdlCapabilities — hook wrapper around `getGamdlCapabilities()` (#853, #963, #1002).
 *
 * `AdvancedTab.tsx` fetches the `GamdlCapabilities` DTO directly with a
 * component-local `useState` + `useEffect` pair (see #853). This hook
 * extracts that same fetch-on-mount shape into a reusable form so other
 * components can react to the installed GAMDL release's capabilities
 * without duplicating the boilerplate.
 *
 * The first consumer is version-aware prose (#963, #1002): GAMDL v3.8's
 * new `/v1/play/assets` endpoint unlocked every non-web song codec
 * except ALAC for wrapper-less downloads, so the static "codecs marked
 * (Experimental) may fail intermittently without the Wrapper service"
 * note in `FallbackTab.tsx` is stale for a 3.8+ install. Per the
 * maintainer decision on #965, the codec dropdown's `(Experimental)`
 * labels themselves stay unconditional — this hook exists so that kind
 * of nuance can be expressed as *surrounding prose* instead, driven by
 * `capabilities.assets_api_unlocks_lossy_codecs`.
 *
 *   const { capabilities, isLoading } = useGamdlCapabilities();
 *   {capabilities.assets_api_unlocks_lossy_codecs
 *     ? <p>Only ALAC still needs the Wrapper service on your GAMDL install.</p>
 *     : <p>Atmos and AC3 may fail intermittently without the Wrapper service.</p>}
 *
 * @see {@link ../lib/tauri-commands.ts} — `GamdlCapabilities` DTO + `getGamdlCapabilities()`
 * @see {@link ../components/settings/tabs/AdvancedTab.tsx} — original inline fetch (#853)
 */

import { useEffect, useState } from 'react';

import { getGamdlCapabilities, type GamdlCapabilities } from '@/lib/tauri-commands';

/**
 * Conservative default mirroring the Rust-side cache-empty default —
 * every flag `false` except `music_video_remux_mode`, which defaults
 * `true` (accept the legacy CLI flag until the probe proves otherwise).
 * Same shape as `AdvancedTab.tsx`'s local default state (#853).
 */
const DEFAULT_CAPABILITIES: GamdlCapabilities = {
  wrapper_v2: false,
  native_muxing: false,
  aac_web_codec_rename: false,
  music_video_remux_mode: true,
  wrapper_m3u8_ip: false,
  playlist_folder_template: false,
  native_codec_priority: false,
  ffmpeg_path: false,
  assets_api_unlocks_lossy_codecs: false,
};

export interface UseGamdlCapabilitiesResult {
  /** Active GAMDL capability flags. Defaults to the conservative
   * cache-empty shape until the IPC call resolves. */
  capabilities: GamdlCapabilities;
  /** True until the first `getGamdlCapabilities()` call settles
   * (success or failure). */
  isLoading: boolean;
}

/**
 * Fetches the active GAMDL capability flags on mount.
 *
 * Zero I/O beyond the single IPC call — the backend reads from the
 * in-memory `detected_version` cache, so this resolves near-instantly
 * once the dependency probe has run. Failure (e.g. IPC unavailable in
 * a non-Tauri context) is swallowed and the conservative default is
 * kept, matching `AdvancedTab.tsx`'s existing catch-and-ignore pattern.
 */
export function useGamdlCapabilities(): UseGamdlCapabilitiesResult {
  const [capabilities, setCapabilities] = useState<GamdlCapabilities>(DEFAULT_CAPABILITIES);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    getGamdlCapabilities()
      .then(setCapabilities)
      .catch(() => {
        // Stay on the conservative default — the cache is populated by
        // the startup dependency probe, so this catch path only fires
        // in unusual scenarios (mirrors AdvancedTab.tsx, #853).
      })
      .finally(() => {
        setIsLoading(false);
      });
  }, []);

  return { capabilities, isLoading };
}
