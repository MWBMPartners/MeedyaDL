// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for whether MeedyaDL considers itself ready to download.
 *
 * The readiness light used to check Python and GAMDL only — two of the seven
 * things a download actually needs. Remove FFmpeg with a package manager and
 * the light stayed green, and the first sign of trouble was a download failing
 * with a message that did not obviously mean "a tool is missing" (#1156).
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { useDependencyStore } from './dependencyStore';
import type { DependencyStatus } from '@/types';

function status(name: string, installed: boolean, required = true): DependencyStatus {
  return {
    name,
    required,
    installed,
    version: installed ? '1.0.0' : null,
    path: null,
  } as unknown as DependencyStatus;
}

describe('whether the app is ready to download', () => {
  beforeEach(() => {
    useDependencyStore.setState({ python: null, gamdl: null, tools: [] });
  });

  it('is not ready before anything has been checked', () => {
    expect(useDependencyStore.getState().isReady()).toBe(false);
  });

  it('is ready when Python, GAMDL and every required tool are installed', () => {
    useDependencyStore.setState({
      python: status('Python', true),
      gamdl: status('GAMDL', true),
      tools: [status('FFmpeg', true), status('MediaInfo', true)],
    });
    expect(useDependencyStore.getState().isReady()).toBe(true);
  });

  it('is NOT ready when a required tool has gone missing', () => {
    // The bug this fixes: a package manager removes FFmpeg, and the light
    // stayed green because only Python and GAMDL were being checked.
    useDependencyStore.setState({
      python: status('Python', true),
      gamdl: status('GAMDL', true),
      tools: [status('FFmpeg', false), status('MediaInfo', true)],
    });
    expect(useDependencyStore.getState().isReady()).toBe(false);
  });

  it('stays ready when only an optional tool is absent', () => {
    // rclone is needed solely for uploading to cloud storage. Its absence
    // must not make the whole app look broken.
    useDependencyStore.setState({
      python: status('Python', true),
      gamdl: status('GAMDL', true),
      tools: [status('FFmpeg', true), status('rclone', false, false)],
    });
    expect(useDependencyStore.getState().isReady()).toBe(true);
  });

  it('is not ready without Python or GAMDL, whatever the tools say', () => {
    useDependencyStore.setState({
      python: status('Python', false),
      gamdl: status('GAMDL', true),
      tools: [status('FFmpeg', true)],
    });
    expect(useDependencyStore.getState().isReady()).toBe(false);
  });
});

describe('naming what is missing', () => {
  beforeEach(() => {
    useDependencyStore.setState({ python: null, gamdl: null, tools: [] });
  });

  it('names nothing when everything needed is present', () => {
    useDependencyStore.setState({
      python: status('Python', true),
      gamdl: status('GAMDL', true),
      tools: [status('FFmpeg', true)],
    });
    expect(useDependencyStore.getState().missingRequirements()).toEqual([]);
  });

  it('names the one thing missing, so the message can be an instruction', () => {
    useDependencyStore.setState({
      python: status('Python', true),
      gamdl: status('GAMDL', true),
      tools: [status('FFmpeg', false), status('MediaInfo', true)],
    });
    expect(useDependencyStore.getState().missingRequirements()).toEqual(['FFmpeg']);
  });

  it('names all of them on a first run', () => {
    useDependencyStore.setState({
      python: status('Python', false),
      gamdl: status('GAMDL', false),
      tools: [status('FFmpeg', false)],
    });
    expect(useDependencyStore.getState().missingRequirements()).toEqual([
      'Python',
      'GAMDL',
      'FFmpeg',
    ]);
  });

  it('never names an optional tool', () => {
    useDependencyStore.setState({
      python: status('Python', true),
      gamdl: status('GAMDL', true),
      tools: [status('rclone', false, false)],
    });
    expect(useDependencyStore.getState().missingRequirements()).toEqual([]);
  });
});
