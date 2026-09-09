// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Test helper for switching the translation system to a non-English
 * language.
 *
 * In the real app, a non-English translation file is fetched over HTTP
 * (`fetch('/locales/de/translation.json')` — see `src/lib/i18n.ts`).
 * There is no HTTP server in a test run, so that fetch would just fail.
 * Instead, this helper imports the German and French JSON files directly
 * as JavaScript modules (the same files the real fetch would have
 * returned) and hands them to i18next by hand.
 *
 * Use this only in tests that specifically want to check what German or
 * French text looks like on screen. Every other test gets English for
 * free from the global setup in `src/test/setup.ts` and doesn't need
 * this file at all.
 *
 * IMPORTANT: a test that calls `useTestLanguage('de')` changes the
 * language for the whole test run, not just itself — i18next is a single
 * shared instance. Switch back to `'en'` afterwards (e.g. in an
 * `afterEach`), or every test that runs after yours in the same file will
 * unexpectedly see German text.
 */

import i18n, { AVAILABLE_LOCALES, type LocaleCode } from '@/lib/i18n';
import deTranslations from '../../public/locales/de/translation.json';
import frTranslations from '../../public/locales/fr/translation.json';

/**
 * The actual locale JSON, keyed by code, for every non-English locale we
 * can load without a network fetch. English isn't here because it's
 * already loaded as part of `initI18n()` — see `src/lib/i18n.ts`.
 */
const BUNDLED_NON_ENGLISH_LOCALES: Partial<Record<LocaleCode, object>> = {
  de: deTranslations,
  fr: frTranslations,
};

/**
 * Switch the shared i18next instance to `code` for the duration of a test,
 * loading that locale's translation file directly (no network involved).
 *
 * Assumes `initI18n()` has already run — true for every test file, since
 * `src/test/setup.ts` does that globally.
 */
export async function useTestLanguage(code: LocaleCode): Promise<void> {
  if (code === 'en') {
    // English is already loaded by initI18n(); just switch to it.
    await i18n.changeLanguage('en');
    return;
  }

  const translations = BUNDLED_NON_ENGLISH_LOCALES[code];
  if (!translations) {
    // Every AVAILABLE_LOCALES entry other than 'en' is expected to have a
    // bundled copy above. If this fires, a new locale was added to
    // LOCALES in src/lib/i18n.ts without a matching import here.
    throw new Error(
      `useTestLanguage: no bundled translation file for "${code}". ` +
        `Known locales: ${AVAILABLE_LOCALES.join(', ')}.`
    );
  }

  i18n.addResourceBundle(code, 'translation', translations, true, true);
  await i18n.changeLanguage(code);
}
