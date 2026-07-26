/**
 * Copyright (c) 2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/stores/updateStore.test.ts - Unit tests for the app-component
 * update gate.
 *
 * Regression coverage for a bug where the frontend compared
 * `ComponentUpdate.name` against the string `'app'`, but the Rust backend
 * always emits the app component as `name: "MeedyaDL"`
 * (`update_checker.rs::parse_release_from_response` and its two early-return
 * paths). The mismatch meant `findAppComponentUpdate()` — and every gate
 * built on top of it (the first-run update prompt, the pre-release notice's
 * stable-release offer) — was permanently dead: it always returned
 * `undefined` even when a genuine app update was available.
 *
 * @see src/stores/updateStore.ts - `APP_COMPONENT_NAME` + `findAppComponentUpdate`
 */

import { describe, expect, it } from 'vitest';
import { APP_COMPONENT_NAME, findAppComponentUpdate } from './updateStore';
import type { ComponentUpdate } from '@/types';

/** Minimal valid `ComponentUpdate` fixture; tests override only what they need. */
function makeComponent(overrides: Partial<ComponentUpdate>): ComponentUpdate {
  return {
    name: 'GAMDL',
    current_version: '1.0.0',
    latest_version: '1.0.0',
    update_available: false,
    is_compatible: true,
    is_untested: false,
    no_compatible_wheel: false,
    description: null,
    release_url: null,
    release_body: null,
    is_prerelease: false,
    tag_name: null,
    pip_package: null,
    tool_id: null,
    ...overrides,
  };
}

describe('findAppComponentUpdate', () => {
  it('finds an actionable MeedyaDL app update', () => {
    const components = [
      makeComponent({ name: APP_COMPONENT_NAME, update_available: true, is_compatible: true }),
    ];

    const result = findAppComponentUpdate(components);

    expect(result).toBeDefined();
    expect(result?.name).toBe('MeedyaDL');
  });

  it('does NOT trigger on a GAMDL-only update result', () => {
    const components = [
      makeComponent({ name: 'GAMDL', update_available: true, is_compatible: true }),
    ];

    expect(findAppComponentUpdate(components)).toBeUndefined();
  });

  it('does not match the old (incorrect) "app" name string', () => {
    // Regression guard: the bug compared against the literal string 'app',
    // which the backend never emits. If someone reintroduces that literal
    // this fixture (using the real backend name) would still fail to match
    // it, proving the comparison uses APP_COMPONENT_NAME and not 'app'.
    const components = [
      makeComponent({ name: 'app', update_available: true, is_compatible: true }),
    ];

    expect(findAppComponentUpdate(components)).toBeUndefined();
  });

  it('ignores a MeedyaDL entry that is not actually available or compatible', () => {
    expect(
      findAppComponentUpdate([
        makeComponent({ name: APP_COMPONENT_NAME, update_available: false, is_compatible: true }),
      ])
    ).toBeUndefined();

    expect(
      findAppComponentUpdate([
        makeComponent({ name: APP_COMPONENT_NAME, update_available: true, is_compatible: false }),
      ])
    ).toBeUndefined();
  });
});
