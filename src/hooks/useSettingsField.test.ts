// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for useSettingsField (audit v2 #6).
 *
 * Pins the hook contract:
 *   - `value` reflects the current settings field value
 *   - `value` updates reactively when the store changes
 *   - `set(next)` writes via updateSettings without duplicating the key
 *   - `set` is referentially stable across renders (useCallback)
 *   - Different field types (boolean, string, number, union) all work
 */

import { renderHook, act } from '@testing-library/react';
import { useSettingsField } from '@/hooks/useSettingsField';
import { useSettingsStore } from '@/stores/settingsStore';

beforeEach(() => {
  // Reset to a known shape — only fields the tests care about matter.
  // We use real updateSettings so we exercise the store's actual
  // write path (shallow merge + isDirty flag).
  act(() => {
    useSettingsStore.setState({
      isDirty: false,
      settings: {
        ...useSettingsStore.getState().settings,
        acoustid_enabled: false,
        ui_language: 'en-US',
        update_check_interval_hours: 6,
        colour_blind_mode: '' as '' | 'deuteranopia' | 'protanopia' | 'tritanopia',
      },
    });
  });
});

describe('useSettingsField', () => {
  // ===========================================================================
  // Read path
  // ===========================================================================

  it('returns the current value of a boolean field', () => {
    const { result } = renderHook(() => useSettingsField('acoustid_enabled'));
    expect(result.current.value).toBe(false);
  });

  it('returns the current value of a string field', () => {
    const { result } = renderHook(() => useSettingsField('ui_language'));
    expect(result.current.value).toBe('en-US');
  });

  it('returns the current value of a numeric field', () => {
    const { result } = renderHook(() => useSettingsField('update_check_interval_hours'));
    expect(result.current.value).toBe(6);
  });

  it('reflects external store updates reactively', () => {
    const { result } = renderHook(() => useSettingsField('acoustid_enabled'));
    expect(result.current.value).toBe(false);
    act(() => {
      useSettingsStore.getState().updateSettings({ acoustid_enabled: true });
    });
    expect(result.current.value).toBe(true);
  });

  // ===========================================================================
  // Write path
  // ===========================================================================

  it('set() writes the new value via updateSettings', () => {
    const { result } = renderHook(() => useSettingsField('acoustid_enabled'));
    act(() => {
      result.current.set(true);
    });
    expect(useSettingsStore.getState().settings.acoustid_enabled).toBe(true);
    expect(useSettingsStore.getState().isDirty).toBe(true);
  });

  it('set() does not touch other fields (shallow merge)', () => {
    const { result } = renderHook(() => useSettingsField('acoustid_enabled'));
    const beforeLang = useSettingsStore.getState().settings.ui_language;
    act(() => {
      result.current.set(true);
    });
    const afterLang = useSettingsStore.getState().settings.ui_language;
    expect(afterLang).toBe(beforeLang);
  });

  it('set() works for string fields', () => {
    const { result } = renderHook(() => useSettingsField('ui_language'));
    act(() => {
      result.current.set('fr-FR');
    });
    expect(useSettingsStore.getState().settings.ui_language).toBe('fr-FR');
  });

  it('set() works for numeric fields', () => {
    const { result } = renderHook(() => useSettingsField('update_check_interval_hours'));
    act(() => {
      result.current.set(24);
    });
    expect(useSettingsStore.getState().settings.update_check_interval_hours).toBe(24);
  });

  it('set() works for union-typed fields (colour_blind_mode)', () => {
    const { result } = renderHook(() => useSettingsField('colour_blind_mode'));
    act(() => {
      result.current.set('deuteranopia');
    });
    expect(useSettingsStore.getState().settings.colour_blind_mode).toBe('deuteranopia');
  });

  // ===========================================================================
  // Setter stability
  // ===========================================================================

  it('set is referentially stable across renders for the same key', () => {
    const { result, rerender } = renderHook(() => useSettingsField('acoustid_enabled'));
    const firstSet = result.current.set;
    rerender();
    expect(result.current.set).toBe(firstSet);
  });

  it('set is independent per key (different hooks return different setters)', () => {
    const { result: a } = renderHook(() => useSettingsField('acoustid_enabled'));
    const { result: b } = renderHook(() => useSettingsField('ui_language'));
    expect(a.current.set).not.toBe(b.current.set);
  });
});
