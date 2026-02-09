/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Vite configuration for the gamdl-GUI frontend build.
 * Configures React plugin, path aliases, and Tauri-specific settings
 * including the dev server port and file watching exclusions.
 */

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

// https://vitejs.dev/config/
export default defineConfig({
  // Register the React plugin for JSX/TSX transformation and Fast Refresh
  plugins: [react()],

  resolve: {
    // Allow importing from '@/' as an alias for the 'src/' directory
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  // Prevent Vite from obscuring Rust errors in the terminal
  clearScreen: false,

  server: {
    // Tauri expects a fixed port; fail if that port is not available
    port: 1420,
    strictPort: true,

    // Watch for changes but ignore Tauri backend files (Rust handles its own rebuild)
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
});
