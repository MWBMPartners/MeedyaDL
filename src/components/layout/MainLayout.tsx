// Copyright (c) 2026 MeedyaSuite
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
 * │          │    UpdateBanner (pinned, non-scroll)   │
 * │          ├───────────────────────────────────────┤
 * │ Sidebar  │          <main> (scrollable)          │
 * │          │   children (active page component)    │
 * │          │                                       │
 * │          ├───────────────────────────────────────┤
 * │          │   GlobalProgressBar (per-item + queue) │
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
 * `useState` and `useRef` -- React hooks for local state and mutable refs.
 * `useCallback` -- memoised callback to avoid re-creating drag event handlers.
 * @see https://react.dev/reference/react/ReactNode
 * @see https://react.dev/reference/react/useState
 * @see https://react.dev/reference/react/useRef
 */
import { type ReactNode, useState, useRef, useCallback } from 'react';

/** Sibling layout components assembled into the shell. */
import { GlobalProgressBar } from './GlobalProgressBar';
import { Sidebar } from './Sidebar';
import { TitleBar } from './TitleBar';
import { StatusBar } from './StatusBar';

/** Toast notification overlay rendered outside the normal document flow. */
/** UpdateBanner: dismissible notification shown when updates are available. */
import { ToastContainer, UpdateBanner, FeatureNoticeBanner } from '@/components/common';
import { ShortcutsHelpDialog } from '@/components/common/ShortcutsHelpDialog';

/** URL parser to validate dropped Apple Music URLs. */
import { parseAppleMusicUrl } from '@/lib/url-parser';

/** Zustand stores for UI navigation and download URL input. */
import { useUiStore } from '@/stores/uiStore';
import { useDownloadStore } from '@/stores/downloadStore';

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
 * Renders six distinct regions:
 *  1. **TitleBar** -- Custom window chrome (minimize / maximize / close)
 *     rendered only on Windows & Linux; macOS uses the native traffic-light
 *     buttons via `titleBarStyle: 'overlay'` in `tauri.conf.json`.
 *  2. **Sidebar** -- Left-hand navigation panel that links to each
 *     application page. Collapsible to icon-only mode.
 *  3. **UpdateBanner** -- Dismissible notification banner shown when app
 *     or component updates are available. Rendered above the scrollable
 *     content area so it stays visible regardless of page scroll position.
 *  4. **<main>** -- Scrollable content area where `children` (the active
 *     page) is injected. `overflow-y-auto` allows vertical scrolling when
 *     content exceeds the viewport.
 *  5. **StatusBar** -- Fixed footer displaying active download counts,
 *     queued items, and the application version string.
 *  6. **ToastContainer** -- Absolutely-positioned overlay that renders
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
  // ---------------------------------------------------------------------------
  // Drag-and-drop state
  // ---------------------------------------------------------------------------

  /**
   * Whether a drag operation is currently hovering over the content area.
   * Controls visibility of the drop-zone overlay.
   */
  const [isDragOver, setIsDragOver] = useState(false);

  /**
   * Counter for nested `dragenter`/`dragleave` events.
   *
   * The browser fires `dragenter` when the cursor enters any child element,
   * paired with a `dragleave` on the element being exited. Without a counter,
   * the overlay would flicker as the cursor moves between child elements.
   * We increment on `dragenter`, decrement on `dragleave`, and only hide
   * the overlay when the counter reaches zero (cursor has fully left the
   * content area).
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/HTML_Drag_and_Drop_API#drag_events
   */
  const dragCounterRef = useRef(0);

  /** UI store actions for navigation and toast notifications. */
  const setPage = useUiStore((s) => s.setPage);
  const addToast = useUiStore((s) => s.addToast);

  /** Download store action to set the URL input field programmatically. */
  const setUrlInput = useDownloadStore((s) => s.setUrlInput);

  // ---------------------------------------------------------------------------
  // Drag event handlers
  // ---------------------------------------------------------------------------

  /**
   * Handle `dragenter` on the content area.
   * Increments the nested-element counter and shows the overlay.
   */
  const handleDragEnter = useCallback((e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    dragCounterRef.current += 1;
    if (dragCounterRef.current === 1) {
      setIsDragOver(true);
    }
  }, []);

  /**
   * Handle `dragleave` on the content area.
   * Decrements the nested-element counter; hides overlay when counter
   * reaches zero (cursor has fully exited the drop target).
   */
  const handleDragLeave = useCallback((e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    dragCounterRef.current -= 1;
    if (dragCounterRef.current <= 0) {
      dragCounterRef.current = 0;
      setIsDragOver(false);
    }
  }, []);

  /**
   * Handle `dragover` on the content area.
   * `preventDefault()` is required to signal that this element accepts drops.
   * Without it, the browser blocks the drop event entirely.
   * @see https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/dragover_event
   */
  const handleDragOver = useCallback((e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
  }, []);

  /**
   * Handle `drop` on the content area.
   *
   * Extracts the URL from the drop event data transfer, validates it as an
   * Apple Music URL, and if valid: navigates to the Download page and populates
   * the URL input field. Shows an error toast for invalid URLs.
   *
   * URL extraction priority:
   *   1. `text/uri-list` -- standard format when dragging links from browsers
   *   2. `text/plain`    -- fallback for plain-text drops (e.g., from text editors)
   *
   * @see https://developer.mozilla.org/en-US/docs/Web/API/DataTransfer/getData
   */
  const handleDrop = useCallback(
    (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault();

      /* Reset drag state immediately on drop. */
      dragCounterRef.current = 0;
      setIsDragOver(false);

      /* Check for .meedyadl file drops first. */
      const files = e.dataTransfer.files;
      if (files.length > 0) {
        const file = files[0];
        if (file.name.endsWith('.meedyadl')) {
          const reader = new FileReader();
          reader.onload = async () => {
            try {
              const manifest = JSON.parse(reader.result as string);
              const urls: string[] = (manifest.sources ?? []).map(
                (s: { url: string }) => s.url
              );
              if (urls.length > 0) {
                setPage('download');
                setUrlInput(urls.join('\n'));
                addToast(
                  `Imported ${urls.length} URL${urls.length !== 1 ? 's' : ''} from manifest`,
                  'success'
                );
              } else {
                addToast('Manifest contains no download sources', 'error');
              }
            } catch {
              addToast('Invalid .meedyadl manifest file', 'error');
            }
          };
          reader.readAsText(file);
          return;
        }
      }

      /*
       * Extract the URL from the data transfer.
       * Try `text/uri-list` first (standard for browser link drags),
       * then fall back to `text/plain` (plain text selections).
       */
      const rawUrl =
        e.dataTransfer.getData('text/uri-list') ||
        e.dataTransfer.getData('text/plain');

      /* Trim whitespace and newlines that browsers may include. */
      const url = rawUrl.trim();

      if (!url) {
        return;
      }

      /* Validate the dropped URL as an Apple Music link. */
      const parsed = parseAppleMusicUrl(url);

      if (parsed.isValid) {
        /* Navigate to the Download page and populate the URL input. */
        setPage('download');
        setUrlInput(url);
        addToast('Apple Music URL dropped successfully', 'success');
      } else {
        addToast('Not a valid Apple Music URL', 'error');
      }
    },
    [setPage, setUrlInput, addToast],
  );

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    /**
     * Root container: full-viewport column layout.
     * `h-screen` ensures it exactly fills the Tauri webview height.
     * @see https://tailwindcss.com/docs/height#screen
     */
    <div className="flex flex-col h-screen">
      {/*
       * Skip navigation link -- visually hidden, becomes visible on focus.
       * Allows keyboard users to bypass the sidebar and jump directly to
       * the main content area. This is a WCAG 2.1 Level A requirement
       * (Success Criterion 2.4.1: Bypass Blocks).
       *
       * Styling: Positioned off-screen by default. On focus, it slides
       * into view at the top-left corner with a high z-index so it sits
       * above all other content including the sidebar.
       */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-[200] focus:px-4 focus:py-2 focus:bg-accent focus:text-white focus:rounded-platform focus:text-sm focus:font-medium focus:shadow-platform focus:outline-none"
      >
        Skip to main content
      </a>

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
         * `relative` establishes a positioning context for the drop-zone overlay.
         *
         * Drag-and-drop event handlers are attached here so the entire content
         * area (excluding the sidebar) acts as a drop zone.
         */}
        <div
          className="flex-1 flex flex-col overflow-hidden relative"
          onDragEnter={handleDragEnter}
          onDragLeave={handleDragLeave}
          onDragOver={handleDragOver}
          onDrop={handleDrop}
        >
          {/*
           * Update banner -- rendered ABOVE the scrollable <main> so it
           * stays visible regardless of page scroll position. It lives in
           * the flex column between the top edge and <main>, taking only
           * the vertical space it needs (flex-shrink-0 is implicit for
           * non-flex-1 children). When dismissed or when no updates are
           * available, it renders nothing and takes zero space.
           * @see UpdateBanner component in @/components/common
           */}
          <UpdateBanner />

          {/*
           * Feature-availability notice banners -- rendered directly below
           * UpdateBanner, in the same non-scrolling flex column, so a
           * disabled/degraded feature's explanation stays visible
           * regardless of page scroll position too. Renders nothing (zero
           * layout space) when there is nothing to show.
           * @see FeatureNoticeBanner component in @/components/common
           */}
          <FeatureNoticeBanner />

          {/*
           * Scrollable page content area.
           * `flex-1` makes it grow to fill the column; `overflow-y-auto`
           * adds a vertical scrollbar only when content overflows.
           * The `children` prop is injected here -- it is the currently
           * active page component (e.g., DownloadForm, DownloadQueue).
           * @see https://react.dev/learn/passing-props-to-a-component#passing-jsx-as-children
           */}
          <main id="main-content" className="flex-1 overflow-y-auto">{children}</main>

          {/*
           * Global progress bars -- always visible when downloads are active.
           * Upper bar: per-item progress (current download)
           * Lower bar: queue-level progress (completed / total)
           * Auto-hides when no downloads are active or queued.
           * @see GlobalProgressBar component in ./GlobalProgressBar.tsx
           */}
          <GlobalProgressBar />

          {/*
           * Bottom status bar -- pinned below the scrollable <main>.
           * Displays download activity counters and the app version.
           * @see StatusBar component in ./StatusBar.tsx
           */}
          <StatusBar />

          {/*
           * Drop-zone overlay -- displayed when a drag operation hovers
           * over the content area. Covers the entire content column with
           * a semi-transparent backdrop, dashed border, and instructional
           * text. Hidden when the drag leaves or the drop completes.
           *
           * `pointer-events-none` ensures the overlay does not intercept
           * mouse events itself (the parent div handles drag events).
           * `z-50` places it above page content but below toasts.
           */}
          {isDragOver && (
            <div className="absolute inset-0 z-50 flex items-center justify-center bg-[var(--color-bg-primary)]/80 backdrop-blur-sm pointer-events-none">
              <div className="flex flex-col items-center gap-3 p-8 border-2 border-dashed border-[var(--color-accent)] rounded-2xl">
                {/*
                 * Arrow-down icon (SVG) -- visual indicator that content
                 * can be dropped here. Uses the accent colour for emphasis.
                 */}
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  className="w-12 h-12 text-[var(--color-accent)]"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}
                  aria-hidden="true"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" d="M12 4v12m0 0l-4-4m4 4l4-4M4 18h16" />
                </svg>
                <span className="text-lg font-medium text-[var(--color-text-primary)]">
                  Drop Apple Music URL here
                </span>
              </div>
            </div>
          )}
        </div>
      </div>

      {/*
       * Toast notification overlay -- positioned fixed in the top-right
       * corner of the viewport. Renders above all other content using
       * a high z-index. Toasts are managed by useUiStore.toasts[].
       * @see ToastContainer component in @/components/common
       */}
      <ToastContainer />

      {/*
        Global keyboard-shortcuts help dialog (#465). Mounted once at
        the root so Cmd/Ctrl + Shift + ? can open it from any page.
        Visibility is driven entirely by `ui.shortcutsHelpOpen` in the
        UI store; the dialog has its own Modal close handling.
      */}
      <ShortcutsHelpDialog />
    </div>
  );
}
