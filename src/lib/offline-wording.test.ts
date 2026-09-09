// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for the offline queueing message.
 *
 * The old message told everyone the download would start when the internet
 * came back. That was untrue twice: nothing watched for the connection, and
 * for anyone with automatic starting switched off it would not have started
 * even if something had. These tests guard against either untruth returning.
 */

import { describe, it, expect } from 'vitest';
import { offlineQueuedMessage, OFFLINE_TOAST_KEY } from './offline-wording';

describe('offlineQueuedMessage', () => {
  it('promises an automatic start only when one will actually happen', () => {
    const message = offlineQueuedMessage('Download queued', true);
    expect(message).toContain('start automatically');
    expect(message).toContain('no internet');
  });

  it('never promises an automatic start when automatic starting is off', () => {
    // The whole point. This person will wait for ever if we tell them it
    // starts by itself.
    const message = offlineQueuedMessage('Download queued', false);
    expect(message).not.toContain('start automatically');
    expect(message).toContain('Start Queue');
  });

  it('tells the user what to do instead when nothing will happen on its own', () => {
    const message = offlineQueuedMessage('Download queued', false);
    expect(message).toContain('switched off');
    expect(message).toContain('Queue page');
  });

  it('keeps whatever summary the caller built, for one download or many', () => {
    expect(offlineQueuedMessage('Download queued', true)).toContain('Download queued');
    expect(offlineQueuedMessage('3 downloads added to queue', true)).toContain(
      '3 downloads added to queue'
    );
  });

  it('uses the same key as the pre-download internet warning', () => {
    // So the two replace one another rather than stacking, and so the
    // connection watcher can take the message away when it fires.
    expect(OFFLINE_TOAST_KEY).toBe('preflight:internet');
  });

  it('says nothing about internet being "available", the old wording', () => {
    // The old sentence read "will start when internet is available", which
    // read as a promise the app could not keep.
    for (const autoStart of [true, false]) {
      expect(offlineQueuedMessage('Download queued', autoStart)).not.toContain(
        'internet is available'
      );
    }
  });
});
