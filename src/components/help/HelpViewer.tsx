/**
 * Copyright (c) 2024-2026 MeedyaDL
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
 * ## Help Topics
 *
 * Topics are defined as a static `HELP_TOPICS` array of `HelpTopic`
 * objects, each containing an ID, label, icon, and inline Markdown string.
 * Topics cover: Getting Started, Downloading, Settings, Cookies, Tools,
 * Audio Codecs, Music Videos, Troubleshooting, and About.
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
import { useState, useMemo, useCallback } from 'react';

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

// Lucide icons for each help topic in the sidebar.
// Each topic has a dedicated icon for quick visual identification.
import {
  BookOpen,     // "Getting Started" topic
  Download,     // "Downloading" topic
  Settings,     // "Settings" topic
  Cookie,       // "Cookies" topic
  Wrench,       // "Tools" topic
  Music,        // "Audio Codecs" topic
  Video,        // "Music Videos" topic
  HelpCircle,   // "Troubleshooting" topic
  FileText,     // "About" topic
  ShieldAlert,  // "Disclaimer" topic
  Search,       // Search icon in the sidebar search bar
  X,            // Clear search button icon
} from 'lucide-react';

// Shared layout component for the page header.
import { PageHeader } from '@/components/layout';

/**
 * Shape of a single help topic entry.
 *
 * @property id      - Unique identifier used for the React `key` prop and
 *                     for tracking the active topic in component state.
 * @property label   - Short display name shown in the sidebar navigation.
 *                     Also searched when the user types in the search bar.
 * @property icon    - Lucide icon component rendered next to the label in
 *                     the sidebar. Typed as `typeof BookOpen` (all Lucide
 *                     icons share the same component signature).
 * @property content - Full Markdown content string rendered in the viewer
 *                     pane when this topic is selected. Also searched
 *                     when the user types in the search bar.
 */
interface HelpTopic {
  id: string;
  label: string;
  icon: typeof BookOpen;
  content: string;
}

/**
 * Static array of all built-in help topics.
 *
 * Each topic contains inline Markdown content rather than loading from
 * external files. This approach keeps help content bundled with the
 * application and eliminates the need for async file loading.
 *
 * Topics are displayed in the sidebar in the order they appear in this
 * array. The order is intentional: Getting Started and Downloading come
 * first as the most common entry points, followed by reference material
 * (Settings, Cookies, Tools, Codecs, Videos), troubleshooting, and About.
 */
const HELP_TOPICS: HelpTopic[] = [
  {
    id: 'getting-started',
    label: 'Getting Started',
    icon: BookOpen,
    content: `# Getting Started

## Welcome to MeedyaDL

MeedyaDL is a media downloader application for downloading music and videos. This guide will help you get started.

### First-Time Setup

When you first launch the app, you'll be guided through a setup wizard that:

1. **Installs Python** - A portable Python runtime is downloaded (no system changes)
2. **Installs GAMDL** - The download tool is installed into the portable Python
3. **Installs Tools** - Required tools like FFmpeg are downloaded automatically
4. **Imports Cookies** - You provide your Apple Music authentication cookies

### Downloading Music

1. Copy an Apple Music URL from your browser or the Apple Music app
2. Paste it into the URL field on the Download page
3. (Optional) Adjust quality settings using the override panel
4. Click **Add to Queue**
5. Monitor progress on the Queue page

### Supported Content Types

- **Songs** - Individual tracks
- **Albums** - Complete albums with all tracks
- **Playlists** - User or editorial playlists
- **Music Videos** - Music videos in up to 4K
- **Artist Pages** - Downloads the artist's top songs`,
  },
  {
    id: 'downloading',
    label: 'Downloading',
    icon: Download,
    content: `# Downloading

## How to Download

### Supported URLs

GAMDL supports the following Apple Music URL formats:

- \`https://music.apple.com/{country}/album/{name}/{id}\`
- \`https://music.apple.com/{country}/album/{name}/{id}?i={track_id}\`
- \`https://music.apple.com/{country}/playlist/{name}/{id}\`
- \`https://music.apple.com/{country}/music-video/{name}/{id}\`
- \`https://music.apple.com/{country}/artist/{name}/{id}\`

### Quality Overrides

By default, downloads use the settings from the Quality settings tab. You can override the codec and resolution for individual downloads using the "Quality Overrides" panel on the Download page.

### Fallback Chain

When the preferred codec or resolution is unavailable, GAMDL automatically tries the next option in the fallback chain. Configure the chain order in **Settings > Fallback**.`,
  },
  {
    id: 'settings-help',
    label: 'Settings',
    icon: Settings,
    content: `# Settings

## Configuration Guide

### General
- **Output Directory** - Where files are saved (default: ~/Music/Apple Music)
- **Language** - Metadata language preference
- **Overwrite** - Whether to replace existing files

### Quality
- **Audio Codec** - Default: ALAC (lossless). Options range from lossless to compressed AAC variants
- **Video Resolution** - Default: 2160p (4K). Falls back to lower resolutions if unavailable
- **Fallback** - Enable/disable automatic fallback when preferred quality isn't available

### Paths
Override paths to external tools. Leave empty to use the managed (auto-installed) versions.

### Templates
Customize how files and folders are named using template variables like \`{artist}\`, \`{album}\`, \`{title}\`, \`{track:02d}\`.`,
  },
  {
    id: 'cookies-help',
    label: 'Cookies',
    icon: Cookie,
    content: `# Cookie Authentication

## Why Cookies Are Needed

Apple Music requires authentication to access content. GAMDL uses browser cookies from your Apple Music subscription to authenticate download requests.

## How to Export Cookies

1. Install a **cookies.txt** browser extension:
   - Chrome: "Get cookies.txt LOCALLY" extension
   - Firefox: "cookies.txt" extension
2. Go to **music.apple.com** and log in with your Apple ID
3. Click the extension icon and choose **Export** or **Download**
4. Save the file somewhere accessible

## Importing Cookies

1. Go to **Settings > Cookies** or use the Setup Wizard
2. Click **Browse** and select your cookies.txt file
3. Click **Validate Cookies** to verify they work
4. Save your settings

## Cookie Expiry

Cookies expire after some time. If downloads start failing with authentication errors, export fresh cookies from your browser.`,
  },
  {
    id: 'tools',
    label: 'Tools',
    icon: Wrench,
    content: `# External Tools

## Required Tools

All four tools below are required for full functionality.

### FFmpeg
Used for audio/video processing and container remuxing. Required for most download operations.

### mp4decrypt
Part of the Bento4 toolkit. Used for decrypting DRM-protected streams. Essential for downloading protected content.

### N_m3u8DL-RE
HLS/DASH stream downloader. Used for downloading segmented media streams.

### MP4Box
Part of the GPAC toolkit. Used for MP4 muxing and remuxing operations.

## Installation

Tools are automatically downloaded during first-time setup. You can also install or update them from **Settings > Paths** or by re-running the setup wizard.`,
  },
  {
    id: 'audio-codecs',
    label: 'Audio Codecs',
    icon: Music,
    content: `# Audio Codecs

## Available Codecs

### Lossless
- **ALAC** - Apple Lossless Audio Codec. CD quality (16-bit/44.1kHz) or Hi-Res (24-bit/up to 192kHz)

### Spatial Audio
- **Atmos** - Dolby Atmos spatial audio tracks
- **AC3** - Dolby Digital surround sound

### AAC Variants
- **AAC** - 256kbps at up to 48kHz (standard quality)
- **AAC Binaural** - 256kbps binaural rendering of spatial tracks
- **AAC Legacy** - 256kbps at up to 44.1kHz
- **AAC-HE** - High Efficiency AAC (experimental)
- **AAC Downmix** - Downmixed from spatial (experimental)

## Recommendation

For best quality, use **ALAC** (lossless) with the fallback chain enabled. This ensures you get the highest quality available, falling back to compressed formats only when lossless isn't available.`,
  },
  {
    id: 'video',
    label: 'Music Videos',
    icon: Video,
    content: `# Music Videos

## Video Quality

Available resolutions (highest to lowest):
- **2160p** (4K Ultra HD)
- **1440p** (QHD)
- **1080p** (Full HD)
- **720p** (HD)
- **540p** (qHD)
- **480p** (SD)
- **360p** (Low)
- **240p** (Lowest)

## Video Codecs

Configure codec priority in **Settings > Quality**:
- **H.265/HEVC** - Better quality at smaller file sizes (recommended)
- **H.264/AVC** - More compatible, larger file sizes

## Remux Format

Choose the container format in **Settings > Quality**:
- **M4V** - Apple standard format
- **MP4** - Universal compatibility
- **MKV** - Matroska (supports more features)`,
  },
  {
    id: 'troubleshooting',
    label: 'Troubleshooting',
    icon: HelpCircle,
    content: `# Troubleshooting

## Common Issues

### "Authentication Failed" / Cookie Errors
- Your cookies may have expired. Export fresh cookies from your browser
- Make sure you're logged into music.apple.com before exporting
- Verify cookies using **Settings > Cookies > Validate**

### "Codec Not Available"
- Not all tracks are available in all codecs
- Enable the fallback chain in **Settings > Quality**
- ALAC (lossless) has the widest availability

### Downloads Stuck at 0%
- Check your internet connection
- Try cancelling and re-adding the download
- Check if FFmpeg is properly installed in **Settings > Paths**

### "Tool Not Found" Errors
- Re-run the setup wizard from **Settings > General**
- Or manually set tool paths in **Settings > Paths**

### Application Won't Start
- Delete the settings file from the app data directory and restart
- On macOS: ~/Library/Application Support/io.github.meedyadl/
- On Windows: %APPDATA%/io.github.meedyadl/
- On Linux: ~/.config/io.github.meedyadl/`,
  },
  {
    id: 'disclaimer',
    label: 'Disclaimer',
    icon: ShieldAlert,
    content: `# Disclaimer

## Important Notice

MeedyaDL is provided "as is" without warranty of any kind, express or implied.

### Third-Party Dependencies

MeedyaDL relies on several third-party libraries and services to function, including but not limited to:

- **GAMDL** — the core download engine
- **Python** — runtime environment for GAMDL
- **FFmpeg, mp4decrypt, N_m3u8DL-RE, MP4Box** — media processing tools

These are independent projects maintained by their respective developers. Changes to these projects may affect MeedyaDL's functionality.

### No Guarantees

- Quality of service, features, and performance are **not guaranteed**
- Third-party services may change, become unavailable, or cease to function at any time
- While we endeavour to provide updates and fixes, this **cannot be guaranteed**
- The developers accept **no liability** for loss of functionality, data, or service

### Your Responsibility

By using MeedyaDL, you acknowledge and accept that:

- You are responsible for complying with all applicable laws and terms of service
- Downloaded content is for personal use in accordance with your existing subscriptions
- The developers are not responsible for how the software is used

### License

MeedyaDL is licensed under the MIT License. See the LICENSE file for full details.`,
  },
  {
    id: 'about',
    label: 'About',
    icon: FileText,
    content: `# About MeedyaDL

## Version
v0.3.5

## Credits
- **GAMDL** by glomatico - The Apple Music download engine
- **Tauri** - Cross-platform desktop framework
- Built with React, TypeScript, and Rust

## License
Copyright (c) 2024-2026 MeedyaDL
Licensed under the MIT License.

## Links
- GitHub: github.com/MeedyaDL/MeedyaDL
- GAMDL: github.com/glomatico/gamdl`,
  },
];

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
function HighlightedLabel({
  label,
  query,
}: {
  label: string;
  query: string;
}) {
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
          <mark
            key={index}
            className="bg-yellow-300/40 text-inherit rounded-sm px-0.5"
          >
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
  const [activeTopic, setActiveTopic] = useState('getting-started');

  /** Tracks the current search input value for filtering the sidebar topics */
  const [searchQuery, setSearchQuery] = useState('');

  /**
   * Determine the platform-appropriate modifier key label once.
   * On macOS we show the Cmd symbol; on other platforms we show "Ctrl".
   * This is memoized because isMacPlatform() accesses navigator, and
   * we only need to evaluate it once per component mount.
   */
  const modifierKey = useMemo(() => (isMacPlatform() ? '\u2318' : 'Ctrl'), []);

  /**
   * Filters HELP_TOPICS based on the current searchQuery.
   *
   * Matching logic:
   * - If the query is empty or whitespace-only, all topics are returned.
   * - Otherwise, a topic matches if the query appears anywhere in its
   *   label OR its markdown content (case-insensitive).
   *
   * The result is memoized so the filter only re-runs when the search
   * query actually changes, avoiding unnecessary array iterations on
   * every render.
   */
  const filteredTopics = useMemo(() => {
    const trimmed = searchQuery.trim().toLowerCase();

    /* No query: return the full topic list unfiltered */
    if (!trimmed) {
      return HELP_TOPICS;
    }

    /* Filter topics whose label or content contains the query substring */
    return HELP_TOPICS.filter(
      (topic) =>
        topic.label.toLowerCase().includes(trimmed) ||
        topic.content.toLowerCase().includes(trimmed)
    );
  }, [searchQuery]);

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
  const handleTopicSelect = useCallback((topicId: string) => {
    setActiveTopic(topicId);
  }, []);

  /**
   * Handles changes to the search input.
   * Updates the searchQuery state which triggers re-filtering
   * of the sidebar topics via the filteredTopics memo.
   */
  const handleSearchChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setSearchQuery(e.target.value);
    },
    []
  );

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

  return (
    <div className="flex flex-col h-full">
      {/* Page header with title and description */}
      <PageHeader
        title="Help"
        subtitle="Documentation and guides for using MeedyaDL"
      />

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
                placeholder="Search topics..."
                aria-label="Search help topics"
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
                    aria-label="Clear search"
                    title="Clear search"
                  >
                    <X size={12} />
                  </button>
                ) : (
                  /* Keyboard shortcut hint: shown when the input is empty.
                     Displays Cmd+K on macOS or Ctrl+K on other platforms.
                     This is a visual placeholder for future keyboard
                     shortcut support (the actual shortcut handler is not
                     yet implemented). */
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
                    title={`${modifierKey}+K to focus search (coming soon)`}
                    aria-label={`Keyboard shortcut: ${modifierKey} plus K (coming soon)`}
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
                {filteredTopics.length === 1
                  ? '1 result'
                  : `${filteredTopics.length} results`}
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
                <Search
                  size={24}
                  className="text-content-tertiary mb-2 opacity-50"
                />
                <p className="text-xs text-content-tertiary">
                  No matching topics found.
                </p>
                <button
                  onClick={handleClearSearch}
                  className="
                    mt-2 text-xs text-accent
                    hover:text-accent-hover
                    transition-colors
                  "
                >
                  Clear search
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
          <div className="max-w-2xl prose prose-sm dark:prose-invert">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>
              {topic.content}
            </ReactMarkdown>
          </div>
        </div>
      </div>
    </div>
  );
}
