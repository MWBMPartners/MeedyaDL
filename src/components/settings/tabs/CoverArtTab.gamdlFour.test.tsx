/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file CoverArtTab.gamdlFour.test.tsx -- the "no effect" note beside Save
 * Cover Art is shown only on a GAMDL 4 that saves a picture beside every
 * song (before 4.0.2).
 */

import { render, screen } from '@testing-library/react';

import { CoverArtTab } from './CoverArtTab';

const caps = vi.hoisted(() => ({
  value: { gamdl_v4: false, gamdl_four_saves_album_cover: false } as Record<string, boolean>,
}));

vi.mock('@/hooks/useGamdlCapabilities', () => ({
  useGamdlCapabilities: () => ({ capabilities: caps.value, isLoading: false }),
}));

describe('CoverArtTab on GAMDL 4', () => {
  it('says the switch has no effect on a GAMDL 4 before 4.0.2', () => {
    caps.value = { gamdl_v4: true, gamdl_four_saves_album_cover: false };
    render(<CoverArtTab />);
    expect(screen.getByText(/this switch has no effect/i)).toBeInTheDocument();
  });

  it('says nothing from GAMDL 4.0.2, where the switch works', () => {
    caps.value = { gamdl_v4: true, gamdl_four_saves_album_cover: true };
    render(<CoverArtTab />);
    expect(screen.queryByText(/this switch has no effect/i)).not.toBeInTheDocument();
  });

  it('says nothing on GAMDL 3', () => {
    caps.value = { gamdl_v4: false, gamdl_four_saves_album_cover: false };
    render(<CoverArtTab />);
    expect(screen.queryByText(/this switch has no effect/i)).not.toBeInTheDocument();
  });
});
