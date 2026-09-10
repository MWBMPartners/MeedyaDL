/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/test/setup.ts - Vitest global test setup file
 *
 * This file is loaded automatically before each test file runs, as configured
 * in `vitest.config.ts` (or `vite.config.ts`) via the `setupFiles` option:
 * ```ts
 * test: {
 *   setupFiles: ['./src/test/setup.ts'],
 * }
 * ```
 *
 * It performs two critical functions:
 *
 * 1. **Jest-DOM matchers**: Imports `@testing-library/jest-dom` to extend
 *    Vitest's `expect()` with DOM-specific matchers like:
 *    - `toBeInTheDocument()` - element exists in the DOM
 *    - `toHaveTextContent()` - element contains specific text
 *    - `toBeVisible()` - element is visible to the user
 *    - `toHaveClass()` - element has specific CSS classes
 *
 * 2. **Tauri API mocking**: Mocks all Tauri-specific modules that are
 *    unavailable in the Node.js/jsdom test environment. Without these mocks,
 *    any component or module that imports from `@tauri-apps/*` would throw
 *    "Module not found" or "Cannot resolve" errors during testing.
 *
 * Mocking strategy:
 * - All mocks provide safe no-op implementations (return null/undefined/empty)
 * - Individual test files can override these mocks with `vi.mocked()` or
 *   `vi.spyOn()` for specific test scenarios
 * - The `vi.mock()` calls are hoisted by Vitest to the top of each test file,
 *   ensuring they take effect before any imports execute
 *
 * @see {@link https://vitest.dev/config/#setupfiles} - setupFiles config
 * @see {@link https://vitest.dev/api/vi.html#vi-mock} - vi.mock API
 * @see {@link https://testing-library.com/docs/ecosystem-jest-dom/} - jest-dom matchers
 */

/**
 * Import jest-dom matchers as a side-effect module.
 * This extends Vitest's `expect` object with DOM-specific assertion methods.
 * The import is a one-time setup -- matchers are available in all test files.
 *
 * @see {@link https://testing-library.com/docs/ecosystem-jest-dom/#custom-matchers}
 */
import '@testing-library/jest-dom';

/**
 * `initI18n` starts the translation system -- see the `beforeAll` block at
 * the bottom of this file for why every test file needs it running.
 */
import { initI18n } from '@/lib/i18n';

/**
 * Mock the Tauri core `invoke()` API.
 *
 * `invoke()` is the primary IPC mechanism for calling Rust command handlers.
 * In tests, we replace it with a mock that resolves to `null` by default.
 *
 * To test specific command responses, override in individual tests:
 * ```ts
 * import { invoke } from '@tauri-apps/api/core';
 * vi.mocked(invoke).mockResolvedValueOnce({ platform: 'macos', arch: 'aarch64' });
 * ```
 *
 * @see src/lib/tauri-commands.ts - all functions that use invoke()
 * @see {@link https://vitest.dev/api/vi.html#vi-mock}
 */
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

/**
 * Mock the Tauri event API (listen/emit).
 *
 * `listen()` returns a Promise<UnlistenFn> -- our mock returns a no-op function.
 * `emit()` returns a Promise<void> -- our mock resolves to undefined.
 *
 * These mocks prevent errors in components that register Tauri event listeners
 * (e.g., App.tsx's gamdl-output and download lifecycle listeners).
 *
 * @see App.tsx Effects 4-6 for event listener registrations
 * @see {@link https://v2.tauri.app/reference/javascript/api/namespacevent/}
 */
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}), // Returns a no-op unlisten function
  emit: vi.fn().mockResolvedValue(undefined),
}));

/**
 * Mock the Tauri OS plugin for platform detection.
 *
 * The usePlatform hook dynamically imports `@tauri-apps/plugin-os`.
 * This mock provides a default macOS/aarch64 environment for tests.
 *
 * Note: `platform` is mocked as a resolved Promise returning 'macos',
 * although in the real plugin it's synchronous. The usePlatform hook
 * handles both cases since it accesses the value after the dynamic import.
 *
 * @see src/hooks/usePlatform.ts - the hook that consumes this plugin
 * @see {@link https://v2.tauri.app/reference/javascript/plugin-os/}
 */
vi.mock('@tauri-apps/plugin-os', () => ({
  platform: vi.fn().mockResolvedValue('macos'), // Default to macOS in tests
  arch: vi.fn().mockResolvedValue('aarch64'), // Default to ARM64 (Apple Silicon)
  type: vi.fn().mockResolvedValue('macos'), // OS type string
}));

/**
 * Start the translation system once, for every test file, before any test
 * in it runs.
 *
 * Why this has to live here and not in each test file: a growing number of
 * components call `useTranslation()` and render `t('some.key')`. If the
 * translation system has never been started, `t()` has nothing to look
 * up, so it prints the raw key back out (e.g. a button would literally
 * say "queue.title" instead of "Queue"). Any test that then looks for the
 * English words on screen fails -- not because the component is broken,
 * but because nobody told it which language to speak. Doing this once
 * here means every test file gets a working `t()` for free, whether or
 * not that file has heard of i18n at all.
 *
 * This is cheap and needs no network: English is bundled straight into
 * the i18n module (see `src/lib/i18n.ts`), so `initI18n()` resolves
 * synchronously from memory rather than fetching a file. German and
 * French are only fetched over HTTP on demand, which is why they are
 * NOT loaded here -- a test that specifically wants to check German or
 * French text should use `useTestLanguage()` from `src/testing/i18n.ts`
 * instead, which imports those files directly.
 *
 * @see src/lib/i18n.ts - the translation system this starts
 * @see src/testing/i18n.ts - the helper for testing non-English text
 */
beforeAll(async () => {
  await initI18n();
});
