/**
 * Copyright (c) 2024-2026 MWBM Partners Ltd
 * Licensed under the MIT License. See LICENSE file in the project root.
 *
 * Toast notification container and individual toast component.
 * Renders a stack of toast notifications in the top-right corner.
 * Toasts auto-dismiss after their duration and can be manually dismissed.
 * Reads from the UI store's toast list.
 */

import {
  CheckCircle,
  AlertCircle,
  AlertTriangle,
  Info,
  X,
} from 'lucide-react';
import { useUiStore } from '@/stores/uiStore';
import type { Toast, ToastType } from '@/types';

/** Icon and color configuration for each toast type */
const TOAST_CONFIG: Record<
  ToastType,
  { icon: typeof CheckCircle; bgClass: string; borderClass: string }
> = {
  success: {
    icon: CheckCircle,
    bgClass: 'bg-green-50 dark:bg-green-950',
    borderClass: 'border-status-success',
  },
  error: {
    icon: AlertCircle,
    bgClass: 'bg-red-50 dark:bg-red-950',
    borderClass: 'border-status-error',
  },
  warning: {
    icon: AlertTriangle,
    bgClass: 'bg-yellow-50 dark:bg-yellow-950',
    borderClass: 'border-status-warning',
  },
  info: {
    icon: Info,
    bgClass: 'bg-blue-50 dark:bg-blue-950',
    borderClass: 'border-status-info',
  },
};

/** Color class for each toast type's icon */
const ICON_COLORS: Record<ToastType, string> = {
  success: 'text-status-success',
  error: 'text-status-error',
  warning: 'text-status-warning',
  info: 'text-status-info',
};

/**
 * Renders a single toast notification with an icon, message, and dismiss button.
 *
 * @param toast - Toast data (id, message, type)
 * @param onDismiss - Callback to remove this toast
 */
function ToastItem({
  toast,
  onDismiss,
}: {
  toast: Toast;
  onDismiss: (id: string) => void;
}) {
  const config = TOAST_CONFIG[toast.type];
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
      {/* Toast type icon */}
      <Icon size={18} className={`flex-shrink-0 mt-0.5 ${ICON_COLORS[toast.type]}`} />

      {/* Message text */}
      <p className="flex-1 text-sm text-content-primary">{toast.message}</p>

      {/* Dismiss button */}
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
 * Reads toasts from the UI store and renders them in reverse order (newest on top).
 */
export function ToastContainer() {
  const toasts = useUiStore((s) => s.toasts);
  const removeToast = useUiStore((s) => s.removeToast);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed top-4 right-4 z-[100] flex flex-col gap-2 max-w-sm w-full pointer-events-none">
      {toasts.map((toast) => (
        <div key={toast.id} className="pointer-events-auto">
          <ToastItem toast={toast} onDismiss={removeToast} />
        </div>
      ))}
    </div>
  );
}
