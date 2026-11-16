// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// The one place that decides whether a message becomes a system
// notification, an in-app message, or both (#1431).

//! # Notifications: one rule, in one place
//!
//! Before #1431 the app had several half-rules that disagreed with each
//! other: the page sent EVERY in-app message to the operating system as
//! well (about 217 places), the download queue sent its own system
//! notifications but never showed anything inside the app, the copied-link
//! prompt sent a third kind directly, and each looked at a different copy
//! of the settings. This module replaces all of that with:
//!
//! - [`decide`] -- a pure function. Given what kind of message it is, the
//!   saved notification settings and whether the person is looking at
//!   MeedyaDL, it answers two questions: send a system notification? show
//!   it in the app? Everything else calls this; nothing else decides.
//! - [`probe_attention`] -- the only code that asks the window system
//!   "is the person looking at MeedyaDL?", returning a small
//!   [`WindowAttention`] so [`decide`] stays testable.
//! - [`notify`] -- for messages the backend owns (a download finishing or
//!   failing, the queue finishing, a computer shut-down warning). It reads
//!   the SAVED settings, applies [`decide`], sends the system notification,
//!   and tells the page (event `app-notification`) whether to show it in
//!   the app.
//! - [`notify_system_for_page`] -- for messages the page owns (every other
//!   toast). The page asks, through the `notify_system` command, and is
//!   told whether a system notification was sent and whether it should
//!   still show its own copy.
//!
//! **Bursts.** When several downloads finish or fail within ten seconds,
//! the system gets ONE summary ("3 downloads finished, 1 failed") instead
//! of a stack of banners. The in-app messages are not merged.
//!
//! **Failures are logged, never shown.** If the system refuses a
//! notification, that goes to the log only. Turning it into a new in-app
//! error message is what used to make a loop possible (an error message
//! sent to the system, the send failing, another error message, ...).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::models::settings::{AppSettings, NativeNotificationScope, NativeNotificationWhen};

// ============================================================
// Kinds of message
// ============================================================

/// What a message is about. Decides which settings apply to it.
///
/// The first seven are the "important" ones (see
/// [`NativeNotificationScope::Important`]). `Error` and `General` are the
/// page's ordinary messages, sorted by whether they are an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotifyKind {
    /// A download finished.
    DownloadComplete,
    /// A download failed.
    DownloadFailed,
    /// A download finished, but some files may need downloading again.
    DownloadWarnings,
    /// Everything in the queue is done.
    QueueFinished,
    /// A copied Apple Music link was found on the clipboard.
    ClipboardLink,
    /// The computer is about to shut down, restart or sleep.
    PowerAction,
    /// An app update is installed and ready once MeedyaDL restarts.
    UpdateReady,
    /// Any other error message.
    Error,
    /// Any other message.
    General,
}

impl NotifyKind {
    /// Whether this is one of the messages always worth a system
    /// notification under the default scope.
    #[must_use]
    pub fn is_important(self) -> bool {
        !matches!(self, Self::Error | Self::General)
    }

    /// Whether this kind takes part in the ten-second burst summary.
    #[must_use]
    fn is_download_result(self) -> bool {
        matches!(
            self,
            Self::DownloadComplete | Self::DownloadFailed | Self::DownloadWarnings
        )
    }
}

// ============================================================
// Settings, as this module sees them
// ============================================================

/// The three notification styles. Stored as text in `AppSettings`; an
/// unknown value is read as the default, `NativeAndInApp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationStyle {
    /// In-app messages only; nothing is sent to the system.
    InAppOnly,
    /// System notifications and in-app messages.
    NativeAndInApp,
    /// System notifications in place of in-app messages, where that is
    /// safe (see [`decide`]).
    NativeOnly,
}

impl NotificationStyle {
    /// Reads the stored text form.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "in_app_only" => Self::InAppOnly,
            "native_only" => Self::NativeOnly,
            _ => Self::NativeAndInApp,
        }
    }
}

/// Just the settings that decide notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationSettings {
    /// The master switch for system notifications.
    pub enabled: bool,
    /// How messages are delivered.
    pub style: NotificationStyle,
    /// When a system notification is shown.
    pub when: NativeNotificationWhen,
    /// Which messages may become system notifications.
    pub scope: NativeNotificationScope,
    /// Per-event switch: a download finished.
    pub download_complete: bool,
    /// Per-event switch: a download failed or finished with warnings.
    pub download_failed: bool,
    /// Per-event switch: the queue finished.
    pub queue_finished: bool,
    /// Per-event switch: a copied link was found.
    pub clipboard_link: bool,
}

impl NotificationSettings {
    /// Picks the notification settings out of the whole settings object.
    #[must_use]
    pub fn from_app_settings(s: &AppSettings) -> Self {
        Self {
            enabled: s.desktop_notifications,
            style: NotificationStyle::parse(&s.notification_style),
            when: s.native_notification_when,
            scope: s.native_notification_scope,
            download_complete: s.notify_download_complete,
            download_failed: s.notify_download_failed,
            queue_finished: s.notify_queue_finished,
            clipboard_link: s.notify_clipboard_link,
        }
    }
}

// ============================================================
// Is the person looking at MeedyaDL?
// ============================================================

/// What the window system says about MeedyaDL's window right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowAttention {
    /// The window has keyboard focus (it is the app in front).
    pub focused: bool,
    /// The window is minimised.
    pub minimised: bool,
    /// The window is shown (not hidden in the tray).
    pub visible: bool,
    /// The mouse pointer is on a different screen from the window.
    pub pointer_on_other_screen: bool,
}

impl WindowAttention {
    /// "Looking at MeedyaDL": in front, not minimised, shown, and the
    /// pointer on the same screen. Used by tests and by [`decide`].
    #[must_use]
    pub const fn looking() -> Self {
        Self {
            focused: true,
            minimised: false,
            visible: true,
            pointer_on_other_screen: false,
        }
    }
}

/// A screen's rectangle: left, top, width, height (physical pixels).
pub type ScreenRect = (i32, i32, u32, u32);

/// Whether two screens are different screens. Two screens are the same
/// when they sit at the same place and have the same size -- the window
/// system has no other identity for a screen that is the same on every
/// platform.
#[must_use]
pub fn screens_differ(a: ScreenRect, b: ScreenRect) -> bool {
    a != b
}

/// Asks the window system whether the person is looking at MeedyaDL.
///
/// This is the ONLY place that asks. When the window system cannot answer
/// a question, the answer chosen is always the one that means "the person
/// is looking": focused, not minimised, shown, pointer on the same screen.
/// That way an error never produces a system notification the person did
/// not want; at worst a message is shown only inside the app.
///
/// If there is no main window at all, nobody can be looking at it.
pub fn probe_attention(app: &AppHandle) -> WindowAttention {
    let Some(window) = app.get_webview_window("main") else {
        return WindowAttention {
            focused: false,
            minimised: false,
            visible: false,
            pointer_on_other_screen: false,
        };
    };
    let focused = window.is_focused().unwrap_or(true);
    let minimised = window.is_minimized().unwrap_or(false);
    let visible = window.is_visible().unwrap_or(true);
    let pointer_on_other_screen = pointer_screen_differs(app, &window).unwrap_or(false);
    WindowAttention {
        focused,
        minimised,
        visible,
        pointer_on_other_screen,
    }
}

/// `Some(true)` when the pointer is on a different screen from the window,
/// `None` when the window system could not say.
fn pointer_screen_differs(app: &AppHandle, window: &tauri::WebviewWindow) -> Option<bool> {
    let pointer = app.cursor_position().ok()?;
    let window_screen = window.current_monitor().ok()??;
    let pointer_screen = app.monitor_from_point(pointer.x, pointer.y).ok()??;
    let rect = |m: &tauri::Monitor| -> ScreenRect {
        (
            m.position().x,
            m.position().y,
            m.size().width,
            m.size().height,
        )
    };
    Some(screens_differ(rect(&window_screen), rect(&pointer_screen)))
}

// ============================================================
// The decision
// ============================================================

/// What to do with one message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// Send a system notification.
    pub system: bool,
    /// Show it inside the app. (The page also keeps a message in the app
    /// whenever it has a button, details, a key, or is an error or
    /// warning -- things a system notification cannot carry or that must
    /// stay until dismissed. See `uiStore.addToast`.)
    pub in_app: bool,
}

/// Whether the "which messages" setting lets this kind through.
#[must_use]
pub fn scope_allows(scope: NativeNotificationScope, kind: NotifyKind) -> bool {
    match scope {
        NativeNotificationScope::Important => kind.is_important(),
        NativeNotificationScope::ImportantAndErrors => {
            kind.is_important() || kind == NotifyKind::Error
        }
        NativeNotificationScope::Everything => true,
    }
}

/// Whether this kind's own on/off switch is on.
#[must_use]
pub fn event_switch_allows(s: &NotificationSettings, kind: NotifyKind) -> bool {
    match kind {
        NotifyKind::DownloadComplete => s.download_complete,
        NotifyKind::DownloadFailed | NotifyKind::DownloadWarnings => s.download_failed,
        NotifyKind::QueueFinished => s.queue_finished,
        NotifyKind::ClipboardLink => s.clipboard_link,
        _ => true,
    }
}

/// Whether the "show system notifications" setting allows one now.
#[must_use]
pub fn when_allows(when: NativeNotificationWhen, a: &WindowAttention) -> bool {
    match when {
        NativeNotificationWhen::Always => true,
        NativeNotificationWhen::OtherScreenOnly => a.pointer_on_other_screen,
        NativeNotificationWhen::BackgroundOrOtherScreen => {
            !a.focused || a.minimised || !a.visible || a.pointer_on_other_screen
        }
    }
}

/// Whether a system notification is possible at all for this kind, before
/// asking whether the person is looking: the master switch, the style, the
/// scope and the kind's own switch. Cheap, so it runs first and the window
/// system is only asked when this is `true`.
#[must_use]
pub fn could_send(kind: NotifyKind, s: &NotificationSettings) -> bool {
    s.enabled
        && s.style != NotificationStyle::InAppOnly
        && scope_allows(s.scope, kind)
        && event_switch_allows(s, kind)
}

/// THE rule. Pure: no window system, no settings file, no clock.
///
/// A system notification is sent when the master switch is on, the style
/// is not "in-app only", the scope covers this kind, its own switch is on,
/// and the "when" setting allows it right now.
///
/// It is shown in the app unless the style is "System only" AND the system
/// notification is actually being sent. So a message that was not sent to
/// the system is never lost.
#[must_use]
pub fn decide(kind: NotifyKind, s: &NotificationSettings, a: &WindowAttention) -> Decision {
    decide_with(kind, s, || *a)
}

/// The same rule, asking the window system (`probe`) only when the answer
/// depends on it: never when [`could_send`] already says no, and never for
/// "Always". The page asks about every message, so this keeps the usual
/// case (an ordinary message under the default scope) to a settings read.
pub fn decide_with(
    kind: NotifyKind,
    s: &NotificationSettings,
    probe: impl FnOnce() -> WindowAttention,
) -> Decision {
    let system = could_send(kind, s)
        && (s.when == NativeNotificationWhen::Always || when_allows(s.when, &probe()));
    let in_app = s.style != NotificationStyle::NativeOnly || !system;
    Decision { system, in_app }
}

// ============================================================
// Sending
// ============================================================

/// Makes text safe to hand to the system's notification service: control
/// characters (line breaks included) become spaces, and -- where the
/// notification service reads simple markup, as many Linux ones do --
/// `&`, `<` and `>` are written so they show as themselves. Pure, so it is
/// tested on every system.
#[must_use]
pub fn clean_system_text(text: &str, escape_markup: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            c if c.is_control() => out.push(' '),
            '&' if escape_markup => out.push_str("&amp;"),
            '<' if escape_markup => out.push_str("&lt;"),
            '>' if escape_markup => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

/// The title and body as they are handed to the system. Linux notification
/// services may read simple markup in the BODY, so there `&`, `<`, `>` are
/// escaped. The title (the freedesktop "summary") is plain text everywhere,
/// so it is never escaped -- an escaped title would show "&amp;" literally.
/// Pure, so it is tested.
#[must_use]
pub fn prepare_system_text(title: &str, body: &str, escape_body_markup: bool) -> (String, String) {
    (
        clean_system_text(title, false),
        clean_system_text(body, escape_body_markup),
    )
}

/// Hands one notification to the operating system. A refusal is logged,
/// never turned into another message (see the module notes).
///
/// Returns whether the system accepted it. On desktop the notification
/// plugin cannot tell whether the person will actually see it (Focus mode,
/// notifications switched off for MeedyaDL), only whether handing it over
/// failed.
pub fn send_system_notification(app: &AppHandle, title: &str, body: &str) -> bool {
    use tauri_plugin_notification::NotificationExt;
    let (title, body) = prepare_system_text(title, body, cfg!(target_os = "linux"));
    match app
        .notification()
        .builder()
        .title(&title)
        .body(&body)
        .show()
    {
        Ok(()) => {
            log::debug!("system notification handed over: {title:?}");
            true
        }
        Err(e) => {
            log::warn!("system notification could not be handed over: {title:?}: {e}");
            false
        }
    }
}

/// The same title and body twice within this many seconds is sent once.
const REPEAT_WINDOW_SECS: u64 = 10;

/// Remembers recent system notifications so an identical one repeated
/// within [`REPEAT_WINDOW_SECS`] is not shown again. (This replaces the old
/// "throttle", which sent every one anyway and added "(N items)".)
#[derive(Debug, Default)]
pub struct RecentSends {
    sent_at: std::collections::HashMap<String, std::time::Instant>,
}

impl RecentSends {
    /// Whether this exact notification was sent within the window.
    /// Forgets entries older than the window as it goes.
    pub fn is_repeat(&mut self, key: &str, now: std::time::Instant) -> bool {
        let window = std::time::Duration::from_secs(REPEAT_WINDOW_SECS);
        self.sent_at
            .retain(|_, at| now.saturating_duration_since(*at) < window);
        self.sent_at.contains_key(key)
    }

    /// Records a notification the system has ACCEPTED. A refused one is
    /// never recorded, so a retry moments later is not mistaken for a
    /// repeat of something the person never saw.
    pub fn record(&mut self, key: &str, now: std::time::Instant) {
        self.sent_at.insert(key.to_string(), now);
    }
}

static RECENT_SENDS: std::sync::LazyLock<std::sync::Mutex<RecentSends>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(RecentSends::default()));

fn recent_sends() -> std::sync::MutexGuard<'static, RecentSends> {
    RECENT_SENDS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Sends unless the same notification was sent moments ago. Returns
/// whether it was sent NOW: a suppressed repeat answers `false`, so in
/// "System only" the message is shown in the app rather than nowhere.
fn send_unless_repeat(app: &AppHandle, title: &str, body: &str) -> bool {
    let key = format!("{title}\n{body}");
    if recent_sends().is_repeat(&key, std::time::Instant::now()) {
        return false;
    }
    let sent = send_system_notification(app, title, body);
    if sent {
        recent_sends().record(&key, std::time::Instant::now());
    }
    sent
}

/// Reads the SAVED settings (what is on disk, through the settings cache)
/// -- never the page's unsaved edits.
fn saved_settings(app: &AppHandle) -> NotificationSettings {
    let s = crate::services::download_queue::load_settings_for_queue(app);
    NotificationSettings::from_app_settings(&s)
}

// ============================================================
// One worker for backend-owned notifications
// ============================================================

/// A piece of notification work, run on the notification worker.
type Job = Box<dyn FnOnce() + Send + 'static>;

static WORKER: std::sync::LazyLock<std::sync::Mutex<Option<std::sync::mpsc::Sender<Job>>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Runs `job` on MeedyaDL's one notification thread.
///
/// Asking the window system whether the person is looking can wait on the
/// app's main thread. A download task must never stand still for that, so
/// backend-owned notifications are handed to this thread and the caller
/// carries straight on. One thread, in order, so a download result is
/// always dealt with before the "queue finished" that follows it.
fn run_on_worker(job: Job) {
    let mut guard = WORKER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let job = match guard.as_ref() {
        Some(tx) => match tx.send(job) {
            Ok(()) => return,
            Err(std::sync::mpsc::SendError(job)) => job,
        },
        None => job,
    };
    // No worker yet (or it has gone): start one and hand it the job.
    let (tx, rx) = std::sync::mpsc::channel::<Job>();
    let started = std::thread::Builder::new()
        .name("meedyadl-notifications".to_string())
        .spawn(move || {
            for job in rx {
                job();
            }
        });
    match started {
        Ok(_) => {
            if let Err(std::sync::mpsc::SendError(job)) = tx.send(job) {
                log::warn!("Notification worker stopped at once; running the notification here");
                job();
                return;
            }
            *guard = Some(tx);
        }
        Err(e) => {
            log::warn!("Could not start the notification worker ({e}); running it here");
            drop(guard);
            job();
        }
    }
}

// ============================================================
// Bursts of download results
// ============================================================

/// How long after the first download result the window stays open.
const BURST_WINDOW_SECS: u64 = 10;

/// Download results that arrived while a burst window was open (the FIRST
/// result is not counted here: it was sent at once, by name).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BurstCounts {
    /// Downloads that finished.
    pub finished: u32,
    /// Downloads that failed.
    pub failed: u32,
    /// Downloads that finished with warnings.
    pub warnings: u32,
}

impl BurstCounts {
    /// Counts one more result.
    pub fn add(&mut self, kind: NotifyKind) {
        match kind {
            NotifyKind::DownloadComplete => self.finished += 1,
            NotifyKind::DownloadFailed => self.failed += 1,
            NotifyKind::DownloadWarnings => self.warnings += 1,
            _ => {}
        }
    }

    /// How many results in all.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.finished + self.failed + self.warnings
    }

    /// Only the results whose system notification the settings still
    /// allow (they may have changed during the window).
    #[must_use]
    pub fn still_allowed(self, s: &NotificationSettings) -> Self {
        Self {
            finished: if could_send(NotifyKind::DownloadComplete, s) {
                self.finished
            } else {
                0
            },
            failed: if could_send(NotifyKind::DownloadFailed, s) {
                self.failed
            } else {
                0
            },
            warnings: if could_send(NotifyKind::DownloadWarnings, s) {
                self.warnings
            } else {
                0
            },
        }
    }
}

/// "1 more download" / "3 more downloads".
fn more_downloads(n: u32) -> String {
    if n == 1 {
        "1 more download".to_string()
    } else {
        format!("{n} more downloads")
    }
}

/// The one summary sent at the end of a burst window, for the results
/// that came after the first (which was already named). Pure, so the
/// wording is tested. `None` when there is nothing to say.
#[must_use]
pub fn more_summary(b: &BurstCounts) -> Option<(String, String)> {
    if b.total() == 0 {
        return None;
    }
    let mut parts = Vec::new();
    if b.finished > 0 {
        parts.push(format!("{} finished", more_downloads(b.finished)));
    }
    if b.warnings > 0 {
        parts.push(if parts.is_empty() {
            format!("{} finished with warnings", more_downloads(b.warnings))
        } else {
            format!("{} with warnings", b.warnings)
        });
    }
    if b.failed > 0 {
        parts.push(if parts.is_empty() {
            format!("{} failed", more_downloads(b.failed))
        } else {
            format!("{} failed", b.failed)
        });
    }
    let title = if b.finished == 0 && b.warnings == 0 {
        "More downloads failed"
    } else {
        "More downloads finished"
    };
    Some((title.to_string(), parts.join(", ")))
}

/// Title and text for one download result.
#[must_use]
pub fn single_result_text(kind: NotifyKind, label: &str, reason: Option<&str>) -> (String, String) {
    match kind {
        NotifyKind::DownloadFailed => (
            "Download failed".to_string(),
            format!(
                "{label} could not be downloaded. {}",
                reason.unwrap_or(DEFAULT_FAILURE_REASON)
            ),
        ),
        NotifyKind::DownloadWarnings => (
            "Download finished with warnings".to_string(),
            format!("{label} finished, but some files may need downloading again."),
        ),
        _ => (
            "Download finished".to_string(),
            format!("{label} finished."),
        ),
    }
}

/// Used when no shorter, plainer reason is known.
pub const DEFAULT_FAILURE_REASON: &str = "Open MeedyaDL for details.";

/// A short, plain reason for a failed download, from the error category
/// `utils::process::classify_error` gives (plus `stalled`, from the queue
/// watchdog). Never the raw error text: that is technical, can be long,
/// and can name files and addresses.
#[must_use]
pub fn short_failure_reason(category: &str) -> &'static str {
    match category {
        "network" => "Check your internet connection.",
        "auth" => "Your Apple Music sign-in may need refreshing.",
        "io" | "io_transient" => "MeedyaDL could not save to your download folder.",
        "not_found" | "media_not_streamable" => "It may no longer be available.",
        "rate_limit" => "Apple Music asked MeedyaDL to slow down. Try again later.",
        "codec" | "license_declined" => "The chosen format is not available for it.",
        "tool" => "A helper program needs attention in Settings > Tools.",
        "stalled" => "It stopped making progress. You can retry it from History.",
        _ => DEFAULT_FAILURE_REASON,
    }
}

/// The open burst window, if any: `(generation, results since the first)`.
/// The generation stops an old window's timer acting on a newer window.
#[derive(Debug, Default)]
pub struct BurstState {
    generation: u64,
    open: Option<(u64, BurstCounts)>,
}

impl BurstState {
    /// Puts a result into the burst. `Some(generation)` when this opened a
    /// new window (send it now, and start that window's timer); `None`
    /// when it was counted into the open window (sent in the summary).
    pub fn join(&mut self, kind: NotifyKind) -> Option<u64> {
        if let Some((_, counts)) = self.open.as_mut() {
            counts.add(kind);
            return None;
        }
        self.generation += 1;
        self.open = Some((self.generation, BurstCounts::default()));
        Some(self.generation)
    }

    /// Closes the window opened as `generation`, handing back what it
    /// counted. `None` if that window is already gone (discarded, or a
    /// newer one is open).
    pub fn close(&mut self, generation: u64) -> Option<BurstCounts> {
        match self.open {
            Some((g, counts)) if g == generation => {
                self.open = None;
                Some(counts)
            }
            _ => None,
        }
    }

    /// Closes any open window without a summary.
    pub fn discard(&mut self) {
        self.open = None;
    }
}

/// Whether a message's arrival should throw away the pending burst
/// summary: only a "queue finished" that WAS sent to the system, because
/// its summary then covers those results. If it was not sent (switched
/// off, out of scope, the person is looking), the burst closes normally.
#[must_use]
pub fn discards_burst(kind: NotifyKind, sent_now: bool) -> bool {
    kind == NotifyKind::QueueFinished && sent_now
}

static BURST: std::sync::LazyLock<std::sync::Mutex<BurstState>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(BurstState::default()));

fn burst_state() -> std::sync::MutexGuard<'static, BurstState> {
    BURST
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What happened to a download result's system notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BurstSlot {
    /// No window was open: send it now, by name (a window is now open).
    SendNow,
    /// A window is open: counted, and sent later in the summary.
    Deferred,
}

/// Puts a download result into the burst: opens a window (and starts its
/// ten-second timer) when none is open, otherwise counts it.
fn join_burst(app: &AppHandle, kind: NotifyKind) -> BurstSlot {
    let Some(generation) = burst_state().join(kind) else {
        return BurstSlot::Deferred;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(BURST_WINDOW_SECS)).await;
        run_on_worker(Box::new(move || close_burst(&app, generation)));
    });
    BurstSlot::SendNow
}

/// The end of a burst window: one summary of the results after the first,
/// if there were any. Reads the saved settings and the window state again
/// now, so switching notifications off, or coming back to MeedyaDL, during
/// the window is honoured. (The in-app messages for those results were
/// already shown, so nothing is lost when the summary is not sent.)
fn close_burst(app: &AppHandle, generation: u64) {
    // `None`: taken by a sent "queue finished", or a newer window is open.
    let Some(counts) = burst_state().close(generation) else {
        return;
    };
    let settings = saved_settings(app);
    let counts = counts.still_allowed(&settings);
    let Some((title, body)) = more_summary(&counts) else {
        return;
    };
    // `still_allowed` has already applied the switches, style and scope;
    // what is left is "is the person looking now?".
    if settings.when == NativeNotificationWhen::Always
        || when_allows(settings.when, &probe_attention(app))
    {
        send_system_notification(app, &title, &body);
    }
}

// ============================================================
// Results since the queue last finished
// ============================================================

/// Downloads that finished and failed since the queue last finished, for
/// the "Queue finished: 5 downloaded, 1 failed" message.
static QUEUE_RUN: std::sync::LazyLock<std::sync::Mutex<(u32, u32)>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new((0, 0)));

/// Counts one download result for the next "Queue finished" summary.
///
/// Called by the queue at the moment the item's state really changes --
/// with the queue lock held and BEFORE the item's slot is released -- so a
/// queue that goes idle the next instant has already counted it (#1431).
/// `notify` itself does not count, so nothing is counted twice.
pub fn count_result(kind: NotifyKind) {
    let mut run = QUEUE_RUN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match kind {
        NotifyKind::DownloadComplete | NotifyKind::DownloadWarnings => run.0 += 1,
        NotifyKind::DownloadFailed => run.1 += 1,
        _ => {}
    }
}

/// Forgets the counts, e.g. when the queue was aborted.
pub fn reset_queue_run_counts() {
    *QUEUE_RUN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = (0, 0);
}

/// The queue-finished message text. Pure, so the wording is tested.
#[must_use]
pub fn queue_finished_text(downloaded: u32, failed: u32) -> String {
    match (downloaded, failed) {
        (0, 0) => "Queue finished.".to_string(),
        (d, 0) => format!("Queue finished: {d} downloaded."),
        (0, f) => format!("Queue finished: {f} failed."),
        (d, f) => format!("Queue finished: {d} downloaded, {f} failed."),
    }
}

// ============================================================
// Backend-owned messages
// ============================================================

/// One backend-owned message.
#[derive(Debug, Clone)]
pub struct NotifyEvent {
    /// What it is about.
    pub kind: NotifyKind,
    /// The system notification's title.
    pub title: String,
    /// The text: the system notification's body, and the in-app message.
    pub body: String,
    /// The in-app message's look: `success`, `error`, `warning` or `info`.
    pub toast_type: &'static str,
    /// The item it is about ("Artist — Album"), when there is one.
    pub label: Option<String>,
}

/// What the page receives as the `app-notification` event.
#[derive(Debug, Clone, Serialize)]
pub struct AppNotificationPayload {
    /// What it is about (snake_case kind).
    pub kind: NotifyKind,
    /// Title (for reference; the in-app message uses `body`).
    pub title: String,
    /// The in-app message text.
    pub body: String,
    /// `success`, `error`, `warning` or `info`.
    pub toast_type: &'static str,
    /// The item it is about, when there is one.
    pub label: Option<String>,
    /// Whether the page should show it in the app.
    pub show_in_app: bool,
}

/// Errors and warnings stay in the app until dismissed, whatever the style.
fn always_in_app(toast_type: &str) -> bool {
    matches!(toast_type, "error" | "warning")
}

/// Whether the page should show a backend message in the app. Pure, so the
/// "System only" rules are tested: the in-app copy is left out only when
/// the rule allows it AND the system notification really went out now --
/// not when it was refused, and not when it was put off to a burst summary
/// (the person must not be left with neither).
#[must_use]
pub fn show_in_app(decision: Decision, toast_type: &str, sent_now: bool) -> bool {
    decision.in_app || always_in_app(toast_type) || !sent_now
}

/// Delivers a backend-owned message: system notification (when the rule
/// says so) and the in-app copy (through the `app-notification` event).
///
/// Returns at once: the work runs on the notification thread (see
/// [`run_on_worker`]), so a download task never waits on the window system.
pub fn notify(app: &AppHandle, event: NotifyEvent) {
    let app = app.clone();
    run_on_worker(Box::new(move || deliver(&app, event)));
}

/// The work [`notify`] hands to the notification thread.
fn deliver(app: &AppHandle, event: NotifyEvent) {
    let settings = saved_settings(app);
    let decision = decide_with(event.kind, &settings, || probe_attention(app));

    let sent_now = if !decision.system {
        false
    } else if event.kind.is_download_result() {
        match join_burst(app, event.kind) {
            BurstSlot::SendNow => send_unless_repeat(app, &event.title, &event.body),
            BurstSlot::Deferred => false,
        }
    } else if event.kind == NotifyKind::QueueFinished {
        // Never filtered as a repeat: two queues finishing with the same
        // counts are two real events.
        send_system_notification(app, &event.title, &event.body)
    } else {
        send_unless_repeat(app, &event.title, &event.body)
    };
    if discards_burst(event.kind, sent_now) {
        // The queue summary just sent covers the results still waiting for
        // a burst summary.
        burst_state().discard();
    }

    let payload = AppNotificationPayload {
        kind: event.kind,
        title: event.title,
        body: event.body,
        toast_type: event.toast_type,
        label: event.label,
        show_in_app: show_in_app(decision, event.toast_type, sent_now),
    };
    if let Err(e) = app.emit("app-notification", &payload) {
        log::debug!("Could not tell the page about a notification: {e}");
    }
}

/// A download finished. `label` is the item's "Artist — Album" text.
pub fn notify_download_complete(app: &AppHandle, label: &str) {
    let (title, body) = single_result_text(NotifyKind::DownloadComplete, label, None);
    notify(
        app,
        NotifyEvent {
            kind: NotifyKind::DownloadComplete,
            title,
            body,
            toast_type: "success",
            label: Some(label.to_string()),
        },
    );
}

/// A download failed. `category` is the error category; the raw error
/// text is deliberately not taken.
pub fn notify_download_failed(app: &AppHandle, label: &str, category: &str) {
    let reason = short_failure_reason(category);
    let (title, body) = single_result_text(NotifyKind::DownloadFailed, label, Some(reason));
    notify(
        app,
        NotifyEvent {
            kind: NotifyKind::DownloadFailed,
            title,
            body,
            toast_type: "error",
            label: Some(label.to_string()),
        },
    );
}

/// A download finished, but some files may need downloading again.
pub fn notify_download_warnings(app: &AppHandle, label: &str) {
    let (title, body) = single_result_text(NotifyKind::DownloadWarnings, label, None);
    notify(
        app,
        NotifyEvent {
            kind: NotifyKind::DownloadWarnings,
            title,
            body,
            toast_type: "warning",
            label: Some(label.to_string()),
        },
    );
}

/// The whole queue is done. Uses (and then clears) the counts collected
/// since the queue last finished.
pub fn notify_queue_finished(app: &AppHandle) {
    let (downloaded, failed) = {
        let mut run = QUEUE_RUN
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::mem::take(&mut *run)
    };
    notify(
        app,
        NotifyEvent {
            kind: NotifyKind::QueueFinished,
            title: "Queue finished".to_string(),
            body: queue_finished_text(downloaded, failed),
            toast_type: if failed == 0 { "success" } else { "warning" },
            label: None,
        },
    );
}

/// The after-queue power actions that warn first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    /// Restart the computer.
    Restart,
    /// Shut the computer down.
    ShutDown,
    /// Put the computer to sleep (or hibernate, on Windows).
    Sleep,
}

/// Whether a Windows copy of MeedyaDL is running straight from a build
/// folder (`target\debug` or `target\release`). The notification plugin
/// then files notifications under PowerShell instead of MeedyaDL's own
/// identity, because an unpackaged program has none registered with
/// Windows. Pure, so it is tested on every system.
#[must_use]
pub fn runs_from_build_folder(exe_dir: &str) -> bool {
    let dir = exe_dir.replace('\\', "/");
    dir.ends_with("/target/debug") || dir.ends_with("/target/release")
}

/// The warning text for a power action. Only Windows waits (30 seconds,
/// `shutdown /t 30`) before restarting or shutting down; everywhere else,
/// and for sleep, the action starts straight away, so the warning says so.
/// Pure, so each system's wording is tested. `os` is
/// `std::env::consts::OS`.
#[must_use]
pub fn power_warning_text(action: PowerAction, os: &str) -> String {
    let delayed = os == "windows";
    match (action, delayed) {
        (PowerAction::Restart, true) => {
            "Downloads finished. Your computer will restart in 30 seconds.".to_string()
        }
        (PowerAction::ShutDown, true) => {
            "Downloads finished. Your computer will shut down in 30 seconds.".to_string()
        }
        (PowerAction::Restart, false) => {
            "Downloads finished. Your computer is restarting now.".to_string()
        }
        (PowerAction::ShutDown, false) => {
            "Downloads finished. Your computer is shutting down now.".to_string()
        }
        // Windows really hibernates (`shutdown /h`); macOS and Linux sleep
        // or hibernate as the system is set up, so "sleep" is the honest word.
        (PowerAction::Sleep, true) => {
            "Downloads finished. Your computer is hibernating now.".to_string()
        }
        (PowerAction::Sleep, false) => {
            "Downloads finished. Your computer is going to sleep now.".to_string()
        }
    }
}

/// The computer is about to shut down, restart or sleep.
pub fn notify_power_action(app: &AppHandle, action: PowerAction) {
    notify(
        app,
        NotifyEvent {
            kind: NotifyKind::PowerAction,
            title: "MeedyaDL".to_string(),
            body: power_warning_text(action, std::env::consts::OS),
            toast_type: "warning",
            label: None,
        },
    );
}

// ============================================================
// Page-owned messages
// ============================================================

/// The answer to the page's `notify_system` request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NotifySystemAnswer {
    /// A system notification was sent (or the identical one moments ago).
    pub sent: bool,
    /// Whether the page should show its own copy (before its own extra
    /// reasons to keep one: a button, details, a key, an error).
    pub in_app: bool,
}

/// Longest title the page may hand to the system, in characters.
const MAX_TITLE_CHARS: usize = 120;
/// Longest text the page may hand to the system, in characters. The
/// system shortens long text anyway; this only stops an unexpected huge
/// string (a whole log, say) from being passed along.
const MAX_BODY_CHARS: usize = 500;

/// Cuts `text` to at most `max` characters (never mid-character), adding
/// "…" when it was cut.
#[must_use]
pub fn clamp_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(max.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// Applies the same rule to a message the page is about to show.
pub fn notify_system_for_page(
    app: &AppHandle,
    kind: NotifyKind,
    title: &str,
    body: &str,
) -> NotifySystemAnswer {
    let settings = saved_settings(app);
    // Settings first; the window system is asked only when it matters.
    let decision = decide_with(kind, &settings, || probe_attention(app));
    if !decision.system {
        return NotifySystemAnswer {
            sent: false,
            in_app: true,
        };
    }
    let title = clamp_chars(title, MAX_TITLE_CHARS);
    let body = clamp_chars(body, MAX_BODY_CHARS);
    let sent = decision.system && send_unless_repeat(app, &title, &body);
    NotifySystemAnswer {
        sent,
        in_app: decision.in_app || !sent,
    }
}

/// Sends the Settings screen's test notification, using the values ON
/// SCREEN (not yet saved) and ignoring "is the person looking" -- they are,
/// they just pressed the button.
pub fn send_test_notification(
    app: &AppHandle,
    desktop_notifications: bool,
    notification_style: &str,
) -> Result<(), String> {
    if !desktop_notifications {
        return Err(
            "System notifications are switched off. Turn on System Notifications first."
                .to_string(),
        );
    }
    if NotificationStyle::parse(notification_style) == NotificationStyle::InAppOnly {
        return Err(
            "Notification Style is set to In-app only, so nothing is sent to the system. \
             Choose another style first."
                .to_string(),
        );
    }
    if send_system_notification(
        app,
        "MeedyaDL test notification",
        "If you can read this, MeedyaDL's notifications are reaching your system.",
    ) {
        Ok(())
    } else {
        Err("Your system did not accept the notification.".to_string())
    }
}

// ============================================================
// The after-queue "play a sound"
// ============================================================

/// How to play the system alert sound on one operating system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundPlan {
    /// Try these programs in order; the first that starts wins.
    Commands(Vec<(&'static str, Vec<&'static str>)>),
    /// Windows: ask the system for its standard alert sound directly.
    WindowsBeep,
    /// Nothing known for this system.
    Unsupported,
}

/// Picks how to play the alert sound. Pure, so each system's choice is
/// tested. `os` is `std::env::consts::OS`.
#[must_use]
pub fn sound_plan(os: &str) -> SoundPlan {
    match os {
        "macos" => SoundPlan::Commands(vec![("afplay", vec!["/System/Library/Sounds/Glass.aiff"])]),
        "windows" => SoundPlan::WindowsBeep,
        "linux" | "freebsd" | "openbsd" | "netbsd" => SoundPlan::Commands(vec![
            ("canberra-gtk-play", vec!["-i", "complete"]),
            (
                "paplay",
                vec!["/usr/share/sounds/freedesktop/stereo/complete.oga"],
            ),
        ]),
        _ => SoundPlan::Unsupported,
    }
}

/// Plays the system alert sound, without waiting for it. Always plays,
/// whatever the notification settings or window focus (the person chose
/// "play a sound" for exactly this moment). A failure is logged only.
pub fn play_alert_sound() {
    match sound_plan(std::env::consts::OS) {
        SoundPlan::Commands(candidates) => {
            std::thread::spawn(move || {
                for (program, args) in candidates {
                    match std::process::Command::new(program).args(&args).status() {
                        Ok(status) if status.success() => return,
                        Ok(status) => {
                            log::debug!("Alert sound: {program} ended with {status}");
                        }
                        Err(e) => log::debug!("Alert sound: {program} could not start: {e}"),
                    }
                }
                log::info!("Alert sound: no sound player was available on this system");
            });
        }
        SoundPlan::WindowsBeep => windows_beep(),
        SoundPlan::Unsupported => {
            log::info!("Alert sound: not available on this system");
        }
    }
}

#[cfg(windows)]
fn windows_beep() {
    use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONASTERISK;
    // SAFETY: `MessageBeep` takes a plain number naming which standard
    // sound to play and returns at once (the sound plays on its own); it
    // reads and writes no memory of ours.
    let ok = unsafe { MessageBeep(MB_ICONASTERISK) };
    if ok == 0 {
        log::info!("Alert sound: Windows did not play the alert sound");
    }
}

#[cfg(not(windows))]
fn windows_beep() {
    log::info!("Alert sound: the Windows alert sound is only available on Windows");
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_KINDS: [NotifyKind; 9] = [
        NotifyKind::DownloadComplete,
        NotifyKind::DownloadFailed,
        NotifyKind::DownloadWarnings,
        NotifyKind::QueueFinished,
        NotifyKind::ClipboardLink,
        NotifyKind::PowerAction,
        NotifyKind::UpdateReady,
        NotifyKind::Error,
        NotifyKind::General,
    ];

    fn defaults() -> NotificationSettings {
        NotificationSettings::from_app_settings(&AppSettings::default())
    }

    fn away() -> WindowAttention {
        WindowAttention {
            focused: false,
            ..WindowAttention::looking()
        }
    }

    // ── the default settings ─────────────────────────────────────────

    #[test]
    fn defaults_are_the_maintainers_choice() {
        let s = defaults();
        assert!(s.enabled);
        assert_eq!(s.style, NotificationStyle::NativeAndInApp);
        assert_eq!(s.when, NativeNotificationWhen::BackgroundOrOtherScreen);
        assert_eq!(s.scope, NativeNotificationScope::Important);
        assert!(s.download_complete && s.download_failed);
        assert!(s.queue_finished && s.clipboard_link);
    }

    // ── the full table ───────────────────────────────────────────────

    /// The expected answer, written out independently of `decide` so the
    /// test is not the function checking itself.
    fn expected(kind: NotifyKind, s: &NotificationSettings, a: &WindowAttention) -> Decision {
        let in_scope = match s.scope {
            NativeNotificationScope::Important => {
                !matches!(kind, NotifyKind::Error | NotifyKind::General)
            }
            NativeNotificationScope::ImportantAndErrors => kind != NotifyKind::General,
            NativeNotificationScope::Everything => true,
        };
        let switch_on = match kind {
            NotifyKind::DownloadComplete => s.download_complete,
            NotifyKind::DownloadFailed | NotifyKind::DownloadWarnings => s.download_failed,
            NotifyKind::QueueFinished => s.queue_finished,
            NotifyKind::ClipboardLink => s.clipboard_link,
            _ => true,
        };
        let away_now = match s.when {
            NativeNotificationWhen::Always => true,
            NativeNotificationWhen::OtherScreenOnly => a.pointer_on_other_screen,
            NativeNotificationWhen::BackgroundOrOtherScreen => {
                !a.focused || a.minimised || !a.visible || a.pointer_on_other_screen
            }
        };
        let system = s.enabled
            && s.style != NotificationStyle::InAppOnly
            && in_scope
            && switch_on
            && away_now;
        Decision {
            system,
            in_app: !(system && s.style == NotificationStyle::NativeOnly),
        }
    }

    #[test]
    fn decide_matches_the_table_for_every_combination() {
        let styles = [
            NotificationStyle::InAppOnly,
            NotificationStyle::NativeAndInApp,
            NotificationStyle::NativeOnly,
        ];
        let whens = [
            NativeNotificationWhen::BackgroundOrOtherScreen,
            NativeNotificationWhen::OtherScreenOnly,
            NativeNotificationWhen::Always,
        ];
        let scopes = [
            NativeNotificationScope::Important,
            NativeNotificationScope::ImportantAndErrors,
            NativeNotificationScope::Everything,
        ];
        let mut checked = 0;
        for enabled in [true, false] {
            for style in styles {
                for when in whens {
                    for scope in scopes {
                        for switches in 0u8..16 {
                            let s = NotificationSettings {
                                enabled,
                                style,
                                when,
                                scope,
                                download_complete: switches & 1 != 0,
                                download_failed: switches & 2 != 0,
                                queue_finished: switches & 4 != 0,
                                clipboard_link: switches & 8 != 0,
                            };
                            for attention in 0u8..16 {
                                let a = WindowAttention {
                                    focused: attention & 1 != 0,
                                    minimised: attention & 2 != 0,
                                    visible: attention & 4 != 0,
                                    pointer_on_other_screen: attention & 8 != 0,
                                };
                                for kind in ALL_KINDS {
                                    assert_eq!(
                                        decide(kind, &s, &a),
                                        expected(kind, &s, &a),
                                        "{kind:?} {s:?} {a:?}"
                                    );
                                    checked += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 2 * 3 * 3 * 3 * 16 * 16 * 9);
    }

    // ── named cases, so a reader can see the rules ───────────────────

    #[test]
    fn nothing_goes_to_the_system_while_the_person_is_looking() {
        let d = decide(
            NotifyKind::DownloadComplete,
            &defaults(),
            &WindowAttention::looking(),
        );
        assert_eq!(
            d,
            Decision {
                system: false,
                in_app: true
            }
        );
    }

    #[test]
    fn a_finished_download_reaches_the_system_when_the_app_is_in_the_background() {
        let d = decide(NotifyKind::DownloadComplete, &defaults(), &away());
        assert_eq!(
            d,
            Decision {
                system: true,
                in_app: true
            }
        );
    }

    #[test]
    fn pointer_on_another_screen_counts_as_not_looking_even_with_focus() {
        let a = WindowAttention {
            pointer_on_other_screen: true,
            ..WindowAttention::looking()
        };
        assert!(decide(NotifyKind::QueueFinished, &defaults(), &a).system);
        let only_other = NotificationSettings {
            when: NativeNotificationWhen::OtherScreenOnly,
            ..defaults()
        };
        assert!(decide(NotifyKind::QueueFinished, &only_other, &a).system);
        // Other-screen-only ignores a merely unfocused window.
        assert!(!decide(NotifyKind::QueueFinished, &only_other, &away()).system);
    }

    #[test]
    fn minimised_or_hidden_counts_as_not_looking() {
        for a in [
            WindowAttention {
                minimised: true,
                ..WindowAttention::looking()
            },
            WindowAttention {
                visible: false,
                ..WindowAttention::looking()
            },
        ] {
            assert!(
                decide(NotifyKind::DownloadFailed, &defaults(), &a).system,
                "{a:?}"
            );
        }
    }

    #[test]
    fn ordinary_messages_stay_in_the_app_under_the_default_scope() {
        assert!(!decide(NotifyKind::General, &defaults(), &away()).system);
        assert!(!decide(NotifyKind::Error, &defaults(), &away()).system);
        let with_errors = NotificationSettings {
            scope: NativeNotificationScope::ImportantAndErrors,
            ..defaults()
        };
        assert!(decide(NotifyKind::Error, &with_errors, &away()).system);
        assert!(!decide(NotifyKind::General, &with_errors, &away()).system);
    }

    #[test]
    fn the_master_switch_only_stops_system_notifications() {
        let off = NotificationSettings {
            enabled: false,
            style: NotificationStyle::NativeOnly,
            ..defaults()
        };
        let d = decide(NotifyKind::DownloadComplete, &off, &away());
        assert_eq!(
            d,
            Decision {
                system: false,
                in_app: true
            }
        );
    }

    #[test]
    fn native_only_keeps_a_message_in_the_app_when_it_was_not_sent() {
        let native_only = NotificationSettings {
            style: NotificationStyle::NativeOnly,
            ..defaults()
        };
        let looking = decide(
            NotifyKind::DownloadComplete,
            &native_only,
            &WindowAttention::looking(),
        );
        assert_eq!(
            looking,
            Decision {
                system: false,
                in_app: true
            }
        );
        let sent = decide(NotifyKind::DownloadComplete, &native_only, &away());
        assert_eq!(
            sent,
            Decision {
                system: true,
                in_app: false
            }
        );
    }

    #[test]
    fn a_per_event_switch_stops_only_the_system_copy() {
        let s = NotificationSettings {
            clipboard_link: false,
            ..defaults()
        };
        let d = decide(NotifyKind::ClipboardLink, &s, &away());
        assert_eq!(
            d,
            Decision {
                system: false,
                in_app: true
            }
        );
        let warnings_off = NotificationSettings {
            download_failed: false,
            ..defaults()
        };
        assert!(!decide(NotifyKind::DownloadWarnings, &warnings_off, &away()).system);
    }

    // ── screens ──────────────────────────────────────────────────────

    #[test]
    fn screens_are_compared_by_place_and_size() {
        assert!(!screens_differ((0, 0, 2560, 1440), (0, 0, 2560, 1440)));
        assert!(screens_differ((0, 0, 2560, 1440), (2560, 0, 2560, 1440)));
        assert!(screens_differ((0, 0, 2560, 1440), (0, 0, 1920, 1080)));
    }

    // ── wording ──────────────────────────────────────────────────────

    #[test]
    fn the_end_of_a_burst_sums_up_only_the_results_after_the_first() {
        assert_eq!(more_summary(&BurstCounts::default()), None);

        let mut b = BurstCounts::default();
        b.add(NotifyKind::DownloadComplete);
        b.add(NotifyKind::DownloadComplete);
        b.add(NotifyKind::DownloadFailed);
        assert_eq!(
            more_summary(&b),
            Some((
                "More downloads finished".to_string(),
                "2 more downloads finished, 1 failed".to_string()
            ))
        );

        let mut all_failed = BurstCounts::default();
        all_failed.add(NotifyKind::DownloadFailed);
        assert_eq!(
            more_summary(&all_failed),
            Some((
                "More downloads failed".to_string(),
                "1 more download failed".to_string()
            ))
        );

        let mut mixed = BurstCounts::default();
        mixed.add(NotifyKind::DownloadComplete);
        mixed.add(NotifyKind::DownloadWarnings);
        assert_eq!(
            more_summary(&mixed).unwrap().1,
            "1 more download finished, 1 with warnings"
        );
    }

    #[test]
    fn a_single_result_is_named() {
        assert_eq!(
            single_result_text(NotifyKind::DownloadComplete, "Artist — Album", None),
            (
                "Download finished".to_string(),
                "Artist — Album finished.".to_string()
            )
        );
        assert_eq!(
            single_result_text(
                NotifyKind::DownloadFailed,
                "Artist — Album",
                Some("Check your internet connection.")
            )
            .1,
            "Artist — Album could not be downloaded. Check your internet connection."
        );
    }

    #[test]
    fn the_burst_summary_honours_settings_changed_during_the_window() {
        let mut b = BurstCounts::default();
        b.add(NotifyKind::DownloadComplete);
        b.add(NotifyKind::DownloadFailed);
        let off = NotificationSettings {
            enabled: false,
            ..defaults()
        };
        assert_eq!(b.still_allowed(&off).total(), 0);
        let no_complete = NotificationSettings {
            download_complete: false,
            ..defaults()
        };
        let left = b.still_allowed(&no_complete);
        assert_eq!((left.finished, left.failed), (0, 1));
    }

    #[test]
    fn system_only_keeps_the_in_app_copy_unless_the_system_copy_went_out_now() {
        let native_only = Decision {
            system: true,
            in_app: false,
        };
        // Sent now: the in-app copy of a plain success can be left out.
        assert!(!show_in_app(native_only, "success", true));
        // Put off to the burst summary, or refused: keep it.
        assert!(show_in_app(native_only, "success", false));
        // Errors and warnings always stay.
        assert!(show_in_app(native_only, "error", true));
        assert!(show_in_app(native_only, "warning", true));
    }

    #[test]
    fn the_window_system_is_asked_only_when_it_matters() {
        let mut asked = 0;
        // Default scope: an ordinary message never needs the window.
        let d = decide_with(NotifyKind::General, &defaults(), || {
            asked += 1;
            away()
        });
        assert!(!d.system);
        let off = NotificationSettings {
            enabled: false,
            ..defaults()
        };
        decide_with(NotifyKind::DownloadComplete, &off, || {
            asked += 1;
            away()
        });
        let always = NotificationSettings {
            when: NativeNotificationWhen::Always,
            ..defaults()
        };
        assert!(
            decide_with(NotifyKind::DownloadComplete, &always, || {
                asked += 1;
                WindowAttention::looking()
            })
            .system
        );
        assert_eq!(asked, 0);
        // When it does matter, it is asked.
        decide_with(NotifyKind::DownloadComplete, &defaults(), || {
            asked += 1;
            away()
        });
        assert_eq!(asked, 1);
    }

    #[test]
    fn power_warnings_say_30_seconds_only_on_windows() {
        assert!(power_warning_text(PowerAction::ShutDown, "windows").contains("in 30 seconds"));
        assert!(power_warning_text(PowerAction::Restart, "windows").contains("in 30 seconds"));
        for os in ["macos", "linux"] {
            for action in [
                PowerAction::ShutDown,
                PowerAction::Restart,
                PowerAction::Sleep,
            ] {
                let text = power_warning_text(action, os);
                assert!(!text.contains("30 seconds"), "{os}: {text}");
                assert!(text.ends_with("now."), "{os}: {text}");
            }
        }
    }

    #[test]
    fn a_burst_window_counts_only_results_after_the_first_and_closes_once() {
        let mut state = BurstState::default();
        let first = state
            .join(NotifyKind::DownloadComplete)
            .expect("opens a window");
        assert_eq!(
            state.join(NotifyKind::DownloadFailed),
            None,
            "counted, not sent"
        );
        assert_eq!(state.join(NotifyKind::DownloadComplete), None);
        let counts = state.close(first).expect("still open");
        assert_eq!((counts.finished, counts.failed), (1, 1));
        assert_eq!(state.close(first), None, "closes once");
        // A new window gets a new number; the old timer cannot close it.
        let second = state.join(NotifyKind::DownloadComplete).unwrap();
        assert_ne!(first, second);
        assert_eq!(state.close(first), None);
        assert!(state.close(second).is_some());
    }

    #[test]
    fn only_a_sent_queue_summary_throws_away_the_burst() {
        assert!(discards_burst(NotifyKind::QueueFinished, true));
        // Not sent (switched off, out of scope, person looking): the burst
        // closes normally with its own summary.
        assert!(!discards_burst(NotifyKind::QueueFinished, false));
        assert!(!discards_burst(NotifyKind::DownloadComplete, true));

        let mut state = BurstState::default();
        let g = state.join(NotifyKind::DownloadComplete).unwrap();
        state.join(NotifyKind::DownloadComplete);
        if discards_burst(NotifyKind::QueueFinished, false) {
            state.discard();
        }
        assert_eq!(state.close(g).map(|c| c.finished), Some(1));
    }

    #[test]
    fn only_the_body_is_escaped_never_the_title() {
        let (title, body) = prepare_system_text("R&B <Live>", "R&B <Live>", true);
        assert_eq!(title, "R&B <Live>");
        assert_eq!(body, "R&amp;B &lt;Live&gt;");
    }

    #[test]
    fn hibernate_is_named_on_windows() {
        assert!(power_warning_text(PowerAction::Sleep, "windows").contains("hibernating"));
        assert!(power_warning_text(PowerAction::Sleep, "macos").contains("sleep"));
    }

    #[test]
    fn a_build_folder_run_is_recognised_on_either_slash() {
        assert!(runs_from_build_folder(
            r"C:\src\MeedyaDL\src-tauri\target\debug"
        ));
        assert!(runs_from_build_folder(
            "/home/me/MeedyaDL/src-tauri/target/release"
        ));
        assert!(!runs_from_build_folder(r"C:\Program Files\MeedyaDL"));
    }

    #[test]
    fn text_for_the_system_has_no_control_characters_and_escapes_markup_when_asked() {
        assert_eq!(clean_system_text("a\nb\u{7}c", false), "a b c");
        assert_eq!(
            clean_system_text("R&B <Live>", true),
            "R&amp;B &lt;Live&gt;"
        );
        assert_eq!(clean_system_text("R&B <Live>", false), "R&B <Live>");
    }

    #[test]
    fn failure_reasons_are_short_and_never_empty() {
        for category in [
            "network",
            "auth",
            "io",
            "io_transient",
            "not_found",
            "rate_limit",
            "codec",
            "license_declined",
            "tool",
            "unknown",
            "",
            "something new",
        ] {
            let reason = short_failure_reason(category);
            assert!(
                !reason.is_empty() && reason.len() < 80,
                "{category}: {reason}"
            );
            assert!(reason.ends_with('.'), "{category}: {reason}");
        }
        assert_eq!(short_failure_reason("whatever"), DEFAULT_FAILURE_REASON);
    }

    #[test]
    fn queue_finished_text_gives_the_counts() {
        assert_eq!(
            queue_finished_text(5, 1),
            "Queue finished: 5 downloaded, 1 failed."
        );
        assert_eq!(queue_finished_text(5, 0), "Queue finished: 5 downloaded.");
        assert_eq!(queue_finished_text(0, 2), "Queue finished: 2 failed.");
        assert_eq!(queue_finished_text(0, 0), "Queue finished.");
    }

    // ── repeats ──────────────────────────────────────────────────────

    #[test]
    fn the_same_notification_is_not_repeated_within_ten_seconds() {
        let mut recent = RecentSends::default();
        let t0 = std::time::Instant::now();
        let at = |s| t0 + std::time::Duration::from_secs(s);
        assert!(!recent.is_repeat("a", t0));
        // Not recorded (say the system refused it): not a repeat.
        assert!(!recent.is_repeat("a", at(1)));
        recent.record("a", at(1));
        assert!(recent.is_repeat("a", at(9)));
        assert!(!recent.is_repeat("b", at(9)));
        assert!(!recent.is_repeat("a", at(12)));
    }

    // ── text from the page is kept to a sensible length ──────────────

    #[test]
    fn page_text_is_cut_at_a_whole_character() {
        assert_eq!(clamp_chars("short", 10), "short");
        let long = "€".repeat(600);
        let cut = clamp_chars(&long, MAX_BODY_CHARS);
        assert_eq!(cut.chars().count(), MAX_BODY_CHARS);
        assert!(cut.ends_with('…'));
    }

    // ── the alert sound ──────────────────────────────────────────────

    #[test]
    fn the_alert_sound_is_chosen_per_system() {
        assert_eq!(
            sound_plan("macos"),
            SoundPlan::Commands(vec![("afplay", vec!["/System/Library/Sounds/Glass.aiff"])])
        );
        assert_eq!(sound_plan("windows"), SoundPlan::WindowsBeep);
        assert_eq!(
            sound_plan("linux"),
            SoundPlan::Commands(vec![
                ("canberra-gtk-play", vec!["-i", "complete"]),
                (
                    "paplay",
                    vec!["/usr/share/sounds/freedesktop/stereo/complete.oga"]
                ),
            ])
        );
        assert_eq!(sound_plan("plan9"), SoundPlan::Unsupported);
    }

    // ── kinds match the page's names ─────────────────────────────────

    #[test]
    fn kinds_use_the_names_the_page_sends() {
        let names: Vec<String> = ALL_KINDS
            .iter()
            .map(|k| {
                serde_json::to_value(k)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(
            names,
            [
                "download_complete",
                "download_failed",
                "download_warnings",
                "queue_finished",
                "clipboard_link",
                "power_action",
                "update_ready",
                "error",
                "general",
            ]
        );
    }
}
