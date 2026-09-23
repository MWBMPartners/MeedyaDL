// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Light render tests for `UpdatesPage`'s per-tool row button label
 * (package-manager abstraction, Phase 2a -- see
 * `.github/audits/package-manager-abstraction-design-2026-08-10.md`).
 *
 * Scope is intentionally narrow: only the new `managed_by` /
 * `manual_update_command` branch on the generic "Component Updates"
 * per-row list. The rest of `UpdatesPage` (release notes markdown,
 * rollback UI, app-update download flow, etc.) is exercised
 * incidentally, not directly asserted on here.
 */

import { render, screen } from '@testing-library/react';
import { act } from 'react';

import { useUpdateStore } from '@/stores/updateStore';
import { UpdatesPage } from '@/components/updates/UpdatesPage';
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

describe('UpdatesPage generic tool row -- managed_by / manual_update_command', () => {
  it('labels the row button "Update via <label>" when managed_by is set', () => {
    setLastResult([
      makeComponentUpdate({
        name: 'FFmpeg',
        managed_by: 'Homebrew',
        manual_update_command: 'brew upgrade ffmpeg',
      }),
    ]);

    render(<UpdatesPage />);

    expect(screen.getByRole('button', { name: /Update via Homebrew/i })).toBeInTheDocument();
    expect(screen.getByText(/Runs: brew upgrade ffmpeg/i)).toBeInTheDocument();
  });

  it('falls back to the plain "Upgrade" label when managed_by is absent', () => {
    setLastResult([makeComponentUpdate({ name: 'MP4Box', managed_by: undefined })]);

    render(<UpdatesPage />);

    expect(screen.queryByText(/Update via/i)).not.toBeInTheDocument();
    // Two "Upgrade" buttons would exist if a bulk action reused the same
    // label; here only the per-row button is expected since GAMDL/App
    // have no active update in this fixture.
    expect(screen.getByRole('button', { name: /^Upgrade$/i })).toBeInTheDocument();
  });

  it('does not render "Runs:" helper text when manual_update_command is absent', () => {
    setLastResult([
      makeComponentUpdate({ name: 'FFmpeg', managed_by: 'Homebrew', manual_update_command: undefined }),
    ]);

    render(<UpdatesPage />);

    expect(screen.getByRole('button', { name: /Update via Homebrew/i })).toBeInTheDocument();
    expect(screen.queryByText(/Runs:/i)).not.toBeInTheDocument();
  });
});

// ===========================================================================
// Things MeedyaDL could not check
// ===========================================================================
//
// The page keeps only entries where an update IS available. Anything
// nobody could work out an answer for used to fall straight through that
// filter and never reach the screen — so "we could not tell" looked
// exactly like "checked, you are up to date". These tests fail if that
// comes back.

describe('things MeedyaDL could not check', () => {
  it('says so, instead of letting them vanish behind "up to date"', () => {
    setLastResult([
      makeComponentUpdate({
        name: 'FFmpeg',
        current_version: '6.0',
        latest_version: null,
        update_available: false,
        not_checkable_reason:
          'Cannot check for an FFmpeg update — this copy was installed before MeedyaDL began recording where builds come from.',
        tool_id: 'ffmpeg',
      }),
    ]);

    render(<UpdatesPage />);

    // The heading still appears, because nothing needs updating...
    expect(screen.getByText(/up to date/i)).toBeInTheDocument();
    // ...but it no longer speaks for FFmpeg, which nobody could check.
    expect(screen.getByText(/could not check/i)).toBeInTheDocument();
    expect(
      screen.getByText(/installed before MeedyaDL began recording/i)
    ).toBeInTheDocument();
  });

  it('counts them, so one reads differently from several', () => {
    setLastResult([
      makeComponentUpdate({
        name: 'FFmpeg',
        update_available: false,
        not_checkable_reason: 'Cannot check — no build date was recorded.',
        tool_id: 'ffmpeg',
      }),
      makeComponentUpdate({
        name: 'MediaInfo',
        update_available: false,
        not_checkable_reason: 'The mirror does not record a version for this tool.',
        tool_id: 'mediainfo',
      }),
    ]);

    render(<UpdatesPage />);
    expect(screen.getByText(/2 things MeedyaDL could not check/i)).toBeInTheDocument();
  });

  it('stays quiet when everything really was checked', () => {
    // The ordinary case: nothing to update, nothing unknown. No notice at
    // all, so this cannot become background noise people learn to ignore.
    setLastResult([
      makeComponentUpdate({
        name: 'FFmpeg',
        update_available: false,
        not_checkable_reason: null,
        tool_id: 'ffmpeg',
      }),
    ]);

    render(<UpdatesPage />);
    expect(screen.getByText(/up to date/i)).toBeInTheDocument();
    expect(screen.queryByText(/could not check/i)).not.toBeInTheDocument();
  });
});
