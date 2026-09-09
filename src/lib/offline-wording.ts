// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file What to tell someone whose download was queued with no connection.
 *
 * The app used to say the same thing to everybody:
 *
 *     "Download queued — will start when internet is available"
 *
 * That was untrue twice over. Nothing watched for the connection returning, so
 * it never started by itself; and for anyone who has switched automatic
 * starting off, it would not have started even if something had been watching.
 *
 * Something now does watch (#1156), so the sentence is true for most people.
 * It is still not true for the ones who turned automatic starting off, and
 * those are exactly the people who need telling what to do instead.
 */

/**
 * The toast key used for every "queued while offline" message.
 *
 * The same key the pre-download checks use for their own internet warning, so
 * the two replace one another rather than stacking, and so the message can be
 * taken away when the connection returns.
 */
export const OFFLINE_TOAST_KEY = 'preflight:internet';

/**
 * Builds the message shown when downloads are queued with no connection.
 *
 * @param summary -- What was queued, already worded by the caller. For a
 *        single download this is a plain sentence; for several it is the
 *        counts. Passed in rather than rebuilt so both paths read the same.
 * @param willStartOnItsOwn -- Whether the app will start these downloads when
 *        the connection returns. False when the user has switched automatic
 *        starting off.
 * @returns The full message.
 */
export function offlineQueuedMessage(summary: string, willStartOnItsOwn: boolean): string {
  if (willStartOnItsOwn) {
    return `${summary} — no internet right now. They will start automatically when the connection comes back.`;
  }
  // Nothing is going to happen on its own for this person, so say what they
  // need to do. Telling them it will start by itself would be the same untruth
  // in a new place.
  return `${summary} — no internet right now. Starting downloads automatically is switched off, so press Start Queue on the Queue page once you are back online.`;
}
