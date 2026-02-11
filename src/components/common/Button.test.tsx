/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/common/Button.test.tsx - Unit tests for the Button component
 *
 * Tests the Button component's rendering behaviour across all supported variants
 * (primary, secondary, ghost, danger), size presets (sm, md, lg), loading state,
 * disabled state, icon slot, fullWidth layout, click handler forwarding, and
 * native HTML attribute pass-through.
 *
 * Uses @testing-library/react to render the component into a jsdom environment
 * and query the resulting DOM for assertions.
 *
 * @see src/components/common/Button.tsx - The component under test
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { Button } from '@/components/common/Button';

describe('Button', () => {
  // ===========================================================================
  // Basic Rendering
  // ===========================================================================

  /**
   * Verifies that the Button renders its children text content.
   * The simplest smoke test: pass a string as children and assert it appears.
   */
  it('renders children text', () => {
    render(<Button>Click Me</Button>);

    /* The button should display the text passed as children */
    expect(screen.getByRole('button', { name: 'Click Me' })).toBeInTheDocument();
  });

  // ===========================================================================
  // Variant Classes
  // ===========================================================================

  /**
   * Verifies that the default variant is 'primary', which applies the accent
   * background class. When no variant prop is provided, the button should use
   * the primary variant's Tailwind classes including 'bg-accent'.
   */
  it('applies primary variant classes by default (bg-accent)', () => {
    render(<Button>Primary</Button>);

    const button = screen.getByRole('button', { name: 'Primary' });
    /* The primary variant includes 'bg-accent' for the solid accent background */
    expect(button.className).toContain('bg-accent');
  });

  /**
   * Verifies that passing variant="secondary" switches to the transparent
   * background styling. The secondary variant is used for less prominent
   * actions like "Cancel" or "Clear".
   */
  it('applies secondary variant classes when variant="secondary" (bg-transparent)', () => {
    render(<Button variant="secondary">Secondary</Button>);

    const button = screen.getByRole('button', { name: 'Secondary' });
    /* The secondary variant uses 'bg-transparent' for a borderless look */
    expect(button.className).toContain('bg-transparent');
  });

  /**
   * Verifies that passing variant="danger" applies destructive-action styling.
   * The danger variant uses a red/error background to signal irreversible actions.
   */
  it('applies danger variant classes when variant="danger" (bg-status-error)', () => {
    render(<Button variant="danger">Delete</Button>);

    const button = screen.getByRole('button', { name: 'Delete' });
    /* The danger variant includes 'bg-status-error' for the red background */
    expect(button.className).toContain('bg-status-error');
  });

  /**
   * Verifies that passing variant="ghost" applies text-only styling with no
   * background or border. The ghost variant is used for toolbar or tertiary
   * actions that should be visually minimal.
   */
  it('applies ghost variant classes when variant="ghost"', () => {
    render(<Button variant="ghost">Ghost</Button>);

    const button = screen.getByRole('button', { name: 'Ghost' });
    /* The ghost variant uses 'bg-transparent' and 'border-transparent' */
    expect(button.className).toContain('bg-transparent');
    expect(button.className).toContain('border-transparent');
    /* Ghost also styles text as secondary content colour */
    expect(button.className).toContain('text-content-secondary');
  });

  // ===========================================================================
  // Size Classes
  // ===========================================================================

  /**
   * Verifies that size="sm" applies the compact size preset classes.
   * The sm size uses smaller padding and font for inline or tag-like buttons.
   */
  it('applies sm size classes', () => {
    render(<Button size="sm">Small</Button>);

    const button = screen.getByRole('button', { name: 'Small' });
    /* sm size should apply text-xs for a 12px font and compact padding */
    expect(button.className).toContain('text-xs');
    expect(button.className).toContain('px-2.5');
  });

  /**
   * Verifies that the default size (md) applies the standard size preset classes.
   * Most form buttons use the md size, which is the middle ground.
   */
  it('applies md size classes by default', () => {
    render(<Button>Medium</Button>);

    const button = screen.getByRole('button', { name: 'Medium' });
    /* md size should apply text-sm for a 14px font and standard padding */
    expect(button.className).toContain('text-sm');
    expect(button.className).toContain('px-4');
  });

  /**
   * Verifies that size="lg" applies the large size preset classes.
   * Large buttons are used for prominent CTAs like a hero download button.
   */
  it('applies lg size classes', () => {
    render(<Button size="lg">Large</Button>);

    const button = screen.getByRole('button', { name: 'Large' });
    /* lg size should apply text-base for a 16px font and spacious padding */
    expect(button.className).toContain('text-base');
    expect(button.className).toContain('px-5');
  });

  // ===========================================================================
  // Loading State
  // ===========================================================================

  /**
   * Verifies that when loading=true, the button renders an SVG spinner element.
   * The spinner is an inline SVG with the 'animate-spin' class that replaces
   * the icon slot while the button is in a loading/working state.
   */
  it('shows spinner SVG when loading=true', () => {
    const { container } = render(<Button loading>Loading</Button>);

    /* The loading spinner is an SVG element with the animate-spin class */
    const spinner = container.querySelector('svg.animate-spin');
    expect(spinner).toBeInTheDocument();
  });

  /**
   * Verifies that the native disabled attribute is set when loading=true.
   * This prevents double-submissions and ensures the button is not clickable
   * during an async operation.
   */
  it('disables button when loading=true', () => {
    render(<Button loading>Loading</Button>);

    const button = screen.getByRole('button', { name: 'Loading' });
    /* The button should have the native disabled attribute */
    expect(button).toBeDisabled();
  });

  // ===========================================================================
  // Disabled State
  // ===========================================================================

  /**
   * Verifies that passing disabled=true sets the native disabled attribute
   * on the underlying <button> element, making it unfocusable and unclickable.
   */
  it('disables button when disabled=true', () => {
    render(<Button disabled>Disabled</Button>);

    const button = screen.getByRole('button', { name: 'Disabled' });
    expect(button).toBeDisabled();
  });

  // ===========================================================================
  // Icon Slot
  // ===========================================================================

  /**
   * Verifies that when the icon prop is provided, the icon element is rendered
   * inside the button. The icon appears to the left of the children text,
   * wrapped in a flex-shrink-0 span to prevent it from being squished.
   */
  it('renders icon when provided', () => {
    render(
      <Button icon={<span data-testid="test-icon">IC</span>}>
        With Icon
      </Button>,
    );

    /* The icon element passed via the icon prop should appear in the DOM */
    expect(screen.getByTestId('test-icon')).toBeInTheDocument();
  });

  // ===========================================================================
  // Full Width
  // ===========================================================================

  /**
   * Verifies that passing fullWidth=true applies the 'w-full' Tailwind class,
   * making the button stretch to fill its parent container's width.
   */
  it('applies w-full when fullWidth=true', () => {
    render(<Button fullWidth>Full Width</Button>);

    const button = screen.getByRole('button', { name: 'Full Width' });
    /* w-full makes the button fill the container width */
    expect(button.className).toContain('w-full');
  });

  // ===========================================================================
  // Click Handler
  // ===========================================================================

  /**
   * Verifies that the onClick handler is invoked when the button is clicked.
   * Uses a vi.fn() mock to track whether the callback was called.
   */
  it('calls onClick handler when clicked', () => {
    const handleClick = vi.fn();
    render(<Button onClick={handleClick}>Click</Button>);

    /* Simulate a user click on the button */
    fireEvent.click(screen.getByRole('button', { name: 'Click' }));

    /* The handler should have been called exactly once */
    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  /**
   * Verifies that the onClick handler is NOT called when the button is disabled.
   * A disabled button should ignore click events entirely -- the browser
   * prevents click events on disabled buttons.
   */
  it('does not call onClick when disabled', () => {
    const handleClick = vi.fn();
    render(<Button disabled onClick={handleClick}>Disabled</Button>);

    /* Simulate a user click on the disabled button */
    fireEvent.click(screen.getByRole('button', { name: 'Disabled' }));

    /* The handler should NOT have been called because the button is disabled */
    expect(handleClick).not.toHaveBeenCalled();
  });

  // ===========================================================================
  // HTML Attribute Forwarding
  // ===========================================================================

  /**
   * Verifies that additional native HTML button attributes (e.g. type="submit")
   * are forwarded to the underlying <button> element via the rest-spread.
   * This ensures the Button component is a transparent wrapper that supports
   * all standard button attributes.
   */
  it('forwards additional HTML attributes (e.g. type="submit")', () => {
    render(<Button type="submit">Submit</Button>);

    const button = screen.getByRole('button', { name: 'Submit' });
    /* The native type attribute should be forwarded to the <button> */
    expect(button).toHaveAttribute('type', 'submit');
  });
});
