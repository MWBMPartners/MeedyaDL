/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Platform-adaptive text input component.
 * Supports labels, descriptions, error messages, and optional icons.
 * Uses the platform theme's border radius, colors, and focus styles.
 */

import { forwardRef, type InputHTMLAttributes, type ReactNode } from 'react';

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /** Label text displayed above the input */
  label?: string;
  /** Helper text displayed below the input */
  description?: string;
  /** Error message displayed below the input (overrides description) */
  error?: string;
  /** Optional icon element rendered inside the input's left side */
  icon?: ReactNode;
  /** Optional element rendered inside the input's right side */
  suffix?: ReactNode;
}

/**
 * Renders a platform-adaptive text input with optional label,
 * description, error state, and icon decorations.
 *
 * Uses forwardRef so parent components can access the underlying
 * <input> element for focus management.
 */
export const Input = forwardRef<HTMLInputElement, InputProps>(
  function Input(
    { label, description, error, icon, suffix, className = '', id, ...props },
    ref,
  ) {
    /* Generate a stable ID if none provided, for label association */
    const inputId = id || (label ? `input-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);

    return (
      <div className="space-y-1.5">
        {/* Label */}
        {label && (
          <label
            htmlFor={inputId}
            className="block text-sm font-medium text-content-primary"
          >
            {label}
          </label>
        )}

        {/* Input wrapper for icon/suffix positioning */}
        <div className="relative">
          {/* Leading icon */}
          {icon && (
            <div className="absolute left-3 top-1/2 -translate-y-1/2 text-content-tertiary">
              {icon}
            </div>
          )}

          <input
            ref={ref}
            id={inputId}
            className={`
              w-full px-3 py-2 text-sm
              rounded-platform border
              bg-surface-secondary text-content-primary
              placeholder-content-tertiary
              transition-colors
              focus:border-accent focus:ring-1 focus:ring-accent
              disabled:opacity-50 disabled:cursor-not-allowed
              ${icon ? 'pl-9' : ''}
              ${suffix ? 'pr-9' : ''}
              ${error ? 'border-status-error focus:border-status-error focus:ring-status-error' : 'border-border'}
              ${className}
            `}
            {...props}
          />

          {/* Trailing suffix element */}
          {suffix && (
            <div className="absolute right-3 top-1/2 -translate-y-1/2 text-content-tertiary">
              {suffix}
            </div>
          )}
        </div>

        {/* Error message (takes priority over description) */}
        {error && (
          <p className="text-xs text-status-error">{error}</p>
        )}

        {/* Description text */}
        {!error && description && (
          <p className="text-xs text-content-tertiary">{description}</p>
        )}
      </div>
    );
  },
);
