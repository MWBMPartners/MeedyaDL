// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Multi-select checkbox group component.
// Renders a list of labeled checkboxes for selecting multiple options
// from a set. Used by the Quality settings tab for custom companion
// codecs and multi-select artist auto-select modes.

import { HelpButton } from './HelpButton';

interface CheckboxGroupProps<T extends string> {
  /** Group label displayed above the checkboxes */
  label: string;
  /** Optional description displayed below the label */
  description?: React.ReactNode;
  /** Map of value → display label for each option */
  options: Record<T, string>;
  /** Currently selected values */
  selected: T[];
  /** Callback when selection changes */
  onChange: (selected: T[]) => void;
  /** When true, all checkboxes are disabled */
  disabled?: boolean;
  /** Optional help topic link */
  helpTopic?: string;
  /** Maximum number of columns for the checkbox grid (default: 2) */
  columns?: 1 | 2 | 3;
}

/**
 * CheckboxGroup -- Renders a grid of labeled checkboxes for multi-select.
 *
 * Each checkbox represents one option from the provided `options` record.
 * The `selected` array controls which checkboxes are checked (controlled
 * component pattern). Changes are reported via `onChange` with the
 * updated selection array.
 */
export function CheckboxGroup<T extends string>({
  label,
  description,
  options,
  selected,
  onChange,
  disabled = false,
  helpTopic,
  columns = 2,
}: CheckboxGroupProps<T>) {
  const handleToggle = (value: T) => {
    if (disabled) return;
    if (selected.includes(value)) {
      onChange(selected.filter((v) => v !== value));
    } else {
      onChange([...selected, value]);
    }
  };

  const gridCols =
    columns === 1
      ? 'grid-cols-1'
      : columns === 3
        ? 'grid-cols-1 sm:grid-cols-3'
        : 'grid-cols-1 sm:grid-cols-2';

  return (
    <div className={disabled ? 'opacity-50' : ''}>
      {label && (
        <span className="flex items-center gap-1.5 text-sm font-medium text-content-primary mb-1">
          {label}
          {helpTopic && <HelpButton topic={helpTopic} />}
        </span>
      )}
      {description && (
        <p className="text-xs text-content-tertiary mb-2">{description}</p>
      )}
      <div
        className={`grid ${gridCols} gap-1.5 p-2 bg-surface-secondary rounded-platform`}
      >
        {(Object.entries(options) as [T, string][]).map(([value, optLabel]) => (
          <label
            key={value}
            className={`flex items-center gap-2 px-2 py-1.5 rounded text-sm
              ${disabled ? 'cursor-not-allowed' : 'cursor-pointer hover:bg-surface-tertiary'}
              ${selected.includes(value) ? 'text-content-primary' : 'text-content-secondary'}`}
          >
            <input
              type="checkbox"
              checked={selected.includes(value)}
              onChange={() => handleToggle(value)}
              disabled={disabled}
              className="h-3.5 w-3.5 rounded border-border text-accent focus:ring-accent"
            />
            <span className="truncate">{optLabel}</span>
          </label>
        ))}
      </div>
    </div>
  );
}
