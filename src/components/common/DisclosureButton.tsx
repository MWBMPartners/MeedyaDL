// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file The one way MeedyaDL shows "this opens and closes": a chevron
 * pointing right, turning to point down when open, placed just before the
 * title it belongs to.
 *
 * Polish pass (L3): there were three expand styles -- a "▶" glyph in
 * Settings sections (centred across the title AND its description, so it
 * looked attached to the description), Lucide chevrons that swapped
 * between two icons on the Cookies tab and in Tools, "▼ / ▶" text in the
 * Advanced tab, and the browser's own "▸" on folded details. Everything
 * now uses `ExpandChevron`; folded `<details>` get the same chevron from
 * globals.css.
 */

import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { ChevronRight } from 'lucide-react';

/** The chevron itself: right when closed, rotated to point down when open. */
export function ExpandChevron({ open, size = 14 }: { open: boolean; size?: number }) {
  return (
    // The icon is decorative (`aria-hidden`): `aria-expanded` on the button
    // already tells a screen reader whether the section is open. Turning the
    // right-pointing chevron 90 degrees makes it point down.
    // See: https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Attributes/aria-expanded
    <ChevronRight
      size={size}
      aria-hidden="true"
      className={`flex-shrink-0 text-content-tertiary transition-transform duration-150 ${open ? 'rotate-90' : ''}`}
    />
  );
}

/** Props for `DisclosureButton`: any button attribute, plus `open` and a title. */
interface DisclosureButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  /** Whether the part it controls is open. */
  open: boolean;
  /** The title, shown after the chevron. */
  children: ReactNode;
}

/**
 * A full-width button that opens and closes a section: the chevron, then
 * the title, left-aligned. `aria-expanded` says whether it is open; pass
 * `aria-controls` when the section has an id.
 */
export function DisclosureButton({
  open,
  children,
  className = '',
  type = 'button',
  ...rest
}: DisclosureButtonProps) {
  return (
    // `type` defaults to "button" so the control never submits a surrounding form by accident.
    // `aria-expanded` is what makes this a disclosure widget for assistive tech.
    // See: https://www.w3.org/WAI/ARIA/apg/patterns/disclosure/
    <button
      type={type}
      aria-expanded={open}
      className={`flex w-full items-center gap-2 text-left transition-colors
        focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2
        ${className}`}
      {...rest}
    >
      <ExpandChevron open={open} />
      <span className="min-w-0 flex-1">{children}</span>
    </button>
  );
}
