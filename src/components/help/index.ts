/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
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
 * documentation viewer that renders inline Markdown help content.
 *
 * Usage:
 * ```ts
 * import { HelpViewer } from '@/components/help';
 * ```
 *
 * @see {@link ./HelpViewer.tsx} -- The help viewer with search and Markdown rendering
 */

export { HelpViewer } from './HelpViewer';
