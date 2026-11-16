// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file A single checkbox with its label.
 *
 * Polish pass (M17): checkboxes were hand-made in six places at three
 * sizes, some tinted with the accent colour and some left as the plain
 * browser box. One component now (CheckboxGroup, for a set of choices,
 * draws each box the same way).
 *
 * With `label`, the whole line is clickable. Without one (a row's
 * "select this item" box), `aria-label` is required so the box still has a
 * name.
 */

import type { InputHTMLAttributes, ReactNode } from 'react';

/**
 * Props for `Checkbox`: every normal `<input>` attribute except `type` and
 * `onChange` (those two are replaced by the simpler versions below).
 *
 * The last line is a union so the compiler enforces the accessibility rule:
 * either give a visible `label`, or give an `aria-label`. A box with neither
 * would have no name for a screen reader.
 * See: https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input/checkbox
 */
type CheckboxProps = Omit<InputHTMLAttributes<HTMLInputElement>, 'type' | 'onChange'> & {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** Smaller text under the label. */
  description?: ReactNode;
} & ({ label: ReactNode; 'aria-label'?: string } | { label?: undefined; 'aria-label': string });

/**
 * A controlled checkbox. The parent owns `checked` and hears about changes
 * through `onChange(newValue)`, so the caller never has to read the event.
 *
 * With a `label` it renders a `<label>` around the box, which makes the
 * whole line clickable. Without one it renders just the box in a `<span>`.
 * Any extra props (`id`, `aria-describedby`, ...) go straight to the input.
 * See: https://react.dev/reference/react-dom/components/input#controlling-an-input-with-a-state-variable
 */
export function Checkbox({
  checked,
  onChange,
  label,
  description,
  disabled,
  className = '',
  ...rest
}: CheckboxProps) {
  // The box is built once and placed either bare or inside the label below.
  const box = (
    <input
      type="checkbox"
      checked={checked}
      disabled={disabled}
      onChange={(e) => onChange(e.target.checked)}
      className={`h-4 w-4 flex-shrink-0 cursor-pointer accent-accent disabled:cursor-not-allowed ${label ? 'mt-0.5' : ''}`}
      {...rest}
    />
  );
  // No label: no wrapper text, so the caller's `aria-label` is the box's only name.
  if (!label) return <span className={`inline-flex items-center ${className}`}>{box}</span>;
  return (
    <label
      className={`flex items-start gap-2 text-sm text-content-secondary select-none ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'} ${className}`}
    >
      {box}
      <span className="min-w-0">
        {label}
        {description && (
          <span className="block text-xs text-content-tertiary mt-0.5">{description}</span>
        )}
      </span>
    </label>
  );
}
