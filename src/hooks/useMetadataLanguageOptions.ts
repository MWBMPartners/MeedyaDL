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
 * Why a value, once seen, STAYS in the list for the rest of the app's run:
 * policy UI-050 says a menu must not change when something in it is
 * chosen. Adding only "the current value" would make the old entry vanish
 * the moment somebody picked a different one -- and they could not pick it
 * again to undo their choice.
 *
 * The values seen are kept in the settings store
 * (`seenMetadataLanguages`), not in this hook's own state. They used to be
 * component state, and the independent review found the gap: switching to
 * another Settings tab unmounts this list, which threw that state away, so
 * after picking another value and coming back the old `zh-CN` was gone.
 */

import { useEffect, useMemo, useState } from 'react';

import { orderLanguagesForDisplay } from '@/lib/tauri-commands';
import { useSettingsStore } from '@/stores/settingsStore';
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
  uiLanguage: string
): LanguageOption[] {
  // Values not in the offered list that the list has already shown during
  // this run (kept in the store so they survive a tab switch -- see the
  // file comment).
  const remembered = useSettingsStore((s) => s.seenMetadataLanguages);
  const noteSeen = useSettingsStore((s) => s.noteMetadataLanguageSeen);
  const isUnoffered = currentValue !== '' && !METADATA_LANGUAGE_TAGS.includes(currentValue);

  // Record the current value once it has been shown. After rendering, not
  // during it: changing a store while React is rendering a component that
  // reads it is not allowed. The current value is added to this render's
  // list directly (just below), so it is there on the very first paint.
  useEffect(() => {
    if (isUnoffered) noteSeen(currentValue);
  }, [isUnoffered, currentValue, noteSeen]);

  // The extra entries, as one string, so the list below is rebuilt -- and
  // asked to be re-ordered -- only when its CONTENT changes, never merely
  // because a different entry was chosen. (A new array identity on each
  // choice made the list fall back to A-Z order for a moment while the
  // new order was fetched: the menu moved when something was chosen.)
  // The separator is a character no language tag or settings value uses.
  const extrasKey = withSavedValues(remembered, isUnoffered ? [currentValue] : []).join('\u0000');
  const tags = useMemo(
    () =>
      withSavedValues(METADATA_LANGUAGE_TAGS, extrasKey === '' ? [] : extrasKey.split('\u0000')),
    [extrasKey]
  );

  const fallback = useMemo(() => collatorFallbackOrder(tags, uiLanguage), [tags, uiLanguage]);

  // What the backend ordering said, and for which list and language.
  // Kept with its inputs so an answer for an older list or an older
  // interface language is never shown for the current one.
  const [ordered, setOrdered] = useState<{ tags: string[]; ui: string; order: string[] } | null>(
    null
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
          alphabeticalPrimaryOrder(tags, uiLanguage)
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
