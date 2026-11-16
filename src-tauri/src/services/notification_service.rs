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
    let system = s.enabled
        && s.style != NotificationStyle::InAppOnly
        && scope_allows(s.scope, kind)
        && event_switch_allows(s, kind)
        && when_allows(s.when, a);
    let in_app = s.style != NotificationStyle::NativeOnly || !system;
    Decision { system, in_app }
}

// ============================================================
// Sending
// ============================================================

/// Hands one notification to the operating system. A refusal is logged,
/// never turned into another message (see the module notes).
///
/// Returns whether the system accepted it. On desktop the notification
/// plugin cannot tell whether the person will actually see it (Focus mode,
/// notifications switched off for MeedyaDL), only whether handing it over
/// failed.
pub fn send_system_notification(app: &AppHandle, title: &str, body: &str) -> bool {
    use tauri_plugin_notification::NotificationExt;
    match app.notification().builder().title(title).body(body).show() {
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
    /// Whether a notification with this key should be sent now; records it
    /// if so. Forgets entries older than the window as it goes.
    pub fn should_send(&mut self, key: &str, now: std::time::Instant) -> bool {
        let window = std::time::Duration::from_secs(REPEAT_WINDOW_SECS);
        self.sent_at
            .retain(|_, at| now.saturating_duration_since(*at) < window);
        if self.sent_at.contains_key(key) {
            return false;
        }
        self.sent_at.insert(key.to_string(), now);
        true
    }
}

static RECENT_SENDS: std::sync::LazyLock<std::sync::Mutex<RecentSends>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(RecentSends::default()));

/// Sends unless the same notification was sent moments ago. Returns `true`
/// when the person has been (or just was) shown it by the system.
fn send_unless_repeat(app: &AppHandle, title: &str, body: &str) -> bool {
    let key = format!("{title}\n{body}");
    let fresh = RECENT_SENDS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .should_send(&key, std::time::Instant::now());
    if !fresh {
        return true;
    }
    send_system_notification(app, title, body)
}

/// Reads the SAVED settings (what is on disk, through the settings cache)
/// -- never the page's unsaved edits.
fn saved_settings(app: &AppHandle) -> NotificationSettings {
    let s = crate::services::download_queue::load_settings_for_queue(app);
    NotificationSettings::from_app_settings(&s)
}

// ============================================================
// Bursts of download results
// ============================================================

/// How long after the first download result the summary waits.
const BURST_WINDOW_SECS: u64 = 10;

/// Download results counted during one burst.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BurstCounts {
    /// Downloads that finished.
    pub finished: u32,
    /// Downloads that failed.
    pub failed: u32,
    /// Downloads that finished with warnings.
    pub warnings: u32,
    /// The first result's kind, label and short reason -- used when the
    /// burst holds only one result, so it can be named.
    pub first: Option<(NotifyKind, String, Option<String>)>,
}

impl BurstCounts {
    /// Counts one more result.
    pub fn add(&mut self, kind: NotifyKind, label: &str, reason: Option<&str>) {
        match kind {
            NotifyKind::DownloadComplete => self.finished += 1,
            NotifyKind::DownloadFailed => self.failed += 1,
            NotifyKind::DownloadWarnings => self.warnings += 1,
            _ => return,
        }
        if self.first.is_none() {
            self.first = Some((kind, label.to_string(), reason.map(str::to_string)));
        }
    }

    fn total(&self) -> u32 {
        self.finished + self.failed + self.warnings
    }
}

/// "1 download" / "3 downloads".
fn downloads(n: u32) -> String {
    if n == 1 {
        "1 download".to_string()
    } else {
        format!("{n} downloads")
    }
}

/// The system notification for a burst: one result is named, several are
/// summed up. Pure, so the wording is tested.
#[must_use]
pub fn burst_summary(b: &BurstCounts) -> (String, String) {
    if b.total() == 1 {
        if let Some((kind, label, reason)) = &b.first {
            return single_result_text(*kind, label, reason.as_deref());
        }
    }
    let mut parts = Vec::new();
    if b.finished > 0 {
        parts.push(format!("{} finished", downloads(b.finished)));
    }
    if b.warnings > 0 {
        let lead = if parts.is_empty() {
            format!("{} finished with warnings", downloads(b.warnings))
        } else {
            format!("{} with warnings", b.warnings)
        };
        parts.push(lead);
    }
    if b.failed > 0 {
        let lead = if parts.is_empty() {
            format!("{} failed", downloads(b.failed))
        } else {
            format!("{} failed", b.failed)
        };
        parts.push(lead);
    }
    let title = if b.finished == 0 && b.warnings == 0 {
        "Downloads failed"
    } else {
        "Downloads finished"
    };
    (title.to_string(), parts.join(", "))
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
/// `utils::process::classify_error` gives. Never the raw error text: that
/// is technical, can be long, and can name files and addresses.
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
        _ => DEFAULT_FAILURE_REASON,
    }
}

/// The burst being collected, if one is.
static BURST: std::sync::LazyLock<std::sync::Mutex<Option<BurstCounts>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Adds a download result to the current burst, starting one (and its
/// ten-second timer) if none is running.
fn record_in_burst(app: &AppHandle, kind: NotifyKind, label: &str, reason: Option<&str>) {
    let starts_burst = {
        let mut guard = BURST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let starts = guard.is_none();
        guard
            .get_or_insert_with(BurstCounts::default)
            .add(kind, label, reason);
        starts
    };
    if starts_burst {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(BURST_WINDOW_SECS)).await;
            let finished = BURST
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(counts) = finished {
                let (title, body) = burst_summary(&counts);
                send_system_notification(&app, &title, &body);
            }
        });
    }
}

// ============================================================
// Results since the queue last finished
// ============================================================

/// Downloads that finished and failed since the queue last finished, for
/// the "Queue finished: 5 downloaded, 1 failed" message.
static QUEUE_RUN: std::sync::LazyLock<std::sync::Mutex<(u32, u32)>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new((0, 0)));

fn count_for_queue_run(kind: NotifyKind) {
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
    /// A short plain reason, for failures.
    pub reason: Option<String>,
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

/// Delivers a backend-owned message: system notification (when the rule
/// says so) and the in-app copy (through the `app-notification` event).
pub fn notify(app: &AppHandle, event: NotifyEvent) {
    count_for_queue_run(event.kind);
    let settings = saved_settings(app);
    let attention = probe_attention(app);
    let decision = decide(event.kind, &settings, &attention);

    if decision.system {
        if event.kind.is_download_result() {
            let label = event.label.as_deref().unwrap_or("A download");
            record_in_burst(app, event.kind, label, event.reason.as_deref());
        } else {
            send_unless_repeat(app, &event.title, &event.body);
        }
    }

    let payload = AppNotificationPayload {
        kind: event.kind,
        title: event.title,
        body: event.body,
        toast_type: event.toast_type,
        label: event.label,
        show_in_app: decision.in_app || always_in_app(event.toast_type),
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
            reason: None,
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
            reason: Some(reason.to_string()),
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
            reason: None,
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
            reason: None,
        },
    );
    // `notify` counted nothing for this kind, but be explicit: the next
    // run starts from zero.
    reset_queue_run_counts();
}

/// The computer is about to shut down, restart or sleep.
pub fn notify_power_action(app: &AppHandle, body: &str) {
    notify(
        app,
        NotifyEvent {
            kind: NotifyKind::PowerAction,
            title: "MeedyaDL".to_string(),
            body: body.to_string(),
            toast_type: "warning",
            label: None,
            reason: None,
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
    let attention = probe_attention(app);
    let decision = decide(kind, &settings, &attention);
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
    fn a_burst_of_one_names_the_item() {
        let mut b = BurstCounts::default();
        b.add(NotifyKind::DownloadComplete, "Artist — Album", None);
        assert_eq!(
            burst_summary(&b),
            (
                "Download finished".to_string(),
                "Artist — Album finished.".to_string()
            )
        );

        let mut f = BurstCounts::default();
        f.add(
            NotifyKind::DownloadFailed,
            "Artist — Album",
            Some("Check your internet connection."),
        );
        assert_eq!(
            burst_summary(&f).1,
            "Artist — Album could not be downloaded. Check your internet connection."
        );
    }

    #[test]
    fn a_burst_of_several_is_summed_up() {
        let mut b = BurstCounts::default();
        for _ in 0..3 {
            b.add(NotifyKind::DownloadComplete, "x", None);
        }
        b.add(NotifyKind::DownloadFailed, "y", None);
        assert_eq!(
            burst_summary(&b),
            (
                "Downloads finished".to_string(),
                "3 downloads finished, 1 failed".to_string()
            )
        );

        let mut all_failed = BurstCounts::default();
        all_failed.add(NotifyKind::DownloadFailed, "a", None);
        all_failed.add(NotifyKind::DownloadFailed, "b", None);
        assert_eq!(
            burst_summary(&all_failed),
            (
                "Downloads failed".to_string(),
                "2 downloads failed".to_string()
            )
        );

        let mut mixed = BurstCounts::default();
        mixed.add(NotifyKind::DownloadComplete, "a", None);
        mixed.add(NotifyKind::DownloadWarnings, "b", None);
        assert_eq!(
            burst_summary(&mixed).1,
            "1 download finished, 1 with warnings"
        );
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
        assert!(recent.should_send("a", t0));
        assert!(!recent.should_send("a", t0 + std::time::Duration::from_secs(9)));
        assert!(recent.should_send("b", t0 + std::time::Duration::from_secs(9)));
        assert!(recent.should_send("a", t0 + std::time::Duration::from_secs(11)));
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
