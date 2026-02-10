/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Vitest configuration for the gamdl-GUI frontend tests.
 * Uses jsdom for DOM simulation, the same path aliases as the main
 * Vite config, and @testing-library/jest-dom for extended matchers.
 */

import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  test: {
    /* Use jsdom to simulate a browser environment for React components */
    environment: 'jsdom',

    /* Load @testing-library/jest-dom matchers before each test file */
    setupFiles: ['./src/test/setup.ts'],

    /* Test file patterns */
    include: ['src/**/*.{test,spec}.{ts,tsx}'],

    /* Enable global test functions (describe, it, expect) without imports */
    globals: true,

    /* CSS handling: don't try to parse CSS in tests */
    css: false,
  },
});
