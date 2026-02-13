// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file Loading spinner component.
 *
 * Renders an animated circular spinner to indicate that an asynchronous
 * operation is in progress. The spinner is a pure SVG element animated via
 * Tailwind's `animate-spin` utility class (CSS `@keyframes spin`).
 *
 * Three size presets are available (sm, md, lg), and an optional text label
 * can be displayed below the spinner.
 *
 * **Usage across the application:**
 * - App.tsx: full-screen loading indicator while the app initialises.
 * - GamdlStep, PythonStep, DependenciesStep: inline spinners during
 *   dependency installation / detection in the setup wizard.
 * - Also used internally by the {@link Button} component when its `loading`
 *   prop is true (though Button has its own inline SVG spinner).
 *
 * @see https://tailwindcss.com/docs/animation#spin -- Tailwind spin animation
 */

/**
 * Size preset union type.
 * Each value maps to a pixel dimension in {@link SIZE_MAP}.
 */
type SpinnerSize = 'sm' | 'md' | 'lg';

/**
 * Props accepted by the {@link LoadingSpinner} component.
 */
interface LoadingSpinnerProps {
  /**
   * Spinner diameter preset. Defaults to 'md' (24px).
   * - sm: 16px -- suitable for inline use next to text.
   * - md: 24px -- default, good for section-level loading.
   * - lg: 40px -- full-screen or hero-level loading indicator.
   */
  size?: SpinnerSize;

  /** Optional descriptive text displayed below the spinner (e.g. "Loading...") */
  label?: string;

  /** Additional Tailwind classes merged onto the outer flex container */
  className?: string;
}

/**
 * Maps each size preset to an SVG width/height in pixels.
 * The SVG viewBox is always "0 0 24 24" regardless of the rendered size,
 * so the paths scale smoothly at any dimension.
 */
const SIZE_MAP: Record<SpinnerSize, number> = {
  sm: 16,  // 16px -- compact inline spinner
  md: 24,  // 24px -- default
  lg: 40,  // 40px -- large / hero spinner
};

/**
 * Renders a circular spinning indicator for loading states.
 *
 * **SVG structure:**
 * The SVG contains two overlapping shapes:
 * 1. A full circle (opacity-25) that acts as a faint "track".
 * 2. A quarter-arc path (opacity-75) that is the visible spinning segment.
 * The entire SVG rotates via Tailwind's `animate-spin` (360deg linear infinite).
 *
 * The colour is inherited from `text-accent` via `currentColor`, so the
 * spinner automatically matches the app's accent colour theme.
 *
 * @example
 * ```tsx
 * <LoadingSpinner size="lg" label="Checking for updates..." />
 * ```
 *
 * @param size      - Spinner diameter preset (default: 'md')
 * @param label     - Optional descriptive text below the spinner
 * @param className - Additional Tailwind classes for the outer container
 */
export function LoadingSpinner({
  size = 'md',
  label,
  className = '',
}: LoadingSpinnerProps) {
  /* Resolve the pixel dimension from the size preset */
  const dimension = SIZE_MAP[size];

  return (
    /* Outer container -- centres the spinner and optional label vertically */
    <div className={`flex flex-col items-center gap-2 ${className}`}>
      {/*
       * Animated SVG spinner.
       * - animate-spin: Tailwind's CSS keyframe for continuous 360deg rotation.
       * - text-accent: sets currentColor to the app's accent colour.
       * - viewBox="0 0 24 24" with variable width/height scales the SVG.
       *
       * @see https://tailwindcss.com/docs/animation#spin
       */}
      <svg
        className="animate-spin text-accent"
        width={dimension}
        height={dimension}
        viewBox="0 0 24 24"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        {/*
         * Background track -- a full circle rendered at 25% opacity.
         * Provides a subtle visual track behind the spinning arc.
         * Uses stroke (not fill) so it appears as a ring.
         */}
        <circle
          className="opacity-25"
          cx="12"
          cy="12"
          r="10"
          stroke="currentColor"
          strokeWidth="4"
        />
        {/*
         * Spinning arc -- a quarter-circle filled path at 75% opacity.
         * This is the visually prominent part of the spinner. The SVG
         * path draws an arc from the 12-o'clock position to the 9-o'clock
         * position, and the animate-spin rotation makes it appear to chase
         * around the track.
         */}
        <path
          className="opacity-75"
          fill="currentColor"
          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
        />
      </svg>

      {/* Optional label text displayed below the spinner in muted colour */}
      {label && (
        <span className="text-sm text-content-secondary">{label}</span>
      )}
    </div>
  );
}
