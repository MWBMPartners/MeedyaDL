// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for the fixture builder helpers (audit v2 #4).
 *
 * Pins the contract for `makeFixture` + the three named builders so
 * a future change to a domain type (QueueItemStatus, ActivityLogEntry,
 * ScannedManifest) that breaks the default shape is caught here
 * before it cascades into the consumer test files.
 */

import {
  makeActivityEntry,
  makeFixture,
  makeQueueItem,
  makeScannedManifest,
} from '@/testing/fixtures';

describe('makeFixture', () => {
  it('returns the defaults when no overrides are supplied', () => {
    const make = makeFixture({ a: 1, b: 'two' });
    expect(make()).toEqual({ a: 1, b: 'two' });
  });

  it('shallow-merges Partial overrides on top of defaults', () => {
    const make = makeFixture({ a: 1, b: 'two', c: false });
    expect(make({ b: 'three' })).toEqual({ a: 1, b: 'three', c: false });
  });

  it('does not share state between calls (each call returns a fresh object)', () => {
    const make = makeFixture({ count: 0, items: [] as number[] });
    const first = make();
    const second = make();
    expect(first).not.toBe(second);
    // Defaults reused by reference for nested values — that's by
    // design (shallow spread); tests that mutate nested state should
    // override the field with a fresh reference.
  });
});

describe('makeQueueItem', () => {
  it('produces a queued item with default URL when called bare', () => {
    const item = makeQueueItem();
    expect(item.id).toBe('item-1');
    expect(item.state).toBe('queued');
    expect(item.urls).toEqual(['https://music.apple.com/us/album/foo/123']);
    expect(item.progress).toBe(0);
    expect(item.fallback_occurred).toBe(false);
    expect(item.used_wrapper).toBe(false);
  });

  it('overrides only the fields supplied', () => {
    const item = makeQueueItem({ id: 'x', state: 'downloading', progress: 47 });
    expect(item.id).toBe('x');
    expect(item.state).toBe('downloading');
    expect(item.progress).toBe(47);
    // Untouched defaults preserved
    expect(item.urls).toEqual(['https://music.apple.com/us/album/foo/123']);
    expect(item.error).toBeNull();
  });
});

describe('makeActivityEntry', () => {
  it('produces a stdout download entry with UUID-shaped download_id', () => {
    const entry = makeActivityEntry();
    expect(entry.stream).toBe('stdout');
    expect(entry.download_id).toMatch(/^[a-f0-9]{8}-/); // realistic UUID prefix
    expect(entry.line).toBe('Downloading track 1/12');
  });

  it('overrides only the fields supplied', () => {
    const entry = makeActivityEntry({
      _id: 42,
      download_id: 'system',
      line: 'App started',
    });
    expect(entry._id).toBe(42);
    expect(entry.download_id).toBe('system');
    expect(entry.line).toBe('App started');
    expect(entry.stream).toBe('stdout'); // default preserved
  });
});

describe('makeScannedManifest', () => {
  it('produces a single-album manifest with one missing track', () => {
    const m = makeScannedManifest();
    expect(m.album).toBe('Test Album');
    expect(m.artist).toBe('Test Artist');
    expect(m.track_count).toBe(10);
    expect(m.audio_file_count).toBe(9);
    expect(m.current_codec).toBe('alac');
    expect(m.last_modified_date).toBeNull();
  });

  it('overrides only the fields supplied', () => {
    const m = makeScannedManifest({ album: 'Other', last_modified_date: '2026-01-01' });
    expect(m.album).toBe('Other');
    expect(m.last_modified_date).toBe('2026-01-01');
    expect(m.artist).toBe('Test Artist'); // default preserved
  });
});
