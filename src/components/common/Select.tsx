// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file Platform-adaptive select dropdown component.
 *
 * Wraps the native HTML <select> element with the same visual treatment
 * (label, description, error state) used by the {@link Input} component to
 * maintain a consistent form design language. Using the native <select>
 * ensures the dropdown uses the OS-native popup on every platform (macOS
 * popover, Windows dropdown, etc.) for the best user experience.
 *
 * The component also exports the {@link SelectOption} TypeScript interface
 * so that consumers can strongly type their option arrays.
 *
 * **Usage across the application:**
 * - DownloadForm: media-type and codec selectors.
 * - AdvancedTab, QualityTab, CoverArtTab: quality / format settings.
 * - GeneralTab: default output-format selector.
 * - LyricsTab: lyrics-format selector.
 *
 * @see https://react.dev/reference/react-dom/components/select
 *      React docs -- the native <select> element, controlled vs. uncontrolled.
 * @see https://tailwindcss.com/docs/hover-focus-and-other-states#focus
 *      Tailwind CSS docs -- focus state modifiers.
 */

import type { SelectHTMLAttributes } from 'react';

/**
 * Describes a single option in the select dropdown.
 *
 * This interface is re-exported from the barrel file (index.ts) as a
 * type-only export so consumers can use it to build option arrays:
 *
 * ```ts
 * import type { SelectOption } from '@/components/common';
 * const codecs: SelectOption[] = [
 *   { value: 'aac',  label: 'AAC' },
 *   { value: 'alac', label: 'ALAC' },
 * ];
 * ```
 */
export interface SelectOption {
  /** The value string emitted by the <select> onChange event */
  value: string;
  /** Human-readable display text shown in the dropdown list */
  label: string;
  /** When true, the option is visible but not selectable (greyed out) */
  disabled?: boolean;
}

/**
 * Props accepted by the {@link Select} component.
 *
 * Extends the native <select> HTML attributes minus `children` (since option
 * elements are generated internally from the {@link options} array).
 * Standard props like `value`, `onChange`, `disabled`, `name`, and all
 * `aria-*` / `data-*` attributes are forwarded to the underlying <select>.
 */
interface SelectProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, 'children'> {
  /** Array of options to render inside the dropdown */
  options: SelectOption[];

  /**
   * Label text displayed above the select. When provided, a <label>
   * element is rendered and associated via htmlFor / id for accessibility.
   */
  label?: string;

  /**
   * Small helper text displayed below the select in muted colour.
   * Hidden when {@link error} is set (error takes visual priority).
   */
  description?: string;

  /**
   * Error message string. When set, the select border turns red and
   * this message replaces the description text.
   */
  error?: string;

  /**
   * Placeholder text shown as a disabled <option> at the top of the list
   * when no value has been selected yet. E.g. "Choose a codec..."
   */
  placeholder?: string;
}

/**
 * Renders a platform-adaptive select dropdown using the native <select>
 * element for the best OS integration (native dropdowns on macOS, Windows, etc.).
 *
 * The visual treatment (label, description, error) mirrors the {@link Input}
 * component so all form controls have a consistent look and feel.
 *
 * @example
 * ```tsx
 * <Select
 *   label="Codec"
 *   options={[
 *     { value: 'aac',  label: 'AAC' },
 *     { value: 'alac', label: 'ALAC', disabled: true },
 *   ]}
 *   placeholder="Choose a codec..."
 *   value={codec}
 *   onChange={(e) => setCodec(e.target.value)}
 * />
 * ```
 *
 * @param options     - Array of { value, label, disabled? } objects
 * @param label       - Optional label above the select
 * @param description - Optional helper text below
 * @param error       - Optional error message (overrides description)
 * @param placeholder - Text shown as a disabled first option
 * @param className   - Additional Tailwind classes merged onto the <select>
 * @param id          - Explicit DOM id; auto-generated from label when omitted
 * @param props       - All remaining native <select> attributes
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
  /*
   * Auto-generate a DOM id from the label text (same pattern as Input).
   * This ensures the <label htmlFor> matches the <select id> for
   * accessibility without requiring the consumer to manage ids manually.
   */
  const selectId = id || (label ? `select-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);

  return (
    /* Outer wrapper -- space-y-1.5 adds 6px vertical gap between children */
    <div className="space-y-1.5">
      {/* Accessible <label> -- only rendered when label text is provided */}
      {label && (
        <label
          htmlFor={selectId}
          className="block text-sm font-medium text-content-primary"
        >
          {label}
        </label>
      )}

      {/*
       * Native <select> element.
       *
       * Class composition mirrors the Input component:
       * 1. Base        -- full-width, padding, small text
       * 2. Platform    -- rounded-platform border radius from CSS var
       * 3. Colours     -- surface-secondary bg, themed text
       * 4. Focus ring  -- accent-coloured border + ring on focus
       * 5. Disabled    -- reduced opacity and not-allowed cursor
       * 6. Error state -- red border when error is set
       * 7. Consumer    -- extra classes via className
       *
       * @see https://tailwindcss.com/docs/ring-width -- focus ring utilities
       */}
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
        /* Forward all remaining native <select> attributes */
        {...props}
      >
        {/*
         * Placeholder option -- rendered as a disabled <option> with an
         * empty value. The browser shows this text when no real option
         * is selected (i.e. the <select> value is "").
         */}
        {placeholder && (
          <option value="" disabled>
            {placeholder}
          </option>
        )}

        {/*
         * Map over the options array to render each <option>.
         * Uses option.value as the React key since values must be unique
         * within a single <select>.
         */}
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

      {/* Error message -- takes priority over description */}
      {error && (
        <p className="text-xs text-status-error">{error}</p>
      )}

      {/* Helper description text -- only shown when there is no error */}
      {!error && description && (
        <p className="text-xs text-content-tertiary">{description}</p>
      )}
    </div>
  );
}
