/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Tailwind CSS configuration for gamdl-GUI.
 * Defines platform-adaptive design tokens via CSS custom properties,
 * custom font families for each OS, and color palette extensions
 * that reference CSS variables set by the active platform theme.
 */

/** @type {import('tailwindcss').Config} */
export default {
  // Scan these files for Tailwind class usage (tree-shaking unused styles)
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],

  theme: {
    extend: {
      // Platform-specific font stacks (applied via CSS class on <html>)
      fontFamily: {
        // macOS: San Francisco system font
        'sf-pro': [
          '-apple-system',
          'BlinkMacSystemFont',
          'SF Pro Display',
          'SF Pro Text',
          'sans-serif',
        ],
        // Windows: Segoe UI Variable (Windows 11) with fallbacks
        segoe: ['Segoe UI Variable', 'Segoe UI', 'sans-serif'],
        // Linux/generic: System default font stack
        system: ['system-ui', '-apple-system', 'sans-serif'],
      },

      // Colors that adapt to the active platform theme via CSS custom properties
      colors: {
        // Sidebar navigation colors
        sidebar: {
          bg: 'var(--sidebar-bg)',
          hover: 'var(--sidebar-hover)',
          active: 'var(--sidebar-active)',
          text: 'var(--sidebar-text)',
          'text-active': 'var(--sidebar-text-active)',
          border: 'var(--sidebar-border)',
        },
        // Content surface colors (backgrounds, cards)
        surface: {
          primary: 'var(--surface-primary)',
          secondary: 'var(--surface-secondary)',
          elevated: 'var(--surface-elevated)',
          overlay: 'var(--surface-overlay)',
        },
        // Accent colors for interactive elements
        accent: {
          DEFAULT: 'var(--accent)',
          hover: 'var(--accent-hover)',
          light: 'var(--accent-light)',
        },
        // Text colors
        content: {
          primary: 'var(--text-primary)',
          secondary: 'var(--text-secondary)',
          tertiary: 'var(--text-tertiary)',
          inverse: 'var(--text-inverse)',
        },
        // Status colors for download states
        status: {
          success: 'var(--status-success)',
          warning: 'var(--status-warning)',
          error: 'var(--status-error)',
          info: 'var(--status-info)',
        },
        // Border colors
        border: {
          DEFAULT: 'var(--border)',
          light: 'var(--border-light)',
          strong: 'var(--border-strong)',
        },
      },

      // Platform-adaptive border radius values
      borderRadius: {
        platform: 'var(--radius)',
        'platform-sm': 'var(--radius-sm)',
        'platform-lg': 'var(--radius-lg)',
      },

      // Box shadow presets for elevation
      boxShadow: {
        platform: 'var(--shadow)',
        'platform-lg': 'var(--shadow-lg)',
      },
    },
  },

  // Tailwind plugins
  plugins: [],
};
