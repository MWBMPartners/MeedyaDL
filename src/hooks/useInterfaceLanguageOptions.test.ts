// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for useInterfaceLanguageOptions (independent review,
 * rounds 3 to 5 of #1244).
 *
 * Pins the contract GeneralTab.tsx's Interface Language dropdown relies on:
 *   - the order the backend sends back is used, asked with the interface
 *     language as the SOLE preference;
 *   - if the backend fails, a fallback is shown: the interface language's
 *     whole primary-language group first, then the rest alphabetical by
 *     name in that language.
 *
 * The fallback must never visibly JUMP when the real answer arrives, which
 * can only be checked against what the backend really says. So the table in
 * "the fallback matches the real backend order" is not worked out by hand:
 * it is copied from the Rust test
 * `the_real_order_for_every_interface_language_case_the_frontend_fallback_mirrors`
 * (`src-tauri/src/utils/language.rs`), which runs the real `order_for_display`
 * for each case with the alphabetical order `Intl` gives (en/de/fr offered).
 * Round 4 only tried bare codes (`en`, `fr`), not the full tags "Auto (System)"
 * really sends (`fr-FR`, `fr-CA`...), so its exact-match pin passed every test
 * and still jumped; the reviewer's `fr-FR` probe is in the table.
 */

import { renderHook, waitFor } from '@testing-library/react';

import * as commands from '@/lib/tauri-commands';
import { useInterfaceLanguageOptions } from '@/hooks/useInterfaceLanguageOptions';
import { AVAILABLE_LOCALES } from '@/lib/i18n';

vi.mock('@/lib/tauri-commands', () => ({
  orderLanguagesForDisplay: vi.fn(),
  // Left failing: the browser's reading is then used, which agrees with the
  // backend for en/de/fr and their regional forms (the table above).
  languageIdentities: vi.fn(() => Promise.reject(new Error('not mocked here'))),
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

  it.each([
    ['en', ['en', 'fr', 'de']],
    ['de', ['de', 'en', 'fr']],
    ['fr', ['fr', 'de', 'en']],
    ['fr-FR', ['fr', 'de', 'en']],
    ['fr-CA', ['fr', 'de', 'en']],
    ['de-DE', ['de', 'en', 'fr']],
    ['en-GB', ['en', 'fr', 'de']],
    ['es', ['de', 'fr', 'en']],
    ['ja', ['de', 'fr', 'en']],
    ['', ['en', 'fr', 'de']],
  ] as const)(
    'the fallback matches the real backend order for %j',
    async (uiLanguage, expected) => {
      order.mockRejectedValue(new Error('command not available'));
      const { result } = renderHook(() => useInterfaceLanguageOptions(uiLanguage));
      await waitFor(() => expect(order).toHaveBeenCalled());
      expect(result.current).toEqual(expected);
    }
  );

  it('ignores a backend answer that lost an entry', async () => {
    order.mockImplementation(async (tags) => tags.slice(1));
    const { result } = renderHook(() => useInterfaceLanguageOptions('en'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    expect(result.current).toHaveLength(AVAILABLE_LOCALES.length);
  });

  it('asks with the alphabetical-by-French-name order as the third argument when the interface is French (independent review, round 4 of #1244)', async () => {
    // The three offered locales' own names IN FRENCH are "allemand"
    // (German), "anglais" (English) and "français" (French) -- so the
    // alphabetical order this hook must work out itself (the platform's
    // own collator is the only thing that can do this correctly) is
    // de, en, fr. This is the one argument `useInterfaceLanguageOptions`
    // computes rather than merely forwarding, so it is the one worth
    // pinning on its own, independently of what the backend sends back.
    order.mockImplementation(async (tags) => [...tags]);
    renderHook(() => useInterfaceLanguageOptions('fr'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    const [, , alphabeticalPrimaryOrder] = order.mock.calls[0];
    expect(alphabeticalPrimaryOrder).toEqual(['de', 'en', 'fr']);
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
