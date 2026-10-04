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
import { useLanguageIdentities } from '@/hooks/useLanguageIdentities';

/**
 * The codes from `AVAILABLE_LOCALES`, ordered for display in `uiLanguage`:
 * `uiLanguage`'s whole primary-language group first (when an offered locale
 * shares it), then the rest alphabetical by name in `uiLanguage` (policy
 * UI-020 to UI-040).
 *
 * Shows that pinned-then-alphabetical fallback until the backend answers,
 * and if it ever fails or sends back a list with an entry lost or added --
 * the same never-empty, never-broken guarantee `useMetadataLanguageOptions`
 * makes. Which group a tag belongs to comes from `useLanguageIdentities`
 * (the backend's reading, Codex's catch-up review of #1244), with the
 * browser's reading while that is pending or if it failed -- only ever to
 * decide the ORDER here, never which entries are shown.
 *
 * @param uiLanguage -- the language the interface is showing text in right
 *   now (`i18n.language`), used both to name the entries (by the caller)
 *   and as the one preference sent to the backend.
 */
export function useInterfaceLanguageOptions(uiLanguage: string): readonly string[] {
  const tags = AVAILABLE_LOCALES;
  // "Auto (System)" hands this hook the OS's FULL tag (`fr-FR`, `fr-CA`,
  // `de-DE`, `en-GB`), never the bare `fr`/`de`/`en` offered here, so
  // `uiLanguage` has to be identified too, not only the offered tags.
  const identityKey = JSON.stringify([...tags, uiLanguage]);
  const identityTags = useMemo(() => JSON.parse(identityKey) as string[], [identityKey]);
  const identities = useLanguageIdentities(identityTags);

  // The interface language's whole primary-language GROUP first, then the
  // rest alphabetical -- where the real backend order puts them, so the
  // list does not visibly JUMP when the real answer arrives. Round 4 of
  // #1244 pinned only an EXACT match, which passed every test (all bare
  // codes) yet still jumped for `fr-FR`, the shape "Auto" really sends;
  // grouping by identity fixes that, and by the BACKEND's identity keeps
  // this and the order request below from disagreeing (round 5).
  const fallback = useMemo(() => {
    const alphabetical = collatorFallbackOrder(tags, uiLanguage);
    const ui = identities.primaryOf(uiLanguage);
    const pinned = alphabetical.filter((tag) => identities.primaryOf(tag) === ui);
    if (pinned.length === 0) return alphabetical;
    return [...pinned, ...alphabetical.filter((tag) => identities.primaryOf(tag) !== ui)];
  }, [tags, uiLanguage, identities]);

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
          alphabeticalPrimaryOrder(tags, uiLanguage, identities.primaryOf)
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
        console.warn(
          '[settings] interface language order unavailable, showing the pinned-then-A-Z fallback',
          error
        );
      });
    return () => {
      cancelled = true;
    };
  }, [
    tags,
    uiLanguage,
    // Asked again once the backend's identities arrive, so the real order
    // uses the real groups ("the backend's answer replaces it").
    identities,
  ]);

  return ordered !== null && ordered.ui === uiLanguage ? ordered.order : fallback;
}
