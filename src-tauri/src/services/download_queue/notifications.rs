// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Desktop notifications, notification throttling, and after-queue actions.
//
// Extracted verbatim from the former single-file `download_queue.rs`
// during the behaviour-preserving module split. `use super::*;` pulls in
// the shared imports and sibling items re-exported by the module root.

use super::*;

/// Strips query parameters AND userinfo credentials from a URL before
/// logging, preventing credential leakage into plaintext log files.
///
/// Wrapper URLs may contain authentication tokens as query parameters
/// (e.g., `http://host:port/?token=abc`) or embedded Basic-Auth-style
/// credentials in the URL's userinfo component
/// (`http://user:pass@host:port/...`). Delegates to
/// `crash_report_service::redact_single_url`, which already implements
/// both redactions, so the two call sites can't drift out of sync.
pub(crate) fn redact_url_query(url: &str) -> String {
    crate::services::crash_report_service::redact_single_url(url)
}

/// Notification throttling state: tracks last notification time per category
/// and batched count to prevent notification spam during rapid queue processing.
pub(crate) static NOTIFICATION_THROTTLE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, u32)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Minimum interval between notifications of the same category (seconds).
pub(crate) const NOTIFICATION_THROTTLE_SECS: u64 = 10;

/// Sends a native OS desktop notification if the setting is enabled and the
/// main application window is not focused.
///
/// This avoids interrupting users who are actively watching the queue. When the
/// window is minimized, in the background, or the user has switched to another
/// app, a notification alerts them that a download has completed or failed.
///
/// Silently does nothing if:
/// - The `desktop_notifications` setting is `false`.
/// - The main window is currently focused (visible and in foreground).
/// - The notification fails to build or send (non-critical).
pub(crate) fn send_desktop_notification(app: &AppHandle, title: &str, body: &str) {
    use tauri::Manager;
    use tauri_plugin_notification::NotificationExt;

    // Check if desktop notifications are enabled in user settings
    let settings = load_settings_for_queue(app);
    if !settings.desktop_notifications {
        return;
    }

    // Respect the user's notification style preference (#658).
    // The backend used to fire native notifications regardless of style, which
    // contradicted the `in_app_only` choice and gave the impression that the
    // setting did nothing. Skip the OS notification when the user picked
    // `in_app_only` — the in-app toast path is unaffected.
    if settings.notification_style == "in_app_only" {
        return;
    }

    // Only send notifications when the window is NOT focused.
    if let Some(window) = app.get_webview_window("main") {
        if window.is_focused().unwrap_or(false) {
            return;
        }
    }

    // Throttle: batch rapid notifications of the same title category.
    // If the same title was sent within the last 10 seconds, update the
    // count and modify the body to show "N downloads completed" etc.
    let throttle_key = title.to_string();
    let display_body = {
        let mut throttle = NOTIFICATION_THROTTLE.lock().unwrap_or_else(|e| e.into_inner());
        let now = std::time::Instant::now();
        let entry = throttle.entry(throttle_key).or_insert((now, 0));
        let elapsed = now.duration_since(entry.0);

        if elapsed < std::time::Duration::from_secs(NOTIFICATION_THROTTLE_SECS) {
            entry.1 += 1;
            if entry.1 > 1 {
                // Batch: update the body with count
                format!("{} ({} items)", body, entry.1)
            } else {
                body.to_string()
            }
        } else {
            // Reset: enough time has passed
            *entry = (now, 1);
            body.to_string()
        }
    };

    // Send the OS-native notification.
    //
    // Instrumentation (#834): the previous `.ok()` swallowed every
    // failure silently, which made it impossible to tell whether
    // notifications were being dropped at the plugin layer, the
    // OS permission layer, or somewhere else. Now log both arms
    // through tracing so the on-disk log (#541) captures the truth
    // for any future bug report. The user's in-app activity log is
    // *not* spammed — these are OS-pipeline events, not download
    // events.
    match app
        .notification()
        .builder()
        .title(title)
        .body(&display_body)
        .show()
    {
        Ok(()) => {
            log::debug!(
                "desktop notification sent: title={:?} body={:?}",
                title,
                display_body
            );
        }
        Err(e) => {
            log::warn!(
                "desktop notification FAILED: title={:?} error={:?} \
                 (likely OS-level: permission revoked, Focus mode, or sandbox block)",
                title,
                e
            );
        }
    }
}

/// Sends a one-off test notification through the **real** backend
/// pipeline so the user can self-diagnose why OS notifications are
/// or aren't appearing.
///
/// Differs from `send_desktop_notification` in two ways:
/// - Bypasses the focus check. The user is clicking "Send Test
///   Notification" while the app is focused (by definition — they're
///   on a Settings page); we don't want to silently no-op on them.
/// - Bypasses the throttle. They might click the button repeatedly.
///
/// Otherwise hits the exact same plugin entrypoint, so a successful
/// test means the production path will also work; a failure surfaces
/// the actual reason via the returned `Err`.
///
/// Closes #834 (instrumentation half).
pub fn test_desktop_notification(
    app: &AppHandle,
) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;

    let settings = load_settings_for_queue(app);
    if !settings.desktop_notifications {
        return Err(
            "Desktop Notifications toggle is off. \
             Turn it on in Settings → General → Notifications first."
                .to_string(),
        );
    }
    if settings.notification_style == "in_app_only" {
        return Err(
            "Notification Style is set to 'In-app only'. \
             Switch to 'Native + in-app' or 'Native only' to test the OS pipeline."
                .to_string(),
        );
    }

    app.notification()
        .builder()
        .title("MeedyaDL — Backend Test")
        .body(
            "If you can read this, the native notification pipeline is working \
             from the Rust side. If you don't see this notification, check macOS \
             System Settings → Notifications → MeedyaDL.",
        )
        .show()
        .map_err(|e| {
            format!(
                "OS-level send failed: {e}. \
                 Likely causes: macOS notification permission revoked, \
                 Focus / Do Not Disturb mode enabled, or the app bundle \
                 missing from System Settings → Notifications."
            )
        })
}

/// Executes the configured after-queue action when the queue becomes idle.
///
/// Checks `after_queue_once` first (one-shot override), then `after_queue_action`
/// (persistent). One-shot actions are cleared after execution. Called after
/// `on_task_finished()` when `is_idle()` returns true.
pub(crate) fn execute_after_queue_action(app: &AppHandle) {
    use crate::models::settings::AfterQueueAction;

    let mut settings = load_settings_for_queue(app);

    // Resolve which action to execute: one-shot overrides persistent.
    // `take()` empties `after_queue_once`, so we must capture whether it was
    // set BEFORE the take — otherwise the persist-and-clear block below is
    // dead code (`is_some()` on the already-emptied field is always false),
    // and a one-shot that was ever persisted to settings.json would re-fire
    // on every subsequent queue completion (e.g. "Shut down" firing forever).
    let one_shot = settings.after_queue_once.take(); // consume one-shot
    let had_one_shot = one_shot.is_some();
    let action = one_shot.unwrap_or(settings.after_queue_action);

    // Persist the cleared one-shot to disk when one was set. `take()` above
    // already set `settings.after_queue_once = None`, so writing `settings`
    // now records the cleared state.
    if had_one_shot {
        // Save updated settings (clear the one-shot flag)
        let data_dir = crate::utils::platform::get_app_data_dir(app);
        let settings_path = data_dir.join("settings.json");
        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            let _ = std::fs::write(settings_path, json);
        }
        // #690: also refresh the in-process settings cache so the
        // next reader after a one-shot clear sees the post-clear
        // value without re-reading disk. Without this, every read
        // until the user next saves settings would see the stale
        // pre-clear after_queue_once = Some(...) value.
        use tauri::Manager as _;
        if let Some(cache) =
            app.try_state::<super::settings_cache::SettingsCache>()
        {
            cache.refresh(settings.clone());
        }
    }

    match action {
        AfterQueueAction::DoNothing => {}
        AfterQueueAction::OpenOutputFolder => {
            let path = if settings.output_path.is_empty() {
                crate::services::config_service::get_default_output_path()
                    .unwrap_or_else(|_| ".".to_string())
            } else {
                settings.output_path.clone()
            };
            // Open the folder in the system file manager using platform-native commands
            #[cfg(target_os = "macos")]
            { let _ = std::process::Command::new("open").arg(&path).spawn(); }
            #[cfg(target_os = "windows")]
            { let _ = std::process::Command::new("explorer").arg(&path).spawn(); }
            #[cfg(target_os = "linux")]
            { let _ = std::process::Command::new("xdg-open").arg(&path).spawn(); }
            log::info!("After-queue: opened output folder {path}");
            emit_app_log(app, &format!("After-queue action: opened output folder ({path})"));
        }
        AfterQueueAction::PlaySound => {
            send_desktop_notification(app, "Queue Complete", "All downloads finished.");
            log::info!("After-queue: played notification sound");
            emit_app_log(app, "After-queue action: notification sound");
        }
        AfterQueueAction::CloseMeedyadl => {
            log::info!("After-queue: closing MeedyaDL");
            emit_app_log(app, "After-queue action: closing MeedyaDL...");
            // Brief delay to let the activity log event propagate
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                app_clone.exit(0);
            });
        }
        AfterQueueAction::RestartComputer => {
            log::info!("After-queue: restarting computer");
            emit_app_log(app, "After-queue action: restarting computer in 30 seconds...");
            send_desktop_notification(app, "MeedyaDL", "Computer will restart in 30 seconds...");
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to restart"])
                    .spawn();
            }
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("shutdown")
                    .args(["/r", "/t", "30"])
                    .spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("systemctl")
                    .args(["reboot"])
                    .spawn();
            }
        }
        AfterQueueAction::HibernateComputer => {
            log::info!("After-queue: hibernating computer");
            emit_app_log(app, "After-queue action: hibernating computer...");
            #[cfg(target_os = "macos")]
            {
                // macOS uses sleep (pmset sleepnow) — true hibernate requires
                // hibernatemode 25 which most Macs don't use by default.
                let _ = std::process::Command::new("pmset").arg("sleepnow").spawn();
            }
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("shutdown")
                    .args(["/h"])
                    .spawn();
            }
            #[cfg(target_os = "linux")]
            {
                // systemctl hibernate requires swap; falls back gracefully
                let _ = std::process::Command::new("systemctl")
                    .args(["hibernate"])
                    .spawn();
            }
        }
        AfterQueueAction::ShutdownComputer => {
            log::info!("After-queue: shutting down computer");
            emit_app_log(app, "After-queue action: shutting down computer in 30 seconds...");
            send_desktop_notification(app, "MeedyaDL", "Computer will shut down in 30 seconds...");
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to shut down"])
                    .spawn();
            }
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("shutdown")
                    .args(["/s", "/t", "30"])
                    .spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = std::process::Command::new("systemctl")
                    .args(["poweroff"])
                    .spawn();
            }
        }
    }
}

