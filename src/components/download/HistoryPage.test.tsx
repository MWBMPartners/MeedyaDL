// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for HistoryPage's "Clear History" confirmation
 * (code review fix, September 2026).
 *
 * A review found that "Clear History" wiped every entry -- up to 1,000
 * of them -- the moment the button was clicked, with no confirmation
 * at all. That was already out of step with the rest of this same
 * page: removing a single entry asks first, and so does "Retry All
 * Failed" (which doesn't even delete anything). Worse, download
 * history isn't just a list on screen -- duplicate detection's
 * "including history" setting and the smart re-download check both
 * read it, so clearing it changes what the app decides on the NEXT
 * download too.
 *
 * These tests exercise the real `HistoryPage` component (not a
 * re-implementation of its logic) and cover exactly the fix:
 *   - Clicking "Clear History" opens a confirmation modal instead of
 *     clearing immediately.
 *   - The modal explains the loss is permanent AND that it affects
 *     more than this page (duplicate detection / re-download checks).
 *   - `clear_history` is only called once the user confirms.
 *   - Cancelling leaves the history untouched.
 *
 * @see src/components/download/HistoryPage.tsx -- the component under test
 */

import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';

import { HistoryPage } from '@/components/download/HistoryPage';
import { makeFixture } from '@/testing/fixtures';
import { clearHistory, listHistory } from '@/lib/tauri-commands';
import type { HistoryEntry } from '@/types';

/**
 * Mock the Tauri IPC command wrappers HistoryPage calls. Only
 * `listHistory` and `clearHistory` matter for these tests; the rest
 * are plain resolved-value stubs so the component doesn't throw if it
 * touches them incidentally (e.g. on an unrelated re-render).
 */
vi.mock('@/lib/tauri-commands', () => ({
  listHistory: vi.fn(),
  clearHistory: vi.fn().mockResolvedValue(undefined),
  searchHistory: vi.fn().mockResolvedValue([]),
  startDownload: vi.fn().mockResolvedValue(undefined),
  deleteHistoryEntry: vi.fn().mockResolvedValue(undefined),
  resolveRevealPath: vi.fn().mockResolvedValue(''),
}));

/** Build a `HistoryEntry` fixture. No named builder exists yet in
 *  `src/testing/fixtures.ts`, so this uses the generic `makeFixture`
 *  helper rather than hand-rolling another one-off builder in this
 *  file. */
const makeEntry = makeFixture<HistoryEntry>({
  id: 'entry-1',
  url: 'https://music.apple.com/us/album/example/1',
  title: 'Example Track',
  artist: 'Example Artist',
  album: 'Example Album',
  codec: 'alac',
  started_at: '2026-09-01T10:00:00Z',
  completed_at: '2026-09-01T10:05:00Z',
  status: 'success',
});

beforeEach(() => {
  vi.mocked(listHistory).mockReset();
  vi.mocked(clearHistory).mockReset().mockResolvedValue(undefined);
});

describe('HistoryPage -- "Clear History" confirmation', () => {
  it('does not clear anything on click -- opens a confirmation modal instead', async () => {
    vi.mocked(listHistory).mockResolvedValue([makeEntry({ id: 'a' }), makeEntry({ id: 'b' })]);

    render(<HistoryPage />);
    await waitFor(() => expect(screen.getByRole('button', { name: /clear history/i })).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: /clear history/i }));

    // The modal is open and nothing has been deleted yet.
    expect(screen.getByText('Clear Download History')).toBeInTheDocument();
    expect(clearHistory).not.toHaveBeenCalled();
  });

  it("explains the loss is permanent and affects more than this page's list", async () => {
    vi.mocked(listHistory).mockResolvedValue([makeEntry({ id: 'a' })]);

    render(<HistoryPage />);
    await waitFor(() => expect(screen.getByRole('button', { name: /clear history/i })).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /clear history/i }));

    // "Cannot be undone" -- the plain-English promise every other
    // destructive confirmation on this page already makes.
    expect(screen.getByText(/cannot be undone/i)).toBeInTheDocument();
    // The part that makes this specific confirmation necessary: history
    // isn't only a display list, it feeds duplicate detection and the
    // re-download check, so clearing it changes future behaviour too.
    expect(screen.getByText(/duplicate downloads/i)).toBeInTheDocument();
    expect(screen.getByText(/next time you download something/i)).toBeInTheDocument();
  });

  it('clears history only after the user confirms', async () => {
    vi.mocked(listHistory).mockResolvedValue([makeEntry({ id: 'a' })]);

    render(<HistoryPage />);
    await waitFor(() => expect(screen.getByRole('button', { name: /clear history/i })).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /clear history/i }));

    // Confirm inside the modal -- its button carries the same visible
    // text as the trigger ("Clear History"), so this asserts on the
    // one inside the open dialog rather than assuming there's only one
    // match on the page.
    const dialog = screen.getByRole('dialog');
    fireEvent.click(
      within(dialog).getByRole('button', { name: /clear history/i })
    );

    await waitFor(() => expect(clearHistory).toHaveBeenCalledTimes(1));
    // Modal closes and the list empties once the clear resolves.
    await waitFor(() => expect(screen.queryByText('Clear Download History')).not.toBeInTheDocument());
    expect(screen.getByText(/no download history yet/i)).toBeInTheDocument();
  });

  it('Cancel leaves the history untouched', async () => {
    vi.mocked(listHistory).mockResolvedValue([makeEntry({ id: 'a' })]);

    render(<HistoryPage />);
    await waitFor(() => expect(screen.getByRole('button', { name: /clear history/i })).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: /clear history/i }));
    expect(screen.getByText('Clear Download History')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /^cancel$/i }));

    expect(screen.queryByText('Clear Download History')).not.toBeInTheDocument();
    expect(clearHistory).not.toHaveBeenCalled();
  });
});
