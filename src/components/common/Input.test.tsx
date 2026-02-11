/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/common/Input.test.tsx - Unit tests for the Input component
 *
 * Tests the Input component's rendering behaviour including label association,
 * description/error text display, leading icon and trailing suffix rendering,
 * native input prop forwarding, and automatic id generation from label text.
 *
 * The Input component uses React.forwardRef to expose the underlying <input>
 * DOM node, but these tests focus on the rendered output and user-facing
 * behaviour rather than the ref mechanism.
 *
 * @see src/components/common/Input.tsx - The component under test
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { Input } from '@/components/common/Input';

describe('Input', () => {
  // ===========================================================================
  // Basic Rendering
  // ===========================================================================

  /**
   * Verifies that the Input component renders an <input> element in the DOM.
   * This is the most basic smoke test to confirm the component mounts correctly.
   */
  it('renders an input element', () => {
    render(<Input />);

    /* There should be exactly one <input> element (role="textbox" by default) */
    expect(screen.getByRole('textbox')).toBeInTheDocument();
  });

  // ===========================================================================
  // Label
  // ===========================================================================

  /**
   * Verifies that when the label prop is provided, a <label> element is
   * rendered with the specified text. The label provides a visible text
   * description of the input's purpose.
   */
  it('renders label when provided', () => {
    render(<Input label="Username" />);

    /* The label text should be visible in the document */
    expect(screen.getByText('Username')).toBeInTheDocument();
  });

  /**
   * Verifies that the <label> element is properly associated with the <input>
   * via matching htmlFor/id attributes. This is critical for accessibility:
   * clicking the label should focus the input, and screen readers should
   * announce the label when the input receives focus.
   */
  it('associates label with input via htmlFor/id', () => {
    render(<Input label="Email Address" />);

    const input = screen.getByRole('textbox');
    const label = screen.getByText('Email Address');

    /* The label's htmlFor should match the input's id attribute */
    expect(label).toHaveAttribute('for', input.id);
    /* Both should use the auto-generated id derived from the label text */
    expect(input).toHaveAttribute('id', 'input-email-address');
  });

  // ===========================================================================
  // Description Text
  // ===========================================================================

  /**
   * Verifies that when the description prop is provided, a small helper
   * text paragraph appears below the input. This text gives users additional
   * context about what to enter in the field.
   */
  it('renders description text', () => {
    render(<Input description="Enter your full name" />);

    /* The description text should be visible in the document */
    expect(screen.getByText('Enter your full name')).toBeInTheDocument();
  });

  // ===========================================================================
  // Error State
  // ===========================================================================

  /**
   * Verifies that when the error prop is set, the error message is displayed
   * AND the description text is hidden. The error takes visual priority over
   * the description to focus the user's attention on the validation issue.
   */
  it('renders error text and hides description when error is set', () => {
    render(
      <Input
        description="This will be hidden"
        error="This field is required"
      />,
    );

    /* The error message should be visible */
    expect(screen.getByText('This field is required')).toBeInTheDocument();
    /* The description should NOT be rendered when an error is present */
    expect(screen.queryByText('This will be hidden')).not.toBeInTheDocument();
  });

  /**
   * Verifies that when error is set, the input receives the red border class
   * 'border-status-error'. This visually highlights the field as invalid.
   */
  it('applies error border class (border-status-error)', () => {
    render(<Input error="Invalid value" />);

    const input = screen.getByRole('textbox');
    /* The input should have the error border class for visual indication */
    expect(input.className).toContain('border-status-error');
  });

  // ===========================================================================
  // Leading Icon
  // ===========================================================================

  /**
   * Verifies that when the icon prop is provided, the icon element is rendered
   * inside the input's wrapper. The icon appears on the left side of the input
   * and the input receives extra left padding (pl-9) to avoid text overlap.
   */
  it('renders leading icon', () => {
    render(<Input icon={<span data-testid="search-icon">S</span>} />);

    /* The icon element passed via the icon prop should appear in the DOM */
    expect(screen.getByTestId('search-icon')).toBeInTheDocument();
  });

  // ===========================================================================
  // Trailing Suffix
  // ===========================================================================

  /**
   * Verifies that when the suffix prop is provided, the suffix element is
   * rendered inside the input's wrapper on the right side. The suffix is
   * typically a unit label like "px" or a small action button.
   */
  it('renders trailing suffix', () => {
    render(<Input suffix={<span data-testid="suffix-unit">px</span>} />);

    /* The suffix element passed via the suffix prop should appear in the DOM */
    expect(screen.getByTestId('suffix-unit')).toBeInTheDocument();
  });

  // ===========================================================================
  // Native Input Prop Forwarding
  // ===========================================================================

  /**
   * Verifies that native HTML input attributes (placeholder, type, value,
   * onChange) are forwarded to the underlying <input> element via the
   * rest-spread. This ensures the Input component is transparent to all
   * standard input attributes.
   */
  it('forwards native input props (placeholder, type, value, onChange)', () => {
    const handleChange = vi.fn();

    render(
      <Input
        placeholder="Type here..."
        type="email"
        value="test@example.com"
        onChange={handleChange}
      />,
    );

    const input = screen.getByRole('textbox');

    /* The placeholder attribute should be forwarded to the <input> */
    expect(input).toHaveAttribute('placeholder', 'Type here...');
    /* The type attribute should be forwarded (note: email still has role=textbox) */
    expect(input).toHaveAttribute('type', 'email');
    /* The value attribute should be forwarded for controlled input behaviour */
    expect(input).toHaveValue('test@example.com');

    /* Simulate typing to verify the onChange handler is called */
    fireEvent.change(input, { target: { value: 'new@example.com' } });
    expect(handleChange).toHaveBeenCalledTimes(1);
  });

  // ===========================================================================
  // Auto-Generated ID from Label
  // ===========================================================================

  /**
   * Verifies that the component generates a deterministic id from the label
   * text when no explicit id prop is provided. The id is constructed by
   * lowercasing the label, replacing whitespace with hyphens, and prefixing
   * with 'input-'. Example: "Filename Template" -> "input-filename-template".
   * This ensures unique, readable ids for accessibility association.
   */
  it('generates id from label text (e.g. "Filename Template" -> "input-filename-template")', () => {
    render(<Input label="Filename Template" />);

    const input = screen.getByRole('textbox');
    /* The id should be a kebab-case version of the label, prefixed with 'input-' */
    expect(input).toHaveAttribute('id', 'input-filename-template');
  });
});
