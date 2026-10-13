// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file A small on/off pill used to filter a list ("Complete (3)",
 * "System").
 *
 * The Queue page and the Activity page each drew their own version: the
 * Queue's were round pills, the Activity page's were square buttons with
 * a different size, and the Activity ones said "checkbox" to a screen
 * reader while the Queue ones said "toggle button". One component now,
 * one look and one meaning: a toggle button (`aria-pressed`), which is
 * what a filter that is either on or off is.
 *
 * `tone` only changes the colour when the chip is ON, so the Activity
 * page can keep its colour per category (blue System, accent Download,
 * amber Verbose) and match the colours of the lines it filters.
 */

import type { ButtonHTMLAttributes, ReactNode } from 'react';

type FilterChipTone = 'accent' | 'info' | 'warning';

/** Colours when the chip is on. */
const ON_CLASSES: Record<FilterChipTone, string> = {
  accent: 'bg-accent text-content-on-accent border-accent',
  info: 'bg-status-info/15 text-status-info-text border-status-info/30',
  warning: 'bg-status-warning/15 text-status-warning-text border-status-warning/30',
};

interface FilterChipProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'onClick'> {
  /** Whether the filter is on. */
  pressed: boolean;
  /** Called when the chip is clicked (or pressed with Enter / Space). */
  onClick: () => void;
  /** The chip's label. */
  children: ReactNode;
  /** Colour when on; accent unless the list colours its lines by category. */
  tone?: FilterChipTone;
}

export function FilterChip({ pressed, onClick, children, tone = 'accent', className = '', ...rest }: FilterChipProps) {
  return (
    <button
      type="button"
      aria-pressed={pressed}
      onClick={onClick}
      className={`px-2.5 py-0.5 text-xs font-medium rounded-full border transition-colors whitespace-nowrap cursor-pointer
        focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2
        ${pressed ? ON_CLASSES[tone] : 'bg-surface-elevated text-content-secondary border-border-light hover:border-accent'}
        ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
