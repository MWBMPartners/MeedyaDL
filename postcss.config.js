/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * PostCSS configuration for processing Tailwind CSS directives
 * and adding vendor prefixes via Autoprefixer.
 */

export default {
  plugins: {
    // Process Tailwind CSS @tailwind directives and utility classes
    tailwindcss: {},
    // Add vendor prefixes (-webkit-, -moz-, etc.) for cross-browser compatibility
    autoprefixer: {},
  },
};
