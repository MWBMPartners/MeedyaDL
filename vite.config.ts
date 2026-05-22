/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Vite Configuration for MeedyaDL Frontend
 * ==========================================
 *
 * This file configures Vite as the frontend build tool and dev server for the
 * Tauri 2.0 desktop application. Vite handles:
 *   - TypeScript/JSX transpilation via the React plugin (uses Oxc in v6+)
 *   - Hot Module Replacement (HMR) during development via React Fast Refresh
 *   - Production bundling with Rolldown (tree-shaking, code-splitting, minification)
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
import { readFileSync } from 'fs';

// Read version from package.json at build time for use in Sentry release tags
const pkg = JSON.parse(readFileSync(path.resolve(__dirname, 'package.json'), 'utf-8'));

/**
 * defineConfig() provides type hints and auto-completion for Vite options.
 * @see https://vite.dev/config/#config-intellisense
 */
export default defineConfig({
  /**
   * define -- Compile-time constants replaced in source code during build.
   * `__APP_VERSION__` is used by the Sentry SDK integration in main.tsx
   * for the release tag (e.g., "meedyadl@0.5.3").
   * @see https://vite.dev/config/shared-options.html#define
   */
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },

  /**
   * plugins -- Array of Vite plugins to apply during build and dev.
   *
   * @vitejs/plugin-react enables:
   *   - JSX/TSX transformation using Oxc (fast compilation, Babel-free in v6+)
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
  /**
   * envPrefix -- Environment variable prefixes exposed to the frontend code.
   *
   * Variables matching these prefixes are available via `import.meta.env.*`:
   *   - 'VITE_':       Standard Vite convention for app-specific env vars
   *   - 'TAURI_ENV_*': Tauri 2.0 injects build metadata (TAURI_ENV_PLATFORM,
   *     TAURI_ENV_ARCH, TAURI_ENV_DEBUG, etc.) that can be used at build time
   *     to conditionally configure the build (e.g., platform-specific targets).
   *
   * @see https://vite.dev/config/shared-options.html#envprefix
   * @see https://v2.tauri.app/start/frontend/vite/ -- Tauri Vite integration
   */
  envPrefix: ['VITE_', 'TAURI_ENV_*'],

  /**
   * build -- Production build configuration.
   *
   * These settings follow the official Tauri 2.0 Vite integration guide to
   * ensure the compiled JavaScript is compatible with each platform's WebView engine:
   *   - macOS/Linux: WebKit (Safari-based) -- target safari16.4
   *   - Windows: Chromium (Edge WebView2) -- target chrome111
   *
   * The TAURI_ENV_PLATFORM environment variable is set by the Tauri CLI during
   * `cargo tauri build` and `cargo tauri dev`, allowing platform-conditional config.
   *
   * @see https://v2.tauri.app/start/frontend/vite/ -- Official Tauri Vite config
   * @see https://esbuild.github.io/api/#target -- esbuild target documentation
   */
  build: {
    /**
     * target -- Browser compatibility target for the bundler output.
     *
     * Controls which JavaScript syntax features will be downleveled:
     *   - 'safari16.4': Ensures compatibility with WebKit on macOS 13.3+ and Linux.
     *     Required by Tailwind CSS v4 which uses modern CSS features (oklch, @property).
     *   - 'chrome111': Ensures compatibility with Edge WebView2 on Windows.
     *     Aligns with Tailwind CSS v4's minimum browser requirements.
     *
     * Without this, Vite defaults to 'modules' (browsers supporting ES modules),
     * which may include syntax too modern for the Tauri WebView on older OS versions.
     */
    target:
      process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome111' : 'safari16.4',

    /**
     * minify -- Minification and obfuscation strategy.
     *
     * Release builds use 'terser' for aggressive minification + name mangling
     * that makes the compiled JS harder to reverse-engineer. Terser runs at
     * build time only — zero runtime performance impact (the output is just
     * standard JavaScript with mangled variable/function names).
     *
     * Debug builds disable minification entirely for readable DevTools output.
     *
     * @see https://terser.org/docs/options/ -- Terser options reference
     */
    minify: !process.env.TAURI_ENV_DEBUG ? 'terser' : false,

    /**
     * terserOptions -- Terser minification and obfuscation settings.
     *
     * These options provide source code protection for release builds:
     * - **mangle**: Renames local variables, function parameters, and class
     *   names to short identifiers (a, b, c...). `toplevel: true` also mangles
     *   top-level declarations.
     * - **compress**: Removes dead code, inlines constants, collapses
     *   variable declarations, and drops `console.log` calls (debug noise).
     *   `passes: 2` runs compression twice for better optimisation.
     * - **format**: Removes all comments and whitespace from output.
     *
     * IMPORTANT: Do NOT enable `mangle.properties` — it renames object property
     * names across the entire bundle, breaking React internals (._reactInternals,
     * ._payload, ._init, etc.), Zustand, and other libraries that rely on
     * underscore-prefixed properties for internal state. This causes a blank
     * screen in production builds while dev mode works fine.
     *
     * Performance: zero runtime overhead — all processing happens at build time.
     * The output is valid, functional JavaScript; just much harder to read.
     */
    terserOptions: !process.env.TAURI_ENV_DEBUG
      ? {
          mangle: {
            toplevel: true, // Mangle top-level names (module scope)
          },
          compress: {
            drop_console: true, // Remove console.log/warn/error in production
            drop_debugger: true, // Remove debugger statements
            passes: 2, // Run compression twice for better results
            pure_getters: true, // Assume getters have no side effects
            unsafe_arrows: true, // Convert functions to arrows where possible
          },
          format: {
            comments: false, // Strip all comments
          },
        }
      : undefined,

    /**
     * sourcemap -- Generate source maps only for debug builds.
     *
     * Source maps add significant bundle size and expose source code structure.
     * They are useful during development (WebKit Inspector / Chrome DevTools)
     * but should not be included in release builds.
     */
    sourcemap: !!process.env.TAURI_ENV_DEBUG,

    /**
     * `chunkSizeWarningLimit` -- Threshold (in kB) for the "chunk too large" warning.
     *
     * The default is 500 kB. MeedyaDL's main bundle is ~605 kB minified (~180 kB
     * gzipped) due to the feature-rich UI (markdown rendering, i18n, Zustand stores,
     * multiple page components). This is acceptable for a desktop app where the
     * bundle is loaded from the local filesystem, not over a network.
     */
    chunkSizeWarningLimit: 700,

    rolldownOptions: {
      onwarn(warning, warn) {
        // Suppress the "dynamically imported by X but also statically imported by Y"
        // warning for uiStore.ts. The dynamic imports in main.tsx's global error
        // handlers are intentional — they avoid circular dependencies during early
        // bootstrap, while other components statically import the same store.
        if (
          warning.code === 'MIXED_IMPORTS' ||
          warning.code === 'INEFFECTIVE_DYNAMIC_IMPORT' ||
          (warning.message?.includes('is dynamically imported') &&
            warning.message?.includes('uiStore'))
        ) {
          return;
        }
        warn(warning);
      },
    },
  },

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
