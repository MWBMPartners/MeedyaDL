// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// i18n tests (#111)
// ==================
//
// Two kinds of test live here:
//
//   1. A regression test for a real bug that shipped: a German or French
//      user who never touched the language dropdown (the default state)
//      never actually saw the app in their own language, even though the
//      right translation file WAS being downloaded. See the big comment
//      on the first test below for the full story.
//
//   2. Data-quality checks on the translation files themselves. Every
//      locale under `public/locales/` must carry the same set of keys as
//      the canonical `en` locale, with no blanks and no dropped
//      placeholders. When a developer adds a key to `en` but forgets to
//      add it to `de`/`fr`, this test fails before the gap reaches users
//      as a raw `some.missing.key` on screen, or as invisible blank text.

import { describe, expect, it, beforeEach, afterEach, vi } from 'vitest';
import { createElement } from 'react';
import { render, screen, act } from '@testing-library/react';
import { useTranslation } from 'react-i18next';

import deTranslations from '../../public/locales/de/translation.json';
import enTranslations from '../../public/locales/en/translation.json';
import frTranslations from '../../public/locales/fr/translation.json';
import i18n, {
  AVAILABLE_LOCALES,
  LOCALES,
  baseLanguageOf,
  changeUiLanguage,
  initI18n,
  systemLanguageOrEnglish,
  isMachineAssisted,
} from './i18n';

/* ------------------------------------------------------------------ */
/* 1. The addResourceBundle-doesn't-repaint-the-screen bug             */
/* ------------------------------------------------------------------ */

// A tiny component whose whole job is to render one translated string, so
// the test can look at the screen and ask "which language is this". This
// file is plain .ts (not .tsx), so it builds the element with
// createElement instead of JSX syntax.
function LanguageProbe() {
  const { t } = useTranslation();
  return createElement('div', { 'data-testid': 'probe' }, t('nav.queue'));
}

describe('initI18n shows the detected language, not stale English (#111)', () => {
  beforeEach(() => {
    // Pretend the user's OS/browser is set to German and that they've
    // never touched the in-app language dropdown -- this is the default,
    // most common case described in the bug report: `ui_language` starts
    // empty, which means "whatever the OS reports". i18next's detector
    // order is ['localStorage', 'navigator'], so with nothing remembered
    // it falls through to reading `navigator.language`, exactly like a
    // real browser reporting the OS locale would.
    //
    // The detector now reads ONLY the system (browser) language -- it no
    // longer keeps or reads a remembered copy in localStorage (see the
    // `detection` block in initI18n()). So the browser language below is
    // the whole of the setup.
    //
    // (This test used to set localStorage to German as well, because the
    // detector read that first -- and on some versions of Node it was
    // available and on others not, which once made this test quietly
    // check nothing on the build machines. With localStorage out of the
    // picture, that trap is gone too.)
    vi.stubGlobal('navigator', { language: 'de-DE', languages: ['de-DE'] });

    // There is no real network in a test run, so stand in for the browser
    // fetch that `loadLocaleResources()` makes for the German file. This
    // mirrors exactly what a real browser would hand back.
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string) => {
        if (typeof url === 'string' && url.includes('/locales/de/translation.json')) {
          return Promise.resolve({
            ok: true,
            json: () => Promise.resolve(deTranslations),
          } as Response);
        }
        return Promise.resolve({ ok: false } as Response);
      })
    );
  });

  afterEach(async () => {
    // Leave the shared i18next instance the way every other test file
    // expects to find it: English, and the real navigator/fetch back in
    // place.
    vi.unstubAllGlobals();
    await i18n.changeLanguage('en');
    // Also put the <html> tag back to English -- initI18n's language
    // listener will have moved it, and it is shared, real jsdom state
    // for the rest of this file's tests.
    document.documentElement.lang = 'en';
  });

  it('renders German text after startup for a German-OS user who never opened Settings', async () => {
    // This is the bug, made concrete. A German user installs MeedyaDL,
    // opens it for the first time, and never goes near the language
    // dropdown. Before the fix, they would sit staring at English
    // forever: `initI18n()` correctly detects "de", correctly fetches
    // de/translation.json, correctly hands it to i18next... and then
    // nothing on screen ever changes, because adding a translation
    // bundle does not, by itself, tell React to re-render. The fetch
    // succeeding gave every appearance of the feature working, while the
    // one visible thing a user cares about -- the actual text on
    // screen -- stayed wrong. That gap between "looks fixed in the
    // network tab" and "still broken for the user" is exactly why this
    // needs a test that checks rendered text, not just that a promise
    // resolved.
    render(createElement(LanguageProbe));

    // Sanity check on the starting state: before initI18n runs (again,
    // simulating first startup), the probe is showing whatever the
    // shared i18n instance already had loaded -- English, from the
    // global test setup.
    expect(screen.getByTestId('probe')).toHaveTextContent('Queue');

    // initI18n touches i18next state outside of a React event handler,
    // so it has to be wrapped in act() or React Testing Library warns
    // that an update happened it didn't get to observe.
    await act(async () => {
      await initI18n();
    });

    // Before checking the screen, check that the setup actually took --
    // that startup really did decide on German. If it did not, then this
    // test is not testing the bug at all, and saying so plainly here is
    // far more useful than the puzzle of "expected the German word, got
    // the English one", which reads like the fix has broken when really
    // the test never got as far as trying it. That exact confusion cost
    // real time once already.
    expect(i18n.language.split('-')[0]).toBe('de');

    // The real assertion: after startup finishes, the screen shows the
    // German word, not the English one. Without the `changeLanguage`
    // fix in `initI18n()`, this line is the one that fails -- the probe
    // keeps showing "Queue" even though the German file was fetched.
    expect(screen.getByTestId('probe')).toHaveTextContent('Warteschlange');
  });

  it('sets <html lang> to the detected language at startup (#WCAG 3.1.1)', async () => {
    // `index.html` hard-codes lang="en" and nothing used to update it.
    // A screen reader picks its pronunciation rules from this attribute,
    // so a German-OS user who never opens Settings would have every
    // German word on screen read aloud with English pronunciation rules
    // -- the text was correct, the announced LANGUAGE of that text was
    // not. Sanity check first: jsdom's default is "en" before anything
    // runs, so a pass here has to come from the fix, not the starting
    // state.
    expect(document.documentElement.lang).toBe('en');
    await act(async () => {
      await initI18n();
    });
    expect(document.documentElement.lang).toBe('de');
  });
});

describe('document.documentElement.lang tracks every later language change too', () => {
  beforeEach(async () => {
    // Deliberately does not depend on a previous test having already
    // called `initI18n()` -- that would make this test's result depend
    // on file-wide execution order, which is exactly the "passed here,
    // failed on the build machine" trap the big comment earlier in this
    // file already burned time on once. `initI18n()` is what registers
    // the listener under test (see i18n.ts), and it is idempotent to
    // call again, so calling it explicitly here makes the precondition
    // this test needs true regardless of what ran before it.
    await act(async () => {
      await initI18n();
    });
  });

  afterEach(async () => {
    await i18n.changeLanguage('en');
    document.documentElement.lang = 'en';
  });

  it('updates when the language changes after startup, not only at startup', async () => {
    // App.tsx's Effect 3b calls `changeUiLanguage(...)` (which itself
    // calls `i18n.changeLanguage()`) whenever the `ui_language` setting
    // changes -- including a language picked from the Settings > General
    // dropdown while the app is already running, long after `initI18n()`
    // has finished. If the <html> tag were only set once, at startup,
    // switching languages in Settings would leave it wrong for the rest
    // of the session.
    await act(async () => {
      await i18n.changeLanguage('fr');
    });
    expect(document.documentElement.lang).toBe('fr');

    // And back, proving this isn't a one-way/one-shot listener either.
    await act(async () => {
      await i18n.changeLanguage('de');
    });
    expect(document.documentElement.lang).toBe('de');
  });
});

/* ------------------------------------------------------------------ */
/* 1b. changeUiLanguage -- switching language should take one call,    */
/*     not two restarts                                                */
/* ------------------------------------------------------------------ */

describe('changeUiLanguage fetches the file itself, rather than assuming it is already loaded', () => {
  beforeEach(() => {
    // Deliberately NOT setting German in localStorage or navigator.language
    // here. This reproduces picking German from the Settings dropdown on a
    // machine whose OS/browser is English -- `initI18n()`'s own
    // auto-detection never runs for, or fetches, German in that case.
    // `changeUiLanguage` has to do its own fetching; it cannot be riding on
    // work initI18n already did.
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string) => {
        if (typeof url === 'string' && url.includes('/locales/de/translation.json')) {
          return Promise.resolve({
            ok: true,
            json: () => Promise.resolve(deTranslations),
          } as Response);
        }
        return Promise.resolve({ ok: false } as Response);
      })
    );
  });

  afterEach(async () => {
    vi.unstubAllGlobals();
    await i18n.changeLanguage('en');
    document.documentElement.lang = 'en';
  });

  it('shows the new language after a single call', async () => {
    // This is the bug, made concrete. Before this function existed,
    // App.tsx called `i18next.changeLanguage('de')` straight from a
    // freshly-loaded setting -- exactly like this test does one line
    // down, but with the plain, unfixed call. Nothing had ever fetched
    // de/translation.json, and there is no i18next HTTP backend
    // registered to do that automatically (see i18n.ts's own comment on
    // this function). The probe would keep showing "Queue" even though
    // `i18n.language` claimed to be "de".
    render(createElement(LanguageProbe));
    expect(screen.getByTestId('probe')).toHaveTextContent('Queue');

    await act(async () => {
      await changeUiLanguage('de');
    });

    // The real assertion: one call, and the file was already fetched as
    // part of it -- no prior `initI18n()` detection of German, no second
    // restart needed.
    expect(i18n.language).toBe('de');
    expect(screen.getByTestId('probe')).toHaveTextContent('Warteschlange');
  });

  it('reports a language file that could not be loaded, instead of staying English in silence', async () => {
    // "zz" has no file, and the stand-in fetch refuses anything but German.
    // Before, the loader swallowed that and the switch "worked" -- with
    // every string still English and nothing said.
    await expect(changeUiLanguage('zz')).rejects.toThrow(/could not be loaded/);
  });

  it('keeps no copy of the language in the browser -- the saved setting is the only record', () => {
    // A language merely tried out in Settings used to be written into
    // localStorage and read back FIRST at the next start, so it came back
    // although Save was never pressed. This pins the configuration itself
    // rather than poking localStorage, because whether localStorage is
    // usable here depends on the Node version -- which once made a test in
    // this file quietly check nothing.
    const detection = i18n.options.detection as { order?: string[]; caches?: string[] };
    expect(detection.order).toEqual(['navigator']);
    expect(detection.caches).toEqual([]);
  });

  it('applies only the latest request when an earlier language file arrives late', async () => {
    // German is slow to load; English is chosen before it arrives. The
    // late German file used to switch the screen back to German (Codex,
    // batch-3 review).
    let releaseGerman: (() => void) | undefined;
    vi.stubGlobal(
      'fetch',
      vi.fn(
        () =>
          new Promise<Response>((resolve) => {
            releaseGerman = () =>
              resolve({ ok: true, json: () => Promise.resolve(deTranslations) } as Response);
          }),
      ),
    );
    // Make sure German is not already loaded from an earlier test.
    i18n.removeResourceBundle('de', 'translation');

    const german = changeUiLanguage('de');
    await changeUiLanguage('en');
    releaseGerman?.();
    await german;

    expect(i18n.language).toBe('en');
  });

  it('does not re-fetch a language that has already been loaded', async () => {
    await act(async () => {
      await changeUiLanguage('de');
    });
    const fetchMock = fetch as unknown as ReturnType<typeof vi.fn>;
    const callsAfterFirstLoad = fetchMock.mock.calls.length;

    // Switching away and back -- e.g. someone previewing two languages
    // in the Settings dropdown -- must not hit the network again for a
    // language i18next already has the data for in memory.
    await act(async () => {
      await changeUiLanguage('en');
    });
    await act(async () => {
      await changeUiLanguage('de');
    });

    expect(fetchMock.mock.calls.length).toBe(callsAfterFirstLoad);
  });
});

/* ------------------------------------------------------------------ */
/* 2. Locale file data-quality checks                                  */
/* ------------------------------------------------------------------ */

/**
 * Walk a nested translation object and return the set of all leaf-key
 * paths joined by `.` -- e.g. `["nav.download", "settings.title", ...]`.
 *
 * Arrays are treated as leaves (i18next supports array values for
 * pluralisation patterns, but treats them as a single resolved value
 * from the consumer's perspective).
 */
function flattenKeys(obj: unknown, prefix = ''): Map<string, unknown> {
  const out = new Map<string, unknown>();
  if (obj === null || typeof obj !== 'object' || Array.isArray(obj)) {
    if (prefix) out.set(prefix, obj);
    return out;
  }
  for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
    const path = prefix ? `${prefix}.${k}` : k;
    if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
      for (const [childKey, childVal] of flattenKeys(v, path)) out.set(childKey, childVal);
    } else {
      out.set(path, v);
    }
  }
  return out;
}

/** Every `{{placeholder}}` token found in a string, e.g. `new Set(['{{count}}'])`. */
function placeholdersIn(value: unknown): Set<string> {
  if (typeof value !== 'string') return new Set();
  const matches = value.match(/\{\{.*?\}\}/g);
  return new Set(matches ?? []);
}

describe('startup on a system whose language MeedyaDL does not have', () => {
  afterEach(async () => {
    vi.unstubAllGlobals();
    await i18n.changeLanguage('en');
    document.documentElement.lang = 'en';
  });

  it('starts in English instead of stopping the app', async () => {
    // A Spanish system: there is no Spanish file. Startup awaits
    // initI18n() and loads settings after it, so a failure here used to
    // stop the whole app from starting (Codex, batch-3 review).
    vi.stubGlobal('navigator', { language: 'es-ES', languages: ['es-ES'] });
    vi.stubGlobal('fetch', vi.fn(() => Promise.resolve({ ok: false } as Response)));

    await expect(initI18n()).resolves.toBeUndefined();
    expect(baseLanguageOf(i18n.language)).toBe('en');
  });

  it('"Auto" chooses English, not a language with no file', () => {
    vi.stubGlobal('navigator', { language: 'es-ES', languages: ['es-ES'] });
    expect(systemLanguageOrEnglish()).toBe('en');
    vi.stubGlobal('navigator', { language: 'de-DE', languages: ['de-DE'] });
    expect(systemLanguageOrEnglish()).toBe('de-DE');
  });
});

describe('translation file quality', () => {
  const enKeys = flattenKeys(enTranslations);
  const locales: Record<string, unknown> = { de: deTranslations, fr: frTranslations };

  it('en has at least 100 translation keys (sanity check)', () => {
    // Guards against accidentally emptying the canonical locale -- if
    // `en` ever shrinks below this floor the test surfaces it before the
    // runtime fallback hides the issue.
    expect(enKeys.size).toBeGreaterThan(100);
  });

  for (const [code, translations] of Object.entries(locales)) {
    describe(`${code} locale`, () => {
      const keys = flattenKeys(translations);

      it('has every key that en has (no missing translations)', () => {
        // A key that's missing from a locale isn't a visual bug in that
        // language -- it's the ENGLISH fallback quietly leaking through,
        // which a French or German reader would read as "this app
        // wasn't finished" even though the feature works fine.
        const missing = [...enKeys.keys()].filter((k) => !keys.has(k));
        expect(missing, `${code} is missing: ${missing.join(', ')}`).toEqual([]);
      });

      it('has no stale keys that en no longer has', () => {
        // The opposite drift: a key removed from en but left behind in a
        // translation file. Harmless at runtime (nothing looks it up
        // any more) but it's dead weight that will confuse the next
        // person trying to translate a genuinely new key, and it means
        // the file no longer tells the truth about what's translated.
        const stale = [...keys.keys()].filter((k) => !enKeys.has(k));
        expect(stale, `${code} has stale keys no longer in en: ${stale.join(', ')}`).toEqual([]);
      });

      it('has no empty-string values', () => {
        // An empty translated string is invisible: the UI renders
        // nothing where a label or button text should be, and it looks
        // exactly like a rendering bug in the app rather than a gap in
        // the translation -- there's no error, no missing-key fallback,
        // just a blank space where words should be.
        const empties = [...keys.entries()]
          .filter(([, v]) => typeof v === 'string' && v === '')
          .map(([k]) => k);
        expect(empties, `${code} has empty-string values for: ${empties.join(', ')}`).toEqual([]);
      });

      it('keeps the same {{placeholder}} tokens as en for every shared key', () => {
        // A translator can reorder or drop a `{{count}}`/`{{name}}`
        // placeholder without meaning to -- the sentence still reads
        // fine in isolation. The break only shows up at runtime, in the
        // one language the developer testing the change usually can't
        // read, when i18next either leaves the literal "{{count}}" in
        // the rendered text or silently omits the value.
        const mismatches: string[] = [];
        for (const key of enKeys.keys()) {
          if (!keys.has(key)) continue; // already reported by the missing-keys test
          const enPlaceholders = placeholdersIn(enKeys.get(key));
          const otherPlaceholders = placeholdersIn(keys.get(key));
          const same =
            enPlaceholders.size === otherPlaceholders.size &&
            [...enPlaceholders].every((p) => otherPlaceholders.has(p));
          if (!same) {
            mismatches.push(
              `${key} (en: ${[...enPlaceholders].join(',') || 'none'} vs ${code}: ${
                [...otherPlaceholders].join(',') || 'none'
              })`
            );
          }
        }
        expect(mismatches, `placeholder mismatches: ${mismatches.join('; ')}`).toEqual([]);
      });
    });
  }

  it('AVAILABLE_LOCALES and LOCALES enumerate every supported language', () => {
    // If someone adds a new locale JSON but forgets to add an entry to
    // LOCALES in src/lib/i18n.ts, the language dropdown in Settings
    // would silently omit it. This is the cheap reverse-check.
    expect([...AVAILABLE_LOCALES]).toEqual(['en', 'de', 'fr']);
    expect(LOCALES.map((l) => l.code)).toEqual([...AVAILABLE_LOCALES]);
  });
});

/* ------------------------------------------------------------------ */
/* 3. baseLanguageOf / isMachineAssisted                                */
/* ------------------------------------------------------------------ */

describe('baseLanguageOf', () => {
  it('strips the region from a full locale tag, e.g. "de-DE" -> "de"', () => {
    // This is what makes a Windows/macOS "German (Germany)" locale report
    // resolve to the same folder as a plain "de" -- if this regressed,
    // a whole region of German-speaking users would silently fall back
    // to English despite there being a perfectly good de/translation.json.
    expect(baseLanguageOf('de-DE')).toBe('de');
  });

  it('leaves an already-bare code alone', () => {
    expect(baseLanguageOf('de')).toBe('de');
  });

  it('falls back to "en" for an empty string', () => {
    expect(baseLanguageOf('')).toBe('en');
  });

  it('falls back to "en" for undefined', () => {
    // i18next's `.language` can be undefined before detection completes;
    // callers must not crash or silently pick a wrong locale in that
    // window.
    expect(baseLanguageOf(undefined)).toBe('en');
  });
});

describe('isMachineAssisted', () => {
  it('is false for English', () => {
    // English is the language every string was originally written in --
    // there's nothing to disclaim.
    expect(isMachineAssisted('en')).toBe(false);
    expect(isMachineAssisted('en-US')).toBe(false);
  });

  it('is true for German', () => {
    // If this ever flips to false without a real human review of
    // de/translation.json, the app would stop telling German readers
    // that the text they're reading hasn't been checked by a person --
    // silently promoting an unreviewed translation to "trusted".
    expect(isMachineAssisted('de')).toBe(true);
    expect(isMachineAssisted('de-DE')).toBe(true);
  });

  it('is true for French', () => {
    expect(isMachineAssisted('fr')).toBe(true);
    expect(isMachineAssisted('fr-FR')).toBe(true);
  });
});
