// Copyright (c) 2024-2026 MeedyaDL
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
      {/* Clickable header */}
      <button
        type="button"
        className="w-full flex items-center gap-2 px-4 py-3 text-left hover:bg-surface-secondary/50 transition-colors rounded-t-lg"
        onClick={() => setOpen(!open)}
        aria-expanded={open ? 'true' : 'false'}
      >
        <span className={`text-xs text-content-tertiary select-none transition-transform duration-150 ${open ? 'rotate-90' : 'rotate-0'}`}>
          ▶
        </span>
        <div className="flex-1 min-w-0">
          <h3 className="text-sm font-semibold text-content-primary">{title}</h3>
          {description && (
            <p className="text-xs text-content-tertiary mt-0.5 leading-relaxed">{description}</p>
          )}
        </div>
      </button>

      {/* Collapsible content */}
      {open && (
        <div className="px-4 pb-4 pt-1 space-y-4 border-t border-border/50">
          {children}
        </div>
      )}
    </div>
  );
}
