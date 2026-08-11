// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file upgrade-generic-component.ts -- Shared per-component upgrade routing.
 *
 * `ComponentUpdate` entries for anything other than GAMDL, the MeedyaDL
 * app itself, or the Python runtime ("generic" components -- pip-based
 * engines like votify, or binary tools like FFmpeg/MP4Box tracked by
 * `tool_id`) all upgrade through the same three-way dispatch. This was
 * previously inlined once inside `UpdatesPage.tsx`'s bulk "Update All"
 * handler; it is now shared so both `UpdatesPage.tsx`'s per-row buttons
 * and (bounded) `UpdateBanner.tsx` rows call the exact same routing
 * logic rather than risking drift between two copies.
 *
 * For a package-manager-owned tool (`ComponentUpdate.managed_by` set),
 * the actual delegation to the owning package manager's `upgrade()`
 * happens entirely on the Rust side, inside `install_tool` Step 0 --
 * directly for no-elevation managers (Homebrew/pipx/Scoop) or through the
 * `sudo -n`/`pkexec` elevation tiers for root-requiring ones
 * (apt/dnf/snap/MacPorts); a failed or un-elevatable update is non-fatal
 * (adopt-as-found + Activity-Log guidance). See
 * `.github/audits/package-manager-abstraction-design-2026-08-10.md`
 * §3.C Seam 1. This function's `tool_id` branch below is unaware of
 * that distinction by design; it always calls `installDependency`, and
 * the backend decides what "install/upgrade" means for that tool.
 */

import type { ComponentUpdate } from '@/types';

/**
 * Runs the update for a single generic (non-core) component and
 * returns its resulting version string.
 *
 * @param c - The `ComponentUpdate` entry to upgrade. Must have either
 *   `pip_package` or `tool_id` set; anything else throws.
 * @returns The resulting version string reported by the backend.
 * @throws If neither `pip_package` nor `tool_id` is set on `c`.
 */
export async function upgradeGenericComponent(c: ComponentUpdate): Promise<string> {
  // votify (A4) has its own validated version window — route through
  // the bounded `upgradeVotify` so this can never silently jump to an
  // unaudited above-ceiling release the way the unbounded generic
  // `upgradePipEngine` path would. Only pass an explicit target when
  // this specific update was flagged "Untested" (above-ceiling) —
  // otherwise the backend resolves the newest version inside the
  // tested window on its own.
  if (c.pip_package === 'votify') {
    const { upgradeVotify } = await import('@/lib/tauri-commands');
    return upgradeVotify(c.is_untested ? (c.latest_version ?? undefined) : undefined);
  } else if (c.pip_package) {
    const { upgradePipEngine } = await import('@/lib/tauri-commands');
    return upgradePipEngine(c.pip_package);
  } else if (c.tool_id) {
    const { installDependency } = await import('@/lib/tauri-commands');
    return installDependency(c.tool_id);
  }
  throw new Error(`No upgrade method for ${c.name}`);
}
