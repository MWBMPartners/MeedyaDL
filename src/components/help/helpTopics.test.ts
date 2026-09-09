/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/help/helpTopics.test.ts - Unit tests for helpTopics.ts
 *
 * The whole point of `helpTopics.ts` is that the in-app Help screen and the
 * `help/*.md` files on GitHub can never quietly disagree again -- before
 * this rewrite they were two hand-typed copies of the same words, and
 * nobody was ever told when someone edited one and forgot the other. These
 * tests are the thing standing guard over that promise. Each one is written
 * around a specific way a person could accidentally reintroduce the old
 * drift, or break a link, while doing completely ordinary work like adding
 * a new help page.
 *
 * @see src/components/help/helpTopics.ts - The module under test
 * @see src/components/help/HelpViewer.tsx - The component that renders HELP_TOPICS
 */

import {
  HELP_TOPIC_MANIFEST,
  HELP_TOPICS,
  prepareHelpMarkdown,
  buildAboutBuildSection,
} from './helpTopics';

/**
 * Loads every Markdown file under `help/` directly, independently of
 * `helpTopics.ts`'s own loader. This is deliberate: if the test reused
 * `HELP_PAGE_FILES` (or just read `HELP_TOPICS`), a page whose file is
 * missing from the manifest -- or a manifest entry with no matching file
 * -- would be invisible to it, because the loader itself already throws
 * (or simply never lists the orphan file) before the test ever gets a
 * chance to compare the two lists. Reading the filesystem ourselves is
 * what lets this test catch "I added `help/new-page.md` and forgot to
 * add it to `HELP_TOPIC_MANIFEST`" -- a mistake the loader alone cannot
 * notice, because from the loader's point of view a file nobody asked
 * for simply doesn't exist.
 */
const RAW_HELP_FILES = import.meta.glob<string>('/help/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
});

/** Turns a glob key like "/help/getting-started.md" into "getting-started". */
function fileIdOf(globPath: string): string {
  const fileName = globPath.split('/').pop() ?? globPath;
  return fileName.replace(/\.md$/, '');
}

describe('help/*.md files vs. HELP_TOPIC_MANIFEST', () => {
  /**
   * If a real person adds `help/some-new-page.md` on GitHub and forgets
   * to add the matching line to `HELP_TOPIC_MANIFEST`, the new page is
   * fully written and reviewable in the repo but never shows up anywhere
   * in the app -- there's no sidebar entry, no search hit, nothing. The
   * opposite mistake (a manifest entry with no file) is worse: it makes
   * the whole app fail to start, because `helpTopics.ts` throws the
   * moment it tries to load a page that isn't there. Either way, this is
   * the test that would have caught it, by comparing the file list on
   * disk against the manifest list independently of each other.
   */
  it('has exactly one manifest entry for every help file, and one help file for every manifest entry', () => {
    const idsOnDisk = new Set(Object.keys(RAW_HELP_FILES).map(fileIdOf));
    const idsInManifest = new Set(HELP_TOPIC_MANIFEST.map((topic) => topic.id));

    // index.md is the one deliberate exception -- see the file-level
    // comment at the top of helpTopics.ts for why it has no in-app page.
    const expectedOnDisk = new Set([...idsInManifest, 'index']);

    expect([...idsOnDisk].sort()).toEqual([...expectedOnDisk].sort());
  });

  /**
   * Two manifest entries pointing at the same id would mean the second
   * one silently shadows the first in the sidebar -- whichever topic
   * comes first in `HELP_TOPICS.find()` calls wins, and the other one's
   * label is dead weight nobody would ever click into.
   */
  it('never lists the same page id twice in the manifest', () => {
    const ids = HELP_TOPIC_MANIFEST.map((topic) => topic.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  /**
   * If a page's Markdown doesn't open with a top-level `# ` heading, the
   * rendered page has no title -- a reader lands on it from the sidebar
   * or a search result and sees body text with nothing above it saying
   * what page they're on.
   */
  it('starts every loaded topic with a top-level heading', () => {
    for (const topic of HELP_TOPICS) {
      expect(topic.content.startsWith('# ')).toBe(true);
    }
  });
});

describe('prepared help content has nothing GitHub-only left in it', () => {
  /**
   * The licence-comment block every `help/*.md` file opens with is meant
   * for someone reading the raw file on GitHub, not for a user of the
   * app. If it survived into `topic.content`, the Help screen's search
   * box -- which searches the raw text of every page, not just what's
   * drawn on screen -- would "find" the word "copyright" in all twenty
   * pages at once, and the comment would never actually render (both
   * browsers and react-markdown swallow HTML comments), so nobody would
   * ever notice it was there except by hitting this false-positive search
   * result.
   */
  it('has no leftover HTML comment in any prepared topic', () => {
    for (const topic of HELP_TOPICS) {
      expect(topic.content).not.toMatch(/<!--[\s\S]*?-->/);
    }
  });

  /**
   * "Back to Help Index" only makes sense on GitHub, where `index.md` is
   * a real page to jump back to. Inside the app there is no `index.md`
   * page -- the sidebar already is the index -- so if this line survived
   * into a topic's content, clicking it would be a dead end: at best
   * nothing happens, at worst (before the link-safety fix this whole
   * change is part of) it could try to navigate the WebView to a URL
   * that doesn't resolve to anything and blank the app out.
   */
  it('has no leftover "Back to Help Index" footer line in any prepared topic', () => {
    for (const topic of HELP_TOPICS) {
      expect(topic.content).not.toMatch(/^\[Back to Help Index\]\(index\.md\)/m);
    }
  });

  /**
   * An emoji shortcode like `:tada:` is written for GitHub's Markdown
   * renderer, which turns it into 🎉. react-markdown does no such
   * conversion, so a shortcode that leaked through would show up to a
   * user as the literal text ":tada:" sitting in the middle of a
   * sentence. The regex is deliberately narrow (lower-case only, no
   * leading/trailing word character or colon) so it doesn't fire on
   * something like `MeedyaMeta:SpotifyUrl` or `com.apple.iTunes:isBinaural`
   * -- real tag names that appear throughout the metadata-mapping and
   * lyrics-and-metadata pages and happen to contain a colon. Fenced code
   * blocks are stripped before the check runs, in case a future help
   * page shows a literal `:something:` as a code sample -- that would be
   * a real piece of example text, not a stray shortcode, and shouldn't
   * fail this test.
   */
  it('has no leftover emoji shortcode in the prose of any prepared topic', () => {
    const shortcodeRe = /(?<![\w:]):[a-z][a-z0-9_+-]*:(?![\w:])/;
    for (const topic of HELP_TOPICS) {
      const withoutFencedCode = topic.content.replace(/```[\s\S]*?```/g, '');
      expect(withoutFencedCode).not.toMatch(shortcodeRe);
    }
  });
});

describe('cross-page help links resolve to real pages', () => {
  /**
   * This is the defect the whole HelpViewer rewrite exists to fix: a
   * relative link from one help page to another (e.g. `wrapper.md`) that
   * points at a page which doesn't exist, or was renamed and never
   * updated, used to navigate the entire Tauri WebView to a URL that
   * resolves to nothing -- the app disappears behind a blank window with
   * no way back except restarting it. `HelpViewer.tsx` now intercepts
   * these links and switches to the target topic instead of letting the
   * browser navigate, but that only works if the target topic actually
   * exists. This test is what makes a broken cross-page link a failing
   * test instead of something a user discovers by clicking it.
   */
  it('points every relative link from one help page to another at a page the app actually has', () => {
    const linkRe = /\]\((?:\.\/)?([a-z0-9-]+)\.md(?:#[^)]*)?\)/g;
    // Widened to Set<string> deliberately: `targetId` below comes out of
    // a regex match, which TypeScript can only ever type as `string` --
    // narrowing this Set to `HelpTopicId` would make the membership
    // check itself fail to compile, for the exact same reason the whole
    // point of this test is that a stray link CAN name an id that isn't
    // one of the real ones.
    const manifestIds = new Set<string>(HELP_TOPIC_MANIFEST.map((topic) => topic.id));

    for (const topic of HELP_TOPICS) {
      const matches = [...topic.content.matchAll(linkRe)];
      for (const match of matches) {
        const targetId = match[1];
        expect(
          manifestIds.has(targetId),
          `"${topic.id}.md" links to "${targetId}.md", which is not a real help page`
        ).toBe(true);
      }
    }
  });
});

describe('prepareHelpMarkdown', () => {
  it('strips the leading HTML licence-comment block', () => {
    const raw = '<!--\nCopyright (c) 2026 MeedyaSuite\n-->\n\n# Getting Started\n\nSome text.';
    expect(prepareHelpMarkdown(raw)).toBe('# Getting Started\n\nSome text.');
  });

  it('strips the "Back to Help Index" footer line and the horizontal rule left stranded above it', () => {
    const raw = '# Some Page\n\nBody text.\n\n---\n\n[Back to Help Index](index.md)\n';
    expect(prepareHelpMarkdown(raw)).toBe('# Some Page\n\nBody text.');
  });

  it('trims blank lines left over once the comment block above the heading is removed', () => {
    const raw = '<!-- comment -->\n\n\n# Heading\n\nBody\n\n';
    expect(prepareHelpMarkdown(raw)).toBe('# Heading\n\nBody');
  });

  /**
   * Known limitation, not a bug: the comment-stripping step has no idea
   * a fenced code block exists, so an HTML comment written *inside* one
   * (e.g. a code sample showing someone else's HTML) gets removed too,
   * same as a real licence comment would. This is accepted rather than
   * fixed because it costs nothing today -- every `help/*.md` page is
   * prose documentation, and none of them show HTML-comment syntax as an
   * example inside a code fence. If a future page ever needs to, this
   * test is the marker that says the stripping logic will need to learn
   * about fences at that point, not before.
   */
  it('also strips an HTML comment written inside a fenced code block (accepted limitation -- no help page does this today)', () => {
    const raw = '# Heading\n\n```html\n<!-- keep me -->\n```\n';
    expect(prepareHelpMarkdown(raw)).not.toContain('<!-- keep me -->');
  });
});

describe('buildAboutBuildSection', () => {
  const parts = {
    version: '1.13.0-test.1',
    componentVersionsTable: '| Component | Version |\n|---|---|\n| FFmpeg | 7.0 |',
    acknowledgementsMd: 'FULL ACKNOWLEDGEMENTS TEXT',
    thirdPartyLicencesMd: 'FULL THIRD PARTY LICENCE TEXT',
  };

  /**
   * The version number is the one piece of the About page a Markdown
   * file could never contain -- it's only known once the app is
   * actually running. If it were missing here, a user would have no way
   * to tell Support which build they're on.
   */
  it('includes the version number it was given', () => {
    expect(buildAboutBuildSection(parts)).toContain(parts.version);
  });

  /**
   * The About page presents licence/acknowledgement/dependency data as
   * three separate collapsible sections rather than one wall of text, so
   * a reader can open only the one they care about. If any of the three
   * <details> blocks silently stopped being generated, that section's
   * content would still be embedded in the binary but would have no
   * accessible entry point in the UI.
   */
  it('includes all three collapsible build-info sections', () => {
    const result = buildAboutBuildSection(parts);
    expect(result).toContain('Open Source Acknowledgements');
    expect(result).toContain('Third-Party Licences');
    expect(result).toContain('Dependencies');
  });

  /** Each section must actually embed the text it was handed, not a placeholder. */
  it('embeds the exact acknowledgements, licence, and component-table text it was given', () => {
    const result = buildAboutBuildSection(parts);
    expect(result).toContain(parts.acknowledgementsMd);
    expect(result).toContain(parts.thirdPartyLicencesMd);
    expect(result).toContain(parts.componentVersionsTable);
  });
});

describe('sidebar reading order', () => {
  /**
   * `HelpViewer` opens on `HELP_TOPICS[0]` (via its `useState<HelpTopicId>('getting-started')`
   * default) on the assumption that the first manifest entry IS
   * "Getting Started". If a future reorder ever moved something else to
   * the top of `HELP_TOPIC_MANIFEST`, the Help screen would still open
   * on "Getting Started" by name while the sidebar visually led with a
   * different page -- a small but confusing mismatch between what's
   * highlighted and what's on screen.
   */
  it('opens the manifest with Getting Started, since that is what the Help screen shows first', () => {
    expect(HELP_TOPIC_MANIFEST[0]?.id).toBe('getting-started');
  });
});
