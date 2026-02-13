/**
 * Copyright (c) 2024-2026 MeedyaDL
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
     * We explicitly set navigator.userAgent to a known Windows UA string
     * so the test is deterministic regardless of the CI runner's OS
     * (jsdom's default userAgent varies by host platform).
     */
    vi.doMock('@tauri-apps/plugin-os', () => {
      throw new Error('Cannot find module');
    });

    /* Set a deterministic Windows userAgent for the fallback path */
    const originalUserAgent = navigator.userAgent;
    Object.defineProperty(navigator, 'userAgent', {
      value: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)',
      configurable: true,
    });

    /* Re-import to pick up the throwing mock */
    const { usePlatform: freshHook } = await import('@/hooks/usePlatform');
    const { result } = renderHook(() => freshHook());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    /* The mocked userAgent contains "win", so fallback detects 'windows' */
    expect(result.current.platform).toBe('windows');
    expect(result.current.isWindows).toBe(true);

    /* Restore original userAgent and unmock the plugin */
    Object.defineProperty(navigator, 'userAgent', {
      value: originalUserAgent,
      configurable: true,
    });
    vi.doUnmock('@tauri-apps/plugin-os');
  });
});
