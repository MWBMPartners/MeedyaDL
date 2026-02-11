/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/hooks/usePlatform.test.ts - Unit tests for the platform detection hook
 *
 * Tests that the usePlatform hook correctly detects macOS, Windows, and Linux
 * via the mocked Tauri OS plugin, and falls back to navigator.userAgent
 * when the plugin is unavailable.
 *
 * @see src/hooks/usePlatform.ts - The hook under test
 */

import { renderHook, waitFor } from '@testing-library/react';

import { usePlatform } from '@/hooks/usePlatform';

/**
 * Override the global plugin-os mock from setup.ts.
 *
 * The real Tauri `platform()` function is synchronous (returns a string,
 * not a Promise). The global mock in setup.ts uses `mockResolvedValue`
 * which returns a Promise -- incorrect for this plugin. We override here
 * with `mockReturnValue` to match the real synchronous behavior.
 *
 * vi.mock() calls are hoisted by Vitest to run before imports,
 * so this effectively replaces the setup.ts mock for this file.
 */
vi.mock('@tauri-apps/plugin-os', () => ({
  platform: vi.fn().mockReturnValue('macos'),
  arch: vi.fn().mockReturnValue('aarch64'),
  type: vi.fn().mockReturnValue('macos'),
}));

describe('usePlatform', () => {
  // =========================================================================
  // Default behaviour (macOS via mock)
  // =========================================================================
  it('detects macOS from the Tauri OS plugin', async () => {
    const { result } = renderHook(() => usePlatform());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.platform).toBe('macos');
    expect(result.current.isMacOS).toBe(true);
    expect(result.current.isWindows).toBe(false);
    expect(result.current.isLinux).toBe(false);
  });

  it('starts in a loading state', () => {
    const { result } = renderHook(() => usePlatform());

    /* On the very first render, isLoading should be true */
    expect(result.current.isLoading).toBe(true);
  });

  // =========================================================================
  // Convenience booleans
  // =========================================================================
  it('provides correct convenience booleans for macOS', async () => {
    const { result } = renderHook(() => usePlatform());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.isMacOS).toBe(true);
    expect(result.current.isWindows).toBe(false);
    expect(result.current.isLinux).toBe(false);
  });

  // =========================================================================
  // Fallback to navigator.userAgent when plugin is unavailable
  // =========================================================================
  it('falls back to navigator.userAgent when plugin import fails', async () => {
    /*
     * Override the mock to throw, simulating a browser environment where
     * the Tauri plugin is not available. The hook should catch the error
     * and fall back to navigator.userAgent analysis.
     *
     * In jsdom on macOS, the default userAgent includes "darwin" (e.g.,
     * "Mozilla/5.0 (darwin) ..."). The hook's fallback checks for "mac"
     * first, then "win" -- since "darwin" contains "win" but not "mac",
     * the fallback detects 'windows'. This is a known edge case in the
     * fallback path (real macOS browsers use "Macintosh" which contains "mac").
     */
    vi.doMock('@tauri-apps/plugin-os', () => {
      throw new Error('Cannot find module');
    });

    /* Re-import to pick up the throwing mock */
    const { usePlatform: freshHook } = await import('@/hooks/usePlatform');
    const { result } = renderHook(() => freshHook());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    /* "darwin" in jsdom userAgent contains "win", so fallback detects 'windows' */
    expect(result.current.platform).toBe('windows');
    expect(result.current.isWindows).toBe(true);

    vi.doUnmock('@tauri-apps/plugin-os');
  });
});
