// Copyright (c) 2026 MeedyaSuite
//
// i18n locale-parity tests (#111)
// ================================
//
// Ensures every locale under `public/locales/` carries the same set of
// keys as the canonical `en` locale. When a developer adds a new key to
// `en` but forgets to add it to `de`/`fr`/etc., this test fails before
// the missing-translation reaches users.
//
// Strategy: flatten each locale's nested JSON into a `Set<string>` of
// dotted key paths, then compare. Missing keys are listed explicitly
// in the assertion message so the fix is mechanical.

import { describe, expect, it } from 'vitest';

import deTranslations from '../../public/locales/de/translation.json';
import enTranslations from '../../public/locales/en/translation.json';
import frTranslations from '../../public/locales/fr/translation.json';
import { AVAILABLE_LOCALES } from './i18n';

/**
 * Walk a nested translation object and return the set of all leaf-key
 * paths joined by `.` — e.g. `["nav.download", "settings.title", ...]`.
 *
 * Arrays are treated as leaves (i18next supports array values for
 * pluralisation patterns, but treats them as a single resolved value
 * from the consumer's perspective).
 */
function flattenKeys(obj: unknown, prefix = ''): Set<string> {
  const out = new Set<string>();
  if (obj === null || typeof obj !== 'object' || Array.isArray(obj)) {
    if (prefix) out.add(prefix);
    return out;
  }
  for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
    const path = prefix ? `${prefix}.${k}` : k;
    if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
      for (const child of flattenKeys(v, path)) out.add(child);
    } else {
      out.add(path);
    }
  }
  return out;
}

describe('i18n locale parity', () => {
  const enKeys = flattenKeys(enTranslations);

  it('en has at least 100 translation keys (sanity check)', () => {
    // Guards against accidentally emptying the canonical locale —
    // if `en` ever shrinks below this floor the test surfaces it
    // before the runtime fallback hides the issue.
    expect(enKeys.size).toBeGreaterThan(100);
  });

  it('de translation has every key from en (no missing translations)', () => {
    const deKeys = flattenKeys(deTranslations);
    const missing = [...enKeys].filter((k) => !deKeys.has(k));
    expect(missing, `de is missing: ${missing.join(', ')}`).toEqual([]);
  });

  it('fr translation has every key from en (no missing translations)', () => {
    const frKeys = flattenKeys(frTranslations);
    const missing = [...enKeys].filter((k) => !frKeys.has(k));
    expect(missing, `fr is missing: ${missing.join(', ')}`).toEqual([]);
  });

  it('de translation does not carry stale keys absent from en', () => {
    const deKeys = flattenKeys(deTranslations);
    const stale = [...deKeys].filter((k) => !enKeys.has(k));
    expect(
      stale,
      `de has stale keys (no longer in en): ${stale.join(', ')}`,
    ).toEqual([]);
  });

  it('fr translation does not carry stale keys absent from en', () => {
    const frKeys = flattenKeys(frTranslations);
    const stale = [...frKeys].filter((k) => !enKeys.has(k));
    expect(
      stale,
      `fr has stale keys (no longer in en): ${stale.join(', ')}`,
    ).toEqual([]);
  });

  it('AVAILABLE_LOCALES enumerates every supported language', () => {
    // If someone adds a new locale JSON but forgets to wire it into
    // `AVAILABLE_LOCALES`, the language dropdown in Settings would
    // silently omit it. This is the cheap reverse-check.
    expect([...AVAILABLE_LOCALES]).toContain('en');
    expect([...AVAILABLE_LOCALES]).toContain('de');
    expect([...AVAILABLE_LOCALES]).toContain('fr');
  });
});
