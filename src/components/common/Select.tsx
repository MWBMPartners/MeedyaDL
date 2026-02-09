/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Platform-adaptive select dropdown component.
 * Wraps the native <select> element with consistent styling, labels,
 * and error states. Uses the native dropdown for maximum OS integration.
 */

import type { SelectHTMLAttributes } from 'react';

/** A single option in the select dropdown */
export interface SelectOption {
  /** The value sent to the onChange handler */
  value: string;
  /** The display text shown in the dropdown */
  label: string;
  /** Whether this option is disabled */
  disabled?: boolean;
}

interface SelectProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, 'children'> {
  /** Available options to choose from */
  options: SelectOption[];
  /** Label text displayed above the select */
  label?: string;
  /** Helper text displayed below the select */
  description?: string;
  /** Error message displayed below the select */
  error?: string;
  /** Placeholder text when no value is selected */
  placeholder?: string;
}

/**
 * Renders a platform-adaptive select dropdown using the native <select>
 * element for the best OS integration (native dropdowns on macOS, Windows, etc.).
 *
 * @param options - Array of { value, label, disabled? } objects
 * @param label - Optional label above the select
 * @param description - Optional helper text below
 * @param error - Optional error message (overrides description)
 * @param placeholder - Text shown when no value is selected
 */
export function Select({
  options,
  label,
  description,
  error,
  placeholder,
  className = '',
  id,
  ...props
}: SelectProps) {
  const selectId = id || (label ? `select-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);

  return (
    <div className="space-y-1.5">
      {/* Label */}
      {label && (
        <label
          htmlFor={selectId}
          className="block text-sm font-medium text-content-primary"
        >
          {label}
        </label>
      )}

      <select
        id={selectId}
        className={`
          w-full px-3 py-2 text-sm
          rounded-platform border
          bg-surface-secondary text-content-primary
          transition-colors
          focus:border-accent focus:ring-1 focus:ring-accent
          disabled:opacity-50 disabled:cursor-not-allowed
          ${error ? 'border-status-error' : 'border-border'}
          ${className}
        `}
        {...props}
      >
        {/* Placeholder option (disabled, shown when no value selected) */}
        {placeholder && (
          <option value="" disabled>
            {placeholder}
          </option>
        )}

        {/* Render each option */}
        {options.map((option) => (
          <option
            key={option.value}
            value={option.value}
            disabled={option.disabled}
          >
            {option.label}
          </option>
        ))}
      </select>

      {/* Error message */}
      {error && (
        <p className="text-xs text-status-error">{error}</p>
      )}

      {/* Description text */}
      {!error && description && (
        <p className="text-xs text-content-tertiary">{description}</p>
      )}
    </div>
  );
}
