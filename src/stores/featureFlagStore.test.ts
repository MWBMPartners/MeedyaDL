// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for `selectNoticeEntries` (featureFlagStore.ts).
 *
 * Pins the selector contract: it picks up exactly the verdicts that are
 * worth surfacing a notice for -- disabled features (with or without a
 * notice attached) and enabled features that still carry a notice -- and
 * nothing else. Also pins the state-separation invariant: the selector
 * must never look at `snapshot.meta`.
 */

import { selectNoticeEntries, selectServiceEnabled } from '@/stores/featureFlagStore';
import { makeFeatureFlagsSnapshot, makeFlagVerdict } from '@/testing/fixtures';

describe('selectNoticeEntries', () => {
  it('picks up a disabled verdict with no notice', () => {
    const snapshot = makeFeatureFlagsSnapshot({
      verdicts: {
        'service.bbc-iplayer': makeFlagVerdict({ enabled: false, notice: null }),
      },
    });

    const entries = selectNoticeEntries(snapshot);

    expect(entries).toHaveLength(1);
    expect(entries[0].key).toBe('service.bbc-iplayer');
    expect(entries[0].verdict.enabled).toBe(false);
  });

  it('picks up an enabled verdict that carries a notice', () => {
    const snapshot = makeFeatureFlagsSnapshot({
      verdicts: {
        'core.updater': makeFlagVerdict({
          enabled: true,
          notice: { message: 'Heads up: brief maintenance window tonight.', severity: 'maintenance', url: null },
        }),
      },
    });

    const entries = selectNoticeEntries(snapshot);

    expect(entries).toHaveLength(1);
    expect(entries[0].key).toBe('core.updater');
    expect(entries[0].verdict.enabled).toBe(true);
    expect(entries[0].verdict.notice?.message).toBe('Heads up: brief maintenance window tonight.');
  });

  it('does NOT pick up an enabled verdict with no notice', () => {
    const snapshot = makeFeatureFlagsSnapshot({
      verdicts: {
        'core.updater': makeFlagVerdict({ enabled: true, notice: null }),
      },
    });

    expect(selectNoticeEntries(snapshot)).toHaveLength(0);
  });

  it('returns multiple entries when several flags qualify', () => {
    const snapshot = makeFeatureFlagsSnapshot({
      verdicts: {
        'service.spotify': makeFlagVerdict({ enabled: false }),
        'core.updater': makeFlagVerdict({
          enabled: true,
          notice: { message: 'Degraded performance.', severity: 'info', url: null },
        }),
        'service.youtube': makeFlagVerdict({ enabled: true, notice: null }),
      },
    });

    const entries = selectNoticeEntries(snapshot);
    const keys = entries.map((e) => e.key).sort();

    expect(keys).toEqual(['core.updater', 'service.spotify']);
  });

  it('is driven purely by verdicts -- a failure-laden meta with no disabled verdicts yields nothing', () => {
    const snapshot = makeFeatureFlagsSnapshot({
      verdicts: {
        'core.updater': makeFlagVerdict({ enabled: true, notice: null }),
      },
      meta: {
        source: 'disk_cache',
        fetched_at: null,
        consecutive_failures: 7,
        last_error: 'connection refused',
      },
    });

    expect(selectNoticeEntries(snapshot)).toHaveLength(0);
  });
});

/**
 * `selectServiceEnabled` is the courtesy half of the enforcement layer —
 * the Rust `feature_flag_service::service_gate` is authoritative. These
 * tests pin the fail-open rule (a key the snapshot has never heard of is
 * enabled) and the same never-read-`meta` invariant as above.
 */
describe('selectServiceEnabled', () => {
  it('returns true for a key the snapshot does not mention (fail-open)', () => {
    const snapshot = makeFeatureFlagsSnapshot({ verdicts: {} });
    expect(selectServiceEnabled(snapshot, 'service-apple-music')).toBe(true);
  });

  it('returns true for an explicitly enabled service', () => {
    const snapshot = makeFeatureFlagsSnapshot({
      verdicts: { 'service-apple-music': makeFlagVerdict({ enabled: true }) },
    });
    expect(selectServiceEnabled(snapshot, 'service-apple-music')).toBe(true);
  });

  it('returns false for an explicitly disabled service', () => {
    const snapshot = makeFeatureFlagsSnapshot({
      verdicts: { 'service-spotify': makeFlagVerdict({ enabled: false }) },
    });
    expect(selectServiceEnabled(snapshot, 'service-spotify')).toBe(false);
  });

  it('does not let one disabled service affect another', () => {
    const snapshot = makeFeatureFlagsSnapshot({
      verdicts: {
        'service-spotify': makeFlagVerdict({ enabled: false }),
        'service-apple-music': makeFlagVerdict({ enabled: true }),
      },
    });
    expect(selectServiceEnabled(snapshot, 'service-spotify')).toBe(false);
    expect(selectServiceEnabled(snapshot, 'service-apple-music')).toBe(true);
  });

  it('ignores meta entirely -- a failure-laden snapshot still reads as enabled', () => {
    const snapshot = makeFeatureFlagsSnapshot({
      verdicts: {},
      meta: {
        source: 'disk_cache',
        fetched_at: null,
        consecutive_failures: 12,
        last_error: 'connection refused',
      },
    });
    expect(selectServiceEnabled(snapshot, 'service-apple-music')).toBe(true);
  });
});
