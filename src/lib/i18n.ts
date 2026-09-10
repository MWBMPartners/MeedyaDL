// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file i18n initialization and locale loading.
 *
 * Sets up i18next with browser language detection and dynamic locale loading
 * from `public/locales/{lang}/translation.json`. English is the fallback
 * language. The detected (or user-selected) language is cached in localStorage
 * under the key `meedyadl-ui-language`.
 *
 * Usage:
 *   1. Import this module in App.tsx (side-effect import)
 *   2. Call `initI18n()` during app initialization
 *   3. In components: `const { t } = useTranslation()` then `t('key')`
 *
 * Adding a new language:
 *   1. Create `public/locales/{code}/translation.json`
 *   2. Add one entry to `LOCALES` below (this is the only list to edit —
 *      the Settings dropdown, the tests, and the audit checks all read it)
 */

import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

// English translations are bundled inline so they're available synchronously
// from the very first render. Without this, components using useTranslation()
// would briefly display raw keys (e.g., "sidebar.ready") until the async
// fetch of /locales/en/translation.json completes.
import enTranslations from '../../public/locales/en/translation.json';

/**
 * The one place a supported language is described.
 *
 * Adding a language to MeedyaDL means two things: one entry here, and one
 * file at `public/locales/<code>/translation.json`. Nothing else needs
 * editing — the Settings language dropdown, the locale-parity tests, and
 * the audit checks all read this list rather than keeping their own copy.
 *
 * `machineAssisted` means the translation was produced by a machine and has
 * not been read through by a person who speaks the language. We say so
 * openly in the UI rather than letting a translation nobody has checked
 * pass as equivalent to one a person has reviewed — a wrong or misleading
 * translation is worse than an honest "this may not be quite right" label.
 *
 * `machineAssistedLabel` is the little disclaimer text itself, written in
 * *that* language (e.g. the German entry's label is in German). It sits
 * right next to the language's own native name, which is also written in
 * that language, so the two read as one consistent line instead of an
 * English sentence bolted onto a foreign word.
 */
export const LOCALES = [
  { code: 'en', nativeName: 'English', machineAssisted: false, machineAssistedLabel: '' },
  {
    code: 'de',
    nativeName: 'Deutsch',
    machineAssisted: true,
    machineAssistedLabel: 'automatische Übersetzung',
  },
  {
    code: 'fr',
    nativeName: 'Français',
    machineAssisted: true,
    machineAssistedLabel: 'traduction automatique',
  },
] as const;

/** A supported locale code, derived from `LOCALES` so it can never drift from it. */
export type LocaleCode = (typeof LOCALES)[number]['code'];

/**
 * Just the codes, in `LOCALES` order. Derived, never edited by hand — kept
 * as its own export because other code and tests already refer to it by
 * this name, and a flat list of codes is often all a caller needs.
 */
export const AVAILABLE_LOCALES: readonly LocaleCode[] = LOCALES.map((l) => l.code);

/**
 * Turn whatever the browser/OS reports (e.g. "de-DE", "en-US", "fr") into
 * the base language code we key our locale files by (e.g. "de", "en",
 * "fr"). Falls back to "en" when nothing usable was reported.
 *
 * This is the one rule for that conversion. `initI18n` uses it below, and
 * the help screen will use it too, so the two can never disagree about
 * which locale folder a given person's language setting actually means.
 */
export function baseLanguageOf(language: string | undefined): string {
  return language?.split('-')[0] || 'en';
}

/**
 * True when the given language resolves to a locale marked
 * `machineAssisted` in `LOCALES` above. Used to decide whether the UI
 * should show the "this translation was machine-made" notice.
 */
export function isMachineAssisted(language: string | undefined): boolean {
  const base = baseLanguageOf(language);
  return LOCALES.some((l) => l.code === base && l.machineAssisted);
}

/**
 * Keeps the page's declared language (the `lang` attribute on `<html>`)
 * in sync with whatever language i18next is actually showing text in.
 *
 * A screen reader uses that attribute to choose pronunciation rules --
 * `index.html` hard-codes `lang="en"` and nothing ever updated it, so a
 * screen reader user who had set German or French would still hear
 * German/French *words* read with English pronunciation rules the
 * whole time. This is WCAG 3.1.1 ("Language of Page"), a Level A
 * requirement.
 *
 * Only the base code is written (e.g. "de", not "de-DE") -- that
 * matches how `LOCALES` and the `public/locales/<code>/` folders are
 * keyed, via the same `baseLanguageOf()` helper everything else here
 * uses, so this can never disagree with the rest of the module about
 * what a given language setting means.
 */
function syncDocumentLanguage(language: string): void {
  document.documentElement.lang = baseLanguageOf(language);
}

/**
 * Load a locale's translation JSON from the public directory and add it
 * to i18next's resource bundle. Fails silently — missing locales fall
 * through to the English fallback.
 */
async function loadLocaleResources(lng: string): Promise<void> {
  try {
    const response = await fetch(`/locales/${lng}/translation.json`);
    if (response.ok) {
      const translations = await response.json();
      i18n.addResourceBundle(lng, 'translation', translations, true, true);
    }
  } catch {
    // Silently fail — fallback language will be used
  }
}

/**
 * Initialize i18next with language detection and React integration.
 * Pre-loads English (fallback) and the detected/selected language.
 *
 * Call this once during app startup, before rendering.
 */
export async function initI18n(): Promise<void> {
  // Registered before `.init()` so it also catches whatever language
  // detection resolves to on this very first call, not just later
  // changes -- `i18next` fires `languageChanged` as part of `init()`
  // itself once a language has been resolved.
  i18n.on('languageChanged', syncDocumentLanguage);

  await i18n
    .use(LanguageDetector)
    .use(initReactI18next)
    .init({
      fallbackLng: 'en',
      debug: false,
      interpolation: {
        escapeValue: false, // React already escapes
      },
      detection: {
        order: ['localStorage', 'navigator'],
        lookupLocalStorage: 'meedyadl-ui-language',
        caches: ['localStorage'],
      },
      resources: {
        en: { translation: enTranslations },
      },
    });

  // Belt-and-suspenders: set it directly too, in case some i18next
  // version/config path resolves the initial language without firing
  // the event synchronously during `init()`.
  syncDocumentLanguage(i18n.language);

  // If the detected language is not English, fetch and add its file too.
  const detected = baseLanguageOf(i18n.language);
  if (detected !== 'en') {
    await loadLocaleResources(detected);

    // Adding a resource bundle above does NOT, by itself, tell React that
    // anything changed. react-i18next only re-renders components when
    // i18next fires its `languageChanged` event, and `addResourceBundle`
    // does not fire that event — it just quietly puts the data in memory.
    //
    // Concretely, this used to mean: a German or French user who had
    // never touched the language dropdown (the default state, since
    // `ui_language` starts empty and just means "whatever the OS
    // reports") would have the German file fetched and stored... and
    // then every already-rendered component would go on showing English
    // forever, because nothing ever told them to look again.
    //
    // Re-issuing the same language through `changeLanguage` forces that
    // event to fire, which is what actually makes the screen update. It
    // looks like a no-op (we're "changing" to the language we're already
    // on) but the event is the whole point.
    await i18n.changeLanguage(i18n.language);
  }
}

export default i18n;
