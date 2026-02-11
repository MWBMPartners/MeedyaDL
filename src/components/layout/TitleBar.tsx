// Copyright (c) 2024-2026 MWBM Partners Ltd
/**
 * @file Custom title bar component (Windows/Linux only).
 *
 * On macOS, the native title bar is used with traffic-light buttons
 * and `titleBarStyle: 'overlay'` in `tauri.conf.json`, so this component
 * renders **nothing** (`null`) on macOS. On Windows and Linux the OS
 * provides no built-in window decorations for `decorations: false`
 * webview windows, so we must render our own minimize / maximize / close
 * buttons and a draggable title-bar region.
 *
 * Tauri window control flow:
 *  1. The user clicks a button (e.g., minimize).
 *  2. The handler dynamically imports `@tauri-apps/api/window` to access
 *     the current window instance (`getCurrentWindow()`).
 *  3. The corresponding Tauri IPC method (`minimize()`, `toggleMaximize()`,
 *     or `close()`) is invoked on the window handle.
 *  4. Tauri's Rust core processes the request and instructs the OS window
 *     manager to carry out the action.
 *
 * The dynamic import pattern (`await import(...)`) is intentional: it
 * avoids hard failures when this React code is rendered in a regular
 * browser during development (`npm run dev` without the Tauri shell).
 *
 * CSS classes `drag-region` and `no-drag` are special Tauri helpers:
 *  - `drag-region`: makes the element act as a window drag handle,
 *    equivalent to the OS title bar drag area.
 *  - `no-drag`: nested inside a `drag-region`, this exempts interactive
 *    elements (buttons) from the drag behavior so clicks register normally.
 *
 * @see https://v2.tauri.app/reference/javascript/api/namespacewindow/
 *      Tauri v2 Window API reference.
 * @see https://v2.tauri.app/reference/javascript/api/namespacewindow/#getcurrentwindow
 *      `getCurrentWindow()` -- returns a handle to the current Tauri window.
 * @see https://lucide.dev/icons/minus   -- Minus icon (minimize button)
 * @see https://lucide.dev/icons/square  -- Square icon (maximize button)
 * @see https://lucide.dev/icons/x       -- X icon (close button)
 */

/**
 * Lucide React icons used for the three window-control buttons.
 * Lucide is a fork of Feather Icons with a consistent 24x24 grid.
 * @see https://lucide.dev/icons/
 */
import { Minus, Square, X } from 'lucide-react';

/**
 * Custom hook that detects the current OS platform at runtime via the
 * Tauri OS plugin (`@tauri-apps/plugin-os`), with a browser-based
 * fallback for development outside the Tauri shell.
 * @see usePlatform in @/hooks/usePlatform.ts
 */
import { usePlatform } from '@/hooks/usePlatform';

/**
 * Renders a custom window title bar with minimize, maximize, and close buttons.
 *
 * Only visible on **Windows** and **Linux**. On macOS this component returns
 * `null` because native decorations (traffic-light close/minimize/maximize
 * buttons) are provided by the OS via `titleBarStyle: 'overlay'` configured
 * in `tauri.conf.json`.
 *
 * The component is intentionally stateless -- it does not track whether
 * the window is currently maximized. The `toggleMaximize()` API handles
 * that internally.
 *
 * @returns JSX title bar on Windows/Linux, or `null` on macOS.
 *
 * @see https://v2.tauri.app/reference/javascript/api/namespacewindow/
 */
export function TitleBar() {
  /**
   * Destructure `isMacOS` from the platform detection hook.
   * This triggers a re-render once platform detection completes (async).
   * @see usePlatform in @/hooks/usePlatform.ts
   */
  const { isMacOS } = usePlatform();

  /*
   * Early return for macOS: the native title bar with traffic-light
   * buttons is used instead. Returning `null` means React renders
   * nothing for this component on macOS.
   */
  if (isMacOS) return null;

  // ---------------------------------------------------------------
  // Window control handlers
  // ---------------------------------------------------------------
  // Each handler dynamically imports the Tauri window API at call time.
  // The dynamic import (`await import(...)`) serves two purposes:
  //   1. Tree-shaking: the module is only loaded when actually needed.
  //   2. Graceful degradation: if the app runs in a plain browser
  //      (e.g., `npm run dev` outside Tauri), the import fails and
  //      the catch block silently absorbs the error.
  //
  // @see https://v2.tauri.app/reference/javascript/api/namespacewindow/#getcurrentwindow

  /**
   * Minimize the application window to the taskbar / dock.
   * Calls `getCurrentWindow().minimize()` via Tauri's Window API.
   * @see https://v2.tauri.app/reference/javascript/api/namespacewindow/#minimize
   */
  const handleMinimize = async () => {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().minimize();
    } catch {
      /* Tauri API unavailable (running in browser) -- silently ignore */
    }
  };

  /**
   * Toggle between maximized and restored window states.
   * Uses `toggleMaximize()` which internally checks the current state
   * and switches to the opposite one, so we do not need to track
   * the maximized flag in React state.
   * @see https://v2.tauri.app/reference/javascript/api/namespacewindow/#togglemaximize
   */
  const handleMaximize = async () => {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().toggleMaximize();
    } catch {
      /* Tauri API unavailable (running in browser) */
    }
  };

  /**
   * Close the application window (and terminate the process).
   * This is equivalent to clicking the OS-native close button.
   * @see https://v2.tauri.app/reference/javascript/api/namespacewindow/#close
   */
  const handleClose = async () => {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().close();
    } catch {
      /* Tauri API unavailable (running in browser) */
    }
  };

  return (
    /**
     * Title bar container.
     *
     * `drag-region` -- Tauri CSS class that makes this element behave as the
     * OS window drag handle. Users can click-and-drag anywhere on this bar
     * to reposition the window, just like a native title bar.
     *
     * `h-8` (32px) matches the standard Windows title bar height.
     * `bg-surface-primary` and `border-b border-border-light` use the app's
     * design-token-based color scheme for consistent theming.
     *
     * @see https://tailwindcss.com/docs/height -- h-8 utility
     */
    <div className="drag-region flex items-center justify-end h-8 bg-surface-primary border-b border-border-light">
      {/*
       * Window control button group.
       *
       * `no-drag` -- Tauri CSS class that removes the drag behavior for
       * this nested element, so button clicks are not intercepted by
       * the window drag handler. Without this, clicking the buttons
       * would start a window drag instead of triggering onClick.
       */}
      <div className="no-drag flex items-center">
        {/*
         * Minimize button.
         * Uses the Lucide `Minus` icon (horizontal line) at 14px to
         * mimic the standard Windows minimize button appearance.
         * `aria-label` provides screen-reader accessibility.
         */}
        <button
          onClick={handleMinimize}
          className="w-12 h-8 flex items-center justify-center text-content-secondary hover:bg-surface-secondary transition-colors"
          aria-label="Minimize"
        >
          <Minus size={14} />
        </button>

        {/*
         * Maximize / Restore button.
         * Uses the Lucide `Square` icon (empty square) at 12px -- slightly
         * smaller than the other icons to visually match the Windows
         * maximize glyph proportions. Toggles between maximized and
         * restored window states.
         */}
        <button
          onClick={handleMaximize}
          className="w-12 h-8 flex items-center justify-center text-content-secondary hover:bg-surface-secondary transition-colors"
          aria-label="Maximize"
        >
          <Square size={12} />
        </button>

        {/*
         * Close button.
         * Uses the Lucide `X` icon at 14px. On hover, the background
         * turns `bg-status-error` (red) and text turns white, matching
         * the Windows convention for the close button highlight color.
         */}
        <button
          onClick={handleClose}
          className="w-12 h-8 flex items-center justify-center text-content-secondary hover:bg-status-error hover:text-white transition-colors"
          aria-label="Close"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
