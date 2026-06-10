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
    /// User has selected `session_type = "desktop"` (for FLAC) but
    /// `spotify_dll_path` is unset or points to a file that doesn't
    /// exist. M9-5 / M9-6 — desktop session needs the Spotify
    /// Windows desktop DLL `1.2.88.483` as an external dependency
    /// the user supplies, and we'd rather fail the dispatch with a
    /// clear "go set the path in Settings" outcome than let votify
    /// fail mid-track with a confusing traceback.
    MissingSpotifyDll,
    /// User has selected `session_type = "web"` (for FLAC) but
    /// `wvd_path` is unset or points to a file that doesn't exist.
    /// M9-6 — the web session needs a Widevine `.wvd` file the
    /// user supplies. Acquiring a `.wvd` is non-trivial (extracted
    /// from rooted Android or via tools like pywidevine), and the
    /// React layer should surface this in the error copy so users
    /// understand the prerequisite isn't a MeedyaDL ship gap.
    MissingWvd,
    /// `daily_download_cap` has been hit. The counter rolls over
    /// at local midnight; the message says when. Carries the
    /// current counter so the React layer can render "100 / 100
    /// downloaded today" without a second IPC round-trip.
    DailyCapReached { count: u32, cap: u32 },
}

/// Pure gating logic — what the IPC handlers below decide once they
/// have the relevant state in hand. Pulled out for tests.
///
/// Evaluation order is deliberate:
///
/// 1. **dev_access** first, so users see the "unlock" path before
///    any later modal they don't have access to anyway.
/// 2. **consent** second, so the consent modal fires before any
///    session-artifact error suggests something the user can fix —
///    they haven't even agreed to download yet.
/// 3. **session-artifact** third (M9-5/M9-6 — `MissingSpotifyDll`
///    and `MissingWvd`). These check the configured session_type's
///    required external file before we waste a votify spawn.
/// 4. **cap** last — it's the most informational of the blockers,
///    and the others must be resolved first anyway.
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
    if let Some(missing) = check_session_artifacts(&settings.service_settings.spotify) {
        return missing;
    }
    if counter.would_exceed(&settings.service_settings.spotify.anti_ban) {
        return DispatchGateOutcome::DailyCapReached {
            count: counter.count,
            cap: settings.service_settings.spotify.anti_ban.daily_download_cap,
        };
    }
    DispatchGateOutcome::Allowed
}

/// Validate that the external file required by the selected
/// `session_type` is present on disk.
///
/// Returns:
///
/// * `None` when no validation is needed (session is `librespot` or
///   unset — both fall through to the free-tier path that doesn't
///   need an external artefact).
/// * `Some(MissingSpotifyDll)` when `session_type == "desktop"` but
///   `spotify_dll_path` is unset or points at a non-existent file.
/// * `Some(MissingWvd)` when `session_type == "web"` but `wvd_path`
///   is unset or points at a non-existent file.
///
/// Filesystem-touching but pure-ish: it only stats the configured
/// paths, never mutates anything. Tests inject the paths via the
/// settings object and the existence check uses
/// `Path::new(path).is_file()` — a deleted-mid-flight artefact
/// resurfaces as a clean error rather than a votify traceback.
///
/// ## Acquisition complexity (UI-facing context)
///
/// Both the desktop DLL and the Widevine `.wvd` are **user-supplied
/// external artefacts** — MeedyaDL doesn't ship them and can't
/// download them legally. The Settings UI surface should reflect
/// this: a brief "Where do I get this?" link rather than a generic
/// "Browse for file" affordance. The doc-comments on
/// `MissingSpotifyDll` and `MissingWvd` carry the user-facing copy
/// the React layer can render verbatim.
#[must_use]
pub fn check_session_artifacts(
    spotify: &crate::models::settings::SpotifySettings,
) -> Option<DispatchGateOutcome> {
    let session = spotify.session_type.as_deref().unwrap_or("librespot");
    match session {
        "desktop" => {
            let ok = spotify
                .spotify_dll_path
                .as_deref()
                .map(|p| !p.is_empty() && std::path::Path::new(p).is_file())
                .unwrap_or(false);
            if ok {
                None
            } else {
                Some(DispatchGateOutcome::MissingSpotifyDll)
            }
        }
        "web" => {
            let ok = spotify
                .wvd_path
                .as_deref()
                .map(|p| !p.is_empty() && std::path::Path::new(p).is_file())
                .unwrap_or(false);
            if ok {
                None
            } else {
                Some(DispatchGateOutcome::MissingWvd)
            }
        }
        // `librespot`, unset, or any future session type we haven't
        // taught the gate about → no artifact required at the
        // gate. votify itself will reject an unknown session type
        // with its own clear error, so we don't need to second-
        // guess here.
        _ => None,
    }
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

    // ---- M9-6: session-artifact gate (desktop DLL + Widevine .wvd) ----

    fn make_spotify_settings(
        session: Option<&str>,
        dll: Option<&str>,
        wvd: Option<&str>,
    ) -> crate::models::settings::SpotifySettings {
        crate::models::settings::SpotifySettings {
            cookies_path: None,
            session_type: session.map(String::from),
            spotify_dll_path: dll.map(String::from),
            wvd_path: wvd.map(String::from),
            anti_ban: crate::models::spotify_anti_ban::AntiBanSettings::default(),
        }
    }

    #[test]
    fn librespot_session_needs_no_artifact() {
        // The free-tier path doesn't need a DLL or .wvd file at all.
        let s = make_spotify_settings(Some("librespot"), None, None);
        assert_eq!(check_session_artifacts(&s), None);
    }

    #[test]
    fn unset_session_treated_as_librespot() {
        // `None` falls through to the default — same shape as the
        // votify CLI behaviour when `--session-type` isn't passed.
        let s = make_spotify_settings(None, None, None);
        assert_eq!(check_session_artifacts(&s), None);
    }

    #[test]
    fn desktop_session_without_dll_blocked() {
        let s = make_spotify_settings(Some("desktop"), None, None);
        assert_eq!(
            check_session_artifacts(&s),
            Some(DispatchGateOutcome::MissingSpotifyDll)
        );
    }

    #[test]
    fn desktop_session_with_empty_dll_path_blocked() {
        // Edge: settings field present but empty string. Still blocked.
        let s = make_spotify_settings(Some("desktop"), Some(""), None);
        assert_eq!(
            check_session_artifacts(&s),
            Some(DispatchGateOutcome::MissingSpotifyDll)
        );
    }

    #[test]
    fn desktop_session_with_nonexistent_dll_blocked() {
        let s = make_spotify_settings(
            Some("desktop"),
            Some("/definitely/does/not/exist/spotify.dll"),
            None,
        );
        assert_eq!(
            check_session_artifacts(&s),
            Some(DispatchGateOutcome::MissingSpotifyDll)
        );
    }

    #[test]
    fn desktop_session_with_real_file_passes() {
        // Use the cargo manifest as the "exists" file — guaranteed
        // to be present in the test environment without any setup.
        let manifest = env!("CARGO_MANIFEST_PATH");
        let s = make_spotify_settings(Some("desktop"), Some(manifest), None);
        assert_eq!(check_session_artifacts(&s), None);
    }

    #[test]
    fn web_session_without_wvd_blocked() {
        let s = make_spotify_settings(Some("web"), None, None);
        assert_eq!(
            check_session_artifacts(&s),
            Some(DispatchGateOutcome::MissingWvd)
        );
    }

    #[test]
    fn web_session_with_real_file_passes() {
        let manifest = env!("CARGO_MANIFEST_PATH");
        let s = make_spotify_settings(Some("web"), None, Some(manifest));
        assert_eq!(check_session_artifacts(&s), None);
    }

    #[test]
    fn unknown_session_type_falls_through_to_no_check() {
        // A future session_type votify adds (or a typo) doesn't
        // block the dispatch at the gate — votify itself raises a
        // clear error for unknown values, which is more useful than
        // our gate guessing.
        let s = make_spotify_settings(Some("future-session-type"), None, None);
        assert_eq!(check_session_artifacts(&s), None);
    }

    // ---- evaluate_dispatch_gate composes the new check correctly ----

    #[test]
    fn full_gate_runs_artifact_check_after_consent() {
        // User cleared dev + consent but selected the desktop
        // session without setting a DLL path. The gate should
        // surface MissingSpotifyDll, not skip to the cap check.
        let mut s = settings_with(true, true, 100);
        s.service_settings.spotify.session_type = Some("desktop".to_string());
        s.service_settings.spotify.spotify_dll_path = None;
        let c = counter_today(0);
        assert_eq!(
            evaluate_dispatch_gate(&s, &c),
            DispatchGateOutcome::MissingSpotifyDll
        );
    }

    #[test]
    fn full_gate_runs_artifact_check_before_cap() {
        // Even when the cap would block, the artifact error wins —
        // because we'd rather tell the user "your DLL is missing"
        // than "you're at the cap" for a download they can't
        // attempt anyway.
        let mut s = settings_with(true, true, 0); // 0 = unlimited
        s.service_settings.spotify.anti_ban.daily_download_cap = 100;
        s.service_settings.spotify.session_type = Some("web".to_string());
        s.service_settings.spotify.wvd_path = None;
        let c = counter_today(100); // would hit cap if cap-check ran
        assert_eq!(
            evaluate_dispatch_gate(&s, &c),
            DispatchGateOutcome::MissingWvd
        );
    }

    #[test]
    fn full_gate_allows_when_artifact_present_and_other_gates_clear() {
        let manifest = env!("CARGO_MANIFEST_PATH");
        let mut s = settings_with(true, true, 100);
        s.service_settings.spotify.session_type = Some("web".to_string());
        s.service_settings.spotify.wvd_path = Some(manifest.to_string());
        let c = counter_today(50);
        assert_eq!(evaluate_dispatch_gate(&s, &c), DispatchGateOutcome::Allowed);
    }

    // ---- discriminator serialization for the two new variants ----

    #[test]
    fn missing_artifact_outcomes_serialise_with_snake_case_discriminators() {
        let dll_json = serde_json::to_value(DispatchGateOutcome::MissingSpotifyDll).unwrap();
        assert_eq!(dll_json["kind"], "missing_spotify_dll");

        let wvd_json = serde_json::to_value(DispatchGateOutcome::MissingWvd).unwrap();
        assert_eq!(wvd_json["kind"], "missing_wvd");
    }
}
