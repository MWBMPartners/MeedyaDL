// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.

//! Waiting for the internet to come back, then starting the queue (#1156).
//!
//! Paste a link with no connection and MeedyaDL queues it and says:
//!
//! > "Download queued — will start when internet is available"
//!
//! That was not true. Nothing watched for the connection returning, so the
//! download sat there until the person noticed and pressed Start Queue
//! themselves. This makes the sentence true.
//!
//! # How it behaves
//!
//! It starts only when a download was queued **because** the connection check
//! failed, and only when the user has automatic starting switched on. It is not
//! running the rest of the time.
//!
//! It checks, waits a little longer each time, and stops as soon as it has
//! nothing left to do: the queue emptied, someone started it by hand, the user
//! paused it, or the app is closing. One job, then it goes away.
//!
//! # Two things it is careful about
//!
//! **It never starts work while something is running.** Everything it decides
//! is taken from one look at the queue, under the lock, so the picture cannot
//! change halfway through a decision. When it does act it calls the same
//! function every other part of the app calls, which refuses to start anything
//! if a download is already in flight.
//!
//! **It waits for the actual service, not just any internet.** Your connection
//! coming back while Apple Music is still unreachable is a real situation, and
//! starting then would burn every retry and leave the download failed — worse
//! than waiting.
//!
//! # Why the decisions are separated out
//!
//! Everything that decides anything is a plain function of its inputs, with no
//! network and no clock. Those can be tested directly. What is left is glue:
//! sleep, take a look, ask the plain functions, act. That part cannot be
//! usefully unit-tested here and is deliberately kept as small as possible.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use tauri::{AppHandle, Emitter};

use crate::services::download_queue::{DownloadQueue, QueueWatchSnapshot, ShutdownSignal};

/// How long to wait before the first check.
///
/// Short, because the commonest case by far is someone whose connection drops
/// for a moment while they are sitting at the machine.
const FIRST_DELAY: Duration = Duration::from_secs(5);

/// The longest gap between checks.
///
/// Reached after about two and a half minutes of doubling. Two minutes is
/// still "starts when the internet comes back" as a person would understand
/// it, and at this rate a whole night offline is about 240 lightweight
/// requests — less than an idle browser tab, and nowhere near enough to look
/// like misbehaviour to the sites being asked.
const MAX_DELAY: Duration = Duration::from_secs(120);

/// How much longer than planned a wait has to run before we assume the machine
/// was asleep.
///
/// A minute of slack covers ordinary scheduling delay on a busy machine. Past
/// that, something unusual happened — almost always a closed laptop lid.
const SLEEP_DETECTION_SLACK: Duration = Duration::from_secs(60);

/// What a check found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityOutcome {
    /// The internet works and the service we need is answering.
    Reachable,
    /// No internet at all.
    NoInternet,
    /// The internet works, but the service we need is not answering. A real
    /// state worth telling apart: starting now would fail.
    ServiceUnreachable,
}

/// Why the watcher stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// Nothing is waiting any more — finished, cancelled, or cleared.
    NothingQueued,
    /// Something is already running, so this is somebody else's job now.
    QueueActive,
    /// The user paused the queue. Their call, not ours.
    QueuePaused,
    /// The user started the queue by hand while we were waiting.
    Disarmed,
    /// The app is closing.
    Shutdown,
}

/// What the watcher should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherAction {
    /// Stop, for this reason.
    Exit(ExitReason),
    /// Wait and check again.
    KeepWaiting,
    /// The connection is back — start the queue.
    StartQueue { waiting: usize },
    /// The connection is back, but the user has automatic starting switched
    /// off, so tell them rather than starting anything.
    TellUserItIsBack { waiting: usize },
}

/// Decides what the watcher should do, from what it can see.
///
/// A plain function with no network, no clock and no locks, so every path
/// through it can be tested directly.
///
/// The order of the checks is the important part. Stopping conditions come
/// first, so a queue that is paused, empty, or already running is never
/// started no matter what the connection is doing.
///
/// # Arguments
///
/// * `snapshot` -- One look at the queue, taken under the lock.
/// * `outcome` -- What the last check found, or `None` before the first one.
/// * `auto_start` -- Whether the user wants downloads to begin on their own.
/// * `disarmed` -- Whether the user has since started the queue themselves.
/// * `shutting_down` -- Whether the app is closing.
pub fn decide(
    snapshot: &QueueWatchSnapshot,
    outcome: Option<ConnectivityOutcome>,
    auto_start: bool,
    disarmed: bool,
    shutting_down: bool,
) -> WatcherAction {
    // Closing beats everything else.
    if shutting_down {
        return WatcherAction::Exit(ExitReason::Shutdown);
    }
    // The user took over. Their action wins over anything we were going to do.
    if disarmed {
        return WatcherAction::Exit(ExitReason::Disarmed);
    }
    // Something is already running, so the queue will carry itself on from
    // here. Checked before the queued count, because an item that has started
    // is no longer counted as waiting.
    if snapshot.active {
        return WatcherAction::Exit(ExitReason::QueueActive);
    }
    // Nothing left to wait for.
    if snapshot.queued == 0 {
        return WatcherAction::Exit(ExitReason::NothingQueued);
    }
    // Paused is a deliberate choice by the user and we do not override it.
    if snapshot.paused {
        return WatcherAction::Exit(ExitReason::QueuePaused);
    }

    match outcome {
        // Nothing checked yet — go and check.
        None => WatcherAction::KeepWaiting,
        // Still nothing, or the service is still down. Either way, waiting is
        // better than starting something that would fail.
        Some(ConnectivityOutcome::NoInternet | ConnectivityOutcome::ServiceUnreachable) => {
            WatcherAction::KeepWaiting
        }
        Some(ConnectivityOutcome::Reachable) => {
            if auto_start {
                WatcherAction::StartQueue {
                    waiting: snapshot.queued,
                }
            } else {
                // Read fresh each time, so someone who switches automatic
                // starting off while we are waiting is respected.
                WatcherAction::TellUserItIsBack {
                    waiting: snapshot.queued,
                }
            }
        }
    }
}

/// How long to wait before the next check.
///
/// Doubles from five seconds up to a two-minute ceiling: quick while someone
/// is likely still sitting there, then patient.
///
/// # Arguments
///
/// * `attempt` -- How many checks have already been made. Zero for the first.
#[must_use]
pub fn next_delay(attempt: u32) -> Duration {
    // Doubling past this cannot change the answer and would overflow.
    let capped_attempt = attempt.min(16);
    let seconds = FIRST_DELAY.as_secs().saturating_mul(1u64 << capped_attempt);
    Duration::from_secs(seconds.min(MAX_DELAY.as_secs()))
}

/// Whether the machine looks to have been asleep during the last wait.
///
/// A closed laptop is the ordinary case. When it opens, the connection usually
/// returns within half a minute — so the sensible thing is to go back to
/// checking quickly rather than continuing at the two-minute ceiling.
///
/// A wait that came back *early*, or a clock that moved backwards (which
/// happens when the machine corrects its time), is not a sleep and must not be
/// treated as one.
///
/// # Arguments
///
/// * `intended` -- How long the wait was meant to be.
/// * `actual` -- How much real-world time passed.
#[must_use]
pub fn should_reset_after_gap(intended: Duration, actual: Duration) -> bool {
    actual > intended.saturating_add(SLEEP_DETECTION_SLACK)
}

/// Tracks whether a watcher is running, so only one ever is.
///
/// Registered once for the whole app. The pairing of "running" and "asked
/// again" closes a gap that would otherwise lose a request: if someone queues
/// a download in the instant the loop has decided to stop, the loop notices the
/// new request as it leaves and keeps going instead.
#[derive(Debug, Default)]
pub struct ConnectivityWatcher {
    state: Mutex<WatcherState>,
}

#[derive(Debug, Default)]
struct WatcherState {
    /// A loop is running right now.
    running: bool,
    /// Someone asked for a watcher while one was already running.
    asked_again: bool,
    /// The user has started the queue themselves, so stop.
    disarmed: bool,
}

impl ConnectivityWatcher {
    /// Creates the watcher. One per app.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Claims the right to run the loop.
    ///
    /// # Returns
    ///
    /// `true` when the caller should run the loop. `false` when one is already
    /// running — in which case the request is remembered, so the running loop
    /// will not stop without honouring it.
    fn try_begin(&self) -> bool {
        let mut state = self.lock();
        state.disarmed = false;
        if state.running {
            state.asked_again = true;
            false
        } else {
            state.running = true;
            state.asked_again = false;
            true
        }
    }

    /// Gives up the right to run, unless someone asked again meanwhile.
    ///
    /// # Returns
    ///
    /// `true` when the loop should really stop. `false` when a request arrived
    /// while it was deciding to stop, and it should carry on instead.
    fn try_finish(&self) -> bool {
        let mut state = self.lock();
        if state.asked_again {
            state.asked_again = false;
            false
        } else {
            state.running = false;
            true
        }
    }

    /// Tells any running watcher to stop, because the user has taken over.
    pub fn disarm(&self) {
        self.lock().disarmed = true;
    }

    /// Whether the user has taken over.
    fn is_disarmed(&self) -> bool {
        self.lock().disarmed
    }

    /// Takes the lock, recovering if a previous holder panicked.
    ///
    /// A panic elsewhere must not leave the app unable to wait for the
    /// internet. The flags are three booleans, so there is no half-finished
    /// state to be confused by.
    fn lock(&self) -> std::sync::MutexGuard<'_, WatcherState> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Starts waiting for the connection to come back.
///
/// Safe to call repeatedly: if a watcher is already waiting, this simply makes
/// sure it keeps waiting rather than starting a second one.
///
/// # Arguments
///
/// * `app` -- Used to tell the user what happened.
/// * `queue` -- The download queue to look at and eventually start.
/// * `shutdown` -- So the loop stops when the app closes.
pub fn arm(
    watcher: Arc<ConnectivityWatcher>,
    app: AppHandle,
    queue: Arc<tokio::sync::Mutex<DownloadQueue>>,
    shutdown: Arc<ShutdownSignal>,
) {
    if !watcher.try_begin() {
        log::debug!("connection watcher: already waiting — request noted");
        return;
    }
    // `tauri::async_runtime::spawn` rather than `tokio::spawn`, for the same
    // reason as the queue watchdog: this can be called from a thread that has
    // no tokio runtime attached, and `tokio::spawn` would panic across a
    // boundary Rust cannot unwind, taking the whole app down.
    tauri::async_runtime::spawn(async move {
        run_loop(&watcher, &app, &queue, &shutdown).await;
    });
}

/// The waiting loop. Glue only — every decision is made by the plain functions
/// above, which are the ones under test.
async fn run_loop(
    watcher: &Arc<ConnectivityWatcher>,
    app: &AppHandle,
    queue: &Arc<tokio::sync::Mutex<DownloadQueue>>,
    shutdown: &Arc<ShutdownSignal>,
) {
    log::info!("connection watcher: waiting for the internet to come back");
    let mut attempt: u32 = 0;
    let mut last_outcome: Option<ConnectivityOutcome> = None;

    loop {
        let delay = next_delay(attempt);
        let started_waiting = SystemTime::now();
        tokio::time::sleep(delay).await;

        // A wait that ran far longer than asked almost always means the
        // machine was asleep. Start checking quickly again, because the
        // connection usually returns within half a minute of it waking.
        let actual = started_waiting.elapsed().unwrap_or_default();
        if should_reset_after_gap(delay, actual) {
            log::debug!("connection watcher: looks like the machine was asleep — checking quickly again");
            attempt = 0;
        }

        // One look at the queue, then release the lock before anything slow.
        let snapshot = queue.lock().await.watch_snapshot();

        if let WatcherAction::Exit(reason) = decide(
            &snapshot,
            None,
            true,
            watcher.is_disarmed(),
            shutdown.is_triggered(),
        ) {
            if watcher.try_finish() {
                log::info!("connection watcher: stopping — {reason:?}");
                return;
            }
            attempt = 0;
            continue;
        }

        let outcome = probe(snapshot.first_queued_url.as_deref()).await;
        if Some(outcome) != last_outcome {
            // Only say something when the situation actually changes. Every
            // check would be noise — up to thirty an hour.
            announce_change(app, outcome, snapshot.queued);
            attempt = 0;
        }
        last_outcome = Some(outcome);

        let auto_start = crate::services::config_service::load_settings_from_default_path()
            .map(|s| s.auto_start_queue)
            .unwrap_or(true);

        match decide(
            &snapshot,
            Some(outcome),
            auto_start,
            watcher.is_disarmed(),
            shutdown.is_triggered(),
        ) {
            WatcherAction::KeepWaiting => attempt = attempt.saturating_add(1),
            WatcherAction::Exit(reason) => {
                if watcher.try_finish() {
                    log::info!("connection watcher: stopping — {reason:?}");
                    return;
                }
                attempt = 0;
            }
            WatcherAction::StartQueue { waiting } => {
                // Check once more under the lock, and clear the cooldown in
                // the same breath, so the checks that run before a download
                // genuinely re-run rather than being skipped as recent.
                {
                    let mut q = queue.lock().await;
                    let now = q.watch_snapshot();
                    if now.queued == 0 || now.active || now.paused {
                        // It changed while we were checking the connection.
                        // Go round again rather than starting anything.
                        continue;
                    }
                    q.reset_preflight_cooldown();
                }
                crate::utils::activity_log::emit_app_log(
                    app,
                    &format!("Back online — starting {waiting} waiting download(s)"),
                );
                let _ = app.emit(
                    "connectivity-restored",
                    serde_json::json!({ "queue_started": true, "waiting": waiting }),
                );
                crate::services::download_queue::process_queue(app.clone(), queue.clone()).await;
                if watcher.try_finish() {
                    return;
                }
                attempt = 0;
            }
            WatcherAction::TellUserItIsBack { waiting } => {
                crate::utils::activity_log::emit_app_log(
                    app,
                    &format!(
                        "Back online — {waiting} download(s) are waiting. Starting downloads \
                         automatically is switched off, so press Start Queue when you are ready."
                    ),
                );
                let _ = app.emit(
                    "connectivity-restored",
                    serde_json::json!({ "queue_started": false, "waiting": waiting }),
                );
                if watcher.try_finish() {
                    return;
                }
                attempt = 0;
            }
        }
    }
}

/// Checks whether the internet, and the service we need, are reachable.
async fn probe(first_url: Option<&str>) -> ConnectivityOutcome {
    let service = first_url.and_then(crate::models::media_service::MediaServiceId::from_url);
    match crate::services::health_check_service::check_internet_connectivity(service).await {
        None => ConnectivityOutcome::Reachable,
        Some(warning) => {
            // The existing check gives one message for "no internet at all"
            // and another for "internet is fine, the service is not". They are
            // told apart by the mention of working internet, which is the only
            // thing that distinguishes them in the message today.
            if warning.message.contains("internet is working") {
                ConnectivityOutcome::ServiceUnreachable
            } else {
                ConnectivityOutcome::NoInternet
            }
        }
    }
}

/// Notes a change in the situation in the activity log.
fn announce_change(app: &AppHandle, outcome: ConnectivityOutcome, waiting: usize) {
    let message = match outcome {
        ConnectivityOutcome::NoInternet => format!(
            "Still no internet — {waiting} download(s) waiting. Will keep checking."
        ),
        ConnectivityOutcome::ServiceUnreachable => {
            "Internet is back, but the music service is still unreachable. Will keep checking."
                .to_string()
        }
        // The good news is announced by the caller, alongside what it did
        // about it, so there is one message rather than two.
        ConnectivityOutcome::Reachable => return,
    };
    crate::utils::activity_log::emit_app_log(app, &message);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(queued: usize, active: bool, paused: bool) -> QueueWatchSnapshot {
        QueueWatchSnapshot {
            queued,
            active,
            paused,
            first_queued_url: Some("https://music.apple.com/gb/album/x/1".to_string()),
        }
    }

    // ── When to stop ────────────────────────────────────────────────────

    #[test]
    fn stops_when_nothing_is_waiting_any_more() {
        let action = decide(&snapshot(0, false, false), None, true, false, false);
        assert_eq!(action, WatcherAction::Exit(ExitReason::NothingQueued));
    }

    #[test]
    fn stops_when_something_is_already_running() {
        // Somebody else got there first — the queue carries on by itself now.
        let action = decide(&snapshot(3, true, false), None, true, false, false);
        assert_eq!(action, WatcherAction::Exit(ExitReason::QueueActive));
    }

    #[test]
    fn stops_when_the_user_paused_the_queue() {
        let action = decide(&snapshot(3, false, true), None, true, false, false);
        assert_eq!(action, WatcherAction::Exit(ExitReason::QueuePaused));
    }

    #[test]
    fn stops_when_the_user_started_the_queue_themselves() {
        let action = decide(&snapshot(3, false, false), None, true, true, false);
        assert_eq!(action, WatcherAction::Exit(ExitReason::Disarmed));
    }

    #[test]
    fn stops_when_the_app_is_closing() {
        let action = decide(&snapshot(3, false, false), None, true, false, true);
        assert_eq!(action, WatcherAction::Exit(ExitReason::Shutdown));
    }

    #[test]
    fn closing_beats_every_other_reason() {
        // Even with everything else saying "go", closing wins.
        let action = decide(
            &snapshot(3, false, false),
            Some(ConnectivityOutcome::Reachable),
            true,
            false,
            true,
        );
        assert_eq!(action, WatcherAction::Exit(ExitReason::Shutdown));
    }

    #[test]
    fn something_already_running_beats_the_connection_being_back() {
        // The rule the whole queue is built on: never start work while
        // something is in flight.
        let action = decide(
            &snapshot(3, true, false),
            Some(ConnectivityOutcome::Reachable),
            true,
            false,
            false,
        );
        assert_eq!(action, WatcherAction::Exit(ExitReason::QueueActive));
    }

    // ── When to wait ────────────────────────────────────────────────────

    #[test]
    fn keeps_waiting_while_there_is_no_internet() {
        let action = decide(
            &snapshot(2, false, false),
            Some(ConnectivityOutcome::NoInternet),
            true,
            false,
            false,
        );
        assert_eq!(action, WatcherAction::KeepWaiting);
    }

    #[test]
    fn keeps_waiting_when_the_internet_is_back_but_the_service_is_not() {
        // Starting now would use up every retry and leave the download
        // failed — worse than waiting a little longer.
        let action = decide(
            &snapshot(2, false, false),
            Some(ConnectivityOutcome::ServiceUnreachable),
            true,
            false,
            false,
        );
        assert_eq!(action, WatcherAction::KeepWaiting);
    }

    // ── When to act ─────────────────────────────────────────────────────

    #[test]
    fn starts_the_queue_when_everything_is_reachable_again() {
        let action = decide(
            &snapshot(4, false, false),
            Some(ConnectivityOutcome::Reachable),
            true,
            false,
            false,
        );
        assert_eq!(action, WatcherAction::StartQueue { waiting: 4 });
    }

    #[test]
    fn never_starts_the_queue_when_the_user_turned_automatic_starting_off() {
        // Read fresh each time, so switching it off while waiting is honoured.
        let action = decide(
            &snapshot(4, false, false),
            Some(ConnectivityOutcome::Reachable),
            false,
            false,
            false,
        );
        assert_eq!(action, WatcherAction::TellUserItIsBack { waiting: 4 });
    }

    // ── How long to wait ────────────────────────────────────────────────

    #[test]
    fn the_wait_doubles_then_settles_at_two_minutes() {
        let expected = [5, 10, 20, 40, 80, 120, 120, 120];
        for (attempt, seconds) in expected.iter().enumerate() {
            assert_eq!(
                next_delay(u32::try_from(attempt).unwrap()),
                Duration::from_secs(*seconds),
                "wait before check {attempt} changed"
            );
        }
    }

    #[test]
    fn the_wait_never_overflows_however_long_it_goes_on() {
        // A machine left offline for days must not wrap round to no wait.
        for attempt in [20u32, 64, 1000, u32::MAX] {
            assert_eq!(next_delay(attempt), MAX_DELAY, "attempt {attempt}");
        }
    }

    #[test]
    fn a_wait_that_ran_far_over_means_the_machine_slept() {
        assert!(should_reset_after_gap(
            Duration::from_secs(120),
            Duration::from_secs(8 * 60 * 60)
        ));
        assert!(should_reset_after_gap(
            Duration::from_secs(5),
            Duration::from_secs(300)
        ));
    }

    #[test]
    fn ordinary_delay_is_not_mistaken_for_sleeping() {
        // A busy machine running a wait a little long is normal.
        assert!(!should_reset_after_gap(
            Duration::from_secs(120),
            Duration::from_secs(150)
        ));
        assert!(!should_reset_after_gap(
            Duration::from_secs(120),
            Duration::from_secs(180)
        ));
    }

    #[test]
    fn a_clock_moving_backwards_is_not_a_sleep() {
        // Machines correct their clocks. A wait that appears to have taken no
        // time must not be read as an eight-hour nap.
        assert!(!should_reset_after_gap(
            Duration::from_secs(120),
            Duration::ZERO
        ));
    }

    // ── Only one watcher, and no lost requests ──────────────────────────

    #[test]
    fn only_one_watcher_runs_at_a_time() {
        let watcher = ConnectivityWatcher::new();
        assert!(watcher.try_begin(), "the first caller should run the loop");
        assert!(!watcher.try_begin(), "a second caller must not start another loop");
    }

    #[test]
    fn a_request_arriving_as_the_loop_stops_is_not_lost() {
        // The gap this closes: someone queues a download in the instant the
        // loop has decided to stop. Without this the request would vanish and
        // the download would wait for ever.
        let watcher = ConnectivityWatcher::new();
        assert!(watcher.try_begin());
        assert!(!watcher.try_begin(), "second request noted while running");
        assert!(
            !watcher.try_finish(),
            "should carry on, because a request arrived while stopping"
        );
        assert!(watcher.try_finish(), "nothing pending now, so it may stop");
    }

    #[test]
    fn a_fresh_request_clears_a_previous_take_over() {
        // Someone pressed Start Queue, then later queued something else while
        // offline. The second request must not be killed by the first
        // take-over.
        let watcher = ConnectivityWatcher::new();
        watcher.disarm();
        assert!(watcher.is_disarmed());
        assert!(watcher.try_begin());
        assert!(!watcher.is_disarmed(), "arming again should clear it");
    }

    #[test]
    fn taking_over_is_visible_to_the_running_loop() {
        let watcher = ConnectivityWatcher::new();
        assert!(watcher.try_begin());
        assert!(!watcher.is_disarmed());
        watcher.disarm();
        assert!(watcher.is_disarmed());
    }
}
