// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// macOS self-relocation offer modal (#1057).
// Offers to move MeedyaDL into an /Applications/MeedyaSuite folder so it
// lives alongside other MeedyaSuite apps. Shown at most once per install
// (or until the user declines) — the `relocation_declined` setting
// prevents re-prompting after "Not now".

import { useCallback, useState } from 'react';
import { Modal } from '@/components/common';
import { useUiStore } from '@/stores/uiStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { relocateAppBundle, setStoredPreference } from '@/lib/tauri-commands';

/**
 * Self-relocation offer modal shown on macOS when MeedyaDL detects it is
 * running from a bare `/Applications/MeedyaDL.app` install (see
 * `checkAppRelocation()` / `services::app_relocation` on the backend).
 */
export default function AppRelocationModal() {
  const show = useUiStore((s) => s.showAppRelocationPrompt);
  const destination = useUiStore((s) => s.appRelocationDestination);
  const [isMoving, setIsMoving] = useState(false);

  const handleDecline = useCallback(async () => {
    // Written to DISK. It used to go to the page's copy only, so "Not
    // now" was forgotten the moment the app closed and this prompt came
    // back at every single launch, for ever.
    useSettingsStore.getState().updateSettings({ relocation_declined: true });
    try {
      await setStoredPreference({ kind: 'relocation_declined', declined: true });
    } catch (err) {
      // Worth saying, because the cost of silence here is this prompt
      // coming back at every launch with no explanation — which is the
      // exact complaint the setting was added to fix.
      console.error('Could not remember that the move was declined:', err);
      useUiStore
        .getState()
        .addToast(
          'MeedyaDL could not remember that, so it will ask again next time you open it.',
          'error'
        );
    }
    useUiStore.getState().setShowAppRelocationPrompt(false);
  }, []);

  const handleMove = useCallback(async () => {
    setIsMoving(true);
    try {
      // On success the backend relaunches MeedyaDL from its new location
      // and exits this process — this call effectively never resolves
      // in that case, so there is nothing to do after a successful await.
      await relocateAppBundle();
    } catch (err) {
      // Relocation failed, or the user cancelled the OS admin-approval
      // prompt. MeedyaDL keeps running from its original location —
      // surface the failure and close the modal WITHOUT marking the
      // offer declined, so it can be retried on a later launch.
      console.warn('App relocation failed:', err);
      useUiStore
        .getState()
        .addToast(
          'Could not move MeedyaDL into the MeedyaSuite folder. You can try again next time you open the app.',
          'error'
        );
      setIsMoving(false);
      useUiStore.getState().setShowAppRelocationPrompt(false);
    }
  }, []);

  return (
    <Modal open={show} onClose={handleDecline} title="Tidy Up Your Applications Folder" maxWidth="max-w-md">
      <div className="space-y-4 text-sm text-content-secondary">
        <p>
          Move MeedyaDL into an <span className="font-medium">Applications/MeedyaSuite</span>{' '}
          folder, so it lives together with your other MeedyaSuite apps instead of loose in
          Applications?
        </p>
        {destination && (
          <p className="text-xs text-content-tertiary break-all">New location: {destination}</p>
        )}
        <p className="text-xs text-content-tertiary">
          You can always say no — MeedyaDL works exactly the same either way.
        </p>
        <div className="flex justify-end gap-3 pt-2">
          <button
            type="button"
            className="px-4 py-2 rounded-lg text-sm font-medium text-content-secondary hover:text-content-primary transition-colors disabled:opacity-50"
            onClick={handleDecline}
            disabled={isMoving}
          >
            Not now
          </button>
          <button
            type="button"
            className="px-4 py-2 rounded-lg text-sm font-medium bg-accent text-content-on-accent hover:opacity-90 transition-opacity disabled:opacity-50"
            onClick={handleMove}
            disabled={isMoving}
          >
            {isMoving ? 'Moving…' : 'Move MeedyaDL'}
          </button>
        </div>
      </div>
    </Modal>
  );
}
