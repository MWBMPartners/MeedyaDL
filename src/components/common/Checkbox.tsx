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

type CheckboxProps = Omit<InputHTMLAttributes<HTMLInputElement>, 'type' | 'onChange'> & {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** Smaller text under the label. */
  description?: ReactNode;
} & ({ label: ReactNode; 'aria-label'?: string } | { label?: undefined; 'aria-label': string });

export function Checkbox({ checked, onChange, label, description, disabled, className = '', ...rest }: CheckboxProps) {
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
  if (!label) return <span className={`inline-flex items-center ${className}`}>{box}</span>;
  return (
    <label className={`flex items-start gap-2 text-sm text-content-secondary select-none ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'} ${className}`}>
      {box}
      <span className="min-w-0">
        {label}
        {description && <span className="block text-xs text-content-tertiary mt-0.5">{description}</span>}
      </span>
    </label>
  );
}
