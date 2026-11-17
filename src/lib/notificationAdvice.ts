// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file notificationAdvice.ts -- where to look when a system notification
 * does not appear, in this computer's own words (#1431).
 *
 * Before #1431 the test button named macOS's System Settings on every
 * system. Shared by Settings > General > Notifications (the test button)
 * and Settings > Advanced > Diagnostics.
 */

import type { Platform } from '@/hooks/usePlatform';

/**
 * One plain sentence (or two) saying where to look. `null` = the platform
 * is not known yet, so the advice names no particular system.
 *
 * @param developmentBuild -- on macOS a development build's notifications
 *   appear as Terminal's, so they are governed by Terminal's settings.
 */
export function whereToLookForNotifications(
  platform: Platform | null,
  developmentBuild = false
): string {
  // Until the platform is known (it is detected a moment after start-up),
  // say nothing that names one system's settings: it could be the wrong one.
  if (platform === null) {
    return "If nothing appeared, check that notifications are allowed for MeedyaDL in your computer's notification settings, and that do-not-disturb is off.";
  }
  switch (platform) {
    case 'macos':
      return developmentBuild
        ? 'This is a development build, so macOS shows its notifications as Terminal: check System Settings > Notifications > Terminal, and that Focus is off.'
        : 'If nothing appeared, open System Settings > Notifications > MeedyaDL and allow notifications, and check that Focus is off.';
    case 'windows':
      return 'If nothing appeared, open the Windows Settings app, go to System, then Notifications, make sure notifications are on for MeedyaDL, and check that Do not disturb is off.';
    default:
      return "If nothing appeared, check your desktop's notification settings, and that a notification service is running (most desktops start one for you).";
  }
}
