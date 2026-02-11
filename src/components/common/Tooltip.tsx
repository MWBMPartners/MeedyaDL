// Copyright (c) 2024-2026 MWBM Partners Ltd

/**
 * @file Tooltip component.
 *
 * Displays a small floating text label that appears when the user hovers over
 * or focuses (via keyboard) a trigger element. The tooltip uses absolute CSS
 * positioning relative to the trigger wrapper -- no portal or JavaScript
 * measurement is required.
 *
 * A 300ms delay is applied before the tooltip becomes visible to avoid
 * accidental flashes when the cursor passes over the trigger briefly.
 *
 * **Position options:** top (default), bottom, left, right.
 *
 * **Usage across the application:**
 * - Sidebar: tooltips on icon-only navigation buttons.
 * - CookiesTab: helper tooltips on configuration fields.
 *
 * @see https://react.dev/reference/react/useState -- React useState hook
 * @see https://tailwindcss.com/docs/position#absolute -- Tailwind absolute positioning
 * @see https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles/tooltip_role
 *      MDN -- the ARIA tooltip role.
 */

import { useState, type ReactNode } from 'react';

/**
 * Props accepted by the {@link Tooltip} component.
 */
interface TooltipProps {
  /**
   * Plain text content displayed inside the tooltip bubble.
   * When this is an empty string (or falsy), the tooltip is not rendered
   * even if the visibility state is true, preventing an empty popup.
   */
  content: string;

  /**
   * The trigger element that the user hovers over or focuses to reveal
   * the tooltip. Wrapped inside a relative-positioned <div> for
   * absolute-positioning of the tooltip bubble.
   */
  children: ReactNode;

  /**
   * Placement of the tooltip relative to its trigger. Defaults to 'top'.
   * Each position maps to a set of Tailwind positioning / translation
   * classes in {@link POSITION_CLASSES}.
   */
  position?: 'top' | 'bottom' | 'left' | 'right';
}

/**
 * Tailwind positioning and translation classes for each tooltip placement.
 *
 * Each entry positions the tooltip on the correct side of the trigger element
 * and centres it along the perpendicular axis using translate transforms.
 * A small margin (mb-2, mt-2, mr-2, ml-2 = 8px) provides breathing room.
 *
 * @see https://tailwindcss.com/docs/translate -- translate utilities
 * @see https://tailwindcss.com/docs/top-right-bottom-left -- inset utilities
 */
const POSITION_CLASSES: Record<string, string> = {
  /** Above the trigger, horizontally centred */
  top: 'bottom-full left-1/2 -translate-x-1/2 mb-2',
  /** Below the trigger, horizontally centred */
  bottom: 'top-full left-1/2 -translate-x-1/2 mt-2',
  /** To the left of the trigger, vertically centred */
  left: 'right-full top-1/2 -translate-y-1/2 mr-2',
  /** To the right of the trigger, vertically centred */
  right: 'left-full top-1/2 -translate-y-1/2 ml-2',
};

/**
 * Renders a tooltip that appears on hover or focus of its child element.
 *
 * **Show / hide logic:**
 * - On mouse enter (or focus): a 300ms timeout is started. If the cursor
 *   remains over the trigger for the full 300ms, `visible` is set to true.
 * - On mouse leave (or blur): the timeout is cancelled (if still pending)
 *   and `visible` is set to false immediately.
 *
 * This delay prevents the tooltip from flashing when the user moves the
 * mouse quickly across the trigger without intending to read the tooltip.
 *
 * @example
 * ```tsx
 * <Tooltip content="Download settings" position="right">
 *   <Settings size={20} />
 * </Tooltip>
 * ```
 *
 * @param content  - Text to display inside the tooltip bubble
 * @param children - The trigger element (wrapped in a relative container)
 * @param position - Placement relative to the trigger (default: 'top')
 */
export function Tooltip({
  content,
  children,
  position = 'top',
}: TooltipProps) {
  /** Whether the tooltip bubble is currently visible */
  const [visible, setVisible] = useState(false);

  /**
   * Stores the setTimeout id so it can be cleared on mouse leave.
   * Using `ReturnType<typeof setTimeout>` keeps the type compatible
   * across browser (number) and Node.js (NodeJS.Timeout) environments.
   */
  const [timeoutId, setTimeoutId] = useState<ReturnType<typeof setTimeout> | null>(null);

  /**
   * Starts the 300ms show delay. Called on mouseenter and focus events.
   * The timeout id is stored in state so it can be cancelled if the
   * user leaves before the delay completes.
   */
  const handleMouseEnter = () => {
    const id = setTimeout(() => setVisible(true), 300);
    setTimeoutId(id);
  };

  /**
   * Cancels any pending show delay and hides the tooltip immediately.
   * Called on mouseleave and blur events.
   */
  const handleMouseLeave = () => {
    if (timeoutId) clearTimeout(timeoutId);
    setTimeoutId(null);
    setVisible(false);
  };

  return (
    /*
     * Wrapper div -- relative positioning establishes the containing block
     * for the absolutely-positioned tooltip bubble. inline-flex keeps the
     * wrapper the same size as the trigger child element.
     *
     * Event handlers for both mouse and keyboard (focus/blur) ensure the
     * tooltip is accessible to keyboard-only users.
     */
    <div
      className="relative inline-flex"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      onFocus={handleMouseEnter}
      onBlur={handleMouseLeave}
    >
      {/* Trigger element -- rendered as-is from the consumer */}
      {children}

      {/*
       * Tooltip bubble -- only rendered when visible AND content is non-empty.
       *
       * Styling:
       * - absolute + z-50: floats above surrounding content.
       * - POSITION_CLASSES[position]: placement-specific offsets.
       * - whitespace-nowrap: prevents the tooltip from line-wrapping.
       * - pointer-events-none: allows the cursor to pass through the bubble
       *   so it does not interfere with clicks on nearby elements.
       * - role="tooltip": ARIA role for assistive technologies.
       *
       * @see https://tailwindcss.com/docs/z-index -- stacking context
       */}
      {visible && content && (
        <div
          className={`
            absolute z-50 ${POSITION_CLASSES[position]}
            px-2.5 py-1.5 text-xs font-medium
            bg-surface-elevated text-content-primary
            rounded-platform-sm border border-border-light
            shadow-platform whitespace-nowrap
            pointer-events-none
          `}
          role="tooltip"
        >
          {content}
        </div>
      )}
    </div>
  );
}
