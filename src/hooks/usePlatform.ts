/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/hooks/usePlatform.ts - Runtime platform detection hook
 *
 * This custom React hook detects the current operating system at runtime,
 * enabling the application to load platform-specific CSS themes and adapt
 * UI behavior (e.g., macOS-style traffic lights vs. Windows title bar buttons).
 *
 * Detection strategy (two-tier fallback):
 * 1. Primary: Tauri's `@tauri-apps/plugin-os` plugin, which uses Rust's
 *    `std::env::consts::OS` for accurate OS detection in the desktop app.
 * 2. Fallback: `navigator.userAgent` string analysis for development in
 *    a regular browser (when Tauri APIs are unavailable).
 *
 * The hook is consumed by App.tsx on startup to:
 * - Add a `platform-{os}` CSS class to the `<html>` element
 * - Dynamically import the platform-specific theme CSS file
 * - Gate rendering until platform detection completes (prevents FOUC)
 *
 * @see {@link https://react.dev/reference/react/useState} - useState hook
 * @see {@link https://react.dev/reference/react/useEffect} - useEffect hook
 * @see {@link https://v2.tauri.app/reference/javascript/plugin-os/} - Tauri OS plugin
 * @see {@link https://react.dev/learn/reusing-logic-with-custom-hooks} - Custom hooks guide
 */

/**
 * React hooks for state management and side effects.
 * - `useState`: holds the detected platform and loading state
 * - `useEffect`: triggers the async detection on mount (runs once)
 *
 * @see {@link https://react.dev/reference/react/useState}
 * @see {@link https://react.dev/reference/react/useEffect}
 */
import { useEffect, useState } from 'react';

/**
 * Supported platform identifiers for theme selection.
 *
 * These correspond to the three CSS theme files:
 * - 'macos'   -> src/styles/themes/macos.css
 * - 'windows' -> src/styles/themes/windows.css
 * - 'linux'   -> src/styles/themes/linux.css
 *
 * Note: 'linux' is also used as the default fallback for unrecognized
 * platforms (e.g., FreeBSD) since the Linux theme has the most neutral styling.
 *
 * @see {@link https://www.typescriptlang.org/docs/handbook/2/everyday-types.html#literal-types}
 */
export type Platform = 'macos' | 'windows' | 'linux';

/**
 * Return type of the usePlatform hook.
 *
 * Provides both the raw platform string and convenience boolean flags
 * so consumers can use whichever is more ergonomic:
 * - `platform` for switch statements and string interpolation
 * - `isMacOS`/`isWindows`/`isLinux` for conditional rendering
 */
interface UsePlatformResult {
  /** Detected platform identifier ('macos', 'windows', or 'linux') */
  platform: Platform;
  /** Whether platform detection is still in progress (true on initial render) */
  isLoading: boolean;
  /** Convenience: true when platform === 'macos' */
  isMacOS: boolean;
  /** Convenience: true when platform === 'windows' */
  isWindows: boolean;
  /** Convenience: true when platform === 'linux' (or any unrecognized OS) */
  isLinux: boolean;
}

/**
 * Detects the current operating system platform.
 *
 * Uses Tauri's OS plugin for accurate detection when running as a
 * desktop app. Falls back to navigator.platform for development
 * in a regular browser.
 *
 * @returns Platform information and convenience boolean flags
 *
 * @example
 * ```tsx
 * function MyComponent() {
 *   const { platform, isMacOS, isLoading } = usePlatform();
 *
 *   if (isLoading) return <Spinner />;
 *
 *   return <div className={`theme-${platform}`}>...</div>;
 * }
 * ```
 */
export function usePlatform(): UsePlatformResult {
  /*
   * Initialize platform to 'linux' as the safest default. This value is used
   * briefly during the first render before async detection completes. 'linux'
   * is chosen because its theme is the most neutral/generic.
   *
   * @see {@link https://react.dev/reference/react/useState#parameters}
   */
  const [platform, setPlatform] = useState<Platform>('linux');

  /*
   * Loading flag: starts true and becomes false once detection completes.
   * App.tsx checks this to gate rendering until the theme is ready.
   */
  const [isLoading, setIsLoading] = useState(true);

  /*
   * Run platform detection once on mount (empty dependency array).
   * The effect defines and immediately invokes an async function because
   * useEffect callbacks cannot be async directly (they must return void
   * or a cleanup function, not a Promise).
   *
   * @see {@link https://react.dev/reference/react/useEffect#parameters}
   */
  useEffect(() => {
    /**
     * Async platform detection function.
     *
     * Strategy:
     * 1. Try to dynamically import the Tauri OS plugin
     * 2. If available (running as desktop app), use its `platform()` function
     * 3. If unavailable (running in browser), fall back to navigator.userAgent
     */
    async function detectPlatform() {
      try {
        /*
         * Dynamic import of the Tauri OS plugin.
         *
         * Using `import()` instead of a top-level `import` statement makes this
         * a code-split point. If the plugin isn't available (browser dev mode),
         * the import() Promise rejects, and we fall through to the catch block.
         *
         * Note: the `platform` export from `@tauri-apps/plugin-os` is a synchronous
         * function (not async) -- it reads a cached value set during Tauri initialization.
         *
         * @see {@link https://v2.tauri.app/reference/javascript/plugin-os/#platform}
         */
        const { platform: getPlatform } = await import('@tauri-apps/plugin-os');
        const detectedPlatform = getPlatform();

        /*
         * Map Tauri's platform string to our Platform type.
         * Tauri returns lowercase strings: 'macos', 'windows', 'linux', 'ios', 'android', etc.
         * We only care about desktop platforms; everything else maps to 'linux'.
         */
        switch (detectedPlatform) {
          case 'macos':
            setPlatform('macos');
            break;
          case 'windows':
            setPlatform('windows');
            break;
          default:
            /* Linux, FreeBSD, and other Unix-like systems use the Linux theme */
            setPlatform('linux');
            break;
        }
      } catch {
        /*
         * Fallback: Tauri OS plugin not available.
         * This happens when running the frontend in a regular browser via
         * `npm run dev` (without the Tauri wrapper). We use navigator.userAgent
         * for a best-effort detection so the UI looks correct during development.
         *
         * @see {@link https://developer.mozilla.org/en-US/docs/Web/API/Navigator/userAgent}
         */
        const userAgent = navigator.userAgent.toLowerCase();

        if (userAgent.includes('mac')) {
          setPlatform('macos');
        } else if (userAgent.includes('win')) {
          setPlatform('windows');
        } else {
          setPlatform('linux');
        }
      }

      /* Detection complete -- unblock rendering in App.tsx */
      setIsLoading(false);
    }

    detectPlatform();
  }, []); // Empty deps: run once on mount

  /*
   * Return the platform info plus convenience booleans.
   * The boolean flags are derived values (not separate state) to avoid
   * synchronization issues. They update automatically when `platform` changes.
   */
  return {
    platform,
    isLoading,
    isMacOS: platform === 'macos',
    isWindows: platform === 'windows',
    isLinux: platform === 'linux',
  };
}
