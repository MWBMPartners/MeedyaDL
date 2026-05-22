// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Unit tests for `computeItemProgress` (#790).
 *
 * Validates each resolution path. Per the user's clarification
 * (2026-05-17), the bar prefers per-file byte progress (cycles
 * 0 → 100 per file, label changes per file) over album-level
 * aggregation, and only falls back to the indeterminate
 * animation when no signal at all is available.
 */

import { describe, expect, it } from 'vitest';

import { makeQueueItem } from '@/testing/fixtures';
import { computeItemProgress } from './progress-percent';

describe('computeItemProgress', () => {
  describe('Path 1: active GAMDL byte stream', () => {
    it('returns the raw per-file percentage when speed is set', () => {
      // Per the user clarification: per-file accuracy beats
      // album-level aggregation. The bar resetting between files
      // is correct — the caption changes per file too.
      const item = makeQueueItem({
        state: 'downloading',
        speed: '2.5 MB/s',
        progress: 42,
      });
      expect(computeItemProgress(item)).toBe(42);
    });

    it('uses raw progress during processing state too (companion download phase)', () => {
      const item = makeQueueItem({
        state: 'processing',
        speed: '1.2 MB/s',
        progress: 75,
      });
      expect(computeItemProgress(item)).toBe(75);
    });

    it('ignores total_tracks / completed_tracks — those drive the queue-level bar, not the per-item bar', () => {
      // Confirms the previous album-aggregation behaviour was
      // reverted. The bar shows the current file's byte
      // progress; album-level rollup lives on the queue-level
      // (second) bar.
      const item = makeQueueItem({
        state: 'downloading',
        speed: '2.5 MB/s',
        total_tracks: 12,
        completed_tracks: 4,
        progress: 60,
      });
      expect(computeItemProgress(item)).toBe(60);
    });
  });

  describe('Path 2: enrichment stage weight', () => {
    it('scales the 0-1 fraction to 0-100', () => {
      const item = makeQueueItem({
        state: 'processing',
        speed: null,
        processing_progress: 0.75,
      });
      expect(computeItemProgress(item)).toBe(75);
    });

    it('handles early enrichment stage (0.05)', () => {
      const item = makeQueueItem({
        state: 'processing',
        speed: null,
        processing_progress: 0.05,
      });
      expect(computeItemProgress(item)).toBe(5);
    });

    it('handles the final enrichment stage (1.0)', () => {
      const item = makeQueueItem({
        state: 'processing',
        speed: null,
        processing_progress: 1.0,
      });
      expect(computeItemProgress(item)).toBe(100);
    });
  });

  describe('Path 3: non-active states', () => {
    it('queued items return their raw progress (typically 0)', () => {
      const item = makeQueueItem({
        state: 'queued',
        speed: null,
        progress: 0,
      });
      expect(computeItemProgress(item)).toBe(0);
    });

    it('complete items return their raw progress (typically 100)', () => {
      const item = makeQueueItem({
        state: 'complete',
        speed: null,
        progress: 100,
      });
      expect(computeItemProgress(item)).toBe(100);
    });

    it('error items return their raw progress (last known value)', () => {
      const item = makeQueueItem({
        state: 'error',
        speed: null,
        progress: 35,
      });
      expect(computeItemProgress(item)).toBe(35);
    });
  });

  describe('Path 4: indeterminate fallback (LAST resort)', () => {
    it('returns null when processing without speed AND without processing_progress', () => {
      // Only path that produces null. Should be rare — fires in
      // the brief gaps between enrichment stages where no signal
      // has fired yet.
      const item = makeQueueItem({
        state: 'processing',
        speed: null,
        processing_progress: null,
      });
      expect(computeItemProgress(item)).toBeNull();
    });

    it('returns null for downloading state without speed AND no fallback signal', () => {
      // Edge case: state says downloading but speed is unset
      // (sub-second window between subprocess spawn and first
      // [download] X% line). Honest answer: we don't know yet.
      const item = makeQueueItem({
        state: 'downloading',
        speed: null,
        processing_progress: null,
      });
      expect(computeItemProgress(item)).toBeNull();
    });
  });

  describe('Defensive behaviour', () => {
    it('returns null for null input', () => {
      expect(computeItemProgress(null)).toBeNull();
    });

    it('returns null for undefined input', () => {
      expect(computeItemProgress(undefined)).toBeNull();
    });

    it('clamps an out-of-range backend value to the 0-100 envelope', () => {
      const item = makeQueueItem({
        state: 'downloading',
        speed: '1 MB/s',
        progress: 250,
      });
      expect(computeItemProgress(item)).toBe(100);
    });

    it('clamps a negative value to 0', () => {
      const item = makeQueueItem({
        state: 'downloading',
        speed: '1 MB/s',
        progress: -10,
      });
      expect(computeItemProgress(item)).toBe(0);
    });

    it('handles NaN gracefully by returning 0', () => {
      const item = makeQueueItem({
        state: 'downloading',
        speed: '1 MB/s',
        progress: NaN,
      });
      expect(computeItemProgress(item)).toBe(0);
    });

    it('clamps processing_progress > 1 to 100', () => {
      const item = makeQueueItem({
        state: 'processing',
        speed: null,
        processing_progress: 1.5,
      });
      expect(computeItemProgress(item)).toBe(100);
    });
  });
});
