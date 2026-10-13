/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file useMetadataLanguageOptions -- the entries of the Metadata Language
 * list in Settings > General, named and ordered by the shared language
 * policy (#1249; docs/standards/media-language-bcp47-policy.md, UI-010 to
 * UI-050).
 *
 *   const list = useMetadataLanguageOptions(language.value, i18n.language);
 *   <Select options={list.options} value={list.selectedValue} ... />
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
 *
 * "Already offered" is decided by STANDARD FORM from the backend
 * (`useLanguageIdentities`; Codex's catch-up review of #1244, finding 2): a
 * stored `"EN-us"` is the offered `"en-US"`, not a second, dead row. The
 * stored value is never rewritten (COMPAT-030).
 *
 * Only the BACKEND's standard form ever merges two entries (Codex's review
 * of round 5, finding 2). While its answer is pending, or if it never
 * comes, every entry keeps a row of its own -- `"EN-us"` beside `"en-US"`,
 * and a stored Mandarin `"cmn-Hans-CN"` beside the offered Chinese
 * `"zh-Hans-CN"`, which the browser alone would have taken for the same
 * language and hidden. Two rows that would carry the same name show their
 * tag after it, so they can be told apart (`toLanguageOptions`; stand-in
 * review of round 6, finding 7).
 *
 * Which row shows as SELECTED is returned too (`selectedValue`; Codex's
 * review of round 5, finding 1). Once a stored `"EN-us"` has become the
 * one `"en-US"` row, the dropdown cannot be handed `"EN-us"` as its value:
 * a native dropdown matches its value to a row by exact text, finds none,
 * and shows its FIRST row instead -- German, in a German interface. So the
 * dropdown is handed the row's own value, found by the backend-verified
 * standard form. That changes only what is shown: the stored setting stays
 * `"EN-us"` until the person picks a different language and saves.
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
import { useLanguageIdentities } from '@/hooks/useLanguageIdentities';

/** The system's preferred languages, or none where there is no browser. */
function systemLanguages(): readonly string[] {
  return typeof navigator !== 'undefined' && Array.isArray(navigator.languages)
    ? navigator.languages
    : [];
}

/** What `useMetadataLanguageOptions` returns. */
export interface MetadataLanguageList {
  /** The list's entries, in display order. */
  options: LanguageOption[];
  /**
   * The value to hand the dropdown so it shows the saved language as
   * selected: the saved value itself when a row has exactly that value,
   * otherwise the value of the row the backend says is the same language
   * (a saved `"EN-us"` shows as the `"en-US"` row). For display only --
   * never written back to the setting.
   */
  selectedValue: string;
}

/**
 * The Metadata Language list entries for the current value and interface
 * language, and which of them shows as selected. See the file comment for
 * what it does and why.
 *
 * @param currentValue - The setting's value right now (the stored tag).
 * @param uiLanguage   - The language the interface is showing text in.
 */
export function useMetadataLanguageOptions(
  currentValue: string,
  uiLanguage: string
): MetadataLanguageList {
  // Values not in the offered list that the list has already shown during
  // this run (kept in the store so they survive a tab switch -- see the
  // file comment).
  const remembered = useSettingsStore((s) => s.seenMetadataLanguages);
  const noteSeen = useSettingsStore((s) => s.noteMetadataLanguageSeen);

  // Identities for every offered tag, the current value and everything
  // remembered (a plain raw-text list is enough to ASK about; it needs to
  // be complete, not deduplicated).
  const identityKey = JSON.stringify(
    withSavedValues(METADATA_LANGUAGE_TAGS, [...remembered, currentValue])
  );
  const identityTags = useMemo(() => JSON.parse(identityKey) as string[], [identityKey]);
  const identities = useLanguageIdentities(identityTags);
  const offeredStandards = useMemo(
    () => new Set(METADATA_LANGUAGE_TAGS.map(identities.standardOf)),
    [identities]
  );
  const isUnoffered =
    currentValue !== '' && !offeredStandards.has(identities.standardOf(currentValue));

  // Record the current value once it has been shown. After rendering, not
  // during it: changing a store while React is rendering a component that
  // reads it is not allowed. The current value is added to this render's
  // list directly (just below), so it is there on the very first paint.
  useEffect(() => {
    if (isUnoffered) noteSeen(currentValue);
  }, [isUnoffered, currentValue, noteSeen]);

  // The extra entries, as a JSON string, so the list below is rebuilt --
  // and asked to be re-ordered -- only when its CONTENT changes, never
  // merely because a different entry was chosen. (A new array identity on
  // each choice made the list fall back to A-Z order for a moment while
  // the new order was fetched: the menu moved when something was chosen.)
  //
  // This used to join the values with a U+0000 (NUL) separator, on the
  // claim that "no language tag or settings value uses" that character.
  // Nothing actually enforced that claim AT THE TIME: an imported
  // settings file's fields generally only have `\n` and `\r` stripped
  // (see the "Import validation" section of `.claude/CLAUDE.md`), which
  // does not touch a NUL byte.
  //
  // Since #1246, though, the metadata language field specifically is an
  // exception to that general rule (same `.claude/CLAUDE.md` section): an
  // IMPORTED value is checked as a language tag, and a NUL byte makes it
  // unreadable as one, so it is refused outright and this machine's
  // current value is kept instead -- it can never reach `currentValue`
  // here by that route. The only way one still could is a value already
  // on disk that nobody imported -- a settings file edited by hand, or
  // one written before this check existed (policy COMPAT-030 keeps such
  // a value exactly as it is, never rewriting or refusing it after the
  // fact). `JSON.stringify` / `JSON.parse` round-trip a string exactly,
  // whatever characters it contains, so there is no separator left to
  // collide with even then (independent review, round 3 of #1244;
  // comment corrected for accuracy, round 4).
  const extrasKey = JSON.stringify(
    withSavedValues(remembered, isUnoffered ? [currentValue] : [], identities.standardOf)
  );
  const tags = useMemo(
    () =>
      withSavedValues(
        METADATA_LANGUAGE_TAGS,
        JSON.parse(extrasKey) as string[],
        identities.standardOf
      ),
    [extrasKey, identities]
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
          alphabeticalPrimaryOrder(tags, uiLanguage, identities.primaryOf)
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
  }, [
    tags,
    uiLanguage,
    // Asked again once the backend's identities arrive.
    identities,
  ]);

  const options = useMemo(() => {
    const useBackendOrder = ordered !== null && ordered.tags === tags && ordered.ui === uiLanguage;
    return toLanguageOptions(useBackendOrder ? ordered.order : fallback, uiLanguage);
  }, [ordered, tags, uiLanguage, fallback]);

  // The row to show as selected (see the file comment). `standardOf` is
  // the backend's answer, or the raw text while that is pending -- and
  // while it is pending the saved value has a row of its own anyway, so
  // the exact match below finds it.
  const selectedValue = useMemo(() => {
    if (options.some((option) => option.value === currentValue)) return currentValue;
    const standard = identities.standardOf(currentValue);
    return (
      options.find((option) => identities.standardOf(option.value) === standard)?.value ??
      currentValue
    );
  }, [options, currentValue, identities]);

  return useMemo(() => ({ options, selectedValue }), [options, selectedValue]);
}
