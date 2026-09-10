// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file crashReportRedaction — removes a person's folder path from text
 * before it can be sent to the crash-reporting service.
 *
 * The Settings page tells the user "No personal data, download history,
 * or account information is ever included" in a crash report. That was
 * not quite true. When a call into the Rust backend fails, the error
 * text it carries is the exact wording Rust produced — and some of
 * those messages are written to include the folder that went wrong, on
 * purpose, so a person reading the activity log knows what to check
 * (see, for example, the "Filesystem error — check that the output
 * directory is accessible and writable: {error}" message). A folder
 * path under a person's home directory always contains their username
 * (`/Users/jane/Music/...`, `C:\Users\jane\Music\...`), and if that
 * error ever escapes as an unhandled promise rejection, the crash
 * reporting library — once the user has switched it on — picks it up
 * and sends it exactly as written, username included.
 *
 * These functions strip that out before a report leaves the machine.
 * They work on plain text, and `scrubCrashReportEvent` applies them to
 * every string anywhere inside a crash report, however deeply nested,
 * so nobody has to keep a list of which fields might one day carry a
 * path.
 */

/**
 * Replaces the username segment of a home-directory path with a fixed
 * placeholder, on all three desktop platforms this app ships on.
 *
 * Only the username is replaced — `/Users/jane/Music/Album` becomes
 * `/Users/<user>/Music/Album`, so the rest of the path (still useful for
 * understanding what went wrong) is kept.
 */
export function redactHomeDirectory(text: string): string {
  return text
    .replace(/\/Users\/[^/\s]+/g, '/Users/<user>')
    .replace(/\/home\/[^/\s]+/g, '/home/<user>')
    .replace(/([A-Za-z]:\\Users\\)[^\\\s]+/g, '$1<user>');
}

/**
 * Removes everything from the first `?` onward on any `http(s)://`
 * address found in the text. A query string can carry a token or other
 * one-time value that identifies a specific request or account — the
 * rest of the address (which service, which endpoint) is the useful,
 * shareable part.
 */
export function stripQueryStrings(text: string): string {
  return text.replace(/(https?:\/\/[^\s"']+)\?[^\s"']*/g, '$1');
}

/** Applies both redactions above to one string. */
function scrubString(text: string): string {
  return stripQueryStrings(redactHomeDirectory(text));
}

/**
 * True for a plain `{ ... }` object — the shape every field of a crash
 * report actually is. Deliberately false for anything with its own
 * class (a `Date`, a `Map`, a DOM node, and so on): walking into one of
 * those with `Object.entries` would silently turn it into `{}`, which
 * is worse than leaving it alone.
 */
function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null) return false;
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

/**
 * Walks any value — a string, a plain object, an array, or arbitrarily
 * nested combinations of those — and returns an equivalent value with
 * every string passed through `scrubString`. Anything else (numbers,
 * booleans, `null`, `undefined`, or an object that isn't a plain `{}`)
 * is returned unchanged.
 */
function deepScrub<T>(value: T): T {
  if (typeof value === 'string') {
    return scrubString(value) as unknown as T;
  }
  if (Array.isArray(value)) {
    return value.map((item) => deepScrub(item)) as unknown as T;
  }
  if (isPlainObject(value)) {
    const result: Record<string, unknown> = {};
    for (const [key, val] of Object.entries(value)) {
      result[key] = deepScrub(val);
    }
    return result as T;
  }
  return value;
}

/**
 * Sentry's `beforeSend` hook: scrubs a crash-report event before it
 * leaves the machine, wherever in its (deeply nested, ever-growing)
 * shape a personal path or a query string happens to be — the message,
 * an exception value, a stack frame's file name, a breadcrumb, and so
 * on. Returning the event (rather than `null`) means the report still
 * gets sent -- only the two things named above are ever changed.
 */
export function scrubCrashReportEvent<T>(event: T): T {
  return deepScrub(event);
}
