/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file helpTopics.ts -- Loads the in-app Help screen's content from the
 * Markdown files in `help/*.md`.
 *
 * This is the only place in the app that knows help content lives in
 * files rather than being typed into a component. That used to not be
 * true: the Help screen used to carry its own copy of every topic's text,
 * hand-typed a second time inside `HelpViewer.tsx`. Whenever someone
 * edited a page on GitHub and forgot the in-app copy (or the other way
 * round), the two silently drifted apart -- a user reading the in-app
 * Help screen would see different words than someone reading the same
 * topic in the repository, and nothing ever warned either of them. This
 * file removes that trap by reading the real files instead of a second,
 * hand-typed copy of them.
 *
 * The rule going forward is simple: every file in `help/` except
 * `index.md` is a page shown in the app, and every page shown in the app
 * is a file in `help/`. `index.md` is the one deliberate exception --
 * it is a table of contents written for someone reading the docs on
 * GitHub, where there is no sidebar to browse by. Inside the app, the
 * Help screen's own sidebar already *is* the table of contents, so an
 * in-app copy of `index.md` would just be a page of links to pages
 * already sitting one click away in that same sidebar -- worth nothing.
 */

import type { LucideIcon } from 'lucide-react';
import {
  BookOpen, // "Getting Started" -- first thing a new user needs
  Download, // "Downloading Music"
  Video, // "Downloading Videos"
  Music, // "Audio Codecs"
  SlidersHorizontal, // "Quality Settings"
  ArrowDownUp, // "Fallback Quality"
  Captions, // "Lyrics & Metadata"
  Tags, // "Metadata Mapping"
  Cookie, // "Cookies"
  Shield, // "Wrapper"
  Film, // "Animated Artwork"
  Settings, // "Settings"
  Wrench, // "Tools"
  Globe, // "Supported Services"
  GitBranch, // "Release Channels"
  Keyboard, // "Keyboard Shortcuts"
  HelpCircle, // "Troubleshooting"
  MessageCircleQuestionMark, // "FAQ"
  ShieldAlert, // "Disclaimer"
  FileText, // "About"
} from 'lucide-react';

/**
 * Every page the Help screen's sidebar shows, in the order the sidebar
 * shows them. That order is a deliberate reading path, not alphabetical:
 * first the pages a new user reaches for (Getting Started, then the two
 * "how do I download X" pages), then reference material grouped by
 * subject (quality/codecs, lyrics/metadata, cookies/wrapper/artwork,
 * settings/tools), then "about this app" material (services, channels,
 * shortcuts, troubleshooting, FAQ), and the two legal pages last.
 *
 * Adding a help page means adding BOTH the `help/<id>.md` file AND one
 * line here -- a unit test and a PR audit check both fail if only one of
 * the two is done, so the sidebar and the files on disk can't quietly
 * drift out of step the way the old hand-typed copy could.
 */
export const HELP_TOPIC_MANIFEST = [
  { id: 'getting-started', label: 'Getting Started', icon: BookOpen },
  { id: 'downloading-music', label: 'Downloading Music', icon: Download },
  { id: 'downloading-videos', label: 'Downloading Videos', icon: Video },
  { id: 'audio-codecs', label: 'Audio Codecs', icon: Music },
  { id: 'quality-settings', label: 'Quality Settings', icon: SlidersHorizontal },
  { id: 'fallback-quality', label: 'Fallback Quality', icon: ArrowDownUp },
  { id: 'lyrics-and-metadata', label: 'Lyrics & Metadata', icon: Captions },
  { id: 'metadata-mapping', label: 'Metadata Mapping', icon: Tags },
  { id: 'cookie-management', label: 'Cookies', icon: Cookie },
  { id: 'wrapper', label: 'Wrapper', icon: Shield },
  { id: 'animated-artwork', label: 'Animated Artwork', icon: Film },
  { id: 'settings', label: 'Settings', icon: Settings },
  { id: 'tools', label: 'Tools', icon: Wrench },
  { id: 'supported-services', label: 'Supported Services', icon: Globe },
  { id: 'release-channels', label: 'Release Channels', icon: GitBranch },
  { id: 'keyboard-shortcuts', label: 'Keyboard Shortcuts', icon: Keyboard },
  { id: 'troubleshooting', label: 'Troubleshooting', icon: HelpCircle },
  { id: 'faq', label: 'FAQ', icon: MessageCircleQuestionMark },
  { id: 'disclaimer', label: 'Disclaimer', icon: ShieldAlert },
  { id: 'about', label: 'About', icon: FileText },
] as const satisfies readonly { id: string; label: string; icon: LucideIcon }[];

/** The set of valid help page ids, derived from the manifest above. */
export type HelpTopicId = (typeof HELP_TOPIC_MANIFEST)[number]['id'];

/** A help page ready to render: manifest metadata plus its loaded Markdown text. */
export interface HelpTopic {
  id: HelpTopicId;
  label: string;
  icon: LucideIcon;
  content: string;
}

/**
 * Reads every Markdown file under `help/` into memory, right now, at
 * module load time, rather than fetching each one only when its page is
 * opened.
 *
 * Why load them all up front instead of on demand: `src/lib/i18n.ts`
 * makes the same trade-off for the app's English UI text, for the same
 * reason. It bundles English straight into the JS file with a normal
 * `import`, so the very first render already has real text instead of
 * showing raw translation keys while a network request is still in
 * flight -- only a *second* language, if the user picks one, is fetched
 * over the network afterwards. Help works the same way: these twenty
 * pages are the only language today, so they are loaded eagerly and the
 * Help screen has real content the instant it opens, with no spinner and
 * no empty page while a file loads. When translated help pages arrive
 * (issue #111), they will live at `help/<language>/<same file name>.md`
 * and be read through a *second*, non-eager glob, so a page in French or
 * German is only pulled into memory when someone actually views it in
 * that language -- mirroring how `i18n.ts` only fetches a non-English
 * translation file after the app detects or the user picks that language.
 *
 * `import.meta.glob` is a Vite build-time feature: Vite scans for every
 * file matching the pattern while it builds the app and inlines each
 * one's text directly into the bundle. `query: '?raw'` says "give me the
 * file's raw text, not an HTML-imported component"; `eager: true` says
 * "inline it now" instead of generating a lazy `import()` per file.
 */
const HELP_PAGE_FILES = import.meta.glob<string>('/help/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
});

/**
 * Cleans up one page's raw file text so it is fit to show inside the
 * app. Every `help/*.md` file is written to read well on GitHub as well
 * as in the app, which means it carries a couple of things that only
 * make sense on GitHub. This function strips those, and nothing else --
 * the wording of the page itself is untouched.
 */
export function prepareHelpMarkdown(raw: string): string {
  let text = raw;

  // 1. Strip the HTML licence-comment block every help file opens with.
  // It renders as literally nothing on screen either way (browsers and
  // react-markdown both simply don't display an HTML comment), but the
  // Help screen's search box searches the *raw* text of every page, not
  // just what's visible -- so if we left it in, searching for the word
  // "copyright" would "find" it in all twenty pages at once. Stripping
  // it here means search only ever matches words a person can actually
  // see. (This regex would also strip an HTML comment written inside a
  // fenced code block, which none of today's pages do -- these are prose
  // documentation pages, not code samples, so that edge case isn't worth
  // the extra parsing it would take to tell the two apart.)
  text = text.replace(/<!--[\s\S]*?-->/g, '');

  // 2. Strip the "Back to Help Index" footer link, and the horizontal
  // rule left dangling above it once the link is gone. That link exists
  // so a reader on GitHub can hop back to `index.md`'s table of
  // contents -- but the app has no `index.md` page (see the file-level
  // comment above for why), so the link would point at a page that
  // doesn't exist. Left in, clicking it would either do nothing or, once
  // the WebView link-safety fix in HelpViewer.tsx is in place, be
  // silently swallowed -- either way it's a dead end for the user, so we
  // remove it before it's ever rendered.
  text = text.replace(/^\[Back to Help Index\]\(index\.md\)[ \t]*$/gm, '');
  text = text.replace(/\n+-{3,}\s*$/, '');

  // 3. Trim so the page starts right at its heading, not on a blank
  // line left over from removing the comment block above.
  return text.trim();
}

/**
 * Every help page, loaded and cleaned, in sidebar order. This is what
 * `HelpViewer.tsx` renders.
 *
 * Building this list eagerly (rather than lazily per-page) means a typo
 * in `HELP_TOPIC_MANIFEST` -- an id with no matching file, most likely --
 * is caught the moment the app starts, not the moment a user happens to
 * click that one topic.
 */
export const HELP_TOPICS: readonly HelpTopic[] = HELP_TOPIC_MANIFEST.map((meta) => {
  const raw = HELP_PAGE_FILES[`/help/${meta.id}.md`];
  if (raw === undefined) {
    // Thrown, not logged: this is a broken build, not a runtime hiccup.
    // Failing loudly here -- in the dev server, in `npm run build`, and
    // in the test suite -- means the mistake is caught by whoever made
    // it, right away, instead of shipping as a Help screen that quietly
    // shows nothing for one topic.
    throw new Error(
      `Help page "${meta.id}" is listed in HELP_TOPIC_MANIFEST but help/${meta.id}.md does not exist. Add the file, or remove the entry.`
    );
  }
  return { ...meta, content: prepareHelpMarkdown(raw) };
});

/**
 * Builds the part of the About page that a Markdown file could never
 * contain on its own, and glues it onto the end of `help/about.md`'s
 * own content at render time.
 *
 * A file on disk is the same for every user on every machine. But "what
 * version of MeedyaDL is this", "which tools are actually installed on
 * this computer", and the exact legal text bundled inside this
 * particular build are all things that are only known once the app is
 * running -- `about.md` cannot contain real answers to them, only
 * placeholders. Keeping this section in code (instead of writing
 * `{{VERSION}}`-style placeholders into `about.md` itself) also means
 * nobody reading `about.md` on GitHub ever sees a raw, unfilled
 * placeholder sitting in the middle of the page.
 */
export function buildAboutBuildSection(parts: {
  version: string;
  componentVersionsTable: string;
  acknowledgementsMd: string;
  thirdPartyLicencesMd: string;
}): string {
  return `## This build

**Version** v${parts.version}

<details>
<summary><strong>Open Source Acknowledgements</strong></summary>

MeedyaDL is built on top of many open-source projects. The full inventory of every direct dependency, its licence, and its purpose lives in the \`ACKNOWLEDGEMENTS.md\` file bundled inside this build (#802). Expand below to read it inline without leaving the app.

<details>
<summary><em>Show full acknowledgements (ACKNOWLEDGEMENTS.md)</em></summary>

${parts.acknowledgementsMd}

</details>

</details>

<details>
<summary><strong>Third-Party Licences</strong></summary>

The verbatim upstream copyright notices and licence text for every external engine and tool MeedyaDL invokes or bundles (GAMDL, FFmpeg, MP4Box, MediaInfo, mp4decrypt, N_m3u8DL-RE, Python, etc.) — plus the LGPL/GPL written offer for source code that applies to the offline-installer build — are reproduced verbatim from upstream in the \`THIRD_PARTY_LICENSES.md\` file bundled inside this build. Expand to read.

<details>
<summary><em>Show full third-party licences (THIRD_PARTY_LICENSES.md)</em></summary>

${parts.thirdPartyLicencesMd}

</details>

</details>

<details>
<summary><strong>Dependencies</strong></summary>

${parts.componentVersionsTable}

</details>`;
}
