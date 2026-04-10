// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// useClipboardMonitor hook -- polls the system clipboard for supported URLs.
//
// When enabled in settings, this hook reads the clipboard every 2 seconds
// and shows an actionable toast when a supported URL (Apple Music) is
// detected. The user can click "Download" to navigate to the Download
// page with the URL pre-filled, or dismiss the toast.
//
// Privacy-first:
//   - Only checks clipboard text for URL patterns.
//   - Never stores or logs clipboard contents.
//   - Tracks previously-seen URLs in a session-scoped Set to avoid
//     re-prompting for the same URL.
//
// @see src/lib/tauri-commands.ts -- readClipboard() IPC wrapper
// @see src/lib/url-parser.ts -- isAppleMusicUrl() validation

import { useEffect, useRef } from 'react';
import { useSettingsStore } from '@/stores/settingsStore';
import { useUiStore } from '@/stores/uiStore';
import { readClipboard, startDownload } from '@/lib/tauri-commands';
import { isAppleMusicUrl, parseAppleMusicUrl, getContentTypeLabel } from '@/lib/url-parser';
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';

/** Polling interval in milliseconds (2 seconds) */
const POLL_INTERVAL_MS = 2000;

/** Toast key for clipboard URL prompts (ensures only one on screen) */
const CLIPBOARD_TOAST_KEY = 'clipboard-url';

/**
 * Hook that monitors the system clipboard for supported URLs.
 *
 * Call this hook once in `App.tsx` after the app is ready. It manages
 * its own polling interval and cleans up on unmount.
 *
 * @param isReady - Whether the app has finished initialising
 */
export function useClipboardMonitor(isReady: boolean): void {
  // Track URLs we've already prompted about in this session to avoid
  // re-prompting when the same URL remains on the clipboard.
  const seenUrls = useRef<Set<string>>(new Set());

  useEffect(() => {
    if (!isReady) return;

    const interval = setInterval(() => {
      // Read setting imperatively to avoid re-registering the interval
      const { clipboard_monitoring } = useSettingsStore.getState().settings;
      if (!clipboard_monitoring) return;

      readClipboard()
        .then((text) => {
          if (!text) return;

          // Trim and take only the first line (ignore multi-line pastes)
          const trimmed = text.trim().split('\n')[0].trim();
          if (!trimmed) return;

          // Check if it's a supported URL
          if (!isAppleMusicUrl(trimmed)) return;

          // Don't re-prompt for URLs we've already shown
          if (seenUrls.current.has(trimmed)) return;
          seenUrls.current.add(trimmed);

          // Parse the URL to get the content type for a descriptive toast
          const parsed = parseAppleMusicUrl(trimmed);
          const typeLabel = parsed.isValid
            ? getContentTypeLabel(parsed.contentType)
            : 'content';

          const toastMessage = `Apple Music ${typeLabel} URL detected in clipboard`;
          const downloadAction = () => {
            // Directly queue the download — skip the input pre-fill step
            startDownload({ urls: [trimmed] })
              .then((result) => {
                if (result.download_id) {
                  useUiStore.getState().addToast('Added to queue from clipboard', 'success');
                }
              })
              .catch((err) => {
                useUiStore.getState().addToast(
                  err instanceof Error ? err.message : String(err),
                  'error',
                );
              });
          };

          // Always show the in-app toast (visible when window is focused)
          useUiStore.getState().addToast(
            toastMessage,
            'info',
            10000, // 10 seconds — longer than default to give the user time
            CLIPBOARD_TOAST_KEY,
            { label: 'Download', onClick: downloadAction }
          );

          // Also send a native OS notification when the window is NOT focused
          // so the user sees the detection even when MeedyaDL is minimised.
          const { desktop_notifications } = useSettingsStore.getState().settings;
          if (desktop_notifications && !document.hasFocus()) {
            isPermissionGranted()
              .then((granted) => {
                if (!granted) return requestPermission();
                return 'granted';
              })
              .then((permission) => {
                if (permission === 'granted' || permission === 'default') {
                  sendNotification({
                    title: 'MeedyaDL',
                    body: `Apple Music ${typeLabel} URL detected — click to download`,
                  });
                }
              })
              .catch(() => { /* notification permission denied or unavailable */ });
          }
        })
        .catch(() => {
          // Silently ignore clipboard read errors (e.g., no display server)
        });
    }, POLL_INTERVAL_MS);

    return () => clearInterval(interval);
  }, [isReady]);
}
