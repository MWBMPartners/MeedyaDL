// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file Platform-adaptive button component.
 *
 * Provides a reusable, theme-aware <button> element with four visual variants:
 *   - **primary** -- filled with the accent colour (for main CTAs)
 *   - **secondary** -- outlined / transparent (for secondary actions)
 *   - **ghost** -- text-only with no border (for tertiary / toolbar actions)
 *   - **danger** -- red / destructive-action styling
 *
 * The component adapts to the active platform theme through CSS custom
 * properties (`--radius-platform`, `--color-accent`, etc.) which are defined
 * in the global stylesheet and toggled by the platform detector.
 *
 * **Usage across the application:**
 * - DownloadForm: primary "Download" CTA, secondary "Clear" action.
 * - DownloadQueue: primary "Start All" / secondary "Clear" buttons.
 * - SettingsPage, SetupWizard steps: navigation and action buttons.
 * - UpdateBanner: primary "Upgrade" button for GAMDL updates.
 * - FilePickerButton (internal): secondary "Browse" button.
 * - CookiesTab, FallbackTab: various action buttons.
 *
 * @see https://react.dev/reference/react-dom/components/button
 *      React docs -- the native <button> element.
 * @see https://tailwindcss.com/docs/hover-focus-and-other-states
 *      Tailwind CSS docs -- hover, focus, active, and disabled state modifiers.
 */

import type { ButtonHTMLAttributes, ReactNode } from 'react';

/**
 * Visual style variants for the button.
 * Each variant maps to a distinct set of Tailwind utility classes in
 * {@link VARIANT_CLASSES}. The "primary" variant uses the global accent colour,
 * while "secondary" is outlined, "ghost" is borderless, and "danger" signals
 * destructive actions.
 */
type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';

/**
 * Size presets controlling padding, font size, and icon-to-text gap.
 * Each size maps to a set of Tailwind utility classes in {@link SIZE_CLASSES}.
 * - sm: compact (used in tags, inline actions, UpdateBanner upgrade button)
 * - md: default (most form buttons)
 * - lg: prominent (large CTAs)
 */
type ButtonSize = 'sm' | 'md' | 'lg';

/**
 * Props accepted by the {@link Button} component.
 *
 * Extends the native HTML button attributes so that all standard props
 * (onClick, type, aria-*, data-*, etc.) are forwarded to the underlying
 * <button> element via the rest-spread operator.
 *
 * @see https://react.dev/reference/react-dom/components/common -- common React DOM props
 */
interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /**
   * Visual style variant. Defaults to 'primary'.
   * - primary: solid accent-coloured background, white/inverse text.
   * - secondary: transparent background, border, and themed text.
   * - ghost: no border or background; text-only with hover highlight.
   * - danger: solid red background for destructive actions.
   */
  variant?: ButtonVariant;

  /**
   * Size preset that controls horizontal/vertical padding, font size, and
   * the gap between the icon and the label text. Defaults to 'md'.
   */
  size?: ButtonSize;

  /**
   * Optional leading icon (ReactNode) rendered to the left of the children.
   * Typically a Lucide icon component, e.g. <RefreshCw size={14} />.
   * When {@link loading} is true the icon is replaced by the spinner.
   *
   * @see https://lucide.dev/guide/packages/lucide-react -- Lucide React icon usage
   */
  icon?: ReactNode;

  /**
   * When true, replaces the icon with an animated SVG spinner and sets
   * the native `disabled` attribute to prevent duplicate submissions.
   */
  loading?: boolean;

  /**
   * When true, applies `w-full` so the button stretches to fill its
   * parent container's width (useful in stacked / card layouts).
   */
  fullWidth?: boolean;
}

/**
 * Tailwind utility class strings for each button variant.
 *
 * These classes reference the project's custom design-token colours
 * (bg-accent, text-content-inverse, etc.) defined in tailwind.config.ts.
 * Hover and active states are handled via Tailwind state modifiers.
 *
 * @see https://tailwindcss.com/docs/hover-focus-and-other-states -- state modifiers
 * @see https://tailwindcss.com/docs/customizing-colors -- custom colour tokens
 */
const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  /** Solid accent background, inverse (white) text; lightens on hover */
  primary:
    'bg-accent text-content-inverse hover:bg-accent-hover active:opacity-90 border-transparent',
  /** Transparent with a visible border; subtle surface colour on hover */
  secondary:
    'bg-transparent text-content-primary border-border hover:bg-surface-secondary active:bg-surface-elevated',
  /** No border or background; text dims to secondary and highlights on hover */
  ghost:
    'bg-transparent text-content-secondary hover:text-content-primary hover:bg-surface-secondary border-transparent',
  /** Red/error background for destructive actions; slightly fades on hover/active */
  danger:
    'bg-status-error text-white hover:opacity-90 active:opacity-80 border-transparent',
};

/**
 * Tailwind utility class strings for each button size.
 * Controls horizontal padding (px-*), vertical padding (py-*), font size
 * (text-*), and the flexbox gap between the icon and label text (gap-*).
 */
const SIZE_CLASSES: Record<ButtonSize, string> = {
  /** Compact: 10px horizontal, 4px vertical, 12px font, 6px gap */
  sm: 'px-2.5 py-1 text-xs gap-1.5',
  /** Default: 16px horizontal, 8px vertical, 14px font, 8px gap */
  md: 'px-4 py-2 text-sm gap-2',
  /** Large: 20px horizontal, 10px vertical, 16px font, 10px gap */
  lg: 'px-5 py-2.5 text-base gap-2.5',
};

/**
 * Renders a platform-adaptive button with support for loading state,
 * icons, and multiple visual variants.
 *
 * The component uses Tailwind CSS template-literal class composition to
 * merge base styles, variant styles, size styles, and any additional
 * `className` passed by the consumer.
 *
 * **Accessibility notes:**
 * - The native `disabled` attribute is set when loading or explicitly disabled,
 *   ensuring the button is skipped by keyboard navigation and announced as
 *   disabled by screen readers.
 * - All remaining HTML button attributes (aria-label, type, etc.) are
 *   forwarded via the rest-spread.
 *
 * @example
 * ```tsx
 * <Button variant="primary" icon={<RefreshCw size={14} />} loading={isUpgrading}>
 *   Upgrade
 * </Button>
 * ```
 *
 * @param variant  - Visual style variant (default: 'primary')
 * @param size     - Size preset (default: 'md')
 * @param icon     - Optional leading icon element
 * @param loading  - Whether to show the inline spinner and disable interaction
 * @param fullWidth - Whether to stretch to the full container width
 * @param disabled - Explicitly disable the button
 * @param children - Button label / content
 * @param className - Additional Tailwind classes merged at the end
 * @param props    - All remaining native <button> attributes
 */
export function Button({
  variant = 'primary',
  size = 'md',
  icon,
  loading = false,
  fullWidth = false,
  disabled,
  children,
  className = '',
  ...props
}: ButtonProps) {
  return (
    <button
      /* ------------------------------------------------------------------ *
       * Class composition:                                                  *
       * 1. Base layout  -- inline-flex, centered, font-medium               *
       * 2. Platform      -- rounded-platform uses a CSS var for the OS      *
       * 3. Variant       -- colour / background from VARIANT_CLASSES        *
       * 4. Size          -- padding / font from SIZE_CLASSES                *
       * 5. Width         -- optional w-full for fullWidth mode              *
       * 6. State         -- opacity-50 + cursor-not-allowed when disabled   *
       * 7. Consumer      -- any extra classes passed via className prop     *
       * ------------------------------------------------------------------ */
      className={`
        inline-flex items-center justify-center font-medium
        rounded-platform border transition-colors
        ${VARIANT_CLASSES[variant]}
        ${SIZE_CLASSES[size]}
        ${fullWidth ? 'w-full' : ''}
        ${disabled || loading ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
        ${className}
      `}
      /* Disable the native button when loading OR explicitly disabled */
      disabled={disabled || loading}
      /* Forward all remaining native button attributes (onClick, type, aria-*, etc.) */
      {...props}
    >
      {/*
       * Icon / spinner slot:
       * - When loading === true: render an animated SVG spinner that matches
       *   the button's text colour via currentColor.
       * - When loading === false and an icon is provided: render the icon
       *   wrapped in a flex-shrink-0 span so it keeps its intrinsic size.
       * - Otherwise: render nothing in this slot.
       */}
      {loading ? (
        /* Inline SVG spinner -- uses Tailwind's animate-spin for the rotation.
         * The circle (opacity-25) provides a faint track, and the arc
         * (opacity-75) is the visible spinning segment.
         * @see https://tailwindcss.com/docs/animation#spin -- Tailwind spin animation */
        <svg
          className="animate-spin h-4 w-4"
          xmlns="http://www.w3.org/2000/svg"
          fill="none"
          viewBox="0 0 24 24"
        >
          {/* Faint background track circle */}
          <circle
            className="opacity-25"
            cx="12"
            cy="12"
            r="10"
            stroke="currentColor"
            strokeWidth="4"
          />
          {/* Visible spinning arc segment */}
          <path
            className="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
          />
        </svg>
      ) : icon ? (
        /* Icon wrapper -- flex-shrink-0 prevents the icon from being
         * squished when the button text is long or the container is narrow */
        <span className="flex-shrink-0">{icon}</span>
      ) : null}

      {/* Button label / child content */}
      {children}
    </button>
  );
}
