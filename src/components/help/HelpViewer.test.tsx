/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/help/HelpViewer.test.tsx - Unit tests for HelpViewer
 *
 * Pins the contract that matters most now that Help content comes from
 * real `help/*.md` files instead of a second, hand-typed copy:
 *   - Every page listed in the manifest is reachable from the sidebar.
 *   - Search actually narrows what's shown, and can be cleared again.
 *   - A "?" help button elsewhere in the app (a deep link into
 *     `helpActiveTopic`) opens the right page and cleans up after itself.
 *   - Above all: a link from one help page to another opens INSIDE the
 *     app instead of trying to navigate the WebView to a `.md` URL that
 *     doesn't exist. That used to blank the entire app window with no
 *     way back except restarting it -- this is the test that would have
 *     caught it, and the one most worth keeping green.
 *   - An ordinary `https://` link still goes to the user's real browser,
 *     so the in-app-navigation fix above didn't accidentally swallow
 *     every link on the page.
 *
 * @see src/components/help/HelpViewer.tsx - The component under test
 * @see src/components/help/helpTopics.ts - Where the real page content comes from
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { HelpViewer } from './HelpViewer';
import { HELP_TOPIC_MANIFEST } from './helpTopics';
import { useUiStore } from '@/stores/uiStore';

// Test-only helper that switches the shared i18next instance to German or
// French without a real network fetch -- see src/testing/i18n.ts. Used by
// the "HelpViewer translated pages" tests below.
import { useTestLanguage } from '@/testing/i18n';

/**
 * Capture what the shell plugin would have opened, same pattern as
 * ErrorMessageDisplay.test.tsx. This is how test 6 proves an external
 * link left the app, and how test 5 proves an internal one did NOT.
 */
const openMock = vi.fn().mockResolvedValue(undefined);
vi.mock('@tauri-apps/plugin-shell', () => ({
  open: (url: string) => openMock(url),
}));

/**
 * `@tauri-apps/api/app`'s `getVersion()` is not mocked in the shared
 * `src/test/setup.ts` (only `api/core`, `api/event`, and `plugin-os`
 * are) -- see that file for the full list. HelpViewer calls it
 * unconditionally on mount to populate the About page's version line,
 * so every test in this file needs it resolved rather than hanging.
 */
const getVersionMock = vi.fn().mockResolvedValue('1.13.0-test.1');
vi.mock('@tauri-apps/api/app', () => ({
  getVersion: () => getVersionMock(),
}));

beforeEach(() => {
  openMock.mockClear();
  getVersionMock.mockClear();
  // The deep-link field is the one piece of uiStore state HelpViewer
  // reads. Reset it so a leftover value from one test can't decide
  // which page another test opens on.
  useUiStore.setState({ helpActiveTopic: null });
});

describe('HelpViewer sidebar', () => {
  it('shows a sidebar button for every page in the manifest, and opens on Getting Started', () => {
    render(<HelpViewer />);

    for (const topic of HELP_TOPIC_MANIFEST) {
      expect(screen.getByRole('button', { name: topic.label })).toBeInTheDocument();
    }

    // getting-started.md's own "# Getting Started" heading should
    // already be on screen with no click needed -- this is what a user
    // opening the Help page for the first time actually sees.
    expect(
      screen.getByRole('heading', { level: 1, name: 'Getting Started' })
    ).toBeInTheDocument();
  });

  it('shows the clicked page and hides the previous one', () => {
    render(<HelpViewer />);

    fireEvent.click(screen.getByRole('button', { name: 'Cookies' }));

    expect(
      screen.getByRole('heading', { level: 1, name: 'Cookie Management' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { level: 1, name: 'Getting Started' })
    ).not.toBeInTheDocument();
  });
});

describe('HelpViewer search', () => {
  /**
   * "Lyricsfile" was picked by actually checking `help/*.md` (not
   * guessed): it is real body text on the Lyrics & Metadata page and
   * does not appear -- as a whole word or inside another word -- in any
   * other page's title or body, so narrowing to it proves the search is
   * reading page content, not just matching against every page by luck.
   */
  it('narrows the sidebar to pages containing the search text, and the clear button restores the full list', () => {
    render(<HelpViewer />);

    fireEvent.change(screen.getByLabelText('Search help topics'), {
      target: { value: 'Lyricsfile' },
    });

    expect(screen.getByRole('button', { name: 'Lyrics & Metadata' })).toBeInTheDocument();
    for (const topic of HELP_TOPIC_MANIFEST) {
      if (topic.id === 'lyrics-and-metadata') continue;
      expect(screen.queryByRole('button', { name: topic.label })).not.toBeInTheDocument();
    }

    fireEvent.click(screen.getByRole('button', { name: 'Clear search' }));

    expect(screen.getByLabelText('Search help topics')).toHaveValue('');
    for (const topic of HELP_TOPIC_MANIFEST) {
      expect(screen.getByRole('button', { name: topic.label })).toBeInTheDocument();
    }
  });
});

describe('HelpViewer deep links', () => {
  /**
   * A `<HelpButton>` elsewhere in the app (e.g. next to a setting) sets
   * `helpActiveTopic` on the store and switches to the Help page. This
   * is what HelpViewer's own side of that contract looks like: pick up
   * the requested topic on mount, then clear the flag so a later visit
   * to the Help page doesn't get silently redirected again.
   */
  it('opens the topic named by helpActiveTopic on mount, then clears the deep-link flag', async () => {
    useUiStore.setState({ helpActiveTopic: 'cookie-management' });

    render(<HelpViewer />);

    await waitFor(() => {
      expect(
        screen.getByRole('heading', { level: 1, name: 'Cookie Management' })
      ).toBeInTheDocument();
    });
    expect(useUiStore.getState().helpActiveTopic).toBeNull();
  });
});

describe('HelpViewer link handling inside rendered Markdown', () => {
  /**
   * This is the bug the whole rewrite exists to prevent: the "Audio
   * Codecs" page links to "Wrapper Authentication" (`wrapper.md`).
   * Before HelpViewer intercepted these links, clicking one tried to
   * navigate the entire Tauri WebView to a `help/wrapper.md` URL that
   * resolves to nothing -- the app itself would disappear behind a
   * blank window. Proving the click switches the in-app page AND never
   * reaches the "open in the user's browser" path is the most important
   * assertion in this file.
   */
  it('opens a link to another help page inside the app, without calling the browser-open function', () => {
    render(<HelpViewer />);
    fireEvent.click(screen.getByRole('button', { name: 'Audio Codecs' }));
    expect(
      screen.getByRole('heading', { level: 1, name: 'Audio Codecs' })
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('link', { name: 'Wrapper Authentication' }));

    expect(
      screen.getByRole('heading', { level: 1, name: 'Wrapper authentication' })
    ).toBeInTheDocument();
    expect(openMock).not.toHaveBeenCalled();
  });

  /**
   * The flip side of the test above: an ordinary `https://` link (the
   * Wrapper page links out to the upstream wrapper-v2 project on
   * GitHub) must still leave the app and go to the user's real browser,
   * and must NOT be swallowed by the same handler that now catches
   * internal `.md` links.
   */
  it('sends an ordinary web link to the system browser and stays on the same page', async () => {
    render(<HelpViewer />);
    fireEvent.click(screen.getByRole('button', { name: 'Wrapper' }));
    expect(
      screen.getByRole('heading', { level: 1, name: 'Wrapper authentication' })
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('link', { name: 'wrapper-v2' }));

    // The click handler reaches the shell plugin through a dynamic
    // `import()`, which resolves a tick later than the click itself --
    // waitFor gives that microtask a chance to run before we check it.
    await waitFor(() =>
      expect(openMock).toHaveBeenCalledWith('https://github.com/glomatico/wrapper-v2')
    );
    // Still on the same page -- an external link must never change activeTopic.
    expect(
      screen.getByRole('heading', { level: 1, name: 'Wrapper authentication' })
    ).toBeInTheDocument();
  });
});

describe('HelpViewer translated pages (#111)', () => {
  /**
   * `useTestLanguage` changes the shared i18next instance for the whole
   * test run, not just one test (see that helper's own doc comment) --
   * every test in this block must switch back to English afterwards so
   * it doesn't leak into whichever test file runs next.
   */
  afterEach(async () => {
    await useTestLanguage('en');
  });

  /**
   * "Keyboard Shortcuts" is the one page that has a real German
   * translation today (help/de/keyboard-shortcuts.md). Once the app is
   * displaying German, opening that page should show the German text
   * -- not the English original -- with a note above it saying plainly
   * that the translation was made by a machine.
   */
  it('shows the German page and the "translated by a machine" note when the app is displaying German', async () => {
    await useTestLanguage('de');
    render(<HelpViewer />);

    fireEvent.click(screen.getByRole('button', { name: 'Keyboard Shortcuts' }));

    await waitFor(() => {
      expect(
        screen.getByRole('heading', { level: 1, name: 'Tastaturkürzel' })
      ).toBeInTheDocument();
    });
    expect(screen.getByText(/wurde maschinell übersetzt/)).toBeInTheDocument();
  });

  /**
   * "Getting Started" -- the page HelpViewer opens on by default --
   * has no German translation yet. With the app displaying German, it
   * must still show the real (English) page rather than a blank screen
   * or a spinner, alongside a note that plainly says this particular
   * page isn't translated into German yet.
   */
  it('shows the English page and the "not translated yet" note for a page with no German version', async () => {
    await useTestLanguage('de');
    render(<HelpViewer />);

    expect(
      screen.getByRole('heading', { level: 1, name: 'Getting Started' })
    ).toBeInTheDocument();
    expect(screen.getByText(/noch nicht auf Deutsch übersetzt/)).toBeInTheDocument();
  });

  /**
   * Somebody reading the German pages sees German headings, so German
   * is what they will type into the search box. Searching only the
   * English text would answer "no results" for a heading that is
   * visible on screen, which reads as the search being broken rather
   * than as a missing translation.
   */
  it('finds a page by a heading that only exists in the German translation', async () => {
    await useTestLanguage('de');
    render(<HelpViewer />);

    // Wait for the German pages to arrive before searching, since the
    // search has nothing German to look at until they do.
    await waitFor(() => {
      expect(screen.getByText(/noch nicht auf Deutsch übersetzt/)).toBeInTheDocument();
    });

    // Find the box by its role, not by its English placeholder — the
    // placeholder is itself translated, which is the whole point here.
    fireEvent.change(screen.getByRole('textbox'), {
      target: { value: 'Tastaturkürzel' },
    });

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Keyboard Shortcuts' })).toBeInTheDocument();
    });
  });
});

describe('HelpViewer About page', () => {
  /**
   * The version number is the one line of the About page that can't
   * live in a Markdown file -- it's only known once the app is actually
   * running (see `buildAboutBuildSection` in helpTopics.ts). This test
   * proves that live value actually reaches the screen rather than
   * getting lost somewhere in the effect/state plumbing that fetches it.
   */
  it('shows the app version returned by getVersion() once About is opened', async () => {
    render(<HelpViewer />);

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'About' }));
    });

    await waitFor(() => {
      expect(screen.getByText(/v1\.13\.0-test\.1/)).toBeInTheDocument();
    });
  });
});
