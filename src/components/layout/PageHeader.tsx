// Copyright (c) 2024-2026 MeedyaDL
/**
 * @file Page header component.
 *
 * Renders the title and optional subtitle for each content page (Download,
 * Queue, Settings, Help). Provides consistent header styling across all
 * pages and an optional right-aligned action slot for buttons.
 *
 * This component is **not** responsible for navigation -- it is purely
 * presentational. The parent page component decides what title, subtitle,
 * and actions to display.
 *
 * Usage pattern (consumed by page components):
 * ```tsx
 * <PageHeader
 *   title="Queue"
 *   subtitle="3 items in queue"
 *   actions={<Button onClick={refresh}>Refresh</Button>}
 * />
 * ```
 *
 * @see https://react.dev/learn/passing-props-to-a-component
 *      React docs -- passing props including optional props.
 * @see https://react.dev/learn/passing-props-to-a-component#passing-jsx-as-children
 *      React docs -- passing JSX via props (used here for the `actions` slot).
 * @see https://tailwindcss.com/docs/border-width  -- border-b utility.
 */

/**
 * React's `ReactNode` type allows the `actions` prop to accept any
 * renderable JSX: buttons, fragments, null, etc.
 * @see https://react.dev/reference/react/ReactNode
 */
import type { ReactNode } from 'react';

/**
 * Props for the {@link PageHeader} component.
 *
 * Only `title` is required. Both `subtitle` and `actions` are optional,
 * making the component flexible enough for simple headings (Help page)
 * and rich headings with counters and action buttons (Queue page).
 */
interface PageHeaderProps {
  /**
   * Main heading text displayed as an `<h2>`.
   * Should be a short page name (e.g., "Download", "Queue", "Settings").
   */
  title: string;

  /**
   * Optional description text rendered below the title in a smaller,
   * muted font. Useful for contextual info like item counts
   * (e.g., "3 items in queue").
   */
  subtitle?: string;

  /**
   * Optional ReactNode rendered on the right side of the header row.
   * Typically one or more `<Button>` components for page-level actions
   * (e.g., "Refresh", "Clear Finished").
   *
   * @see https://react.dev/learn/passing-props-to-a-component#passing-jsx-as-children
   */
  actions?: ReactNode;
}

/**
 * Renders a page header with title, optional subtitle, and action buttons.
 *
 * Layout:
 * ```
 * ┌──────────────────────────────────────────────┐
 * │  Title                         [Action Btns] │
 * │  Subtitle (optional)                         │
 * ├──────────────────────────────────────────────┤
 * ```
 *
 * `items-start` aligns the left (title) and right (actions) columns to
 * the top, so multi-line titles do not vertically center the buttons.
 * `justify-between` pushes the actions to the far right.
 * `border-b border-border-light` draws a subtle separator between the
 * header and the page body content below.
 *
 * @param title    - Main heading text.
 * @param subtitle - Optional description below the title.
 * @param actions  - Optional ReactNode rendered to the right of the title.
 *
 * @see https://tailwindcss.com/docs/justify-content -- justify-between
 * @see https://tailwindcss.com/docs/align-items     -- items-start
 */
export function PageHeader({ title, subtitle, actions }: PageHeaderProps) {
  return (
    <header className="flex items-start justify-between px-6 py-4 border-b border-border-light">
      {/* Left column: title and optional subtitle */}
      <div>
        {/* Page title as an <h2> heading for semantic HTML and accessibility */}
        <h2 className="text-xl font-semibold text-content-primary">{title}</h2>
        {/* Subtitle rendered only when provided (conditional rendering) */}
        {subtitle && (
          <p className="text-sm text-content-secondary mt-0.5">{subtitle}</p>
        )}
      </div>
      {/*
       * Right column: action buttons.
       * Only rendered when the `actions` prop is truthy.
       * `gap-2` (8px) spaces multiple buttons evenly.
       */}
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </header>
  );
}
