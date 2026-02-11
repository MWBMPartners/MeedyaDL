/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Vite Configuration for gamdl-GUI Frontend
 * ==========================================
 *
 * This file configures Vite as the frontend build tool and dev server for the
 * Tauri 2.0 desktop application. Vite handles:
 *   - TypeScript/JSX transpilation via the React plugin (uses esbuild under the hood)
 *   - Hot Module Replacement (HMR) during development via React Fast Refresh
 *   - Production bundling with Rollup (tree-shaking, code-splitting, minification)
 *   - Dev server that Tauri's WebView connects to during development
 *
 * Related config files:
 *   - vitest.config.ts  -- Test runner config; mirrors the resolve.alias settings from here
 *   - tsconfig.json     -- TypeScript compiler options; the "@/*" path alias here must match
 *   - postcss.config.js -- PostCSS pipeline (Tailwind + Autoprefixer) invoked by Vite automatically
 *   - tailwind.config.js -- Tailwind CSS configuration consumed by the PostCSS plugin
 *   - src-tauri/tauri.conf.json -- Tauri config that references devUrl (port 1420) and frontendDist
 *
 * @see https://vite.dev/config/ -- Vite configuration reference
 * @see https://v2.tauri.app/start/frontend/vite/ -- Tauri 2.0 Vite integration guide
 * @see https://github.com/vitejs/vite-plugin-react -- @vitejs/plugin-react documentation
 */

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

/**
 * defineConfig() provides type hints and auto-completion for Vite options.
 * @see https://vite.dev/config/#config-intellisense
 */
export default defineConfig({
  /**
   * plugins -- Array of Vite plugins to apply during build and dev.
   *
   * @vitejs/plugin-react enables:
   *   - JSX/TSX transformation using esbuild (fast compilation)
   *   - React Fast Refresh for instant HMR without losing component state
   *   - Automatic JSX runtime (no need to `import React from 'react'`)
   *
   * @see https://vite.dev/guide/using-plugins
   * @see https://github.com/vitejs/vite-plugin-react#readme
   */
  plugins: [react()],

  resolve: {
    /**
     * alias -- Map import paths to filesystem directories.
     *
     * '@' is mapped to the 'src/' directory, enabling clean imports like:
     *   import { Button } from '@/components/ui/Button'
     * instead of fragile relative paths:
     *   import { Button } from '../../../components/ui/Button'
     *
     * IMPORTANT: This alias must also be defined in tsconfig.json ("paths": {"@/*": ["./src/*"]})
     * for TypeScript to resolve types correctly, and in vitest.config.ts for tests.
     *
     * @see https://vite.dev/config/shared-options.html#resolve-alias
     */
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  /**
   * clearScreen -- When false, prevents Vite from clearing the terminal on each rebuild.
   *
   * This is important for Tauri development because:
   *   - Tauri's Rust backend logs errors to the same terminal
   *   - Clearing the screen would hide Rust compilation errors and panics
   *   - Both frontend (Vite) and backend (Cargo) output need to remain visible
   *
   * @see https://vite.dev/config/server-options.html#server-clearscreen
   */
  clearScreen: false,

  server: {
    /**
     * port -- Fixed TCP port for the Vite dev server.
     *
     * Set to 1420 because Tauri's tauri.conf.json specifies "devUrl": "http://localhost:1420".
     * The WebView loads this URL during development. This value must match exactly.
     *
     * @see https://vite.dev/config/server-options.html#server-port
     * @see src-tauri/tauri.conf.json -- build.devUrl must reference this port
     */
    port: 1420,

    /**
     * strictPort -- When true, Vite will exit with an error if port 1420 is already in use
     * instead of silently trying the next available port. This prevents Tauri from
     * connecting to a stale server or the wrong port.
     *
     * @see https://vite.dev/config/server-options.html#server-strictport
     */
    strictPort: true,

    /**
     * watch -- File system watcher configuration (uses chokidar under the hood).
     *
     * The 'ignored' patterns tell Vite's HMR watcher to skip the src-tauri/ directory
     * because Rust source files are handled by Cargo's own file watcher (cargo-watch or
     * Tauri CLI's built-in watcher). Watching Rust files would cause unnecessary
     * frontend rebuilds and potential race conditions.
     *
     * @see https://vite.dev/config/server-options.html#server-watch
     * @see https://github.com/paulmillr/chokidar#path-filtering -- chokidar ignore syntax
     */
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
});
