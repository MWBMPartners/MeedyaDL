// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for withErrorToast (audit v2 #1; polish pass M8).
 *
 * Pins the helper's contract:
 *   - Success path: returns value + optional success toast
 *   - Error path: returns undefined + error toast (suppressible)
 *   - The message is always the caller's plain sentence; the thrown
 *     error's own text goes under "Details", never as the message
 *   - successVariant: 'success' (default) | 'info'
 *   - suppressOn: case-insensitive match on the thrown text or the message
 *
 * Tests use real uiStore (no IPC dependencies) and inspect its
 * `toasts` array to verify what was emitted.
 */

import { withErrorToast } from '@/lib/withErrorToast';
import { useUiStore } from '@/stores/uiStore';

const PLAIN = 'MeedyaDL could not save your settings. Try again.';

beforeEach(() => {
  useUiStore.setState({ toasts: [] });
});

/** Convenience: the toasts currently in the store, oldest first. */
function getToasts() {
  return useUiStore.getState().toasts;
}

describe('withErrorToast', () => {
  // ===========================================================================
  // Success path
  // ===========================================================================

  it('returns the resolved value when fn succeeds', async () => {
    const result = await withErrorToast(async () => 42, { errorMsg: PLAIN });
    expect(result).toBe(42);
  });

  it('emits no toast when no successMsg supplied', async () => {
    await withErrorToast(async () => 'ok', { errorMsg: PLAIN });
    expect(getToasts()).toHaveLength(0);
  });

  it('emits a success toast when successMsg supplied', async () => {
    await withErrorToast(async () => 'ok', { successMsg: 'Saved!', errorMsg: PLAIN });
    const toasts = getToasts();
    expect(toasts).toHaveLength(1);
    expect(toasts[0].message).toBe('Saved!');
    expect(toasts[0].type).toBe('success');
  });

  it('honours successVariant: "info"', async () => {
    await withErrorToast(async () => 'ok', {
      successMsg: 'Reset',
      successVariant: 'info',
      errorMsg: PLAIN,
    });
    expect(getToasts()[0].type).toBe('info');
  });

  // ===========================================================================
  // Error path
  // ===========================================================================

  it('returns undefined when fn rejects', async () => {
    const result = await withErrorToast(async () => {
      throw new Error('boom');
    }, { errorMsg: PLAIN });
    expect(result).toBeUndefined();
  });

  it('shows the plain message, with the thrown text folded under Details', async () => {
    await withErrorToast(
      async () => {
        throw new Error('IPC timeout (os error 60)');
      },
      { errorMsg: PLAIN },
    );
    const [toast] = getToasts();
    expect(toast.message).toBe(PLAIN);
    expect(toast.type).toBe('error');
    expect(toast.details).toBe('IPC timeout (os error 60)');
    // Never the raw text as the message.
    expect(toast.message).not.toContain('os error');
  });

  it('turns a non-Error rejection into text for Details', async () => {
    await withErrorToast(async () => {
      throw 'plain string failure';
    }, { errorMsg: PLAIN });
    expect(getToasts()[0].details).toBe('plain string failure');
  });

  it('a message function can choose the sentence from what was thrown', async () => {
    await withErrorToast(
      async () => {
        throw new Error('disk full');
      },
      { errorMsg: (err) => (String(err).includes('disk') ? 'There is no room left on that disk. Free some space, then try again.' : PLAIN) },
    );
    expect(getToasts()[0].message).toBe('There is no room left on that disk. Free some space, then try again.');
  });

  // ===========================================================================
  // suppressOn
  // ===========================================================================

  it('suppresses the toast when the thrown text matches a suppressOn pattern (case-insensitive)', async () => {
    await withErrorToast(
      async () => {
        throw new Error('User Cancelled');
      },
      { errorMsg: PLAIN, suppressOn: ['cancel'] },
    );
    expect(getToasts()).toHaveLength(0);
  });

  it('still suppresses when the message matches', async () => {
    await withErrorToast(
      async () => {
        throw new Error('aborted');
      },
      {
        errorMsg: () => 'dismissed by you',
        suppressOn: ['dismissed'],
      },
    );
    expect(getToasts()).toHaveLength(0);
  });

  it('does NOT suppress when no suppressOn pattern matches', async () => {
    await withErrorToast(
      async () => {
        throw new Error('Real failure');
      },
      { errorMsg: PLAIN, suppressOn: ['cancel'] },
    );
    expect(getToasts()).toHaveLength(1);
  });

  it('empty suppressOn array does nothing (no special behaviour)', async () => {
    await withErrorToast(
      async () => {
        throw new Error('boom');
      },
      { errorMsg: PLAIN, suppressOn: [] },
    );
    expect(getToasts()).toHaveLength(1);
  });
});
