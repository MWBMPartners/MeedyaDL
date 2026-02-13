// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file Platform-adaptive toggle switch component.
 *
 * Renders a sliding toggle switch similar to the iOS / macOS system toggle.
 * Used throughout the Settings tabs for boolean preferences (e.g.
 * "Save lyrics", "Embed cover art", "Animated cover art").
 *
 * The toggle uses the WAI-ARIA `role="switch"` and `aria-checked` attributes
 * to communicate its state to assistive technologies. The outer <label>
 * wraps both the text and the switch so that clicking anywhere in the row
 * toggles the state.
 *
 * **Usage across the application:**
 * - AdvancedTab: advanced boolean settings.
 * - GeneralTab: general preference toggles.
 * - QualityTab, CoverArtTab: quality / artwork boolean options.
 * - LyricsTab: lyrics-related boolean settings.
 *
 * @see https://tailwindcss.com/docs/translate -- Tailwind translate utilities
 *      used for the sliding thumb animation.
 * @see https://tailwindcss.com/docs/transition-property -- Tailwind transitions
 * @see https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles/switch_role
 *      MDN -- the ARIA switch role.
 */

/**
 * Props accepted by the {@link Toggle} component.
 */
interface ToggleProps {
  /** Current on/off state of the toggle (controlled component pattern) */
  checked: boolean;

  /**
   * Callback fired when the user clicks the toggle.
   * Receives the *new* boolean state (i.e. the inverse of `checked`).
   */
  onChange: (checked: boolean) => void;

  /** Label text displayed to the left of the toggle switch */
  label?: string;

  /** Smaller description text displayed below the label in muted colour */
  description?: string;

  /** When true, the toggle is visually dimmed and click events are ignored */
  disabled?: boolean;
}

/**
 * Renders a sliding toggle switch with optional label and description.
 *
 * **Visual design:**
 * - OFF state: neutral `bg-border` track, thumb at translate-x-0.
 * - ON state: accent-coloured `bg-accent` track, thumb at translate-x-5.
 * - Transition: 200ms ease-in-out on both colour and transform for a smooth slide.
 *
 * **Accessibility:**
 * - Uses `role="switch"` and `aria-checked` on the <button> element.
 * - Wrapped in a <label> so clicking the text also toggles the switch.
 * - Focus ring (ring-2 ring-accent) is shown on keyboard focus.
 *
 * @example
 * ```tsx
 * <Toggle
 *   label="Save Lyrics"
 *   description="Download and embed lyrics when available"
 *   checked={saveLyrics}
 *   onChange={setSaveLyrics}
 * />
 * ```
 *
 * @param checked     - Current on/off state
 * @param onChange     - Callback receiving the new boolean state
 * @param label       - Text label displayed to the left
 * @param description - Smaller helper text below the label
 * @param disabled    - Whether to disable interaction (default: false)
 */
export function Toggle({
  checked,
  onChange,
  label,
  description,
  disabled = false,
}: ToggleProps) {
  return (
    /*
     * Outer <label> wraps the entire row so that clicking anywhere
     * (text or switch) toggles the state. Uses items-start (not
     * items-center) so the toggle aligns with the first line of text
     * when the description wraps to multiple lines.
     */
    <label
      className={`
        flex items-start gap-3
        ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
      `}
    >
      {/*
       * Left side: label + description text.
       * flex-1 makes the text fill the available space.
       * min-w-0 prevents long text from overflowing the flex container.
       * Only rendered when at least one text prop is provided.
       */}
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

      {/*
       * Toggle track (the pill-shaped background).
       *
       * - type="button" prevents form submission when used inside a <form>.
       * - role="switch" + aria-checked communicate the on/off state to
       *   assistive technologies.
       * - The background transitions between bg-border (off) and bg-accent (on).
       * - focus:ring-2 shows a visible focus indicator for keyboard users.
       *   The ring-offset-2 adds space between the ring and the track.
       *
       * @see https://tailwindcss.com/docs/ring-width -- ring utilities
       */}
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
        {/*
         * Sliding thumb (the circular knob inside the track).
         *
         * - pointer-events-none ensures clicks pass through to the <button>.
         * - translate-x-0 (off) / translate-x-5 (on) slides the thumb
         *   20px to the right when the toggle is activated.
         * - The 200ms transition-transform creates the smooth slide animation.
         * - shadow-platform applies an OS-appropriate drop shadow.
         *
         * @see https://tailwindcss.com/docs/translate -- translate utilities
         */}
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
