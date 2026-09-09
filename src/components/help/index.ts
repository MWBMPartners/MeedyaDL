/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file help/index.ts -- Barrel export for help components.
 *
 * Re-exports the top-level {@link HelpViewer} component so that other parts
 * of the application can import it from `@/components/help` without reaching
 * into internal module paths.
 *
 * The HelpViewer is rendered when the user navigates to the "Help" page via
 * the application sidebar. It provides a searchable, sidebar-navigated
 * documentation viewer that renders the Markdown pages in `help/*.md`
 * (read and prepared by `./helpTopics.ts` -- see that file for how and why).
 *
 * `HELP_TOPICS` and `HelpTopicId` are also re-exported here so that other
 * components (e.g. a `HelpButton` that deep-links to a specific topic)
 * never need to import `./helpTopics` directly -- everything a caller
 * needs from the help feature comes from this one barrel file.
 *
 * Usage:
 * ```ts
 * import { HelpViewer, HELP_TOPICS, type HelpTopicId } from '@/components/help';
 * ```
 *
 * @see {@link ./HelpViewer.tsx} -- The help viewer with search and Markdown rendering
 * @see {@link ./helpTopics.ts} -- Where help content is loaded from `help/*.md`
 */

export { HelpViewer } from './HelpViewer';
export { HELP_TOPICS, type HelpTopicId } from './helpTopics';
