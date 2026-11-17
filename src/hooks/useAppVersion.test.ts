// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file The app's version on screen is never a placeholder.
 *
 * The status bar and the pre-release notice used to read "v..." while
 * loading and "vunknown" when the version lookup failed.
 */

import { act, renderHook, waitFor } from '@testing-library/react';
import { getVersion } from '@tauri-apps/api/app';
import { BUILD_VERSION, useAppVersion } from './useAppVersion';

vi.mock('@tauri-apps/api/app', () => ({ getVersion: vi.fn() }));

describe('useAppVersion', () => {
  it('knows the build version (the same constant the build bakes in)', () => {
    expect(BUILD_VERSION).toMatch(/^\d+\.\d+\.\d+/);
  });

  it('shows the build version from the first frame, before the app answers', () => {
    vi.mocked(getVersion).mockReturnValue(new Promise(() => {}));
    const { result } = renderHook(() => useAppVersion());
    expect(result.current).toBe(BUILD_VERSION);
  });

  it('keeps the build version, never "unknown", when the lookup fails', async () => {
    vi.mocked(getVersion).mockRejectedValue(new Error('no desktop app'));
    const { result } = renderHook(() => useAppVersion());
    // Let the failed lookup settle, and React apply anything it set.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 20));
    });
    expect(vi.mocked(getVersion)).toHaveBeenCalled();
    expect(result.current).toBe(BUILD_VERSION);
    expect(result.current).not.toMatch(/unknown|\.\.\./);
  });

  it("uses the desktop app's answer when it arrives", async () => {
    vi.mocked(getVersion).mockResolvedValue('9.9.9');
    const { result } = renderHook(() => useAppVersion());
    await waitFor(() => expect(result.current).toBe('9.9.9'));
  });
});
