// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for the setup wizard's first screen.
 *
 * Until October 2026 the very first thing a new user saw read "Welcome to
 * GAMDL" / "Apple Music Downloader" beside a generic download arrow: the
 * app greeted people with the name of the download engine underneath it,
 * not its own. Its first step also promised that "a portable Python runtime
 * will be downloaded", which stopped being the only route when the Python
 * step learned to reuse a Python the computer already has.
 */

import { render, screen } from '@testing-library/react';
import { WelcomeStep } from './WelcomeStep';
import { scanForBundles } from '@/lib/tauri-commands';

// The bundle scan is the only backend call this screen makes; answer it
// with "no bundles found" so the plain welcome screen is what renders.
vi.mock('@/lib/tauri-commands', () => ({
  scanForBundles: vi.fn().mockResolvedValue([]),
  importProfile: vi.fn(),
}));

describe('WelcomeStep', () => {
  it('welcomes people to MeedyaDL, not to the engine underneath it', () => {
    render(<WelcomeStep />);
    const heading = screen.getByRole('heading', { name: 'Welcome to MeedyaDL' });
    expect(heading).toBeInTheDocument();
    expect(screen.queryByText(/Welcome to GAMDL/)).not.toBeInTheDocument();
  });

  it("shows MeedyaDL's own logo", () => {
    const { container } = render(<WelcomeStep />);
    const logo = container.querySelector('img[src="/logo.svg"]');
    expect(logo).not.toBeNull();
  });

  it('says Python can be reused, not only downloaded', () => {
    render(<WelcomeStep />);
    expect(screen.getByText(/reuse a Python you already have/)).toBeInTheDocument();
    expect(screen.queryByText(/portable Python runtime will be downloaded/)).not.toBeInTheDocument();
  });

  // Polish pass L13: the same thing is called "Full Copy of MeedyaDL" in
  // Settings > General; this used to say "previous-install bundle".
  it('calls a found .meedyabundle a full copy, the same name Settings uses', async () => {
    vi.mocked(scanForBundles).mockResolvedValueOnce([
      {
        path: '/Users/demo/Downloads/meedyadl.meedyabundle',
        size_bytes: 2048,
        summary: {
          bundle_version: 1,
          producer: 'MeedyaDL',
          producer_version: '1.13.0',
          producer_platform: 'macos',
          exported_at: '2026-10-01T09:00:00Z',
          exported_by: null,
          note: null,
          sections: ['queue'],
          source_path: '/Users/demo/Downloads/meedyadl.meedyabundle',
          produced_by_this_app: true,
        },
      },
    ]);
    render(<WelcomeStep />);
    expect(await screen.findByRole('heading', { name: /Found a full copy of MeedyaDL from before/ })).toBeInTheDocument();
    expect(screen.queryByText(/previous-install bundle/i)).not.toBeInTheDocument();
  });
});
