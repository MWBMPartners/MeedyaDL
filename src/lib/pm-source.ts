// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file pm-source.ts -- Package-manager provenance label helper.
 *
 * The Rust backend records how a detected dependency (FFmpeg, MP4Box,
 * etc.) was located on disk via a per-tool `.source` marker file. Since
 * the "generalised package-manager abstraction" increment (see
 * `.github/audits/package-manager-abstraction-design-2026-08-10.md`
 * §3.B / §4.2), the marker's value follows one of three shapes:
 *
 *   - `"managed"`        -- MeedyaDL downloaded and manages the binary
 *                            itself (not a "found on your system" case;
 *                            callers guard this value out before ever
 *                            reaching `sourceLabel()` -- see
 *                            `DependenciesStep.tsx`).
 *   - `"system"`          -- found on `$PATH` but its owning package
 *                            manager (if any) could not be identified.
 *   - `"<pm>:<pkg>"`       -- found and attributed to a specific package
 *                            manager, e.g. `"homebrew:ffmpeg"` or
 *                            `"pipx:gamdl"`. `<pm>` is one of the seven
 *                            known prefixes in `PM_LABELS` below.
 *
 * `sourceLabel()` maps any of these (plus `null`/`undefined`/empty/
 * unrecognised input) to the short badge text the UI displays next to
 * a detected tool. Unknown or malformed values degrade gracefully to
 * `"System"` rather than throwing or displaying raw marker text --
 * forward-compatible with any future package manager the backend learns
 * to recognise before the frontend badge map is updated to match.
 */

/**
 * Maps a package-manager marker prefix (the text before the first `:`
 * in a `<pm>:<pkg>` marker) to its human-readable display label.
 *
 * Keys are lowercase and match the Rust `PackageManagerKind` marker
 * prefixes exactly (`package_manager.rs`). Values are the short strings
 * shown in the setup wizard's dependency badge and the Updates page.
 */
const PM_LABELS: Readonly<Record<string, string>> = {
  homebrew: 'Homebrew',
  macports: 'MacPorts',
  pipx: 'pipx',
  scoop: 'Scoop',
  apt: 'APT',
  dnf: 'DNF',
  snap: 'Snap',
};

/**
 * Resolves a `.source` marker value to the display label the UI shows
 * for a detected (non-managed) dependency.
 *
 * @param source - The raw `.source` marker value read for a tool, or
 *   `null`/`undefined` if no marker exists yet. Expected shapes are
 *   `"managed"`, `"system"`, or `"<pm>:<pkg>"` (see file header), but
 *   any other string is treated as unrecognised and degrades to
 *   `"System"` rather than being echoed back verbatim.
 * @returns One of the seven known package-manager labels (`"Homebrew"`,
 *   `"MacPorts"`, `"pipx"`, `"Scoop"`, `"APT"`, `"DNF"`, `"Snap"`) when
 *   `source` begins with a recognised `<pm>:` prefix, otherwise
 *   `"System"`.
 */
export function sourceLabel(source: string | null | undefined): string {
  if (!source) return 'System';

  // Split on the FIRST colon only -- package names can themselves
  // contain colons in edge cases (e.g. a scoop bucket-qualified name),
  // and we only ever care about the prefix before it.
  const colonIndex = source.indexOf(':');
  if (colonIndex === -1) {
    // No colon at all: covers "managed" and "system" (and any other
    // bare, unprefixed value) -- none of these are a known PM prefix.
    return 'System';
  }

  const prefix = source.slice(0, colonIndex);
  return PM_LABELS[prefix] ?? 'System';
}
