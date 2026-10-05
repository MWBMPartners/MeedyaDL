// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file A button that shows only an icon: row actions (cancel, retry, open
 * folder, ⋮), "clear search" crosses, collapse toggles.
 *
 * Polish pass (M17): these were hand-made in a dozen places, each with its
 * own padding, corner and hover colour. One component now. `label` is
 * required because an icon-only button has no other name: it becomes both
 * the accessible name and the tooltip.
 */

import type { ButtonHTMLAttributes, ReactNode, Ref } from 'react';

interface IconButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'aria-label' | 'title'> {
  /** The icon (a Lucide icon, about 14px). */
  icon: ReactNode;
  /** What the button does; its accessible name and its tooltip. */
  label: string;
  /** 'sm' (4px padding) for dense rows and fields; 'md' (6px) otherwise. */
  size?: 'sm' | 'md';
  /** 'danger' turns the hover colour red, for destructive actions. */
  tone?: 'default' | 'danger';
  /** The underlying button (React 19 passes `ref` as a prop). */
  ref?: Ref<HTMLButtonElement>;
}

export function IconButton({ icon, label, size = 'md', tone = 'default', className = '', type = 'button', ...rest }: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      title={label}
      className={`inline-flex flex-shrink-0 items-center justify-center rounded-platform transition-colors
        text-content-tertiary hover:bg-surface-elevated disabled:opacity-50 disabled:cursor-not-allowed
        focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2
        ${tone === 'danger' ? 'hover:text-status-error' : 'hover:text-content-primary'}
        ${size === 'sm' ? 'p-1' : 'p-1.5'}
        ${className}`}
      {...rest}
    >
      {icon}
    </button>
  );
}
