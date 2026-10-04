// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for useLanguageIdentities (Codex's catch-up review of
 * #1244, finding 1): the browser's reading until the backend answers, the
 * backend's answer then replacing it, and the fallback kept on failure.
 */

import { renderHook, waitFor } from '@testing-library/react';

import * as commands from '@/lib/tauri-commands';
import { useLanguageIdentities } from '@/hooks/useLanguageIdentities';

vi.mock('@/lib/tauri-commands', () => ({ languageIdentities: vi.fn() }));
const ask = vi.mocked(commands.languageIdentities);

describe('useLanguageIdentities', () => {
  beforeEach(() => {
    ask.mockReset();
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });
  afterEach(() => vi.restoreAllMocks());

  it('uses the browser reading until the backend answers', () => {
    ask.mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useLanguageIdentities(['cmn-Hans', 'en-us']));
    expect(result.current.ready).toBe(false);
    expect(result.current.primaryOf('cmn-Hans')).toBe('zh'); // wrong, hence a fallback
    expect(result.current.standardOf('en-us')).toBe('en-US');
  });

  it("the backend's answer replaces the fallback", async () => {
    ask.mockImplementation(async (tags) =>
      tags.map((raw) => ({ raw, standard: raw, primary: 'cmn' }))
    );
    const { result } = renderHook(() => useLanguageIdentities(['cmn-Hans']));
    await waitFor(() => expect(result.current.ready).toBe(true));
    expect(result.current.primaryOf('cmn-Hans')).toBe('cmn');
  });

  it('keeps the fallback when the call rejects or throws', async () => {
    ask.mockRejectedValueOnce(new Error('no'));
    const a = renderHook(() => useLanguageIdentities(['fr-FR']));
    await waitFor(() => expect(ask).toHaveBeenCalled());
    expect(a.result.current.ready).toBe(false);
    expect(a.result.current.primaryOf('fr-FR')).toBe('fr');

    ask.mockImplementationOnce(() => {
      throw new Error('no bridge');
    });
    const b = renderHook(() => useLanguageIdentities(['de-DE']));
    await waitFor(() => expect(ask).toHaveBeenCalledTimes(2));
    expect(b.result.current.primaryOf('de-DE')).toBe('de');
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
