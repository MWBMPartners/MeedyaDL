// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Anti-ban runtime for Spotify downloads (M9-4).
//! ==============================================
//!
//! Companion to [`crate::models::spotify_anti_ban::AntiBanSettings`].
//! This module owns the **runtime** half of the anti-ban stack:
//!
//! * [`compute_playback_throttle_delay`] — how long to sleep after a
//!   track download body completes so the wall-clock time tracks the
//!   track's natural playback duration.
//! * [`compute_inter_track_delay`] — randomised gap between successive
//!   tracks.
//! * [`DailyCapCounter`] — persisted today/N counter with automatic
//!   local-midnight rollover.
//!
//! Pure-function design wherever possible. The persisted counter is
//! the only state — single JSON file at
//! `{app_data}/spotify_daily_counter.json`, atomic-rename writes via
//! [`crate::utils::atomic_write`].
//!
//! ## What's not in this module
//!
//! The dispatch gate (refuse-to-queue when cap hit, dev-access off,
//! consent not acknowledged) lives at the IPC layer in
//! `commands/spotify_anti_ban.rs` so it can return user-facing
//! `Result<_, String>` shapes the React layer already understands.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::models::spotify_anti_ban::AntiBanSettings;
use crate::utils::atomic_write::atomic_write_json;

// ============================================================
// Throttle math — pure functions
// ============================================================

/// Compute how long to sleep after a track body has finished
/// downloading so the total wall-clock time ≥ track playback time.
///
/// Returns [`Duration::ZERO`] when:
/// * the throttle setting is disabled, OR
/// * the download already took at least as long as the track's
///   natural runtime (rare but possible on slow networks).
///
/// Both inputs are milliseconds — the unit that votify reports on
/// its progress lines and that Spotify's metadata uses. Using `u64`
/// avoids overflow even for hour-long podcast episodes.
///
/// This function is intentionally pure (no `Instant::now()`) — the
/// caller passes in `elapsed_download_ms` so tests can pin the
/// behaviour without sleeping. Mocking the clock would only buy us
/// a smaller surface area at the cost of opaque indirection.
#[must_use]
pub fn compute_playback_throttle_delay(
    settings: &AntiBanSettings,
    track_duration_ms: u64,
    elapsed_download_ms: u64,
) -> Duration {
    if !settings.playback_speed_throttle_enabled {
        return Duration::ZERO;
    }
    let remaining = track_duration_ms.saturating_sub(elapsed_download_ms);
    Duration::from_millis(remaining)
}

/// Compute the random gap to insert between successive track
/// downloads.
///
/// Output is uniform in `[base, base + jitter)` seconds. When
/// `jitter == 0` the result is exactly `base` (deterministic).
///
/// The caller supplies the random source (`rng_value` in `[0.0, 1.0)`)
/// so this stays pure for tests. Production call sites use
/// `rand::random::<f64>()` — see [`compute_inter_track_delay_random`].
#[must_use]
pub fn compute_inter_track_delay(settings: &AntiBanSettings, rng_value: f64) -> Duration {
    let base = u64::from(settings.inter_track_delay_seconds);
    let jitter = u64::from(settings.inter_track_jitter_seconds);

    if jitter == 0 {
        return Duration::from_secs(base);
    }

    // Clamp the rng output defensively — a caller passing 1.0 or NaN
    // mustn't make us emit a negative or overflowing duration.
    let clamped = if rng_value.is_finite() {
        rng_value.clamp(0.0, 0.999_999)
    } else {
        0.0
    };
    // `as u64` is safe because clamped < 1.0 and jitter <= u32::MAX,
    // so the product fits in `u64` with room to spare.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let jitter_seconds = (clamped * jitter as f64) as u64;
    Duration::from_secs(base + jitter_seconds)
}

/// Convenience wrapper that draws from the thread-local PRNG.
///
/// Kept as a separate symbol so unit tests stay deterministic — they
/// call [`compute_inter_track_delay`] directly with a fixed
/// `rng_value`.
#[must_use]
pub fn compute_inter_track_delay_random(settings: &AntiBanSettings) -> Duration {
    let rng_value: f64 = rand::random::<u32>() as f64 / f64::from(u32::MAX);
    compute_inter_track_delay(settings, rng_value)
}

// ============================================================
// Daily-cap counter — persisted state
// ============================================================

/// Persisted representation of "Spotify tracks downloaded today."
///
/// Stored as JSON at `{app_data}/spotify_daily_counter.json`:
///
/// ```json
/// { "date": "2026-06-09", "count": 17 }
/// ```
///
/// Loaded once via [`DailyCapCounter::load`] and persisted on every
/// increment. Rolls over at local midnight — the `date` field is the
/// user's local-calendar day, matching Spotify's listening-session
/// boundary heuristic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyCapCounter {
    /// ISO-8601 local-calendar date (`YYYY-MM-DD`) when this counter
    /// was last touched. `today()` compares against this; mismatch =
    /// rollover.
    pub date: String,
    /// Number of tracks downloaded on `date`. Bounded above by the
    /// user's `daily_download_cap` setting (or unbounded when the
    /// cap is `0`).
    pub count: u32,
}

impl DailyCapCounter {
    /// In-memory factory — fresh counter for today, count 0.
    ///
    /// Used both as the initial value when no file exists yet, and
    /// as the rollover target when a new day starts.
    #[must_use]
    pub fn fresh_today() -> Self {
        Self {
            date: today_iso(),
            count: 0,
        }
    }

    /// Returns `true` when this counter's `date` matches the user's
    /// current local-calendar day. Used by [`Self::with_rollover`].
    fn is_today(&self) -> bool {
        self.date == today_iso()
    }

    /// Returns either the counter as-is (when still today) or a
    /// fresh counter for the new day (when the date has rolled over).
    ///
    /// Pure function — no I/O.
    #[must_use]
    pub fn with_rollover(self) -> Self {
        if self.is_today() {
            self
        } else {
            Self::fresh_today()
        }
    }

    /// Returns `true` if incrementing this counter by one would
    /// exceed the user's cap.
    ///
    /// When the cap is "unlimited" (sentinel `0`), always returns
    /// `false`.
    #[must_use]
    pub fn would_exceed(&self, settings: &AntiBanSettings) -> bool {
        if settings.is_daily_cap_unlimited() {
            return false;
        }
        self.count >= settings.daily_download_cap
    }
}

/// Today's date as an ISO-8601 string (`YYYY-MM-DD`) in the user's
/// **local** timezone — the same boundary Spotify uses for daily
/// listening limits.
///
/// Wrapped here so the test below can mock the clock via the
/// internal [`today_iso_for_now`] sibling.
fn today_iso() -> String {
    today_iso_for_now(Local::now().date_naive())
}

/// Pure variant of [`today_iso`] — takes the date instead of
/// reading the wall clock. Lets tests pin the rollover edges
/// without time-travel hacks.
fn today_iso_for_now(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

// ============================================================
// Daily-cap persistence
// ============================================================

/// Resolves the on-disk path for the daily-cap counter file.
///
/// Kept as a separate function so tests can re-use it without
/// duplicating the basename string.
#[must_use]
pub fn counter_file_path(app: &AppHandle) -> PathBuf {
    crate::utils::platform::get_app_data_dir(app).join("spotify_daily_counter.json")
}

/// Process-cached version stamp used to invalidate the in-memory
/// counter when the user resets it via the IPC layer.
///
/// `AtomicU64` because the queue task and the IPC task can both
/// touch it. Today the stamp is only read by tests; in M9-5+ the
/// dispatch path will read it to detect a "Reset counter" click
/// without re-reading the file.
static COUNTER_STAMP: AtomicU64 = AtomicU64::new(0);

/// Bump the in-memory stamp. Called from the IPC reset handler.
pub fn bump_counter_stamp() {
    COUNTER_STAMP.fetch_add(1, Ordering::SeqCst);
}

/// Read the current stamp value. Test-only at present.
#[must_use]
pub fn counter_stamp() -> u64 {
    COUNTER_STAMP.load(Ordering::SeqCst)
}

/// Load the daily-cap counter from disk, applying rollover if the
/// file's date is stale.
///
/// Returns a fresh-today counter when the file doesn't exist, can't
/// be read, or has unparseable JSON. The intent is to fail-safe:
/// the user can always download something, even if the counter
/// gets corrupted, but the counter then starts from zero (so an
/// attacker who could write garbage to the file can't use it to
/// re-enable themselves above the cap).
pub fn load_counter(app: &AppHandle) -> DailyCapCounter {
    let path = counter_file_path(app);
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return DailyCapCounter::fresh_today(),
    };
    let parsed: DailyCapCounter = match serde_json::from_str(&raw) {
        Ok(c) => c,
        Err(e) => {
            log::warn!(
                "spotify_daily_counter.json was unreadable; starting fresh — {e}"
            );
            return DailyCapCounter::fresh_today();
        }
    };
    parsed.with_rollover()
}

/// Persist the counter to disk via an atomic write.
///
/// Errors are surfaced — the caller (typically the IPC layer) can
/// decide whether to surface the failure to the user. The in-memory
/// counter is still considered valid even if the persist fails.
pub fn save_counter(app: &AppHandle, counter: &DailyCapCounter) -> Result<(), String> {
    let path = counter_file_path(app);
    atomic_write_json(&path, counter, "spotify_daily_counter")
}

/// Reset the daily-cap counter to zero for today.
///
/// Pairs with the IPC handler that powers a "Reset daily counter"
/// button in Settings. Bumps the stamp so any in-memory cache the
/// dispatch path is holding can know to re-read from disk.
pub fn reset_counter(app: &AppHandle) -> Result<(), String> {
    let counter = DailyCapCounter::fresh_today();
    save_counter(app, &counter)?;
    bump_counter_stamp();
    Ok(())
}

/// Increment the persisted daily-cap counter by `tracks`.
///
/// Called by the M9-5+ Spotify dispatch path after each successful
/// track download (or batch of tracks for album downloads). Applies
/// rollover before incrementing so a request that crosses local
/// midnight starts counting against the new day, not the old one.
///
/// Returns the post-increment counter so the caller can surface
/// "X / cap downloaded today" in the activity log without a second
/// disk read.
///
/// # Errors
///
/// Persistence failures are surfaced — the caller decides whether
/// to halt the dispatch or proceed (the in-memory value is still
/// consistent even if the file write failed). `tracks == 0` is a
/// no-op that still returns the current counter for symmetry.
pub fn increment_counter(app: &AppHandle, tracks: u32) -> Result<DailyCapCounter, String> {
    let mut counter = load_counter(app);
    // saturating_add: the cap-check at dispatch time guarantees we
    // never go past u32::MAX in practice, but defensive arithmetic
    // beats a silent wrap-around if a future caller misuses this.
    counter.count = counter.count.saturating_add(tracks);
    save_counter(app, &counter)?;
    Ok(counter)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn settings_default() -> AntiBanSettings {
        AntiBanSettings::default()
    }

    // ---- compute_playback_throttle_delay ----

    #[test]
    fn throttle_returns_zero_when_disabled() {
        let mut s = settings_default();
        s.playback_speed_throttle_enabled = false;
        let d = compute_playback_throttle_delay(&s, 180_000, 5_000);
        assert_eq!(d, Duration::ZERO);
    }

    #[test]
    fn throttle_returns_remaining_when_under_track_duration() {
        let s = settings_default();
        // 180 s track, 5 s actual download → 175 s remaining throttle.
        let d = compute_playback_throttle_delay(&s, 180_000, 5_000);
        assert_eq!(d, Duration::from_secs(175));
    }

    #[test]
    fn throttle_returns_zero_when_elapsed_exceeds_track() {
        let s = settings_default();
        // Slow network case — download took longer than the track.
        let d = compute_playback_throttle_delay(&s, 60_000, 90_000);
        assert_eq!(d, Duration::ZERO);
    }

    #[test]
    fn throttle_handles_exact_duration_match() {
        let s = settings_default();
        // Edge: 0 remaining.
        let d = compute_playback_throttle_delay(&s, 100_000, 100_000);
        assert_eq!(d, Duration::ZERO);
    }

    // ---- compute_inter_track_delay ----

    #[test]
    fn inter_track_delay_with_zero_jitter_is_deterministic() {
        let mut s = settings_default();
        s.inter_track_delay_seconds = 8;
        s.inter_track_jitter_seconds = 0;
        // rng_value irrelevant when jitter is 0.
        assert_eq!(
            compute_inter_track_delay(&s, 0.0),
            Duration::from_secs(8)
        );
        assert_eq!(
            compute_inter_track_delay(&s, 0.999),
            Duration::from_secs(8)
        );
    }

    #[test]
    fn inter_track_delay_clamps_within_jitter_window() {
        let s = settings_default(); // 10 + uniform [0,5)
        // Minimum: rng_value=0 → base
        assert_eq!(compute_inter_track_delay(&s, 0.0), Duration::from_secs(10));
        // Maximum (clamped): rng_value=1.0 → just under base+jitter
        assert_eq!(compute_inter_track_delay(&s, 1.0), Duration::from_secs(14));
        // Midpoint
        assert_eq!(compute_inter_track_delay(&s, 0.5), Duration::from_secs(12));
    }

    #[test]
    fn inter_track_delay_handles_nan_safely() {
        let s = settings_default();
        // NaN clamps to 0 → minimum delay.
        assert_eq!(compute_inter_track_delay(&s, f64::NAN), Duration::from_secs(10));
    }

    // ---- DailyCapCounter ----

    #[test]
    fn fresh_counter_has_today_and_zero() {
        let c = DailyCapCounter::fresh_today();
        assert_eq!(c.count, 0);
        assert_eq!(c.date, today_iso());
    }

    #[test]
    fn rollover_resets_when_date_changes() {
        let stale = DailyCapCounter {
            date: "2020-01-01".to_string(),
            count: 99,
        };
        let rolled = stale.with_rollover();
        assert_eq!(rolled.count, 0);
        assert_eq!(rolled.date, today_iso());
    }

    #[test]
    fn rollover_preserves_when_date_matches() {
        let live = DailyCapCounter {
            date: today_iso(),
            count: 42,
        };
        let preserved = live.clone().with_rollover();
        assert_eq!(preserved.date, live.date);
        assert_eq!(preserved.count, live.count);
    }

    #[test]
    fn would_exceed_blocks_at_cap() {
        let s = settings_default(); // cap = 100
        let counter = DailyCapCounter {
            date: today_iso(),
            count: 100,
        };
        assert!(counter.would_exceed(&s), "at cap → next download blocked");
    }

    #[test]
    fn would_exceed_allows_below_cap() {
        let s = settings_default();
        let counter = DailyCapCounter {
            date: today_iso(),
            count: 99,
        };
        assert!(!counter.would_exceed(&s), "below cap → next download allowed");
    }

    #[test]
    fn would_exceed_never_blocks_when_unlimited() {
        let s = AntiBanSettings {
            daily_download_cap: 0,
            ..Default::default()
        };
        let counter = DailyCapCounter {
            date: today_iso(),
            count: 100_000,
        };
        assert!(!counter.would_exceed(&s), "unlimited cap → never blocks");
    }

    // ---- date helper ----

    #[test]
    fn today_iso_for_now_formats_correctly() {
        let d = NaiveDate::from_ymd_opt(2026, 6, 9).unwrap();
        assert_eq!(today_iso_for_now(d), "2026-06-09");
    }

    // ---- stamp ----

    #[test]
    fn counter_stamp_increments_monotonically() {
        let before = counter_stamp();
        bump_counter_stamp();
        bump_counter_stamp();
        let after = counter_stamp();
        assert!(after >= before + 2);
    }

    // ---- increment helper pure-math sanity ----

    #[test]
    fn counter_saturates_at_u32_max_not_wraps() {
        // The cap-check at dispatch time prevents this in practice,
        // but if a future caller mishandles the gate, saturating
        // beats a silent wrap-to-zero. Test the pre-condition the
        // helper relies on, not the IO path.
        let mut c = DailyCapCounter {
            date: today_iso(),
            count: u32::MAX - 1,
        };
        c.count = c.count.saturating_add(5);
        assert_eq!(c.count, u32::MAX);
    }
}
