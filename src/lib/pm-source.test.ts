// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for `sourceLabel()` (package-manager abstraction,
 * Phase 2a -- see `.github/audits/package-manager-abstraction-design-2026-08-10.md`).
 *
 * Covers every known package-manager marker prefix, the two
 * non-PM marker values (`"managed"`, `"system"`), empty/nullish
 * input, and an unrecognised prefix -- all of which must degrade
 * gracefully to `"System"` rather than throwing or echoing raw
 * marker text back to the UI.
 */

import { describe, expect, it } from 'vitest';

import { sourceLabel } from './pm-source';

describe('sourceLabel', () => {
  it('maps each known package-manager prefix to its display label', () => {
    expect(sourceLabel('homebrew:ffmpeg')).toBe('Homebrew');
    expect(sourceLabel('macports:ffmpeg')).toBe('MacPorts');
    expect(sourceLabel('pipx:gamdl')).toBe('pipx');
    expect(sourceLabel('scoop:ffmpeg')).toBe('Scoop');
    expect(sourceLabel('apt:ffmpeg')).toBe('APT');
    expect(sourceLabel('dnf:ffmpeg')).toBe('DNF');
    expect(sourceLabel('snap:ffmpeg')).toBe('Snap');
  });

  it('returns "System" for the "system" marker', () => {
    expect(sourceLabel('system')).toBe('System');
  });

  it('returns "System" for the "managed" marker', () => {
    // Callers guard `source !== 'managed'` before ever calling
    // sourceLabel() in the app, but the function itself must still
    // degrade safely if called with it directly.
    expect(sourceLabel('managed')).toBe('System');
  });

  it('returns "System" for an empty string', () => {
    expect(sourceLabel('')).toBe('System');
  });

  it('returns "System" for null', () => {
    expect(sourceLabel(null)).toBe('System');
  });

  it('returns "System" for undefined', () => {
    expect(sourceLabel(undefined)).toBe('System');
  });

  it('returns "System" for an unknown package-manager prefix', () => {
    // Forward-compatible degrade: a backend that learns a new PM
    // before the frontend label map is updated should never crash
    // or leak the raw marker string into the UI.
    expect(sourceLabel('winget:ffmpeg')).toBe('System');
  });

  it('only splits on the first colon', () => {
    // Package identifiers can themselves contain colons in edge
    // cases; only the prefix before the first colon is significant.
    expect(sourceLabel('pipx:some:weird:pkg')).toBe('pipx');
  });
});
