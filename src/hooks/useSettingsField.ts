// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file useSettingsField — typed selector + setter for one AppSettings key.
 *
 * Audit v2 finding #6: every settings tab opens with the same
 * boilerplate plus per-control wiring:
 *
 *   const settings = useSettingsStore((s) => s.settings);
 *   const updateSettings = useSettingsStore((s) => s.updateSettings);
 *   // ...
 *   <Toggle
 *     checked={settings.acoustid_enabled}
 *     onChange={(checked) => updateSettings({ acoustid_enabled: checked })}
 *   />
 *
 * 10 tabs × ~20 form controls × ~3 lines of wiring per control =
 * ~600 LOC of identical lambdas. Each one duplicates the field key
 * (once in `settings.X` read, once in `{ X: v }` write) — a future
 * rename has to walk every site.
 *
 * This hook collapses the wiring to:
 *
 *   const acoustid = useSettingsField('acoustid_enabled');
 *   <Toggle checked={acoustid.value} onChange={acoustid.set} />
 *
 * The key is supplied once. TypeScript checks `K extends keyof
 * AppSettings`, so a typo is a compile error and `value`/`set` are
 * narrowly typed to the field's actual type.
 *
 * **Subscription efficiency:** each `useSettingsField('X')` call
 * subscribes to `state.settings.X` only — Zustand re-renders the
 * consumer only when that field changes, not on unrelated settings
 * updates. The audit-v1 selector pattern is preserved per-field
 * automatically.
 *
 * **Multi-service relevance:** the M8/M9/M10 per-service settings
 * tabs (BBC iPlayer session, Spotify credentials, YouTube API key)
 * will land roughly 20 form controls each. Without this hook, each
 * tab adds another ~60 LOC of identical lambdas. With it: ~10 LOC.
 *
 * @see useSettingsStore — the underlying Zustand store
 * @see audit doc finding #6 in
 *      [.github/audits/codebase-unification-audit-v2.md]
 */

import { useCallback } from 'react';
import { useSettingsStore } from '@/stores/settingsStore';
import type { AppSettings } from '@/types';

/**
 * Reactive accessor for a single AppSettings key.
 *
 * `value` is narrowed to the field's actual type (e.g. `boolean` for
 * a toggle, `string` for a text input, `number` for a numeric slider,
 * union types like `'normal' | 'wide'` preserved verbatim).
 *
 * `set` accepts the same type and calls
 * `updateSettings({ [key]: newValue })` under the hood — no key
 * duplication at the call site.
 */
export interface SettingsField<T> {
  /** Current value of the field. Re-renders when this field changes. */
  value: T;
  /** Set the field to a new value. Marks settings as dirty. */
  set: (next: T) => void;
}

/**
 * Read + set a single field from the AppSettings Zustand store.
 *
 * @param key — a key of the AppSettings interface
 * @returns `{ value, set }` — narrowly typed to `AppSettings[K]`
 *
 * @example Toggle wiring
 *   const enabled = useSettingsField('acoustid_enabled');
 *   <Toggle checked={enabled.value} onChange={enabled.set} />
 *
 * @example Select with a custom string conversion
 *   const lang = useSettingsField('ui_language');
 *   <Select value={lang.value} onChange={(v) => lang.set(v)} options={...} />
 *
 * @example Numeric input requiring parsing
 *   const interval = useSettingsField('update_check_interval_hours');
 *   <input
 *     type="number"
 *     value={interval.value}
 *     onChange={(e) => interval.set(parseInt(e.target.value, 10))}
 *   />
 */
export function useSettingsField<K extends keyof AppSettings>(
  key: K,
): SettingsField<AppSettings[K]> {
  const value = useSettingsStore((s) => s.settings[key]);
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  // Memoise the setter so consumers can pass `field.set` directly to
  // onChange without per-render reference churn (matters for memoised
  // components downstream).
  const set = useCallback(
    (next: AppSettings[K]) => {
      updateSettings({ [key]: next } as Partial<AppSettings>);
    },
    [key, updateSettings],
  );
  return { value, set };
}
