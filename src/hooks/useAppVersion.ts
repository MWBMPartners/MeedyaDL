// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file useAppVersion.ts -- the app's version number, for showing on screen.
 *
 * The status bar and the pre-release notice used to start from "..." and
 * fall back to "unknown" when the desktop app's version lookup failed, so
 * they could read "MeedyaDL v..." while loading and "MeedyaDL vunknown" or
 * "vunknown — Pre-Release" after a failure. The build already knows its own
 * version (`__APP_VERSION__`, baked in from package.json by vite.config.ts),
 * so that is shown from the first frame, and replaced by the desktop app's
 * own answer when it arrives -- the two are the same number in a normal
 * build; the app's answer wins only because it is the one that shipped.
 */

import { useEffect, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';

/** Set at build time by `define` in vite.config.ts (and vitest.config.ts). */
declare const __APP_VERSION__: string | undefined;

/** The version this build was made from, or '' when it is not known. */
export const BUILD_VERSION: string = typeof __APP_VERSION__ === 'string' ? __APP_VERSION__ : '';

/**
 * The app's version number (for example "1.13.0-alpha.76"), never a
 * placeholder: the build's own version until the desktop app answers, and
 * kept if it never does. '' only when neither is known; callers then show
 * no version rather than a made-up one.
 */
export function useAppVersion(): string {
  const [version, setVersion] = useState(BUILD_VERSION);
  useEffect(() => {
    let current = true;
    getVersion()
      .then((v) => {
        if (current && v) setVersion(v);
      })
      .catch(() => {
        // Keep the build's own version.
      });
    return () => {
      current = false;
    };
  }, []);
  return version;
}
