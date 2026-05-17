/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * PostCSS Configuration for MeedyaDL
 * ====================================
 *
 * PostCSS is a CSS processing pipeline that Vite invokes automatically for every
 * CSS file imported in the application.
 *
 * In Tailwind CSS v4, the PostCSS plugin has moved to @tailwindcss/postcss.
 * Autoprefixer and CSS imports are handled automatically by Tailwind v4's
 * built-in LightningCSS integration, so only a single plugin is needed.
 *
 * Related config files:
 *   - tailwind.config.js    -- JS-based theme config (loaded via @config in globals.css)
 *   - vite.config.ts        -- Vite automatically discovers and runs this PostCSS config
 *   - src/styles/globals.css -- Contains @import "tailwindcss" and @config directive
 *
 * @see https://tailwindcss.com/docs/installation/using-postcss
 */

export default {
  plugins: {
    /**
     * @tailwindcss/postcss -- Processes Tailwind CSS v4 directives.
     *
     * Replaces both the old `tailwindcss` and `autoprefixer` plugins.
     * Handles utility generation, content scanning, vendor prefixing,
     * and CSS nesting via LightningCSS.
     *
     * Configuration is loaded via the @config directive in globals.css
     * pointing to tailwind.config.js.
     */
    '@tailwindcss/postcss': {},
  },
};
