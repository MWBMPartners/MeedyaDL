/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Tooltip component.
 * Displays a small text popup on hover/focus over a trigger element.
 * Uses CSS-only positioning with a delayed show for a smooth UX.
 */

import { useState, type ReactNode } from 'react';

interface TooltipProps {
  /** Text content of the tooltip */
  content: string;
  /** The element that triggers the tooltip on hover */
  children: ReactNode;
  /** Position of the tooltip relative to the trigger (default: 'top') */
  position?: 'top' | 'bottom' | 'left' | 'right';
}

/** Tailwind positioning classes for each tooltip placement */
const POSITION_CLASSES: Record<string, string> = {
  top: 'bottom-full left-1/2 -translate-x-1/2 mb-2',
  bottom: 'top-full left-1/2 -translate-x-1/2 mt-2',
  left: 'right-full top-1/2 -translate-y-1/2 mr-2',
  right: 'left-full top-1/2 -translate-y-1/2 ml-2',
};

/**
 * Renders a tooltip that appears on hover or focus of its child element.
 * Uses a 300ms delay before showing to avoid accidental triggers.
 *
 * @param content - Text to display in the tooltip
 * @param children - Trigger element
 * @param position - Placement relative to the trigger (default: 'top')
 */
export function Tooltip({
  content,
  children,
  position = 'top',
}: TooltipProps) {
  const [visible, setVisible] = useState(false);
  const [timeoutId, setTimeoutId] = useState<ReturnType<typeof setTimeout> | null>(null);

  /** Show tooltip after a short delay */
  const handleMouseEnter = () => {
    const id = setTimeout(() => setVisible(true), 300);
    setTimeoutId(id);
  };

  /** Cancel the delay or hide the tooltip */
  const handleMouseLeave = () => {
    if (timeoutId) clearTimeout(timeoutId);
    setTimeoutId(null);
    setVisible(false);
  };

  return (
    <div
      className="relative inline-flex"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      onFocus={handleMouseEnter}
      onBlur={handleMouseLeave}
    >
      {children}

      {/* Tooltip popup */}
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
