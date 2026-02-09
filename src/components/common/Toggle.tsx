/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Platform-adaptive toggle switch component.
 * Renders a sliding toggle (similar to iOS/macOS switches) for boolean
 * settings. Includes an optional label and description text.
 */

interface ToggleProps {
  /** Whether the toggle is currently on */
  checked: boolean;
  /** Callback fired when the toggle state changes */
  onChange: (checked: boolean) => void;
  /** Label text displayed next to the toggle */
  label?: string;
  /** Description text displayed below the label */
  description?: string;
  /** Whether the toggle is disabled */
  disabled?: boolean;
}

/**
 * Renders a sliding toggle switch with optional label and description.
 * The toggle uses the accent color when on and a neutral border when off.
 *
 * @param checked - Current on/off state
 * @param onChange - Callback receiving the new boolean state
 * @param label - Text label to the left of the toggle
 * @param description - Smaller description text below the label
 * @param disabled - Whether to disable interaction
 */
export function Toggle({
  checked,
  onChange,
  label,
  description,
  disabled = false,
}: ToggleProps) {
  return (
    <label
      className={`
        flex items-start gap-3
        ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
      `}
    >
      {/* Label and description text (left side) */}
      {(label || description) && (
        <div className="flex-1 min-w-0">
          {label && (
            <span className="block text-sm font-medium text-content-primary">
              {label}
            </span>
          )}
          {description && (
            <span className="block text-xs text-content-tertiary mt-0.5">
              {description}
            </span>
          )}
        </div>
      )}

      {/* Toggle track and thumb */}
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => !disabled && onChange(!checked)}
        className={`
          relative inline-flex h-6 w-11 flex-shrink-0
          rounded-full border-2 border-transparent
          transition-colors duration-200 ease-in-out
          focus:ring-2 focus:ring-accent focus:ring-offset-2
          ${checked ? 'bg-accent' : 'bg-border'}
          ${disabled ? 'cursor-not-allowed' : 'cursor-pointer'}
        `}
      >
        {/* Sliding thumb circle */}
        <span
          className={`
            pointer-events-none inline-block h-5 w-5
            rounded-full bg-white shadow-platform
            transform transition-transform duration-200 ease-in-out
            ${checked ? 'translate-x-5' : 'translate-x-0'}
          `}
        />
      </button>
    </label>
  );
}
