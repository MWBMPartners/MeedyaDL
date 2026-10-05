// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file The folded "Details" part under an error message (polish pass M8).
 *
 * An error message says what went wrong and what to do next in plain
 * English; the technical text it came with (a backend error, a traceback,
 * a file path) sits in here, closed, for anybody reporting the problem.
 * One component so that toasts, inline errors and failed queue and
 * history rows all fold it the same way.
 */

interface ErrorDetailsProps {
  /** The technical text. Nothing is drawn when it is empty. */
  details: string | null | undefined;
  /** The word on the fold; "Details" unless the caller translates it. */
  label?: string;
  /** Extra classes on the outer element (spacing). */
  className?: string;
}

export function ErrorDetails({ details, label = 'Details', className = 'mt-1' }: ErrorDetailsProps) {
  if (!details) return null;
  return (
    <details className={`text-xs ${className}`}>
      <summary className="cursor-pointer select-none font-medium text-content-tertiary hover:text-content-secondary">
        {label}
      </summary>
      <p className="mt-1 max-h-32 overflow-y-auto whitespace-pre-wrap break-words font-mono text-content-tertiary select-text">
        {details}
      </p>
    </details>
  );
}
