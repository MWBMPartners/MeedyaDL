/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * PostCSS Configuration for MeedyaDL
 * ====================================
 *
 * PostCSS is a CSS processing pipeline that Vite invokes automatically for every
 * CSS file imported in the application. It runs plugins in the order they are listed,
 * transforming CSS through each stage.
 *
 * The processing pipeline:
 *   1. tailwindcss -- Processes @tailwind directives in globals.css, scans source files
 *      for class usage (per tailwind.config.js 'content' setting), and generates the
 *      corresponding utility CSS. Also handles @apply and @layer directives.
 *   2. autoprefixer -- Scans the resulting CSS and adds vendor prefixes (-webkit-,
 *      -moz-, -ms-) based on the target browser list. Uses the 'browserslist' config
 *      from package.json (or defaults to "defaults" if none is specified).
 *
 * Related config files:
 *   - tailwind.config.js  -- Tailwind reads this for theme, content paths, and plugins
 *   - vite.config.ts      -- Vite automatically discovers and runs this PostCSS config
 *   - src/styles/globals.css -- Contains the @tailwind base/components/utilities directives
 *   - package.json        -- May contain a "browserslist" key for Autoprefixer targets
 *
 * @see https://postcss.org/ -- PostCSS documentation
 * @see https://tailwindcss.com/docs/using-with-preprocessors -- Tailwind + PostCSS setup
 * @see https://github.com/postcss/autoprefixer -- Autoprefixer documentation
 * @see https://vite.dev/guide/features.html#postcss -- How Vite integrates PostCSS
 */

export default {
  /**
   * plugins -- PostCSS plugins to apply, in order.
   *
   * Using the object syntax (plugin name as key, options as value) which is the
   * shorthand format. An empty object `{}` means "use default options".
   * Order matters: Tailwind must run first to generate utility CSS before
   * Autoprefixer can add vendor prefixes to the output.
   *
   * @see https://postcss.org/docs/postcss-runner-guidelines
   */
  plugins: {
    /**
     * tailwindcss -- Processes Tailwind CSS directives (@tailwind, @apply, @layer).
     *
     * This plugin reads tailwind.config.js from the project root, scans the 'content'
     * files for Tailwind class usage, and generates optimized CSS. In development mode
     * it generates all utilities for instant HMR; in production mode it tree-shakes
     * unused classes for a minimal bundle.
     *
     * Empty options `{}` means: auto-detect tailwind.config.js from the project root.
     *
     * @see https://tailwindcss.com/docs/installation/using-postcss
     */
    tailwindcss: {},

    /**
     * autoprefixer -- Adds CSS vendor prefixes for cross-browser compatibility.
     *
     * Parses the generated CSS and adds prefixes like -webkit-backdrop-filter,
     * -webkit-user-select, etc. based on the target browser list. This is critical
     * for our platform themes because:
     *   - backdrop-filter (used in macos.css Liquid Glass effects) needs -webkit- for Safari
     *   - user-select (used in globals.css) needs -webkit- for older browsers
     *   - app-region (used for window dragging) needs -webkit-app-region
     *
     * Target browsers are determined by 'browserslist' in package.json or a .browserslistrc file.
     * Default targets cover >0.2% market share browsers, not dead, last 2 versions.
     *
     * Empty options `{}` means: use default browserslist configuration.
     *
     * @see https://github.com/postcss/autoprefixer#options
     * @see https://browsersl.ist/ -- Interactive browserslist query tool
     */
    autoprefixer: {},
  },
};
