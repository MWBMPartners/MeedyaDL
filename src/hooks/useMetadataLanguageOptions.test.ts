// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for useMetadataLanguageOptions (#1249).
 *
 * Pins the contract the Settings screen relies on:
 *   - the order the backend (shared `meedya-lang` code) sends back is used;
 *   - if the backend fails, or sends back a list with an entry lost or
 *     added, the alphabetical fallback is shown instead -- never an empty
 *     or broken list;
 *   - a saved value that is not offered (an old `zh-CN`) is in the list,
 *     named from its own tag, and STAYS in the list after somebody picks a
 *     different entry (policy UI-050: choosing must not change the menu);
 *   - the backend is asked with the interface language first among the
 *     preferences.
 */

import { renderHook, waitFor } from '@testing-library/react';

import * as commands from '@/lib/tauri-commands';
import { useMetadataLanguageOptions } from '@/hooks/useMetadataLanguageOptions';
import { METADATA_LANGUAGE_TAGS } from '@/lib/languageOptions';

vi.mock('@/lib/tauri-commands', () => ({
  orderLanguagesForDisplay: vi.fn(),
}));

const order = vi.mocked(commands.orderLanguagesForDisplay);

describe('useMetadataLanguageOptions', () => {
  beforeEach(() => {
    order.mockReset();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('uses the order the backend sends back', async () => {
    // A recognisable order: the offered tags reversed.
    order.mockImplementation(async (tags) => [...tags].reverse());
    const { result } = renderHook(() => useMetadataLanguageOptions('en-GB', 'en'));
    await waitFor(() =>
      expect(result.current.map((o) => o.value)).toEqual([...METADATA_LANGUAGE_TAGS].reverse()),
    );
    const [, preferences] = order.mock.calls[0];
    expect(preferences[0]).toBe('en');
  });

  it('shows the alphabetical list when the backend fails', async () => {
    order.mockRejectedValue(new Error('command not available'));
    const { result } = renderHook(() => useMetadataLanguageOptions('en-GB', 'en'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    const labels = result.current.map((o) => o.label);
    expect(labels).toHaveLength(METADATA_LANGUAGE_TAGS.length);
    expect(labels).toEqual([...labels].sort((a, b) => new Intl.Collator('en').compare(a, b)));
  });

  it('ignores a backend answer that lost an entry', async () => {
    order.mockImplementation(async (tags) => tags.slice(1));
    const { result } = renderHook(() => useMetadataLanguageOptions('en-GB', 'en'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length);
  });

  it('keeps an old saved value in the list, named from its tag, after another is picked', async () => {
    order.mockImplementation(async (tags) => [...tags]);
    const { result, rerender } = renderHook(
      ({ value }) => useMetadataLanguageOptions(value, 'en'),
      { initialProps: { value: 'zh-CN' } },
    );
    const oldEntry = () => result.current.find((o) => o.value === 'zh-CN');
    expect(oldEntry()?.label).toBe('Chinese (China)');

    // The person picks a different entry: the old one must still be there.
    rerender({ value: 'zh-Hant-TW' });
    await waitFor(() => expect(order).toHaveBeenCalled());
    expect(oldEntry()).toBeDefined();
    expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length + 1);
  });
});

describe('useMetadataLanguageOptions when the IPC call throws before returning a promise', () => {
  it('still shows the whole list, alphabetically', async () => {
    order.mockReset();
    order.mockImplementation(() => {
      throw new Error('IPC bridge missing');
    });
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    const { result } = renderHook(() => useMetadataLanguageOptions('en-GB', 'en'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length);
    vi.restoreAllMocks();
  });
});
