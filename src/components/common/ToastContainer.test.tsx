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

import { act, fireEvent, render, screen, within } from '@testing-library/react';
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
    // Scoped to the visible stack: the hidden screen-reader announcers
    // (#1431) carry the same text.
    const message = within(screen.getByTestId('toast-stack')).getByText(LONG.trim(), { exact: false });
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

  // ── #1431: screen readers and pausing ──────────────────────────────

  it('announces an error urgently and anything else politely', () => {
    act(() =>
      useUiStore.setState({
        toasts: [
          { id: 'a', message: 'Saved', type: 'success', duration: 0, expiresAt: null },
          { id: 'b', message: 'It broke', type: 'error', duration: 0, expiresAt: null },
        ],
      })
    );
    render(<ToastContainer />);
    expect(screen.getByRole('alert')).toHaveTextContent('It broke');
    expect(screen.getByRole('status')).toHaveTextContent('Saved');
    // The stack itself is no longer one polite region holding errors too.
    expect(screen.getByTestId('toast-stack')).not.toHaveAttribute('role', 'status');
  });

  it('keeps a toast open while the pointer is over the stack', () => {
    vi.useFakeTimers();
    try {
      act(() => useUiStore.getState().addToast('Hover me', 'info', 1000));
      render(<ToastContainer />);
      const stack = screen.getByTestId('toast-stack');
      fireEvent.mouseEnter(stack);
      act(() => {
        vi.advanceTimersByTime(3000);
      });
      expect(useUiStore.getState().toasts).toHaveLength(1);
      fireEvent.mouseLeave(stack);
      act(() => {
        vi.advanceTimersByTime(1500);
      });
      expect(useUiStore.getState().toasts).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it('announces only a toast that has just arrived, and falls silent when it goes', () => {
    act(() =>
      useUiStore.setState({
        toasts: [{ id: 'old', message: 'Older error', type: 'error', duration: 0, expiresAt: null }],
      })
    );
    render(<ToastContainer />);
    act(() =>
      useUiStore.setState((st) => ({
        toasts: [
          ...st.toasts,
          { id: 'new', message: 'Newer error', type: 'error', duration: 0, expiresAt: null },
        ],
      }))
    );
    expect(screen.getByRole('alert')).toHaveTextContent('Newer error');
    // The announced one goes: silence, never the older one again.
    act(() => useUiStore.getState().removeToast('new'));
    expect(screen.getByRole('alert')).toHaveTextContent('');
    expect(screen.getByRole('alert')).not.toHaveTextContent('Older error');
  });

  it('resumes auto-dismiss when the toast under the pointer is removed without a pointer-leave', () => {
    vi.useFakeTimers();
    try {
      act(() => {
        useUiStore.getState().addToast('Under the pointer', 'info', 0);
        useUiStore.getState().addToast('Timed one', 'info', 1000);
      });
      render(<ToastContainer />);
      fireEvent.mouseEnter(screen.getByTestId('toast-stack'));
      // Remove the toast under the pointer the way WebKit does: no
      // mouseleave reaches the stack.
      act(() => {
        const id = useUiStore.getState().toasts[0].id;
        useUiStore.getState().removeToast(id);
      });
      act(() => {
        vi.advanceTimersByTime(1500);
      });
      expect(useUiStore.getState().toasts).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it('resumes auto-dismiss when the focused toast is removed without a blur', () => {
    vi.useFakeTimers();
    try {
      act(() => {
        useUiStore.getState().addToast('Focused one', 'info', 0);
        useUiStore.getState().addToast('Timed one', 'info', 1000);
      });
      render(<ToastContainer />);
      const close = within(screen.getByTestId('toast-stack')).getAllByRole('button', {
        name: 'Dismiss',
      })[0];
      act(() => close.focus());
      fireEvent.focus(close);
      // Remove the focused toast's element the way WebKit does: no blur.
      act(() => {
        const id = useUiStore.getState().toasts[0].id;
        useUiStore.getState().removeToast(id);
      });
      act(() => {
        vi.advanceTimersByTime(1500);
      });
      expect(useUiStore.getState().toasts).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });
});
