// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

/**
 * @file SpotifyTab.tsx — Spotify download settings tab (M9-UI).
 *
 * Surfaces the four user-tunable concerns of the M9 Spotify integration:
 *
 *   1. **Session** — `session_type` picker (librespot / desktop / web)
 *      plus the artefact paths each premium session needs
 *      (`spotify_dll_path` for desktop FLAC, `wvd_path` for web FLAC).
 *      The Widevine `.wvd` option stays first-class even though
 *      acquisition is non-trivial — non-Windows users have no other
 *      path to FLAC, and hiding it would silently regress them.
 *   2. **Anti-ban safeguards** — the four knobs from `AntiBanSettings`
 *      annotated with `<RiskPill />` so the consequence of changing
 *      each is legible. Destructive edits (throttle off, cap = 0)
 *      surface a `useConfirmation` modal before applying.
 *   3. **Daily cap status** — read-only snapshot of the persisted
 *      counter (today's count / cap, "resets at local midnight"),
 *      plus a "Reset counter" button gated on `dev_access_enabled`.
 *   4. **Acknowledgement** — read-only status of the consent flag and
 *      a "Revoke consent" button that prompts via useConfirmation and
 *      flips the flag back to false.
 *
 * ## Why the patch helpers (not useSettingsField)
 *
 * `useSettingsField<K>(key)` only operates on top-level `AppSettings`
 * keys. The Spotify surface lives at
 * `service_settings.spotify.{session_type, anti_ban.*, …}`. Rather
 * than introducing a parallel `useSpotifySettingsField` hook (which
 * Design B proposed and the UI verdict rejected as unnecessary
 * complexity), three-line `patchSpotify` / `patchAntiBan` helpers at
 * the top of the component body produce the same DX with no new
 * primitive to learn.
 *
 * @see SpotifyConsentModal — first-run consent flow (M9-UI)
 * @see RiskPill — three-tier consequence pill (M9-UI)
 */

import { useEffect, useState } from 'react';
import { ShieldOff, RotateCcw, AlertCircle } from 'lucide-react';

import {
  Button,
  FilePickerButton,
  Input,
  RiskPill,
  Select,
  SettingsSection,
  Toggle,
} from '@/components/common';
import { useConfirmation } from '@/lib/useConfirmation';
import { useSettingsStore } from '@/stores/settingsStore';
import {
  getSpotifyDailyCapStatus,
  resetSpotifyDailyCapCounter,
  type DailyCapStatus,
} from '@/lib/tauri-commands';
import { withErrorToast } from '@/lib/withErrorToast';
import { useUiStore } from '@/stores/uiStore';
import type { SpotifyServiceSettings, AntiBanSettings } from '@/types';

/**
 * Default-shaped Spotify settings block. Mirrors the Rust
 * `SpotifySettings::default()` so an absent / missing settings
 * object on first render produces sane values that don't get
 * overwritten on save with the Rust defaults.
 */
const DEFAULT_SPOTIFY: SpotifyServiceSettings = {
  cookies_path: null,
  session_type: 'librespot',
  spotify_dll_path: null,
  wvd_path: null,
  anti_ban: {
    playback_speed_throttle_enabled: true,
    inter_track_delay_seconds: 10,
    inter_track_jitter_seconds: 5,
    daily_download_cap: 100,
  },
};

export function SpotifyTab() {
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  // Locally-resolved current Spotify settings — falls back to
  // DEFAULT_SPOTIFY when the optional service_settings.spotify slot
  // is undefined (e.g. on a freshly-installed app whose settings
  // file predates the M9-1 schema bump).
  const spotify: SpotifyServiceSettings =
    settings.service_settings?.spotify ?? DEFAULT_SPOTIFY;
  const antiBan: AntiBanSettings = spotify.anti_ban ?? DEFAULT_SPOTIFY.anti_ban;

  /**
   * Patch the Spotify service block. The verdict rejected a custom
   * `useSpotifySettingsField` hook in favour of this small helper at
   * the component-body level — no new abstraction, type-safe via the
   * existing `SpotifyServiceSettings` interface.
   */
  const patchSpotify = (patch: Partial<SpotifyServiceSettings>) => {
    updateSettings({
      service_settings: {
        ...settings.service_settings,
        spotify: { ...spotify, ...patch },
      },
    });
  };

  /** Patch the nested anti-ban block. */
  const patchAntiBan = (patch: Partial<AntiBanSettings>) => {
    patchSpotify({ anti_ban: { ...antiBan, ...patch } });
  };

  // Daily-cap status — fetched on mount + on window focus so the
  // count is fresh while the user is looking at the tab.
  const [capStatus, setCapStatus] = useState<DailyCapStatus | null>(null);
  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      const result = await withErrorToast(
        async () => getSpotifyDailyCapStatus(),
        { errorMsg: "MeedyaDL could not read today's Spotify download count. Open this tab again to retry." }
      );
      if (!cancelled && result) {
        setCapStatus(result);
      }
    };
    void load();
    const handleFocus = () => void load();
    window.addEventListener('focus', handleFocus);
    return () => {
      cancelled = true;
      window.removeEventListener('focus', handleFocus);
    };
  }, []);

  // ---- Destructive-edit confirmations (verdict amendment 3) ----

  const confirmDisableThrottle = useConfirmation({
    title: 'Disable playback-speed throttle?',
    description: (
      <div className="space-y-2 text-sm text-content-secondary">
        <p>
          <span className="font-medium text-content-primary">
            This is the most important account-safety setting.
          </span>{' '}
          Without it, a whole album can download in seconds, which is
          the clearest sign to Spotify that a program, not a person,
          is listening.
        </p>
        <p>
          Leaving it on is the safer choice. Only turn it off on an
          account you can afford to lose, and accept that Spotify is
          then more likely to suspend it.
        </p>
      </div>
    ),
    confirmLabel: 'Disable throttle anyway',
    onConfirm: () => patchAntiBan({ playback_speed_throttle_enabled: false }),
  });

  const confirmZeroCap = useConfirmation({
    title: 'Remove the daily download cap?',
    description: (
      <div className="space-y-2 text-sm text-content-secondary">
        <p>
          <span className="font-medium text-content-primary">
            Setting the cap to 0 removes every per-day download limit.
          </span>{' '}
          A 500-track day looks much more like a bot than a 100-track
          day.
        </p>
        <p>
          Most users should keep the default of 100. Raise it to 200
          or 300 if you need headroom — don't zero it out.
        </p>
      </div>
    ),
    confirmLabel: 'Remove cap anyway',
    onConfirm: () => patchAntiBan({ daily_download_cap: 0 }),
  });

  // ---- Revoke consent confirmation (verdict amendment 4) ----

  const confirmRevoke = useConfirmation({
    title: 'Revoke Spotify consent?',
    description: (
      <div className="space-y-2 text-sm text-content-secondary">
        <p>
          The consent prompt will appear again the next time you
          queue a Spotify URL.
        </p>
        <p className="text-xs text-content-tertiary">
          Active downloads are not cancelled — only future Spotify
          queueing requires re-acknowledgement.
        </p>
      </div>
    ),
    confirmLabel: 'Revoke',
    onConfirm: () => updateSettings({ spotify_consent_acknowledged: false }),
  });

  // ---- Daily-cap reset confirmation (dev-access gated) ----

  const confirmResetCounter = useConfirmation({
    title: 'Reset Spotify daily counter?',
    description: (
      <div className="space-y-2 text-sm text-content-secondary">
        <p>
          The daily counter is intended as a fixed safety boundary.
          Resetting it mid-day defeats the purpose of the cap —
          use this only when you understand the risk.
        </p>
      </div>
    ),
    confirmLabel: 'Reset counter',
    onConfirm: async () => {
      await withErrorToast(
        async () => {
          await resetSpotifyDailyCapCounter();
          // Re-fetch the snapshot so the UI reflects the reset
          // without waiting for the next window focus.
          const next = await getSpotifyDailyCapStatus();
          setCapStatus(next);
          useUiStore.getState().addToast('Spotify daily counter reset', 'success');
        },
        { errorMsg: 'MeedyaDL could not reset the Spotify download count. Try again in a moment.' }
      );
    },
  });

  return (
    <div className="space-y-3">
      {/* ================================================================ */}
      {/* Section 1: Session                                               */}
      {/* ================================================================ */}
      <SettingsSection
        title="Session"
        description="How MeedyaDL signs in to Spotify. The free option works without Premium but downloads Ogg Vorbis only; the other two need Premium and a file you supply yourself."
      >
        <Select
          label="Session type"
          value={spotify.session_type ?? 'librespot'}
          onChange={(e) =>
            patchSpotify({ session_type: e.target.value || null })
          }
          options={[
            { value: 'librespot', label: 'Free account — Ogg Vorbis only' },
            {
              value: 'desktop',
              label: 'Premium, Windows only — FLAC (needs the Spotify app’s DLL file)',
            },
            { value: 'web', label: 'Premium, any computer — FLAC (needs a Widevine .wvd file)' },
          ]}
        />
        {spotify.session_type === 'desktop' && (
          <FilePickerButton
            label="Spotify desktop DLL"
            description="The DLL file from the Spotify app for Windows, which you supply; MeedyaDL does not include it. Tested with Spotify 1.2.88.483."
            value={spotify.spotify_dll_path}
            onChange={(v) => patchSpotify({ spotify_dll_path: v })}
            filters={[
              { name: 'Spotify DLL', extensions: ['dll', 'exe'] },
            ]}
          />
        )}
        {spotify.session_type === 'web' && (
          <FilePickerButton
            label="Widevine .wvd"
            description="A Widevine device file (.wvd), which you supply. Needed for FLAC on a Mac or Linux. How to get one is deliberately not explained in the app; see the project wiki."
            value={spotify.wvd_path}
            onChange={(v) => patchSpotify({ wvd_path: v })}
            filters={[
              { name: 'Widevine device', extensions: ['wvd'] },
            ]}
          />
        )}
        <FilePickerButton
          label="Cookies file (optional)"
          description="A cookies file exported from your browser while signed in to Spotify. Optional: leave it empty unless Spotify downloads fail to sign in."
          value={spotify.cookies_path}
          onChange={(v) => patchSpotify({ cookies_path: v })}
          filters={[{ name: 'Cookies', extensions: ['txt'] }]}
        />
      </SettingsSection>

      {/* ================================================================ */}
      {/* Section 2: Anti-ban safeguards                                   */}
      {/* ================================================================ */}
      <SettingsSection
        title="Account Safety"
        description="These settings slow downloads to a listening pace, so Spotify is less likely to suspend your account. The defaults are the safe choice; turn them off only on an account you can afford to lose."
      >
        <div className="flex items-center justify-between gap-3">
          <div className="flex-1">
            <Toggle
              label="Real-time playback-speed throttle"
              description="Each track takes at least as long to download as it does to play. This is the most important of these settings."
              checked={antiBan.playback_speed_throttle_enabled}
              onChange={(checked) => {
                if (checked) {
                  // Re-enabling is always safe — no confirmation.
                  patchAntiBan({ playback_speed_throttle_enabled: true });
                } else {
                  confirmDisableThrottle.open();
                }
              }}
            />
          </div>
          <RiskPill tier="highest" />
        </div>
        {confirmDisableThrottle.modal}

        <div className="flex items-center justify-between gap-3">
          <div className="flex-1">
            <Input
              label="Inter-track delay (seconds)"
              description="Base seconds to wait between successive track downloads. Combined with the jitter below."
              type="number"
              min={0}
              max={600}
              step={1}
              value={antiBan.inter_track_delay_seconds.toString()}
              onChange={(e) =>
                patchAntiBan({
                  inter_track_delay_seconds: e.target.value
                    ? parseInt(e.target.value, 10)
                    : 0,
                })
              }
            />
          </div>
          <RiskPill tier="lower" />
        </div>

        <div className="flex items-center justify-between gap-3">
          <div className="flex-1">
            <Input
              label="Inter-track jitter (seconds)"
              description="Up to this many extra seconds, chosen at random, on top of the pause above. Not recommended at 0: pauses of exactly the same length are an obvious sign of a program."
              type="number"
              min={0}
              max={600}
              step={1}
              value={antiBan.inter_track_jitter_seconds.toString()}
              onChange={(e) =>
                patchAntiBan({
                  inter_track_jitter_seconds: e.target.value
                    ? parseInt(e.target.value, 10)
                    : 0,
                })
              }
            />
          </div>
          <RiskPill tier="lower" />
        </div>

        <div className="flex items-center justify-between gap-3">
          <div className="flex-1">
            <Input
              label="Daily download cap"
              description="Maximum tracks per local-calendar day. Resets at midnight. Set to 0 to remove the cap entirely (confirmation required)."
              type="number"
              min={0}
              max={100000}
              step={1}
              value={antiBan.daily_download_cap.toString()}
              onChange={(e) => {
                const next = e.target.value ? parseInt(e.target.value, 10) : 0;
                if (next === 0 && antiBan.daily_download_cap !== 0) {
                  confirmZeroCap.open();
                } else {
                  patchAntiBan({ daily_download_cap: next });
                }
              }}
            />
          </div>
          <RiskPill tier="highest" />
        </div>
        {confirmZeroCap.modal}
      </SettingsSection>

      {/* ================================================================ */}
      {/* Section 3: Daily cap status                                      */}
      {/* ================================================================ */}
      <SettingsSection
        title="Daily Cap Status"
        description="Live snapshot of today's Spotify download counter. The counter resets at local midnight to match Spotify's listening-session day boundary."
      >
        {capStatus ? (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-sm text-content-secondary">Today</span>
              <span className="text-sm font-medium text-content-primary">
                {capStatus.date}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-content-secondary">Tracks counted</span>
              <span
                className={`text-sm font-medium ${
                  capStatus.at_cap ? 'text-status-error-text' : 'text-content-primary'
                }`}
              >
                {capStatus.count} /{' '}
                {capStatus.cap === 0 ? 'Unlimited' : capStatus.cap}
              </span>
            </div>
            {capStatus.at_cap && (
              <div className="flex gap-2 rounded-platform-lg border border-status-error/40 bg-status-error/5 p-2 text-xs text-status-error-text">
                <AlertCircle size={14} className="flex-shrink-0 mt-0.5" aria-hidden="true" />
                <span>
                  Daily cap reached — new Spotify downloads will be blocked
                  until the counter resets at local midnight.
                </span>
              </div>
            )}
            {settings.dev_access_enabled && (
              <div className="pt-2">
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={confirmResetCounter.open}
                  icon={<RotateCcw size={14} />}
                >
                  Reset counter (developer)
                </Button>
                {confirmResetCounter.modal}
              </div>
            )}
          </div>
        ) : (
          <p className="text-sm text-content-tertiary">Loading…</p>
        )}
      </SettingsSection>

      {/* ================================================================ */}
      {/* Section 4: Acknowledgement                                       */}
      {/* ================================================================ */}
      <SettingsSection
        title="Risk Acknowledgement"
        description="Records whether you've accepted the Spotify account-ban-risk consent. Revoking returns you to the first-run modal on the next Spotify queue attempt."
      >
        <div className="flex items-center justify-between">
          <div className="text-sm">
            <p className="text-content-primary">
              {settings.spotify_consent_acknowledged
                ? 'Acknowledged — Spotify downloads enabled.'
                : 'Not acknowledged — consent modal will appear on next Spotify queue attempt.'}
            </p>
            <p className="mt-1 text-xs text-content-tertiary">
              {settings.spotify_consent_acknowledged
                ? 'You can revoke this at any time.'
                : 'Queueing any Spotify URL will trigger the consent flow.'}
            </p>
          </div>
          {settings.spotify_consent_acknowledged && (
            <Button
              variant="secondary"
              size="sm"
              onClick={confirmRevoke.open}
              icon={<ShieldOff size={14} />}
            >
              Revoke consent
            </Button>
          )}
        </div>
        {confirmRevoke.modal}
      </SettingsSection>
    </div>
  );
}
