// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file Platform-adaptive text input component.
 *
 * Wraps the native HTML <input> element with a consistent visual style that
 * adapts to the current OS theme via CSS custom properties. Features:
 *   - Optional label (auto-associated via htmlFor / id)
 *   - Helper description text below the input
 *   - Error state with red border + error message (overrides description)
 *   - Leading icon (e.g. search magnifier)
 *   - Trailing suffix (e.g. unit label, clear button)
 *
 * The component uses `React.forwardRef` so that parent components can
 * imperatively call `.focus()` or read `.value` on the underlying <input>.
 *
 * **Usage across the application:**
 * - AdvancedTab, QualityTab, CoverArtTab: numeric / text settings fields.
 * - TemplatesTab: filename template string inputs.
 *
 * @see https://react.dev/reference/react-dom/components/input
 *      React docs -- the <input> element and controlled vs. uncontrolled inputs.
 * @see https://react.dev/reference/react/forwardRef
 *      React docs -- forwarding refs to DOM elements.
 * @see https://tailwindcss.com/docs/hover-focus-and-other-states#focus
 *      Tailwind CSS docs -- focus ring and focus-within utilities.
 */

import { forwardRef, type InputHTMLAttributes, type ReactNode } from 'react';

/**
 * Props accepted by the {@link Input} component.
 *
 * Extends all native <input> HTML attributes (value, onChange, placeholder,
 * type, min, max, etc.) so they can be forwarded directly to the DOM element.
 *
 * @see https://react.dev/reference/react-dom/components/common -- common React DOM props
 */
interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /**
   * Label text displayed above the input. When provided, a <label> element
   * is rendered and automatically associated with the input via matching
   * htmlFor / id attributes for accessibility.
   */
  label?: string;

  /**
   * Small helper text displayed below the input in a muted colour.
   * Hidden when {@link error} is set (error takes visual priority).
   */
  description?: string;

  /**
   * Error message string. When set:
   * - The input border turns red (border-status-error).
   * - The focus ring also turns red.
   * - This message replaces the description text below the input.
   */
  error?: string;

  /**
   * Optional leading icon rendered inside the input on the left side.
   * Adds left padding (pl-9) to the <input> so text does not overlap the icon.
   * Typically a Lucide icon at size={16}.
   * @see https://lucide.dev/guide/packages/lucide-react
   */
  icon?: ReactNode;

  /**
   * Optional trailing element rendered inside the input on the right side.
   * Adds right padding (pr-9) to the <input> so text does not overlap.
   * Useful for unit labels (e.g. "px"), clear buttons, or toggle visibility.
   */
  suffix?: ReactNode;
}

/**
 * Renders a platform-adaptive text input with optional label,
 * description, error state, and icon decorations.
 *
 * Uses `React.forwardRef` so parent components can imperatively access the
 * underlying <input> DOM node (e.g. to call `.focus()` or `.select()`).
 *
 * The inner function is named `Input` (rather than anonymous) so that
 * React DevTools display a meaningful component name.
 *
 * @example
 * ```tsx
 * <Input
 *   label="Filename Template"
 *   placeholder="{title} - {artist}"
 *   description="Supports {title}, {artist}, {album} tokens"
 *   error={templateError}
 *   value={template}
 *   onChange={(e) => setTemplate(e.target.value)}
 * />
 * ```
 *
 * @see https://react.dev/reference/react/forwardRef -- forwardRef pattern
 */
export const Input = forwardRef<HTMLInputElement, InputProps>(
  function Input(
    { label, description, error, icon, suffix, className = '', id, ...props },
    ref,
  ) {
    /*
     * Generate a deterministic DOM id from the label text when the consumer
     * does not supply one. This id is shared between the <label htmlFor> and
     * the <input id> to satisfy accessibility requirements (clicking the label
     * focuses the input; screen readers announce the label for the input).
     *
     * The regex replaces whitespace runs with hyphens to produce a valid
     * HTML id attribute value, e.g. "Filename Template" -> "input-filename-template".
     */
    const inputId = id || (label ? `input-${label.toLowerCase().replace(/\s+/g, '-')}` : undefined);

    return (
      /* Outer wrapper -- space-y-1.5 adds 6px vertical gap between
       * the label, the input, and the description/error text. */
      <div className="space-y-1.5">
        {/* Accessible <label> element -- only rendered when label text is provided.
          * Uses htmlFor={inputId} to associate with the <input> below. */}
        {label && (
          <label
            htmlFor={inputId}
            className="block text-sm font-medium text-content-primary"
          >
            {label}
          </label>
        )}

        {/*
         * Relative-positioned wrapper enables absolute positioning of the
         * leading icon and trailing suffix elements inside the input field.
         */}
        <div className="relative">
          {/*
           * Leading icon -- absolutely positioned to the left of the input.
           * Vertically centred with top-1/2 + -translate-y-1/2.
           * When present, the <input> receives extra left padding (pl-9)
           * so that typed text does not overlap the icon.
           */}
          {icon && (
            <div className="absolute left-3 top-1/2 -translate-y-1/2 text-content-tertiary">
              {icon}
            </div>
          )}

          {/*
           * The actual <input> element.
           *
           * Class composition:
           * 1. Base         -- full-width, standard padding, small text
           * 2. Platform     -- rounded-platform border radius from CSS var
           * 3. Colours      -- surface-secondary bg, themed text and placeholder
           * 4. Focus ring   -- accent colour border + ring on focus
           * 5. Disabled     -- reduced opacity and not-allowed cursor
           * 6. Icon padding -- extra left (pl-9) / right (pr-9) when icon/suffix present
           * 7. Error state  -- red border and red focus ring when error is set
           * 8. Consumer     -- any extra classes merged via className prop
           *
           * @see https://tailwindcss.com/docs/ring-width -- focus ring utilities
           */}
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
            /* Forward all remaining native <input> attributes (value, onChange, type, etc.) */
            {...props}
          />

          {/*
           * Trailing suffix element -- absolutely positioned to the right.
           * Vertically centred the same way as the leading icon.
           * When present, the <input> receives extra right padding (pr-9).
           */}
          {suffix && (
            <div className="absolute right-3 top-1/2 -translate-y-1/2 text-content-tertiary">
              {suffix}
            </div>
          )}
        </div>

        {/*
         * Error message -- rendered below the input in red text.
         * Takes visual priority over the description: when both error and
         * description are set, only the error is shown.
         */}
        {error && (
          <p className="text-xs text-status-error">{error}</p>
        )}

        {/*
         * Description / helper text -- rendered only when there is no error.
         * Uses a muted tertiary text colour to visually subordinate it.
         */}
        {!error && description && (
          <p className="text-xs text-content-tertiary">{description}</p>
        )}
      </div>
    );
  },
);
