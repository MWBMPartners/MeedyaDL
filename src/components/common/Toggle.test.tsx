/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/common/Toggle.test.tsx - Unit tests for the Toggle component
 *
 * Tests the Toggle component's ARIA switch role, aria-checked state, onChange
 * callback behaviour, label/description rendering, disabled state styling,
 * and accent background class application when checked.
 *
 * The Toggle uses role="switch" and aria-checked instead of a native <input
 * type="checkbox">, following the WAI-ARIA switch pattern for a more polished
 * sliding toggle UX.
 *
 * @see src/components/common/Toggle.tsx - The component under test
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { Toggle } from '@/components/common/Toggle';

describe('Toggle', () => {
  // ===========================================================================
  // ARIA Role
  // ===========================================================================

  /**
   * Verifies that the Toggle component renders a <button> element with
   * role="switch". The switch role is the WAI-ARIA semantic for a toggle
   * control, enabling assistive technologies to announce it as a switch
   * rather than a generic button.
   */
  it('renders a button with role="switch"', () => {
    render(<Toggle checked={false} onChange={vi.fn()} />);

    /* The toggle should be queryable by its ARIA switch role */
    const toggle = screen.getByRole('switch');
    expect(toggle).toBeInTheDocument();
    /* It should be rendered as a <button> element */
    expect(toggle.tagName).toBe('BUTTON');
  });

  // ===========================================================================
  // aria-checked State
  // ===========================================================================

  /**
   * Verifies that when checked=true, the toggle's aria-checked attribute
   * is set to "true". This communicates the "on" state to screen readers
   * and other assistive technologies.
   */
  it('shows aria-checked="true" when checked', () => {
    render(<Toggle checked={true} onChange={vi.fn()} />);

    const toggle = screen.getByRole('switch');
    /* aria-checked should reflect the checked prop value */
    expect(toggle).toHaveAttribute('aria-checked', 'true');
  });

  /**
   * Verifies that when checked=false, the toggle's aria-checked attribute
   * is set to "false". This communicates the "off" state to assistive
   * technologies.
   */
  it('shows aria-checked="false" when unchecked', () => {
    render(<Toggle checked={false} onChange={vi.fn()} />);

    const toggle = screen.getByRole('switch');
    /* aria-checked should reflect the unchecked state */
    expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  // ===========================================================================
  // onChange Callback
  // ===========================================================================

  /**
   * Verifies that clicking the toggle calls onChange with the inverse of
   * the current checked state. When the toggle is OFF (checked=false),
   * clicking it should pass true to onChange. This follows the controlled
   * component pattern where the parent decides the new state.
   */
  it('calls onChange with !checked when clicked', () => {
    const handleChange = vi.fn();

    render(<Toggle checked={false} onChange={handleChange} />);

    /* Click the toggle switch to activate it */
    fireEvent.click(screen.getByRole('switch'));

    /* onChange should receive true (the inverse of the current false state) */
    expect(handleChange).toHaveBeenCalledTimes(1);
    expect(handleChange).toHaveBeenCalledWith(true);
  });

  // ===========================================================================
  // Label Text
  // ===========================================================================

  /**
   * Verifies that when the label prop is provided, the label text is
   * rendered next to the toggle switch. The label describes what setting
   * the toggle controls (e.g. "Save Lyrics", "Embed Cover Art").
   */
  it('renders label text', () => {
    render(<Toggle checked={false} onChange={vi.fn()} label="Save Lyrics" />);

    /* The label text should be visible in the document */
    expect(screen.getByText('Save Lyrics')).toBeInTheDocument();
  });

  // ===========================================================================
  // Description Text
  // ===========================================================================

  /**
   * Verifies that when the description prop is provided, a smaller helper
   * text is rendered below the label. The description provides additional
   * context about what the toggle setting does.
   */
  it('renders description text', () => {
    render(
      <Toggle
        checked={false}
        onChange={vi.fn()}
        label="Save Lyrics"
        description="Download and embed lyrics when available"
      />,
    );

    /* The description text should be visible below the label */
    expect(screen.getByText('Download and embed lyrics when available')).toBeInTheDocument();
  });

  // ===========================================================================
  // Disabled State
  // ===========================================================================

  /**
   * Verifies that when disabled=true, the toggle applies visual disabled
   * styling. The component adds 'opacity-50' and 'cursor-not-allowed' to
   * the outer label wrapper to dim the entire toggle row and indicate
   * that interaction is not available.
   */
  it('applies disabled styling when disabled', () => {
    const { container } = render(
      <Toggle checked={false} onChange={vi.fn()} disabled label="Disabled Toggle" />,
    );

    /* The outer <label> element should have the disabled opacity class */
    const label = container.querySelector('label');
    expect(label).toBeTruthy();
    expect(label!.className).toContain('opacity-50');
    expect(label!.className).toContain('cursor-not-allowed');
  });

  /**
   * Verifies that clicking the toggle does NOT invoke onChange when the
   * component is disabled. The onClick handler includes a guard clause
   * `!disabled && onChange(!checked)` that prevents the callback from
   * being called in the disabled state.
   */
  it('does not call onChange when disabled', () => {
    const handleChange = vi.fn();

    render(<Toggle checked={false} onChange={handleChange} disabled />);

    /* Click the toggle switch while it is disabled */
    fireEvent.click(screen.getByRole('switch'));

    /* The onChange handler should NOT have been called */
    expect(handleChange).not.toHaveBeenCalled();
  });

  // ===========================================================================
  // Checked Background Class
  // ===========================================================================

  /**
   * Verifies that when checked=true, the toggle button applies the
   * 'bg-accent' class to show the accent-coloured background track.
   * In the OFF state, the track uses 'bg-border' (a neutral colour).
   * This visual distinction makes the on/off state immediately obvious.
   */
  it('applies bg-accent class when checked', () => {
    render(<Toggle checked={true} onChange={vi.fn()} />);

    const toggle = screen.getByRole('switch');
    /* The checked toggle should have the accent background class */
    expect(toggle.className).toContain('bg-accent');
  });
});
