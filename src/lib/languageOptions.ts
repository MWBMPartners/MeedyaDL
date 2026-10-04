/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file languageOptions.ts -- Names and order for the Metadata Language list
 * in Settings > General (#1249).
 *
 * The rules come from the shared language policy, MWBM-MEDIA-LANG
 * (docs/standards/media-language-bcp47-policy.md -- read it before changing
 * anything here). The parts that apply to this list:
 *
 *   - UI-010: a language is shown by its name in the person's CURRENT
 *     interface language ("German" in English, "Deutsch" in German), taken
 *     from the platform's own locale data -- here the WebView's
 *     `Intl.DisplayNames` -- never from a hand-typed list of English names.
 *     A regional or script form is named with its qualifier:
 *     "English (United Kingdom)", "Chinese (Traditional, Taiwan)".
 *   - NAME-020: the name is only a label. What is stored, and handed to the
 *     download tool, is always the language tag itself.
 *   - UI-020 to UI-040: the order. The person's own languages first, then
 *     everything else alphabetically by name in the interface language.
 *
 * What this file does NOT do: decide the order. That is done by the shared
 * Rust code (`meedya-lang`) through the `order_languages_for_display`
 * command, so the rules exist once, in the code the policy's test cases
 * check -- not a second time in JavaScript that nothing checks. This file
 * only supplies what the Rust code cannot know by itself: the names, and
 * the alphabetical order of the languages in the interface language, which
 * only the platform's own collator can work out. `collatorFallbackOrder`
 * is used only when that command cannot answer, so the list is never empty
 * or broken.
 */

/**
 * The metadata languages offered. Each is a language tag (BCP 47), which is
 * exactly what is stored in `settings.language` and handed to GAMDL.
 *
 * The two Chinese entries used to be `zh-CN` and `zh-TW`, labelled by hand
 * "Chinese (Simplified)" / "Chinese (Traditional)" -- a country standing in
 * for a writing system. Apple's own documentation for the storefront list
 * gives the localisations it supports as script-bearing tags: `zh-Hans-CN`
 * for mainland China and `zh-Hant-TW` for Taiwan (checked on 28 Sept 2026
 * against Apple's published example response for "Get All Storefronts";
 * Apple also says a requested localisation must be one of the storefront's
 * `supportedLanguageTags`). GAMDL passes the value through unchanged. So
 * those two tags are offered, and their names then say Simplified /
 * Traditional correctly from the tag itself.
 *
 * Somebody who already chose the old `zh-CN` / `zh-TW` keeps it: a saved
 * value that is not in this list is added to the list (see
 * `withSavedValues`) and stays selected until they pick something else.
 */
export const METADATA_LANGUAGE_TAGS: readonly string[] = [
  'en-US',
  'en-GB',
  'ja-JP',
  'ko-KR',
  'zh-Hans-CN',
  'zh-Hant-TW',
  'de-DE',
  'fr-FR',
  'es-ES',
  'pt-BR',
  'it-IT',
  'ru-RU',
];

/** One entry of the list, in the shape the shared `Select` component takes. */
export interface LanguageOption {
  value: string;
  label: string;
}

/**
 * The platform's language names in `uiLanguage`, or in the runtime's own
 * default language if `uiLanguage` is not something the platform accepts.
 * `languageDisplay: 'standard'` gives "English (United Kingdom)" rather than
 * "British English", which is the qualifier style UI-010 asks for.
 */
function displayNamesFor(uiLanguage: string): Intl.DisplayNames | null {
  for (const locales of [[uiLanguage], []]) {
    try {
      return new Intl.DisplayNames(locales, { type: 'language', languageDisplay: 'standard' });
    } catch {
      // An interface language the platform cannot use -- try the default.
    }
  }
  return null;
}

/** A collator for `uiLanguage`, or the runtime's default one. */
function collatorFor(uiLanguage: string): Intl.Collator {
  try {
    return new Intl.Collator([uiLanguage]);
  } catch {
    return new Intl.Collator();
  }
}

/**
 * The name of `tag` in `uiLanguage`. A value the platform cannot read as a
 * language tag -- an old saved value such as `en_US`, say -- is shown as its
 * own text rather than hidden or "corrected": what is stored is shown as it
 * is, and nothing is guessed (policy LANG-003, COMPAT-040).
 */
export function languageDisplayName(tag: string, uiLanguage: string): string {
  const names = displayNamesFor(uiLanguage);
  if (!names) return tag;
  try {
    return names.of(tag) ?? tag;
  } catch {
    return tag;
  }
}

/**
 * The primary language part of `tag` (`zh` for `zh-Hant-TW`), read by the
 * platform's own tag parser rather than by splitting text on hyphens.
 * `null` when the platform cannot read `tag` as a tag at all -- such a value
 * is not an ordinary language group, and the Rust ordering puts it last.
 *
 * **A fallback, not the source of truth** (Codex's catch-up review of
 * #1244): `Intl.Locale` folds `cmn` into `zh` and cannot read `zh-cmn-Hans`
 * at all, which the policy does not. Anything that can use a hook should
 * use `useLanguageIdentities` (the backend's reading) instead.
 */
export function primaryLanguageOf(tag: string): string | null {
  try {
    return new Intl.Locale(tag).language;
  } catch {
    return null;
  }
}

/**
 * The distinct primary languages among `tags`, in alphabetical order of
 * their names in `uiLanguage`, compared with that language's own sorting
 * rules (so "Éwé" sorts with the Es in French). Two primary languages with
 * the same name are ordered by their code, as UI-040 says.
 *
 * This is the one thing the Rust ordering asks the frontend for: the
 * command compares ordinary language groups by their position in this list.
 *
 * @param primaryOf - how to read a tag's group. Defaults to the browser's
 *   reading; real callers pass `useLanguageIdentities(...).primaryOf` so the
 *   groups are the ones the Rust ordering uses (Codex's catch-up review).
 */
export function alphabeticalPrimaryOrder(
  tags: readonly string[],
  uiLanguage: string,
  primaryOf: (tag: string) => string | null = primaryLanguageOf
): string[] {
  const primaries = [...new Set(tags.map(primaryOf).filter((p): p is string => p !== null))];
  const collator = collatorFor(uiLanguage);
  return primaries.sort(
    (a, b) =>
      collator.compare(languageDisplayName(a, uiLanguage), languageDisplayName(b, uiLanguage)) ||
      (a < b ? -1 : a > b ? 1 : 0)
  );
}

/**
 * The order used only when the Rust ordering command cannot answer:
 * alphabetical by displayed name in `uiLanguage`, ties by the tag. Always
 * gives a complete list, so a failure never leaves the menu empty.
 */
export function collatorFallbackOrder(tags: readonly string[], uiLanguage: string): string[] {
  const collator = collatorFor(uiLanguage);
  return [...tags].sort(
    (a, b) =>
      collator.compare(languageDisplayName(a, uiLanguage), languageDisplayName(b, uiLanguage)) ||
      (a < b ? -1 : a > b ? 1 : 0)
  );
}

/**
 * The person's language preferences for UI-020, highest priority first:
 * the interface language, then the system's preferred languages. Empty
 * values and repeats (ignoring letter case) are dropped; nothing else is
 * changed -- the Rust side puts each into its standard form and ignores
 * anything that is not a language tag.
 */
export function languagePreferences(
  uiLanguage: string,
  systemLanguages: readonly string[]
): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const value of [uiLanguage, ...systemLanguages]) {
    const key = value.trim().toLowerCase();
    if (key === '' || seen.has(key)) continue;
    seen.add(key);
    out.push(value.trim());
  }
  return out;
}

/**
 * The offered tags plus any `savedValues` that are not among them, in that
 * order, without repeats. Used so a value somebody already has -- an old
 * `zh-CN`, or anything a settings file brought in -- still appears in the
 * list and stays selected, instead of the list silently showing the first
 * entry while keeping a value nobody can see.
 *
 * "Not among them" is decided by STANDARD FORM when `standardOf` is given
 * (Codex's catch-up review of #1244, finding 2): a stored `"EN-us"` is the
 * same language as an offered `"en-US"`, and adding it made two rows both
 * labelled "English (United States)". The value itself is never rewritten
 * (COMPAT-030) -- only whether it needs its own row is decided this way.
 * The default compares raw text, as before. Callers pass
 * `useLanguageIdentities(...).standardOf`, which is the raw text itself
 * until the backend has answered, so nothing is merged on the browser's
 * reading alone (Codex's review of round 5, finding 2).
 */
export function withSavedValues(
  offered: readonly string[],
  savedValues: readonly string[],
  standardOf: (tag: string) => string = (tag) => tag
): string[] {
  const out = [...offered];
  const standards = out.map(standardOf);
  for (const value of savedValues) {
    if (value === '' || out.includes(value)) continue;
    const standard = standardOf(value);
    if (standards.includes(standard)) continue;
    out.push(value);
    standards.push(standard);
  }
  return out;
}

/**
 * True when `ordered` holds exactly the same entries as `tags` (same values,
 * same count) -- the check made on what the ordering command sends back
 * before it is shown, so a list with an entry lost or added is never used.
 */
export function isSameList(ordered: readonly string[], tags: readonly string[]): boolean {
  if (ordered.length !== tags.length) return false;
  const a = [...ordered].sort();
  const b = [...tags].sort();
  return a.every((value, i) => value === b[i]);
}

/** The list entries, in the given order, named in `uiLanguage`. */
export function toLanguageOptions(
  ordered: readonly string[],
  uiLanguage: string
): LanguageOption[] {
  return ordered.map((tag) => ({ value: tag, label: languageDisplayName(tag, uiLanguage) }));
}

/**
 * The label for one row of the Interface Language list (Settings >
 * General > Language), per policy UI-011: a language's own name (its
 * autonym, "Deutsch") must not be the ONLY label in a general menu, but
 * may be shown as well. So each row reads "name in the interface language
 * — the language's own name": "German — Deutsch" in an English interface,
 * "Allemand — Deutsch" in a French one.
 *
 * - The first name comes from the platform (`Intl.DisplayNames`), in the
 *   interface language, never a hand-typed English word (UI-010).
 * - The second is the language's own name as MeedyaDL already lists it
 *   (`LOCALES.nativeName` in `src/lib/i18n.ts`), followed by that
 *   language's own "machine translation" note when it has one, so the
 *   note stays next to the words it is about, in the same language.
 * - When the two names are the same apart from letter case ("English" in
 *   an English interface, "français" / "Français" in a French one) the
 *   name is shown once, as UI-011 allows.
 *
 * The value of each row stays the locale code; only the label changes.
 */
export function interfaceLanguageLabel(
  code: string,
  ownName: string,
  uiLanguage: string,
  ownNote = ''
): string {
  const localised = languageDisplayName(code, uiLanguage);
  const own = ownNote === '' ? ownName : `${ownName} (${ownNote})`;
  const sameName = localised.localeCompare(ownName, undefined, { sensitivity: 'accent' }) === 0;
  return sameName ? own : `${localised} — ${own}`;
}
