/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Loading spinner component.
 * Renders a spinning circular indicator for async operations.
 * Supports size variants and an optional label.
 */

/** Size presets for the spinner */
type SpinnerSize = 'sm' | 'md' | 'lg';

interface LoadingSpinnerProps {
  /** Size of the spinner (default: 'md') */
  size?: SpinnerSize;
  /** Optional text label displayed below the spinner */
  label?: string;
  /** Additional CSS classes */
  className?: string;
}

/** SVG dimensions for each size preset */
const SIZE_MAP: Record<SpinnerSize, number> = {
  sm: 16,
  md: 24,
  lg: 40,
};

/**
 * Renders a circular spinning indicator for loading states.
 *
 * @param size - Spinner diameter preset (default: 'md')
 * @param label - Optional text below the spinner
 */
export function LoadingSpinner({
  size = 'md',
  label,
  className = '',
}: LoadingSpinnerProps) {
  const dimension = SIZE_MAP[size];

  return (
    <div className={`flex flex-col items-center gap-2 ${className}`}>
      <svg
        className="animate-spin text-accent"
        width={dimension}
        height={dimension}
        viewBox="0 0 24 24"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        {/* Background track circle */}
        <circle
          className="opacity-25"
          cx="12"
          cy="12"
          r="10"
          stroke="currentColor"
          strokeWidth="4"
        />
        {/* Spinning arc */}
        <path
          className="opacity-75"
          fill="currentColor"
          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
        />
      </svg>

      {/* Optional label text */}
      {label && (
        <span className="text-sm text-content-secondary">{label}</span>
      )}
    </div>
  );
}
