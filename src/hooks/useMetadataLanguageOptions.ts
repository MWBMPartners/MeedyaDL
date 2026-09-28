/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file useMetadataLanguageOptions -- the entries of the Metadata Language
 * list in Settings > General, named and ordered by the shared language
 * policy (#1249; docs/standards/media-language-bcp47-policy.md, UI-010 to
 * UI-050).
 *
 *   const options = useMetadataLanguageOptions(language.value, i18n.language);
 *   <Select options={options} value={language.value} ... />
 *
 * What it does:
 *   - Names every entry in the interface language, from the WebView's own
 *     locale data (see `languageOptions.ts`).
 *   - Asks the backend (`order_languages_for_display`, which runs the shared
 *     `meedya-lang` ordering) for the order: the person's languages first,
 *     then alphabetical by name. Until it answers -- and if it ever fails --
 *     the list is shown alphabetically by name instead, never empty.
 *   - Keeps a saved value that is not one of the offered tags (an old
 *     `zh-CN`, or anything a settings file brought in) in the list, named
 *     from its own tag, so it shows as the selected entry. It is never
 *     changed unless the person picks something else.
 *
 * Why a value, once seen, STAYS in the list for as long as the screen is
 * open (the `seenValues` state below): policy UI-050 says a menu must not
 * change when something in it is chosen. Adding only "the current value"
 * would make the old entry vanish the moment somebody picked a different
 * one -- and they could not pick it again to undo their choice.
 */

import { useEffect, useMemo, useState } from 'react';

import { orderLanguagesForDisplay } from '@/lib/tauri-commands';
import {
  METADATA_LANGUAGE_TAGS,
  alphabeticalPrimaryOrder,
  collatorFallbackOrder,
  isSameList,
  languagePreferences,
  toLanguageOptions,
  withSavedValues,
  type LanguageOption,
} from '@/lib/languageOptions';

/** The system's preferred languages, or none where there is no browser. */
function systemLanguages(): readonly string[] {
  return typeof navigator !== 'undefined' && Array.isArray(navigator.languages)
    ? navigator.languages
    : [];
}

/**
 * The Metadata Language list entries for the current value and interface
 * language. See the file comment for what it does and why.
 *
 * @param currentValue - The setting's value right now (the stored tag).
 * @param uiLanguage   - The language the interface is showing text in.
 */
export function useMetadataLanguageOptions(
  currentValue: string,
  uiLanguage: string,
): LanguageOption[] {
  // Every value this screen has shown as the setting, that is not one of
  // the offered tags. Grown during render with a guard -- React's
  // documented way to keep information from earlier renders -- rather
  // than in an effect, so the entry is there on the very first paint.
  const [seenValues, setSeenValues] = useState<string[]>([]);
  if (
    currentValue !== '' &&
    !METADATA_LANGUAGE_TAGS.includes(currentValue) &&
    !seenValues.includes(currentValue)
  ) {
    setSeenValues([...seenValues, currentValue]);
  }

  // `seenValues` is a new array only when a value is added, so the list is
  // rebuilt (and re-ordered) only when its entries actually change.
  const tags = useMemo(() => withSavedValues(METADATA_LANGUAGE_TAGS, seenValues), [seenValues]);

  const fallback = useMemo(() => collatorFallbackOrder(tags, uiLanguage), [tags, uiLanguage]);

  // What the backend ordering said, and for which list and language.
  // Kept with its inputs so an answer for an older list or an older
  // interface language is never shown for the current one.
  const [ordered, setOrdered] = useState<{ tags: string[]; ui: string; order: string[] } | null>(
    null,
  );

  useEffect(() => {
    let cancelled = false;
    // Wrapped so that a failure BEFORE a promise exists (the IPC bridge not
    // there at all, say) takes the same path as a rejected call: the
    // alphabetical fallback stays on screen and nothing breaks.
    const request = (): Promise<string[]> => {
      try {
        return orderLanguagesForDisplay(
          tags,
          languagePreferences(uiLanguage, systemLanguages()),
          alphabeticalPrimaryOrder(tags, uiLanguage),
        );
      } catch (error) {
        return Promise.reject(error);
      }
    };
    request()
      .then((order) => {
        // Used only if it is exactly the same entries, reordered: an
        // answer with one lost or added is never shown.
        if (!cancelled && isSameList(order, tags)) {
          setOrdered({ tags, ui: uiLanguage, order });
        }
      })
      .catch((error: unknown) => {
        // The alphabetical fallback is already on screen; say why in the
        // console for anybody diagnosing it, but do not bother the person.
        console.warn('[settings] metadata language order unavailable, showing A-Z', error);
      });
    return () => {
      cancelled = true;
    };
  }, [tags, uiLanguage]);

  return useMemo(() => {
    const useBackendOrder = ordered !== null && ordered.tags === tags && ordered.ui === uiLanguage;
    return toLanguageOptions(useBackendOrder ? ordered.order : fallback, uiLanguage);
  }, [ordered, tags, uiLanguage, fallback]);
}
