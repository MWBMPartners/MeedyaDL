// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file useLanguageIdentities -- the one place the frontend asks the
 * BACKEND which language a tag really is: its standard form and the group
 * it belongs to (Codex's catch-up review of #1244, finding 1).
 *
 * Grouping, pinning the interface language first, and showing one row
 * rather than two for differently-spelled equivalents (`"EN-us"` and
 * `"en-US"`) used to be worked out here with `Intl.Locale`. That disagrees
 * with the shared language policy for real tags: `Intl.Locale('cmn-Hans')`
 * says the language is `zh` (the policy keeps `cmn` distinct), and
 * `Intl.Locale('zh-cmn-Hans')` throws. The backend (`language_identities`,
 * `utils::language::language_identity`) reads tags with the same code the
 * real ordering uses, so the two can never disagree.
 *
 * `Intl` still supplies display names and their alphabetical order. It
 * does NOT supply identity any more.
 *
 * **Until the backend has answered for a tag, nothing is merged or removed
 * on the browser's say-so** (Codex's review of round 5, finding 2). That
 * used to happen: while the answer was pending, and for good if the one
 * request failed, `standardOf` used `Intl.Locale`, which reads a stored
 * `cmn-Hans-CN` (Mandarin) as `zh-Hans-CN` -- the offered Chinese row -- so
 * the Mandarin row was dropped as a "duplicate" and, after a failure,
 * never came back. Now, while a tag is unverified:
 *   - `standardOf(tag)` is the tag itself, unchanged, so two different
 *     spellings stay two rows (`EN-us` beside `en-US`) until the backend
 *     says they are one;
 *   - `primaryOf(tag)` is still the browser's reading, because it is used
 *     only as an ORDERING hint (which group to sort an entry with); it
 *     never decides whether an entry is shown.
 * A failed request is tried again a few times, each time after a longer
 * wait (`IDENTITY_RETRY_DELAYS_MS`); after the last failure the raw
 * readings simply stay. A success at any point replaces them. The one
 * exception is a call the backend refused as too large
 * (`TOO_LARGE_TO_IDENTIFY`): the same call would be refused the same way,
 * so it is not tried again and the raw readings stay at once (stand-in
 * review of round 6, finding 4).
 */

import { useEffect, useMemo, useState } from 'react';

import { languageIdentities } from '@/lib/tauri-commands';
import { primaryLanguageOf } from '@/lib/languageOptions';

interface Identity {
  standard: string;
  primary: string;
}

/**
 * How long to wait before each further try after a failed request, in
 * milliseconds: three more tries, each after a longer wait, then no more.
 * Bounded on purpose -- the request only fails when the backend is not
 * there at all (or the call is too large, which is not retried at all;
 * see `TOO_LARGE_TO_IDENTIFY`), and asking for ever would not change
 * that. Exported for the tests.
 */
export const IDENTITY_RETRY_DELAYS_MS: readonly number[] = [500, 1000, 2000];

/**
 * How the backend's refusal of a call that is too large begins
 * (`TOO_LARGE_TO_IDENTIFY` in `src-tauri/src/commands/language.rs`, whose
 * test checks that this file holds the same text). It is the one failure
 * certain to come back the same on every try, so it is never retried.
 */
export const TOO_LARGE_TO_IDENTIFY = 'Too large to identify:';

/** True when `error` is the backend refusing the call as too large. */
function isRefusedAsTooLarge(error: unknown): boolean {
  const text = typeof error === 'string' ? error : error instanceof Error ? error.message : '';
  return text.startsWith(TOO_LARGE_TO_IDENTIFY);
}

/**
 * A tag's identity while the backend has not vouched for it: its standard
 * form is the tag itself (so nothing is merged with anything else), and its
 * group is the browser's reading, used only to sort (see the file comment).
 */
function unverifiedIdentity(tag: string): Identity {
  return { standard: tag, primary: primaryLanguageOf(tag) ?? tag.toLowerCase() };
}

/** What `useLanguageIdentities` returns. */
export interface LanguageIdentityMaps {
  /**
   * The tag's standard form, as the backend gives it -- or the tag itself,
   * unchanged, while the backend has not answered for it. Safe to compare
   * for "is this the same language?": an unverified tag only ever equals
   * the exact same text.
   */
  standardOf: (tag: string) => string;
  /**
   * The identity to group the tag by (its primary language). The browser's
   * reading while unverified: an ordering hint, never a reason to drop or
   * merge an entry.
   */
  primaryOf: (tag: string) => string;
  /** True once the backend's answer for the CURRENT tags is in. */
  ready: boolean;
}

/**
 * Backend-verified identities for `tags` (see the file comment for what is
 * used while the answer is pending or after it failed). Asked again only
 * when the CONTENT of `tags` changes, so passing a freshly built array each
 * render is fine. An answer, or a retry, for a list that has since changed
 * is dropped.
 */
export function useLanguageIdentities(tags: readonly string[]): LanguageIdentityMaps {
  const tagsKey = JSON.stringify(tags);
  const [backend, setBackend] = useState<{ key: string; map: Map<string, Identity> } | null>(null);

  useEffect(() => {
    let cancelled = false;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;
    const list = JSON.parse(tagsKey) as string[];

    const attempt = (failuresSoFar: number): void => {
      // `Promise.resolve().then(...)`: a failure before a promise exists
      // (no IPC bridge) takes the same path as a rejected call.
      Promise.resolve()
        .then(() => languageIdentities(list))
        .then((answers) => {
          if (cancelled) return;
          if (!Array.isArray(answers)) throw new Error('unexpected answer');
          const map = new Map(
            answers.map((a) => [a.raw, { standard: a.standard, primary: a.primary }])
          );
          setBackend({ key: tagsKey, map });
        })
        .catch((error: unknown) => {
          if (cancelled) return;
          if (isRefusedAsTooLarge(error)) {
            console.warn(
              '[settings] language identities refused as too large, not retried; ' +
                'every entry keeps its own row',
              error
            );
            return;
          }
          const delay = IDENTITY_RETRY_DELAYS_MS[failuresSoFar];
          if (delay === undefined) {
            console.warn(
              '[settings] language identities unavailable after retrying; ' +
                'every entry keeps its own row',
              error
            );
            return;
          }
          console.warn(
            `[settings] language identities unavailable, trying again in ${delay} ms`,
            error
          );
          retryTimer = setTimeout(() => attempt(failuresSoFar + 1), delay);
        });
    };
    attempt(0);

    return () => {
      cancelled = true;
      if (retryTimer !== undefined) clearTimeout(retryTimer);
    };
  }, [tagsKey]);

  return useMemo(() => {
    const map = backend !== null && backend.key === tagsKey ? backend.map : null;
    const identityOf = (tag: string): Identity => map?.get(tag) ?? unverifiedIdentity(tag);
    return {
      standardOf: (tag: string) => identityOf(tag).standard,
      primaryOf: (tag: string) => identityOf(tag).primary,
      ready: map !== null,
    };
  }, [backend, tagsKey]);
}
