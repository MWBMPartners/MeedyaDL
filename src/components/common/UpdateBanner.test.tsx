// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Light render tests for `UpdateBanner`'s bounded per-tool row
 * button label (package-manager abstraction, Phase 2a -- see
 * `.github/audits/package-manager-abstraction-design-2026-08-10.md`).
 *
 * Scope is intentionally narrow: only the `managed_by` /
 * `manual_update_command` branch on the bounded engine/tool row list
 * (`MAX_BANNER_ENGINE_ROWS`), and its fallback to the original
 * collapsed message above that bound. The rest of `UpdateBanner`
 * (GAMDL upgrade, app download/restart flow, dismiss) is unaffected
 * by this change and not re-asserted here.
 */

import { render, screen } from '@testing-library/react';
import { act } from 'react';

import { useUpdateStore } from '@/stores/updateStore';
import { UpdateBanner } from '@/components/common/UpdateBanner';
import type { ComponentUpdate, UpdateCheckResult } from '@/types';

/** Builds a minimal, fully-populated `ComponentUpdate` fixture. */
function makeComponentUpdate(overrides: Partial<ComponentUpdate> = {}): ComponentUpdate {
  return {
    name: 'FFmpeg',
    current_version: '6.0',
    latest_version: '6.1',
    update_available: true,
    is_compatible: true,
    is_untested: false,
    no_compatible_wheel: false,
    description: null,
    release_url: null,
    release_body: null,
    is_prerelease: false,
    tag_name: null,
    pip_package: null,
    tool_id: 'ffmpeg',
    ...overrides,
  };
}

function setLastResult(components: ComponentUpdate[]) {
  const result: UpdateCheckResult = {
    checked_at: new Date().toISOString(),
    has_updates: components.length > 0,
    components,
    errors: [],
  };
  act(() => {
    useUpdateStore.setState({ lastResult: result, dismissed: [] });
  });
}

beforeEach(() => {
  act(() => {
    useUpdateStore.setState({
      lastResult: null,
      dismissed: [],
      isChecking: false,
      isUpgrading: false,
      isDownloadingUpdate: false,
      downloadProgress: null,
      updateInstalled: false,
      downloadError: null,
    });
  });
});

describe('UpdateBanner bounded tool row -- managed_by / manual_update_command', () => {
  it('labels the row button "Update via <label>" when within the row-count bound', () => {
    setLastResult([
      makeComponentUpdate({
        name: 'FFmpeg',
        managed_by: 'Homebrew',
        manual_update_command: 'brew upgrade ffmpeg',
      }),
    ]);

    render(<UpdateBanner />);

    expect(screen.getByRole('button', { name: /Update via Homebrew/i })).toBeInTheDocument();
    expect(screen.getByText(/Runs: brew upgrade ffmpeg/i)).toBeInTheDocument();
  });

  it('falls back to the collapsed generic message above the row-count bound', () => {
    // MAX_BANNER_ENGINE_ROWS is 3 -- four distinct engine updates should
    // collapse back to the original "Component updates are also
    // available" link instead of four individual rows.
    setLastResult([
      makeComponentUpdate({ name: 'FFmpeg', tool_id: 'ffmpeg' }),
      makeComponentUpdate({ name: 'MP4Box', tool_id: 'mp4box' }),
      makeComponentUpdate({ name: 'N_m3u8DL-RE', tool_id: 'nm3u8dlre' }),
      makeComponentUpdate({ name: 'mp4decrypt', tool_id: 'mp4decrypt' }),
    ]);

    render(<UpdateBanner />);

    expect(screen.getByText(/Component updates are also available/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Update via/i })).not.toBeInTheDocument();
  });
});
