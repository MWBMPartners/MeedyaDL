// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! External queue watchdog (#818).
//!
//! A separate top-level tokio task — owned by the Tauri runtime, NOT
//! spawned by the queue processor — that polls every 60 s and
//! escalates queue items whose progress has stalled.
//!
//! ## Why an external watchdog?
//!
//! The existing safety nets (`compute_completion_timeout` per #461 /
//! #579, the cooperative-cancel flag per #663, the heartbeat ticker
//! per #805) all live INSIDE the same task tree as the work they're
//! meant to guard. When the parent task dies silently (panic / drop /
//! unawaited future), every safety net dies with it. The user's
//! 2026-05-18 stuck-download (#815) showed exactly this: 1h 50m of
//! complete silence after `Companion tier 4: all codecs exhausted`,
//! no heartbeat, no timeout firing — the task was simply gone.
//!
//! This module's task is owned by the Tauri runtime root, so a hang
//! in the queue processor can never take it down.
//!
//! ## Activity signal
//!
//! For each item in `Downloading` or `Processing` state, the watchdog
//! snapshots a tuple of "things that change when work is happening":
//!
//! - `progress` (overall %)
//! - `processing_progress` (intra-Processing %)
//! - `processing_label` (current stage caption)
//! - `current_track` (which track is downloading)
//! - `completed_tracks` (Track N of M counter)
//!
//! If the tuple is **bit-identical** across two consecutive polls
//! `STALL_WARN_SECS` apart, the item is considered stalled.
//!
//! ## Escalation cascade
//!
//! 1. First detection of stall (`STALL_WARN_SECS = 600 s` = 10 min):
//!    WARN to the activity log so the user sees the watchdog
//!    detected something.
//! 2. Continued stall past `STALL_HARD_SECS = 1200 s` = 20 min:
//!    transition the item to `Error` state via `set_error` (which
//!    honours the #661 terminal-state guard so already-complete
//!    items aren't poisoned), release the queue slot via
//!    `on_task_finished`, and re-trigger `process_queue` to start
//!    the next item.
//!
//! Users keep their work-in-progress files on disk — only the queue
//! entry is marked failed. They can manually retry from the History.
//!
//! ## Not in scope (Phase 1)
//!
//! - Auto-cancelling the underlying GAMDL subprocess. The parent
//!   task that spawned it is gone, so we can't await its handle;
//!   any orphaned subprocess will get reaped by `kill_on_drop(true)`
//!   when the OS notices the parent exit. Future work could expose
//!   a per-item PID map for explicit kill.
//! - User-configurable thresholds. Hard-coded 10 / 20 min for now;
//!   add settings keys in a follow-up if power users ask.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::models::download::{DownloadState, QueueItemStatus};
use crate::services::download_queue::{DownloadQueue, ShutdownSignal};
use crate::utils::activity_log::{emit_download_log, get_activity_count};

/// Polling cadence. Short enough that a stalled item is noticed
/// within ~1 min, long enough that the watchdog itself is
/// effectively free.
const POLL_INTERVAL: Duration = Duration::from_secs(60);

/// How long an item's progress tuple has to remain bit-identical
/// before the watchdog emits a WARN.
const STALL_WARN_SECS: u64 = 600; // 10 min

/// How long an item's progress tuple has to remain bit-identical
/// before the watchdog escalates to `set_error` + release-slot.
const STALL_HARD_SECS: u64 = 1200; // 20 min

/// Snapshot of an item's progress signal. `PartialEq` derive lets us
/// compare two snapshots bit-identically.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProgressSnapshot {
    progress_bucket: i64,
    processing_progress_bucket: i64,
    processing_label: Option<String>,
    current_track: Option<String>,
    completed_tracks: Option<usize>,
    /// Activity-log event counter for this download (#846). The
    /// QueueItemStatus alone misses long-running passes that don't
    /// touch the bar/label (e.g. the once-per-item companion lyrics
    /// conversion from #843, which emits 100s of "converted N TTML"
    /// events without changing any of the fields above) and the
    /// watchdog used to false-positive on them. Bumped from inside
    /// `utils::activity_log::emit_inner` /
    /// `emit_subprocess_line` so any activity-log emission for the
    /// item changes the snapshot.
    activity_count: u64,
}

impl ProgressSnapshot {
    fn from(item: &QueueItemStatus) -> Self {
        // Bucket floating-point progress into deci-percent integers
        // so trivial float-noise jitter doesn't reset the stall
        // detection. `processing_progress` is 0.0–1.0; round to
        // 1000ths. `progress` is 0.0–100.0; round to tenths.
        Self {
            progress_bucket: (item.progress * 10.0).round() as i64,
            processing_progress_bucket: item
                .processing_progress
                .map(|p| (p * 1000.0).round() as i64)
                .unwrap_or(-1),
            processing_label: item.processing_label.clone(),
            current_track: item.current_track.clone(),
            completed_tracks: item.completed_tracks,
            activity_count: get_activity_count(&item.id),
        }
    }
}

/// Per-item watchdog state. Owned by the watchdog task; not exposed
/// outside the module.
#[derive(Debug)]
struct ItemState {
    snapshot: ProgressSnapshot,
    /// Wall-clock time the snapshot was first observed unchanged.
    /// Reset whenever the snapshot changes.
    stall_started_at: Instant,
    /// Whether we've already emitted the 10-min WARN for this stall
    /// (so we don't spam the activity log every poll cycle).
    warn_emitted: bool,
}

/// Public entry point — spawn the watchdog task. Call once from the
/// Tauri `.setup()` closure. The task runs until the shutdown signal
/// fires.
///
/// **Important (#827):** must use `tauri::async_runtime::spawn`, NOT
/// `tokio::spawn`. On macOS the Tauri setup closure runs from the
/// AppKit main thread inside `applicationDidFinishLaunching:` BEFORE
/// the Tauri-managed Tokio runtime is registered as the *current*
/// runtime, so `tokio::spawn` → `Handle::current()` panics. The panic
/// crosses the `extern "C"` FFI boundary that tao uses to bridge into
/// AppKit, which Rust can't unwind across, so it aborts the entire
/// process. `tauri::async_runtime::spawn` looks up Tauri's runtime
/// handle directly and is safe to call from any thread.
///
/// Same convention as `setup_queue_recovery()` in lib.rs — see the
/// CLAUDE.md "Queue persistence" section.
pub fn spawn_queue_watchdog(
    app: AppHandle,
    queue: Arc<Mutex<DownloadQueue>>,
    shutdown: Arc<ShutdownSignal>,
) {
    tauri::async_runtime::spawn(async move {
        log::info!("queue watchdog started — polling every {:?}", POLL_INTERVAL);
        let mut tracked: HashMap<String, ItemState> = HashMap::new();
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if shutdown.is_triggered() {
                log::info!("queue watchdog exiting — shutdown signal");
                return;
            }
            poll_once(&app, &queue, &mut tracked).await;
        }
    });
}

/// One iteration of the polling loop. Pulled out for testability.
async fn poll_once(
    app: &AppHandle,
    queue: &Arc<Mutex<DownloadQueue>>,
    tracked: &mut HashMap<String, ItemState>,
) {
    // Snapshot the queue under a brief lock — release before doing
    // anything that might take time. `get_status()` already clones
    // every QueueItemStatus, so we drop the lock immediately.
    let items_snapshot: Vec<(String, DownloadState, ProgressSnapshot)> = {
        let q = queue.lock().await;
        q.get_status()
            .into_iter()
            .filter(|s| {
                matches!(
                    s.state,
                    DownloadState::Downloading | DownloadState::Processing
                )
            })
            .map(|s| {
                let snap = ProgressSnapshot::from(&s);
                (s.id, s.state, snap)
            })
            .collect()
    };

    // Forget any items that are no longer active. Stale entries would
    // never get matched again and just leak.
    let active_ids: std::collections::HashSet<String> =
        items_snapshot.iter().map(|(id, _, _)| id.clone()).collect();
    tracked.retain(|id, _| active_ids.contains(id));

    let now = Instant::now();
    for (id, _state, snapshot) in items_snapshot {
        match tracked.get_mut(&id) {
            Some(state) if state.snapshot == snapshot => {
                // Snapshot unchanged — keep counting the stall.
                let stalled_for = now.duration_since(state.stall_started_at);
                let secs = stalled_for.as_secs();
                if !state.warn_emitted && secs >= STALL_WARN_SECS {
                    let mins = secs / 60;
                    log::warn!(
                        "watchdog: queue item {id} has had no progress signal for {mins} min — flagging"
                    );
                    emit_download_log(
                        app,
                        &id,
                        &format!(
                            "⚠ Watchdog: no progress signal for {mins} min — item may be stuck. Will auto-fail after {} min total.",
                            STALL_HARD_SECS / 60
                        ),
                    );
                    state.warn_emitted = true;
                    // #851: the emit_download_log call above bumps the
                    // per-download activity counter via #846. Without
                    // accounting for our own bump, the next poll would
                    // observe snapshot.activity_count != tracked.snapshot
                    // .activity_count, mistake the watchdog's own warn
                    // for genuine progress, reset the stall timer, and
                    // warn again 10 min later — repeating indefinitely
                    // instead of escalating to auto-fail at 20 min.
                    state.snapshot.activity_count = get_activity_count(&id);
                }
                if secs >= STALL_HARD_SECS {
                    let mins = secs / 60;
                    log::error!(
                        "watchdog: queue item {id} stalled for {mins} min — escalating to Error + releasing slot"
                    );
                    emit_download_log(
                        app,
                        &id,
                        &format!(
                            "⛔ Watchdog: stalled for {mins} min — marking failed + advancing queue. Files already on disk are preserved."
                        ),
                    );
                    escalate_to_error(app, queue, &id).await;
                    // Drop the entry so we don't re-escalate on the
                    // next poll.
                    tracked.remove(&id);
                }
            }
            Some(state) => {
                // Snapshot changed — work is happening. Reset the
                // stall timer + clear the warn flag.
                state.snapshot = snapshot;
                state.stall_started_at = now;
                state.warn_emitted = false;
            }
            None => {
                // First sighting — start tracking.
                tracked.insert(
                    id,
                    ItemState {
                        snapshot,
                        stall_started_at: now,
                        warn_emitted: false,
                    },
                );
            }
        }
    }
}

/// Escalation action — transition the item to Error and release the
/// queue slot so the next item can run. Honours #661's terminal-state
/// guard (which is already inside `set_error`), so a race where the
/// item legitimately completes between the poll and the escalation
/// is harmless — the guard short-circuits.
async fn escalate_to_error(
    app: &AppHandle,
    queue: &Arc<Mutex<DownloadQueue>>,
    dl_id: &str,
) {
    let mut q = queue.lock().await;
    q.set_error(
        dl_id,
        "Queue watchdog detected a stalled download (no progress for >20 min). Marked failed automatically to keep the queue moving. Already-downloaded files on disk are preserved — retry from History to resume.",
    );
    // Best-effort slot release. If the slot was already returned by
    // a legitimate completion racing the watchdog, `on_task_finished`
    // is idempotent.
    q.on_task_finished();
    let _ = app.emit_to_all_with_str("queue-updated", "watchdog-escalated");
}

/// Local trait extension so the watchdog can fire the same Tauri
/// event the queue processor emits. Tauri's `Manager::emit` requires
/// a serializable payload; the watchdog just needs a fire-and-forget
/// notification, hence this tiny shim.
trait WatchdogEmit {
    fn emit_to_all_with_str(&self, event: &str, payload: &str) -> Result<(), String>;
}

impl WatchdogEmit for AppHandle {
    fn emit_to_all_with_str(&self, event: &str, payload: &str) -> Result<(), String> {
        use tauri::Emitter;
        self.emit(event, payload.to_string())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Snapshot constructor that exercises every input ProgressSnapshot
    /// actually reads from QueueItemStatus, without constructing the
    /// full struct (which has 25+ fields and no Default impl).
    fn make_snapshot(
        progress: f64,
        processing_progress: Option<f32>,
        processing_label: Option<&str>,
        current_track: Option<&str>,
        completed_tracks: Option<usize>,
    ) -> ProgressSnapshot {
        ProgressSnapshot {
            progress_bucket: (progress * 10.0).round() as i64,
            processing_progress_bucket: processing_progress
                .map(|p| (p * 1000.0).round() as i64)
                .unwrap_or(-1),
            processing_label: processing_label.map(String::from),
            current_track: current_track.map(String::from),
            completed_tracks,
            activity_count: 0,
        }
    }

    /// Same as [`make_snapshot`] but with an explicit activity counter
    /// (#846).
    fn make_snapshot_with_activity(
        progress: f64,
        activity_count: u64,
    ) -> ProgressSnapshot {
        ProgressSnapshot {
            progress_bucket: (progress * 10.0).round() as i64,
            processing_progress_bucket: -1,
            processing_label: None,
            current_track: None,
            completed_tracks: None,
            activity_count,
        }
    }

    #[test]
    fn snapshot_picks_up_activity_count_change() {
        // #846: even with every other field identical, a change in
        // activity_count must mark the snapshot as different so the
        // watchdog sees long-running activity-log-only passes as
        // progress rather than a stall.
        let s1 = make_snapshot_with_activity(100.0, 10);
        let s2 = make_snapshot_with_activity(100.0, 11);
        assert_ne!(s1, s2);
    }

    #[test]
    fn warn_emission_must_not_reset_stall_via_activity_count_bump() {
        // #851 regression guard: the watchdog's own ⚠ WARN emission
        // bumps the per-download activity counter via #846. If the
        // tracked snapshot's activity_count isn't refreshed after the
        // emit, the NEXT poll observes "snapshot changed → work
        // happened → reset stall_started_at + warn_emitted" — and the
        // watchdog warns every ~11 min indefinitely instead of
        // escalating to auto-fail at 20 min.
        //
        // This test models the contract directly: after a WARN, the
        // tracked snapshot MUST be updated to reflect the post-emit
        // counter so the next snapshot comparison sees "unchanged".
        let before_warn = make_snapshot_with_activity(100.0, 42);
        let mut tracked = before_warn.clone();

        // Simulate the WARN site: post-emit, the activity counter is
        // now 43 (the watchdog's own emit bumped it).
        let post_emit_counter: u64 = 43;
        // Apply the #851 fix: refresh tracked.activity_count.
        tracked.activity_count = post_emit_counter;

        // Next poll constructs a fresh snapshot from queue state +
        // counter — counter is the post-emit value (43), every other
        // field unchanged because no real work happened.
        let next_poll = make_snapshot_with_activity(100.0, post_emit_counter);

        assert_eq!(
            tracked, next_poll,
            "after refreshing activity_count post-WARN, the next poll's \
             snapshot must compare equal so the stall timer doesn't \
             reset (else watchdog warns forever and never escalates)"
        );
    }

    #[test]
    fn snapshot_buckets_progress_to_avoid_jitter() {
        // 42.5 and 42.51 both round to bucket 425 at 1/10pp resolution.
        let s1 = make_snapshot(42.50, Some(0.5001), None, None, None);
        let s2 = make_snapshot(42.51, Some(0.5004), None, None, None);
        assert_eq!(s1, s2, "sub-1/10pp jitter should not change the snapshot");
        // A 1pp change must register (425 → 435).
        let s3 = make_snapshot(43.5, Some(0.5001), None, None, None);
        assert_ne!(s1, s3, "1pp progress change should register");
    }

    #[test]
    fn snapshot_picks_up_processing_label_change() {
        let s1 = make_snapshot(0.0, Some(0.5), Some("Stage 1"), None, None);
        let s2 = make_snapshot(0.0, Some(0.5), Some("Stage 2"), None, None);
        assert_ne!(s1, s2);
    }

    #[test]
    fn snapshot_picks_up_current_track_change() {
        let s1 = make_snapshot(50.0, None, None, Some("Song A"), Some(5));
        let s2 = make_snapshot(50.0, None, None, Some("Song B"), Some(6));
        assert_ne!(s1, s2);
    }

    #[test]
    fn snapshot_processing_progress_none_distinct_from_zero() {
        let none_snap = make_snapshot(0.0, None, None, None, None);
        let zero_snap = make_snapshot(0.0, Some(0.0), None, None, None);
        assert_ne!(
            none_snap, zero_snap,
            "None vs Some(0.0) must register as distinct so the first \
             set-progress-to-0 doesn't look like a stall reset"
        );
    }
}
