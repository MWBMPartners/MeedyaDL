/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/hooks/useTheme.ts - Theme override management hook
 *
 * This custom React hook manages the application's dark/light mode by syncing
 * the user's `theme_override` setting to CSS classes on the `<html>` element.
 *
 * Three modes are supported:
 *   - `null` (Auto): Follows the OS color scheme preference via the CSS
 *     `prefers-color-scheme` media query. No class is added to `<html>`.
 *   - `'dark'`: Forces dark mode regardless of OS setting. Adds `theme-dark`
 *     class to `<html>`, which activates the `.theme-dark` CSS rules in the
 *     theme files (base.css, macos.css, windows.css, linux.css).
 *   - `'light'`: Forces light mode regardless of OS setting. Adds `theme-light`
 *     class to `<html>`, which prevents `@media (prefers-color-scheme: dark)`
 *     rules from applying (via `:not(.theme-light)` guards in the CSS).
 *
 * The hook also sets the `color-scheme` CSS property on `<html>`, which tells
 * the browser to render native form controls (scrollbars, checkboxes, text
 * selection) in the appropriate mode.
 *
 * @see src/styles/themes/base.css -- CSS implementation of theme overrides
 * @see src/stores/settingsStore.ts -- Source of the theme_override setting
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/CSS/color-scheme}
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-color-scheme}
 */

import { useEffect } from 'react';

import { useSettingsStore } from '@/stores/settingsStore';

/**
 * The two CSS classes used to force a specific theme mode on `<html>`.
 * These must match the selectors in the CSS theme files.
 */
const THEME_CLASSES = ['theme-dark', 'theme-light'] as const;

/**
 * Manages the theme override by adding/removing CSS classes on `<html>`.
 *
 * This hook reads `theme_override` from the settings store and reactively
 * updates the DOM whenever the setting changes. It handles three transitions:
 *
 * 1. Setting changes to `'dark'` -> remove `theme-light`, add `theme-dark`
 * 2. Setting changes to `'light'` -> remove `theme-dark`, add `theme-light`
 * 3. Setting changes to `null` (auto) -> remove both classes
 *
 * The cleanup function ensures classes are removed if the component unmounts
 * or the setting changes, preventing stale theme classes from persisting.
 *
 * @example
 * ```tsx
 * function App() {
 *   useTheme(); // Automatically syncs theme to settings
 *   return <MainLayout />;
 * }
 * ```
 */
export function useTheme(): void {
  /**
   * Subscribe to just the theme_override field from settings.
   * This minimizes re-renders: only fires when theme_override specifically changes.
   */
  const themeOverride = useSettingsStore((s) => s.settings.theme_override);

  useEffect(() => {
    const htmlEl = document.documentElement;

    /* Remove any existing theme class to start from a clean state */
    htmlEl.classList.remove(...THEME_CLASSES);

    if (themeOverride === 'dark') {
      /* Force dark mode: add class that activates .theme-dark CSS rules */
      htmlEl.classList.add('theme-dark');
      htmlEl.style.colorScheme = 'dark';
    } else if (themeOverride === 'light') {
      /* Force light mode: add class that blocks dark media query via :not(.theme-light) */
      htmlEl.classList.add('theme-light');
      htmlEl.style.colorScheme = 'light';
    } else {
      /* Auto mode: let the OS media query decide; remove forced color-scheme */
      htmlEl.style.colorScheme = '';
    }

    /* Cleanup: remove theme classes if the hook re-runs or component unmounts */
    return () => {
      htmlEl.classList.remove(...THEME_CLASSES);
      htmlEl.style.colorScheme = '';
    };
  }, [themeOverride]);
}
