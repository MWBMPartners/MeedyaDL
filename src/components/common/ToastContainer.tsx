// Copyright (c) 2024-2026 MeedyaDL

/**
 * @file Toast notification container and individual toast item component.
 *
 * This file exports a single public component, {@link ToastContainer}, which
 * renders a fixed-position stack of toast notifications in the top-right
 * corner of the viewport. Individual toast messages are managed by the global
 * UI store (`useUiStore`) and rendered as {@link ToastItem} sub-components.
 *
 * **Toast lifecycle:**
 * 1. Any part of the app calls `useUiStore.getState().addToast(message, type)`.
 * 2. The store appends a new Toast object (with a generated id and timestamp).
 * 3. ToastContainer re-renders and slides in a new ToastItem.
 * 4. The store's internal timer auto-removes the toast after its duration.
 * 5. The user can also dismiss manually by clicking the "X" button.
 *
 * **Usage across the application:**
 * ToastContainer is rendered once in MainLayout.tsx. It does not need to be
 * placed anywhere else -- the fixed positioning ensures global visibility.
 *
 * @see https://lucide.dev/icons/check-circle  -- success icon
 * @see https://lucide.dev/icons/alert-circle   -- error icon
 * @see https://lucide.dev/icons/alert-triangle -- warning icon
 * @see https://lucide.dev/icons/info           -- info icon
 * @see https://lucide.dev/icons/x              -- dismiss (close) icon
 */

/**
 * Lucide icon imports -- one icon per toast type, plus X for the dismiss button.
 * @see https://lucide.dev/guide/packages/lucide-react
 */
import {
  CheckCircle,
  AlertCircle,
  AlertTriangle,
  Info,
  X,
} from 'lucide-react';

/** Global UI store hook -- provides the `toasts` array and `removeToast` action */
import { useUiStore } from '@/stores/uiStore';

/** TypeScript types for a single toast message and the union of toast types */
import type { Toast, ToastType } from '@/types';

/**
 * Visual configuration for each toast type.
 *
 * Maps every {@link ToastType} to:
 * - `icon`        -- the Lucide icon component to render.
 * - `bgClass`     -- background colour (light + dark mode variants).
 * - `borderClass` -- left/top border colour using the project's status tokens.
 *
 * The `typeof CheckCircle` type ensures any Lucide icon component can be
 * assigned (they all share the same component signature).
 */
const TOAST_CONFIG: Record<
  ToastType,
  { icon: typeof CheckCircle; bgClass: string; borderClass: string }
> = {
  /** Green palette for successful operations */
  success: {
    icon: CheckCircle,
    bgClass: 'bg-green-50 dark:bg-green-950',
    borderClass: 'border-status-success',
  },
  /** Red palette for errors and failures */
  error: {
    icon: AlertCircle,
    bgClass: 'bg-red-50 dark:bg-red-950',
    borderClass: 'border-status-error',
  },
  /** Yellow palette for warnings / non-blocking issues */
  warning: {
    icon: AlertTriangle,
    bgClass: 'bg-yellow-50 dark:bg-yellow-950',
    borderClass: 'border-status-warning',
  },
  /** Blue palette for informational messages */
  info: {
    icon: Info,
    bgClass: 'bg-blue-50 dark:bg-blue-950',
    borderClass: 'border-status-info',
  },
};

/**
 * Tailwind text-colour class for each toast type's icon.
 * Uses the project's semantic status colour tokens defined in tailwind.config.ts.
 */
const ICON_COLORS: Record<ToastType, string> = {
  success: 'text-status-success',  // green icon
  error: 'text-status-error',      // red icon
  warning: 'text-status-warning',  // yellow / amber icon
  info: 'text-status-info',        // blue icon
};

/**
 * Internal sub-component that renders a single toast notification card.
 *
 * Each card contains:
 * 1. A coloured icon indicating the toast type (success, error, warning, info).
 * 2. The message text.
 * 3. A small dismiss ("X") button.
 *
 * The card slides in from the right using the `animate-in slide-in-from-right`
 * utility classes (provided by a Tailwind animate plugin or custom keyframes).
 *
 * This component is **not exported** -- it is used exclusively by
 * {@link ToastContainer}.
 *
 * @param toast     - The Toast data object (id, message, type) from the UI store.
 * @param onDismiss - Callback that receives the toast id to remove it from the store.
 */
function ToastItem({
  toast,
  onDismiss,
}: {
  toast: Toast;
  onDismiss: (id: string) => void;
}) {
  /* Look up the visual config (icon component, bg colour, border colour) for this toast type */
  const config = TOAST_CONFIG[toast.type];
  /* Alias the icon component for JSX usage (must start with uppercase) */
  const Icon = config.icon;

  return (
    <div
      className={`
        flex items-start gap-3 px-4 py-3
        rounded-platform border shadow-platform
        ${config.bgClass} ${config.borderClass}
        animate-in slide-in-from-right
      `}
    >
      {/*
       * Toast type icon -- flex-shrink-0 prevents it from being compressed.
       * mt-0.5 nudges it down slightly to align with the first line of text.
       */}
      <Icon size={18} className={`flex-shrink-0 mt-0.5 ${ICON_COLORS[toast.type]}`} />

      {/* Message text -- flex-1 allows it to fill the remaining horizontal space */}
      <p className="flex-1 text-sm text-content-primary">{toast.message}</p>

      {/*
       * Manual dismiss button.
       * aria-label="Dismiss" ensures screen readers announce its purpose.
       * Clicking calls onDismiss with this toast's unique id.
       */}
      <button
        onClick={() => onDismiss(toast.id)}
        className="flex-shrink-0 p-0.5 rounded text-content-tertiary hover:text-content-primary transition-colors"
        aria-label="Dismiss"
      >
        <X size={14} />
      </button>
    </div>
  );
}

/**
 * Renders the toast notification stack in the top-right corner of the viewport.
 *
 * **Positioning strategy:**
 * - `fixed top-4 right-4` anchors the stack 16px from the top-right corner.
 * - `z-[100]` places toasts above everything including modals (z-50).
 * - `pointer-events-none` on the outer container allows click-through to
 *   elements beneath the stack area. Each individual toast card is wrapped
 *   in a `pointer-events-auto` div so its buttons remain interactive.
 * - `max-w-sm w-full` caps the toast width at 384px.
 *
 * The component subscribes to the UI store's `toasts` array via Zustand
 * selectors. When the array is empty, nothing is rendered (returns null).
 *
 * **Rendered once** in MainLayout.tsx -- do not instantiate this component
 * in multiple places or duplicate toasts will appear.
 */
export function ToastContainer() {
  /* Subscribe to the toasts array from the global UI store (Zustand) */
  const toasts = useUiStore((s) => s.toasts);
  /* Action to remove a single toast by its id */
  const removeToast = useUiStore((s) => s.removeToast);

  /* Early return -- render nothing when there are no active toasts */
  if (toasts.length === 0) return null;

  return (
    /*
     * Fixed container -- top-right corner of the viewport.
     * flex-col gap-2 stacks multiple toasts vertically with 8px spacing.
     * pointer-events-none allows clicks to pass through to the content below.
     */
    <div className="fixed top-4 right-4 z-[100] flex flex-col gap-2 max-w-sm w-full pointer-events-none">
      {toasts.map((toast) => (
        /*
         * Each toast is wrapped in a pointer-events-auto div so that its
         * buttons (dismiss, etc.) remain clickable even though the outer
         * container has pointer-events-none.
         * The key is the toast's unique id (generated by the store).
         */
        <div key={toast.id} className="pointer-events-auto">
          <ToastItem toast={toast} onDismiss={removeToast} />
        </div>
      ))}
    </div>
  );
}
