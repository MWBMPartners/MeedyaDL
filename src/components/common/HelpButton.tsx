// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Contextual help button that deep-links to a Help page topic.
 *
 * Renders a small "?" icon button inline next to settings labels. On click,
 * it navigates to the Help page and auto-selects the corresponding topic.
 *
 * Uses the `navigateToHelp` action from `uiStore` to set both the current
 * page and the deep-link topic in a single state update.
 *
 * @see {@link @/stores/uiStore.ts}         -- navigateToHelp action
 * @see {@link @/components/help/HelpViewer.tsx} -- consumes helpActiveTopic
 */

import { HelpCircle } from 'lucide-react';
import { useUiStore } from '@/stores/uiStore';
import type { HelpTopicId } from '@/components/help';

/**
 * Fix 7 (a11y audit): plain-English labels for every help topic, used
 * below to build this button's accessible name when no `tooltip` prop
 * is given (e.g. "Help: Audio Codecs" instead of the old fallback,
 * "Help: audio-codecs" -- the raw page id).
 *
 * This intentionally duplicates the label text that also lives in
 * `HELP_TOPIC_MANIFEST` (`helpTopics.ts`) rather than importing it
 * from there. Reaching into that file -- or its barrel,
 * `@/components/help`, which re-exports the full `HelpViewer`
 * component alongside it -- as a VALUE (not just the `HelpTopicId`
 * type, which TypeScript erases entirely and costs nothing) would
 * drag in every Lucide icon the Help sidebar uses and `HelpViewer`
 * itself into every settings field that renders a `<HelpButton>` --
 * there are dozens of them. That's exactly what broke
 * `SettingsTabs.test.tsx`'s lucide-react mock the first time this was
 * tried (an unrelated Sidebar icon, three modules deep through that
 * import chain, had no test stub). Typing this as `Record<HelpTopicId,
 * string>` turns the duplication into a compile-time guarantee rather
 * than a silent risk: `HelpTopicId` is a closed union derived from
 * `HELP_TOPIC_MANIFEST`, so adding a help page without adding its
 * label here fails the build instead of quietly falling back to the
 * raw id again.
 */
const HELP_TOPIC_LABELS: Record<HelpTopicId, string> = {
  'getting-started': 'Getting Started',
  'downloading-music': 'Downloading Music',
  'downloading-videos': 'Downloading Videos',
  'audio-codecs': 'Audio Codecs',
  'quality-settings': 'Quality Settings',
  'fallback-quality': 'Fallback Quality',
  'lyrics-and-metadata': 'Lyrics & Metadata',
  'metadata-mapping': 'Metadata Mapping',
  'cookie-management': 'Cookies',
  wrapper: 'Wrapper',
  'animated-artwork': 'Animated Artwork',
  settings: 'Settings',
  tools: 'Tools',
  'supported-services': 'Supported Services',
  'release-channels': 'Release Channels',
  'keyboard-shortcuts': 'Keyboard Shortcuts',
  troubleshooting: 'Troubleshooting',
  faq: 'FAQ',
  disclaimer: 'Disclaimer',
  about: 'About',
};

/**
 * Props for the {@link HelpButton} component.
 */
interface HelpButtonProps {
  /**
   * Which help page to open. Typed against `HelpTopicId` -- the real,
   * loaded set of `help/*.md` pages -- so a typo or a renamed page is a
   * TypeScript error at build time, not a click that silently does
   * nothing (which is exactly what used to happen when this was a plain
   * `string`: nothing stopped you writing a page id that no longer
   * existed).
   */
  topic: HelpTopicId;
  /** Optional tooltip text shown on hover */
  tooltip?: string;
}

/**
 * Small inline "?" button that navigates to the Help page with a specific
 * topic pre-selected. Designed to sit next to form labels in settings tabs.
 *
 * @example
 * ```tsx
 * <HelpButton topic="cookie-management" tooltip="Learn about cookie authentication" />
 * ```
 */
export function HelpButton({ topic, tooltip }: HelpButtonProps) {
  const navigateToHelp = useUiStore((s) => s.navigateToHelp);

  /** The page's real title (see `HELP_TOPIC_LABELS` above), falling back to the raw id just in case. */
  const topicLabel = HELP_TOPIC_LABELS[topic] ?? topic;

  return (
    <button
      type="button"
      onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        navigateToHelp(topic);
      }}
      className="
        inline-flex items-center justify-center
        w-6 h-6 rounded-full
        text-content-tertiary
        hover:text-accent hover:bg-accent-light
        transition-colors
        cursor-pointer
        flex-shrink-0
      "
      /* Fix 14 (a11y audit): w-4 h-4 (16px) was under the 24x24
       * CSS-pixel minimum touch target (WCAG 2.5.8). This button sits
       * inline next to text labels throughout the settings pages, so
       * bumping it to 24px (w-6 h-6) doesn't disturb any layout --
       * the icon inside stays the same visual size. */
      title={tooltip ?? 'View help'}
      aria-label={tooltip ?? `Help: ${topicLabel}`}
    >
      <HelpCircle size={14} />
    </button>
  );
}
