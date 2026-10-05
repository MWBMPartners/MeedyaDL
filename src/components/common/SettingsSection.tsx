// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Collapsible settings section component.
 *
 * Renders a visually distinct, collapsible container for grouping related
 * settings. Used across all settings tabs for consistent section styling.
 *
 * Features:
 * - Clickable header with chevron indicator (▶ / ▼)
 * - Bordered card with subtle background for visual separation
 * - Smooth content reveal via CSS transitions
 * - Optional `defaultOpen` prop (defaults to `true`)
 * - Optional description text below the title
 */

import { useState, type ReactNode } from 'react';

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
    <div className="rounded-lg border border-border bg-surface-secondary/30">
      {/* Header: the heading CONTAINS the button, not the other way round.
          It used to be <button><h3>…</h3></button>; a heading inside a
          button is flattened into the button's name, so screen readers
          found no headings at all in Settings. <h2> because the page title
          above is the page's <h1>. The description sits outside the button
          so the button's name is just the section title. */}
      <div className="px-4 py-3 hover:bg-surface-secondary/50 transition-colors rounded-t-lg">
        <h2 className="text-sm font-semibold text-content-primary">
          <button
            type="button"
            className="w-full flex items-center gap-2 text-left"
            onClick={() => setOpen(!open)}
            aria-expanded={open ? 'true' : 'false'}
          >
            {/* Fix 7 (a11y audit): without aria-hidden, a screen reader
                read "black right-pointing triangle" out loud before every
                single section title on every settings tab -- aria-expanded
                on the button above already says open/closed, so this
                glyph is purely decorative. */}
            <span
              className={`text-xs text-content-tertiary select-none transition-transform duration-150 ${open ? 'rotate-90' : 'rotate-0'}`}
              aria-hidden="true"
            >
              ▶
            </span>
            <span className="flex-1 min-w-0">{title}</span>
          </button>
        </h2>
        {description && (
          <p className="text-xs text-content-tertiary mt-0.5 ml-5 leading-relaxed">{description}</p>
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
