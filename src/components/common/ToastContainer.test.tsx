// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for the toast stack (polish pass M7): long messages are cut
 * to three lines with "Show more", and the stack starts below any area a
 * page marks to keep clear.
 *
 * jsdom does no layout, so the measurements the component reads
 * (scrollHeight, clientHeight, getBoundingClientRect) are given fixed
 * values here; the headless-browser run on the production build checks
 * the real thing at three window sizes.
 */

import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ToastContainer } from './ToastContainer';
import { useUiStore } from '@/stores/uiStore';
import { KEEP_CLEAR_ATTRIBUTE, toastStackTop } from '@/lib/toastPlacement';

const LONG = 'Could not save your settings. '.repeat(20);

/** Make every <p> look taller than its box (text cut) or not. */
function pretendTextIsCut(cut: boolean) {
  vi.spyOn(HTMLElement.prototype, 'scrollHeight', 'get').mockImplementation(function (this: HTMLElement) {
    return this.tagName === 'P' && cut ? 200 : 0;
  });
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockImplementation(function (this: HTMLElement) {
    return this.tagName === 'P' ? 60 : 0;
  });
}

beforeEach(() => {
  act(() => useUiStore.setState({ toasts: [] }));
});
afterEach(() => {
  vi.restoreAllMocks();
  document.querySelectorAll(`[${KEEP_CLEAR_ATTRIBUTE}]`).forEach((el) => el.remove());
});

describe('ToastContainer', () => {
  it('cuts a long message to three lines and offers "Show more"', () => {
    pretendTextIsCut(true);
    act(() => useUiStore.setState({ toasts: [{ id: 't1', message: LONG, type: 'error', duration: 0 }] }));
    render(<ToastContainer />);
    const message = screen.getByText(LONG.trim(), { exact: false });
    expect(message.className).toContain('line-clamp-3');
    const more = screen.getByRole('button', { name: 'Show more' });
    expect(more).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(more);
    expect(message.className).not.toContain('line-clamp-3');
    expect(screen.getByRole('button', { name: 'Show less' })).toHaveAttribute('aria-expanded', 'true');
  });

  it('offers no "Show more" when the whole message already fits', () => {
    pretendTextIsCut(false);
    act(() => useUiStore.setState({ toasts: [{ id: 't1', message: 'Saved.', type: 'success', duration: 0 }] }));
    render(<ToastContainer />);
    expect(screen.queryByRole('button', { name: 'Show more' })).toBeNull();
  });

  it('starts the stack below an area the page marks to keep clear', () => {
    const toolbar = document.createElement('div');
    toolbar.setAttribute(KEEP_CLEAR_ATTRIBUTE, '');
    toolbar.getBoundingClientRect = () => ({ top: 135, bottom: 225, left: 0, right: 500, width: 500, height: 90, x: 0, y: 135, toJSON: () => ({}) });
    document.body.appendChild(toolbar);
    expect(toastStackTop()).toBe(233);
    act(() => useUiStore.setState({ toasts: [{ id: 't1', message: 'Hello', type: 'info', duration: 0 }] }));
    render(<ToastContainer />);
    expect(screen.getByTestId('toast-stack').style.top).toBe('233px');
  });

  it('starts at 80px when nothing on the page is marked', () => {
    expect(toastStackTop()).toBe(80);
  });
});
