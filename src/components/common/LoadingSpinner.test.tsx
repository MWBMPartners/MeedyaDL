/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/common/LoadingSpinner.test.tsx - Unit tests for the LoadingSpinner component
 *
 * Tests the LoadingSpinner component's SVG rendering, size presets (sm, md, lg),
 * optional label text display, and className forwarding.
 *
 * The LoadingSpinner is a pure presentational SVG element that rotates via
 * Tailwind's animate-spin utility class. Its size is controlled by width/height
 * attributes on the SVG element mapped from a size preset.
 *
 * @see src/components/common/LoadingSpinner.tsx - The component under test
 */

import { render, screen } from '@testing-library/react';
import { LoadingSpinner } from '@/components/common/LoadingSpinner';

describe('LoadingSpinner', () => {
  // ===========================================================================
  // Basic Rendering
  // ===========================================================================

  /**
   * Verifies that the LoadingSpinner renders an SVG element.
   * The spinner is a pure SVG with two shapes: a faint background track
   * circle and a visible spinning arc path, both animated by CSS.
   */
  it('renders an SVG spinner element', () => {
    const { container } = render(<LoadingSpinner />);

    /* The spinner should be an <svg> element inside the container */
    const svg = container.querySelector('svg');
    expect(svg).toBeInTheDocument();
    /* The SVG should have the animate-spin class for the rotation animation */
    expect(svg!.classList.contains('animate-spin')).toBe(true);
  });

  // ===========================================================================
  // Size Presets
  // ===========================================================================

  /**
   * Verifies that the default size (md) renders the SVG with width=24 and
   * height=24 pixels. The md size is the standard preset used for section-
   * level loading indicators. When no size prop is provided, md is used.
   */
  it('renders with default md size (width=24, height=24)', () => {
    const { container } = render(<LoadingSpinner />);

    const svg = container.querySelector('svg');
    expect(svg).toBeTruthy();
    /* The default md size maps to 24px in the SIZE_MAP */
    expect(svg!).toHaveAttribute('width', '24');
    expect(svg!).toHaveAttribute('height', '24');
  });

  /**
   * Verifies that size="sm" renders the SVG with width=16 and height=16
   * pixels. The sm size is a compact spinner suitable for inline use next
   * to text or inside small UI elements like tags.
   */
  it('renders sm size (width=16, height=16)', () => {
    const { container } = render(<LoadingSpinner size="sm" />);

    const svg = container.querySelector('svg');
    expect(svg).toBeTruthy();
    /* The sm size maps to 16px in the SIZE_MAP */
    expect(svg!).toHaveAttribute('width', '16');
    expect(svg!).toHaveAttribute('height', '16');
  });

  /**
   * Verifies that size="lg" renders the SVG with width=40 and height=40
   * pixels. The lg size is a large spinner used for full-screen or hero-
   * level loading indicators during app initialisation or major operations.
   */
  it('renders lg size (width=40, height=40)', () => {
    const { container } = render(<LoadingSpinner size="lg" />);

    const svg = container.querySelector('svg');
    expect(svg).toBeTruthy();
    /* The lg size maps to 40px in the SIZE_MAP */
    expect(svg!).toHaveAttribute('width', '40');
    expect(svg!).toHaveAttribute('height', '40');
  });

  // ===========================================================================
  // Label Text
  // ===========================================================================

  /**
   * Verifies that when the label prop is provided, a descriptive text
   * element is rendered below the spinner SVG. The label provides context
   * about what operation is in progress (e.g. "Checking for updates...").
   */
  it('shows label text when provided', () => {
    render(<LoadingSpinner label="Loading..." />);

    /* The label text should be visible below the spinner */
    expect(screen.getByText('Loading...')).toBeInTheDocument();
  });

  /**
   * Verifies that when no label prop is provided, no label text element
   * is rendered. The spinner should appear as a standalone SVG without
   * any accompanying text.
   */
  it('does not render label when not provided', () => {
    const { container } = render(<LoadingSpinner />);

    /*
     * The label is rendered as a <span> with the text-sm class.
     * When no label is provided, the <span> should not exist.
     * We check that no <span> elements are children of the container.
     */
    const labelSpan = container.querySelector('span');
    expect(labelSpan).toBeNull();
  });

  // ===========================================================================
  // className Forwarding
  // ===========================================================================

  /**
   * Verifies that the className prop is merged onto the outer container div.
   * This allows consumers to add spacing, positioning, or other layout
   * utilities to the spinner wrapper without affecting the spinner itself.
   */
  it('applies additional className', () => {
    const { container } = render(
      <LoadingSpinner className="mt-8 custom-spinner" />,
    );

    /*
     * The outer container is the root flex div that holds the SVG and
     * optional label. The className prop is appended to its class list.
     */
    const outerDiv = container.firstElementChild;
    expect(outerDiv).toBeTruthy();
    expect(outerDiv!.className).toContain('mt-8');
    expect(outerDiv!.className).toContain('custom-spinner');
  });
});
