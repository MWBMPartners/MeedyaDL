// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file The Wrapper Decryption Address that follows the Wrapper URL (#1433).
 *
 * On GAMDL 3.8.2 and newer the decryption address is its own setting. It is
 * filled in from the Wrapper URL's host with the standard decryption port
 * (10020) and keeps following the URL until the person types their own value.
 * "Typed their own" is worked out without a stored flag: the stored value is
 * automatic when it is empty, is the old default, or is the value derived from
 * the Wrapper URL as it was before the change.
 */

/** The standard decryption port of the wrapper. */
export const WRAPPER_DECRYPT_PORT = 10020;

/** The old built-in default; always treated as automatic. */
export const DEFAULT_WRAPPER_DECRYPT_ADDRESS = `127.0.0.1:${WRAPPER_DECRYPT_PORT}`;

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

/**
 * What to store as the decryption address after the Wrapper URL changed, or
 * `null` to leave the stored value alone. `previousDerived` is the address
 * derived from the previous (valid) Wrapper URL, or `null` if there was none.
 */
export function nextWrapperDecryptAddress(
  wrapperUrl: string | undefined,
  stored: string | undefined,
  previousDerived: string | null
): string | null {
  const derived = deriveWrapperDecryptAddress(wrapperUrl);
  if (!derived) return null;
  const current = (stored ?? '').trim();
  const automatic =
    current === '' || current === DEFAULT_WRAPPER_DECRYPT_ADDRESS || current === previousDerived || current === derived;
  if (!automatic || current === derived) return null;
  return derived;
}
