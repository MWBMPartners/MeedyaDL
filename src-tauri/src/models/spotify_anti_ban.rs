// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Anti-ban settings model for Spotify downloads (M9-4).
//! =====================================================
//!
//! Spotify's terms of service prohibit automated downloads of streamed
//! audio, and accounts that exhibit obvious bot signatures (e.g. an
//! album downloaded in seconds when its play time is 45 minutes) have
//! been suspended in the wild — see the 2025-11 Zotify community
//! reports.
//!
//! MeedyaDL's response is a layered anti-ban stack — none of the
//! mitigations alone is sufficient; together they shape the
//! subprocess's traffic pattern to look like a slow human listener
//! rather than a fast script:
//!
//! 1. **Real-time playback-speed throttle**: never let the download
//!    finish faster than the track plays. Computed per-track from
//!    `track_duration_ms`.
//! 2. **Inter-track delay + jitter**: pause between successive
//!    downloads so a 30-track album doesn't look like 30 perfectly-
//!    spaced HTTP requests.
//! 3. **Daily download cap**: hard ceiling on tracks per day. The
//!    counter rolls over at the user's local midnight (which is what
//!    Spotify's own listening-session heuristics use).
//! 4. **Dev-access gate**: Spotify support is initially only visible
//!    when [`AppSettings::dev_access_enabled`] is `true` (Konami code
//!    unlock). Power users opt in; novice users don't trip the risk.
//! 5. **First-run consent modal**: even with dev access on, the first
//!    Spotify download requires explicit acknowledgement. Stored as
//!    [`AppSettings::spotify_consent_acknowledged`].
//!
//! ## What this module owns
//!
//! Just the **settings struct** + its defaults. The actual throttle
//! math, jitter generation, and daily-cap persistence live in
//! [`crate::services::spotify_anti_ban`].

use serde::{Deserialize, Serialize};

/// Anti-ban configuration for the Spotify download engine.
///
/// Stored as the `anti_ban` field on
/// [`crate::models::settings::SpotifySettings`]. Every knob is
/// designed to be safe-by-default — even users who never touch the
/// Settings UI get the protective behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AntiBanSettings {
    /// Master switch for the real-time playback-speed throttle.
    ///
    /// When `true` (default), each track download is slowed so it
    /// finishes no faster than the track's runtime. This is the
    /// flagship mitigation — disabling it removes the primary signal
    /// that hides MeedyaDL from Spotify's bot heuristics.
    ///
    /// Off-switch exists for power users running on a long-lived
    /// burner account who accept the risk in exchange for batch
    /// throughput. Not exposed by default in the Settings UI; only
    /// surfaces once the user has acknowledged the consent dialog.
    pub playback_speed_throttle_enabled: bool,

    /// Base seconds to wait between successive track downloads.
    ///
    /// Default: 10. Combined with [`Self::inter_track_jitter_seconds`]
    /// to produce a stochastic gap; an attacker pattern-matching on
    /// fixed intervals would catch a constant 10 s gap, so we add
    /// jitter as well.
    pub inter_track_delay_seconds: u32,

    /// Maximum random jitter to add on top of the base delay.
    ///
    /// Default: 5. The actual gap is sampled uniformly from
    /// `[base, base + jitter)`. Setting this to 0 disables the
    /// random component (don't — see the rationale on
    /// [`Self::inter_track_delay_seconds`]).
    pub inter_track_jitter_seconds: u32,

    /// Hard ceiling on Spotify tracks downloaded per local-calendar day.
    ///
    /// Default: 100. The counter resets at the user's local midnight
    /// (matching Spotify's own listening-session day boundary).
    ///
    /// `0` is a sentinel for "unlimited" — used by power users who
    /// have acknowledged the consent dialog AND want to run an
    /// overnight catch-up of a large library. The user has to
    /// affirmatively zero this; the default never resolves to it.
    pub daily_download_cap: u32,
}

impl Default for AntiBanSettings {
    fn default() -> Self {
        Self {
            // Throttle ON by default — the safer choice.
            playback_speed_throttle_enabled: true,
            // 10 s base + 5 s jitter → uniform [10, 15) s gap. Slow
            // enough to dodge "burst download" heuristics, fast
            // enough that a typical 12-track album finishes in
            // under 5 minutes of wall-clock waiting (most of which
            // is the real-time throttle anyway).
            inter_track_delay_seconds: 10,
            inter_track_jitter_seconds: 5,
            // 100/day is a sustainable rate for a hobbyist user —
            // about 30 albums — without looking obviously
            // automated. Users with bigger libraries can raise it
            // after the consent dialog confirms the risk.
            daily_download_cap: 100,
        }
    }
}

impl AntiBanSettings {
    /// Returns `true` when the daily cap is "unlimited" (cap = 0).
    ///
    /// Provided as a method (rather than `== 0` at the call site) so
    /// the contract for the sentinel value lives in one place — if a
    /// future PR ever swaps the sentinel to `u32::MAX`, only this
    /// function changes.
    #[must_use]
    pub fn is_daily_cap_unlimited(&self) -> bool {
        self.daily_download_cap == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safety_first() {
        let d = AntiBanSettings::default();
        // Critical: throttle ON.
        assert!(d.playback_speed_throttle_enabled);
        // Critical: cap finite (not unlimited).
        assert_ne!(d.daily_download_cap, 0);
        // Critical: inter-track gap > 0.
        assert!(d.inter_track_delay_seconds > 0);
    }

    #[test]
    fn unlimited_sentinel_recognised() {
        let s = AntiBanSettings {
            daily_download_cap: 0,
            ..Default::default()
        };
        assert!(s.is_daily_cap_unlimited());
        let s = AntiBanSettings {
            daily_download_cap: 1,
            ..Default::default()
        };
        assert!(!s.is_daily_cap_unlimited());
    }

    #[test]
    fn serde_round_trip_preserves_safety_settings() {
        let opts = AntiBanSettings::default();
        let json = serde_json::to_string(&opts).unwrap();
        let back: AntiBanSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.playback_speed_throttle_enabled, opts.playback_speed_throttle_enabled);
        assert_eq!(back.inter_track_delay_seconds, opts.inter_track_delay_seconds);
        assert_eq!(back.inter_track_jitter_seconds, opts.inter_track_jitter_seconds);
        assert_eq!(back.daily_download_cap, opts.daily_download_cap);
    }

    #[test]
    fn missing_fields_deserialise_to_safety_defaults() {
        // Forward-compat: an old `settings.json` with only some
        // fields populated must still produce safety-on defaults
        // for the missing ones. `#[serde(default)]` on the struct
        // handles this; the test pins the contract.
        let partial: AntiBanSettings = serde_json::from_str("{}").unwrap();
        assert!(partial.playback_speed_throttle_enabled);
        assert!(!partial.is_daily_cap_unlimited());
    }
}
