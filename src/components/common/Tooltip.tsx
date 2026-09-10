// Copyright (c) 2026 MeedyaSuite

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
 * **Accessibility (a11y audit Fix 14):**
 * - The tooltip text is linked to the trigger element via `aria-describedby`,
 *   so a screen reader announces it as part of the trigger, not as an
 *   unconnected floating box the user has to go hunting for.
 * - Escape hides the tooltip -- the same key that closes every other
 *   floating UI in this app.
 * - The bubble no longer uses `pointer-events-none`. That sounds harmless
 *   but it was the actual cause of "moving the pointer onto the tooltip
 *   dismisses it": the bubble is a DOM child of the hoverable wrapper, so
 *   moving the mouse onto it should never count as leaving the wrapper --
 *   except `pointer-events-none` makes the browser's hit-testing skip
 *   straight through the bubble to whatever is behind it, which very
 *   often is NOT the wrapper. The pointer would then look like it had
 *   left the hoverable area, even though it was sitting right on top of
 *   the tooltip text.
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

import {
  cloneElement,
  isValidElement,
  useEffect,
  useId,
  useRef,
  useState,
  type ReactNode,
} from 'react';

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
   *
   * When this is a single real element (the common case -- an icon, a
   * button, a piece of text), it is cloned with an `aria-describedby`
   * pointing at the tooltip bubble so assistive tech announces the
   * tooltip as a description of that exact element. When it isn't a
   * single element (a fragment, plain text, several siblings), the
   * tooltip still shows visually, it just can't be wired up that way --
   * there is no single DOM node to attach the description to.
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
 * - On Escape: hidden immediately, same as every other floating UI here.
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
export function Tooltip({ content, children, position = 'top' }: TooltipProps) {
  /** Whether the tooltip bubble is currently visible */
  const [visible, setVisible] = useState(false);

  /** Stable id for the tooltip bubble, referenced by the trigger's `aria-describedby`. */
  const tooltipId = useId();

  /**
   * Stores the setTimeout id so it can be cleared on mouse leave or unmount.
   * Uses useRef instead of useState because the timeout ID is a mutable value
   * that doesn't affect rendering and must be accessible in cleanup functions.
   */
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Clear any pending timeout on unmount to prevent state updates
  // on an unmounted component.
  useEffect(() => {
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, []);

  /**
   * Starts the 300ms show delay. Called on mouseenter and focus events.
   * The timeout id is stored in a ref so it can be cancelled if the
   * user leaves before the delay completes.
   */
  const handleMouseEnter = () => {
    timeoutRef.current = setTimeout(() => setVisible(true), 300);
  };

  /**
   * Cancels any pending show delay and hides the tooltip immediately.
   * Called on mouseleave and blur events. Also used to dismiss the
   * tooltip on the events attached to the (now hoverable, see the
   * removal of `pointer-events-none` below) bubble itself, so hovering
   * away from the bubble still closes it.
   */
  const handleMouseLeave = () => {
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = null;
    setVisible(false);
  };

  /** Escape dismisses the tooltip, matching every other floating UI in the app. */
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape' && visible) {
      handleMouseLeave();
    }
  };

  // Wire aria-describedby onto the trigger when it is a single real
  // element -- the common case. Merges with any aria-describedby the
  // caller already put on that element rather than clobbering it.
  const trigger = isValidElement<{ 'aria-describedby'?: string }>(children)
    ? cloneElement(children, {
        'aria-describedby': [children.props['aria-describedby'], tooltipId]
          .filter(Boolean)
          .join(' '),
      })
    : children;

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
      onKeyDown={handleKeyDown}
    >
      {/* Trigger element -- rendered as-is from the consumer, with
          aria-describedby attached when possible (see `trigger` above). */}
      {trigger}

      {/*
       * Tooltip bubble -- only rendered when visible AND content is non-empty.
       *
       * Styling:
       * - absolute + z-50: floats above surrounding content.
       * - POSITION_CLASSES[position]: placement-specific offsets.
       * - whitespace-nowrap: prevents the tooltip from line-wrapping.
       * - role="tooltip": ARIA role for assistive technologies.
       * - id={tooltipId}: the target of the trigger's aria-describedby.
       *
       * Deliberately NOT `pointer-events-none` (see the file-level
       * comment for why that was the actual cause of the tooltip
       * dismissing itself when the pointer moved onto it).
       *
       * @see https://tailwindcss.com/docs/z-index -- stacking context
       */}
      {visible && content && (
        <div
          id={tooltipId}
          role="tooltip"
          onMouseEnter={handleMouseEnter}
          onMouseLeave={handleMouseLeave}
          className={`
            absolute z-50 ${POSITION_CLASSES[position]}
            px-2.5 py-1.5 text-xs font-medium
            bg-surface-elevated text-content-primary
            rounded-platform-sm border border-border-light
            shadow-platform whitespace-nowrap
          `}
        >
          {content}
        </div>
      )}
    </div>
  );
}
