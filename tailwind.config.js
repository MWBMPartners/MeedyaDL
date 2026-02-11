/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Tailwind CSS Configuration for gamdl-GUI
 * =========================================
 *
 * This file customizes Tailwind CSS for the platform-adaptive design system.
 * Instead of hardcoding colors and spacing, most design tokens reference CSS custom
 * properties (variables) that are defined in the theme CSS files:
 *   - src/styles/themes/base.css    -- Default/fallback values for all tokens
 *   - src/styles/themes/macos.css   -- macOS Liquid Glass overrides
 *   - src/styles/themes/windows.css -- Windows Fluent Design overrides
 *   - src/styles/themes/linux.css   -- Linux Adwaita overrides
 *
 * At runtime, the active platform's CSS class (e.g., .platform-macos) is applied
 * to the <html> element, which causes the correct CSS variable values to take effect.
 * Tailwind classes like `bg-sidebar-bg` or `text-content-primary` then resolve to
 * platform-appropriate colors automatically.
 *
 * Related config files:
 *   - postcss.config.js  -- Registers the 'tailwindcss' PostCSS plugin that reads this file
 *   - vite.config.ts     -- Vite invokes PostCSS (and thus Tailwind) during the build
 *   - src/styles/globals.css -- Contains the @tailwind directives that trigger CSS generation
 *
 * @see https://tailwindcss.com/docs/configuration -- Tailwind CSS configuration reference
 * @see https://tailwindcss.com/docs/theme -- Theme customization guide
 * @see https://tailwindcss.com/docs/customizing-colors -- Custom color palette guide
 */

/**
 * @type {import('tailwindcss').Config}
 * JSDoc type annotation provides editor auto-completion for Tailwind config options.
 */
export default {
  /**
   * content -- Specifies which files Tailwind should scan for class name usage.
   *
   * Tailwind uses this to "tree-shake" (purge) unused CSS in production builds.
   * Only classes found in these files will be included in the final CSS bundle.
   * If a class like `bg-sidebar-bg` is only used in a .tsx file, it needs to be
   * in one of these glob patterns to survive purging.
   *
   * Patterns:
   *   - './index.html'                -- Root HTML file (may contain Tailwind classes)
   *   - './src/**\/*.{js,ts,jsx,tsx}' -- All JavaScript/TypeScript source files
   *
   * NOTE: CSS files are NOT listed here because Tailwind scans for class _usage_,
   * not class _definitions_. The theme CSS files define CSS variables, not Tailwind classes.
   *
   * @see https://tailwindcss.com/docs/content-configuration
   */
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],

  theme: {
    /**
     * extend -- Adds new values to Tailwind's default theme without replacing existing ones.
     *
     * Everything inside 'extend' is merged with Tailwind's built-in theme. This means
     * default utilities like `bg-white`, `text-sm`, `rounded-lg` still work alongside
     * our custom platform-adaptive tokens.
     *
     * @see https://tailwindcss.com/docs/theme#extending-the-default-theme
     */
    extend: {
      /**
       * fontFamily -- Custom font stacks for each supported platform.
       *
       * These generate Tailwind classes like `font-sf-pro`, `font-segoe`, `font-system`.
       * However, the primary font selection happens via the --font-family CSS variable
       * (set in each platform's theme CSS), not directly through these Tailwind classes.
       * These are provided as utilities for cases where a specific platform font is needed
       * regardless of the detected OS (e.g., screenshots, previews).
       *
       * @see https://tailwindcss.com/docs/font-family
       */
      fontFamily: {
        /**
         * macOS font stack: San Francisco (SF Pro) system font.
         * -apple-system and BlinkMacSystemFont are vendor-prefixed aliases for the
         * system font on Safari and Chrome on macOS, respectively.
         * Generates class: font-sf-pro
         */
        'sf-pro': [
          '-apple-system',
          'BlinkMacSystemFont',
          'SF Pro Display',
          'SF Pro Text',
          'sans-serif',
        ],
        /**
         * Windows font stack: Segoe UI Variable is the new variable-weight font
         * introduced in Windows 11. Falls back to Segoe UI (Windows 10) then sans-serif.
         * Generates class: font-segoe
         */
        segoe: ['Segoe UI Variable', 'Segoe UI', 'sans-serif'],
        /**
         * Linux/generic font stack: Uses the browser's system-ui generic family,
         * which resolves to the user's configured desktop font (e.g., Cantarell on GNOME,
         * Noto Sans on KDE). Serves as the universal fallback.
         * Generates class: font-system
         */
        system: ['system-ui', '-apple-system', 'sans-serif'],
      },

      /**
       * colors -- Platform-adaptive color palette using CSS custom properties.
       *
       * Each color value is a var() reference to a CSS custom property defined in the
       * theme CSS files (base.css, macos.css, windows.css, linux.css). This approach
       * means Tailwind classes like `bg-sidebar-bg` or `text-content-primary` automatically
       * adapt to the active platform theme and light/dark mode without JavaScript.
       *
       * Color groups are organized by UI purpose (sidebar, surface, accent, etc.)
       * rather than by hue, making it easy to maintain a consistent design system.
       *
       * @see https://tailwindcss.com/docs/customizing-colors
       * @see src/styles/themes/base.css -- Where the CSS variable defaults are defined
       */
      colors: {
        /**
         * sidebar -- Colors for the left sidebar navigation panel.
         * Usage: bg-sidebar-bg, hover:bg-sidebar-hover, text-sidebar-text, etc.
         * Variables defined in: base.css (defaults), macos/windows/linux.css (overrides)
         */
        sidebar: {
          bg: 'var(--sidebar-bg)',             /* Background color of the sidebar panel */
          hover: 'var(--sidebar-hover)',        /* Background on hover over nav items */
          active: 'var(--sidebar-active)',      /* Background of the currently selected nav item */
          text: 'var(--sidebar-text)',          /* Default text color for sidebar items */
          'text-active': 'var(--sidebar-text-active)', /* Text color for the active nav item */
          border: 'var(--sidebar-border)',      /* Right border separating sidebar from content */
        },
        /**
         * surface -- Background colors for content areas, cards, and overlays.
         * Usage: bg-surface-primary, bg-surface-elevated, bg-surface-overlay
         * 'primary' is the main background; 'elevated' is for cards/modals above it.
         */
        surface: {
          primary: 'var(--surface-primary)',    /* Main content area background */
          secondary: 'var(--surface-secondary)', /* Alternate/grouped content background */
          elevated: 'var(--surface-elevated)',  /* Cards, modals, and raised surfaces */
          overlay: 'var(--surface-overlay)',    /* Semi-transparent backdrop behind modals */
        },
        /**
         * accent -- Brand/interactive color used for buttons, links, and active states.
         * Usage: bg-accent, hover:bg-accent-hover, bg-accent-light
         * Each platform defines its own accent blue (Apple blue, Windows blue, Adwaita blue).
         */
        accent: {
          DEFAULT: 'var(--accent)',             /* Primary accent color (buttons, links) */
          hover: 'var(--accent-hover)',         /* Accent color on hover state */
          light: 'var(--accent-light)',         /* Light tint for subtle accent backgrounds */
        },
        /**
         * content -- Text colors at varying emphasis levels.
         * Usage: text-content-primary, text-content-secondary, text-content-tertiary
         * Named 'content' instead of 'text' to avoid collision with Tailwind's built-in text colors.
         */
        content: {
          primary: 'var(--text-primary)',       /* Headings, body text, most prominent text */
          secondary: 'var(--text-secondary)',   /* Descriptions, labels, less prominent text */
          tertiary: 'var(--text-tertiary)',     /* Placeholders, hints, least prominent text */
          inverse: 'var(--text-inverse)',       /* Text on colored backgrounds (e.g., white on accent) */
        },
        /**
         * status -- Semantic colors for feedback states in the download queue.
         * Usage: text-status-success, bg-status-error, border-status-warning
         */
        status: {
          success: 'var(--status-success)',     /* Completed downloads, positive actions */
          warning: 'var(--status-warning)',     /* Warnings, pending states */
          error: 'var(--status-error)',         /* Failed downloads, validation errors */
          info: 'var(--status-info)',           /* Informational messages, active downloads */
        },
        /**
         * border -- Border/divider colors at varying strengths.
         * Usage: border-border (default), border-border-light, border-border-strong
         * The DEFAULT key means `border-border` class uses the base --border variable.
         */
        border: {
          DEFAULT: 'var(--border)',             /* Standard border color for inputs, cards */
          light: 'var(--border-light)',         /* Subtle dividers, section separators */
          strong: 'var(--border-strong)',       /* Emphasized borders, focused input outlines */
        },
      },

      /**
       * borderRadius -- Platform-adaptive corner rounding.
       *
       * macOS uses larger radii (8px) for the Liquid Glass aesthetic.
       * Windows uses smaller radii (4px) matching Fluent Design.
       * Linux uses moderate radii (6px) matching Adwaita.
       *
       * Usage: rounded-platform, rounded-platform-sm, rounded-platform-lg
       *
       * @see https://tailwindcss.com/docs/border-radius
       */
      borderRadius: {
        platform: 'var(--radius)',             /* Standard component radius (buttons, inputs) */
        'platform-sm': 'var(--radius-sm)',     /* Smaller radius (badges, tags, chips) */
        'platform-lg': 'var(--radius-lg)',     /* Larger radius (cards, modals, panels) */
      },

      /**
       * boxShadow -- Platform-adaptive elevation shadows.
       *
       * macOS uses soft, multi-layer shadows with subtle border effects.
       * Windows uses sharper, more defined shadows (Fluent elevation).
       * Linux uses moderate shadows matching GTK conventions.
       *
       * Usage: shadow-platform, shadow-platform-lg
       *
       * @see https://tailwindcss.com/docs/box-shadow
       */
      boxShadow: {
        platform: 'var(--shadow)',             /* Standard elevation (cards, dropdowns) */
        'platform-lg': 'var(--shadow-lg)',     /* Higher elevation (modals, floating panels) */
      },
    },
  },

  /**
   * plugins -- Array of Tailwind CSS plugins to extend functionality.
   *
   * No plugins are currently used. Potential additions:
   *   - @tailwindcss/forms     -- Better default form element styling
   *   - @tailwindcss/typography -- Prose styling for rendered markdown
   *   - tailwind-scrollbar     -- Custom scrollbar utilities
   *
   * @see https://tailwindcss.com/docs/plugins
   */
  plugins: [],
};
