/**
 * Copyright (c) 2024-2026 MeedyaSuite
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

import { useState } from 'react';
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
  X: (props: Record<string, unknown>) => <span data-testid="x-icon" {...props} />,
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
      </Modal>
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
      </Modal>
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
      </Modal>
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
      </Modal>
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
      </Modal>
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
      </Modal>
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
      </Modal>
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
      </Modal>
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

  // ===========================================================================
  // Focus stability across parent re-renders (regression test)
  // ===========================================================================

  /**
   * Regression test for a bug where a field inside the dialog lost focus
   * after every keystroke.
   *
   * The real cause: almost every caller passes a brand new inline function
   * as `onClose` on every render (`onClose={() => setOpen(false)}`). That
   * used to flow into the dependency list of the effect that manages
   * focus, so any re-render of the component that opened the dialog --
   * even one that has nothing to do with the dialog itself, like typing a
   * character into a field -- tore the focus-management effect down
   * (moving focus back out to whatever opened the dialog) and immediately
   * rebuilt it (moving focus back onto the dialog's first control). A field
   * inside the dialog could only ever receive one character before losing
   * focus this way.
   *
   * This test reproduces the exact shape of the bug: a wrapper component
   * whose own state changes on every keystroke (so it re-renders, and its
   * inline `onClose` is a new function reference each time), with a real
   * "opener" element for focus to wrongly snap back to. Typing several
   * characters into a field inside the dialog must not move focus away
   * from that field at any point.
   */
  it('keeps focus on a field inside the dialog through several keystrokes, even though the parent creates a new onClose function every render', () => {
    function Harness() {
      const [open, setOpen] = useState(false);
      const [value, setValue] = useState('');
      return (
        <div>
          <button onClick={() => setOpen(true)}>Opener</button>
          {/* onClose is a fresh arrow function on every render, exactly like
              every real caller in this codebase (DownloadQueue, GeneralTab,
              App.tsx, HistoryPage, SettingsPage, LibraryScanPage, etc). */}
          <Modal open={open} onClose={() => setOpen(false)}>
            <input
              aria-label="secret"
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />
          </Modal>
        </div>
      );
    }

    render(<Harness />);

    /* Focus the element that opens the dialog -- this is what a broken
     * focus-management effect would wrongly snap focus back to. */
    const opener = screen.getByText('Opener');
    opener.focus();
    expect(opener).toHaveFocus();

    /* Open the dialog while the opener still has focus, matching the real
     * sequence: click a button, a dialog appears, focus moves inside it. */
    fireEvent.click(opener);

    /* Move focus into the field inside the dialog by hand -- this test is
     * about whether focus STAYS there across re-renders, not about the
     * (asynchronous, requestAnimationFrame-based) auto-focus-on-open. */
    const input = screen.getByLabelText('secret');
    input.focus();
    expect(input).toHaveFocus();

    /* Type several characters one at a time. Each one changes the
     * Harness's `value` state, which re-renders Harness, which creates a
     * brand new `onClose` closure and passes it to Modal. */
    for (const nextValue of ['a', 'ab', 'abc']) {
      fireEvent.change(input, { target: { value: nextValue } });
      expect(input).toHaveFocus();
    }
  });

  // ===========================================================================
  // Focus trap excludes disabled elements (regression test)
  // ===========================================================================

  /**
   * Regression test for a bug where the keyboard could escape the dialog
   * entirely.
   *
   * The focus trap works out which element is "last" by querying
   * `panel.querySelectorAll('button, [href], input, ...')` with no
   * `:not([disabled])` exclusion. When the actual last element in the
   * dialog's markup is disabled (a "Continue" button greyed out while a
   * download is in progress, for example), that disabled button was still
   * counted as "last" -- even though a disabled button can never actually
   * receive focus. Tab pressed from the REAL last usable control then
   * failed to match the trap's "you're at the end, wrap around" check
   * (because the browser's current focus was never going to equal a
   * disabled element), so the trap did nothing and focus was free to leave
   * the dialog for whatever the browser's native tab order finds next.
   *
   * This test does not attempt to model a real browser's native Tab
   * traversal (jsdom does not implement it -- that is exactly why this
   * component has to do the job itself in JavaScript). Instead it checks
   * the component's own logic directly: with `:not([disabled])` excluding
   * the disabled trailing button, pressing Tab while focus is on the real
   * last enabled control must be recognised as "at the end" and wrap
   * focus back to the first focusable element in the dialog.
   */
  it('excludes a disabled trailing element from the Tab focus trap', () => {
    render(
      <Modal open={true} onClose={vi.fn()} title="Test">
        <button>First body button</button>
        <button disabled>Last button (disabled)</button>
      </Modal>
    );

    /* The header's Close button is the true first focusable element. */
    const closeButton = screen.getByLabelText('Close');
    const lastEnabledButton = screen.getByText('First body button');
    const disabledButton = screen.getByText('Last button (disabled)');

    /* Focus the real last ENABLED control in the dialog. */
    lastEnabledButton.focus();
    expect(lastEnabledButton).toHaveFocus();

    /* Tab forward. The disabled button must be excluded from the trap's
     * idea of "last", so this must be treated as tabbing off the end of
     * the dialog and wrap focus back to the first focusable element. */
    fireEvent.keyDown(document, { key: 'Tab' });

    expect(document.activeElement).toBe(closeButton);
    expect(disabledButton).not.toHaveFocus();
  });
});
