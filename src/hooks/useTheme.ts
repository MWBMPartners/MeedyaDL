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
 * High-contrast accessibility mode:
 *   - Toggled via the `high_contrast` boolean setting in Settings > General.
 *   - Auto-detected from the OS `prefers-contrast: high` media query.
 *   - Adds/removes the `high-contrast` class on `<html>`, which activates
 *     the overrides in `a11y-high-contrast.css`.
 *
 * The hook also sets the `color-scheme` CSS property on `<html>`, which tells
 * the browser to render native form controls (scrollbars, checkboxes, text
 * selection) in the appropriate mode.
 *
 * @see src/styles/themes/base.css -- CSS implementation of theme overrides
 * @see src/styles/themes/a11y-high-contrast.css -- High-contrast CSS overrides
 * @see src/stores/settingsStore.ts -- Source of the theme_override setting
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/CSS/color-scheme}
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-color-scheme}
 * @see {@link https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-contrast}
 */

import { useEffect } from 'react';

import { useSettingsStore } from '@/stores/settingsStore';

/**
 * The two CSS classes used to force a specific theme mode on `<html>`.
 * These must match the selectors in the CSS theme files.
 */
const THEME_CLASSES = ['theme-dark', 'theme-light'] as const;

/**
 * CSS class applied to `<html>` when high-contrast mode is active.
 * Must match the selector in `a11y-high-contrast.css`.
 */
const HIGH_CONTRAST_CLASS = 'high-contrast';

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
 * It also manages the `high-contrast` class based on:
 * - The `high_contrast` boolean setting (explicit user toggle)
 * - The OS-level `prefers-contrast: high` media query (auto-detection)
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

  /**
   * Subscribe to the high_contrast setting for the accessibility theme.
   */
  const highContrast = useSettingsStore((s) => s.settings.high_contrast);

  /* Effect 1: Dark/light theme class management */
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

  /* Effect 2: High-contrast class management (setting + OS media query) */
  useEffect(() => {
    const htmlEl = document.documentElement;

    /**
     * Listen for the OS-level `prefers-contrast: high` media query.
     * When the OS has "Increase Contrast" enabled (macOS) or equivalent,
     * this automatically applies the high-contrast class even if the
     * user hasn't toggled the setting manually.
     *
     * Guard: `window.matchMedia` may be unavailable in test environments
     * (jsdom) or very old browsers. If unavailable, OS auto-detection is
     * skipped and only the explicit setting drives the class.
     */
    const contrastQuery =
      typeof window.matchMedia === 'function'
        ? window.matchMedia('(prefers-contrast: high)')
        : null;

    /** Apply or remove the high-contrast class based on setting + OS preference */
    const applyHighContrast = () => {
      const osPrefers = contrastQuery?.matches ?? false;
      const shouldEnable = highContrast || osPrefers;
      if (shouldEnable) {
        htmlEl.classList.add(HIGH_CONTRAST_CLASS);
      } else {
        htmlEl.classList.remove(HIGH_CONTRAST_CLASS);
      }
    };

    /* Apply immediately on mount / setting change */
    applyHighContrast();

    /* Listen for OS preference changes (e.g., user toggles "Increase Contrast" in System Preferences) */
    contrastQuery?.addEventListener('change', applyHighContrast);

    /* Cleanup: remove class and event listener */
    return () => {
      htmlEl.classList.remove(HIGH_CONTRAST_CLASS);
      contrastQuery?.removeEventListener('change', applyHighContrast);
    };
  }, [highContrast]);
}
