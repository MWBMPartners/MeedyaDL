// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for crashReportRedaction.
 *
 * The behaviour worth protecting: Settings tells the user a crash report
 * never carries personal data, so a username inside a Rust error message
 * must never reach the crash-reporting service, no matter how deeply it
 * is nested inside the report.
 */

import { describe, it, expect } from 'vitest';
import {
  redactHomeDirectory,
  stripQueryStrings,
  scrubCrashReportEvent,
} from './crashReportRedaction';

describe('redactHomeDirectory', () => {
  it('replaces the username in a macOS path, keeping the rest', () => {
    expect(redactHomeDirectory('/Users/jane/Music/Artist/Album')).toBe(
      '/Users/<user>/Music/Artist/Album'
    );
  });

  it('replaces the username in a Linux path', () => {
    expect(redactHomeDirectory('/home/jane/Music/Artist')).toBe('/home/<user>/Music/Artist');
  });

  it('replaces the username in a Windows path, any drive letter', () => {
    expect(redactHomeDirectory('D:\\Users\\jane\\Music\\Album')).toBe(
      'D:\\Users\\<user>\\Music\\Album'
    );
  });

  it('redacts a path embedded inside a full sentence', () => {
    const message =
      'Filesystem error — check that the output directory is accessible and writable: /Users/jane/Music';
    expect(redactHomeDirectory(message)).toBe(
      'Filesystem error — check that the output directory is accessible and writable: /Users/<user>/Music'
    );
  });

  it('leaves text with no home directory in it untouched', () => {
    expect(redactHomeDirectory('Network error — retrying (attempt 2 of 5)')).toBe(
      'Network error — retrying (attempt 2 of 5)'
    );
  });
});

describe('stripQueryStrings', () => {
  it('drops the query string from an http(s) address', () => {
    expect(stripQueryStrings('Wrapper error at http://127.0.0.1:30020/decrypt?token=abc123')).toBe(
      'Wrapper error at http://127.0.0.1:30020/decrypt'
    );
  });

  it('leaves an address with no query string untouched', () => {
    expect(stripQueryStrings('See https://github.com/MWBMPartners/MeedyaDL')).toBe(
      'See https://github.com/MWBMPartners/MeedyaDL'
    );
  });
});

describe('scrubCrashReportEvent', () => {
  it('scrubs a string nested arbitrarily deep in the event', () => {
    const event = {
      message: 'top level, no path here',
      exception: {
        values: [
          {
            value: 'Filesystem error: /Users/jane/Music is not writable',
            stacktrace: {
              frames: [{ filename: 'C:\\Users\\jane\\AppData\\Local\\app.js', lineno: 12 }],
            },
          },
        ],
      },
      breadcrumbs: [{ message: 'Fetched https://api.example.com/status?apikey=secret' }],
    };

    const scrubbed = scrubCrashReportEvent(event);

    expect(scrubbed.exception.values[0].value).toBe(
      'Filesystem error: /Users/<user>/Music is not writable'
    );
    expect(scrubbed.exception.values[0].stacktrace.frames[0].filename).toBe(
      'C:\\Users\\<user>\\AppData\\Local\\app.js'
    );
    expect(scrubbed.breadcrumbs[0].message).toBe('Fetched https://api.example.com/status');
    // Untouched fields stay untouched.
    expect(scrubbed.message).toBe('top level, no path here');
    expect(scrubbed.exception.values[0].stacktrace.frames[0].lineno).toBe(12);
  });

  it('does not turn a Date into an empty object', () => {
    const when = new Date('2026-01-01T00:00:00Z');
    const event = { timestamp: when };
    const scrubbed = scrubCrashReportEvent(event);
    expect(scrubbed.timestamp).toBe(when);
  });

  it('leaves an array of plain strings scrubbed in place', () => {
    const event = { tags: ['/Users/jane/Music', 'unrelated'] };
    const scrubbed = scrubCrashReportEvent(event);
    expect(scrubbed.tags).toEqual(['/Users/<user>/Music', 'unrelated']);
  });
});
