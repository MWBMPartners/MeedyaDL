// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for the Interface Language dropdown in GeneralTab
 * (independent review, round 4 of #1244).
 *
 * The order and labelling of this dropdown are implemented across three
 * layers -- `GeneralTab.tsx` (builds the option list), the
 * `useInterfaceLanguageOptions` hook (asks the backend for the order),
 * and the shared `orderLanguagesForDisplay` command -- and until this
 * round of review, nothing exercised all three together. This file does
 * not re-test the hook's own contract (see
 * `src/hooks/useInterfaceLanguageOptions.test.ts` for that); it proves
 * GeneralTab actually WIRES UP what the hook and the translation system
 * hand it, by planting the three faults an earlier version of this code
 * had and showing each one would now be caught:
 *
 *   1. The dropdown used to always read `LOCALES` in its own fixed
 *      array order ("English, Deutsch, Français"), whatever language
 *      the interface was showing -- ignoring the backend's answer
 *      entirely. Caught below by returning a RECOGNISABLE order from
 *      the mocked backend and checking the rendered `<option>`s follow
 *      it exactly, not `LOCALES`' own order.
 *   2. The "Auto (System)" row used to be a hard-coded English string.
 *      Caught below by mocking the translation function to answer a
 *      distinct German string for that one key and checking the
 *      rendered row uses it, not the English text.
 *   3. The order was asked for with a hard-coded English preference,
 *      whatever language the interface was actually showing. Caught
 *      below by recording the arguments the mocked backend command was
 *      called with and checking the preference sent is `['de']`, not
 *      `['en']`, when the interface language is German.
 */

import { render, screen, waitFor, within } from '@testing-library/react';

import * as commands from '@/lib/tauri-commands';
import { GeneralTab } from '@/components/settings/tabs/GeneralTab';
import { AVAILABLE_LOCALES } from '@/lib/i18n';

// The one German string this suite cares about -- everything else GeneralTab
// asks `t()` for during an ordinary render is returned as its own key,
// which is enough: no other translated text is asserted on here.
const LANGUAGE_AUTO_DE = 'Automatisch (System)';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => (key === 'settings.general.languageAuto' ? LANGUAGE_AUTO_DE : key),
    // The language GeneralTab believes the interface is showing text in
    // right now -- set to German for this whole file, independently of
    // whatever the `ui_language` SETTING happens to be (they are
    // different things; see the comment on `i18n.language` at the top
    // of GeneralTab.tsx).
    i18n: { language: 'de' },
  }),
}));

// Only `orderLanguagesForDisplay` is replaced; everything else GeneralTab
// imports from this module (export/import settings, profile bundles) keeps
// its real implementation, since nothing in this test clicks those buttons.
vi.mock('@/lib/tauri-commands', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/tauri-commands')>();
  return {
    ...actual,
    orderLanguagesForDisplay: vi.fn(),
  };
});

const order = vi.mocked(commands.orderLanguagesForDisplay);

describe('GeneralTab -- Interface Language dropdown (independent review, round 4)', () => {
  beforeEach(() => {
    order.mockReset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('orders the Interface Language rows by the backend answer and shows the translated Auto label', async () => {
    // A recognisable order -- the reverse of AVAILABLE_LOCALES' own array
    // order -- returned ONLY for the interface-language call (identified
    // by its `tags` argument being exactly AVAILABLE_LOCALES; the
    // Metadata Language dropdown asks the same command with a much
    // longer tag list, and its answer is not what this test checks).
    order.mockImplementation(async (tags) => {
      if (tags.length === AVAILABLE_LOCALES.length) {
        return [...tags].reverse();
      }
      return [...tags];
    });

    render(<GeneralTab />);

    const select = screen.getByLabelText('Language') as HTMLSelectElement;

    // Fault 1: the rendered order follows the backend's (recognisable,
    // reversed) answer -- ['fr', 'de', 'en'] after 'auto' -- not
    // `LOCALES`' own fixed array order (['en', 'de', 'fr']).
    await waitFor(() => {
      const values = within(select)
        .getAllByRole('option')
        .map((option) => (option as HTMLOptionElement).value);
      expect(values).toEqual(['auto', 'fr', 'de', 'en']);
    });

    // Fault 2: the "Auto (System)" row's label is the mocked German
    // translation, not the hard-coded English text.
    const autoOption = within(select).getByRole('option', { name: LANGUAGE_AUTO_DE });
    expect((autoOption as HTMLOptionElement).value).toBe('auto');
    // Scoped to THIS select -- the Theme dropdown elsewhere on the page
    // legitimately has its own, unrelated "Auto (System)" option, and a
    // whole-document query would wrongly flag that as a failure too.
    expect(within(select).queryByRole('option', { name: 'Auto (System)' })).not.toBeInTheDocument();

    // Fault 3: the interface-language call's preferences argument is
    // `['de']` -- the language actually showing -- never a hard-coded
    // `['en']`.
    const interfaceCall = order.mock.calls.find(
      ([tags]) => tags.length === AVAILABLE_LOCALES.length
    );
    expect(interfaceCall).toBeDefined();
    const [tags, preferences] = interfaceCall!;
    expect(tags).toEqual(AVAILABLE_LOCALES);
    expect(preferences).toEqual(['de']);
  });
});
