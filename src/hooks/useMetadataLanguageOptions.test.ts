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

import { act, renderHook, waitFor } from '@testing-library/react';

import * as commands from '@/lib/tauri-commands';
import { useMetadataLanguageOptions } from '@/hooks/useMetadataLanguageOptions';
import { IDENTITY_RETRY_DELAYS_MS } from '@/hooks/useLanguageIdentities';
import { METADATA_LANGUAGE_TAGS } from '@/lib/languageOptions';
import { useSettingsStore } from '@/stores/settingsStore';

vi.mock('@/lib/tauri-commands', () => ({
  orderLanguagesForDisplay: vi.fn(),
  // Default: "nothing to add" -- every tag then uses the browser's reading.
  languageIdentities: vi.fn(async () => []),
}));

const order = vi.mocked(commands.orderLanguagesForDisplay);
const identities = vi.mocked(commands.languageIdentities);

/**
 * Answers the way the real backend does (`utils::language::language_identity`
 * in `src-tauri/src/utils/language.rs`, whose own tests pin these): Mandarin
 * keeps its own code, and an ordinary tag is put into its standard letter
 * case (`EN-us` -> `en-US`), grouped by its primary language. For the plain
 * tags these tests use, that standard form is also what `Intl.Locale` gives.
 */
const backendLike = async (tags: readonly string[]) =>
  tags.map((raw) => {
    if (raw === 'cmn-Hans-CN') return { raw, standard: raw, primary: 'cmn' };
    try {
      const locale = new Intl.Locale(raw);
      return { raw, standard: locale.toString(), primary: locale.language };
    } catch {
      return { raw, standard: raw, primary: raw.toLowerCase() };
    }
  });

/** Lets every timer due within `ms` fire, and the promises they start settle. */
async function wait(ms: number): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

describe('useMetadataLanguageOptions', () => {
  beforeEach(() => {
    order.mockReset();
    identities.mockReset();
    identities.mockImplementation(async () => []);
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    // The values seen live in the (shared) settings store; start each test
    // from a fresh app run.
    useSettingsStore.setState({ seenMetadataLanguages: [] });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('uses the order the backend sends back', async () => {
    // A recognisable order: the offered tags reversed.
    order.mockImplementation(async (tags) => [...tags].reverse());
    const { result } = renderHook(() => useMetadataLanguageOptions('en-GB', 'en'));
    await waitFor(() =>
      expect(result.current.map((o) => o.value)).toEqual([...METADATA_LANGUAGE_TAGS].reverse())
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
      { initialProps: { value: 'zh-CN' } }
    );
    const oldEntry = () => result.current.find((o) => o.value === 'zh-CN');
    expect(oldEntry()?.label).toBe('Chinese (China)');

    // The person picks a different entry: the old one must still be there.
    rerender({ value: 'zh-Hant-TW' });
    await waitFor(() => expect(order).toHaveBeenCalled());
    expect(oldEntry()).toBeDefined();
    expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length + 1);
  });

  it('still lists the old value after another is picked and the Settings tab is left and reopened', async () => {
    // Independent review of #1249: the list is unmounted on a tab switch,
    // which used to throw the remembered value away.
    order.mockImplementation(async (tags) => [...tags]);
    const first = renderHook(({ value }) => useMetadataLanguageOptions(value, 'en'), {
      initialProps: { value: 'zh-CN' },
    });
    first.rerender({ value: 'zh-Hant-TW' });
    await waitFor(() => expect(order).toHaveBeenCalled());
    first.unmount(); // the person switches to another Settings tab

    // ...and comes back: a brand-new list, with the new value selected.
    const second = renderHook(() => useMetadataLanguageOptions('zh-Hant-TW', 'en'));
    expect(second.result.current.find((o) => o.value === 'zh-CN')).toBeDefined();
    expect(second.result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length + 1);
  });

  it('keeps a saved value with an embedded NUL character intact, not split in two', async () => {
    // The remembered-values list used to be joined into one string with
    // U+0000 (NUL) as the separator, on the assumption that no language
    // tag or settings value would ever contain one -- an assumption
    // nothing actually enforced (an imported settings file only has its
    // \n and \r stripped). A value that genuinely contained a NUL
    // character would have been split apart into two values the next
    // time this list was rebuilt. JSON.stringify/JSON.parse round-trip
    // a string exactly, whatever characters it holds (independent
    // review, round 3 of #1244).
    order.mockImplementation(async (tags) => [...tags]);
    const withNul = 'zh-CN\u0000not-a-separate-value';
    const { result } = renderHook(() => useMetadataLanguageOptions(withNul, 'en'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    const values = result.current.map((o) => o.value);
    expect(values).toContain(withNul);
    expect(values).not.toContain('zh-CN');
    expect(values).not.toContain('not-a-separate-value');
    expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length + 1);
  });

  it('keeps the same order when a different entry is chosen', async () => {
    // Choosing must not move anything (policy UI-050): the list is keyed on
    // its content, so a new choice does not even ask for a new order.
    order.mockImplementation(async (tags) => [...tags].reverse());
    const { result, rerender } = renderHook(
      ({ value }) => useMetadataLanguageOptions(value, 'en'),
      { initialProps: { value: 'en-GB' } }
    );
    await waitFor(() => expect(result.current[0].value).toBe(METADATA_LANGUAGE_TAGS.at(-1)));
    // Two asks settle: one with the browser's grouping, one once the
    // backend's identities arrive (Codex's catch-up review).
    await waitFor(() => expect(order).toHaveBeenCalledTimes(2));
    const callsBefore = order.mock.calls.length;
    const before = result.current.map((o) => o.value);
    rerender({ value: 'ja-JP' });
    expect(result.current.map((o) => o.value)).toEqual(before);
    // What this test is about: choosing an entry asks for nothing new.
    expect(order).toHaveBeenCalledTimes(callsBefore);
  });

  // -- One row per language (Codex's catch-up review of #1244, finding 2) --

  it('a stored "EN-us" is the offered "en-US": one row, not two, once the backend says so', async () => {
    identities.mockImplementation(backendLike);
    order.mockImplementation(async (tags) => [...tags]);
    const { result } = renderHook(() => useMetadataLanguageOptions('EN-us', 'en'));
    await waitFor(() => expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length));
    const rows = result.current.filter((o) => o.value === 'en-US' || o.value === 'EN-us');
    expect(rows.map((o) => o.value)).toEqual(['en-US']);
  });

  it('a remembered "en-us" does not duplicate the offered "en-US"', async () => {
    useSettingsStore.setState({ seenMetadataLanguages: ['en-us'] });
    identities.mockImplementation(backendLike);
    order.mockImplementation(async (tags) => [...tags]);
    const { result } = renderHook(() => useMetadataLanguageOptions('ja-JP', 'en'));
    await waitFor(() => expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length));
  });

  it("uses the backend's identity, not only the browser's reading, once it answers", async () => {
    // Artificial: the backend says `cmn-Hans` standardises to the offered
    // `ja-JP`, which `Intl` would never conclude.
    identities.mockImplementation(async (tags) =>
      tags.map((raw) => ({
        raw,
        standard: raw === 'cmn-Hans' ? 'ja-JP' : raw,
        primary: raw.toLowerCase(),
      }))
    );
    order.mockImplementation(async (tags) => [...tags]);
    const { result } = renderHook(() => useMetadataLanguageOptions('cmn-Hans', 'en'));
    await waitFor(() => expect(identities).toHaveBeenCalled());
    await waitFor(() => expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length));
    expect(result.current.some((o) => o.value === 'cmn-Hans')).toBe(false);
  });

  it('sends the backend identity of mandarin and cantonese as groups of their own', async () => {
    // With an English interface: the groups handed to the ordering are the
    // backend's (`cmn`, `yue`, `nan`, `zh`), so none is missing.
    const backend: Record<string, string> = {
      'cmn-Hans': 'cmn',
      'zh-cmn-Hans': 'cmn',
      yue: 'yue',
      'zh-Hant': 'zh',
      nan: 'nan',
    };
    identities.mockImplementation(async (tags) =>
      tags.map((raw) => ({ raw, standard: raw, primary: backend[raw] ?? 'x' + raw }))
    );
    useSettingsStore.setState({ seenMetadataLanguages: Object.keys(backend) });
    order.mockImplementation(async (tags) => [...tags]);
    renderHook(() => useMetadataLanguageOptions('en-US', 'en'));
    await waitFor(() => {
      const last = order.mock.calls.at(-1);
      expect(last?.[2]).toEqual(expect.arrayContaining(['cmn', 'yue', 'nan', 'zh']));
    });
  });
});

// -- Unverified identities never merge or remove a row (Codex's review of
// -- round 5, finding 2). The browser reads `cmn-Hans-CN` as `zh-Hans-CN`
// -- (the offered Chinese row); the policy keeps Mandarin distinct. Until
// -- the backend has said which language a tag is, every entry keeps a
// -- row of its own.

describe('useMetadataLanguageOptions while the backend has not identified the tags', () => {
  const MANDARIN = 'cmn-Hans-CN';
  const values = (options: { value: string }[]) => options.map((o) => o.value);

  beforeEach(() => {
    order.mockReset();
    order.mockImplementation(async (tags) => [...tags]);
    identities.mockReset();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    useSettingsStore.setState({ seenMetadataLanguages: [] });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('keeps a stored Mandarin row while the answer is pending', async () => {
    identities.mockImplementation(() => new Promise(() => {}));
    const { result, rerender } = renderHook(() => useMetadataLanguageOptions(MANDARIN, 'en'));
    expect(values(result.current)).toContain(MANDARIN);
    expect(values(result.current)).toContain('zh-Hans-CN');
    await waitFor(() => expect(order).toHaveBeenCalled());
    rerender();
    expect(values(result.current)).toContain(MANDARIN);
    expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length + 1);
  });

  it('keeps a stored Mandarin row when every request fails, across rerenders', async () => {
    vi.useFakeTimers();
    identities.mockRejectedValue(new Error('backend unavailable'));
    const { result, rerender } = renderHook(() => useMetadataLanguageOptions(MANDARIN, 'en'));
    await wait(60_000);
    // Asked once, then retried a bounded number of times, then left alone.
    expect(identities).toHaveBeenCalledTimes(1 + IDENTITY_RETRY_DELAYS_MS.length);
    rerender();
    await wait(60_000);
    rerender();
    expect(identities).toHaveBeenCalledTimes(1 + IDENTITY_RETRY_DELAYS_MS.length);
    expect(values(result.current)).toContain(MANDARIN);
    expect(values(result.current)).toContain('zh-Hans-CN');
    expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length + 1);
  });

  it('a later success keeps Mandarin as its own group', async () => {
    vi.useFakeTimers();
    identities.mockRejectedValueOnce(new Error('not yet')).mockImplementation(backendLike);
    const { result, rerender } = renderHook(() => useMetadataLanguageOptions(MANDARIN, 'en'));
    await wait(0);
    expect(values(result.current)).toContain(MANDARIN);
    await wait(IDENTITY_RETRY_DELAYS_MS[0]);
    expect(identities).toHaveBeenCalledTimes(2);
    rerender();
    expect(values(result.current)).toContain(MANDARIN);
    expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length + 1);
    // The groups handed to the ordering are the backend's: Mandarin has a
    // group of its own, beside (not inside) Chinese.
    const [, , groups] = order.mock.calls.at(-1)!;
    expect(groups).toEqual(expect.arrayContaining(['cmn', 'zh']));
  });
});

describe('useMetadataLanguageOptions when the IPC call throws before returning a promise', () => {
  it('still shows the whole list, alphabetically', async () => {
    order.mockReset();
    order.mockImplementation(() => {
      throw new Error('IPC bridge missing');
    });
    identities.mockReset();
    identities.mockImplementation(async () => []);
    // The store is process-wide: a test above may have left values in it.
    useSettingsStore.setState({ seenMetadataLanguages: [] });
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    const { result } = renderHook(() => useMetadataLanguageOptions('en-GB', 'en'));
    await waitFor(() => expect(order).toHaveBeenCalled());
    expect(result.current).toHaveLength(METADATA_LANGUAGE_TAGS.length);
    vi.restoreAllMocks();
  });
});
