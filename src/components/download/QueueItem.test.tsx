/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file QueueItem.test.tsx -- visibility-contract tests for the
 *   per-row queue action buttons.
 *
 * The component itself has dozens of branches (downloading / queued /
 * complete / error / cancelled / processing × showing/hiding cancel,
 * retry, retry-without-wrapper, delete, the artist/album header, the
 * sub-track caption, the warnings collapse, etc.). This file focuses
 * on the **#890 contract**: the "Retry without Wrapper" pill must
 * appear when (and only when) the row is in `error` state AND was
 * originally attempted with wrapper authentication. Users who don't
 * meet both conditions never see the button — and conversely, users
 * who DO meet both conditions must see something visually prominent
 * enough to act on.
 */

import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { QueueItem } from './QueueItem';
import { makeQueueItem } from '@/testing/fixtures';
import type { DownloadState, QueueItemStatus } from '@/types';

// The component pulls in lucide-react icons, the common library
// components (Button / Modal / ContextMenu / ProgressBar /
// ErrorMessageDisplay), and a small handful of Tauri-isms via the
// store. Defaults from the dev environment are sufficient — we
// only need to assert what the conditional render produces.

/** Default no-op callbacks. Individual tests override the ones they care about. */
const noop = vi.fn();
const noopHandlers = {
  onCancel: noop,
  onRetry: noop,
  onRetryWithoutWrapper: noop,
  onDelete: noop,
  onMoveToTop: noop,
  onMoveUp: noop,
  onMoveDown: noop,
  onMoveToBottom: noop,
  onToggleSelect: noop,
  onCopyUrl: noop,
};

/** Build an Error-state item with the supplied wrapper flag. */
function errorItem(overrides: Partial<QueueItemStatus> = {}): QueueItemStatus {
  return makeQueueItem({
    state: 'error' as DownloadState,
    error: 'gamdl.api.exceptions.GamdlApiResponseError: Error fetching wrapper account info',
    ...overrides,
  });
}

describe('QueueItem — "Retry without Wrapper" visibility (#890)', () => {
  it('renders the pill when the item is in error AND used wrapper', () => {
    const item = errorItem({ used_wrapper: true });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    expect(screen.getByTestId('retry-without-wrapper-pill')).toBeTruthy();
    expect(screen.getByText('Retry without Wrapper')).toBeTruthy();
  });

  it('does NOT render the pill when the item is in error but did not use wrapper', () => {
    // Cookie-only download that failed — the wrapper fallback isn't
    // a meaningful action because wrapper wasn't the auth path that
    // failed in the first place.
    const item = errorItem({ used_wrapper: false });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    expect(screen.queryByTestId('retry-without-wrapper-pill')).toBeNull();
  });

  it('does NOT render the pill when the item is downloading (not yet failed)', () => {
    const item = makeQueueItem({
      state: 'downloading' as DownloadState,
      used_wrapper: true,
    });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    expect(screen.queryByTestId('retry-without-wrapper-pill')).toBeNull();
  });

  it('does NOT render the pill when the item completed successfully (used_wrapper irrelevant)', () => {
    const item = makeQueueItem({
      state: 'complete' as DownloadState,
      used_wrapper: true,
    });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    expect(screen.queryByTestId('retry-without-wrapper-pill')).toBeNull();
  });

  it('does NOT render the pill when the item is cancelled (no retry semantics)', () => {
    const item = makeQueueItem({
      state: 'cancelled' as DownloadState,
      used_wrapper: true,
    });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    // Standard Retry should be visible; Retry-without-Wrapper shouldn't.
    expect(screen.queryByTestId('retry-without-wrapper-pill')).toBeNull();
  });

  it('clicking the pill calls onRetryWithoutWrapper with the item id', () => {
    const handler = vi.fn();
    const item = errorItem({ id: 'abc-123', used_wrapper: true });
    render(
      <QueueItem
        item={item}
        isSelected={false}
        canMoveUp
        canMoveDown
        {...noopHandlers}
        onRetryWithoutWrapper={handler}
      />,
    );
    fireEvent.click(screen.getByTestId('retry-without-wrapper-pill').firstChild as Element);
    expect(handler).toHaveBeenCalledWith('abc-123');
  });

  it('has accessible label + tooltip distinguishing it from the standard retry', () => {
    const item = errorItem({ used_wrapper: true });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    const pill = screen.getByLabelText('Retry without wrapper authentication');
    expect(pill).toBeTruthy();
    expect(pill.getAttribute('title')).toContain('cookie-based authentication');
  });
});

/**
 * #911 Phase 1 sub-item — click-to-expand disclosure pattern.
 *
 * Pins the WAI-ARIA Disclosure contract: chevron carries
 * `aria-expanded`, click toggles it, `aria-controls` resolves to the
 * panel's id, and the panel is in the DOM only when expanded.
 */
describe('QueueItem — click-to-expand row affordance (#911-0)', () => {
  it('renders the chevron with aria-expanded="false" initially', () => {
    const item = makeQueueItem({ state: 'queued' });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    const chevron = screen.getByLabelText('Show additional download details');
    expect(chevron.getAttribute('aria-expanded')).toBe('false');
  });

  it('does NOT render the panel before the chevron is clicked', () => {
    const item = makeQueueItem({ state: 'queued' });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    expect(screen.queryByRole('region')).toBeNull();
  });

  it('flips aria-expanded to true and reveals the panel on click', () => {
    const item = makeQueueItem({
      state: 'complete',
      output_path: '/some/path',
      created_at: '2026-06-15T12:00:00.000Z',
    });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    const chevron = screen.getByLabelText('Show additional download details');
    fireEvent.click(chevron);
    expect(chevron.getAttribute('aria-expanded')).toBe('true');
    const panel = screen.getByRole('region');
    expect(panel).toBeTruthy();
  });

  it('aria-controls id on the chevron matches the panel id', () => {
    const item = makeQueueItem({ state: 'complete', output_path: '/x' });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    const chevron = screen.getByLabelText('Show additional download details');
    const controlsId = chevron.getAttribute('aria-controls');
    expect(controlsId).toBeTruthy();
    fireEvent.click(chevron);
    const panel = screen.getByRole('region');
    expect(panel.getAttribute('id')).toBe(controlsId);
  });

  it('collapses the panel and flips aria-expanded back to false on second click', () => {
    const item = makeQueueItem({ state: 'queued' });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    const chevron = screen.getByLabelText('Show additional download details');
    fireEvent.click(chevron); // expand
    fireEvent.click(chevron); // collapse
    expect(chevron.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByRole('region')).toBeNull();
  });

  it('panel exposes the full URL in a monospace selectable cell separate from the row identifier', () => {
    const item = makeQueueItem({
      urls: ['https://music.apple.com/us/album/very/long/9999'],
      state: 'queued',
    });
    render(<QueueItem item={item} isSelected={false} canMoveUp canMoveDown {...noopHandlers} />);
    fireEvent.click(screen.getByLabelText('Show additional download details'));
    // The same URL appears twice (truncated in the row identifier +
    // full inside the panel) so we scope the assertion to the panel
    // region and check the value cell carries the font-mono class
    // that signals the selectable-copy-paste rendering path.
    const panel = screen.getByRole('region');
    const fullUrlCell = panel.querySelector('.font-mono');
    expect(fullUrlCell?.textContent).toBe(
      'https://music.apple.com/us/album/very/long/9999'
    );
  });
});
