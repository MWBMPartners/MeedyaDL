// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file Tests for showAppNotification -- the page's half of the backend's
 * own messages (#1431): a download finished or failed, the queue finished.
 * These never used to appear inside the app at all.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';

const notifySystemMock = vi.fn();
vi.mock('@/lib/tauri-commands', () => ({
  notifySystem: (...args: unknown[]) => notifySystemMock(...args),
  saveSidebarCollapsed: vi.fn().mockResolvedValue(undefined),
}));

import { showAppNotification, type AppNotificationEvent } from './appNotifications';
import { useUiStore, __resetToastWorkerForTests } from '@/stores/uiStore';

function event(overrides: Partial<AppNotificationEvent> = {}): AppNotificationEvent {
  return {
    kind: 'download_complete',
    title: 'Download finished',
    body: 'Daft Punk — Discovery finished.',
    toast_type: 'success',
    label: 'Daft Punk — Discovery',
    show_in_app: true,
    ...overrides,
  };
}

describe('showAppNotification', () => {
  beforeEach(() => {
    useUiStore.setState({ toasts: [] });
    __resetToastWorkerForTests();
    notifySystemMock.mockReset();
    notifySystemMock.mockResolvedValue({ sent: false, in_app: true });
  });

  it('shows a finished download in the app, naming the item', async () => {
    showAppNotification(event());
    const { toasts } = useUiStore.getState();
    expect(toasts).toHaveLength(1);
    expect(toasts[0].message).toContain('Daft Punk — Discovery');
    expect(toasts[0].type).toBe('success');
    await new Promise((resolve) => setTimeout(resolve, 0));
    // The backend already decided about the system notification.
    expect(notifySystemMock).not.toHaveBeenCalled();
  });

  it('shows a failure as an error that stays until dismissed', () => {
    showAppNotification(
      event({
        kind: 'download_failed',
        toast_type: 'error',
        body: 'Daft Punk — Discovery could not be downloaded. Check your internet connection.',
      })
    );
    const [toast] = useUiStore.getState().toasts;
    expect(toast.type).toBe('error');
    expect(toast.expiresAt).toBeNull();
  });

  it('shows nothing when the backend says the system already has it', () => {
    showAppNotification(event({ show_in_app: false }));
    expect(useUiStore.getState().toasts).toHaveLength(0);
  });
});
