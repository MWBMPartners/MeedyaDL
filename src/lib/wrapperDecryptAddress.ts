// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file The Wrapper Decryption Address shown as automatic (#1433).
 *
 * An empty stored value means AUTOMATIC: the app works the address out when
 * it needs it, from the Wrapper URL's host and the standard port 10020. This
 * helper only works out the same text for the placeholder; the real
 * calculation is `utils::wrapper_address` in the Rust side, and both are
 * tested against the same cases.
 */

/** The standard decryption port of the wrapper. */
export const WRAPPER_DECRYPT_PORT = 10020;

/**
 * The decryption address for a Wrapper URL: its host plus port 10020, with
 * the URL's own port ignored. An IPv6 host keeps its brackets. Returns
 * `null` for anything that is not a usable URL.
 */
export function deriveWrapperDecryptAddress(wrapperUrl: string | undefined): string | null {
  const text = (wrapperUrl ?? '').trim();
  if (!text) return null;
  const withScheme = /^[A-Za-z][A-Za-z0-9+.-]*:\/\//.test(text) ? text : `http://${text}`;
  try {
    const url = new URL(withScheme);
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return null;
    if (!url.hostname) return null;
    return `${url.hostname}:${WRAPPER_DECRYPT_PORT}`;
  } catch {
    return null;
  }
}
