// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file A button that opens a menu of further actions ("More ▾").
 *
 * Built for page headers that have more actions than fit on one row at
 * the smallest window: the important actions stay as buttons, the rest
 * go in here. It reuses {@link ContextMenu} for the menu itself, so the
 * keyboard behaviour is the one the rest of the app already has: the
 * menu opens with focus on its first item, Arrow Up / Down / Home / End
 * move between items, Escape or Tab closes it, and focus goes back to
 * this button however the menu was closed.
 *
 * What a screen reader hears: the button says it opens a menu
 * (`aria-haspopup="menu"`) and whether that menu is open
 * (`aria-expanded`); the menu is named after the button, and each item
 * is a menu item. That follows the WAI-ARIA "menu button" pattern.
 *
 * Arrow Down on the closed button also opens the menu, as that pattern
 * recommends. Arrow Up opening it on the LAST item is not implemented:
 * the shared menu always starts on its first item.
 *
 * @see https://www.w3.org/WAI/ARIA/apg/patterns/menu-button/
 */

import { useCallback, useId, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { ChevronDown } from 'lucide-react';
import { Button } from './Button';
import { ContextMenu, type ContextMenuItem } from './ContextMenu';

interface MenuButtonProps {
  /** Visible label, also the menu's name (for example "More"). */
  label: string;
  /** Optional leading icon. A small down-arrow is always added after the label. */
  icon?: ReactNode;
  /** The menu's items. Disabled items are shown but cannot be chosen. */
  items: ContextMenuItem[];
  /** Tooltip, and the longer name a screen reader reads for the menu. */
  title?: string;
  /** Same sizes as {@link Button}. */
  size?: 'xs' | 'sm' | 'md' | 'lg';
  /** Same variants as {@link Button}; ghost by default, like other header actions. */
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
}

export function MenuButton({ label, icon, items, title, size = 'sm', variant = 'ghost' }: MenuButtonProps) {
  const [anchor, setAnchor] = useState<{ x: number; y: number } | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const menuId = useId();

  /** Open the menu just below the button, lined up with its left edge
   *  (the menu flips left or up by itself if it would leave the window). */
  const open = useCallback(() => {
    const r = buttonRef.current?.getBoundingClientRect();
    if (!r) return;
    setAnchor({ x: r.left, y: r.bottom + 4 });
  }, []);

  const close = useCallback(() => setAnchor(null), []);

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === 'ArrowDown' && !anchor) {
      e.preventDefault();
      open();
    }
  };

  return (
    <>
      <Button
        ref={buttonRef}
        variant={variant}
        size={size}
        icon={icon}
        title={title}
        aria-haspopup="menu"
        aria-expanded={anchor !== null}
        aria-controls={anchor ? menuId : undefined}
        onClick={() => (anchor ? close() : open())}
        onKeyDown={onKeyDown}
        // The open menu closes itself on any mouse press outside it, and
        // this button IS outside it. Without stopping that press here, a
        // click meant to close the menu would close it on the press and
        // open it again on the click.
        onMouseDown={(e) => {
          if (anchor) e.stopPropagation();
        }}
      >
        {label}
        <ChevronDown size={12} aria-hidden="true" />
      </Button>
      {anchor && (
        <ContextMenu id={menuId} label={title ?? label} items={items} x={anchor.x} y={anchor.y} onClose={close} />
      )}
    </>
  );
}
