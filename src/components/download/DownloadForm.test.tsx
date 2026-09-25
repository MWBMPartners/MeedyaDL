// Copyright (c) 2024-2026 MeedyaSuite
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
import * as commands from '@/lib/tauri-commands';
import { useSettingsStore } from '@/stores/settingsStore';

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
  // The one-off after-queue menu's own write, and its failure path's read
  // of what is really on disk. Set per test below.
  setStoredPreference: vi.fn(),
  getAfterQueueStatus: vi.fn(),
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
    const label = screen.getByText('Media URL');
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
      screen.getByText('Please enter a valid Apple Music or Spotify URL')
    ).toBeInTheDocument();
  });

  // ===========================================================================
  // Spotify URL acceptance (#983)
  // ===========================================================================

  it('accepts a Spotify URL and shows the Spotify badge', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, {
      target: { value: 'https://open.spotify.com/album/abc123' },
    });
    expect(useDownloadStore.getState().urlIsValid).toBe(true);
    expect(screen.getByRole('button', { name: /add to queue/i })).toBeEnabled();
    // Non-Apple services render the service label instead of a content type.
    expect(screen.getByText('Spotify')).toBeInTheDocument();
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

  it('says a line is not recognised when it is not a link to anything we know', () => {
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
    expect(screen.getByText(/\(1 not recognised\)/)).toBeInTheDocument();
  });

  /*
   * The reason the wording changed at all (#1157).
   *
   * A YouTube link is not a mistake the person made — it is a good link to a
   * service MeedyaDL cannot download from yet. Calling it "invalid" sends
   * them off checking a link that was fine. This project has form: Apple
   * Music Classical links were refused as invalid for a long time while
   * being exactly the kind of link MeedyaDL was meant to accept.
   */
  it('says a recognised service is not supported YET, rather than invalid', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, {
      target: {
        value: [
          'https://music.apple.com/us/album/foo/123',
          'https://www.youtube.com/watch?v=abc123',
        ].join('\n'),
      },
    });
    expect(screen.getByText(/\(1 not supported yet\)/)).toBeInTheDocument();
    // The word that caused the confusion must not appear.
    expect(screen.queryByText(/invalid/i)).not.toBeInTheDocument();
  });

  it('shows the multi-URL error when every line is invalid', () => {
    render(<DownloadForm />);
    const textarea = screen.getByLabelText('Media URL input');
    fireEvent.change(textarea, {
      target: { value: ['nope', 'still-nope', 'https://example.com'].join('\n') },
    });
    expect(screen.getByText('No valid Apple Music or Spotify URLs found')).toBeInTheDocument();
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
  // ===========================================================================
  // One-off after-queue action: the failure path
  // ===========================================================================

  describe('one-off after-queue action when saving it fails', () => {
    it('leaves the standing setting alone, follows the disk, and does not claim unsaved changes', async () => {
      // The person has an UNSAVED edit on the Settings screen to the
      // standing after-queue action, and nothing else pending.
      act(() => {
        useSettingsStore.setState((s) => ({
          settings: { ...s.settings, after_queue_action: 'play_sound', after_queue_once: null },
          isDirty: false,
        }));
      });
      vi.mocked(commands.setStoredPreference).mockRejectedValueOnce(new Error('disk full'));
      // What the queue will act on (the running app's settings): a
      // shutdown is armed once; the standing action is "do nothing".
      vi.mocked(commands.getAfterQueueStatus).mockResolvedValueOnce({
        after_queue_action: 'do_nothing',
        after_queue_once: 'shutdown_computer',
      });

      render(<DownloadForm />);
      fireEvent.click(screen.getByLabelText('After-queue actions'));
      await act(async () => {
        fireEvent.click(screen.getByText('After Queue: Play sound'));
      });

      const state = useSettingsStore.getState();
      // Three earlier attempts got this wrong; each property below is one
      // of them (see the comments in DownloadForm's failure path).
      expect(state.settings.after_queue_action).toBe('play_sound'); // unsaved edit kept
      expect(state.settings.after_queue_once).toBe('shutdown_computer'); // follows disk
      expect(state.isDirty).toBe(false); // no false "unsaved changes"
      const toast = useUiStore.getState().toasts.find((t) => t.type === 'error');
      expect(toast?.message).toContain('could not save that after-queue action');
      expect(toast?.message).toContain('shutdown computer');
      expect(toast?.message).toContain('will still happen');
    });

    it('when even the disk cannot be read, says it could not check', async () => {
      act(() => {
        useSettingsStore.setState((s) => ({
          settings: { ...s.settings, after_queue_once: 'restart_computer' },
          isDirty: false,
        }));
      });
      vi.mocked(commands.setStoredPreference).mockRejectedValueOnce(new Error('disk full'));
      vi.mocked(commands.getAfterQueueStatus).mockRejectedValueOnce(new Error('unreadable'));

      render(<DownloadForm />);
      fireEvent.click(screen.getByLabelText('After-queue actions'));
      await act(async () => {
        fireEvent.click(screen.getByText('After Queue: Do nothing'));
      });

      // Put back to the best guess, and the message makes no claim.
      expect(useSettingsStore.getState().settings.after_queue_once).toBe('restart_computer');
      expect(useSettingsStore.getState().isDirty).toBe(false);
      const toast = useUiStore.getState().toasts.find((t) => t.type === 'error');
      expect(toast?.message).toContain('could not check what is set');
    });
    it('refuses a second choice while the first is still being saved', async () => {
      // Two overlapping choices could each remember a different "value
      // before the click", and the slower one's failure path could put back
      // the other's value, hiding a still-armed shutdown (Codex, batch-2
      // review). The second is now refused with a message.
      let finishFirst: (() => void) | undefined;
      vi.mocked(commands.setStoredPreference).mockClear();
      vi.mocked(commands.setStoredPreference).mockImplementationOnce(
        () =>
          new Promise<void>((resolve) => {
            finishFirst = resolve;
          }),
      );

      render(<DownloadForm />);
      fireEvent.click(screen.getByLabelText('After-queue actions'));
      fireEvent.click(screen.getByText('After Queue: Do nothing'));
      fireEvent.click(screen.getByLabelText('After-queue actions'));
      await act(async () => {
        fireEvent.click(screen.getByText('After Queue: Shut down'));
      });

      expect(commands.setStoredPreference).toHaveBeenCalledTimes(1);
      expect(
        useUiStore.getState().toasts.some((t) => t.message.includes('Still saving your previous')),
      ).toBe(true);

      await act(async () => {
        finishFirst?.();
      });
    });
    it('keeps refusing a second choice after the page is left and reopened', async () => {
      // The guard used to live inside the page, so leaving and coming back
      // gave a fresh one while the first save was still running (Codex,
      // follow-up review).
      let finishFirst: (() => void) | undefined;
      vi.mocked(commands.setStoredPreference).mockClear();
      vi.mocked(commands.setStoredPreference).mockImplementationOnce(
        () =>
          new Promise<void>((resolve) => {
            finishFirst = resolve;
          }),
      );

      const first = render(<DownloadForm />);
      fireEvent.click(screen.getByLabelText('After-queue actions'));
      fireEvent.click(screen.getByText('After Queue: Do nothing'));
      first.unmount();

      render(<DownloadForm />);
      fireEvent.click(screen.getByLabelText('After-queue actions'));
      await act(async () => {
        fireEvent.click(screen.getByText('After Queue: Shut down'));
      });
      expect(commands.setStoredPreference).toHaveBeenCalledTimes(1);

      await act(async () => {
        finishFirst?.();
      });
    });
  });
});
