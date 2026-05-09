// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file useConfirmation — confirmation modal hook.
 *
 * Audit v2 finding #3: every destructive action across the app shows
 * a confirmation modal with the same shape — title, body text, Cancel
 * + Confirm buttons. DownloadQueue.tsx alone has four (Retry All,
 * Clear All, Delete Item, Abort Queue), each ~20 lines of inline JSX.
 * CrashReportSection, future per-service "Revoke credentials" actions,
 * and other gates will all hit the same shape.
 *
 * This hook centralises the chrome:
 *
 *   const confirmDelete = useConfirmation({
 *     title: 'Delete crash report',
 *     description: 'This cannot be undone.',
 *     confirmLabel: 'Delete',
 *     onConfirm: () => deleteCrashReport(id),
 *   });
 *
 *   return (
 *     <>
 *       <Button onClick={confirmDelete.open}>Delete</Button>
 *       {confirmDelete.modal}
 *     </>
 *   );
 *
 * The hook owns the open/close state. `description` accepts any
 * ReactNode so callers can include item details, "don't ask again"
 * checkboxes (bound to parent state), etc. Auto-closes on successful
 * `onConfirm`; stays open if `onConfirm` throws so the user can
 * retry — `onConfirm` should surface its own error toast (typically
 * via withErrorToast).
 *
 * @see audit doc finding #3 in
 *      [.github/audits/codebase-unification-audit-v2.md]
 * @see withErrorToast — natural pairing for the onConfirm body
 */

import { useCallback, useState, type ReactNode } from 'react';
import { Button, Modal } from '@/components/common';

export interface UseConfirmationOptions {
  /** Modal title (rendered in the header bar). */
  title: string;
  /**
   * Modal body content. Accepts any ReactNode — paragraphs, item
   * details, secondary action checkboxes (bound to parent state),
   * etc. Caller controls spacing via wrapper elements.
   */
  description: ReactNode;
  /** Confirm button label. Default: `"Confirm"`. */
  confirmLabel?: string;
  /** Cancel button label. Default: `"Cancel"`. */
  cancelLabel?: string;
  /**
   * Called when the user clicks Confirm. The modal auto-closes after
   * a successful resolution. If `onConfirm` throws (or returns a
   * rejected promise), the modal stays open so the user can retry —
   * `onConfirm` should surface its own error feedback.
   */
  onConfirm: () => void | Promise<void>;
  /**
   * Called when the user dismisses the modal (Cancel button, backdrop
   * click, Escape key). Optional — most callers don't need it.
   */
  onCancel?: () => void;
}

export interface UseConfirmationReturn {
  /** Show the modal. Typically wired to a trigger button's onClick. */
  open: () => void;
  /**
   * Hide the modal programmatically without firing `onCancel`. Rarely
   * needed — the auto-close on confirm + dismiss handlers cover most
   * cases. Useful for parent-driven race conditions (e.g. an external
   * event invalidates the action while the modal is up).
   */
  close: () => void;
  /** True while the modal is rendered. Caller can use this for
   *  styling or to disable the trigger button. */
  isOpen: boolean;
  /** Render this once in your component's JSX. Returns null when closed. */
  modal: ReactNode;
}

/**
 * Confirmation modal hook. See file header for the API + rationale.
 */
export function useConfirmation(opts: UseConfirmationOptions): UseConfirmationReturn {
  const [isOpen, setIsOpen] = useState(false);

  const open = useCallback(() => setIsOpen(true), []);
  const close = useCallback(() => setIsOpen(false), []);

  const handleCancel = useCallback(() => {
    opts.onCancel?.();
    setIsOpen(false);
  }, [opts]);

  const handleConfirm = useCallback(async () => {
    try {
      await opts.onConfirm();
      setIsOpen(false);
    } catch {
      // Stay open so the user can retry. onConfirm is expected to
      // surface its own error feedback (typically via withErrorToast).
    }
  }, [opts]);

  const modal = (
    <Modal open={isOpen} onClose={handleCancel} title={opts.title}>
      <div className="text-sm text-content-secondary mb-6">
        {opts.description}
      </div>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={handleCancel}>
          {opts.cancelLabel ?? 'Cancel'}
        </Button>
        <Button variant="primary" onClick={handleConfirm}>
          {opts.confirmLabel ?? 'Confirm'}
        </Button>
      </div>
    </Modal>
  );

  return { open, close, isOpen, modal };
}
