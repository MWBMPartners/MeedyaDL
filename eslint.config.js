// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// ESLint Flat Configuration for gamdl-GUI Frontend
// =================================================
//
// This file uses the ESLint 9.x "flat config" format (eslint.config.js), which replaces
// the legacy .eslintrc format. Flat configs are arrays of configuration objects that are
// merged in order, with later entries overriding earlier ones.
//
// The configuration provides:
//   - JavaScript recommended rules (baseline quality checks)
//   - TypeScript type-aware linting via @typescript-eslint
//   - React Hooks correctness enforcement
//   - React Fast Refresh compatibility checking
//
// Related config files:
//   - tsconfig.json        -- TypeScript compiler options that the parser uses
//   - vite.config.ts       -- Vite's dev server relies on ESLint for lint-on-save (if configured)
//   - .github/workflows/ci.yml -- CI runs `npm run lint` which invokes ESLint with this config
//
// @see https://eslint.org/docs/latest/use/configure/configuration-files -- Flat config format
// @see https://eslint.org/docs/latest/use/configure/configuration-files#configuration-objects -- Config object shape
// @see https://typescript-eslint.io/getting-started -- TypeScript ESLint setup guide
// @see https://github.com/facebook/react/tree/main/packages/eslint-plugin-react-hooks -- React Hooks plugin

import js from '@eslint/js';
import tsPlugin from '@typescript-eslint/eslint-plugin';
import tsParser from '@typescript-eslint/parser';
import reactHooksPlugin from 'eslint-plugin-react-hooks';
import reactRefreshPlugin from 'eslint-plugin-react-refresh';

export default [
  /**
   * Config object #1: ESLint's built-in recommended rules.
   *
   * This is a pre-defined configuration from @eslint/js that enables rules like:
   *   - no-debugger, no-duplicate-case, no-empty, no-extra-semi, etc.
   * These rules apply to ALL files by default (no 'files' restriction).
   *
   * @see https://eslint.org/docs/latest/rules/ -- Full rules reference
   * @see https://eslint.org/docs/latest/use/configure/configuration-files#using-predefined-configurations
   */
  js.configs.recommended,

  /**
   * Config object #2: TypeScript + React configuration for source files.
   *
   * This configuration block targets only .ts and .tsx files in the src/ directory.
   * It sets up the TypeScript parser, browser globals, and all project-specific rules.
   */
  {
    /**
     * files -- Glob patterns for files this config applies to.
     *
     * Only TypeScript/TSX source files in src/ are linted. Config files, scripts,
     * and Rust source files are excluded (handled by the 'ignores' block below).
     *
     * @see https://eslint.org/docs/latest/use/configure/configuration-files#specifying-files-and-ignores
     */
    files: ['src/**/*.{ts,tsx}'],

    languageOptions: {
      /**
       * parser -- Use the TypeScript parser instead of ESLint's default JavaScript parser.
       *
       * @typescript-eslint/parser understands TypeScript syntax (interfaces, generics,
       * type annotations, enums, etc.) that the default parser would reject as errors.
       *
       * @see https://typescript-eslint.io/packages/parser/
       */
      parser: tsParser,

      parserOptions: {
        /**
         * ecmaVersion -- JavaScript language version to support.
         * 'latest' enables all finalized ECMAScript features (ES2024+).
         * @see https://eslint.org/docs/latest/use/configure/language-options#specifying-parser-options
         */
        ecmaVersion: 'latest',

        /**
         * sourceType -- Treat files as ES modules (import/export syntax).
         * @see https://eslint.org/docs/latest/use/configure/language-options#specifying-parser-options
         */
        sourceType: 'module',

        /**
         * ecmaFeatures.jsx -- Enable JSX parsing in .tsx files.
         * Required for React component syntax like <Component prop="value" />.
         */
        ecmaFeatures: { jsx: true },
      },

      /**
       * globals -- Declare global variables available in the browser environment.
       *
       * Since we're not using eslint-plugin-environments (removed in flat config),
       * browser globals must be declared explicitly. 'readonly' means these variables
       * exist but should not be reassigned (e.g., `window = {}` would trigger an error).
       *
       * @see https://eslint.org/docs/latest/use/configure/language-options#specifying-globals
       */
      globals: {
        // DOM and Browser APIs
        window: 'readonly',          // The global window object
        document: 'readonly',        // DOM document access
        navigator: 'readonly',       // Browser/OS detection (used for platform detection)
        console: 'readonly',         // Logging (console.log, console.error)

        // Timer APIs (used in download queue polling, debouncing)
        setTimeout: 'readonly',
        clearTimeout: 'readonly',
        setInterval: 'readonly',
        clearInterval: 'readonly',

        // Network and URL APIs
        fetch: 'readonly',           // HTTP fetch API (not used directly; Tauri IPC preferred)
        URL: 'readonly',             // URL parsing for Apple Music links

        // DOM element types (used in TypeScript type annotations)
        HTMLElement: 'readonly',
        HTMLInputElement: 'readonly',
        KeyboardEvent: 'readonly',
        MouseEvent: 'readonly',
        Event: 'readonly',
        EventTarget: 'readonly',

        // Node.js types (used for setTimeout return type in TypeScript)
        NodeJS: 'readonly',
      },
    },

    /**
     * plugins -- Register ESLint plugins that provide additional rules.
     *
     * In flat config format, plugins are registered as key-value pairs where
     * the key becomes the rule prefix (e.g., '@typescript-eslint/no-unused-vars').
     *
     * @see https://eslint.org/docs/latest/use/configure/plugins
     */
    plugins: {
      /** TypeScript-specific lint rules (type safety, best practices) */
      '@typescript-eslint': tsPlugin,
      /** React Hooks correctness rules (rules-of-hooks, exhaustive-deps) */
      'react-hooks': reactHooksPlugin,
      /** React Refresh compatibility (ensures components work with HMR) */
      'react-refresh': reactRefreshPlugin,
    },

    rules: {
      /**
       * Spread the @typescript-eslint recommended ruleset as a baseline.
       * This includes rules like no-explicit-any (warn), no-non-null-assertion (warn),
       * prefer-as-const, ban-types, etc.
       *
       * @see https://typescript-eslint.io/rules/?=recommended -- Full recommended rules list
       */
      ...tsPlugin.configs.recommended.rules,

      /**
       * @typescript-eslint/no-unused-vars -- Warn on unused variables/parameters.
       *
       * Configured to ignore variables and arguments prefixed with underscore (_).
       * This is a common convention in both Rust and TypeScript for intentionally
       * unused bindings (e.g., `const [_loading, setLoading] = useState(false)`).
       *
       * Severity: 'warn' (not error) -- unused vars are a code smell but shouldn't
       * block development. CI can optionally promote warnings to errors.
       *
       * @see https://typescript-eslint.io/rules/no-unused-vars
       */
      '@typescript-eslint/no-unused-vars': [
        'warn',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],

      /**
       * react-hooks/rules-of-hooks -- Enforces the Rules of Hooks.
       *
       * Ensures hooks (useState, useEffect, etc.) are:
       *   - Only called at the top level (not inside loops, conditions, or nested functions)
       *   - Only called from React function components or custom hooks
       *
       * Severity: 'error' -- Violating the rules of hooks causes runtime bugs that are
       * extremely difficult to debug (stale state, infinite re-renders, etc.).
       *
       * @see https://react.dev/reference/rules/rules-of-hooks
       */
      'react-hooks/rules-of-hooks': 'error',

      /**
       * react-hooks/exhaustive-deps -- Validates dependency arrays in hooks.
       *
       * Warns when useEffect, useMemo, or useCallback dependency arrays are missing
       * values that are used inside the hook. Missing dependencies cause stale closures.
       *
       * Severity: 'warn' -- Sometimes intentional omissions are necessary (e.g., stable refs).
       *
       * @see https://react.dev/reference/react/useEffect#specifying-reactive-dependencies
       */
      'react-hooks/exhaustive-deps': 'warn',

      /**
       * react-refresh/only-export-components -- Ensures HMR-compatible exports.
       *
       * React Fast Refresh (HMR) requires that each module exports only React components
       * or a limited set of constant values. Mixing component exports with utility
       * functions/constants can break Fast Refresh, forcing full page reloads.
       *
       * allowConstantExport: true -- Permits `export const SOME_CONSTANT = ...` alongside
       * component exports, which is a common pattern for co-locating configuration.
       *
       * @see https://github.com/ArnaudBarre/eslint-plugin-react-refresh
       */
      'react-refresh/only-export-components': [
        'warn',
        { allowConstantExport: true },
      ],

      /**
       * no-undef -- Disabled because TypeScript's own type checker handles undefined
       * variable detection far more accurately (it understands type imports, ambient
       * declarations, and module augmentation).
       *
       * @see https://typescript-eslint.io/troubleshooting/faqs/eslint#i-get-errors-from-the-no-undef-rule-about-global-variables-not-being-defined-even-though-there-are-no-typescript-errors
       */
      'no-undef': 'off',

      /**
       * no-unused-vars -- Disabled in favor of @typescript-eslint/no-unused-vars.
       * The base ESLint rule doesn't understand TypeScript type-only imports and would
       * produce false positives for type annotations, interfaces, and enums.
       *
       * @see https://typescript-eslint.io/rules/no-unused-vars#how-to-use
       */
      'no-unused-vars': 'off',
    },
  },

  /**
   * Config object #3: Global ignore patterns.
   *
   * Files and directories matching these patterns are completely excluded from linting.
   * This block uses only 'ignores' (no 'files' or 'rules'), making it a global ignore
   * that applies to all configuration objects.
   *
   *   - dist/         -- Vite production build output (generated, not authored)
   *   - node_modules/ -- Third-party dependencies (never lint these)
   *   - src-tauri/    -- Rust backend code (linted by cargo clippy, not ESLint)
   *   - *.config.js   -- Config files themselves (simple, rarely need linting)
   *   - *.config.ts   -- TypeScript config files (like vite.config.ts, vitest.config.ts)
   *
   * @see https://eslint.org/docs/latest/use/configure/configuration-files#globally-ignoring-files-with-ignores
   */
  {
    ignores: ['dist/', 'node_modules/', 'src-tauri/', '*.config.js', '*.config.ts'],
  },
];
