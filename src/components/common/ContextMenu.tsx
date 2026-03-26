// Copyright (c) 2026 MeedyaDL
/**
 * @file Reusable right-click context menu overlay.
 *
 * Renders a fixed-position menu at the cursor location. The menu auto-
 * adjusts if it would overflow the viewport (flips up/left as needed).
 *
 * Rendered via a React Portal to `document.body` so it is never clipped
 * by parent overflow rules.
 *
 * Closes on: click outside, Escape key, clicking a menu item, or
 * window scroll.
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
 */

import { useEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

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
  const menuRef = useRef<HTMLDivElement>(null);

  // Adjusted position after viewport overflow check
  const [pos, setPos] = useState({ x, y });

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

  // Close on Escape key, outside click, and window scroll
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
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

  // Filter out items with no meaningful action
  const visibleItems = items.filter(Boolean);
  if (visibleItems.length === 0) return null;

  return createPortal(
    <div
      ref={menuRef}
      role="menu"
      aria-label="Actions menu"
      style={{ left: pos.x, top: pos.y }}
      className="fixed z-50 min-w-[180px] py-1 bg-surface-elevated border border-border rounded-platform shadow-lg"
    >
      {visibleItems.map((item, i) => (
        <div key={i}>
          {/* Separator line above this item */}
          {item.separator && i > 0 && <div className="border-t border-border-light my-1" />}

          <button
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
