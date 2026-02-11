/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * @file src/components/common/Modal.test.tsx - Unit tests for the Modal component
 *
 * Tests the Modal component's visibility toggling (open/closed), title rendering,
 * keyboard dismissal (Escape key), backdrop click dismissal, click containment
 * within the panel (stopPropagation), close button functionality, and maxWidth
 * class application.
 *
 * The lucide-react X icon is mocked to avoid importing the full icon library
 * in the test environment and to provide a stable test target.
 *
 * @see src/components/common/Modal.tsx - The component under test
 */

import { render, screen, fireEvent } from '@testing-library/react';
import { Modal } from '@/components/common/Modal';

/**
 * Mock the lucide-react library to replace the X icon with a lightweight
 * <span> element. This avoids pulling in the full SVG icon library during
 * tests and provides a data-testid for easy querying. The mock spreads
 * all received props onto the span so that class, aria-*, and event handler
 * props still work correctly.
 */
vi.mock('lucide-react', () => ({
  X: (props: any) => <span data-testid="x-icon" {...props} />,
}));

describe('Modal', () => {
  // ===========================================================================
  // Visibility
  // ===========================================================================

  /**
   * Verifies that when open=false, the Modal component returns null and
   * renders nothing to the DOM. This is the default closed state -- no
   * backdrop, no panel, no children should be present.
   */
  it('returns null when open=false (nothing rendered)', () => {
    const { container } = render(
      <Modal open={false} onClose={vi.fn()}>
        <p>Hidden Content</p>
      </Modal>,
    );

    /* The container should be completely empty when the modal is closed */
    expect(container.innerHTML).toBe('');
    /* The children text should not appear anywhere in the document */
    expect(screen.queryByText('Hidden Content')).not.toBeInTheDocument();
  });

  /**
   * Verifies that when open=true, the Modal renders its children content
   * into the DOM. This confirms the component transitions from null to a
   * fully rendered dialog when the open prop changes.
   */
  it('renders children when open=true', () => {
    render(
      <Modal open={true} onClose={vi.fn()}>
        <p>Visible Content</p>
      </Modal>,
    );

    /* The children content should be visible in the document */
    expect(screen.getByText('Visible Content')).toBeInTheDocument();
  });

  // ===========================================================================
  // Title
  // ===========================================================================

  /**
   * Verifies that when the title prop is provided, it is rendered inside
   * an <h3> heading element within the modal header bar. The title gives
   * the dialog a clear purpose description.
   */
  it('renders title in header when provided', () => {
    render(
      <Modal open={true} onClose={vi.fn()} title="Confirm Action">
        <p>Body</p>
      </Modal>,
    );

    /* The title should appear as a heading in the modal header */
    expect(screen.getByText('Confirm Action')).toBeInTheDocument();
    /* It should be rendered within an h3 element (the component's markup) */
    expect(screen.getByText('Confirm Action').tagName).toBe('H3');
  });

  // ===========================================================================
  // Keyboard Dismissal (Escape)
  // ===========================================================================

  /**
   * Verifies that pressing the Escape key calls the onClose callback.
   * The Modal registers a global keydown listener when open, and removes
   * it when closed or unmounted. This test simulates the keyboard event
   * on the document level, matching the component's event listener target.
   */
  it('closes on Escape key press', () => {
    const handleClose = vi.fn();

    render(
      <Modal open={true} onClose={handleClose} title="Test">
        <p>Content</p>
      </Modal>,
    );

    /* Simulate pressing the Escape key at the document level */
    fireEvent.keyDown(document, { key: 'Escape' });

    /* The onClose callback should have been called once */
    expect(handleClose).toHaveBeenCalledTimes(1);
  });

  // ===========================================================================
  // Backdrop Click Dismissal
  // ===========================================================================

  /**
   * Verifies that clicking the semi-transparent backdrop (the outer overlay
   * covering the viewport) calls the onClose callback. This allows users to
   * dismiss the modal by clicking outside the dialog panel.
   */
  it('closes when backdrop is clicked', () => {
    const handleClose = vi.fn();

    render(
      <Modal open={true} onClose={handleClose} title="Test">
        <p>Content</p>
      </Modal>,
    );

    /*
     * The backdrop is the outermost div with the fixed overlay classes.
     * It has onClick={onClose} so clicking it should trigger dismissal.
     * We target the backdrop by its class -- it is the element with
     * 'fixed' and 'bg-surface-overlay'.
     */
    const backdrop = screen.getByText('Content').closest('.fixed');
    expect(backdrop).toBeTruthy();
    fireEvent.click(backdrop!);

    /* The onClose callback should have been called from the backdrop click */
    expect(handleClose).toHaveBeenCalledTimes(1);
  });

  // ===========================================================================
  // Click Containment (stopPropagation)
  // ===========================================================================

  /**
   * Verifies that clicking INSIDE the modal panel does NOT trigger the
   * onClose callback. The panel's onClick handler calls e.stopPropagation()
   * to prevent the click from bubbling up to the backdrop's onClick.
   * Without this, any click inside the modal would dismiss it.
   */
  it('does NOT close when clicking inside the modal panel (stopPropagation)', () => {
    const handleClose = vi.fn();

    render(
      <Modal open={true} onClose={handleClose} title="Test">
        <p>Inner Content</p>
      </Modal>,
    );

    /* Click the inner content text -- this should NOT bubble to the backdrop */
    fireEvent.click(screen.getByText('Inner Content'));

    /* The onClose callback should NOT have been called */
    expect(handleClose).not.toHaveBeenCalled();
  });

  // ===========================================================================
  // Close Button
  // ===========================================================================

  /**
   * Verifies that the close button (X icon) in the modal header calls the
   * onClose callback when clicked. The close button is rendered with
   * aria-label="Close" for accessibility.
   */
  it('renders close button that calls onClose', () => {
    const handleClose = vi.fn();

    render(
      <Modal open={true} onClose={handleClose} title="Test">
        <p>Content</p>
      </Modal>,
    );

    /* Find the close button by its accessible label */
    const closeButton = screen.getByLabelText('Close');
    expect(closeButton).toBeInTheDocument();

    /* Click the close button */
    fireEvent.click(closeButton);

    /* The onClose callback should have been called once */
    expect(handleClose).toHaveBeenCalledTimes(1);
  });

  // ===========================================================================
  // Max Width Class
  // ===========================================================================

  /**
   * Verifies that the modal panel applies the default maxWidth class 'max-w-lg'
   * when no maxWidth prop is provided. This controls the maximum width of the
   * dialog panel (512px by default). The class is applied to the inner panel
   * div that contains the header and body.
   */
  it('applies maxWidth class (default max-w-lg)', () => {
    render(
      <Modal open={true} onClose={vi.fn()} title="Test">
        <p>Content</p>
      </Modal>,
    );

    /*
     * The panel is the div that contains both the title and the body content.
     * It receives the maxWidth class as part of its className template literal.
     * We locate it as the parent container of the heading element.
     */
    const heading = screen.getByText('Test');
    /* The panel is the grandparent of the h3 (h3 -> header div -> panel div) */
    const panel = heading.closest('.overflow-hidden');
    expect(panel).toBeTruthy();
    /* The default maxWidth 'max-w-lg' should be present in the panel's classes */
    expect(panel!.className).toContain('max-w-lg');
  });
});
