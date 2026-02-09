/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Modal dialog component.
 * Renders a centered overlay dialog with a backdrop. Supports a title,
 * close button, and custom content. Closes on backdrop click and
 * Escape key press for accessibility.
 */

import { useEffect, useCallback, type ReactNode } from 'react';
import { X } from 'lucide-react';

interface ModalProps {
  /** Whether the modal is currently visible */
  open: boolean;
  /** Callback to close the modal */
  onClose: () => void;
  /** Title displayed in the modal header */
  title?: string;
  /** Modal content */
  children: ReactNode;
  /** Optional maximum width class (default: 'max-w-lg') */
  maxWidth?: string;
}

/**
 * Renders a centered modal dialog with a semi-transparent backdrop.
 * Handles keyboard (Escape) and backdrop-click dismissal.
 *
 * @param open - Controls visibility
 * @param onClose - Called when the modal should close
 * @param title - Optional header title
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
  /* Close on Escape key press */
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    },
    [onClose],
  );

  useEffect(() => {
    if (open) {
      document.addEventListener('keydown', handleKeyDown);
      return () => document.removeEventListener('keydown', handleKeyDown);
    }
  }, [open, handleKeyDown]);

  if (!open) return null;

  return (
    /* Backdrop overlay */
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-surface-overlay"
      onClick={onClose}
    >
      {/* Modal panel - stop click propagation so clicking inside doesn't close */}
      <div
        className={`
          ${maxWidth} w-full mx-4
          bg-surface-primary rounded-platform-lg
          shadow-platform-lg border border-border-light
          overflow-hidden
        `}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header with title and close button */}
        {title && (
          <div className="flex items-center justify-between px-5 py-4 border-b border-border-light">
            <h3 className="text-base font-semibold text-content-primary">
              {title}
            </h3>
            <button
              onClick={onClose}
              className="p-1 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-surface-secondary transition-colors"
              aria-label="Close"
            >
              <X size={18} />
            </button>
          </div>
        )}

        {/* Modal body content */}
        <div className="px-5 py-4">{children}</div>
      </div>
    </div>
  );
}
