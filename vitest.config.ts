/**
 * Copyright (c) 2024-2026 MeedyaDL
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Vitest Configuration for MeedyaDL Frontend Tests
 * ==================================================
 *
 * Vitest is the test runner for the React frontend. It is built on top of Vite,
 * sharing the same plugin pipeline and transformation logic, which means tests
 * run with the same JSX/TypeScript handling as the dev server and production build.
 *
 * This configuration mirrors several settings from vite.config.ts (plugins, resolve.alias)
 * to ensure test code resolves imports identically to application code.
 *
 * Related config files:
 *   - vite.config.ts       -- Main build config; the resolve.alias '@' must match here
 *   - tsconfig.json         -- TypeScript paths must include "@/*" for type resolution
 *   - src/test/setup.ts     -- Test setup file that registers @testing-library/jest-dom matchers
 *
 * @see https://vitest.dev/config/ -- Vitest configuration reference
 * @see https://vitest.dev/guide/ -- Vitest getting started guide
 * @see https://testing-library.com/docs/react-testing-library/intro/ -- React Testing Library docs
 */

import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

/**
 * defineConfig() from 'vitest/config' extends Vite's defineConfig with test-specific type hints.
 * @see https://vitest.dev/config/#configuration
 */
export default defineConfig({
  /**
   * plugins -- Same React plugin as vite.config.ts.
   *
   * Required here so that JSX/TSX in test files and imported components is
   * correctly transformed. Without this, importing .tsx components in tests would fail.
   *
   * @see https://vitest.dev/config/#plugins
   */
  plugins: [react()],

  resolve: {
    /**
     * alias -- Must mirror the alias in vite.config.ts exactly.
     *
     * Without this, imports like `import { Button } from '@/components/ui/Button'`
     * would fail to resolve in test files. This is the most common source of
     * "module not found" errors in Vitest when migrating from a Vite project.
     *
     * @see https://vitest.dev/config/#resolve-alias
     */
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  test: {
    /**
     * environment -- Specifies the DOM implementation for testing React components.
     *
     * 'jsdom' provides a pure-JavaScript browser-like environment (window, document, etc.)
     * that React components can render into. Alternative options include:
     *   - 'happy-dom' -- Faster but less complete DOM implementation
     *   - 'node'      -- No DOM (for pure logic/utility tests)
     *
     * Requires the 'jsdom' package as a devDependency.
     *
     * @see https://vitest.dev/config/#environment
     * @see https://github.com/jsdom/jsdom -- jsdom documentation
     */
    environment: 'jsdom',

    /**
     * setupFiles -- Scripts to run before each test file.
     *
     * './src/test/setup.ts' imports '@testing-library/jest-dom' which adds custom
     * matchers like toBeInTheDocument(), toHaveTextContent(), toBeVisible(), etc.
     * These matchers make DOM assertions more readable and produce better error messages.
     *
     * @see https://vitest.dev/config/#setupfiles
     * @see https://github.com/testing-library/jest-dom -- jest-dom matchers reference
     */
    setupFiles: ['./src/test/setup.ts'],

    /**
     * include -- Glob patterns that identify test files.
     *
     * Matches any file in src/ with a .test.ts, .test.tsx, .spec.ts, or .spec.tsx extension.
     * By convention:
     *   - .test.* files are unit tests (co-located next to source files)
     *   - .spec.* files are integration/behavior tests
     *
     * @see https://vitest.dev/config/#include
     */
    include: ['src/**/*.{test,spec}.{ts,tsx}'],

    /**
     * globals -- When true, injects test functions (describe, it, expect, vi, beforeEach, etc.)
     * into the global scope so they don't need to be imported in each test file.
     *
     * This matches the Jest convention and reduces boilerplate. Without this, every test
     * file would need: `import { describe, it, expect } from 'vitest'`
     *
     * Requires "types": ["vitest/globals"] in tsconfig.json for TypeScript support.
     *
     * @see https://vitest.dev/config/#globals
     */
    globals: true,

    /**
     * css -- When false, CSS imports are mocked/ignored in tests.
     *
     * This prevents Vitest from trying to parse CSS files (including Tailwind utility classes),
     * which would be slow and unnecessary since we are testing component behavior, not styling.
     * CSS modules would return empty objects, and regular CSS imports become no-ops.
     *
     * @see https://vitest.dev/config/#css
     */
    css: false,
  },
});
