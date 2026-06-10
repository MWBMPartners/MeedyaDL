// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file SpotifyConsentModal — first-run consent modal for Spotify downloads.
 *
 * Fires when the user tries to queue a Spotify URL and
 * `spotify_consent_acknowledged` is still `false`. Single-screen
 * design (per the M9-UI workflow's hybrid verdict) — names the
 * specific account-ban risk in plain English, recommends a
 * throwaway account, and requires an affirmative "I understand"
 * click before the queue accepts the URL.
 *
 * ## Design choices (from the UI workflow verdict)
 *
 * - **Single screen, not a three-step wizard.** The verdict
 *   rejected Design B's mid-flow per-knob opt-outs because they
 *   conflate "I accept the risk" with "I want to tune my
 *   safeguards" and silently mutate settings during consent.
 *   Anti-ban tuning lives in the Spotify Settings tab; consent is
 *   a yes/no question.
 * - **Default focus on the dismiss button.** An accidental Enter
 *   press dismisses rather than accepts. The accept action is
 *   "Acknowledge and queue downloads" — never the default action.
 * - **Effect-style aria-labels.** "Acknowledge Spotify
 *   account-ban risk and proceed with download" reads what the
 *   button DOES, not just what it's labelled.
 * - **Mixed-batch behaviour.** If the user pasted Apple + Spotify
 *   URLs and dismisses the consent modal, NO URLs queue (avoids
 *   the surprising "we queued half your URLs"). The dismiss toast
 *   tells them how to retry with only the non-Spotify URLs.
 * - **The IPC writes the consent flag.** Frontend does NOT also
 *   call `updateSettings({ spotify_consent_acknowledged: true })`
 *   — that would race the IPC's own settings save and trip the
 *   SHA-256 checksum-mismatch warning.
 *
 * @see acknowledge_spotify_consent (Rust IPC)
 * @see commands::spotify_anti_ban::DispatchGateOutcome
 */

import { useCallback, useEffect, useRef } from 'react';
import { AlertTriangle } from 'lucide-react';

import { Modal, Button } from '@/components/common';
import { useUiStore } from '@/stores/uiStore';
import { acknowledgeSpotifyConsent } from '@/lib/tauri-commands';
import { withErrorToast } from '@/lib/withErrorToast';

/**
 * Spotify-download first-run consent modal.
 *
 * Render once at the App level (alongside `<CrashReportOptInModal />`).
 * State lives on `useUiStore` — `showSpotifyConsent` toggles
 * visibility, `pendingSpotifyConsentCallback` carries the callback
 * fired on accept (typically a re-invocation of the download form's
 * preflight + submit flow).
 */
export default function SpotifyConsentModal() {
  const show = useUiStore((s) => s.showSpotifyConsent);
  const callback = useUiStore((s) => s.pendingSpotifyConsentCallback);
  const setShow = useUiStore.getState().setShowSpotifyConsent;
  const setCallback = useUiStore.getState().setPendingSpotifyConsentCallback;
  const addToast = useUiStore.getState().addToast;

  /** Default-focus target — the dismiss button per the UI verdict. */
  const dismissButtonRef = useRef<HTMLButtonElement>(null);

  /**
   * Set initial focus on the dismiss button when the modal opens.
   * An accidental Enter press should dismiss, not accept — the
   * accept action is the more dangerous of the two and must
   * require deliberate intent.
   */
  useEffect(() => {
    if (show) {
      // Delay so Modal's own focus management runs first.
      const timer = setTimeout(() => {
        dismissButtonRef.current?.focus();
      }, 50);
      return () => clearTimeout(timer);
    }
    return undefined;
  }, [show]);

  const handleAccept = useCallback(async () => {
    // Single source of truth — the IPC writes the flag and persists
    // settings.json atomically. Frontend does not double-write.
    const success = await withErrorToast(
      async () => {
        await acknowledgeSpotifyConsent();
        return true;
      },
      { errorMsg: 'Failed to record consent — please try again.' }
    );
    if (success) {
      // Capture the callback BEFORE clearing it; the closure persists
      // even after the store wipes the slot.
      const resume = callback;
      setShow(false);
      setPendingCallback(setCallback, null);
      if (resume) {
        // Re-run the preflight + submit flow. The Spotify dispatch
        // gate IPC will now return `Allowed` and the queue accepts
        // the URLs that triggered this modal.
        await resume();
      }
    }
  }, [callback, setShow, setCallback]);

  const handleDismiss = useCallback(() => {
    setShow(false);
    setPendingCallback(setCallback, null);
    addToast(
      'Cancelled — no downloads were queued. To queue only the non-Spotify URLs, remove the Spotify URLs from the textarea and submit again.',
      'info'
    );
  }, [setShow, setCallback, addToast]);

  return (
    <Modal
      open={show}
      onClose={handleDismiss}
      title="Spotify downloads — please read before continuing"
      maxWidth="max-w-lg"
    >
      <div className="space-y-4 text-sm text-content-secondary">
        <div className="flex gap-3 rounded-lg border border-status-warning/40 bg-status-warning/5 p-3">
          <AlertTriangle
            size={20}
            className="text-status-warning flex-shrink-0 mt-0.5"
            aria-hidden="true"
          />
          <div>
            <p className="font-medium text-content-primary">
              Spotify downloads carry an account-ban risk.
            </p>
            <p className="mt-1 text-xs">
              Spotify's terms of service prohibit automated downloads.
              Accounts that look like bots have been suspended — including
              cases reported as recently as late 2025. The risk is real but
              manageable with the right precautions.
            </p>
          </div>
        </div>
        <p>
          MeedyaDL shapes votify's traffic pattern to look more like a slow
          human listener: a real-time playback-speed throttle, randomised
          pauses between tracks, and a daily download cap. These defaults
          are designed to be safe for a hobbyist user. Disabling any of
          them in the Spotify Settings tab materially increases your risk.
        </p>
        <p>
          <span className="font-medium text-content-primary">Strongly recommended:</span>{' '}
          use a throwaway Spotify account, not your primary one. A
          suspended throwaway is an inconvenience; a suspended primary
          loses your playlists, follows, and listening history.
        </p>
        <p className="text-xs text-content-tertiary">
          This consent is recorded once. You can revoke it any time from
          Settings &gt; Services &gt; Spotify.
        </p>
        <div className="flex justify-end gap-3 pt-2">
          {/* Native <button> for the dismiss action so we can attach a
              ref for initial-focus targeting. Styled to mirror Button's
              `secondary` variant — kept inline rather than ref-forwarding
              the shared Button just for this one site. */}
          <button
            ref={dismissButtonRef}
            type="button"
            className="px-4 py-2 rounded-lg text-sm font-medium text-content-secondary hover:text-content-primary transition-colors"
            onClick={handleDismiss}
            aria-label="Cancel Spotify download and return to the download form"
          >
            Cancel
          </button>
          <Button
            variant="primary"
            onClick={handleAccept}
            aria-label="Acknowledge Spotify account-ban risk and proceed with download"
          >
            I understand — queue the download
          </Button>
        </div>
      </div>
    </Modal>
  );
}

/** Type helper for the pending callback setter; isolates the cast. */
function setPendingCallback(
  setter: (cb: (() => Promise<void>) | null) => void,
  value: (() => Promise<void>) | null
) {
  setter(value);
}
