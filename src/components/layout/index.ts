// Copyright (c) 2024-2026 MeedyaDL
/**
 * @file Barrel export for layout components.
 *
 * Re-exports all layout-level components from a single entry point so that
 * consumers can import them with a short path:
 *
 *   import { MainLayout, Sidebar, PageHeader } from '@/components/layout';
 *
 * This barrel pattern keeps import statements concise and decouples
 * consumers from the internal file structure of the layout directory.
 * If a layout component is renamed or moved to a different file, only
 * this barrel needs to be updated -- all external imports remain stable.
 *
 * Exported components and their roles:
 *  - {@link MainLayout}  - Root shell: sidebar + title bar + content + status bar + toasts.
 *  - {@link Sidebar}     - Left-hand navigation panel with page links & collapse toggle.
 *  - {@link TitleBar}    - Custom window chrome (minimize / maximize / close) for Windows & Linux.
 *  - {@link StatusBar}   - Thin bar at the bottom displaying download counts and app version.
 *  - {@link PageHeader}  - Consistent page-level heading with optional subtitle and action slot.
 *
 * @see https://www.typescriptlang.org/docs/handbook/modules.html#re-exports
 *      TypeScript re-export documentation.
 */

/** Root application layout (sidebar + content area + status bar). */
export { MainLayout } from './MainLayout';

/** Collapsible left-side navigation sidebar with page links. */
export { Sidebar } from './Sidebar';

/** Custom window title bar with minimize / maximize / close for non-macOS platforms. */
export { TitleBar } from './TitleBar';

/** Bottom status bar showing active downloads, queue counts, and version. */
export { StatusBar } from './StatusBar';

/** Reusable page-level heading with optional subtitle and action buttons. */
export { PageHeader } from './PageHeader';
