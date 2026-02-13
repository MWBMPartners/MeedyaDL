// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file Progress bar component.
 *
 * Renders a horizontal progress indicator with two modes:
 * - **Determinate** (value = 0-100): a filled bar whose width represents the
 *   completion percentage. The width transitions smoothly via CSS.
 * - **Indeterminate** (value = null): an animated sliding bar that bounces
 *   back and forth, indicating "working but unknown progress".
 *
 * **Usage across the application:**
 * - QueueItem: shows per-track download progress in the download queue.
 *
 * @see https://tailwindcss.com/docs/width -- Tailwind width utilities
 * @see https://tailwindcss.com/docs/animation -- Tailwind animation utilities
 * @see https://tailwindcss.com/docs/transition-property -- CSS transitions
 */

/**
 * Props accepted by the {@link ProgressBar} component.
 */
interface ProgressBarProps {
  /**
   * Current progress percentage (0-100).
   * - When a number: the bar fills to that percentage (clamped to 0-100).
   * - When null: the bar enters indeterminate mode (animated sliding bar).
   */
  value: number | null;

  /**
   * Optional label text displayed above the bar. When provided in
   * determinate mode, the rounded percentage is shown on the right side.
   */
  label?: string;

  /** Additional Tailwind classes merged onto the outer container div */
  className?: string;
}

/**
 * Renders a horizontal progress bar.
 *
 * **Determinate mode** (value is a number):
 * - The filled bar width is set via an inline `style={{ width }}`.
 * - `transition-all duration-300 ease-out` smoothly animates width changes
 *   so the bar does not jump when the percentage updates.
 * - The value is clamped to [0, 100] to prevent visual overflow.
 *
 * **Indeterminate mode** (value is null):
 * - A 1/3-width bar slides back and forth via a CSS `@keyframes` animation.
 * - The animation name `indeterminate` must be defined in the project's
 *   global stylesheet or Tailwind config.
 *
 * @example
 * ```tsx
 * // Determinate: 45% complete
 * <ProgressBar value={45} label="Downloading" />
 *
 * // Indeterminate: unknown progress
 * <ProgressBar value={null} label="Preparing..." />
 * ```
 *
 * @param value     - Progress percentage (0-100), or null for indeterminate
 * @param label     - Optional text displayed above the bar
 * @param className - Additional Tailwind classes for the outer container
 */
export function ProgressBar({
  value,
  label,
  className = '',
}: ProgressBarProps) {
  /** Boolean flag for mode selection */
  const isIndeterminate = value === null;

  /**
   * Clamp the value to [0, 100] to prevent the bar from exceeding its track.
   * Math.max(0, ...) guards against negative values.
   * Math.min(100, ...) guards against values > 100.
   * Defaults to 0 when indeterminate (value === null) since this variable
   * is not used in that branch, but avoids a nullable type.
   */
  const clampedValue = value !== null ? Math.max(0, Math.min(100, value)) : 0;

  return (
    /* Outer container -- space-y-1 adds 4px gap between label row and bar */
    <div className={`space-y-1 ${className}`}>
      {/*
       * Label row -- only rendered when a label is provided.
       * In determinate mode, the rounded percentage is shown on the right.
       * Uses justify-between to push the label left and percentage right.
       */}
      {label && (
        <div className="flex justify-between text-xs text-content-secondary">
          <span>{label}</span>
          {!isIndeterminate && <span>{Math.round(clampedValue)}%</span>}
        </div>
      )}

      {/*
       * Progress track -- the grey background "rail" for the bar.
       * - h-2: 8px tall.
       * - rounded-full: fully rounded (pill shape).
       * - overflow-hidden: clips the filled bar to the track bounds.
       */}
      <div className="h-2 w-full rounded-full bg-surface-elevated overflow-hidden">
        {isIndeterminate ? (
          /*
           * Indeterminate bar -- 1/3 of the track width, continuously
           * sliding via the `indeterminate` @keyframes animation.
           * The animation is declared both as a Tailwind arbitrary value
           * `animate-[indeterminate_1.5s_ease-in-out_infinite]` and as
           * an inline style for maximum compatibility.
           */
          <div
            className="h-full w-1/3 rounded-full bg-accent animate-[indeterminate_1.5s_ease-in-out_infinite]"
            style={{
              animation: 'indeterminate 1.5s ease-in-out infinite',
            }}
          />
        ) : (
          /*
           * Determinate bar -- width set as an inline percentage.
           * transition-all duration-300 ease-out provides a smooth 300ms
           * animation when the value changes (e.g. from 45% to 50%).
           */
          <div
            className="h-full rounded-full bg-accent transition-all duration-300 ease-out"
            style={{ width: `${clampedValue}%` }}
          />
        )}
      </div>
    </div>
  );
}
