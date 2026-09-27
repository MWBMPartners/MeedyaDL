// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for ErrorMessageDisplay.
 *
 * Pins the contract:
 *   - Renders the message text
 *   - Returns null when message is empty
 *   - Right-click opens a context menu
 *   - "Copy error message" writes to navigator.clipboard
 *   - "Report this bug to GAMDL" appears ONLY for messages prefixed
 *     with the real `GAMDL_BUG_MARKER` — not for arbitrary errors
 *   - The pre-filled GitHub URL contains the error text + source URL,
 *     and never references MeedyaDL (upstream maintainers want a
 *     clean GAMDL-user-shaped report)
 *   - The link is built from the CLEANED text (the backend's
 *     `redact_for_public_report` command), never the raw message
 *   - A failed clean opens nothing and shows an error toast (#1231)
 *
 * `REAL_GAMDL_BUG_MESSAGE` below is the actual wording MeedyaDL's
 * backend emits — see `process::gamdl_mv_cover_template_bug_message`
 * in `src-tauri/src/utils/process.rs`. It used to be a made-up
 * sentence that happened to start with "GAMDL bug", which is exactly
 * how issue #1231 went unnoticed: the test passed while the real
 * feature was completely disconnected, because nothing in the backend
 * ever produced a message shaped the way this test pretended it did.
 * Using the real wording here means a future change to either side
 * that breaks the connection fails this test, not just a live user.
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { ErrorMessageDisplay } from '@/components/common/ErrorMessageDisplay';
import { useUiStore } from '@/stores/uiStore';

// Capture what the shell plugin would have opened.
const openMock = vi.fn().mockResolvedValue(undefined);
vi.mock('@tauri-apps/plugin-shell', () => ({
  open: (url: string) => openMock(url),
}));

// The backend's cleaning command. Default behaviour is "identity" —
// hand the text straight back — so tests that aren't specifically
// about cleaning don't have to think about it. Individual tests
// override this with `mockResolvedValueOnce` / `mockRejectedValueOnce`
// to check the wiring itself.
const redactForPublicReportMock = vi.fn((text: string) => Promise.resolve(text));
// The whole-address cleaner used for the download link (#1231).
const redactUrlForPublicReportMock = vi.fn((url: string) => Promise.resolve(url));
vi.mock('@/lib/tauri-commands', () => ({
  redactForPublicReport: (text: string) => redactForPublicReportMock(text),
  redactUrlForPublicReport: (url: string) => redactUrlForPublicReportMock(url),
}));

/**
 * The real message MeedyaDL's backend emits for the music-video
 * cover-art templating bug — see
 * `process::gamdl_mv_cover_template_bug_message` in
 * `src-tauri/src/utils/process.rs`. This is a TypeScript test file,
 * so the Rust wording can't be imported; it's copied here as a
 * literal on purpose, so a wording change on either side that breaks
 * the connection is caught by a human noticing this test fail, not by
 * the two sides silently drifting apart again.
 */
const REAL_GAMDL_BUG_MESSAGE =
  'GAMDL bug — music video cover art: 3 track(s) skipped. Audio for those ' +
  'tracks did not download. This is an upstream bug (Apple returns 400 Bad ' +
  'Request because GAMDL sends literal `{w}x{h}` placeholders instead of ' +
  "real dimensions). The album cover is still attached separately during " +
  "MeedyaDL's enrichment pass. Please report at " +
  'https://github.com/glomatico/gamdl/issues.';

/**
 * The OTHER real marker-prefixed backend message — the truncated-write
 * defect (gamdl#328) described in
 * `download_queue::helpers::integrity_failure_message` in
 * `src-tauri/src/services/download_queue/helpers.rs`. Kept as a
 * separate literal (rather than reusing `REAL_GAMDL_BUG_MESSAGE`
 * everywhere) because it does NOT mention "MeedyaDL", which some tests
 * below specifically need.
 */
const REAL_GAMDL_BUG_MESSAGE_NO_MEEDYADL_MENTION =
  'GAMDL bug — all 3 probed file(s) appear corrupted or truncated ' +
  '(01 Track.m4a, 02 Track.m4a, 03 Track.m4a) — a known upstream ' +
  'truncated-write defect (gamdl#328). Try re-downloading.';

beforeEach(() => {
  openMock.mockClear();
  redactForPublicReportMock.mockClear();
  redactForPublicReportMock.mockImplementation((text: string) => Promise.resolve(text));
  redactUrlForPublicReportMock.mockClear();
  redactUrlForPublicReportMock.mockImplementation((url: string) => Promise.resolve(url));
  useUiStore.setState({ toasts: [] });
  // jsdom's clipboard is undefined by default — provide a writeText spy.
  Object.defineProperty(navigator, 'clipboard', {
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
    configurable: true,
    writable: true,
  });
});

describe('ErrorMessageDisplay', () => {
  // ===========================================================================
  // Rendering
  // ===========================================================================

  it('renders the message text', () => {
    render(<ErrorMessageDisplay message="disk full" />);
    expect(screen.getByText('disk full')).toBeInTheDocument();
  });

  it('returns null when message is empty', () => {
    const { container } = render(<ErrorMessageDisplay message="" />);
    expect(container.innerHTML).toBe('');
  });

  it('applies line-clamp-2 by default', () => {
    render(<ErrorMessageDisplay message="boom" />);
    expect(screen.getByText('boom').className).toMatch(/line-clamp-2/);
  });

  it('honours truncateLines={null} (no clamp class)', () => {
    render(<ErrorMessageDisplay message="boom" truncateLines={null} />);
    const el = screen.getByText('boom');
    expect(el.className).not.toMatch(/line-clamp/);
  });

  it('honours custom truncateLines via the literal lookup', () => {
    render(<ErrorMessageDisplay message="boom" truncateLines={3} />);
    expect(screen.getByText('boom').className).toMatch(/line-clamp-3/);
  });

  // ===========================================================================
  // Context menu
  // ===========================================================================

  it('right-click opens a context menu with "Copy error message"', () => {
    render(<ErrorMessageDisplay message="boom" />);
    fireEvent.contextMenu(screen.getByText('boom'));
    expect(screen.getByText('Copy error message')).toBeInTheDocument();
  });

  it('Copy writes the message to navigator.clipboard', async () => {
    render(<ErrorMessageDisplay message="boom-copy" />);
    fireEvent.contextMenu(screen.getByText('boom-copy'));
    await act(async () => {
      fireEvent.click(screen.getByText('Copy error message'));
    });
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('boom-copy');
  });

  // ===========================================================================
  // GAMDL-bug detection + GitHub link
  // ===========================================================================

  it('hides the GAMDL report option for non-GAMDL errors', () => {
    render(<ErrorMessageDisplay message="Network timeout" />);
    fireEvent.contextMenu(screen.getByText('Network timeout'));
    expect(screen.queryByText('Report this bug to GAMDL')).not.toBeInTheDocument();
  });

  it('shows the GAMDL report option for a real backend "known GAMDL bug" message', () => {
    render(<ErrorMessageDisplay message={REAL_GAMDL_BUG_MESSAGE} />);
    fireEvent.contextMenu(screen.getByText(REAL_GAMDL_BUG_MESSAGE));
    expect(screen.getByText('Report this bug to GAMDL')).toBeInTheDocument();
  });

  it('Report opens upstream GAMDL issues/new with the (cleaned) error in the body', async () => {
    render(
      <ErrorMessageDisplay
        message={REAL_GAMDL_BUG_MESSAGE}
        sourceUrl="https://music.apple.com/us/album/foo/123"
      />
    );
    fireEvent.contextMenu(screen.getByText(REAL_GAMDL_BUG_MESSAGE));
    await act(async () => {
      fireEvent.click(screen.getByText('Report this bug to GAMDL'));
    });

    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(1));
    // The identity mock means "cleaned" == the original text here — see
    // the dedicated cleaning tests below for the case where it differs.
    expect(redactForPublicReportMock).toHaveBeenCalledWith(REAL_GAMDL_BUG_MESSAGE);
    const url = openMock.mock.calls[0][0];
    expect(url).toMatch(/^https:\/\/github\.com\/glomatico\/gamdl\/issues\/new\?/);
    // Use URL.searchParams to decode `+`-as-space form-encoding
    // properly (decodeURIComponent leaves `+` alone).
    const params = new URL(url).searchParams;
    // Title strips the "GAMDL bug — " prefix so it reads as a user
    // summary — the exact wording is the real backend's first sentence.
    expect(params.get('title')).toBe('music video cover art: 3 track(s) skipped.');
    // Body contains the failed URL + the original (cleaned) error text.
    const body = params.get('body') ?? '';
    expect(body).toContain('https://music.apple.com/us/album/foo/123');
    expect(body).toContain(REAL_GAMDL_BUG_MESSAGE);
  });

  it('GitHub URL never references MeedyaDL (upstream wants a clean GAMDL report)', async () => {
    // Uses the OTHER real marker-prefixed backend message (the
    // gamdl#328 truncated-write defect) because the cover-art message
    // above legitimately names MeedyaDL — it's describing what
    // MeedyaDL's own enrichment step still does afterwards, which is
    // real content, not the boilerplate branding this test guards
    // against.
    render(
      <ErrorMessageDisplay
        message={REAL_GAMDL_BUG_MESSAGE_NO_MEEDYADL_MENTION}
        sourceUrl="https://music.apple.com/us/album/x/1"
      />
    );
    fireEvent.contextMenu(screen.getByText(REAL_GAMDL_BUG_MESSAGE_NO_MEEDYADL_MENTION));
    await act(async () => {
      fireEvent.click(screen.getByText('Report this bug to GAMDL'));
    });
    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(1));
    const params = new URL(openMock.mock.calls[0][0]).searchParams;
    const wholeText = `${params.get('title')}\n${params.get('body')}`;
    expect(wholeText.toLowerCase()).not.toContain('meedyadl');
  });

  it('omits the URL block from the body when sourceUrl is not provided', async () => {
    render(<ErrorMessageDisplay message={REAL_GAMDL_BUG_MESSAGE_NO_MEEDYADL_MENTION} />);
    fireEvent.contextMenu(screen.getByText(REAL_GAMDL_BUG_MESSAGE_NO_MEEDYADL_MENTION));
    await act(async () => {
      fireEvent.click(screen.getByText('Report this bug to GAMDL'));
    });
    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(1));
    const body = new URL(openMock.mock.calls[0][0]).searchParams.get('body') ?? '';
    // No "### URL" section when sourceUrl is omitted
    expect(body).not.toContain('### URL');
    // But the error block is still present
    expect(body).toContain('### Error output');
  });

  // ===========================================================================
  // Cleaning before a PUBLIC, third-party report (#1231)
  // ===========================================================================

  it('builds the GitHub link from the CLEANED text, not the raw message', async () => {
    // Simulate the backend's redaction actually removing something —
    // an account name in a file path and a token in a web address —
    // and check the built link reflects what came BACK from cleaning,
    // never the raw text that went in.
    const raw = `${REAL_GAMDL_BUG_MESSAGE} Output was /Users/alice/Music, wrapper http://127.0.0.1:30020/account?token=SECRET`;
    const cleaned = `${REAL_GAMDL_BUG_MESSAGE} Output was /Users/{user}/Music, wrapper http://127.0.0.1:30020/account`;
    redactForPublicReportMock.mockResolvedValueOnce(cleaned);

    render(<ErrorMessageDisplay message={raw} />);
    fireEvent.contextMenu(screen.getByText(raw));
    await act(async () => {
      fireEvent.click(screen.getByText('Report this bug to GAMDL'));
    });

    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(1));
    expect(redactForPublicReportMock).toHaveBeenCalledWith(raw);
    const body = new URL(openMock.mock.calls[0][0]).searchParams.get('body') ?? '';
    expect(body).toContain(cleaned);
    expect(body).not.toContain('alice');
    expect(body).not.toContain('SECRET');
  });

  it('cleans the download link too, not only the message (Codex, review of 7edd178c)', async () => {
    const rawLink = "https://music.apple.com/us/album/test/123?token=abc'SECRET123";
    const cleanedLink = 'https://music.apple.com/us/album/test/123';
    redactUrlForPublicReportMock.mockImplementation((url: string) =>
      Promise.resolve(url === rawLink ? cleanedLink : url),
    );

    render(<ErrorMessageDisplay message={REAL_GAMDL_BUG_MESSAGE} sourceUrl={rawLink} />);
    fireEvent.contextMenu(screen.getByText(REAL_GAMDL_BUG_MESSAGE));
    await act(async () => {
      fireEvent.click(screen.getByText('Report this bug to GAMDL'));
    });

    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(1));
    // The whole-address cleaner, never the free-text one, gets the link.
    expect(redactUrlForPublicReportMock).toHaveBeenCalledWith(rawLink);
    expect(redactForPublicReportMock).not.toHaveBeenCalledWith(rawLink);
    const body = new URL(openMock.mock.calls[0][0]).searchParams.get('body') ?? '';
    expect(body).toContain(cleanedLink);
    expect(body).not.toContain('SECRET123');
  });

  it('does not open anything when the cleaning step fails, and shows an error toast instead', async () => {
    redactForPublicReportMock.mockRejectedValueOnce(new Error('IPC bridge unavailable'));

    render(<ErrorMessageDisplay message={REAL_GAMDL_BUG_MESSAGE} />);
    fireEvent.contextMenu(screen.getByText(REAL_GAMDL_BUG_MESSAGE));
    await act(async () => {
      fireEvent.click(screen.getByText('Report this bug to GAMDL'));
    });

    // The toast is the signal that the failure was actually handled —
    // wait for it instead of an arbitrary tick, then confirm nothing
    // was ever opened with the raw, unredacted text.
    await waitFor(() => {
      expect(
        useUiStore
          .getState()
          .toasts.some((t) => t.message === 'Could not prepare the report safely'),
      ).toBe(true);
    });
    expect(openMock).not.toHaveBeenCalled();
  });
});
