// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file useInterfaceLanguageOptions -- the order of the Interface Language
 * list in Settings > General > Language (independent review, round 3 of
 * #1244).
 *
 * Until this hook existed, the dropdown built its rows straight from
 * `LOCALES` (src/lib/i18n.ts) in whatever order they happen to be written
 * there -- "English, Deutsch, Français", always, no matter which language
 * the interface was actually showing. Policy UI-040 asks for the interface
 * language itself first, then everything else alphabetical by name IN the
 * interface language -- the same rule the Metadata Language list already
 * follows (see `useMetadataLanguageOptions.ts`). This hook asks the SAME
 * backend command (`order_languages_for_display`, which runs the shared
 * `meedya-lang` crate the policy's own test cases check) so the rule is
 * implemented once, not copied into a second place nothing checks.
 *
 * "Auto (System)" is not a language, so it plays no part in this ordering
 * -- the caller (GeneralTab.tsx) keeps it as its own leading entry, exactly
 * as it always has.
 *
 * Simpler than `useMetadataLanguageOptions`: the list of locale codes is
 * fixed (`AVAILABLE_LOCALES`) and never carries a value from outside it
 * that has to be remembered across a Settings tab switch, so there is no
 * "remembered extra value" bookkeeping to do here.
 */

import { useEffect, useMemo, useState } from 'react';

import { orderLanguagesForDisplay } from '@/lib/tauri-commands';
import { AVAILABLE_LOCALES } from '@/lib/i18n';
import { alphabeticalPrimaryOrder, collatorFallbackOrder, isSameList } from '@/lib/languageOptions';

/**
 * The codes from `AVAILABLE_LOCALES`, ordered for display in `uiLanguage`:
 * `uiLanguage` itself first (when it is one of the offered locales), then
 * the rest alphabetical by name in `uiLanguage` (policy UI-020 to UI-040).
 *
 * Shows the plain alphabetical order (`collatorFallbackOrder`) until the
 * backend answers, and if it ever fails or sends back a list with an entry
 * lost or added -- the same never-empty, never-broken guarantee
 * `useMetadataLanguageOptions` makes.
 *
 * @param uiLanguage -- the language the interface is showing text in right
 *   now (`i18n.language`), used both to name the entries (by the caller)
 *   and as the one preference sent to the backend.
 */
export function useInterfaceLanguageOptions(uiLanguage: string): readonly string[] {
  const tags = AVAILABLE_LOCALES;
  // `uiLanguage`'s own row pinned first (when it is one of `tags`), then
  // the rest in `collatorFallbackOrder`'s alphabetical order -- matching
  // where the real, backend-driven order (policy UI-020) would put it.
  // Without this, the fallback shown before the backend answers (or if it
  // never does) put every row in plain alphabetical order with no pin at
  // all, so the interface language's own row could sit anywhere in the
  // list -- and then visibly JUMP to the top the moment the real answer
  // arrived, for any interface language whose own name does not happen
  // to sort first (independent review, round 4 of #1244: French is
  // exactly such a case -- "allemand" (German) sorts before "français"
  // (French) alphabetically, so the un-pinned fallback showed German
  // first for a French interface, then jumped to French-first once the
  // backend replied).
  const fallback = useMemo(() => {
    const alphabetical = collatorFallbackOrder(tags, uiLanguage);
    if (!alphabetical.includes(uiLanguage)) return alphabetical;
    return [uiLanguage, ...alphabetical.filter((tag) => tag !== uiLanguage)];
  }, [tags, uiLanguage]);

  // What the backend said, and for which interface language -- kept with
  // its input so an answer worked out for a previous language is never
  // shown for the current one.
  const [ordered, setOrdered] = useState<{ ui: string; order: string[] } | null>(null);

  useEffect(() => {
    let cancelled = false;
    // Wrapped exactly as `useMetadataLanguageOptions` wraps its own
    // request: a failure before a promise even exists (the IPC bridge not
    // being there at all) must take the same path as a rejected call.
    const request = (): Promise<string[]> => {
      try {
        return orderLanguagesForDisplay(
          tags,
          [uiLanguage],
          alphabeticalPrimaryOrder(tags, uiLanguage)
        );
      } catch (error) {
        return Promise.reject(error);
      }
    };
    request()
      .then((order) => {
        // Used only if it is exactly the same entries, reordered -- an
        // answer with one lost or added is never shown.
        if (!cancelled && isSameList(order, tags as readonly string[])) {
          setOrdered({ ui: uiLanguage, order });
        }
      })
      .catch((error: unknown) => {
        console.warn('[settings] interface language order unavailable, showing A-Z', error);
      });
    return () => {
      cancelled = true;
    };
  }, [tags, uiLanguage]);

  return ordered !== null && ordered.ui === uiLanguage ? ordered.order : fallback;
}
