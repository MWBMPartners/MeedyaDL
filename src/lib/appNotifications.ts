// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file appNotifications.ts -- shows the backend's own messages in the app (#1431).
 *
 * The backend owns a few messages: a download finished, failed or finished
 * with warnings, the queue finished, the computer is about to shut down.
 * For each it applies the one notification rule
 * (`src-tauri/src/services/notification_service.rs`), sends the system
 * notification itself when the rule says so, and emits `app-notification`
 * telling the page whether to show it in the app as well.
 *
 * Before #1431 none of these ever appeared inside the app: the backend
 * sent a system notification (only while the window was in the
 * background) and the page only updated the queue row.
 */

import { useUiStore } from '@/stores/uiStore';
import type { NotifyKind, ToastType } from '@/types';

/** The payload of the backend's `app-notification` event. */
export interface AppNotificationEvent {
  kind: NotifyKind;
  /** The system notification's title (not shown in the app). */
  title: string;
  /** The message text, already naming the item where there is one. */
  body: string;
  toast_type: ToastType;
  /** The item it is about ("Artist — Album"), when there is one. */
  label: string | null;
  /** Whether to show it in the app. */
  show_in_app: boolean;
}

const TOAST_TYPES: readonly ToastType[] = ['success', 'error', 'warning', 'info'];

/**
 * Shows one backend message in the app, when the backend says to.
 *
 * Passes `system: false`: the backend has already made (and acted on) the
 * system-notification decision, and asking again would send it twice.
 */
export function showAppNotification(event: AppNotificationEvent): void {
  if (!event.show_in_app) return;
  const type: ToastType = TOAST_TYPES.includes(event.toast_type) ? event.toast_type : 'info';
  useUiStore.getState().addToast(event.body, type, undefined, undefined, undefined, undefined, {
    kind: event.kind,
    system: false,
  });
}
