// Copyright (c) 2024-2026 MeedyaSuite
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
 * How many distinct clipboard URLs to remember within one session before
 * the oldest one is forgotten.
 *
 * `seenUrls` below used to have no limit at all -- every different URL
 * that ever passed through the clipboard while monitoring was switched on
 * stayed in memory for as long as the app kept running. For most people
 * that is a handful of entries and never mattered, but MeedyaDL is meant
 * to sit open in the background for a long time, and clipboard monitoring
 * runs the whole time it's open -- so a session left running for days
 * could quietly build up thousands of entries that serve no purpose once
 * that link is no longer on the clipboard.
 *
 * The only job this set does is stop the same toast popping up again
 * while the same link is STILL sitting on the clipboard. Nothing needs
 * to be remembered for longer than "the last few dozen different things
 * that were recently copied", so 200 is a generous cap that will never
 * be reached by ordinary use, while still putting a ceiling on it.
 */
const MAX_SEEN_URLS = 200;

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

          // Keep the set bounded (see MAX_SEEN_URLS above). A JS Set
          // remembers the order things were added in, so the first key
          // its iterator hands back is whichever one has been sitting
          // there longest -- that's the one to drop before adding the
          // new arrival, so the set never grows past the cap.
          if (seenUrls.current.size >= MAX_SEEN_URLS) {
            const oldest = seenUrls.current.values().next().value;
            if (oldest !== undefined) {
              seenUrls.current.delete(oldest);
            }
          }
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
                  return;
                }
                // This branch used to do nothing at all. The backend
                // deliberately sends back an EMPTY download_id, with the
                // reason in `duplicate_warning`, for perfectly ordinary
                // situations -- the commonest being "this link is already
                // in the queue". That's not a failure, so it never reached
                // the .catch() below either: clicking Download just sat
                // there and produced no toast, no error, nothing, leaving
                // the person with no way to tell whether it had worked.
                // downloadStore.ts's undo-clear handler already deals with
                // this identical shape from the same IPC call, so this
                // follows the same fallback: the server's own explanation
                // first, then a plain sentence if it didn't send one.
                useUiStore.getState().addToast(
                  result.duplicate_warning ?? 'Already in the queue — nothing added',
                  'info',
                );
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
                // This used to also send when permission was 'default',
                // which reads as though it might mean "go ahead, that's
                // the normal case" -- it means the opposite. 'default' is
                // the OS's answer when the person has never been asked or
                // never answered, i.e. permission has NOT been granted.
                // Calling sendNotification() anyway didn't throw or show
                // an error; the OS just silently drops a notification it
                // was never given permission to show. So this looked
                // fine in testing (nothing crashed) while quietly never
                // notifying a single real user who hadn't already said
                // yes. Only 'granted' actually means granted.
                if (permission === 'granted') {
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
