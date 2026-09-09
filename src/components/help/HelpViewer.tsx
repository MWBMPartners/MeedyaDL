/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file HelpViewer.tsx -- Help documentation viewer with search.
 *
 * Renders the "Help" page within the main application. The component
 * provides a two-column layout:
 *
 *   - **Left sidebar** -- Searchable list of help topics, each with an
 *     icon and label. Clicking a topic displays its content in the viewer.
 *   - **Right content area** -- Renders the selected topic's Markdown
 *     content using `react-markdown` with the `remark-gfm` plugin.
 *
 * ## Search Feature
 *
 * The sidebar includes a text input that filters topics in real-time:
 *   - Filters by topic label AND topic content (case-insensitive).
 *   - Matched portions of the label are highlighted with `<mark>` elements
 *     via the `HighlightedLabel` sub-component.
 *   - A result count is displayed below the search input.
 *   - A clear (X) button resets the search when text is entered.
 *   - A keyboard shortcut hint (Cmd+K / Ctrl+K) is shown as a visual
 *     placeholder for future shortcut implementation.
 *
 * ## Where the content comes from
 *
 * Every topic's Markdown text is read from a real file in `help/*.md` --
 * this component does not carry its own copy of the words. `./helpTopics.ts`
 * is what does the reading: it lists which pages exist and in what
 * sidebar order (`HELP_TOPIC_MANIFEST`), loads each one's file at build
 * time, and hands this component the finished list (`HELP_TOPICS`). See
 * that file for the full reasoning. This component's only job with
 * respect to content is picking which topic is active and rendering it --
 * it never needs to know that the words live in files at all.
 *
 * The one topic that isn't pure file content is "About": its version
 * number, installed-tool list, and the bundled licence text can only be
 * known once the app is actually running, so `helpTopics.ts` exposes
 * `buildAboutBuildSection()` to build that part, and this component
 * appends it to `about.md`'s file content before rendering.
 *
 * ## Translated pages (#111)
 *
 * When the app's display language is not English, this component asks
 * `helpTopics.ts`'s `loadTranslatedHelpPages()` for that language's
 * translated pages (from `help/<language>/*.md`) and, for whichever
 * topic is on screen, swaps in the translated text if one exists for it.
 * A small notice appears above the page either way: "this was
 * machine-translated" when a translation was found, or "not translated
 * yet, showing English" when it wasn't. English itself is always a
 * ready fallback -- this component never shows a blank page or a
 * loading spinner while a translation is still being fetched.
 *
 * ## Markdown Rendering
 *
 * Content is rendered using:
 *   - `react-markdown` (v9+) -- Core Markdown-to-React renderer
 *     @see {@link https://www.npmjs.com/package/react-markdown}
 *   - `remark-gfm` -- Plugin for GitHub Flavored Markdown support
 *     (tables, strikethrough, task lists, autolinks)
 *     @see {@link https://www.npmjs.com/package/remark-gfm}
 *
 * Tailwind CSS `prose` classes from `@tailwindcss/typography` provide
 * typographic styling with automatic dark mode support via `dark:prose-invert`.
 *
 * ## Sub-components (file-private)
 *
 * - `isMacPlatform()` -- Detects macOS for modifier key display.
 * - `escapeRegExp()` -- Escapes regex special characters in search queries.
 * - `HighlightedLabel` -- Renders a label with search matches highlighted.
 *
 * ## Store Connections
 *
 * This component does NOT connect to any Zustand stores. It is entirely
 * self-contained with local state for the active topic and search query.
 *
 * @see {@link https://www.npmjs.com/package/react-markdown}  -- react-markdown
 * @see {@link https://www.npmjs.com/package/remark-gfm}      -- remark-gfm plugin
 * @see {@link https://react.dev/reference/react/useState}     -- React useState
 * @see {@link https://react.dev/reference/react/useMemo}      -- React useMemo
 * @see {@link https://react.dev/reference/react/useCallback}  -- React useCallback
 * @see {@link https://lucide.dev/}                            -- Lucide icon library
 */

// React hooks: useState for active topic and search state, useMemo for
// memoized filtering and platform detection, useCallback for stable handlers.
import { useState, useEffect, useMemo, useCallback } from 'react';

// react-i18next's hook gives us the app's CURRENT display language
// (i18n.language) -- e.g. "de", "fr-FR", "en" -- which is what decides
// whether this screen has a translated page to show and which note (if
// any) to display above it. See the effect further down for how it's
// used.
import { useTranslation } from 'react-i18next';

/**
 * react-markdown -- Renders Markdown strings as React components.
 * Used to display help topic content in the right-side viewer pane.
 * @see https://www.npmjs.com/package/react-markdown
 * @see https://github.com/remarkjs/react-markdown
 */
import ReactMarkdown from 'react-markdown';

/**
 * remark-gfm -- Remark plugin that adds support for GitHub Flavored
 * Markdown (GFM) extensions: tables, strikethrough (~text~), task
 * lists (- [x] item), and autolinks. Passed to ReactMarkdown's
 * `remarkPlugins` prop.
 * @see https://www.npmjs.com/package/remark-gfm
 * @see https://github.github.com/gfm/
 */
import remarkGfm from 'remark-gfm';

/**
 * rehype-raw: Allows raw HTML tags (e.g., <details>, <summary>) to pass
 * through ReactMarkdown without being stripped. Required for collapsible
 * sections in the About page using native HTML disclosure elements.
 * @see {@link https://github.com/rehypejs/rehype-raw}
 */
import rehypeRaw from 'rehype-raw';
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize';

/**
 * Custom sanitization schema extending the default GitHub-style schema
 * to allow <details>/<summary> (collapsible sections in About page).
 * All dangerous elements (script, iframe, event handlers) are stripped.
 * @see {@link https://github.com/MWBMPartners/MeedyaDL/issues/227}
 */
const helpSanitizeSchema = {
  ...defaultSchema,
  tagNames: [...(defaultSchema.tagNames ?? []), 'details', 'summary'],
};

// Only two Lucide icons are used directly by this component itself (the
// search bar). The per-topic icons come attached to each entry in
// `HELP_TOPICS` -- see helpTopics.ts -- so they don't need to be
// imported here by name.
import { Search, X } from 'lucide-react';

// Tauri app API for reading the version from tauri.conf.json at runtime.
import { getVersion } from '@tauri-apps/api/app';

// Shared layout component for the page header.
import { PageHeader } from '@/components/layout';

// UI store for reading/clearing the help deep-link topic.
import { useUiStore } from '@/stores/uiStore';

// The loaded help pages (content read from help/*.md, see that file for
// the full explanation) plus the "About" build-info builder, the
// translated-page loader, and the HelpTopicId type used to keep the
// active-topic state honest.
import {
  HELP_TOPICS,
  buildAboutBuildSection,
  loadTranslatedHelpPages,
  type HelpTopicId,
} from './helpTopics';

// LOCALES gives us each language's own name (e.g. "Deutsch" for "de") so
// the "not translated yet" note can say which language is missing, in
// words a reader recognises, instead of a bare code like "de".
// baseLanguageOf is the one place "de-DE means de" is decided -- reused
// here so this screen can never disagree with helpTopics.ts or the
// Settings language dropdown about what a given language setting means.
import { LOCALES, baseLanguageOf } from '@/lib/i18n';

/**
 * Detects whether the user is on macOS so we can display the correct
 * modifier key hint (Cmd on macOS, Ctrl on everything else).
 * Uses navigator.platform with a fallback to navigator.userAgent for
 * broader browser compatibility.
 */
function isMacPlatform(): boolean {
  if (typeof navigator !== 'undefined') {
    return (
      navigator.platform?.toUpperCase().includes('MAC') ||
      navigator.userAgent?.toUpperCase().includes('MAC')
    );
  }
  return false;
}

/**
 * Escapes special regex characters in a user-supplied string so it can
 * be safely used inside a RegExp constructor without unintended pattern matching.
 * For example, a search query containing "C++" would be escaped to "C\\+\\+"
 * so the plus signs are matched literally.
 *
 * @param str - The raw string to escape
 * @returns The escaped string safe for RegExp construction
 */
function escapeRegExp(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Renders a label string with search match segments highlighted.
 * Splits the label on case-insensitive matches of the query, then wraps
 * every matched segment in a styled <mark> element to visually distinguish
 * it from the surrounding text.
 *
 * If the query is empty or there are no matches, the label is returned as
 * plain text with no extra markup.
 *
 * @param label - The full label text to render
 * @param query - The current search query to highlight within the label
 * @returns A React fragment containing text nodes and <mark> elements
 */
function HighlightedLabel({ label, query }: { label: string; query: string }) {
  /* When there is no search query, render the label as plain text */
  if (!query.trim()) {
    return <>{label}</>;
  }

  /**
   * Build a case-insensitive regex that captures the matched portion.
   * The capturing group ensures that String.prototype.split retains
   * the matched segments in the resulting array (interleaved between
   * the non-matching parts).
   */
  const regex = new RegExp(`(${escapeRegExp(query.trim())})`, 'gi');
  const parts = label.split(regex);

  return (
    <>
      {parts.map((part, index) => {
        /**
         * Check whether this segment is a match by comparing it
         * case-insensitively against the query. Matched segments
         * receive highlight styling; non-matched segments render
         * as ordinary text.
         */
        const isMatch = part.toLowerCase() === query.trim().toLowerCase();
        return isMatch ? (
          <mark key={index} className="bg-yellow-300/40 text-inherit rounded-sm px-0.5">
            {part}
          </mark>
        ) : (
          <span key={index}>{part}</span>
        );
      })}
    </>
  );
}

/**
 * Renders the help page with a searchable topic sidebar and markdown content viewer.
 *
 * The component maintains two pieces of state:
 * - activeTopic: the ID of the currently selected help topic
 * - searchQuery: the current text in the search input
 *
 * When the user types a search query, the sidebar filters help topics by checking
 * whether the query appears (case-insensitively) in the topic label or markdown
 * content. Matching portions of the label are highlighted inline. A result count
 * is shown below the search input when a query is active.
 */
export function HelpViewer() {
  /** Tracks which help topic is currently displayed in the content viewer */
  const [activeTopic, setActiveTopic] = useState<HelpTopicId>('getting-started');

  /** Tracks the current search input value for filtering the sidebar topics */
  const [searchQuery, setSearchQuery] = useState('');

  /** The app's current display language, e.g. "de", "fr-FR", "en". */
  const { t, i18n } = useTranslation();

  /**
   * Every translated help page for the current language, keyed by page
   * id. Starts empty on every language change and only fills in once
   * `loadTranslatedHelpPages()` resolves -- see the effect below for why
   * that "starts empty" moment is never shown to the user as a blank
   * page or a spinner: while this is empty (or simply doesn't have the
   * page someone's looking at), the render logic further down falls
   * back to the English content that's already sitting in `HELP_TOPICS`,
   * which was loaded eagerly and is always ready immediately.
   */
  const [translatedPages, setTranslatedPages] = useState<Record<string, string>>({});

  /**
   * (Re)loads the translated help pages whenever the app's display
   * language changes. `cancelled` guards against a slow load from a
   * PREVIOUS language finishing after the user has already switched to
   * a different one and landing in the wrong state.
   */
  useEffect(() => {
    let cancelled = false;
    loadTranslatedHelpPages(i18n.language).then((pages) => {
      if (!cancelled) {
        setTranslatedPages(pages);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [i18n.language]);

  /** App version fetched from tauri.conf.json, used in the About topic */
  const [appVersion, setAppVersion] = useState('...');
  /** Component version table (markdown) for the About > Component Library section */
  const [componentVersions, setComponentVersions] = useState('*Loading...*');
  /**
   * Verbatim content of `ACKNOWLEDGEMENTS.md` (#802). Embedded into the
   * binary via `include_str!()` and surfaced through the legal IPC. Fed
   * into `buildAboutBuildSection()` below to fill in the About topic's
   * "Open Source Acknowledgements" section at render time. `*Loading…*`
   * is the pre-load fallback so the section never renders an empty block.
   */
  const [acknowledgementsMd, setAcknowledgementsMd] = useState('*Loading…*');
  /**
   * Verbatim content of `THIRD_PARTY_LICENSES.md` (#802) — the actual
   * MIT/BSD/LGPL/GPL/PSF licence text + source offers for bundled
   * components. Fed into `buildAboutBuildSection()` below to fill in the
   * About topic's "Third-Party Licences" section at render time.
   */
  const [thirdPartyLicensesMd, setThirdPartyLicensesMd] = useState('*Loading…*');
  useEffect(() => {
    getVersion()
      .then((v) => setAppVersion(v))
      .catch(() => setAppVersion('unknown'));
  }, []);

  // Fetch component versions, acknowledgements, and third-party licence
  // text when the About topic is selected. All three IPCs are cheap and
  // idempotent — the legal text is `include_str!`-embedded so there's no
  // disk I/O — but we still defer until About is opened so the data
  // isn't loaded unnecessarily on every app launch.
  useEffect(() => {
    if (activeTopic !== 'about') return;
    import('@/lib/tauri-commands')
      .then(({ getComponentVersions, getAcknowledgementsText, getThirdPartyLicensesText }) => {
        getComponentVersions()
          .then((versions) => {
            const rows = versions
              .filter((v) => v.installed)
              .map((v) => `| ${v.name} | ${v.version ?? 'unknown'} |`)
              .join('\n');
            const table = `| Component | Version |\n|-----------|--------|\n${rows}`;
            setComponentVersions(table);
          })
          .catch(() => setComponentVersions('*Unable to load component versions.*'));

        // #802: load the embedded ACKNOWLEDGEMENTS.md + THIRD_PARTY_LICENSES.md
        getAcknowledgementsText()
          .then(setAcknowledgementsMd)
          .catch(() => setAcknowledgementsMd('*Unable to load acknowledgements.*'));
        getThirdPartyLicensesText()
          .then(setThirdPartyLicensesMd)
          .catch(() => setThirdPartyLicensesMd('*Unable to load third-party licences.*'));
      });
  }, [activeTopic]);

  /* ---- Store bindings for help deep-linking ---- */
  /** Deep-link topic ID set by HelpButton clicks (null when no deep-link) */
  const helpActiveTopic = useUiStore((s) => s.helpActiveTopic);
  /** Action to clear the deep-link after consuming it */
  const clearHelpActiveTopic = useUiStore((s) => s.clearHelpActiveTopic);

  /**
   * Consume the helpActiveTopic deep-link from the UI store.
   * When a HelpButton sets a topic and navigates here, this effect
   * auto-selects the requested topic and clears the deep-link so
   * subsequent visits to the Help page start on the last-viewed topic.
   */
  useEffect(() => {
    if (helpActiveTopic) {
      // Only navigate if the topic exists in our list. helpActiveTopic is
      // typed as HelpTopicId (see uiStore.ts), which already stops a
      // mistyped or renamed page id from compiling anywhere a HelpButton
      // is written -- this find() is a second, defensive check for a
      // value that reached the store some other way (a deep link parsed
      // from outside React, for instance), so it stays a silent no-op
      // instead of a broken active-topic state rather than throwing.
      const target = HELP_TOPICS.find((t) => t.id === helpActiveTopic);
      if (target) {
        setActiveTopic(target.id);
      }
      clearHelpActiveTopic();
    }
  }, [helpActiveTopic, clearHelpActiveTopic]);

  /**
   * Determine the platform-appropriate modifier key label once.
   * On macOS we show the Cmd symbol; on other platforms we show "Ctrl".
   * This is memoized because isMacPlatform() accesses navigator, and
   * we only need to evaluate it once per component mount.
   */
  const modifierKey = useMemo(() => (isMacPlatform() ? '⌘' : 'Ctrl'), []);

  /**
   * Narrows the page list down to whatever the person typed in the
   * search box.
   *
   * An empty box means "show everything". Otherwise a page is kept if
   * what they typed appears in its name or anywhere in its text,
   * ignoring capitals.
   *
   * **Both languages are searched, not just English.** Somebody reading
   * the German pages sees German headings, so those are the words they
   * will type. Searching only the English would mean typing a heading
   * that is visible on screen and being told there are no results,
   * which reads as the search being broken. The English is searched as
   * well as the translation, not instead of it, because plenty of the
   * words worth searching for — setting names, file names, service
   * names — stay in English on every page in every language.
   *
   * Recalculated when the search text changes and when a translation
   * finishes loading, so results do not stay stale behind a language
   * that has just arrived.
   */
  const filteredTopics = useMemo(() => {
    const trimmed = searchQuery.trim().toLowerCase();

    /* No query: return the full topic list unfiltered */
    if (!trimmed) {
      return HELP_TOPICS;
    }

    return HELP_TOPICS.filter((topic) => {
      const translated = translatedPages[topic.id];
      return (
        topic.label.toLowerCase().includes(trimmed) ||
        topic.content.toLowerCase().includes(trimmed) ||
        (translated !== undefined && translated.toLowerCase().includes(trimmed))
      );
    });
  }, [searchQuery, translatedPages]);

  /**
   * Look up the currently active topic object.
   * Falls back to the first topic in the full list if the active ID
   * is not found (e.g. on initial render or after a state reset).
   */
  const topic = HELP_TOPICS.find((t) => t.id === activeTopic) || HELP_TOPICS[0];

  /**
   * Handles selecting a topic from the sidebar.
   * Updates the active topic state so the content viewer shows
   * the selected topic's markdown.
   */
  const handleTopicSelect = useCallback((topicId: HelpTopicId) => {
    setActiveTopic(topicId);
  }, []);

  /**
   * Handles changes to the search input.
   * Updates the searchQuery state which triggers re-filtering
   * of the sidebar topics via the filteredTopics memo.
   */
  const handleSearchChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setSearchQuery(e.target.value);
  }, []);

  /**
   * Clears the search input and resets the filtered view to show
   * all topics. Called when the user clicks the clear (X) button.
   */
  const handleClearSearch = useCallback(() => {
    setSearchQuery('');
  }, []);

  /**
   * Determines whether a search is actively filtering the topic list.
   * Used to conditionally render the result count and clear button.
   */
  const isSearchActive = searchQuery.trim().length > 0;

  /**
   * The base two-letter code for the app's current display language
   * (e.g. "de-DE" -> "de"). English is treated as "not a translated
   * language" -- there is nothing to translate FROM English INTO
   * English, so no note and no translated-page lookup ever apply when
   * this is "en".
   */
  const activeLanguage = baseLanguageOf(i18n.language);
  const isTranslatedLanguage = activeLanguage !== 'en';

  /**
   * Does the CURRENT topic specifically have a translated version for
   * the current language? Most of the app's ~20 help pages don't have
   * one yet (translation is added page by page -- see helpTopics.ts for
   * the full plan), so this is checked per-topic, not once for the
   * whole screen.
   */
  const translatedContent = translatedPages[topic.id];
  const hasTranslation = isTranslatedLanguage && translatedContent !== undefined;

  /**
   * The English name of the currently active locale (e.g. "German" is
   * NOT what we want -- we want "Deutsch", the name IN that language),
   * used to fill in the "not translated yet" note below. Falls back to
   * the bare code on the (should-never-happen) chance a language is
   * active that isn't in `LOCALES` at all.
   */
  const activeLanguageName =
    LOCALES.find((locale) => locale.code === activeLanguage)?.nativeName ?? activeLanguage;

  /**
   * The Markdown text to actually render for the active topic: the
   * translated version when the current language has one, otherwise the
   * original English text from `HELP_TOPICS`. English is ALWAYS a valid
   * fallback here -- never a blank page, never a spinner -- because a
   * reader who can't yet get a page in their own language is still far
   * better served by the real English page than by nothing at all.
   *
   * Every topic except "About" is just this content, unchanged. "About"
   * is the one page whose content can't be fully known until the app is
   * running (see the file-level comment above), so its content is
   * followed by a build-info section assembled from live data -- the
   * app version and the three pieces of licence/tool text fetched by
   * the effect above. That build-info section is only ever generated in
   * English today -- there is no translated build-info to fall back
   * from, so this is unaffected by the language logic above.
   */
  const activeContent = hasTranslation ? translatedContent : topic.content;
  const markdown =
    topic.id === 'about'
      ? `${activeContent}\n\n${buildAboutBuildSection({
          version: appVersion,
          componentVersionsTable: componentVersions,
          acknowledgementsMd,
          thirdPartyLicencesMd: thirdPartyLicensesMd,
        })}`
      : activeContent;

  return (
    <div className="flex flex-col h-full">
      {/* Page header with title and description */}
      <PageHeader title={t('help.title')} subtitle={t('help.subtitle')} />

      <div className="flex flex-1 overflow-hidden">
        {/* ----------------------------------------------------------------
         * Topic sidebar
         * Contains the search bar and the scrollable list of help topics.
         * The sidebar has a fixed width and does not shrink when the
         * content area needs more space.
         * ---------------------------------------------------------------- */}
        <nav className="w-56 flex-shrink-0 border-r border-border-light overflow-y-auto flex flex-col">
          {/* --------------------------------------------------------------
           * Search bar section
           * Positioned at the top of the sidebar with sticky behavior so
           * it remains visible as the user scrolls through topics.
           * -------------------------------------------------------------- */}
          <div className="sticky top-0 bg-surface-primary z-10 p-2 pb-1 border-b border-border-light">
            {/* Search input wrapper: contains the icon, input, keyboard
                hint, and clear button in a single horizontal row */}
            <div className="relative flex items-center">
              {/* Search icon on the left side of the input */}
              <Search
                size={14}
                className="absolute left-2.5 text-content-tertiary pointer-events-none"
                aria-hidden="true"
              />

              {/* The search text input. Padded on the left to make room
                  for the search icon, and on the right for the keyboard
                  shortcut hint and clear button. */}
              <input
                type="text"
                value={searchQuery}
                onChange={handleSearchChange}
                placeholder={t('help.searchPlaceholder')}
                aria-label={t('help.searchAriaLabel')}
                className="
                  w-full pl-8 pr-16 py-1.5
                  text-xs rounded-platform
                  bg-surface-secondary
                  border border-border-light
                  text-content-primary
                  placeholder:text-content-tertiary
                  focus:outline-none focus:ring-1 focus:ring-accent
                  transition-colors
                "
              />

              {/* Right-side controls positioned absolutely within the input.
                  Shows the keyboard shortcut hint when idle, or the clear
                  button when a search query is entered. */}
              <div className="absolute right-2 flex items-center gap-1">
                {isSearchActive ? (
                  /* Clear search button: visible only when there is text
                     in the search input. Resets the query on click. */
                  <button
                    onClick={handleClearSearch}
                    className="
                      p-0.5 rounded
                      text-content-tertiary
                      hover:text-content-primary
                      hover:bg-surface-tertiary
                      transition-colors
                    "
                    aria-label={t('help.clearSearch')}
                    title={t('help.clearSearch')}
                  >
                    <X size={12} />
                  </button>
                ) : (
                  /* Keyboard shortcut hint: shown when the input is empty.
                     Displays Cmd+K on macOS or Ctrl+K on other platforms.
                     This is a visual placeholder for future keyboard
                     shortcut support (the actual shortcut handler is not
                     yet implemented). The "Cmd"/"Ctrl" part of `modifierKey`
                     is a key name, never translated -- it's passed into the
                     translated sentence as `{{key}}` rather than baked into
                     the English string, so a translator only ever touches
                     the surrounding words. */
                  <kbd
                    className="
                      hidden sm:inline-flex items-center gap-0.5
                      px-1 py-0.5 rounded
                      text-[10px] leading-none
                      font-mono
                      text-content-tertiary
                      bg-surface-tertiary
                      border border-border-light
                    "
                    title={t('help.searchShortcutHint', { key: modifierKey })}
                    aria-label={t('help.searchShortcutAriaLabel', { key: modifierKey })}
                  >
                    {modifierKey}+K
                  </kbd>
                )}
              </div>
            </div>

            {/* Result count: displayed below the search input when the user
                has entered a search query. Shows the number of matching
                topics to give immediate feedback on the search scope. */}
            {isSearchActive && (
              <div className="mt-1 px-1 text-[10px] text-content-tertiary">
                {t('help.resultCount', { count: filteredTopics.length })}
              </div>
            )}
          </div>

          {/* --------------------------------------------------------------
           * Topic list
           * Renders a button for each help topic that passes the current
           * search filter. Each button shows the topic's icon and label.
           * When a search is active, matching portions of the label text
           * are highlighted.
           * -------------------------------------------------------------- */}
          <div className="p-2 space-y-0.5 flex-1">
            {filteredTopics.length > 0 ? (
              filteredTopics.map(({ id, label, icon: Icon }) => (
                <button
                  key={id}
                  onClick={() => handleTopicSelect(id)}
                  className={`
                    w-full flex items-center gap-2.5 px-3 py-2
                    rounded-platform text-sm transition-colors
                    ${
                      activeTopic === id
                        ? 'bg-accent-light text-accent font-medium'
                        : 'text-content-secondary hover:text-content-primary hover:bg-surface-secondary'
                    }
                  `}
                >
                  {/* Topic icon: fixed size to maintain alignment across
                      all sidebar entries regardless of label length */}
                  <Icon size={16} className="flex-shrink-0" />

                  {/* Topic label: rendered with search match highlighting
                      when a query is active, or as plain text otherwise */}
                  <span className="truncate">
                    <HighlightedLabel label={label} query={searchQuery} />
                  </span>
                </button>
              ))
            ) : (
              /* Empty state: shown when the search query matches no topics.
                 Provides a visual cue that the filter returned zero results
                 and encourages the user to modify their search. */
              <div className="flex flex-col items-center justify-center py-8 text-center">
                <Search size={24} className="text-content-tertiary mb-2 opacity-50" />
                <p className="text-xs text-content-tertiary">{t('help.noResults')}</p>
                <button
                  onClick={handleClearSearch}
                  className="
                    mt-2 text-xs text-accent
                    hover:text-accent-hover
                    transition-colors
                  "
                >
                  {t('help.clearSearch')}
                </button>
              </div>
            )}
          </div>
        </nav>

        {/* ----------------------------------------------------------------
         * Markdown content viewer
         * Displays the full markdown content of the currently selected
         * help topic. Uses react-markdown with the remark-gfm plugin
         * to support GitHub-flavored markdown features such as tables,
         * strikethrough, and task lists. The prose classes provide
         * typographic styling with dark mode support.
         * ---------------------------------------------------------------- */}
        <div className="flex-1 overflow-y-auto p-6">
          {/*
           * Language notice -- deliberately its OWN small block, sitting
           * above and outside the `.prose` markdown container below, not
           * inside it. If this text were rendered as part of the
           * markdown, a reader could mistake it for something the help
           * page itself says; keeping it a separate, quietly-styled
           * element makes clear it's a note from the app about the
           * page, not a line the page's author wrote.
           *
           * Two different situations get two different messages here:
           *   - The page DOES have a translation for the current
           *     language: say so, and point back to English as the
           *     version the developers actually wrote (in case
           *     something reads oddly).
           *   - The page does NOT have one yet: say so plainly, and
           *     name the person's own language (not just "it's in
           *     English") so they understand why they're reading
           *     English on a screen they set to something else.
           * Nothing is shown at all while the active language is
           * English -- there is no translation situation to report.
           */}
          {isTranslatedLanguage && (
            <p className="text-xs text-content-tertiary italic mb-3">
              {hasTranslation
                ? t('help.machineTranslatedPage')
                : t('help.pageNotTranslated', { language: activeLanguageName })}
            </p>
          )}

          <div className="prose prose-sm dark:prose-invert max-w-none">
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              rehypePlugins={[rehypeRaw, [rehypeSanitize, helpSanitizeSchema]]}
              components={{
                // Custom link handler. Help pages carry three different
                // kinds of link, and each needs different handling inside
                // a Tauri WebView (which has no browser chrome to fall
                // back on -- a navigation that "fails" just leaves the
                // user staring at a blank window):
                //
                //  1. `#topic-id` -- an anchor naming another help topic
                //     by id. Handled within the viewer: switches the
                //     active topic instead of scrolling (there's nothing
                //     to scroll to; the target topic isn't on screen).
                //  2. `some-page.md` / `./some-page.md`, optionally with
                //     a `#fragment` -- a relative link from one help page
                //     to another, written the way it needs to be for the
                //     pages to also read correctly as plain files on
                //     GitHub. This is the fix this handler exists for:
                //     previously these were left to the browser's default
                //     handling, which in a WebView means navigating the
                //     entire app window to a `help/some-page.md` URL that
                //     doesn't resolve to anything -- the app itself
                //     disappears, replaced by a blank page, with no way
                //     back except restarting it. That bug is real today:
                //     the "Audio Codecs" page links to `wrapper.md`. Any
                //     trailing `#fragment` is ignored -- the renderer
                //     doesn't currently give headings ids to jump to, so
                //     there is nowhere for it to jump.
                //  3. `http://` / `https://` -- opened in the user's
                //     normal web browser, not inside the app.
                //
                // Everything else (a handful of pages link to source
                // files in the repo, e.g. `../src-tauri/tags.toml`, for
                // someone reading on GitHub) falls through to a final
                // catch-all that blocks the default navigation. That
                // catch-all is deliberately unconditional: it is the
                // guarantee that no link, known or not-yet-written, can
                // ever navigate the WebView away from the app.
                a: ({ href, children, ...props }) => (
                  <a
                    {...props}
                    href={href}
                    onClick={(e) => {
                      if (!href) return;
                      // 1. Internal help topic anchor (e.g., #cookie-management)
                      if (href.startsWith('#')) {
                        e.preventDefault();
                        const topicId = href.slice(1);
                        const target = HELP_TOPICS.find((t) => t.id === topicId);
                        if (target) {
                          setActiveTopic(target.id);
                        }
                        return;
                      }
                      // 2. Relative link to another help page, e.g.
                      // "cookie-management.md" or "./cookie-management.md",
                      // optionally with a "#fragment" that we ignore.
                      const pageLink = href.match(/^(?:\.\/)?([a-z0-9-]+)\.md(?:#[\w-]*)?$/);
                      if (pageLink) {
                        e.preventDefault();
                        const target = HELP_TOPICS.find((t) => t.id === pageLink[1]);
                        if (target) {
                          setActiveTopic(target.id);
                        }
                        return;
                      }
                      // 3. External link — open in system default browser
                      if (href.startsWith('http://') || href.startsWith('https://')) {
                        e.preventDefault();
                        import('@tauri-apps/plugin-shell')
                          .then(({ open }) => open(href))
                          .catch(() => {});
                        return;
                      }
                      // Catch-all: anything not handled above (e.g. a
                      // link into the repo's source tree) is inert rather
                      // than left to navigate the WebView away from the
                      // app -- see the block comment above this handler.
                      e.preventDefault();
                    }}
                  >
                    {children}
                  </a>
                ),
              }}
            >
              {markdown}
            </ReactMarkdown>
          </div>
        </div>
      </div>
    </div>
  );
}
