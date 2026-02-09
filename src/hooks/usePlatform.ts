/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Platform detection hook.
 * Uses the Tauri OS plugin to detect the current operating system
 * at runtime. Returns the platform identifier used to load the
 * correct CSS theme and adapt UI behavior.
 *
 * Falls back to browser-based detection when Tauri APIs are unavailable
 * (e.g., during development in a regular browser).
 */

import { useEffect, useState } from 'react';

/** Supported platform identifiers for theme selection */
export type Platform = 'macos' | 'windows' | 'linux';

/** Return type of the usePlatform hook */
interface UsePlatformResult {
  /** Detected platform identifier */
  platform: Platform;
  /** Whether platform detection is still in progress */
  isLoading: boolean;
  /** Whether the current platform is macOS */
  isMacOS: boolean;
  /** Whether the current platform is Windows */
  isWindows: boolean;
  /** Whether the current platform is Linux */
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
  // Default to 'linux' as the safest fallback (neutral theme)
  const [platform, setPlatform] = useState<Platform>('linux');
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    /**
     * Attempts to detect the platform using Tauri's OS plugin.
     * This is an async operation because the plugin API is asynchronous.
     */
    async function detectPlatform() {
      try {
        // Try to import the Tauri OS plugin dynamically
        // This will fail gracefully if running outside Tauri (e.g., in a browser)
        const { platform: getPlatform } = await import('@tauri-apps/plugin-os');
        const detectedPlatform = getPlatform();

        // Map Tauri platform strings to our theme identifiers
        switch (detectedPlatform) {
          case 'macos':
            setPlatform('macos');
            break;
          case 'windows':
            setPlatform('windows');
            break;
          default:
            // Linux, FreeBSD, and other Unix-like systems use the Linux theme
            setPlatform('linux');
            break;
        }
      } catch {
        // Tauri API not available - fall back to browser-based detection
        // This enables development in a regular browser with `npm run dev`
        const userAgent = navigator.userAgent.toLowerCase();

        if (userAgent.includes('mac')) {
          setPlatform('macos');
        } else if (userAgent.includes('win')) {
          setPlatform('windows');
        } else {
          setPlatform('linux');
        }
      }

      // Mark detection as complete
      setIsLoading(false);
    }

    detectPlatform();
  }, []);

  return {
    platform,
    isLoading,
    isMacOS: platform === 'macos',
    isWindows: platform === 'windows',
    isLinux: platform === 'linux',
  };
}
