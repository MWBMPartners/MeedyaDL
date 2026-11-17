// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE for details.

/**
 * Works out which wrapper address the Settings > Advanced > Wrapper
 * "Test Connection" button should try.
 *
 * The newer wrapper (used by GAMDL 3.6 and later) is reached through the
 * single "Wrapper URL" field. The older wrapper (GAMDL 3.5 and earlier)
 * is reached through the "Wrapper Account URL" field. Only one of the two
 * fields is shown at a time, so the button must test the one that is
 * shown, and be usable exactly when that one has an address in it.
 *
 * Kept as a small plain function so the rule can be tested on its own.
 *
 * @param wrapperV2 - True when the installed GAMDL uses the newer wrapper.
 * @param accountUrl - The saved "Wrapper Account URL" (older wrapper).
 * @param wrapperUrl - The saved "Wrapper URL" (newer wrapper).
 * @returns The address to test, with surrounding spaces removed. An empty
 *   string means there is nothing to test yet.
 */
export function wrapperTestAddress(
  wrapperV2: boolean,
  accountUrl: string | undefined,
  wrapperUrl: string | undefined
): string {
  return ((wrapperV2 ? wrapperUrl : accountUrl) ?? '').trim();
}
