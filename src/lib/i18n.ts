// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file i18n initialization and locale loading.
 *
 * Sets up i18next with browser language detection and dynamic locale loading
 * from `public/locales/{lang}/translation.json`. English is the fallback
 * language. Only the system's language is detected; nothing is remembered by
 * the browser. A language someone chooses is remembered by the saved
 * `ui_language` setting alone (older builds also kept a copy in localStorage
 * under `meedyadl-ui-language`; that copy is no longer read or written — see
 * the `detection` block in initI18n()).
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
 * Switches the app to `lng`, making sure that language's translation
 * file has actually been loaded first.
 *
 * `i18n.changeLanguage()` on its own only switches WHICH already-loaded
 * language i18next reads from — it does not fetch anything by itself.
 * This app registers no i18next HTTP-fetching plugin (check
 * `package.json`: just `i18next`, `i18next-browser-languagedetector`,
 * `react-i18next`), so calling `i18n.changeLanguage('de')` for a
 * language nobody has fetched yet does not fail or warn — it just
 * quietly keeps showing English. `i18n.language` will report "de", and
 * every `t()` call will still return the English string, because there
 * is no German data for i18next to read, so it silently falls back to
 * `fallbackLng: 'en'`.
 *
 * This used to bite real users: choosing a language in Settings wrote
 * the choice to the `ui_language` setting, and on the next app launch,
 * `App.tsx` called `i18next.changeLanguage(uiLang)` directly on a
 * language nobody had fetched — so the FIRST restart after picking
 * German still showed English. It only started working on the SECOND
 * restart, because by then `changeLanguage` had already written the
 * choice into the browser's remembered-language storage (the language
 * detector caches there), so `initI18n()`'s own startup detection found
 * German on its own next time and fetched the file as part of that
 * separate path. The help text next to the setting used to say "requires
 * restart" (singular) — it actually needed two.
 *
 * @param lng — A language code, e.g. "de" or "de-DE". Only the base part
 *   (before any "-REGION") is used to find the translation file — that
 *   matches how `LOCALES` and the `public/locales/<code>/` folders are
 *   keyed, via `baseLanguageOf()`, same as everywhere else in this file.
 */
/**
 * The system's language if MeedyaDL has a translation for it, otherwise
 * English. What "Auto" means.
 *
 * Asking for a language MeedyaDL does not have (a Spanish system, say)
 * would only ever fail to load a file that does not exist -- so it is not
 * asked for at all. It used to be, and since a failed load now reports
 * itself, a Spanish-system user on "Auto" would have been told on every
 * launch that "es" could not be loaded (Codex, batch-3 review).
 */
export function systemLanguageOrEnglish(): string {
  const system = typeof navigator !== 'undefined' ? navigator.language : 'en';
  return (AVAILABLE_LOCALES as readonly string[]).includes(baseLanguageOf(system)) ? system : 'en';
}

/**
 * Counts language requests, so only the LATEST one is applied.
 *
 * Loading a language file takes a moment. Choosing German and then
 * English before German finished used to apply English at once and then,
 * when the German file arrived, switch the screen back to German -- while
 * the setting said English (Codex, batch-3 review, reproduced).
 */
let latestLanguageRequest = 0;

export async function changeUiLanguage(lng: string): Promise<void> {
  const thisRequest = ++latestLanguageRequest;
  const base = baseLanguageOf(lng);
  /*
   * English is bundled into the app at build time (see the import at
   * the top of this file), so there's nothing to fetch for it.
   * `hasResourceBundle` skips re-fetching a language that's already
   * loaded — this matters once this function is called live every time
   * someone changes the Settings dropdown, including switching back to
   * a language already seen earlier in the same session.
   */
  if (base !== 'en' && !i18n.hasResourceBundle(base, 'translation')) {
    await loadLocaleResources(base);
    // Overtaken by a newer request while this file was loading: stop now,
    // before reporting anything. Checking only after the error below meant
    // a late failure for a language nobody wanted any more still showed a
    // message -- one that could also name the wrong language as the one on
    // screen (Codex, follow-up review).
    if (thisRequest !== latestLanguageRequest) return;
    // loadLocaleResources() hides its own failures (startup has to carry
    // on regardless). Here, a person has just CHOSEN this language, and the
    // screen staying in English with no word of why is the silent failure
    // the help text's "changes straight away" would make worse. So say so;
    // the caller shows it. English stays in use either way.
    if (!i18n.hasResourceBundle(base, 'translation')) {
      throw new Error(`The ${lng} language file could not be loaded, so the language has not changed.`);
    }
  }
  // A newer request arrived while this one's file was loading: it wins.
  if (thisRequest !== latestLanguageRequest) return;
  await i18n.changeLanguage(lng);
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
      // The system language only, and nothing remembered by the browser.
      //
      // This used to read `localStorage` FIRST and write every language
      // change into it (`caches: ['localStorage']`). Since the dropdown
      // switches the language the moment it changes — before Save — a
      // language merely tried out and never saved came back after a
      // restart, while the dropdown said "Auto"; and choosing "Auto"
      // afterwards could never take effect, because the browser's copy
      // still said otherwise (stand-in review, 24 Sept 2026). The saved
      // `ui_language` setting is now the only thing that remembers a
      // choice; App.tsx applies it once settings load. The cost: someone
      // who saved a non-English language may see English for a moment at
      // startup, until then. A stale value left in `localStorage` by older
      // builds is simply never read.
      detection: {
        order: ['navigator'],
        caches: [],
      },
      resources: {
        en: { translation: enTranslations },
      },
    });

  // Belt-and-suspenders: set it directly too, in case some i18next
  // version/config path resolves the initial language without firing
  // the event synchronously during `init()`.
  syncDocumentLanguage(i18n.language);

  // If the detected language is not English, fetch its file and apply it
  // -- but only a language MeedyaDL actually has, and never letting a
  // failure stop startup. Startup awaits this function, and settings are
  // loaded AFTER it, so a throw here stopped the whole app from starting
  // on, for example, a Spanish system, whatever language was saved
  // (Codex, batch-3 review, reproduced -- caused by making a failed load
  // report itself). A failure here just leaves English; the saved
  // language is applied once settings load.
  const detected = baseLanguageOf(i18n.language);
  if (detected !== 'en' && (AVAILABLE_LOCALES as readonly string[]).includes(detected)) {
    // `changeUiLanguage()` does two things: fetches the file (if it
    // isn't already loaded) and then calls `i18n.changeLanguage()`.
    // That second step matters on its own, separately from the fetch:
    // adding a resource bundle does NOT, by itself, tell React that
    // anything changed. react-i18next only re-renders components when
    // i18next fires its `languageChanged` event, and just adding the
    // data to memory does not fire that event.
    //
    // Concretely, this used to mean: a German or French user who had
    // never touched the language dropdown (the default state, since
    // `ui_language` starts empty and just means "whatever the OS
    // reports") would have the German file fetched and stored... and
    // then every already-rendered component would go on showing English
    // forever, because nothing ever told them to look again.
    //
    // Passing the language we are already on into `changeLanguage` looks
    // like a no-op, but the event it fires is the whole point — that's
    // what actually makes the screen update.
    try {
      await changeUiLanguage(i18n.language);
    } catch (err) {
      console.warn('Could not load the system language at startup; using English:', err);
    }
  } else if (detected !== 'en') {
    // Not a language MeedyaDL has: show English, and say so in the log.
    console.info(`No ${detected} translation is available; using English.`);
    await i18n.changeLanguage('en');
  }
}

export default i18n;
