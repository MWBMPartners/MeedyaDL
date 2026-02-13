// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file Modal dialog (overlay) component.
 *
 * Renders a centered dialog panel on top of a semi-transparent backdrop.
 * The modal supports:
 *   - A title bar with an integrated close ("X") button.
 *   - Dismissal via Escape key press (keyboard accessibility).
 *   - Dismissal via clicking the backdrop outside the panel.
 *   - Configurable maximum width via the `maxWidth` Tailwind class.
 *
 * **Implementation notes:**
 * This component does NOT use `React.createPortal` -- it renders inline in
 * the component tree. The `fixed inset-0 z-50` classes ensure it covers the
 * entire viewport regardless of its DOM position. If portal-based rendering
 * is ever needed (e.g. to escape overflow:hidden ancestors), wrapping the
 * return value in `createPortal(jsx, document.body)` is straightforward.
 *
 * **Usage across the application:**
 * Available for any feature requiring a dialog overlay. Currently exported
 * from the barrel file but not yet used by any feature component.
 *
 * @see https://react.dev/reference/react-dom/createPortal
 *      React docs -- createPortal (for future portal-based rendering).
 * @see https://lucide.dev/icons/x -- Lucide X (close) icon.
 * @see https://tailwindcss.com/docs/z-index -- z-index stacking context.
 */

import { useEffect, useCallback, type ReactNode } from 'react';

/**
 * Lucide "X" icon used for the modal close button.
 * @see https://lucide.dev/guide/packages/lucide-react -- Lucide React usage
 */
import { X } from 'lucide-react';

/**
 * Props accepted by the {@link Modal} component.
 */
interface ModalProps {
  /**
   * Controls visibility of the modal. When false, the component returns
   * null (renders nothing). When true, the backdrop and panel are displayed.
   */
  open: boolean;

  /**
   * Callback invoked when the modal should close. Triggered by:
   * - Clicking the backdrop overlay.
   * - Pressing the Escape key.
   * - Clicking the close ("X") button in the header.
   */
  onClose: () => void;

  /**
   * Optional title text displayed in the modal header bar. When provided,
   * the header bar (with title + close button) is rendered above the body.
   * When omitted, only the body content is shown (no header).
   */
  title?: string;

  /** Arbitrary React content rendered inside the modal body */
  children: ReactNode;

  /**
   * Tailwind max-width utility class controlling how wide the panel can grow.
   * Defaults to 'max-w-lg' (512px). Pass e.g. 'max-w-2xl' for wider dialogs.
   * @see https://tailwindcss.com/docs/max-width -- Tailwind max-width utilities
   */
  maxWidth?: string;
}

/**
 * Renders a centered modal dialog with a semi-transparent backdrop.
 * Handles keyboard (Escape) and backdrop-click dismissal.
 *
 * **Rendering behaviour:**
 * - When `open` is false the component returns null (nothing is rendered).
 * - When `open` is true a global keydown listener is registered for Escape.
 * - The listener is cleaned up when the modal closes or the component unmounts.
 *
 * @param open     - Controls visibility (true = visible)
 * @param onClose  - Callback invoked when the modal should close
 * @param title    - Optional header title (omit to render body only)
 * @param children - Modal body content
 * @param maxWidth - Tailwind max-width class (default: 'max-w-lg')
 */
export function Modal({
  open,
  onClose,
  title,
  children,
  maxWidth = 'max-w-lg',
}: ModalProps) {
  /*
   * Memoised keydown handler -- only recreated when the onClose reference
   * changes. Calls onClose() when the Escape key is pressed.
   *
   * @see https://react.dev/reference/react/useCallback -- useCallback docs
   */
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    },
    [onClose],
  );

  /*
   * Attach / detach the global keydown listener whenever the modal opens
   * or closes. The cleanup function returned by useEffect removes the
   * listener when `open` becomes false or the component unmounts, preventing
   * memory leaks and stale handler calls.
   *
   * @see https://react.dev/reference/react/useEffect -- useEffect cleanup pattern
   */
  useEffect(() => {
    if (open) {
      document.addEventListener('keydown', handleKeyDown);
      return () => document.removeEventListener('keydown', handleKeyDown);
    }
  }, [open, handleKeyDown]);

  /* Early return -- render nothing when the modal is closed */
  if (!open) return null;

  return (
    /*
     * Backdrop overlay.
     * - fixed inset-0: covers the entire viewport.
     * - z-50: stacks above normal content (but below toasts at z-[100]).
     * - flex items-center justify-center: centres the panel vertically
     *   and horizontally.
     * - bg-surface-overlay: semi-transparent dark background (defined
     *   as a custom theme colour, e.g. rgba(0,0,0,0.5)).
     * - onClick={onClose}: clicking the backdrop dismisses the modal.
     */
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-surface-overlay"
      onClick={onClose}
    >
      {/*
       * Modal panel container.
       * - e.stopPropagation() prevents clicks inside the panel from
       *   bubbling up to the backdrop and triggering onClose.
       * - rounded-platform-lg uses a CSS custom property for the OS-
       *   appropriate large border radius.
       * - mx-4 ensures a minimum 16px gap from the viewport edges on
       *   narrow screens.
       */}
      <div
        className={`
          ${maxWidth} w-full mx-4
          bg-surface-primary rounded-platform-lg
          shadow-platform-lg border border-border-light
          overflow-hidden
        `}
        onClick={(e) => e.stopPropagation()}
      >
        {/*
         * Optional header bar with title and close button.
         * Only rendered when the `title` prop is provided.
         * A bottom border visually separates the header from the body.
         */}
        {title && (
          <div className="flex items-center justify-between px-5 py-4 border-b border-border-light">
            <h3 className="text-base font-semibold text-content-primary">
              {title}
            </h3>
            {/*
             * Close button -- uses the Lucide X icon at 18px.
             * aria-label="Close" ensures screen readers announce its purpose.
             * @see https://lucide.dev/icons/x -- Lucide X icon
             */}
            <button
              onClick={onClose}
              className="p-1 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-surface-secondary transition-colors"
              aria-label="Close"
            >
              <X size={18} />
            </button>
          </div>
        )}

        {/* Modal body -- renders the consumer's children with consistent padding */}
        <div className="px-5 py-4">{children}</div>
      </div>
    </div>
  );
}
