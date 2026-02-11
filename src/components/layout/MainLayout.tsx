// Copyright (c) 2024-2026 MWBM Partners Ltd
/**
 * @file Main application layout component.
 *
 * Assembles the full app shell: sidebar navigation, custom title bar
 * (Windows/Linux), main content area with page routing, and status bar.
 * The content area renders the appropriate page based on the UI store's
 * `currentPage` value.
 *
 * Visual structure (top to bottom, left to right):
 * ┌──────────────────────────────────────────────────┐
 * │               TitleBar (Win/Linux)               │
 * ├──────────┬───────────────────────────────────────┤
 * │          │          <main> (scrollable)          │
 * │ Sidebar  │   children (active page component)    │
 * │          │                                       │
 * │          ├───────────────────────────────────────┤
 * │          │             StatusBar                 │
 * ├──────────┴───────────────────────────────────────┤
 * │          ToastContainer (overlay, top-right)     │
 * └──────────────────────────────────────────────────┘
 *
 * The `children` prop receives whichever page component the App-level
 * router has selected (DownloadForm, DownloadQueue, SettingsPage, etc.).
 * This follows the standard React "composition" pattern where a layout
 * component accepts arbitrary children.
 *
 * @see https://react.dev/learn/passing-props-to-a-component#passing-jsx-as-children
 *      React docs -- passing JSX as the `children` prop.
 * @see https://tailwindcss.com/docs/flex  Tailwind flex utilities used here.
 *
 * Related components:
 *  - {@link Sidebar}        -- left navigation panel (from ./Sidebar)
 *  - {@link TitleBar}       -- custom window chrome (from ./TitleBar)
 *  - {@link StatusBar}      -- download-count footer (from ./StatusBar)
 *  - {@link ToastContainer} -- toast notification overlay (from @/components/common)
 */

/**
 * React's `ReactNode` type -- the broadest type for anything renderable
 * (elements, strings, numbers, fragments, portals, null, etc.).
 * @see https://react.dev/reference/react/ReactNode
 */
import type { ReactNode } from 'react';

/** Sibling layout components assembled into the shell. */
import { Sidebar } from './Sidebar';
import { TitleBar } from './TitleBar';
import { StatusBar } from './StatusBar';

/** Toast notification overlay rendered outside the normal document flow. */
import { ToastContainer } from '@/components/common';

/**
 * Props for the {@link MainLayout} component.
 *
 * Uses React's `ReactNode` for maximum flexibility -- the parent can pass
 * any valid JSX (single element, fragment, array, null, etc.).
 *
 * @see https://react.dev/learn/passing-props-to-a-component#passing-jsx-as-children
 */
interface MainLayoutProps {
  /**
   * The active page component to render in the scrollable main area.
   * This is typically one of: DownloadForm, DownloadQueue, SettingsPage, or HelpPage,
   * selected by the App-level page router based on `useUiStore.currentPage`.
   */
  children: ReactNode;
}

/**
 * Root layout shell for the entire application window.
 *
 * Renders five distinct regions:
 *  1. **TitleBar** -- Custom window chrome (minimize / maximize / close)
 *     rendered only on Windows & Linux; macOS uses the native traffic-light
 *     buttons via `titleBarStyle: 'overlay'` in `tauri.conf.json`.
 *  2. **Sidebar** -- Left-hand navigation panel that links to each
 *     application page. Collapsible to icon-only mode.
 *  3. **<main>** -- Scrollable content area where `children` (the active
 *     page) is injected. `overflow-y-auto` allows vertical scrolling when
 *     content exceeds the viewport.
 *  4. **StatusBar** -- Fixed footer displaying active download counts,
 *     queued items, and the application version string.
 *  5. **ToastContainer** -- Absolutely-positioned overlay that renders
 *     transient notification toasts (success, error, info, warning).
 *
 * Layout mechanics (Tailwind CSS):
 *  - Outer `div`: `flex flex-col h-screen` -- fills the entire Tauri
 *    webview and stacks TitleBar, body, and ToastContainer vertically.
 *  - Body row: `flex flex-1 overflow-hidden` -- Sidebar and content sit
 *    side-by-side; `overflow-hidden` prevents the body from scrolling so
 *    only the inner `<main>` scrolls.
 *  - Content column: `flex-1 flex flex-col overflow-hidden` -- the main
 *    area takes all remaining width; `<main>` grows to fill and scrolls,
 *    while StatusBar stays pinned at the bottom.
 *
 * @param children - The active page component to render in the content area.
 *
 * @see https://tailwindcss.com/docs/overflow      -- overflow utilities
 * @see https://tailwindcss.com/docs/flex           -- flex utilities
 * @see https://tailwindcss.com/docs/height#screen  -- h-screen
 */
export function MainLayout({ children }: MainLayoutProps) {
  return (
    /**
     * Root container: full-viewport column layout.
     * `h-screen` ensures it exactly fills the Tauri webview height.
     * @see https://tailwindcss.com/docs/height#screen
     */
    <div className="flex flex-col h-screen">
      {/*
       * Custom title bar for Windows/Linux (hidden on macOS).
       * On macOS the native title bar is used because `titleBarStyle`
       * is set to "overlay" in tauri.conf.json, which renders the
       * traffic-light buttons on top of the webview content.
       * @see TitleBar component in ./TitleBar.tsx
       */}
      <TitleBar />

      {/*
       * Main body row: sidebar (fixed width) + content column (flex-1).
       * `overflow-hidden` is critical here -- it prevents the body row
       * from scrolling and delegates scroll responsibility to the inner
       * <main> element only.
       */}
      <div className="flex flex-1 overflow-hidden">
        {/*
         * Left sidebar navigation panel.
         * Width transitions between 64px (collapsed / icon-only) and
         * 224px (expanded / labels visible), driven by uiStore.sidebarCollapsed.
         * @see Sidebar component in ./Sidebar.tsx
         */}
        <Sidebar />

        {/*
         * Right content column containing the page content and status bar.
         * `flex-1` absorbs all remaining horizontal space after the sidebar.
         * `flex flex-col` stacks <main> on top of StatusBar.
         * `overflow-hidden` prevents this column itself from scrolling.
         */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/*
           * Scrollable page content area.
           * `flex-1` makes it grow to fill the column; `overflow-y-auto`
           * adds a vertical scrollbar only when content overflows.
           * The `children` prop is injected here -- it is the currently
           * active page component (e.g., DownloadForm, DownloadQueue).
           * @see https://react.dev/learn/passing-props-to-a-component#passing-jsx-as-children
           */}
          <main className="flex-1 overflow-y-auto">{children}</main>

          {/*
           * Bottom status bar -- pinned below the scrollable <main>.
           * Displays download activity counters and the app version.
           * @see StatusBar component in ./StatusBar.tsx
           */}
          <StatusBar />
        </div>
      </div>

      {/*
       * Toast notification overlay -- positioned fixed in the top-right
       * corner of the viewport. Renders above all other content using
       * a high z-index. Toasts are managed by useUiStore.toasts[].
       * @see ToastContainer component in @/components/common
       */}
      <ToastContainer />
    </div>
  );
}
