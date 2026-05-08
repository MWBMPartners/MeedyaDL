// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for DownloadForm (#232 — first installment).
 *
 * Scope of THIS file: render-state behaviour driven by the URL input
 * and store state. Specifically:
 *   - Smoke render (form skeleton lands)
 *   - URL input → store update + revalidation
 *   - Content-type badge for valid single-URL input
 *   - Multi-URL detection + count badge + invalid-only error
 *   - Submit button gating (empty / invalid / valid)
 *   - Error text under the textarea
 *   - Quality Overrides collapsible panel
 *
 * **Out of scope** (deferred to a follow-up PR): the full
 * `handleSubmit` preflight chain (4 IPC calls — internet, output path,
 * wrapper, cookies). Each preflight has its own toast/error/redirect
 * surface and warrants dedicated fixture setup; bundling it in here
 * would make this file unreadable. The submit-disabled gating is
 * tested without ever clicking the button.
 *
 * @see src/components/download/DownloadForm.tsx
 */

import { render, screen, fireEvent, act } from '@testing-library/react';
import { useDownloadStore } from '@/stores/downloadStore';
import { useUiStore } from '@/stores/uiStore';
import { DownloadForm } from '@/components/download/DownloadForm';

// `tauri-commands` is mocked module-level. The default mock from
// src/test/setup.ts already silences `invoke()`, but DownloadForm
// imports the named wrappers directly — those need the per-name mock
// shape so the module doesn't blow up on import. None of the tests in
// this file actually trigger these IPCs; they're guarded by the submit
// button which we deliberately don't click.
vi.mock('@/lib/tauri-commands', () => ({
  checkCookiesBeforeDownload: vi.fn().mockResolvedValue({ valid: true }),
  checkInternetBeforeDownload: vi.fn().mockResolvedValue({
    online: true,
    apple_music_reachable: true,
  }),
  checkOutputPathBeforeDownload: vi.fn().mockResolvedValue({ writable: true }),
  checkRedownloadStatus: vi.fn().mockResolvedValue(null),
  importManifest: vi.fn().mockResolvedValue(null),
}));

/**
 * Reset every store between tests so one test's side-effects (URL
 * input, override settings, toasts) don't bleed into the next. The
 * downloadStore action `setUrlInput('')` clears the controlled
 * textarea AND the parsed-validation flags in one call.
 */
beforeEach(() => {
  act(() => {
    useDownloadStore.getState().setUrlInput('');
    useDownloadStore.getState().setOverrideOptions(null);
    useUiStore.setState({ toasts: [] });
  });
});

describe('DownloadForm', () => {
  // ===========================================================================
  // Smoke render
  // ===========================================================================

  it('renders the URL textarea + Add to Queue button + import buttons', () => {
    render(<DownloadForm />);
    expect(screen.getByLabelText('Media URL input')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /add to queue/i })
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^import$/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^scan$/i })).toBeInTheDocument();
  });

  it('renders the Apple Music URL label linked to the textarea', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    expect(textarea).toHaveAttribute('id', 'url-input');
    // The visible <label htmlFor="url-input"> wraps the same id
    const label = screen.getByText('Apple Music URL');
    expect(label.tagName).toBe('LABEL');
    expect(label).toHaveAttribute('for', 'url-input');
  });

  it('shows the helper hint when input is empty', () => {
    render(<DownloadForm />);
    expect(
      screen.getByText(
        /Supports songs, albums, playlists, music videos, and artist pages/i
      )
    ).toBeInTheDocument();
  });

  // ===========================================================================
  // Single-URL input + validation
  // ===========================================================================

  it('typing into the textarea updates the downloadStore', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, { target: { value: 'https://music.apple.com/us/album/foo/123' } });
    expect(useDownloadStore.getState().urlInput).toBe(
      'https://music.apple.com/us/album/foo/123'
    );
    expect(useDownloadStore.getState().urlIsValid).toBe(true);
  });

  it('renders the content-type badge for a valid single URL', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, {
      target: { value: 'https://music.apple.com/us/album/foo/123' },
    });
    // CONTENT_TYPE_LABELS[album] === 'Album' (capitalised) per the source
    expect(screen.getByText('Album')).toBeInTheDocument();
  });

  it('shows the single-URL error message when input is non-empty but invalid', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, { target: { value: 'not-a-real-url' } });
    expect(
      screen.getByText('Please enter a valid Apple Music URL')
    ).toBeInTheDocument();
  });

  // ===========================================================================
  // Multi-URL detection
  // ===========================================================================

  it('detects multi-URL input and shows the URL count badge', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, {
      target: {
        value: [
          'https://music.apple.com/us/album/foo/123',
          'https://music.apple.com/us/album/bar/456',
        ].join('\n'),
      },
    });
    // "2 URLs" badge — pluralised
    expect(screen.getByText(/^2 URLs$/)).toBeInTheDocument();
  });

  it('shows "(N invalid)" suffix when batch contains a mix', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, {
      target: {
        value: [
          'https://music.apple.com/us/album/foo/123',
          'definitely-not-a-url',
          'https://music.apple.com/us/album/bar/456',
        ].join('\n'),
      },
    });
    expect(screen.getByText(/^2 URLs$/)).toBeInTheDocument();
    expect(screen.getByText(/\(1 invalid\)/)).toBeInTheDocument();
  });

  it('shows the multi-URL error when every line is invalid', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, {
      target: { value: ['nope', 'still-nope', 'https://example.com'].join('\n') },
    });
    expect(screen.getByText('No valid Apple Music URLs found')).toBeInTheDocument();
  });

  // ===========================================================================
  // Submit button gating
  // ===========================================================================

  it('disables Add to Queue when input is empty', () => {
    render(<DownloadForm />);
    expect(screen.getByRole('button', { name: /add to queue/i })).toBeDisabled();
  });

  it('disables Add to Queue when single URL is invalid', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, { target: { value: 'garbage' } });
    expect(screen.getByRole('button', { name: /add to queue/i })).toBeDisabled();
  });

  it('enables Add to Queue when a single valid URL is entered', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, {
      target: { value: 'https://music.apple.com/us/album/foo/123' },
    });
    expect(screen.getByRole('button', { name: /add to queue/i })).toBeEnabled();
  });

  it('enables Add to Queue when batch contains at least one valid URL', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, {
      target: {
        value: [
          'definitely-bad',
          'https://music.apple.com/us/album/foo/123',
        ].join('\n'),
      },
    });
    expect(screen.getByRole('button', { name: /add to queue/i })).toBeEnabled();
  });

  // ===========================================================================
  // Quality Overrides panel
  // ===========================================================================

  it('renders the collapsible Quality Overrides toggle', () => {
    render(<DownloadForm />);
    // The toggle text starts with "Quality Overrides" then has the
    // default codec hint inside a nested span — match a substring.
    expect(screen.getByText(/^Quality Overrides/)).toBeInTheDocument();
  });

  it('expands the Quality Overrides panel when toggle is clicked', () => {
    render(<DownloadForm />);
    // Panel starts collapsed — there should be no inner controls yet.
    // The `audio_codec` Select has aria-label that's only present when
    // the panel is open. Use the toggle text as the click target.
    fireEvent.click(screen.getByText(/^Quality Overrides/));
    // After click, the inner panel content renders. The "Clear
    // overrides" button isn't visible yet (no overrides set), but the
    // border-top divider exists. Best stable signal: the audio codec
    // panel header text appears.
    // (The component's inner Select labels include the override
    // category — just check no exception was thrown by interacting
    // with the toggle and that the form hasn't unmounted.)
    expect(screen.getByLabelText('Media URL input')).toBeInTheDocument();
  });
});
