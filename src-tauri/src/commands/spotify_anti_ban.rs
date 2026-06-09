// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Anti-ban IPC layer for Spotify downloads (M9-4).
//! ================================================
//!
//! Three IPC commands plus the dispatch gate helper:
//!
//! * [`acknowledge_spotify_consent`] — flips
//!   `AppSettings::spotify_consent_acknowledged` to `true`.
//!   Called by the React first-run modal once the user has clicked
//!   "I understand the risk."
//! * [`get_spotify_daily_cap_status`] — read-only snapshot of the
//!   today/cap counter for display in Settings > Services > Spotify
//!   and (eventually) in the queue toolbar.
//! * [`reset_spotify_daily_cap_counter`] — flips the persisted
//!   counter back to zero. Pairs with a "Reset" button in Settings.
//! * [`check_spotify_dispatch_allowed`] — pure gating function the
//!   download IPC will consume in M9-5/M9-6 when Spotify URLs start
//!   being accepted by `start_download`.
//!
//! The dispatch path itself is **not** wired here — M9-5 (desktop
//! session, FLAC) is when Spotify URLs first reach `start_download`,
//! and the gate function lands behind that flow.

use serde::Serialize;
use tauri::AppHandle;

use crate::services::config_service;
use crate::services::spotify_anti_ban;

// ============================================================
// Consent acknowledgment
// ============================================================

/// Flip `spotify_consent_acknowledged` to `true` and persist.
///
/// Idempotent — flipping an already-true flag is a no-op (still
/// returns `Ok(())`). Returns `Err` on settings load / save failure.
///
/// **Frontend caller:** `acknowledgeSpotifyConsent()` in
/// `src/lib/tauri-commands.ts`.
#[tauri::command]
pub async fn acknowledge_spotify_consent(app: AppHandle) -> Result<(), String> {
    let mut settings = config_service::load_settings(&app)
        .map_err(|e| format!("Failed to load settings: {e}"))?;
    if settings.spotify_consent_acknowledged {
        return Ok(());
    }
    settings.spotify_consent_acknowledged = true;
    config_service::save_settings(&app, &settings)
        .map_err(|e| format!("Failed to save settings: {e}"))?;
    log::info!("Spotify download consent acknowledged");
    Ok(())
}

// ============================================================
// Daily-cap status
// ============================================================

/// Snapshot of the current daily-cap counter.
///
/// Serialised to JSON for the React frontend. `cap == 0` is the
/// "unlimited" sentinel — the UI should render that as "Unlimited"
/// rather than literal "0".
#[derive(Debug, Clone, Serialize)]
pub struct DailyCapStatus {
    /// ISO-8601 local-calendar date the counter belongs to. The
    /// UI uses this to display "X / cap downloaded today (resets
    /// at midnight)."
    pub date: String,
    /// Tracks downloaded so far on `date`.
    pub count: u32,
    /// User's configured cap. `0` means unlimited.
    pub cap: u32,
    /// `true` when the next download would be blocked.
    pub at_cap: bool,
}

/// Read-only snapshot of the counter + the user's configured cap.
///
/// Returns a populated [`DailyCapStatus`] regardless of whether the
/// persisted file exists — when it doesn't, the response shows a
/// fresh-today counter with count 0.
#[tauri::command]
pub async fn get_spotify_daily_cap_status(app: AppHandle) -> Result<DailyCapStatus, String> {
    let settings = config_service::load_settings(&app)
        .map_err(|e| format!("Failed to load settings: {e}"))?;
    let counter = spotify_anti_ban::load_counter(&app);
    let cap = settings.service_settings.spotify.anti_ban.daily_download_cap;
    let at_cap = counter.would_exceed(&settings.service_settings.spotify.anti_ban);

    Ok(DailyCapStatus {
        date: counter.date,
        count: counter.count,
        cap,
        at_cap,
    })
}

/// Reset the persisted daily-cap counter to today/0.
///
/// Pairs with a "Reset" button in Settings > Services > Spotify.
/// Surfaces only when the user has the dev-access unlock — the button
/// itself is gated in the React layer, but a hostile caller could
/// still reach this command via the IPC bridge, so the handler
/// double-checks `dev_access_enabled`. Defence-in-depth, not
/// authentication; the keychain sentinel is the real auth layer.
#[tauri::command]
pub async fn reset_spotify_daily_cap_counter(app: AppHandle) -> Result<(), String> {
    let settings = config_service::load_settings(&app)
        .map_err(|e| format!("Failed to load settings: {e}"))?;
    if !settings.dev_access_enabled {
        return Err(
            "Resetting the Spotify daily-cap counter requires developer access. \
             Enable it from Settings > Advanced > Developer Tools first."
                .to_string(),
        );
    }
    spotify_anti_ban::reset_counter(&app)
}

// ============================================================
// Dispatch gate (consumed by M9-5+ when Spotify URLs accept)
// ============================================================

/// Outcome of the pre-dispatch gate for a Spotify download request.
///
/// `Allowed` = the queue may proceed.
/// Every other variant carries a user-facing explanation that the
/// React layer surfaces as a toast / modal.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DispatchGateOutcome {
    /// All gates cleared — caller may dispatch.
    Allowed,
    /// `dev_access_enabled` is `false`. Surfaced as the
    /// "Spotify is in active development" copy with the Konami
    /// unlock hint.
    DevAccessRequired,
    /// `dev_access_enabled` is `true` but the first-run consent
    /// modal hasn't been acknowledged yet. The React layer should
    /// show the modal and call `acknowledge_spotify_consent` once
    /// the user accepts.
    ConsentRequired,
    /// `daily_download_cap` has been hit. The counter rolls over
    /// at local midnight; the message says when. Carries the
    /// current counter so the React layer can render "100 / 100
    /// downloaded today" without a second IPC round-trip.
    DailyCapReached { count: u32, cap: u32 },
}

/// Pure gating logic — what the IPC handlers below decide once they
/// have the relevant state in hand. Pulled out for tests.
#[must_use]
pub fn evaluate_dispatch_gate(
    settings: &crate::models::settings::AppSettings,
    counter: &spotify_anti_ban::DailyCapCounter,
) -> DispatchGateOutcome {
    if !settings.dev_access_enabled {
        return DispatchGateOutcome::DevAccessRequired;
    }
    if !settings.spotify_consent_acknowledged {
        return DispatchGateOutcome::ConsentRequired;
    }
    if counter.would_exceed(&settings.service_settings.spotify.anti_ban) {
        return DispatchGateOutcome::DailyCapReached {
            count: counter.count,
            cap: settings.service_settings.spotify.anti_ban.daily_download_cap,
        };
    }
    DispatchGateOutcome::Allowed
}

/// IPC entry — used by the React layer to **preview** the gate's
/// answer before showing the download form's Spotify path.
///
/// `start_download` runs the same [`evaluate_dispatch_gate`] call
/// internally for every batch containing a Spotify URL (M9-5).
/// Frontend should call this preview when the form first detects a
/// Spotify URL so the appropriate modal (dev-access unlock /
/// consent / cap warning) fires before the user clicks "Add to
/// Queue."
#[tauri::command]
pub async fn check_spotify_dispatch_allowed(
    app: AppHandle,
) -> Result<DispatchGateOutcome, String> {
    let settings = config_service::load_settings(&app)
        .map_err(|e| format!("Failed to load settings: {e}"))?;
    let counter = spotify_anti_ban::load_counter(&app);
    Ok(evaluate_dispatch_gate(&settings, &counter))
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::AppSettings;
    use crate::services::spotify_anti_ban::DailyCapCounter;

    fn settings_with(
        dev: bool,
        consent: bool,
        cap: u32,
    ) -> AppSettings {
        let mut s = AppSettings::default();
        s.dev_access_enabled = dev;
        s.spotify_consent_acknowledged = consent;
        s.service_settings.spotify.anti_ban.daily_download_cap = cap;
        s
    }

    fn counter_today(count: u32) -> DailyCapCounter {
        let mut c = DailyCapCounter::fresh_today();
        c.count = count;
        c
    }

    #[test]
    fn gate_blocks_when_dev_access_off() {
        let s = settings_with(false, true, 100);
        let c = counter_today(0);
        assert_eq!(
            evaluate_dispatch_gate(&s, &c),
            DispatchGateOutcome::DevAccessRequired
        );
    }

    #[test]
    fn gate_blocks_when_consent_not_acknowledged() {
        let s = settings_with(true, false, 100);
        let c = counter_today(0);
        assert_eq!(
            evaluate_dispatch_gate(&s, &c),
            DispatchGateOutcome::ConsentRequired
        );
    }

    #[test]
    fn gate_blocks_at_cap() {
        let s = settings_with(true, true, 100);
        let c = counter_today(100);
        assert_eq!(
            evaluate_dispatch_gate(&s, &c),
            DispatchGateOutcome::DailyCapReached {
                count: 100,
                cap: 100,
            }
        );
    }

    #[test]
    fn gate_allows_when_all_clear() {
        let s = settings_with(true, true, 100);
        let c = counter_today(50);
        assert_eq!(evaluate_dispatch_gate(&s, &c), DispatchGateOutcome::Allowed);
    }

    #[test]
    fn gate_allows_under_unlimited_cap_even_at_high_count() {
        let s = settings_with(true, true, 0); // 0 = unlimited
        let c = counter_today(100_000);
        assert_eq!(evaluate_dispatch_gate(&s, &c), DispatchGateOutcome::Allowed);
    }

    #[test]
    fn gate_evaluates_dev_access_before_consent() {
        // Both blockers present — dev_access takes precedence so the
        // user sees the right modal copy ("unlock dev access first")
        // rather than the consent modal that they can't reach yet.
        let s = settings_with(false, false, 100);
        let c = counter_today(0);
        assert_eq!(
            evaluate_dispatch_gate(&s, &c),
            DispatchGateOutcome::DevAccessRequired
        );
    }

    #[test]
    fn gate_evaluates_consent_before_cap() {
        // User opted into dev access but hasn't seen the consent
        // modal yet — they shouldn't be told "you're at the cap"
        // for downloads they haven't even been allowed to attempt.
        let s = settings_with(true, false, 100);
        let c = counter_today(100);
        assert_eq!(
            evaluate_dispatch_gate(&s, &c),
            DispatchGateOutcome::ConsentRequired
        );
    }

    #[test]
    fn dispatch_gate_outcome_serialises_with_kind_discriminator() {
        // The React layer routes on `kind`; pin the discriminator
        // shape so renaming a variant in Rust doesn't silently
        // break the modal-selection logic.
        let json = serde_json::to_value(DispatchGateOutcome::DevAccessRequired).unwrap();
        assert_eq!(json["kind"], "dev_access_required");

        let cap = DispatchGateOutcome::DailyCapReached {
            count: 100,
            cap: 100,
        };
        let json = serde_json::to_value(cap).unwrap();
        assert_eq!(json["kind"], "daily_cap_reached");
        assert_eq!(json["count"], 100);
        assert_eq!(json["cap"], 100);
    }

    #[test]
    fn allowed_outcome_carries_no_payload() {
        // Sanity: the `Allowed` variant is shapeless. A future PR
        // adding fields here would force the React-side type union
        // update; this test makes that change visible.
        let json = serde_json::to_value(DispatchGateOutcome::Allowed).unwrap();
        assert_eq!(json["kind"], "allowed");
        // No additional fields beyond `kind`.
        assert_eq!(json.as_object().unwrap().len(), 1);
    }
}
