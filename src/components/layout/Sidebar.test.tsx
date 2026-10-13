// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file The sidebar's logo area has the same padding on every platform.
 *
 * It used to add an extra 32 pixels on macOS "to avoid the traffic-light
 * buttons" of an overlay title bar the app has never used -- the window has
 * the operating system's normal title bar everywhere -- which only left an
 * empty strip above the logo on Macs.
 */

import { render } from '@testing-library/react';
import { Sidebar } from './Sidebar';

let platform: 'macos' | 'windows' = 'macos';

vi.mock('@/hooks/usePlatform', () => ({
  usePlatform: () => ({
    platform,
    isLoading: false,
    isMacOS: platform === 'macos',
    isWindows: platform === 'windows',
    isLinux: false,
    arch: 'aarch64',
    supportsWrapper: false,
  }),
}));

/** The classes on the logo area (the element holding the logo image). */
function logoAreaClasses(): string {
  const { container, unmount } = render(<Sidebar />);
  const logo = container.querySelector('img[src^="/logo.svg"]');
  const classes = (logo?.parentElement?.className ?? '').replace(/\s+/g, ' ').trim();
  unmount();
  return classes;
}

describe('Sidebar logo area', () => {
  it('has the same padding on macOS as on Windows', () => {
    platform = 'windows';
    const windows = logoAreaClasses();
    platform = 'macos';
    const mac = logoAreaClasses();
    expect(windows).not.toBe('');
    expect(mac).toBe(windows);
    expect(mac).not.toMatch(/\bpt-8\b/);
  });
});
