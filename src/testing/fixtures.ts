// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Centralised test fixture builders (audit v2 finding #4).
 *
 * Each domain type with non-trivial defaults gets a builder function
 * that returns a fully-populated instance with sensible test defaults.
 * Tests pass `Partial<T>` overrides for the fields they actually care
 * about; everything else falls back to the defaults defined here.
 *
 * **Why this exists:**
 *
 * Pre-v2, every test file with a fixture re-defined its own builder
 * inline (`makeItem` in DownloadQueue.test.tsx, `makeEntry` in
 * ActivityLog.test.tsx, `baseManifest` in MvGapFillModal.test.tsx).
 * Each builder duplicated the type's defaults across files, so a
 * change to the type definition meant walking every test file and
 * updating the same hardcoded shape. This module collects them in
 * one place.
 *
 * **`makeFixture` generic helper:**
 *
 * For one-off types that don't warrant a named builder yet, use
 * `makeFixture(defaults)` to produce a builder with the same
 * shape. This keeps new tests from re-implementing the
 * spread-overrides pattern by hand.
 *
 * @example
 *   import { makeQueueItem } from '@/testing/fixtures';
 *   const item = makeQueueItem({ state: 'downloading', progress: 50 });
 *
 * @example
 *   import { makeFixture } from '@/testing/fixtures';
 *   const makeUser = makeFixture<User>({ id: '1', name: 'Test' });
 *   const u = makeUser({ name: 'Alice' });  // → { id: '1', name: 'Alice' }
 */

import type {
  ActivityLogEntry,
  QueueItemStatus,
} from '@/types';
import type { ScannedManifest } from '@/lib/tauri-commands';

/**
 * Generic fixture builder factory. Captures `defaults` once; returns
 * a function that merges `Partial<T>` overrides on each call.
 *
 * Use this for ad-hoc fixtures that don't yet warrant a named
 * builder export. Promotes the inline-builder pattern previously
 * scattered across test files into a one-line declaration.
 */
export function makeFixture<T extends object>(
  defaults: T
): (overrides?: Partial<T>) => T {
  return (overrides = {}) => ({ ...defaults, ...overrides });
}

/**
 * Build a `QueueItemStatus` fixture with sensible defaults.
 *
 * Default state is `queued` with a single Apple Music album URL,
 * zero progress, and no error/output/codec data. Tests typically
 * override `id` (when multiple items in one test) and `state`
 * (to exercise specific lifecycle branches).
 *
 * Fields not part of the stable `QueueItemStatus` interface that may
 * be added later (e.g. service-specific extras for M8/M9/M10) should
 * be added to this builder's defaults at the same time so existing
 * tests don't break.
 */
export const makeQueueItem = makeFixture<QueueItemStatus>({
  id: 'item-1',
  urls: ['https://music.apple.com/us/album/foo/123'],
  state: 'queued',
  progress: 0,
  current_track: null,
  album_name: null,
  artist_name: null,
  total_tracks: null,
  completed_tracks: null,
  speed: null,
  eta: null,
  processing_label: null,
  processing_progress: null,
  error: null,
  output_path: null,
  codec_used: null,
  fallback_occurred: false,
  used_wrapper: false,
} as QueueItemStatus);

/**
 * Build an `ActivityLogEntry` fixture with sensible defaults.
 *
 * Default produces a typical download stdout line tagged with a
 * UUID-shaped `download_id` (so tests of the `[shortId]` truncation
 * logic see a realistic 8-char prefix). Tests usually override
 * `_id` (for stable React keys), `download_id` (to switch between
 * `system` / per-download routing), `stream` (for the
 * `[MeedyaDL]` internal-stream badge), or `line` (to exercise the
 * search filter).
 */
export const makeActivityEntry = makeFixture<ActivityLogEntry>({
  download_id: 'abc12345-de00-4000-9000-000000000000',
  stream: 'stdout',
  line: 'Downloading track 1/12',
  timestamp: '2026-05-08T12:00:00.000Z',
});

/**
 * Build a `ScannedManifest` fixture with sensible defaults.
 *
 * Default produces a single-album manifest at a fictitious music
 * library path with the codec set to `alac` and one missing track
 * (10 tracks expected, 9 on disk). Tests typically override `album`,
 * `urls`, or `last_modified_date` to exercise the diff/freshness
 * branches in Library Scan.
 */
export const makeScannedManifest = makeFixture<ScannedManifest>({
  manifest_path: '/music/Artist/Album/manifest.meedyadl',
  album_dir: '/music/Artist/Album',
  urls: ['https://music.apple.com/us/album/foo/123'],
  platform: 'apple-music',
  artist: 'Test Artist',
  album: 'Test Album',
  downloaded_at: null,
  track_count: 10,
  current_codec: 'alac',
  audio_file_count: 9,
  last_modified_date: null,
});
