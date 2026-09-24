// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

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

import { useEffect, useLayoutEffect, useCallback, useId, useRef, useState, type ReactNode } from 'react';

// Every dialog in the app is built on this shared shell, so translating
// its one piece of fixed text (the close button) here means every modal
// inherits the right language automatically -- no per-dialog wiring needed.
import { useTranslation } from 'react-i18next';

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
/**
 * Every open dialog, in the order it opened; the last one is on top.
 *
 * Each open dialog listens for keys across the whole page. With two open
 * at once, both acted on every key: the lower one pulled focus back into
 * itself on each Tab and the upper one pulled it straight back, so Tab
 * only ever landed on the first or last button; and Escape closed BOTH —
 * which, for the crash-reporting question, recorded "no" for good without
 * the person having seen it (stand-in review, 24 Sept 2026). Now only the
 * top-most dialog handles Tab and Escape.
 */
const openDialogs: object[] = [];

/**
 * The layer each open dialog is drawn on. A new dialog goes one above the
 * highest layer still open. Using "how many are open" instead gave two
 * dialogs the same layer after one below them closed (open A, open B,
 * close A, open C: B and C both 2), and the page order then decided which
 * was drawn on top while the keyboard went to C (Codex, follow-up review).
 * The numbers start again from 1 once every dialog has closed.
 */
const dialogLayers = new Map<object, number>();

export function Modal({ open, onClose, title, children, maxWidth = 'max-w-lg' }: ModalProps) {
  // This dialog's own place in `openDialogs`. A plain object, so identity
  // is all that is compared.
  const dialogToken = useRef<object>({});
  // This dialog's layer on screen: 1 for the first open dialog, 2 for one
  // opened on top of it, and so on. See the `style` on the overlay below.
  const [layer, setLayer] = useState(1);
  /** i18n translation function -- reuses the generic "common.close" word,
   * since that's exactly what the close button says everywhere else. */
  const { t } = useTranslation();
  /**
   * Fix 14 (a11y audit): the title heading's id was hard-coded to the
   * literal string "modal-title", so two `<Modal>`s open at the same
   * time (e.g. a confirmation dialog opened from inside another
   * dialog) would collide -- `aria-labelledby` on the second one would
   * resolve to whichever element with that id the browser finds
   * first, which could be the WRONG modal's title. `useId()` gives
   * every `<Modal>` instance its own unique id.
   */
  const titleId = useId();
  /** Ref to the modal panel for focus management */
  const panelRef = useRef<HTMLDivElement>(null);
  /** Ref to the element that had focus before the modal opened */
  const previousFocusRef = useRef<HTMLElement | null>(null);

  /*
   * Why `onClose` is kept in a ref instead of being read directly.
   * ------------------------------------------------------------------
   * Almost every caller of `<Modal>` passes a brand new inline function
   * as `onClose` on every single render, e.g. `onClose={() => setOpen(false)}`.
   * A new function means a new value, and a new value used to feed straight
   * into `useCallback`/`useEffect` dependency arrays below, which meant the
   * effect that manages focus tore itself down and rebuilt itself on every
   * re-render of whatever opened the dialog -- not just when the dialog
   * actually opened or closed.
   *
   * Tearing the effect down moved focus back to the element that opened
   * the dialog (the "restore focus on close" cleanup step), and rebuilding
   * it immediately moved focus back onto the dialog's first focusable
   * element. In other words: every re-render silently punted focus out of
   * the dialog and back in again.
   *
   * That is exactly what was happening while typing into a text field
   * inside a dialog whose parent re-renders on every keystroke (the
   * MusicKit credentials password field in GeneralTab, the developer
   * passphrase box in App.tsx): each keystroke re-rendered the parent,
   * which created a new `onClose`, which reset focus away from the field
   * the user was actively typing in -- so a field inside a dialog could
   * only ever receive one character before losing focus. With
   * `DownloadQueue` re-rendering roughly ten times a second while
   * downloads are active, the effect fired that often too, meaning the
   * abort-queue confirmation dialog's own button could never be reached
   * by keyboard or even reliably by mouse.
   *
   * The fix: read the *current* `onClose` through a ref instead of taking
   * it as a dependency. The ref's value is kept up to date by the small
   * effect immediately below, but updating the ref does not, by itself,
   * cause anything to re-run -- so the focus-management effect further
   * down can depend on `open` alone and stops caring how often `onClose`'s
   * identity changes.
   */
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  /*
   * Focus trap and keyboard handling.
   * - Escape closes the modal (via the ref above, so this function's own
   *   identity never needs to change when `onClose` changes).
   * - Tab/Shift+Tab cycle between focusable elements inside the panel.
   * See: https://github.com/MWBMPartners/MeedyaDL/issues/218
   */
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    // Only the top-most open dialog answers keys — see `openDialogs`.
    if (openDialogs[openDialogs.length - 1] !== dialogToken.current) return;
    if (e.key === 'Escape') {
      onCloseRef.current();
      return;
    }
    // Focus trap: cycle Tab within the modal.
    // `:not([disabled])` is required here -- without it, Tab from the
    // real last *enabled* control could land on a trailing disabled
    // button (e.g. a "Continue" button disabled while a download is in
    // progress) and treat that as the end of the trap, which then let
    // focus escape the dialog entirely. See the sibling fix that added
    // this exclusion for the full explanation.
    if (e.key === 'Tab' && panelRef.current) {
      const focusable = panelRef.current.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"]):not([disabled])'
      );
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];

      /*
       * A hole in the trap that the two checks below cannot close by
       * themselves: they only ever compare `document.activeElement`
       * against the FIRST or LAST focusable element, so they do nothing
       * at all when focus is somewhere else entirely -- most commonly
       * on `document.body`.
       *
       * That happens more often than it sounds. If the element that
       * currently has focus is removed from the page, or has its
       * `disabled` attribute set, while the dialog is still open (both
       * are ordinary things a re-render can do -- a row disappearing
       * from a list mid-dialog, a "Continue" button being disabled
       * while a download starts), the browser does not wait for a Tab
       * press to react: it moves focus to `document.body` immediately,
       * on its own. From that point, pressing Tab matched neither the
       * "first" nor the "last" check above, so the trap did nothing and
       * the browser's own tab order took over -- walking focus straight
       * out of the dialog and into whatever is behind it on the page.
       *
       * Catching "focus is not inside the panel at all" first, before
       * the first/last checks, closes that hole regardless of how focus
       * got out or what it currently sits on -- there is no longer a
       * focus position this trap fails to catch.
       */
      if (!panelRef.current.contains(document.activeElement)) {
        e.preventDefault();
        (e.shiftKey ? last : first).focus();
        return;
      }

      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }, []); // Stable for the component's whole lifetime -- reads onCloseRef.current, never onClose directly.

  /*
   * Manage focus: move into modal on open, restore on close.
   * Attach keydown listener for Escape + Tab trap.
   *
   * Deliberately depends on `open` ONLY. `handleKeyDown` is intentionally
   * left out of the dependency list: its identity never changes (see the
   * `useCallback` above), so including it would be a no-op for behaviour,
   * but naming it here as a reminder that this effect must re-run ONLY
   * when the dialog actually opens or closes -- never on an unrelated
   * parent re-render -- is the entire point of this fix (see the long
   * comment above `onCloseRef`).
   */
  // useLayoutEffect, not useEffect: the layer must be known BEFORE the
  // dialog is first drawn, or its first frame is drawn at the starting
  // layer and a second dialog can flash underneath the first (stand-in
  // review).
  useLayoutEffect(() => {
    if (open) {
      const token = dialogToken.current;
      openDialogs.push(token);
      // Capped at 50, so a dialog never covers a toast (z-index 100) even if
      // the layers keep climbing -- which they can while one dialog stays
      // open and others below it keep opening and closing (stand-in review).
      const nextLayer = Math.min(Math.max(0, ...dialogLayers.values()) + 1, 50);
      dialogLayers.set(token, nextLayer);
      setLayer(nextLayer);
      // Save the previously focused element to restore later
      previousFocusRef.current = document.activeElement as HTMLElement;
      document.addEventListener('keydown', handleKeyDown);
      // Move focus into the modal after a tick (allows render to complete)
      requestAnimationFrame(() => {
        if (panelRef.current) {
          const firstFocusable = panelRef.current.querySelector<HTMLElement>(
            'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"]):not([disabled])'
          );
          (firstFocusable ?? panelRef.current).focus();
        }
      });
      return () => {
        const at = openDialogs.lastIndexOf(token);
        if (at !== -1) openDialogs.splice(at, 1);
        dialogLayers.delete(token);
        document.removeEventListener('keydown', handleKeyDown);
        // Restore focus to the element that opened the modal -- this now
        // only runs when `open` actually flips back to false (or the
        // component unmounts), not on every unrelated re-render.
        previousFocusRef.current?.focus();
      };
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- see comment above: `handleKeyDown` is stable by construction and must not be allowed to retrigger this effect.
  }, [open]);

  /* Early return -- render nothing when the modal is closed */
  if (!open) return null;

  return (
    /*
     * Backdrop overlay.
     * - fixed inset-0: covers the entire viewport.
     * - z-50, overridden by the inline zIndex: stacks above normal content
     *   (but below toasts at z-[100]), one layer per open dialog in the
     *   order they opened — see the `style` below.
     * - flex items-center justify-center: centres the panel vertically
     *   and horizontally.
     * - bg-surface-overlay: semi-transparent dark background (defined
     *   as a custom theme colour, e.g. rgba(0,0,0,0.5)).
     * - onClick={onClose}: clicking the backdrop dismisses the modal.
     */
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-surface-overlay"
      // Drawn in OPENING order, the same order the keyboard uses. Every
      // dialog used to share `z-50`, so which one appeared on top was
      // decided by where it sat in the page — while Tab and Escape went to
      // the most recently opened. Opening keyboard-shortcut help over the
      // pre-release notice put the help UNDER the notice yet gave it the
      // keys (Codex, batch-3 review). 49 + layer stays below toasts
      // (z-index 100): the layer is capped at 50.
      style={{ zIndex: 49 + layer }}
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
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={title ? titleId : undefined}
        tabIndex={-1}
        className={`
          ${maxWidth} w-full mx-4
          bg-surface-primary rounded-platform-lg
          shadow-platform-lg border border-border-light
          overflow-hidden outline-none
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
            <h3 id={titleId} className="text-base font-semibold text-content-primary">{title}</h3>
            {/*
             * Close button -- uses the Lucide X icon at 18px.
             * aria-label="Close" ensures screen readers announce its purpose.
             * @see https://lucide.dev/icons/x -- Lucide X icon
             */}
            <button
              onClick={onClose}
              className="p-1 rounded-platform text-content-tertiary hover:text-content-primary hover:bg-surface-secondary transition-colors"
              aria-label={t('common.close')}
            >
              <X size={18} />
            </button>
          </div>
        )}

        {/* Modal body -- renders the consumer's children with consistent
            padding. `select-text` (Fix 14, a11y audit) opts back into
            selection -- the app-wide `user-select: none` in
            globals.css otherwise makes dialog text (explanations,
            confirmation copy, error details) impossible to select or
            copy, even though it's ordinary text a user might want to
            quote in a bug report. */}
        <div className="px-5 py-4 select-text">{children}</div>
      </div>
    </div>
  );
}
