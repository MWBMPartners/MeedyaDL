// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/** @file The test-notification advice names the right place on each system (#1431). */

import { describe, it, expect } from 'vitest';
import { whereToLookForNotifications } from './notificationAdvice';

describe('whereToLookForNotifications', () => {
  it('names macOS System Settings only on a Mac', () => {
    expect(whereToLookForNotifications('macos')).toMatch(/System Settings > Notifications > MeedyaDL/);
    expect(whereToLookForNotifications('windows')).not.toMatch(/System Settings/);
    expect(whereToLookForNotifications('linux')).not.toMatch(/System Settings/);
  });

  it('names Windows Settings on Windows', () => {
    expect(whereToLookForNotifications('windows')).toMatch(/Windows Settings app, go to System, then Notifications/);
  });

  it('points a macOS development build at Terminal', () => {
    expect(whereToLookForNotifications('macos', true)).toMatch(/Terminal/);
  });

  it('mentions the notification service on Linux', () => {
    expect(whereToLookForNotifications('linux')).toMatch(/notification service/);
  });
});
