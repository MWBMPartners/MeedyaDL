// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for useInterfaceLanguageOptions (independent review,
 * round 3 of #1244).
 *
 * Pins the contract GeneralTab.tsx's Interface Language dropdown relies
 * on:
 *   - the order the backend (shared `meedya-lang` code) sends back is
 *     used, and it is asked with the interface language as the SOLE
 *     preference;
 *   - if the backend fails, the alphabetical fallback is shown instead --
 *     and for the three locales MeedyaDL actually ships, that fallback
 *     already reads exactly right for both interface languages this round
 *     of review named: "English, French, German" when the interface is
 *     English, "Deutsch, Englisch, Französisch" when it is German. (For
 *     this particular set of three names, the interface language happens
 *     to sort first alphabetically too -- "English" and "Deutsch" both
 *     start earliest in their own alphabets -- so the two rules give the
 *     same answer here; that is a property of these three names, not a
 *     coincidence this test relies on for languages in general.)
 */

import { renderHook, waitFor } from '@testing-library/react';

import * as commands from '@/lib/tauri-commands';
import { useInterfaceLanguageOptions } from '@/hooks/useInterfaceLanguageOptions';
import { AVAILABLE_LOCALES } from '@/lib/i18n';

vi.mock('@/lib/tauri-commands', () => ({
  orderLanguagesForDisplay: vi.fn(),
}));

const order = vi.mocked(commands.orderLanguagesForDisplay);

describe('useInterfaceLanguageOptions', () => {
  beforeEach(() => {
    order.mockReset();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('uses the order the backend sends back, asked with the interface language as the sole preference', async () => {
    // A recognisable order: the offered codes reversed.
    order.mockImplementation(async (tags) => [...tags].reverse());
    const { result } = renderHook(() => useInterfaceLanguageOptions('en'));
    await waitFor(() => expect(result.current).toEqual([...AVAILABLE_LOCALES].reverse()));
    const [tags, preferences] = order.mock.calls[0];
    expect(tags).toEqual(AVAILABLE_LOCALES);
    expect(preferences).toEqual(['en']);
  });

  it('shows English, French, German when the backend fails and the interface is English', async () => {
    order.mockRejectedValue(new Error('command not available'));
    const { result } = renderHook(() => useInterfaceLanguageOptions('en'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    expect(result.current).toEqual(['en', 'fr', 'de']);
  });

  it('shows Deutsch, Englisch, Französisch when the backend fails and the interface is German', async () => {
    order.mockRejectedValue(new Error('command not available'));
    const { result } = renderHook(() => useInterfaceLanguageOptions('de'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    expect(result.current).toEqual(['de', 'en', 'fr']);
  });

  it('ignores a backend answer that lost an entry', async () => {
    order.mockImplementation(async (tags) => tags.slice(1));
    const { result } = renderHook(() => useInterfaceLanguageOptions('en'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    expect(result.current).toHaveLength(AVAILABLE_LOCALES.length);
  });

  it('re-asks and re-orders when the interface language changes', async () => {
    order.mockImplementation(async (tags) => [...tags]);
    const { result, rerender } = renderHook(({ ui }) => useInterfaceLanguageOptions(ui), {
      initialProps: { ui: 'en' },
    });
    await waitFor(() => expect(order).toHaveBeenCalledTimes(1));
    rerender({ ui: 'de' });
    await waitFor(() => expect(order).toHaveBeenCalledTimes(2));
    const [, secondPreferences] = order.mock.calls[1];
    expect(secondPreferences).toEqual(['de']);
    // `order` being called twice only means the mock function was invoked
    // -- not that its promise has resolved and the hook's state has caught
    // up. Wrapped in its own `waitFor` so this does not read a stale value
    // left over from the first (still-fine, but now superseded) answer.
    await waitFor(() => expect(result.current).toEqual(AVAILABLE_LOCALES));
  });
});

describe('useInterfaceLanguageOptions when the IPC call throws before returning a promise', () => {
  it('still shows the whole list, alphabetically', async () => {
    order.mockReset();
    order.mockImplementation(() => {
      throw new Error('IPC bridge missing');
    });
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    const { result } = renderHook(() => useInterfaceLanguageOptions('en'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    expect(result.current).toHaveLength(AVAILABLE_LOCALES.length);
    vi.restoreAllMocks();
  });
});
