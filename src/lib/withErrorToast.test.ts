// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for withErrorToast (audit v2 #1).
 *
 * Pins the helper's contract:
 *   - Success path: returns value + optional success toast
 *   - Error path: returns undefined + error toast (suppressible)
 *   - errorMsg overload: string | (err) => string | omitted
 *   - successVariant: 'success' (default) | 'info'
 *   - suppressOn: case-insensitive substring match on the displayed msg
 *
 * Tests use real uiStore (no IPC dependencies) and inspect its
 * `toasts` array to verify what was emitted.
 */

import { withErrorToast } from '@/lib/withErrorToast';
import { useUiStore } from '@/stores/uiStore';

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
    const result = await withErrorToast(async () => 42);
    expect(result).toBe(42);
  });

  it('emits no toast when no successMsg supplied', async () => {
    await withErrorToast(async () => 'ok');
    expect(getToasts()).toHaveLength(0);
  });

  it('emits a success toast when successMsg supplied', async () => {
    await withErrorToast(async () => 'ok', { successMsg: 'Saved!' });
    const toasts = getToasts();
    expect(toasts).toHaveLength(1);
    expect(toasts[0].message).toBe('Saved!');
    expect(toasts[0].type).toBe('success');
  });

  it('honours successVariant: "info"', async () => {
    await withErrorToast(async () => 'ok', {
      successMsg: 'Reset',
      successVariant: 'info',
    });
    expect(getToasts()[0].type).toBe('info');
  });

  // ===========================================================================
  // Error path
  // ===========================================================================

  it('returns undefined when fn rejects', async () => {
    const result = await withErrorToast(async () => {
      throw new Error('boom');
    });
    expect(result).toBeUndefined();
  });

  it('emits the raw error message when no errorMsg supplied', async () => {
    await withErrorToast(async () => {
      throw new Error('IPC timeout');
    });
    const toasts = getToasts();
    expect(toasts).toHaveLength(1);
    expect(toasts[0].message).toBe('IPC timeout');
    expect(toasts[0].type).toBe('error');
  });

  it('stringifies non-Error rejections', async () => {
    await withErrorToast(async () => {
      throw 'plain string failure';
    });
    expect(getToasts()[0].message).toBe('plain string failure');
  });

  it('uses static string errorMsg verbatim, ignoring the actual error', async () => {
    await withErrorToast(
      async () => {
        throw new Error('underlying noise');
      },
      { errorMsg: 'Failed to save settings' },
    );
    expect(getToasts()[0].message).toBe('Failed to save settings');
  });

  it('uses errorMsg fn to format the displayed text', async () => {
    await withErrorToast(
      async () => {
        throw new Error('disk full');
      },
      { errorMsg: (err) => `Failed to delete crash report: ${err instanceof Error ? err.message : err}` },
    );
    expect(getToasts()[0].message).toBe('Failed to delete crash report: disk full');
  });

  // ===========================================================================
  // suppressOn
  // ===========================================================================

  it('suppresses error toast when displayed message matches a suppressOn pattern (case-insensitive)', async () => {
    await withErrorToast(
      async () => {
        throw new Error('User Cancelled');
      },
      { suppressOn: ['cancel'] },
    );
    expect(getToasts()).toHaveLength(0);
  });

  it('still suppresses when errorMsg fn rewrites to include the pattern', async () => {
    await withErrorToast(
      async () => {
        throw new Error('aborted');
      },
      {
        errorMsg: (err) => `dismissed: ${err}`,
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
      { suppressOn: ['cancel'] },
    );
    expect(getToasts()).toHaveLength(1);
  });

  it('empty suppressOn array does nothing (no special behaviour)', async () => {
    await withErrorToast(
      async () => {
        throw new Error('boom');
      },
      { suppressOn: [] },
    );
    expect(getToasts()).toHaveLength(1);
  });
});
