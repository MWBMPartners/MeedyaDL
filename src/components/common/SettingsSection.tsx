// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Collapsible settings section component.
 *
 * Renders a visually distinct, collapsible container for grouping related
 * settings. Used across all settings tabs for consistent section styling.
 *
 * Features:
 * - Clickable header with the shared expand chevron (ExpandChevron)
 * - Bordered card with subtle background for visual separation
 * - Smooth content reveal via CSS transitions
 * - Optional `defaultOpen` prop (defaults to `true`)
 * - Optional description text below the title
 */

import { useState, type ReactNode } from 'react';
import { ExpandChevron } from './DisclosureButton';

interface SettingsSectionProps {
  /** Section heading text */
  title: string;
  /** Optional description shown below the title */
  description?: string;
  /** Whether the section is expanded by default */
  defaultOpen?: boolean;
  /** Section content (toggles, inputs, etc.) */
  children: ReactNode;
}

/**
 * SettingsSection -- A collapsible, visually distinct settings group.
 *
 * Wraps children in a bordered card with a clickable header. All settings
 * tabs use this component for consistent section styling and layout.
 */
export function SettingsSection({
  title,
  description,
  defaultOpen = true,
  children,
}: SettingsSectionProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className="rounded-platform-lg border border-border bg-surface-secondary/30">
      {/* Header: the heading CONTAINS the button, not the other way round.
          It used to be <button><h3>…</h3></button>; a heading inside a
          button is flattened into the button's name, so screen readers
          found no headings at all in Settings. <h2> because the page title
          above is the page's <h1>. The description sits outside the button
          so the button's name is just the section title. */}
      <div className="px-4 py-3 hover:bg-surface-secondary/50 transition-colors rounded-t-platform-lg">
        <h2 className="text-sm font-semibold text-content-primary">
          <button
            type="button"
            className="w-full flex items-center gap-2 text-left"
            onClick={() => setOpen(!open)}
            aria-expanded={open ? 'true' : 'false'}
          >
            {/* The shared chevron (polish pass L3): this used to be a "▶"
                character, one of three expand styles in the app. It is
                hidden from screen readers; aria-expanded on the button
                already says open or closed. */}
            <ExpandChevron open={open} />
            <span className="flex-1 min-w-0">{title}</span>
          </button>
        </h2>
        {description && (
          // ml-5.5 lines the description up under the title: the chevron
          // is 14px wide plus the 8px gap (gap-2) beside it.
          <p className="text-xs text-content-tertiary mt-0.5 ml-5.5 leading-relaxed">{description}</p>
        )}
      </div>

      {/* Collapsible content */}
      {open && (
        <div className="px-4 pb-4 pt-1 space-y-4 border-t border-border/50">
          {children}
        </div>
      )}
    </div>
  );
}
