/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Commitlint configuration for enforcing conventional commit messages.
 * Commit messages must follow the format: type(scope): description
 * Example: feat(download): add fallback quality chain support
 */

export default {
  // Extend the standard conventional commit configuration
  extends: ['@commitlint/config-conventional'],

  rules: {
    // Enforce specific commit type prefixes
    'type-enum': [
      2, // Error level (block commit)
      'always',
      [
        'feat',     // New feature
        'fix',      // Bug fix
        'docs',     // Documentation only
        'style',    // Formatting, no code change
        'refactor', // Code restructuring, no behavior change
        'perf',     // Performance improvement
        'test',     // Adding or fixing tests
        'build',    // Build system or dependency changes
        'ci',       // CI/CD pipeline changes
        'chore',    // Maintenance tasks
        'revert',   // Revert a previous commit
      ],
    ],
  },
};
