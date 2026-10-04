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
 * `Intl` still supplies display names and their alphabetical order. It no
 * longer supplies identity -- except as a FALLBACK: until the backend
 * answers (or if it never does), both functions below use the browser's
 * reading. That is close for most tags and wrong for a few, which is
 * exactly why the backend's answer replaces it the moment it arrives.
 */

import { useEffect, useMemo, useState } from 'react';

import { languageIdentities } from '@/lib/tauri-commands';
import { primaryLanguageOf } from '@/lib/languageOptions';

interface Identity {
  standard: string;
  primary: string;
}

/** The browser's reading of a tag; a fallback only (see the file comment). */
function fallbackIdentity(tag: string): Identity {
  let standard = tag;
  try {
    // `Intl.Locale` normalises case (`EN-us` -> `en-US`) but not, for
    // instance, a retired language code.
    standard = new Intl.Locale(tag).toString();
  } catch {
    // Not readable as a tag: shown and grouped by its own text.
  }
  return { standard, primary: primaryLanguageOf(tag) ?? tag.toLowerCase() };
}

/** What `useLanguageIdentities` returns. */
export interface LanguageIdentityMaps {
  /** The tag's standard form. */
  standardOf: (tag: string) => string;
  /** The identity to group the tag by (its primary language). */
  primaryOf: (tag: string) => string;
  /** True once the backend's answer for the CURRENT tags is in. */
  ready: boolean;
}

/**
 * Backend-verified identities for `tags`, with the browser's reading while
 * the answer is pending. Asked again only when the CONTENT of `tags`
 * changes, so passing a freshly built array each render is fine.
 */
export function useLanguageIdentities(tags: readonly string[]): LanguageIdentityMaps {
  const tagsKey = JSON.stringify(tags);
  const [backend, setBackend] = useState<{ key: string; map: Map<string, Identity> } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const list = JSON.parse(tagsKey) as string[];
    // A failure before a promise exists (no IPC bridge) takes the same
    // path as a rejected call.
    const request = (): ReturnType<typeof languageIdentities> => {
      try {
        return languageIdentities(list);
      } catch (error) {
        return Promise.reject(error);
      }
    };
    request()
      .then((answers) => {
        if (cancelled) return;
        const map = new Map(
          answers.map((a) => [a.raw, { standard: a.standard, primary: a.primary }])
        );
        setBackend({ key: tagsKey, map });
      })
      .catch((error: unknown) => {
        console.warn(
          "[settings] language identities unavailable, using the browser's reading",
          error
        );
      });
    return () => {
      cancelled = true;
    };
  }, [tagsKey]);

  return useMemo(() => {
    const map = backend !== null && backend.key === tagsKey ? backend.map : null;
    const identityOf = (tag: string): Identity => map?.get(tag) ?? fallbackIdentity(tag);
    return {
      standardOf: (tag: string) => identityOf(tag).standard,
      primaryOf: (tag: string) => identityOf(tag).primary,
      ready: map !== null,
    };
  }, [backend, tagsKey]);
}
