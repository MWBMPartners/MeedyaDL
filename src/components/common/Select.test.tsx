/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/common/Select.test.tsx - Unit tests for the Select component
 *
 * Tests the Select component's rendering of options, label association,
 * placeholder option behaviour, description/error text display, htmlFor/id
 * accessibility association, onChange handler forwarding, and disabled option
 * rendering.
 *
 * @see src/components/common/Select.tsx - The component under test
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { Select } from '@/components/common/Select';

/**
 * Shared test options used across multiple test cases.
 * Provides a realistic set of codec choices similar to the app's actual usage.
 */
const testOptions = [
  { value: 'aac', label: 'AAC' },
  { value: 'alac', label: 'ALAC' },
  { value: 'flac', label: 'FLAC' },
];

describe('Select', () => {
  // ===========================================================================
  // Basic Rendering
  // ===========================================================================

  /**
   * Verifies that the Select component renders a <select> element containing
   * <option> elements for each entry in the options array. Each option should
   * display the label text and carry the correct value attribute.
   */
  it('renders a select element with options', () => {
    render(<Select options={testOptions} />);

    /* The <select> element should be in the DOM with the combobox role */
    const select = screen.getByRole('combobox');
    expect(select).toBeInTheDocument();

    /* Each option should be rendered with the correct label text */
    expect(screen.getByText('AAC')).toBeInTheDocument();
    expect(screen.getByText('ALAC')).toBeInTheDocument();
    expect(screen.getByText('FLAC')).toBeInTheDocument();
  });

  // ===========================================================================
  // Label
  // ===========================================================================

  /**
   * Verifies that when the label prop is provided, a visible <label> element
   * is rendered above the select dropdown. The label helps users understand
   * what the dropdown selection is for.
   */
  it('renders label when provided', () => {
    render(<Select options={testOptions} label="Codec" />);

    /* The label text should be visible in the document */
    expect(screen.getByText('Codec')).toBeInTheDocument();
  });

  // ===========================================================================
  // Placeholder
  // ===========================================================================

  /**
   * Verifies that when the placeholder prop is provided, it is rendered as
   * a disabled <option> at the top of the dropdown with an empty value.
   * This acts as a prompt text (e.g. "Choose a codec...") that guides the
   * user but cannot be selected as a valid choice.
   */
  it('renders placeholder as a disabled option', () => {
    render(
      <Select
        options={testOptions}
        placeholder="Choose a codec..."
      />,
    );

    /* The placeholder should appear as an option element */
    const placeholder = screen.getByText('Choose a codec...');
    expect(placeholder).toBeInTheDocument();
    /* The placeholder option should be disabled so it cannot be re-selected */
    expect(placeholder).toBeDisabled();
    /* The placeholder option should have an empty value attribute */
    expect(placeholder).toHaveAttribute('value', '');
  });

  // ===========================================================================
  // Description Text
  // ===========================================================================

  /**
   * Verifies that when the description prop is provided, a small helper
   * text paragraph appears below the select dropdown. This gives users
   * additional context about the selection.
   */
  it('renders description text', () => {
    render(
      <Select
        options={testOptions}
        description="Choose your preferred audio codec"
      />,
    );

    /* The description text should be visible in the document */
    expect(screen.getByText('Choose your preferred audio codec')).toBeInTheDocument();
  });

  // ===========================================================================
  // Error State
  // ===========================================================================

  /**
   * Verifies that when the error prop is set, the error message replaces
   * the description text. Only the error should be shown -- the description
   * is hidden to focus the user's attention on the validation issue.
   */
  it('renders error text and hides description', () => {
    render(
      <Select
        options={testOptions}
        description="This will be hidden"
        error="Please select a codec"
      />,
    );

    /* The error message should be visible */
    expect(screen.getByText('Please select a codec')).toBeInTheDocument();
    /* The description should NOT be rendered when an error is present */
    expect(screen.queryByText('This will be hidden')).not.toBeInTheDocument();
  });

  // ===========================================================================
  // Label-Select Association (Accessibility)
  // ===========================================================================

  /**
   * Verifies that the <label> element is properly associated with the
   * <select> element via matching htmlFor/id attributes. This is essential
   * for accessibility: clicking the label should focus the select, and
   * screen readers should announce the label when the select receives focus.
   * The id is auto-generated from the label text with a 'select-' prefix.
   */
  it('associates label with select via htmlFor/id', () => {
    render(<Select options={testOptions} label="Audio Format" />);

    const select = screen.getByRole('combobox');
    const label = screen.getByText('Audio Format');

    /* The label's htmlFor should match the select's id */
    expect(label).toHaveAttribute('for', select.id);
    /* The auto-generated id should follow the select-{kebab-case-label} pattern */
    expect(select).toHaveAttribute('id', 'select-audio-format');
  });

  // ===========================================================================
  // onChange Handler
  // ===========================================================================

  /**
   * Verifies that the onChange callback is invoked when the user changes the
   * selected option. Uses fireEvent.change to simulate a dropdown selection
   * and asserts that the handler receives the new value.
   */
  it('calls onChange when selection changes', () => {
    const handleChange = vi.fn();

    render(
      <Select
        options={testOptions}
        value="aac"
        onChange={handleChange}
      />,
    );

    const select = screen.getByRole('combobox');

    /* Simulate the user selecting a different option */
    fireEvent.change(select, { target: { value: 'flac' } });

    /* The onChange handler should have been called exactly once */
    expect(handleChange).toHaveBeenCalledTimes(1);
  });

  // ===========================================================================
  // Disabled Options
  // ===========================================================================

  /**
   * Verifies that options with disabled=true are rendered as disabled
   * <option> elements. Disabled options appear in the dropdown list but
   * cannot be selected by the user (they are greyed out by the browser).
   */
  it('renders disabled options correctly', () => {
    const optionsWithDisabled = [
      { value: 'aac', label: 'AAC' },
      { value: 'alac', label: 'ALAC', disabled: true },
      { value: 'flac', label: 'FLAC' },
    ];

    render(<Select options={optionsWithDisabled} />);

    /* The ALAC option should be in the DOM but disabled */
    const alacOption = screen.getByText('ALAC');
    expect(alacOption).toBeDisabled();

    /* The AAC option should NOT be disabled */
    const aacOption = screen.getByText('AAC');
    expect(aacOption).not.toBeDisabled();

    /* The FLAC option should NOT be disabled */
    const flacOption = screen.getByText('FLAC');
    expect(flacOption).not.toBeDisabled();
  });
});
