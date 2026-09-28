// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for the Metadata Language list helpers (#1249).
 *
 * The ORDER of the list is decided by the shared Rust code, and is tested
 * there (src-tauri/src/utils/language.rs, and the policy conformance test).
 * What is tested here is the part only the frontend can do: names in the
 * interface language from the platform's own data (policy UI-010), the
 * alphabetical order of language names that the Rust ordering asks for,
 * the fallback order used when the Rust command cannot answer, and keeping
 * a saved value that is not in the offered list.
 *
 * These use the test runner's own `Intl` data (Node ships full ICU), so an
 * exact name could differ between ICU versions; the assertions are chosen
 * to hold for any reasonable version.
 */

import { describe, expect, it } from 'vitest';

import {
  METADATA_LANGUAGE_TAGS,
  alphabeticalPrimaryOrder,
  collatorFallbackOrder,
  isSameList,
  languageDisplayName,
  languagePreferences,
  interfaceLanguageLabel,
  primaryLanguageOf,
  toLanguageOptions,
  withSavedValues,
} from './languageOptions';

describe('METADATA_LANGUAGE_TAGS', () => {
  it('offers the script-bearing Chinese tags Apple lists, not a country standing in for a script', () => {
    expect(METADATA_LANGUAGE_TAGS).toContain('zh-Hans-CN');
    expect(METADATA_LANGUAGE_TAGS).toContain('zh-Hant-TW');
    expect(METADATA_LANGUAGE_TAGS).not.toContain('zh-CN');
    expect(METADATA_LANGUAGE_TAGS).not.toContain('zh-TW');
  });

  it('has no repeats', () => {
    expect(new Set(METADATA_LANGUAGE_TAGS).size).toBe(METADATA_LANGUAGE_TAGS.length);
  });
});

describe('languageDisplayName', () => {
  it('names a language in the interface language, with its qualifier', () => {
    expect(languageDisplayName('en-GB', 'en')).toBe('English (United Kingdom)');
    expect(languageDisplayName('de-DE', 'de')).toMatch(/^Deutsch/);
    expect(languageDisplayName('de-DE', 'fr')).toMatch(/^allemand/);
  });

  it('says Simplified and Traditional from the tag itself', () => {
    expect(languageDisplayName('zh-Hans-CN', 'en')).toMatch(/Simplified/);
    expect(languageDisplayName('zh-Hant-TW', 'en')).toMatch(/Traditional/);
  });

  it('names an old saved value from its own tag', () => {
    // No "Simplified" invented for zh-CN: the tag says a country, not a script.
    expect(languageDisplayName('zh-CN', 'en')).toBe('Chinese (China)');
  });

  it('shows a value that is not a tag as its own text rather than hiding it', () => {
    expect(languageDisplayName('en_US', 'en')).toBe('en_US');
  });

  it('still names things when the interface language is not usable', () => {
    expect(languageDisplayName('ja-JP', 'not a language!')).not.toBe('');
  });
});

describe('primaryLanguageOf', () => {
  it('reads the primary language part with the platform parser', () => {
    expect(primaryLanguageOf('zh-Hant-TW')).toBe('zh');
    expect(primaryLanguageOf('EN-gb')).toBe('en');
  });

  it('gives null for a value that is not a tag', () => {
    expect(primaryLanguageOf('en_US')).toBeNull();
    expect(primaryLanguageOf('')).toBeNull();
  });
});

describe('alphabeticalPrimaryOrder', () => {
  it('orders distinct primary languages by their name in the interface language', () => {
    const tags = ['ja-JP', 'de-DE', 'en-US', 'en-GB', 'zh-Hans-CN'];
    // English names: Chinese, English, German, Japanese.
    expect(alphabeticalPrimaryOrder(tags, 'en')).toEqual(['zh', 'en', 'de', 'ja']);
    // German names: Chinesisch, Deutsch, Englisch, Japanisch.
    expect(alphabeticalPrimaryOrder(tags, 'de')).toEqual(['zh', 'de', 'en', 'ja']);
  });

  it('leaves out values that are not tags (they are ordered last by the Rust side)', () => {
    expect(alphabeticalPrimaryOrder(['en_US', 'fr-FR'], 'en')).toEqual(['fr']);
  });
});

describe('collatorFallbackOrder', () => {
  it('gives every entry, alphabetically by displayed name', () => {
    const tags = ['ja-JP', 'de-DE', 'en-GB', 'en-US'];
    const ordered = collatorFallbackOrder(tags, 'en');
    expect(isSameList(ordered, tags)).toBe(true);
    expect(ordered).toEqual(['en-GB', 'en-US', 'de-DE', 'ja-JP']);
  });
});

describe('languagePreferences', () => {
  it('puts the interface language first, then the system languages, without repeats', () => {
    expect(languagePreferences('en', ['en-GB', 'EN', 'fr', ''])).toEqual(['en', 'en-GB', 'fr']);
  });
});

describe('withSavedValues', () => {
  it('adds a saved value that is not offered, and nothing else', () => {
    expect(withSavedValues(['en-US', 'ja-JP'], ['zh-CN'])).toEqual(['en-US', 'ja-JP', 'zh-CN']);
    expect(withSavedValues(['en-US', 'ja-JP'], ['ja-JP', ''])).toEqual(['en-US', 'ja-JP']);
  });
});

describe('isSameList', () => {
  it('accepts a reordering and rejects a lost or added entry', () => {
    expect(isSameList(['b', 'a'], ['a', 'b'])).toBe(true);
    expect(isSameList(['a'], ['a', 'b'])).toBe(false);
    expect(isSameList(['a', 'c'], ['a', 'b'])).toBe(false);
  });
});

describe('toLanguageOptions', () => {
  it('keeps the tag as the value and uses the name as the label', () => {
    expect(toLanguageOptions(['en-GB'], 'en')).toEqual([
      { value: 'en-GB', label: 'English (United Kingdom)' },
    ]);
  });
});

describe('interfaceLanguageLabel', () => {
  it("shows the name in the interface language first, then the language's own name", () => {
    expect(interfaceLanguageLabel('de', 'Deutsch', 'en')).toBe('German — Deutsch');
    expect(interfaceLanguageLabel('de', 'Deutsch', 'en', 'automatische Übersetzung')).toBe(
      'German — Deutsch (automatische Übersetzung)'
    );
  });

  it('shows one name when the two are the same apart from letter case', () => {
    expect(interfaceLanguageLabel('en', 'English', 'en')).toBe('English');
    expect(interfaceLanguageLabel('fr', 'Français', 'fr', 'traduction automatique')).toBe(
      'Français (traduction automatique)'
    );
  });
});
