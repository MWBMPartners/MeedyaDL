/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Vitest test setup file.
 * Loaded before each test file to configure the test environment.
 * Adds @testing-library/jest-dom matchers (toBeInTheDocument, etc.)
 * and mocks Tauri APIs that are unavailable in the test environment.
 */

import '@testing-library/jest-dom';

/**
 * Mock the Tauri invoke API so that IPC commands don't fail in tests.
 * Individual tests can override these mocks for specific test cases.
 */
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

/**
 * Mock the Tauri event API so that listen/emit don't fail in tests.
 */
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn().mockResolvedValue(undefined),
}));

/**
 * Mock the Tauri OS plugin for platform detection.
 */
vi.mock('@tauri-apps/plugin-os', () => ({
  platform: vi.fn().mockResolvedValue('macos'),
  arch: vi.fn().mockResolvedValue('aarch64'),
  type: vi.fn().mockResolvedValue('macos'),
}));
