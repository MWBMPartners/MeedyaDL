// Copyright (c) 2026 MeedyaSuite
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
import i18n, { AVAILABLE_LOCALES, LOCALES, baseLanguageOf, initI18n, isMachineAssisted } from './i18n';

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
    // The detector has two places to look, in this order: the language
    // it remembered last time, then the browser. This test sets BOTH to
    // German rather than picking one, because which of them is available
    // is not the same everywhere.
    //
    // That is not caution for its own sake. This test passed here and
    // failed on all three build machines. The reason turned out to be
    // the version of Node: on this machine `localStorage` is not usable
    // at all, so the detector fell through to the browser setting below
    // and found German. On the build machines it works, so the detector
    // answered from what it had remembered and never looked at the
    // browser -- and the test was quietly checking nothing.
    //
    // Setting both means the answer is German whichever one it consults,
    // on whatever version of Node it happens to be running.
    try {
      window.localStorage.setItem('meedyadl-ui-language', 'de-DE');
    } catch {
      // No usable localStorage here. The browser setting below is then
      // the only source, and it says German too.
    }

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
    // Clear the remembered language too, so this test does not decide the
    // language for anything that runs after it.
    try {
      window.localStorage.removeItem('meedyadl-ui-language');
    } catch {
      // No usable localStorage here; nothing was remembered to clear.
    }
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
