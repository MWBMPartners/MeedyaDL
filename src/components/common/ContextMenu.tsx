// Copyright (c) 2026 MeedyaSuite
/**
 * @file Reusable right-click context menu overlay.
 *
 * Renders a fixed-position menu at the cursor location. The menu auto-
 * adjusts if it would overflow the viewport (flips up/left as needed).
 *
 * Rendered via a React Portal to `document.body` so it is never clipped
 * by parent overflow rules.
 *
 * Closes on: click outside, Escape key, clicking a menu item, Tab (lets
 * the browser continue moving focus to whatever comes next), or window
 * scroll.
 *
 * **Keyboard support** (a11y audit fix — this menu used to be mouse-only):
 * when the menu opens, focus moves to its first item. Arrow Up / Arrow
 * Down move between items (wrapping at the ends); Home / End jump to the
 * first / last item. Whatever had focus before the menu opened — a
 * right-clicked row, or an overflow ("⋯") button — gets focus back the
 * moment the menu closes, no matter how it closed (Escape, an item
 * click, clicking outside, Tab, or scrolling away). Without that, a
 * keyboard user who opened the menu would be dropped onto the page with
 * no idea where focus went.
 *
 * @example
 * ```tsx
 * <ContextMenu
 *   items={[
 *     { label: 'Copy Link', icon: <Copy size={14} />, onClick: handleCopy },
 *     { label: 'Delete', icon: <Trash2 size={14} />, onClick: handleDelete, separator: true },
 *   ]}
 *   x={event.clientX}
 *   y={event.clientY}
 *   onClose={() => setMenuVisible(false)}
 * />
 * ```
 *
 * @see https://react.dev/reference/react-dom/createPortal
 * @see https://www.w3.org/WAI/ARIA/apg/patterns/menu-button/ -- WAI-ARIA menu keyboard pattern
 */

import { useEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

// This is the one place a right-click menu's own name is spoken by a
// screen reader, so translating it here covers every menu in the app.
import { useTranslation } from 'react-i18next';

/**
 * A single item in the context menu.
 *
 * Set `separator: true` to render a visual divider line **above** the
 * item, grouping related actions together.
 */
export interface ContextMenuItem {
  /** Display text for the menu item */
  label: string;
  /** Optional leading icon (typically a 14px Lucide icon) */
  icon?: ReactNode;
  /** Callback invoked when the item is clicked */
  onClick: () => void;
  /** When true, the item is shown but not clickable */
  disabled?: boolean;
  /** When true, a horizontal divider is rendered above this item */
  separator?: boolean;
}

/**
 * Props for the {@link ContextMenu} component.
 */
interface ContextMenuProps {
  /** Menu items to display */
  items: ContextMenuItem[];
  /** Horizontal position (px from viewport left edge) */
  x: number;
  /** Vertical position (px from viewport top edge) */
  y: number;
  /** Callback to close/unmount the menu */
  onClose: () => void;
}

/**
 * ContextMenu -- Fixed-position context menu rendered via a portal.
 *
 * After the initial render the component measures its own dimensions
 * and adjusts position so the menu stays fully within the viewport.
 */
export function ContextMenu({ items, x, y, onClose }: ContextMenuProps) {
  const { t } = useTranslation();
  const menuRef = useRef<HTMLDivElement>(null);

  // Adjusted position after viewport overflow check
  const [pos, setPos] = useState({ x, y });

  // Filter out items with no meaningful action -- computed up front so
  // the ref array below is sized to what actually renders.
  const visibleItems = items.filter(Boolean);

  /**
   * One button ref per rendered item, indexed the same way as
   * `visibleItems`. Rebuilt every render (menus are small and rebuild
   * cheaply) so a menu whose item list changes while open — e.g. a
   * "disabled" flag flips — never nav igates against stale refs.
   */
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  itemRefs.current = itemRefs.current.slice(0, visibleItems.length);

  /**
   * The element that had focus immediately before this menu mounted --
   * the row that was right-clicked, or the overflow button that opened
   * it. Restored on close so a keyboard user is never left with focus
   * nowhere in particular. Captured once, on mount.
   */
  const previouslyFocusedRef = useRef<HTMLElement | null>(null);
  useEffect(() => {
    previouslyFocusedRef.current = document.activeElement as HTMLElement | null;
    return () => {
      previouslyFocusedRef.current?.focus?.();
    };
  }, []);

  /**
   * Move focus onto the first enabled item once, right after mount.
   * Refs are already attached by the time this effect runs (React
   * commits refs before running effects), so no extra tick is needed.
   */
  useEffect(() => {
    const firstEnabled = itemRefs.current.find(
      (el) => el && !el.disabled
    );
    firstEnabled?.focus();
    // Deliberately empty deps -- this should fire exactly once, when
    // the menu first appears, not every time the item list re-renders.
  }, []);

  // Measure menu dimensions and flip if necessary
  useEffect(() => {
    const el = menuRef.current;
    if (!el) return;

    const rect = el.getBoundingClientRect();
    let adjustedX = x;
    let adjustedY = y;

    // Flip left if menu overflows right edge
    if (x + rect.width > window.innerWidth) {
      adjustedX = x - rect.width;
    }
    // Flip up if menu overflows bottom edge
    if (y + rect.height > window.innerHeight) {
      adjustedY = y - rect.height;
    }
    // Clamp to viewport edges (safety net)
    adjustedX = Math.max(0, adjustedX);
    adjustedY = Math.max(0, adjustedY);

    setPos({ x: adjustedX, y: adjustedY });
  }, [x, y]);

  // Close on Escape key, outside click, and window scroll. Also handles
  // the arrow-key / Home / End navigation between items.
  useEffect(() => {
    const enabledIndexes = () =>
      itemRefs.current
        .map((el, i) => (el && !el.disabled ? i : -1))
        .filter((i) => i !== -1);

    const focusAt = (index: number) => {
      itemRefs.current[index]?.focus();
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
        return;
      }
      // Tab is the WAI-ARIA-menu convention for "leave the menu" -- it
      // is deliberately NOT trapped here, so the browser's normal Tab
      // order continues from wherever focus lands once the menu closes.
      if (e.key === 'Tab') {
        onClose();
        return;
      }

      const enabled = enabledIndexes();
      if (enabled.length === 0) return;

      const activeIndex = itemRefs.current.findIndex(
        (el) => el === document.activeElement
      );
      const posInEnabled = enabled.indexOf(activeIndex);

      if (e.key === 'ArrowDown') {
        e.preventDefault();
        const next = enabled[(posInEnabled + 1 + enabled.length) % enabled.length];
        focusAt(next);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        const prevSlot = posInEnabled <= 0 ? enabled.length - 1 : posInEnabled - 1;
        focusAt(enabled[prevSlot]);
      } else if (e.key === 'Home') {
        e.preventDefault();
        focusAt(enabled[0]);
      } else if (e.key === 'End') {
        e.preventDefault();
        focusAt(enabled[enabled.length - 1]);
      }
    };
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleScroll = () => onClose();

    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('mousedown', handleClick);
    window.addEventListener('scroll', handleScroll, true);

    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.removeEventListener('mousedown', handleClick);
      window.removeEventListener('scroll', handleScroll, true);
    };
  }, [onClose]);

  if (visibleItems.length === 0) return null;

  return createPortal(
    <div
      ref={menuRef}
      role="menu"
      aria-label={t('common.actionsMenu')}
      style={{ left: pos.x, top: pos.y }}
      className="fixed z-50 min-w-[180px] py-1 bg-surface-elevated border border-border rounded-platform shadow-lg"
    >
      {visibleItems.map((item, i) => (
        <div key={i}>
          {/* Separator line above this item */}
          {item.separator && i > 0 && <div className="border-t border-border-light my-1" />}

          <button
            ref={(el) => {
              itemRefs.current[i] = el;
            }}
            type="button"
            role="menuitem"
            disabled={item.disabled}
            onClick={() => {
              if (!item.disabled) {
                item.onClick();
                onClose();
              }
            }}
            className={`w-full flex items-center gap-2 px-3 py-1.5 text-sm text-left transition-colors ${
              item.disabled
                ? 'opacity-50 cursor-default'
                : 'text-content-primary hover:bg-surface-secondary cursor-pointer'
            }`}
          >
            {item.icon && <span className="text-content-secondary flex-shrink-0">{item.icon}</span>}
            {item.label}
          </button>
        </div>
      ))}
    </div>,
    document.body
  );
}
