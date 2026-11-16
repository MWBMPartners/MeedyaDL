// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
/**
 * @file NotificationSettingsSection.tsx -- Settings > General > Notifications (#1431).
 *
 * Every control here is an ordinary setting: it changes the page's copy,
 * and reaches disk when the person presses Save Changes. The backend's one
 * notification rule (`src-tauri/src/services/notification_service.rs`)
 * reads the SAVED values, so a change takes effect once saved.
 *
 * Two things are deliberately different from before #1431:
 * - "System Notifications" (the stored `desktop_notifications`) is the
 *   switch for SYSTEM notifications only. Messages inside the app always
 *   show, so the Auto-Dismiss control (which governs those) is always
 *   visible, not hidden behind that switch.
 * - The test button sends with the values ON SCREEN, so a choice can be
 *   tried before it is saved, and says honestly what happened: the
 *   notification was handed to the system, and if nothing appeared, where
 *   on this computer to look.
 */

import { Bell } from 'lucide-react';
import { useSettingsField } from '@/hooks/useSettingsField';
import { usePlatform } from '@/hooks/usePlatform';
import { useUiStore } from '@/stores/uiStore';
import { Button, Select, SettingsSection, Toggle } from '@/components/common';
import { showError } from '@/lib/errorMessages';
import { testDesktopNotification } from '@/lib/tauri-commands';
import { whereToLookForNotifications } from '@/lib/notificationAdvice';
import type { NativeNotificationScope, NativeNotificationWhen } from '@/types';

/** The three delivery styles. */
const NOTIFICATION_STYLE_OPTIONS = [
  { value: 'in_app_only', label: 'In-app only' },
  { value: 'native_and_in_app', label: 'System and in-app (default)' },
  { value: 'native_only', label: 'System only' },
];

/** When a system notification is shown (rule A of #1431). */
const WHEN_OPTIONS: { value: NativeNotificationWhen; label: string }[] = [
  {
    value: 'background_or_other_screen',
    label: 'When MeedyaDL is not in front, or the pointer is on another screen (default)',
  },
  { value: 'other_screen_only', label: 'Only when the pointer is on another screen' },
  { value: 'always', label: 'Always' },
];

/** Which messages may become system notifications (rule B of #1431). */
const SCOPE_OPTIONS: { value: NativeNotificationScope; label: string }[] = [
  { value: 'important', label: 'Important messages only (default)' },
  { value: 'important_and_errors', label: 'Important messages and errors' },
  { value: 'everything', label: 'Every message' },
];

/** How long a message stays inside the app. */
const NOTIFICATION_DISMISS_OPTIONS = [
  { value: '3', label: '3 seconds' },
  { value: '5', label: '5 seconds (default)' },
  { value: '10', label: '10 seconds' },
  { value: '15', label: '15 seconds' },
  { value: '30', label: '30 seconds' },
  { value: '60', label: '60 seconds' },
];

/** The Notifications section of Settings > General. */
export function NotificationSettingsSection() {
  // `isLoading`: the platform is detected a moment after start-up; until
  // then the advice must not name one system's settings.
  const { platform, isLoading: platformUnknown } = usePlatform();
  const knownPlatform = platformUnknown ? null : platform;
  const addToast = useUiStore((s) => s.addToast);

  const desktopNotifications = useSettingsField('desktop_notifications');
  const notificationStyle = useSettingsField('notification_style');
  const when = useSettingsField('native_notification_when');
  const scope = useSettingsField('native_notification_scope');
  const notifyComplete = useSettingsField('notify_download_complete');
  const notifyFailed = useSettingsField('notify_download_failed');
  const notifyQueue = useSettingsField('notify_queue_finished');
  const notifyClipboard = useSettingsField('notify_clipboard_link');
  const dismissSeconds = useSettingsField('notification_auto_dismiss_seconds');

  const style = notificationStyle.value ?? 'native_and_in_app';
  const systemOn = desktopNotifications.value && style !== 'in_app_only';

  /** Sends the test notification with the values on screen. */
  const sendTest = async () => {
    try {
      await testDesktopNotification(desktopNotifications.value, style);
      addToast(
        `MeedyaDL handed a test notification to your system. ${whereToLookForNotifications(knownPlatform)}`,
        'info',
        15000,
        undefined,
        undefined,
        undefined,
        // This message is about the system notification; sending it to
        // the system as well would only muddle the test.
        { system: false }
      );
    } catch (err) {
      showError(
        `MeedyaDL could not send a test notification. ${whereToLookForNotifications(knownPlatform)}`,
        err
      );
    }
  };

  return (
    <SettingsSection
      title="Notifications"
      description="Messages always appear inside MeedyaDL. These settings decide when your system shows them too. Changes apply once saved."
    >
      <Toggle
        label="System Notifications"
        description="Let MeedyaDL show notifications from your system (outside the app), for example when a download finishes. Messages inside the app are shown either way."
        checked={desktopNotifications.value}
        onChange={desktopNotifications.set}
      />

      {desktopNotifications.value && (
        <>
          <Select
            label="Show System Notifications"
            description="With one screen, 'Only when the pointer is on another screen' never shows a system notification."
            options={WHEN_OPTIONS}
            value={when.value ?? 'background_or_other_screen'}
            onChange={(e) => when.set(e.target.value as NativeNotificationWhen)}
          />

          <Select
            label="Which Messages"
            description="Important messages are: a download finished or failed, the queue finished, a copied link was found, your computer is about to shut down or restart, and an app update is ready."
            options={SCOPE_OPTIONS}
            value={scope.value ?? 'important'}
            onChange={(e) => scope.set(e.target.value as NativeNotificationScope)}
          />

          <Select
            label="Notification Style"
            description="'System only' leaves out the in-app copy of a plain message once the system has shown it. Errors, warnings, messages with a button, and anything the system was not asked to show still appear in the app."
            options={NOTIFICATION_STYLE_OPTIONS}
            value={style}
            onChange={(e) =>
              notificationStyle.set(e.target.value as 'in_app_only' | 'native_and_in_app' | 'native_only')
            }
          />

          <Toggle
            label="A Download Finished"
            description="Show a system notification when a download finishes. The first is shown at once, by name; more within the next ten seconds are summed up in one."
            checked={notifyComplete.value}
            onChange={notifyComplete.set}
          />
          <Toggle
            label="A Download Failed"
            description="Show a system notification when a download fails or finishes with warnings."
            checked={notifyFailed.value}
            onChange={notifyFailed.set}
          />
          <Toggle
            label="The Queue Finished"
            description="Show a system notification when everything in the queue is done, with how many downloaded and failed."
            checked={notifyQueue.value}
            onChange={notifyQueue.set}
          />
          <Toggle
            label="A Copied Link Was Found"
            description="Show a system notification when you copy an Apple Music link (needs Clipboard Monitoring)."
            checked={notifyClipboard.value}
            onChange={notifyClipboard.set}
          />

          {systemOn && (
            <div className="flex flex-col gap-1">
              <Button variant="secondary" icon={<Bell size={14} />} onClick={sendTest}>
                Send Test Notification
              </Button>
              <p className="text-xs text-content-tertiary">
                Sends one system notification now, using the choices on this screen, even
                before you save them.
              </p>
            </div>
          )}
        </>
      )}

      {/* Always shown: it governs messages inside the app, which appear
          whatever the switches above say. */}
      <Select
        label="Message Auto-Dismiss"
        description="How long plain messages inside the app stay before going away. Errors and warnings stay until you close them. The pointer resting on a message, or a message having keyboard focus, keeps it open."
        options={NOTIFICATION_DISMISS_OPTIONS}
        value={String(dismissSeconds.value ?? 5)}
        onChange={(e) => dismissSeconds.set(Number(e.target.value))}
      />
    </SettingsSection>
  );
}
