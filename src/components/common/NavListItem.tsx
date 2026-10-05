// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file One entry in a vertical list of places: the main sidebar, the
 * Settings tabs, the Help topics.
 *
 * Polish pass (M17, L3): the three lists were hand-made separately and had
 * drifted -- different gaps, the Help list's current topic in a colour too
 * light to read as small text, and a two-line Settings label ("Codec &
 * Resolution") centred because a <button> centres its text unless told
 * otherwise, while one-line labels sat on the left. One component, labels
 * always left-aligned. `tone` picks the sidebar's own colours or the
 * content area's.
 */

import type { ButtonHTMLAttributes, ReactNode } from 'react';

interface NavListItemProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  /** The icon, already sized (18px in the sidebar, 16px elsewhere). */
  icon: ReactNode;
  /** The label; omitted when only the icon is shown (collapsed sidebar). */
  children?: ReactNode;
  /** Whether this is the current page, tab or topic. */
  active: boolean;
  /** The sidebar's colours, or the content area's. */
  tone?: 'sidebar' | 'content';
  /**
   * 'sm' for the sidebar's footer entry (smaller text, muted until
   * hovered); 'md' for the lists of places.
   */
  size?: 'sm' | 'md';
  /** Draw the label in the accent colour (the footer's "updates available"). */
  highlight?: boolean;
}

export function NavListItem({
  icon,
  children,
  active,
  tone = 'content',
  size = 'md',
  highlight = false,
  className = '',
  type = 'button',
  ...rest
}: NavListItemProps) {
  const colours =
    size === 'sm'
      ? highlight
        ? 'text-accent-hover hover:bg-sidebar-hover'
        : 'text-content-tertiary hover:text-content-primary hover:bg-sidebar-hover'
      : tone === 'sidebar'
      ? active
        ? 'bg-sidebar-active text-sidebar-text-active font-medium'
        : 'text-sidebar-text hover:bg-sidebar-hover'
      : active
        ? // text-accent-hover, not the plain accent: it clears 4.5:1 as small text (see base.css)
          'bg-accent-light text-accent-hover font-medium'
        : 'text-content-secondary hover:text-content-primary hover:bg-surface-secondary';
  return (
    <button
      type={type}
      aria-current={active ? 'page' : undefined}
      className={`w-full flex items-center rounded-platform text-left transition-colors
        ${size === 'sm' ? 'gap-2 px-2 py-1.5 text-xs' : 'gap-2.5 px-3 py-2 text-sm'}
        focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2
        ${colours} ${children ? '' : 'justify-center'} ${className}`}
      {...rest}
    >
      {icon}
      {children && <span className="min-w-0 flex-1">{children}</span>}
    </button>
  );
}
