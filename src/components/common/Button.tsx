/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Platform-adaptive button component.
 * Supports primary (accent), secondary (outlined), ghost (text-only),
 * and danger variants. Adapts to the active platform theme via
 * CSS custom properties for border radius, colors, and transitions.
 */

import type { ButtonHTMLAttributes, ReactNode } from 'react';

/** Visual style variants for the button */
type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';

/** Size presets for the button */
type ButtonSize = 'sm' | 'md' | 'lg';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual variant (primary = accent color, secondary = outlined, ghost = text only) */
  variant?: ButtonVariant;
  /** Size preset controlling padding and font size */
  size?: ButtonSize;
  /** Optional icon element rendered before the label */
  icon?: ReactNode;
  /** Whether to show a loading spinner and disable the button */
  loading?: boolean;
  /** Whether the button should fill its container width */
  fullWidth?: boolean;
}

/** Tailwind classes for each button variant */
const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    'bg-accent text-content-inverse hover:bg-accent-hover active:opacity-90 border-transparent',
  secondary:
    'bg-transparent text-content-primary border-border hover:bg-surface-secondary active:bg-surface-elevated',
  ghost:
    'bg-transparent text-content-secondary hover:text-content-primary hover:bg-surface-secondary border-transparent',
  danger:
    'bg-status-error text-white hover:opacity-90 active:opacity-80 border-transparent',
};

/** Tailwind classes for each button size */
const SIZE_CLASSES: Record<ButtonSize, string> = {
  sm: 'px-2.5 py-1 text-xs gap-1.5',
  md: 'px-4 py-2 text-sm gap-2',
  lg: 'px-5 py-2.5 text-base gap-2.5',
};

/**
 * Renders a platform-adaptive button with support for loading state,
 * icons, and multiple visual variants.
 *
 * @param variant - Visual style variant (default: 'primary')
 * @param size - Size preset (default: 'md')
 * @param icon - Optional leading icon element
 * @param loading - Whether to show loading state
 * @param fullWidth - Whether to stretch to full container width
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
      className={`
        inline-flex items-center justify-center font-medium
        rounded-platform border transition-colors
        ${VARIANT_CLASSES[variant]}
        ${SIZE_CLASSES[size]}
        ${fullWidth ? 'w-full' : ''}
        ${disabled || loading ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
        ${className}
      `}
      disabled={disabled || loading}
      {...props}
    >
      {/* Loading spinner replaces the icon when loading */}
      {loading ? (
        <svg
          className="animate-spin h-4 w-4"
          xmlns="http://www.w3.org/2000/svg"
          fill="none"
          viewBox="0 0 24 24"
        >
          <circle
            className="opacity-25"
            cx="12"
            cy="12"
            r="10"
            stroke="currentColor"
            strokeWidth="4"
          />
          <path
            className="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
          />
        </svg>
      ) : icon ? (
        <span className="flex-shrink-0">{icon}</span>
      ) : null}
      {children}
    </button>
  );
}
