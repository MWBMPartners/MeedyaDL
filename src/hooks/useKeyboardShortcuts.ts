// Copyright (c) 2026 MeedyaSuite
/**
 * @file useKeyboardShortcuts.ts -- Global keyboard shortcut handler
 * @license MIT -- See LICENSE file in the project root.
 *
 * Registers a global `keydown` listener on the `window` object to provide
 * application-wide keyboard shortcuts for common navigation and actions.
 *
 * ## Supported shortcuts
 *
 * | Shortcut                | Action                                         |
 * |-------------------------|------------------------------------------------|
 * | `Cmd/Ctrl + D`          | Navigate to Download page and focus URL input  |
 * | `Cmd/Ctrl + ,`          | Navigate to Settings page                      |
 * | `Cmd/Ctrl + Q`          | Navigate to Queue page                         |
 * | `Cmd/Ctrl + Shift + .`  | Abort all active and queued downloads (#620)   |
 * | `Cmd/Ctrl + Enter`      | Start download (when URL input is focused)     |
 * | `Escape`                | Close active modal                             |
 *
 * ## Design decisions
 *
 * - **Modifier key detection**: Uses `e.metaKey` for macOS (Cmd) and `e.ctrlKey`
 *   for Windows/Linux (Ctrl). Both are checked simultaneously so the correct
 *   modifier works on each platform.
 * - **Input guard**: Shortcuts that navigate or trigger actions are suppressed when
 *   focus is inside an `<input>`, `<textarea>`, or `<select>` element, preventing
 *   interference with normal text editing.
 * - **Escape handling**: Not handled here — the {@link Modal} component already
 *   registers its own Escape key listener when open. Adding a global Escape handler
 *   would create conflicts and duplicate dismiss behaviour.
 * - **Cmd/Ctrl+Enter handling**: Not handled here — the URL input's `onKeyDown`
 *   handler in `DownloadForm.tsx` already triggers form submission on Enter
 *   (including `Cmd/Ctrl+Enter`), and that handler runs essential pre-download
 *   checks (internet, output path, cookies). Duplicating submission here would
 *   cause double-fire and bypass those safety checks.
 * - **`e.preventDefault()`**: Called for handled shortcuts to suppress browser/OS
 *   default behaviour (e.g., Cmd+D bookmarks the page in browsers, Cmd+Q quits
 *   the app on macOS).
 *
 * ## Usage
 *
 * Call `useKeyboardShortcuts()` once in the root `<App>` component. It manages
 * its own cleanup via the `useEffect` return function.
 *
 * ```tsx
 * function App() {
 *   useKeyboardShortcuts();
 *   return <MainLayout>...</MainLayout>;
 * }
 * ```
 *
 * @see {@link https://react.dev/reference/react/useEffect} -- useEffect lifecycle
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent} -- KeyboardEvent API
 */

import { useEffect } from 'react';

/**
 * Zustand UI store for page navigation actions.
 * @see ./stores/uiStore.ts
 */
import { useUiStore } from '@/stores/uiStore';

/**
 * Zustand download store for the abort-all action (#620).
 * @see ./stores/downloadStore.ts
 */
import { useDownloadStore } from '@/stores/downloadStore';

/**
 * Zustand settings store — consulted for `abort_queue_confirm` to decide
 * whether the Cmd/Ctrl+Shift+. shortcut fires immediately or raises a
 * confirmation prompt.
 */
import { useSettingsStore } from '@/stores/settingsStore';

/**
 * ID of the URL input element in DownloadForm.
 * Must match the `id` attribute on the `<input>` in DownloadForm.tsx.
 */
const URL_INPUT_ID = 'url-input';

/**
 * Registers global keyboard shortcuts for the application.
 *
 * This hook should be called exactly once in the root component (`App.tsx`).
 * It attaches a single `keydown` listener to `window` that handles all
 * global shortcuts, and cleans up the listener on unmount.
 *
 * Shortcuts are suppressed when focus is inside form elements (input, textarea,
 * select) to avoid interfering with normal text editing.
 */
export function useKeyboardShortcuts(): void {
  useEffect(() => {
    /**
     * Global keydown handler. Checks for modifier keys and routes to the
     * appropriate action. Uses `useUiStore.getState()` for imperative access
     * to avoid subscribing to store state (which would cause re-renders).
     *
     * @param e - The native KeyboardEvent from the `window` listener
     */
    function handleKeyDown(e: KeyboardEvent): void {
      /*
       * Determine whether the platform modifier key is held.
       * macOS uses the Command key (metaKey), Windows/Linux use Ctrl (ctrlKey).
       * Both are checked so shortcuts work correctly on all platforms.
       */
      const hasModifier = e.metaKey || e.ctrlKey;

      /*
       * All shortcuts in this handler require the modifier key.
       * Early-exit if no modifier is held to avoid unnecessary checks.
       *
       * Note: Escape is handled by the Modal component's own keydown listener,
       * and Cmd/Ctrl+Enter is handled by DownloadForm's onKeyDown handler.
       * Neither needs to be duplicated here.
       */
      if (!hasModifier) return;

      /*
       * Detect whether focus is inside a form element (input, textarea, select).
       * When typing in these elements, navigation shortcuts should be suppressed
       * to avoid interfering with text editing.
       */
      const activeElement = document.activeElement;
      const isInFormElement =
        activeElement instanceof HTMLInputElement ||
        activeElement instanceof HTMLTextAreaElement ||
        activeElement instanceof HTMLSelectElement;

      if (isInFormElement) return;

      /*
       * Route the key press to the appropriate action.
       * `e.key` is case-insensitive for letter keys when checked in lowercase.
       */
      switch (e.key.toLowerCase()) {
        /*
         * Cmd/Ctrl+D: Navigate to the Download page and focus the URL input.
         * Uses `requestAnimationFrame` to defer the focus call until after
         * React has rendered the Download page (if navigating from another page).
         * `e.preventDefault()` suppresses the browser's "Add Bookmark" dialog.
         */
        case 'd': {
          e.preventDefault();
          useUiStore.getState().setPage('download');
          requestAnimationFrame(() => {
            const urlInput = document.getElementById(URL_INPUT_ID);
            if (urlInput) {
              urlInput.focus();
              /*
               * Select all text in the input so the user can immediately
               * start typing a new URL without manually clearing the field.
               */
              if (urlInput instanceof HTMLInputElement) {
                urlInput.select();
              }
            }
          });
          break;
        }

        /*
         * Cmd/Ctrl+, (comma): Navigate to the Settings page.
         * Follows the macOS convention where Cmd+, opens Preferences.
         * `e.preventDefault()` suppresses any default browser behaviour.
         */
        case ',': {
          e.preventDefault();
          useUiStore.getState().setPage('settings');
          break;
        }

        /*
         * Cmd/Ctrl+Q: Navigate to the Queue page.
         * On macOS, Cmd+Q normally quits the app — `e.preventDefault()`
         * intercepts this to navigate to the Queue page instead.
         * In the Tauri desktop app, the native quit behaviour is handled
         * by the window close event, not by the web keydown event.
         */
        case 'q': {
          e.preventDefault();
          useUiStore.getState().setPage('queue');
          break;
        }

        /*
         * Cmd/Ctrl+Shift+. (period): Abort all active and queued downloads
         * (#620). macOS convention for "stop loading" uses Cmd+. but that
         * conflicts with browser dev-console shortcuts and platform-level
         * interrupts on some terminals. Requiring Shift scopes the binding
         * to the app alone and is unambiguous across all three platforms.
         *
         * Honours the `abort_queue_confirm` setting: fires immediately
         * when disabled, uses a native `window.confirm()` when enabled.
         * The Queue-page modal is the canonical rich confirmation with
         * "Don't ask again"; the shortcut and status-bar button share
         * this lighter-weight path.
         */
        case '.': {
          if (!e.shiftKey) break;
          e.preventDefault();
          const settings = useSettingsStore.getState().settings;
          if (settings.abort_queue_confirm) {
            const confirmed = window.confirm(
              'Abort every active and queued download? This cannot be undone.',
            );
            if (!confirmed) break;
          }
          void useDownloadStore.getState().abortAll();
          break;
        }

        /*
         * Page-navigation expansions (#465). Same shape as Cmd+D /
         * Cmd+Q above — set the page via uiStore and let React
         * render. Suppressed inside form elements via the
         * `isInFormElement` guard above. Lower-case letters since
         * `e.key.toLowerCase()` is the switch discriminator.
         */
        case 'l': {
          e.preventDefault();
          useUiStore.getState().setPage('library');
          break;
        }
        case 'h': {
          e.preventDefault();
          useUiStore.getState().setPage('history');
          break;
        }
        case 'k': {
          // 'k' rather than 'a' (which is Select All) or 'l'
          // (taken by Library above) — k is unused by every
          // common desktop convention I checked, and the Activity
          // page is what the user most often jumps to during a
          // long download.
          e.preventDefault();
          useUiStore.getState().setPage('activity');
          break;
        }

        /*
         * Cmd/Ctrl + Shift + ? — show the keyboard shortcuts help
         * dialog (#465). The standard "what shortcuts does this app
         * have?" binding across macOS / Linux. `e.key === '?'`
         * works because Shift is part of the chord — without Shift
         * we'd see `/` instead, which doesn't match.
         *
         * `useUiStore.getState().setShortcutsHelpOpen(true)` flips
         * the global `ShortcutsHelpDialog` (mounted in MainLayout)
         * to visible; the user closes via Escape or the Modal's
         * built-in close button.
         */
        case '?': {
          if (!e.shiftKey) break;
          e.preventDefault();
          useUiStore.getState().setShortcutsHelpOpen(true);
          break;
        }

        /* No matching shortcut — let the event propagate normally */
        default:
          break;
      }
    }

    /*
     * Register the global keydown listener.
     * Using `window` (not `document`) ensures the listener captures events
     * regardless of which DOM element has focus.
     */
    window.addEventListener('keydown', handleKeyDown);

    /* Cleanup: remove the listener when the component unmounts */
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, []); // Empty deps: register once on mount, clean up on unmount
}
