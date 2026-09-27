// Copyright (c) 2024-2026 MeedyaSuite
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
        // Clear the one-shot through the same narrow, serialised path
        // everything else now uses.
        //
        // This used to be a raw `std::fs::write` of the whole cached
        // settings object, which was wrong in two ways.
        //
        // First, it skipped the atomic rename, the 0600 permissions and
        // the `.sha256` companion file. Because the checksum was never
        // updated, the next startup compared the new file against the old
        // checksum, found a mismatch, and warned that the settings file
        // "may have been modified externally" — after every single
        // one-shot action, with nothing actually wrong.
        //
        // Second, it wrote back every OTHER field from a snapshot that
        // could be minutes old by the time the queue finished, quietly
        // undoing anything changed in between.
        //
        // `update_settings_field` re-reads the file, changes only this
        // field, writes it back properly, and refreshes the in-process
        // cache itself — so the separate cache refresh that used to sit
        // here is no longer needed (#690's requirement is still met).
        // update_settings_field_and_memory clears the running app's copy
        // too, even if the file write fails, and does both inside the
        // settings lock — see its doc for the race this closes.
        if let Err(e) =
            crate::services::config_service::update_settings_field_and_memory(app, |s| {
                s.after_queue_once = None;
            })
        {
            log::warn!("Failed to clear the one-shot after-queue action: {e}");

            // The write failed — a full disk, a read-only folder. The
            // running app's copy has still been cleared (inside the lock, by
            // update_settings_field_and_memory), so the action cannot fire
            // again this session: every other settings write keeps the
            // running app's value for this field rather than the file's
            // stale one (config_service::one_off_in_memory), so none of them
            // can put it back.
            //
            // What remains: the FILE still has it until the next successful
            // settings write (any write corrects it, taking the running
            // app's value). If MeedyaDL is restarted before one happens, the
            // file's stale value is loaded again. That is the safer of the
            // two wrong answers: doing it twice in one session is worse than
            // doing it once more after a restart the person chose to make.
            //
            // (Earlier versions said a restart was the ONLY way it could
            // come back; a stand-in review, 24 Sept 2026, found any
            // successful one-field write also brought it back, until every
            // writer was made to keep the running app's value.)
            // The running app's copy has already been cleared, inside the
            // lock, by update_settings_field_and_memory. (This used to be
            // done here, after the lock was released — which left a gap a
            // Save could slip into and re-arm the action.)
        }

        // Tell the page the one-off has been used. The page keeps its own
        // copy of the settings, and nothing else would tell it: the status
        // bar went on showing a finished action as still armed, and that
        // stale copy is what a later whole-settings Save used to write back
        // (see save_settings, which now ignores it — this keeps the screen
        // honest too). Best effort: a page that misses it is corrected the
        // next time the settings are read.
        use tauri::Emitter as _;
        if let Err(e) = app.emit("after-queue-once-used", ()) {
            log::debug!("Could not tell the page the one-off after-queue action was used: {e}");
        }
    }

    match action {
        AfterQueueAction::DoNothing => {}
        AfterQueueAction::OpenOutputFolder => {
            let path = if settings.output_path.is_empty() {
                match crate::services::config_service::get_default_output_path() {
                    Ok(p) => p,
                    Err(e) => {
                        // This used to fall back to ".", the folder the app
                        // happened to be started from — not the output
                        // folder, and not anything the person chose.
                        log::warn!("After-queue: no output folder to open ({e})");
                        emit_app_log(
                            app,
                            "After-queue action: did not open the output folder, because \
                             MeedyaDL could not work out where it is",
                        );
                        return;
                    }
                }
            } else {
                settings.output_path.clone()
            };
            if let Err(why) = output_folder_is_safe_to_open(&path) {
                log::warn!("After-queue: did not open {path}: {why}");
                emit_app_log(
                    app,
                    &format!("After-queue action: did not open the output folder ({path}) — {why}"),
                );
                return;
            }
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

/// Decides whether `path` may be handed to the system file manager as
/// "the output folder". Returns why not, in plain words, when it may not.
///
/// **Why this check exists.** The after-queue action opens this path with
/// the system's own "open" command — `open` on macOS, `explorer` on
/// Windows, `xdg-open` on Linux. Those open whatever they are given the
/// way a double-click would, and double-clicking a program runs it:
/// `explorer C:\somewhere\program.exe` starts the program. The path is an
/// ordinary setting: the page saves it, and an imported settings file
/// deliberately carries it across (see `preserve_local_only_settings`).
/// So without this check, anything able to change that one setting could
/// have a program run when the queue finished. It is the same kind of
/// door as finding 2 of #1215 (the page could get any file opened), found
/// beside it.
///
/// So this acts only on a folder that exists. Asking "is it a folder?"
/// follows a symbolic link to wherever it leads, which is also what the
/// file manager will act on: a link to a folder is accepted — people do
/// keep their music behind one — and a link to a file is refused, like
/// the file itself. A Windows `.lnk` shortcut and a macOS Finder alias are
/// ordinary files, so they are refused as files.
///
/// On macOS there is one more case. An application is a FOLDER (a
/// "bundle" whose name ends in `.app`), and `open` starts it rather than
/// showing what is inside. So a folder whose real name — after following
/// any link — ends in `.app` is refused too. If the real name cannot be
/// worked out, it is refused rather than given the benefit of the doubt.
///
/// **What this cannot do.** It checks, and the file manager is started a
/// moment later; something able to swap the folder for something else in
/// between could still get that opened. That needs control of the disk,
/// not just of a setting. On macOS it knows only about `.app`. Other
/// kinds of bundle (a document saved as a folder, for instance) are
/// opened in the program they belong to, which is opening a document
/// rather than running a program — but that has not been checked for
/// every kind of bundle macOS knows about.
pub(crate) fn output_folder_is_safe_to_open(path: &str) -> Result<(), String> {
    // `metadata` follows a symbolic link to where it leads — the same
    // thing the file manager will be looking at. (The file-opening
    // command in `commands/history.rs` refuses links outright instead;
    // a folder of music behind a link is ordinary, a music FILE behind
    // one is not, which is why the two differ.)
    let meta = std::fs::metadata(path)
        .map_err(|_| "it is not there any more, or MeedyaDL cannot look at it".to_string())?;
    if !meta.is_dir() {
        return Err("it is not a folder, and MeedyaDL only opens folders here".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let real = std::fs::canonicalize(path)
            .map_err(|_| "MeedyaDL could not check what it really is".to_string())?;
        let is_application = real
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("app"));
        if is_application {
            return Err(
                "it is an application, which macOS would start instead of showing".to_string(),
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::output_folder_is_safe_to_open;

    // ── "Open output folder" must never run a program (#1215) ───────────
    //
    // Each test works in its own fresh folder, on a real disk, so what is
    // checked is what the file manager would really have been handed.

    fn text(path: &std::path::Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn a_real_folder_is_opened() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(output_folder_is_safe_to_open(&text(dir.path())), Ok(()));
    }

    #[test]
    fn a_program_is_refused() {
        // What `explorer` on Windows would have started.
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("program.exe");
        std::fs::write(&program, b"MZ").unwrap();
        let refused = output_folder_is_safe_to_open(&text(&program)).unwrap_err();
        assert!(refused.contains("not a folder"), "{refused}");
    }

    #[test]
    fn a_missing_or_empty_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(output_folder_is_safe_to_open(&text(&dir.path().join("gone"))).is_err());
        assert!(output_folder_is_safe_to_open("").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_file_is_refused_and_a_link_to_a_folder_is_not() {
        let dir = tempfile::tempdir().unwrap();

        let program = dir.path().join("payload.sh");
        std::fs::write(&program, b"#!/bin/sh\necho pwned\n").unwrap();
        let link_to_program = dir.path().join("Music");
        std::os::unix::fs::symlink(&program, &link_to_program).unwrap();
        assert!(
            output_folder_is_safe_to_open(&text(&link_to_program)).is_err(),
            "a folder-looking name that leads to a program must be refused"
        );

        let folder = dir.path().join("Real Music");
        std::fs::create_dir(&folder).unwrap();
        let link_to_folder = dir.path().join("Music Link");
        std::os::unix::fs::symlink(&folder, &link_to_folder).unwrap();
        assert_eq!(
            output_folder_is_safe_to_open(&text(&link_to_folder)),
            Ok(())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_macos_application_is_refused_even_though_it_is_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["Something.app", "Shouting.APP"] {
            let bundle = dir.path().join(name);
            std::fs::create_dir(&bundle).unwrap();
            assert!(bundle.is_dir(), "an application really is a folder");
            let refused = output_folder_is_safe_to_open(&text(&bundle)).unwrap_err();
            assert!(refused.contains("application"), "{name}: {refused}");
        }

        // Behind a link with an innocent name, it is still an application.
        let link = dir.path().join("Music");
        std::os::unix::fs::symlink(dir.path().join("Something.app"), &link).unwrap();
        assert!(output_folder_is_safe_to_open(&text(&link)).is_err());
    }
}
