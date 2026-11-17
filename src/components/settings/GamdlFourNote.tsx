/**
 * Copyright (c) 2024-2026 MeedyaSuite
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file GamdlFourNote — a one-line note beside a setting that works
 * differently, or not at all, when GAMDL 4 is installed.
 *
 * GAMDL 4 (released 12 Nov 2026) is a rewrite: some settings MeedyaDL
 * used to pass on have no equivalent there. MeedyaDL keeps the setting
 * (it still applies whenever GAMDL 3 is installed, and a person may move
 * back) but says plainly what happens with GAMDL 4, rather than leave it
 * looking as if it still did something. The caller decides whether GAMDL 4
 * is installed (`useGamdlCapabilities().capabilities.gamdl_v4`), so one
 * capability lookup serves every note on a screen.
 */

import type { ReactNode } from 'react';

export interface GamdlFourNoteProps {
  /** Show the note only when the installed GAMDL is 4.x. */
  show: boolean;
  /** What happens with GAMDL 4, in plain words. */
  children: ReactNode;
}

/** Renders nothing unless `show` is true. */
export function GamdlFourNote({ show, children }: GamdlFourNoteProps) {
  if (!show) return null;
  return <p className="text-xs text-content-tertiary">With GAMDL 4: {children}</p>;
}
