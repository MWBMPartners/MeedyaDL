// Copyright (c) 2026 MeedyaSuite
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
 *     with "GAMDL bug" — not for arbitrary errors
 *   - The pre-filled GitHub URL contains the error text + source URL,
 *     and never references MeedyaDL (upstream maintainers want a
 *     clean GAMDL-user-shaped report)
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { ErrorMessageDisplay } from '@/components/common/ErrorMessageDisplay';
import { useUiStore } from '@/stores/uiStore';

// Capture what the shell plugin would have opened.
const openMock = vi.fn().mockResolvedValue(undefined);
vi.mock('@tauri-apps/plugin-shell', () => ({
  open: (url: string) => openMock(url),
}));

beforeEach(() => {
  openMock.mockClear();
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

  it('shows the GAMDL report option for messages prefixed "GAMDL bug"', () => {
    render(
      <ErrorMessageDisplay message="GAMDL bug — music-video cover URL not templated" />
    );
    fireEvent.contextMenu(
      screen.getByText('GAMDL bug — music-video cover URL not templated')
    );
    expect(screen.getByText('Report this bug to GAMDL')).toBeInTheDocument();
  });

  it('Report opens upstream GAMDL issues/new with the error in the body', async () => {
    render(
      <ErrorMessageDisplay
        message="GAMDL bug — music-video cover URL not templated."
        sourceUrl="https://music.apple.com/us/album/foo/123"
      />
    );
    fireEvent.contextMenu(
      screen.getByText('GAMDL bug — music-video cover URL not templated.')
    );
    await act(async () => {
      fireEvent.click(screen.getByText('Report this bug to GAMDL'));
    });

    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(1));
    const url = openMock.mock.calls[0][0];
    expect(url).toMatch(/^https:\/\/github\.com\/glomatico\/gamdl\/issues\/new\?/);
    // Use URL.searchParams to decode `+`-as-space form-encoding
    // properly (decodeURIComponent leaves `+` alone).
    const params = new URL(url).searchParams;
    // Title strips the "GAMDL bug — " prefix so it reads as a user summary.
    expect(params.get('title')).toBe('music-video cover URL not templated.');
    // Body contains the failed URL + the original error text.
    const body = params.get('body') ?? '';
    expect(body).toContain('https://music.apple.com/us/album/foo/123');
    expect(body).toContain('GAMDL bug — music-video cover URL not templated.');
  });

  it('GitHub URL never references MeedyaDL (upstream wants a clean GAMDL report)', async () => {
    render(
      <ErrorMessageDisplay
        message="GAMDL bug — sample defect"
        sourceUrl="https://music.apple.com/us/album/x/1"
      />
    );
    fireEvent.contextMenu(screen.getByText('GAMDL bug — sample defect'));
    await act(async () => {
      fireEvent.click(screen.getByText('Report this bug to GAMDL'));
    });
    await waitFor(() => expect(openMock).toHaveBeenCalledTimes(1));
    const params = new URL(openMock.mock.calls[0][0]).searchParams;
    const wholeText = `${params.get('title')}\n${params.get('body')}`;
    expect(wholeText.toLowerCase()).not.toContain('meedyadl');
  });

  it('omits the URL block from the body when sourceUrl is not provided', async () => {
    render(<ErrorMessageDisplay message="GAMDL bug — defect with no URL context" />);
    fireEvent.contextMenu(screen.getByText('GAMDL bug — defect with no URL context'));
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
});
