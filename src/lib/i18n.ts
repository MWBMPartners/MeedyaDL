// Copyright (c) 2026 MeedyaDL

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
 *   2. Add the code to `AVAILABLE_LOCALES` below
 *   3. Add the entry to `LANGUAGE_UI_OPTIONS` in GeneralTab.tsx
 */

import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

/**
 * Available locale codes. When adding a new translation, add the code here
 * and create the corresponding `public/locales/{code}/translation.json` file.
 */
export const AVAILABLE_LOCALES = ['en', 'de', 'fr'] as const;

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
      resources: {}, // loaded dynamically below
    });

  // Pre-load English (always available as fallback)
  await loadLocaleResources('en');

  // If detected language is not English, load it too
  const detected = i18n.language?.split('-')[0];
  if (detected && detected !== 'en') {
    await loadLocaleResources(detected);
  }
}

export default i18n;
