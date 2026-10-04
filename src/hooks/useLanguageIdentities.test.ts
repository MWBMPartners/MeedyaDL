// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for useLanguageIdentities (Codex's catch-up review of
 * #1244, finding 1, and Codex's review of round 5, finding 2).
 *
 * What is pinned here:
 *   - until the backend answers, and if it never does, a tag's standard
 *     form is the tag ITSELF, so no two spellings are ever merged on the
 *     browser's say-so (the browser reads `cmn-Hans-CN` as `zh-Hans-CN`,
 *     which the shared policy does not);
 *   - the browser's reading is still used for the GROUP while waiting, as
 *     an ordering hint only (it never removes an entry);
 *   - a failed request is tried again a bounded number of times, with a
 *     growing delay, and a success on a retry replaces the raw reading;
 *   - an answer, or a retry, for a list that has since changed is dropped.
 */

import { act, renderHook, waitFor } from '@testing-library/react';

import * as commands from '@/lib/tauri-commands';
import { IDENTITY_RETRY_DELAYS_MS, useLanguageIdentities } from '@/hooks/useLanguageIdentities';

vi.mock('@/lib/tauri-commands', () => ({ languageIdentities: vi.fn() }));
const ask = vi.mocked(commands.languageIdentities);

/** The answers the real backend gives for the tags used here (see the
 * `language_identity` tests in `src-tauri/src/utils/language.rs`). */
const BACKEND: Record<string, { standard: string; primary: string }> = {
  'cmn-Hans-CN': { standard: 'cmn-Hans-CN', primary: 'cmn' },
  'EN-us': { standard: 'en-US', primary: 'en' },
  'en-US': { standard: 'en-US', primary: 'en' },
};
const backendAnswer = async (tags: readonly string[]) =>
  tags.map((raw) => ({ raw, ...(BACKEND[raw] ?? { standard: raw, primary: raw }) }));

/** Lets every timer due within `ms` fire, and the promises they start settle. */
async function wait(ms: number): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

describe('useLanguageIdentities', () => {
  beforeEach(() => {
    ask.mockReset();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('never merges two spellings while the answer is pending', () => {
    ask.mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() =>
      useLanguageIdentities(['cmn-Hans-CN', 'EN-us', 'cmn-Hans'])
    );
    expect(result.current.ready).toBe(false);
    // The tag itself, not the browser's `zh-Hans-CN` / `en-US`.
    expect(result.current.standardOf('cmn-Hans-CN')).toBe('cmn-Hans-CN');
    expect(result.current.standardOf('EN-us')).toBe('EN-us');
    // The group is still the browser's reading: an ordering hint only.
    expect(result.current.primaryOf('cmn-Hans')).toBe('zh');
  });

  it("the backend's answer replaces the raw reading", async () => {
    ask.mockImplementation(backendAnswer);
    const { result } = renderHook(() => useLanguageIdentities(['cmn-Hans-CN', 'EN-us']));
    await waitFor(() => expect(result.current.ready).toBe(true));
    expect(result.current.primaryOf('cmn-Hans-CN')).toBe('cmn');
    expect(result.current.standardOf('EN-us')).toBe('en-US');
  });

  it('a tag the answer leaves out keeps its own spelling', async () => {
    ask.mockImplementation(async () => []);
    const { result } = renderHook(() => useLanguageIdentities(['EN-us']));
    await waitFor(() => expect(result.current.ready).toBe(true));
    expect(result.current.standardOf('EN-us')).toBe('EN-us');
  });

  it('retries a failed request three times with a growing delay, then keeps each tag as itself', async () => {
    vi.useFakeTimers();
    ask.mockRejectedValue(new Error('no'));
    const { result, rerender } = renderHook(() => useLanguageIdentities(['cmn-Hans-CN']));
    await wait(0);
    expect(ask).toHaveBeenCalledTimes(1);

    const [first, second, third] = IDENTITY_RETRY_DELAYS_MS;
    expect(first).toBeLessThan(second);
    expect(second).toBeLessThan(third);
    await wait(first - 1);
    expect(ask).toHaveBeenCalledTimes(1);
    await wait(1);
    expect(ask).toHaveBeenCalledTimes(2);
    await wait(second);
    expect(ask).toHaveBeenCalledTimes(3);
    await wait(third);
    expect(ask).toHaveBeenCalledTimes(4);

    // Then it stops: no endless asking, and the raw reading stays.
    await wait(60_000);
    rerender();
    await wait(60_000);
    expect(ask).toHaveBeenCalledTimes(1 + IDENTITY_RETRY_DELAYS_MS.length);
    expect(result.current.ready).toBe(false);
    expect(result.current.standardOf('cmn-Hans-CN')).toBe('cmn-Hans-CN');
  });

  it('a success on a retry replaces the raw reading', async () => {
    vi.useFakeTimers();
    ask.mockRejectedValueOnce(new Error('not yet')).mockImplementation(backendAnswer);
    const { result } = renderHook(() => useLanguageIdentities(['EN-us']));
    await wait(0);
    expect(result.current.standardOf('EN-us')).toBe('EN-us');
    await wait(IDENTITY_RETRY_DELAYS_MS[0]);
    expect(ask).toHaveBeenCalledTimes(2);
    expect(result.current.ready).toBe(true);
    expect(result.current.standardOf('EN-us')).toBe('en-US');
  });

  it('a call that throws before returning a promise is retried the same way', async () => {
    vi.useFakeTimers();
    ask
      .mockImplementationOnce(() => {
        throw new Error('no bridge');
      })
      .mockImplementation(backendAnswer);
    const { result } = renderHook(() => useLanguageIdentities(['EN-us']));
    await wait(0);
    expect(result.current.ready).toBe(false);
    expect(result.current.primaryOf('EN-us')).toBe('en');
    await wait(IDENTITY_RETRY_DELAYS_MS[0]);
    expect(result.current.standardOf('EN-us')).toBe('en-US');
  });

  it('drops an answer and a pending retry that belong to a list that has since changed', async () => {
    vi.useFakeTimers();
    ask.mockImplementation(async (tags) => {
      if (tags[0] === 'old') throw new Error('no');
      return backendAnswer(tags);
    });
    const { result, rerender } = renderHook(({ tags }) => useLanguageIdentities(tags), {
      initialProps: { tags: ['old'] },
    });
    await wait(0);
    rerender({ tags: ['EN-us'] });
    await wait(60_000);
    // One ask for the old list, one for the new; the old list's retry was
    // cancelled when the list changed.
    expect(ask.mock.calls.map(([tags]) => tags[0])).toEqual(['old', 'EN-us']);
    expect(result.current.standardOf('EN-us')).toBe('en-US');
  });

  it('asks again only when the list content changes', async () => {
    ask.mockImplementation(async (tags) =>
      tags.map((raw) => ({ raw, standard: raw, primary: raw }))
    );
    const { rerender } = renderHook(({ tags }) => useLanguageIdentities(tags), {
      initialProps: { tags: ['en'] },
    });
    await waitFor(() => expect(ask).toHaveBeenCalledTimes(1));
    rerender({ tags: ['en'] });
    rerender({ tags: ['de'] });
    await waitFor(() => expect(ask).toHaveBeenCalledTimes(2));
  });
});
