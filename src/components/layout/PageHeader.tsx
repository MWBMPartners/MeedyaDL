/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Page header component.
 * Renders the title and subtitle for each content page (Download, Queue,
 * Settings, Help). Provides consistent header styling across all pages.
 */

import type { ReactNode } from 'react';

interface PageHeaderProps {
  /** Page title text */
  title: string;
  /** Optional subtitle/description text */
  subtitle?: string;
  /** Optional action elements (buttons, etc.) rendered on the right side */
  actions?: ReactNode;
}

/**
 * Renders a page header with title, optional subtitle, and action buttons.
 *
 * @param title - Main heading text
 * @param subtitle - Optional description text below the title
 * @param actions - Optional ReactNode rendered to the right of the title
 */
export function PageHeader({ title, subtitle, actions }: PageHeaderProps) {
  return (
    <header className="flex items-start justify-between px-6 py-4 border-b border-border-light">
      <div>
        <h2 className="text-xl font-semibold text-content-primary">{title}</h2>
        {subtitle && (
          <p className="text-sm text-content-secondary mt-0.5">{subtitle}</p>
        )}
      </div>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </header>
  );
}
