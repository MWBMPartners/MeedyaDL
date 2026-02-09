/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Progress bar component.
 * Renders a horizontal bar indicating completion percentage.
 * Used in the download queue to show track download progress.
 * Supports determinate (percent-based) and indeterminate (animated) modes.
 */

interface ProgressBarProps {
  /** Current progress percentage (0-100). Pass null for indeterminate mode. */
  value: number | null;
  /** Optional label text displayed above the bar */
  label?: string;
  /** Additional CSS classes for the container */
  className?: string;
}

/**
 * Renders a horizontal progress bar.
 * When value is null, shows an indeterminate animated bar.
 * When value is 0-100, shows a determinate filled bar.
 *
 * @param value - Progress percentage (0-100), or null for indeterminate
 * @param label - Optional text above the bar
 */
export function ProgressBar({
  value,
  label,
  className = '',
}: ProgressBarProps) {
  const isIndeterminate = value === null;
  const clampedValue = value !== null ? Math.max(0, Math.min(100, value)) : 0;

  return (
    <div className={`space-y-1 ${className}`}>
      {/* Label and percentage display */}
      {label && (
        <div className="flex justify-between text-xs text-content-secondary">
          <span>{label}</span>
          {!isIndeterminate && <span>{Math.round(clampedValue)}%</span>}
        </div>
      )}

      {/* Progress track */}
      <div className="h-2 w-full rounded-full bg-surface-elevated overflow-hidden">
        {isIndeterminate ? (
          /* Indeterminate: animated sliding bar */
          <div
            className="h-full w-1/3 rounded-full bg-accent animate-[indeterminate_1.5s_ease-in-out_infinite]"
            style={{
              animation: 'indeterminate 1.5s ease-in-out infinite',
            }}
          />
        ) : (
          /* Determinate: filled bar based on percentage */
          <div
            className="h-full rounded-full bg-accent transition-all duration-300 ease-out"
            style={{ width: `${clampedValue}%` }}
          />
        )}
      </div>
    </div>
  );
}
