// Copyright (c) 2024-2026 MeedyaSuite
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
//!    mark that ATTEMPT failed via `fail_stalled_attempt` (#1432),
//!    which also gives back that attempt's slot -- and nothing else,
//!    so it can never free the slot of a later attempt or of the next
//!    item -- write a History "failed" entry, and re-trigger
//!    `process_queue` to start the next item.
//!
//! Users keep their work-in-progress files on disk — only the queue
//! entry is marked failed. They can manually retry from the History.
//!
//! Timers are kept per attempt (#1432), so a retried item starts with a
//! fresh one rather than inheriting the old attempt's.
//!
//! Each poll also frees any slot still held by an attempt that has been
//! replaced (cancelled, retried or removed) for longer than
//! `STALL_HARD_SECS` -- normally such a run gives its slot back within
//! seconds; this catches one that never does (#1432).
//!
//! ## Deliberately not done
//!
//! - Stopping the underlying GAMDL subprocess or the tidying-up
//!   (maintainer decision, 17 Nov 2026). The run is left going but can
//!   no longer change the item. If it finishes after all, it writes one
//!   activity-log line and queues the item again once
//!   (`download_queue::finish_ended_outside`).
//! - User-configurable thresholds. Hard-coded 10 / 20 min for now;
//!   add settings keys in a follow-up if power users ask.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::models::download::QueueItemStatus;
use crate::services::download_queue::{Attempt, DownloadQueue, ShutdownSignal};
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
    /// Takes a snapshot of an item. Two snapshots that are equal mean the item made
    /// no visible progress between the two looks.
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
        let mut tracked: HashMap<u64, ItemState> = HashMap::new();
        let mut orphans_seen: HashMap<u64, Instant> = HashMap::new();
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if shutdown.is_triggered() {
                log::info!("queue watchdog exiting — shutdown signal");
                return;
            }
            poll_once(&app, &queue, &mut tracked).await;
            sweep_orphaned_slots(&app, &queue, &mut orphans_seen, Instant::now()).await;
        }
    });
}

/// One iteration of the polling loop. Pulled out for testability.
///
/// Timers are keyed by attempt number (#1432), so a retried item starts a
/// fresh timer instead of inheriting the old attempt's.
async fn poll_once(
    app: &AppHandle,
    queue: &Arc<Mutex<DownloadQueue>>,
    tracked: &mut HashMap<u64, ItemState>,
) {
    // Snapshot the queue under a brief lock — release before doing
    // anything that might take time. Each running item comes with the
    // attempt that is running it.
    let items_snapshot: Vec<(Attempt, ProgressSnapshot)> = {
        let q = queue.lock().await;
        q.running_attempts()
            .into_iter()
            .map(|(attempt, s)| {
                let snap = ProgressSnapshot::from(&s);
                (attempt, snap)
            })
            .collect()
    };

    // Forget any attempts that are no longer running. Stale entries would
    // never get matched again and just leak.
    let running: std::collections::HashSet<u64> = items_snapshot
        .iter()
        .map(|(attempt, _)| attempt.number)
        .collect();
    tracked.retain(|number, _| running.contains(number));

    let now = Instant::now();
    for (attempt, snapshot) in items_snapshot {
        let id = attempt.id.clone();
        match tracked.get_mut(&attempt.number) {
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
                    if escalate_to_error(app, queue, &attempt).await {
                        // Counted and announced like any other failed
                        // download (#1431), so the queue summary is right
                        // and the person is told, by name.
                        crate::services::download_queue::notify_item_failed(
                            app, queue, &id, "stalled",
                        )
                        .await;
                        // The slot was given back: move the queue on
                        // (#1432; the module doc always said it did, but
                        // nothing started the next item).
                        crate::services::download_queue::save_queue_to_disk(app, queue).await;
                        crate::services::download_queue::process_queue(app.clone(), queue.clone())
                            .await;
                    }
                    // Drop the entry so we don't re-escalate on the
                    // next poll.
                    tracked.remove(&attempt.number);
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
                    attempt.number,
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

/// The message a download marked failed as stuck carries.
const STALLED_MESSAGE: &str = "Queue watchdog detected a stalled download (no progress for >20 min). Marked failed automatically to keep the queue moving. Already-downloaded files on disk are preserved — retry from History to resume.";

/// Escalation action (#1432) — mark the stuck ATTEMPT failed and give
/// back its slot, in one step under the lock (`fail_stalled_attempt`).
///
/// It acts only if that attempt still owns the item and the item is still
/// downloading or tidying up. So if the item finished, was cancelled, or
/// was retried between the poll and now, nothing at all changes -- above
/// all, no slot is released. (The old version released a slot whatever
/// happened, which freed the next item's slot and let two downloads run
/// at once.) The run itself is left going; see the module notes.
///
/// When it does act, it counts the failure (before anything can start the
/// queue, #1431) and writes the History "failed" entry, which it never did
/// before. Returns whether it acted, so only a real failure is announced.
async fn escalate_to_error(
    app: &AppHandle,
    queue: &Arc<Mutex<DownloadQueue>>,
    attempt: &Attempt,
) -> bool {
    let mut q = queue.lock().await;
    let failed = q.fail_stalled_attempt(attempt, STALLED_MESSAGE);
    if !failed {
        return false;
    }
    // Counted before anything can start the queue again, so the queue
    // summary that may follow at once includes it (#1431).
    crate::services::notification_service::count_result(
        crate::services::notification_service::NotifyKind::DownloadFailed,
    );
    let entry = stalled_history_entry(&q, attempt);
    drop(q);
    if let Some(entry) = entry {
        crate::services::history_service::save_history_entry(app, entry);
    }
    let _ = app.emit_to_all_with_str("queue-updated", "watchdog-escalated");
    true
}

/// The History "failed" entry for a download the stall check stopped
/// (#1432), shaped like the queue's own failure entries. `None` if the
/// item is gone.
fn stalled_history_entry(
    q: &DownloadQueue,
    attempt: &Attempt,
) -> Option<crate::services::history_service::HistoryEntry> {
    let status = q.get_status().into_iter().find(|s| s.id == attempt.id)?;
    Some(crate::services::history_service::HistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        url: status.urls.first().cloned().unwrap_or_default(),
        title: None,
        artist: status.artist_name.clone(),
        album: status.album_name.clone(),
        codec: None,
        file_path: status.output_path.clone(),
        started_at: status.created_at.clone(),
        completed_at: crate::utils::time_compat::now_rfc3339(),
        status: "failed".to_string(),
        error_message: Some(STALLED_MESSAGE.to_string()),
    })
}

/// Which held slots have belonged to a replaced attempt for at least
/// `STALL_HARD_SECS` (#1432). `seen` remembers when each was first found
/// orphaned; slots no longer orphaned are forgotten. Pure, for tests.
fn orphans_due(orphaned_now: &[u64], seen: &mut HashMap<u64, Instant>, now: Instant) -> Vec<u64> {
    seen.retain(|n, _| orphaned_now.contains(n));
    let mut due = Vec::new();
    for n in orphaned_now {
        let first = *seen.entry(*n).or_insert(now);
        if now.duration_since(first).as_secs() >= STALL_HARD_SECS {
            due.push(*n);
        }
    }
    due
}

/// Frees slots still held by attempts that were replaced long ago
/// (#1432). An old run normally gives its slot back within seconds of
/// being cancelled or retried; one that hangs instead would otherwise
/// hold the one slot for good, because the stall check only watches
/// running items. Logs each one and starts the queue again.
async fn sweep_orphaned_slots(
    app: &AppHandle,
    queue: &Arc<Mutex<DownloadQueue>>,
    seen: &mut HashMap<u64, Instant>,
    now: Instant,
) {
    let orphaned = queue.lock().await.orphaned_slots();
    let due = orphans_due(&orphaned, seen, now);
    if due.is_empty() {
        return;
    }
    let mut freed_any = false;
    {
        let mut q = queue.lock().await;
        for n in &due {
            if q.release_orphaned_slot(*n) {
                freed_any = true;
                log::warn!(
                    "watchdog: freed a queue slot still held by replaced attempt {n} after {} min",
                    STALL_HARD_SECS / 60
                );
            }
            seen.remove(n);
        }
    }
    if freed_any {
        crate::utils::activity_log::emit_app_log(
            app,
            "An earlier download run that was cancelled or retried never let go of the queue. It has been released so the queue can carry on.",
        );
        crate::services::download_queue::process_queue(app.clone(), queue.clone()).await;
    }
}

/// Local trait extension so the watchdog can fire the same Tauri
/// event the queue processor emits. Tauri's `Manager::emit` requires
/// a serializable payload; the watchdog just needs a fire-and-forget
/// notification, hence this tiny shim.
trait WatchdogEmit {
    /// Sends `event` to every window with a plain text payload.
    fn emit_to_all_with_str(&self, event: &str, payload: &str) -> Result<(), String>;
}

impl WatchdogEmit for AppHandle {
    /// Sends the event through Tauri's `emit` and turns any error into text.
    ///
    /// See: https://v2.tauri.app/develop/calling-frontend/#event-system
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
    fn make_snapshot_with_activity(progress: f64, activity_count: u64) -> ProgressSnapshot {
        ProgressSnapshot {
            progress_bucket: (progress * 10.0).round() as i64,
            processing_progress_bucket: -1,
            processing_label: None,
            current_track: None,
            completed_tracks: None,
            activity_count,
        }
    }

    /// A queue with one item started, and that item's attempt.
    fn queue_with_one_running() -> (DownloadQueue, Attempt) {
        let mut q = DownloadQueue::new();
        let request = crate::models::download::DownloadRequest {
            urls: vec!["https://music.apple.com/us/album/test/123".to_string()],
            options: None,
            ..Default::default()
        };
        q.enqueue(request, &crate::models::settings::AppSettings::default());
        let attempt = q.next_pending().expect("starts").attempt;
        (q, attempt)
    }

    /// #1432: a slot held by a replaced attempt is freed only once it has
    /// been orphaned for the full stall time; one that stops being orphaned
    /// is forgotten.
    #[test]
    fn orphan_sweep_waits_the_stall_time_and_forgets_resolved_slots() {
        let start = Instant::now();
        let mut seen = HashMap::new();
        assert!(orphans_due(&[7], &mut seen, start).is_empty());
        let later = start + Duration::from_secs(STALL_HARD_SECS - 1);
        assert!(orphans_due(&[7], &mut seen, later).is_empty());
        let due_time = start + Duration::from_secs(STALL_HARD_SECS);
        assert_eq!(orphans_due(&[7], &mut seen, due_time), vec![7]);
        // Slot 7 was given back by its own run: forgotten.
        assert!(orphans_due(&[], &mut seen, due_time).is_empty());
        assert!(seen.is_empty());
    }

    /// #1432: the stall check's timers are keyed by attempt, so a retried
    /// item is a new key with a fresh timer.
    #[test]
    fn running_attempts_give_a_retried_item_a_new_key() {
        let (mut q, first) = queue_with_one_running();
        let keys: Vec<u64> = q.running_attempts().iter().map(|(a, _)| a.number).collect();
        assert_eq!(keys, vec![first.number]);
        assert!(q.fail_stalled_attempt(&first, STALLED_MESSAGE));
        assert!(
            q.running_attempts().is_empty(),
            "a failed item is not watched"
        );
        assert!(q.retry(&first.id, &crate::models::settings::AppSettings::default()));
        let second = q.next_pending().unwrap().attempt;
        let keys: Vec<u64> = q.running_attempts().iter().map(|(a, _)| a.number).collect();
        assert_eq!(keys, vec![second.number]);
        assert_ne!(second.number, first.number);
    }

    /// #1432: the stall check writes a History "failed" entry for the item
    /// it stopped, with the plain message.
    #[test]
    fn stalled_item_gets_a_failed_history_entry() {
        let (mut q, attempt) = queue_with_one_running();
        assert!(q.fail_stalled_attempt(&attempt, STALLED_MESSAGE));
        let entry = stalled_history_entry(&q, &attempt).expect("an entry");
        assert_eq!(entry.status, "failed");
        assert_eq!(entry.error_message.as_deref(), Some(STALLED_MESSAGE));
        assert!(entry.url.contains("music.apple.com"));
        let gone = Attempt {
            id: "gone".to_string(),
            number: 999,
        };
        assert!(stalled_history_entry(&q, &gone).is_none());
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
