/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/layout/MainLayout.test.tsx - Skip-link focus test (a11y audit)
 *
 * Before the fix, activating the "Skip to main content" link moved the
 * page's scroll position (because it's a plain `#main-content` fragment
 * link) but did NOT move actual keyboard focus, because `<main>` was not
 * a focusable element. A keyboard-only user who used the link to jump
 * past the sidebar would then press Tab and land back wherever focus
 * already was -- not inside the content they'd just "skipped" to. This
 * test checks the one thing that actually matters: that
 * `document.activeElement` really is the main content region after the
 * link is activated.
 *
 * @see src/components/layout/MainLayout.tsx - The component under test
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MainLayout } from '@/components/layout/MainLayout';
import { useDownloadStore } from '@/stores/downloadStore';
import { useUiStore } from '@/stores/uiStore';

/**
 * Finds the element that MainLayout's drag-and-drop handlers are
 * actually attached to.
 *
 * That element has no test id of its own -- it is the content column
 * that wraps `<main id="main-content">`, `UpdateBanner`, `StatusBar`,
 * and so on (see the JSX in `MainLayout.tsx`). Rather than adding a
 * test-only attribute to production markup, this reuses the same
 * `#main-content` anchor the skip-link tests above already rely on:
 * `<main>` is always a direct child of the div carrying `onDrop`.
 */
function getDropZone(): HTMLElement {
  const main = document.getElementById('main-content');
  if (!main || !main.parentElement) {
    throw new Error('Could not find the drop zone (main-content has no parent)');
  }
  return main.parentElement;
}

/** Builds a `.meedyadl` File object from a plain JS value, the same way
 * a file dropped from Finder/Explorer would arrive as JSON text. */
function meedyadlFile(contents: unknown, name = 'test.meedyadl'): File {
  return new File([JSON.stringify(contents)], name, { type: 'application/json' });
}

describe('MainLayout skip link (a11y audit)', () => {
  it('moves real keyboard focus to the main content region, not just the page scroll position', () => {
    render(
      <MainLayout>
        <div>Page content</div>
      </MainLayout>
    );

    const skipLink = screen.getByText(/skip to main content/i);
    fireEvent.click(skipLink);

    const main = document.getElementById('main-content');
    expect(main).not.toBeNull();
    expect(document.activeElement).toBe(main);
  });

  it('the main content region is reachable via script focus (tabIndex=-1) but not part of the normal Tab order', () => {
    render(
      <MainLayout>
        <div>Page content</div>
      </MainLayout>
    );

    const main = document.getElementById('main-content');
    // tabIndex={-1}: focusable via .focus()/script, but explicitly
    // excluded from sequential (Tab-key) navigation.
    expect(main).toHaveAttribute('tabindex', '-1');
  });
});

/**
 * Dropping a `.meedyadl` manifest file onto the window (review fix,
 * "a dropped file is read with no checking at all").
 *
 * Before the fix, the drop handler did
 * `(manifest.sources ?? []).map((s) => s.url)` with no check of the
 * manifest's shape at all. This meant: a source entry with no `url`
 * property put the literal JavaScript word "undefined" into the URL
 * text box as if it were a real link; nothing checked that `sources`
 * was even an array; and there was no limit on how many lines a single
 * dropped file could add at once. These tests drop real `File` objects
 * onto the real rendered component and read back the two places the
 * fix is observable from outside: the URL text box
 * (`useDownloadStore.urlInput`) and the toast that explains what
 * happened (`useUiStore.toasts`).
 *
 * These do NOT re-implement `extractManifestUrls()`'s own logic --
 * they drive the real drop event through the real component and check
 * what it did, the same way a user dropping a file would experience it.
 */
describe('MainLayout .meedyadl drop handling (review fix)', () => {
  beforeEach(() => {
    useDownloadStore.setState({ urlInput: '' });
    // Start on a page other than Download so a successful import's
    // `setPage('download')` call is actually observable as a change.
    useUiStore.setState({ currentPage: 'settings', toasts: [] });
  });

  it('imports every URL from a well-formed manifest and switches to the Download page', async () => {
    render(
      <MainLayout>
        <div>Page content</div>
      </MainLayout>
    );

    const file = meedyadlFile({
      version: 1,
      app: 'MeedyaDL',
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
      sources: [
        { platform: 'apple-music', url: 'https://music.apple.com/us/album/foo/111', downloaded_at: '2026-01-01T00:00:00Z' },
        { platform: 'apple-music', url: 'https://music.apple.com/us/album/bar/222', downloaded_at: '2026-01-01T00:00:00Z' },
      ],
    });

    fireEvent.drop(getDropZone(), { dataTransfer: { files: [file] } });

    await waitFor(() => {
      expect(useDownloadStore.getState().urlInput).toBe(
        'https://music.apple.com/us/album/foo/111\nhttps://music.apple.com/us/album/bar/222'
      );
    });

    expect(useUiStore.getState().currentPage).toBe('download');
    expect(
      useUiStore.getState().toasts.some((t) => t.type === 'success' && /imported 2 urls/i.test(t.message))
    ).toBe(true);
  });

  it('rejects the whole file, and never writes the word "undefined", when one source entry has no url', async () => {
    render(
      <MainLayout>
        <div>Page content</div>
      </MainLayout>
    );

    // The second entry has no `url` field at all -- this is exactly the
    // shape that used to produce the literal text "undefined" in the
    // URL box via `s.url` on a missing property.
    const file = meedyadlFile({
      version: 1,
      sources: [
        { platform: 'apple-music', url: 'https://music.apple.com/us/album/foo/111' },
        { platform: 'apple-music' },
      ],
    });

    fireEvent.drop(getDropZone(), { dataTransfer: { files: [file] } });

    await waitFor(() => {
      expect(
        useUiStore.getState().toasts.some((t) => t.type === 'error' && /invalid .meedyadl manifest/i.test(t.message))
      ).toBe(true);
    });

    // Nothing was written to the URL box, and specifically the box never
    // contains the bare word "undefined".
    expect(useDownloadStore.getState().urlInput).toBe('');
    // The page must not have navigated away either -- an invalid drop
    // should change nothing else about the app's state.
    expect(useUiStore.getState().currentPage).toBe('settings');
  });

  it('rejects a file whose "sources" is missing or the wrong shape, the same as a manifest missing entirely', async () => {
    render(
      <MainLayout>
        <div>Page content</div>
      </MainLayout>
    );

    // No `sources` array and no `version` number at all -- not a
    // manifest, just some unrelated JSON file that happened to be
    // renamed to end in `.meedyadl`.
    const file = meedyadlFile({ hello: 'world' });

    fireEvent.drop(getDropZone(), { dataTransfer: { files: [file] } });

    await waitFor(() => {
      expect(
        useUiStore.getState().toasts.some((t) => t.type === 'error' && /invalid .meedyadl manifest/i.test(t.message))
      ).toBe(true);
    });
    expect(useDownloadStore.getState().urlInput).toBe('');
  });

  it('caps a manifest with more sources than the import limit, and says so in a warning toast', async () => {
    render(
      <MainLayout>
        <div>Page content</div>
      </MainLayout>
    );

    // One more than the 500-URL cap (see MAX_DROPPED_MANIFEST_URLS in
    // MainLayout.tsx) so the truncation path is actually exercised.
    const sources = Array.from({ length: 501 }, (_, i) => ({
      platform: 'apple-music',
      url: `https://music.apple.com/us/album/track-${i}/${i}`,
    }));
    const file = meedyadlFile({ version: 1, sources });

    fireEvent.drop(getDropZone(), { dataTransfer: { files: [file] } });

    await waitFor(() => {
      const urlInput = useDownloadStore.getState().urlInput;
      expect(urlInput.length).toBeGreaterThan(0);
    });

    const importedUrls = useDownloadStore.getState().urlInput.split('\n');
    expect(importedUrls).toHaveLength(500);
    // The first URL in the capped list should still be the first one
    // from the manifest -- truncation keeps the front of the list, it
    // doesn't reorder or sample it.
    expect(importedUrls[0]).toBe('https://music.apple.com/us/album/track-0/0');

    expect(
      useUiStore.getState().toasts.some((t) => t.type === 'warning' && /only imported the first 500/i.test(t.message))
    ).toBe(true);
  });
});
