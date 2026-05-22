// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Core library for the MeedyaDL Tauri application.
// ====================================================
//
// This is the central wiring module that connects every part of the Rust
// backend together and exposes it to the React/TypeScript frontend. It is
// responsible for:
//
//   - Declaring the four sub-module trees (commands, models, services, utils)
//   - Registering Tauri plugins that provide native OS capabilities
//   - Registering all IPC command handlers callable from the frontend
//   - Injecting managed state (the download queue) into the Tauri runtime
//   - Setting up the system tray icon with its context menu and event handlers
//   - Bootstrapping the Tauri event loop via `Builder::default().run()`
//
// Architecture overview:
//   main.rs  -->  lib.rs::run()
//                   |
//                   +-- commands/   (thin IPC wrappers -- #[tauri::command] fns)
//                   +-- services/   (business logic -- Python, GAMDL, queue, etc.)
//                   +-- models/     (shared data types -- serde structs/enums)
//                   +-- utils/      (cross-cutting helpers -- platform, archive, process)
//
// Reference: https://v2.tauri.app/develop/
// Reference: https://v2.tauri.app/develop/calling-rust/
// Reference: https://docs.rs/tauri/latest/tauri/

// ---------------------------------------------------------------------------
// Sub-module declarations.
// Each `pub mod` makes the module available to other crates (e.g., tests)
// and to the rest of this library. Rust resolves these to the corresponding
// `src/{name}/mod.rs` file on disk.
// Reference: https://doc.rust-lang.org/reference/items/modules.html
// ---------------------------------------------------------------------------

/// IPC command handlers exposed to the React frontend via `invoke()`.
///
/// Each sub-module groups related commands (system, dependencies, settings,
/// gamdl, credentials, updates). Commands are thin wrappers that validate
/// inputs and delegate to the `services` layer.
pub mod commands;

/// Shared data models used across commands, services, and IPC payloads.
///
/// All models derive `Serialize` (and often `Deserialize`) so they can
/// cross the Rust <-> TypeScript boundary automatically via Tauri's
/// JSON serialization layer.
pub mod models;

/// Business-logic services that perform the actual work: managing the
/// Python runtime, orchestrating GAMDL downloads, checking for updates,
/// reading/writing settings, and managing the download queue.
pub mod services;

/// Cross-cutting utility modules for platform detection, archive
/// extraction, and subprocess output parsing.
pub mod utils;

// ---------------------------------------------------------------------------
// Helper functions extracted from `run()` to keep it under the 100-line
// clippy::too_many_lines threshold. Each helper encapsulates a distinct
// phase of application startup.
// ---------------------------------------------------------------------------

/// Initialises the `tracing` subscriber with file logging and optional stderr:
///
/// 1. **stderr** (dev/debug only) -- Coloured, human-readable output for the
///    terminal. Only active in debug builds or when `RUST_LOG` is explicitly
///    set. Suppressed in release builds to avoid flooding the terminal when
///    the app is launched from a command line.
/// 2. **Rolling file** -- Daily-rotated log files written to
///    `{app_data_dir}/logs/` with the prefix `meedyadl`. Files are named
///    `meedyadl.YYYY-MM-DD.log` and old files are kept for 7 days.
///
/// If Sentry is enabled in the user's settings, a `sentry_tracing::layer()`
/// is added to the subscriber stack so that `error!()` events are forwarded
/// to Sentry and lower-level events become breadcrumbs.
///
/// All existing `log::info!()` / `log::error!()` calls throughout the codebase
/// continue to work unchanged because `tracing` is compatible with the `log`
/// facade via its built-in bridge.
///
/// # Arguments
/// * `sentry_enabled` -- Whether the Sentry tracing layer should be active.
///
/// # Returns
/// A `WorkerGuard` that **must** be kept alive for the application's lifetime.
/// Dropping it flushes and closes the file appender. Bind it to a named
/// variable (e.g., `let _guard = ...`) in the caller.
fn setup_tracing(sentry_enabled: bool) -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    // Determine the log directory. Use the platform app data dir if available,
    // otherwise fall back to the OS temp dir.
    let log_dir = dirs::data_dir()
        .map(|d| d.join("io.github.meedyadl").join("logs"))
        .unwrap_or_else(|| std::env::temp_dir().join("MeedyaDL").join("logs"));

    // Ensure the log directory exists
    let _ = std::fs::create_dir_all(&log_dir);

    // Create a daily-rotating file appender: meedyadl.YYYY-MM-DD.log
    let file_appender = tracing_appender::rolling::daily(&log_dir, "meedyadl");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Environment filter: respect RUST_LOG, default to `info` for our crate
    // and `warn` for everything else to keep logs manageable.
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("meedyadl=info,warn"));

    // stderr layer: coloured, with timestamps, for dev console.
    // Only enabled in debug builds or when RUST_LOG is explicitly set, so that
    // release builds launched from a terminal don't flood it with log output.
    let stderr_layer = if cfg!(debug_assertions) || std::env::var("RUST_LOG").is_ok() {
        Some(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(false),
        )
    } else {
        None
    };

    // Build the layered subscriber
    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer)
        // File layer: plain text (no ANSI colours), with timestamps
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true),
        );

    // Conditionally add the Sentry tracing layer
    if sentry_enabled {
        registry.with(sentry_tracing::layer()).init();
    } else {
        registry.init();
    }

    guard
}

/// Deletes log files older than their retention period from each of the
/// logging directories used by MeedyaDL.
///
/// Three file classes with independent retention windows:
///
/// | Prefix                | Contents                              | Retention |
/// | --------------------- | ------------------------------------- | --------- |
/// | `meedyadl.*`          | `tracing` structured log output       | 7 days    |
/// | `session-*`           | Trimmed activity-log entries (dumped  | 30 days   |
/// |                       |   from `activityStore` on overflow)   |           |
/// | `activity-*` (#541)   | Persistent on-disk activity log       | 7 days    |
///
/// `tracing_appender::rolling::daily()` creates new log files daily but
/// never deletes old ones. The `activity_log_writer` background task
/// similarly appends forever. Without cleanup, log files accumulate
/// indefinitely. This function provides the same retention policy as
/// `clear_old_reports()` does for crash reports.
///
/// # Arguments
/// * `app_data_dir` — Default app data directory (used for default log folder).
/// * `extra_log_dir` — Optional user-chosen override directory from
///   `AppSettings::activity_log_path_override`. When set and different
///   from the default, it is scanned too so `activity-*.log` files in
///   the override location also honour the 7-day retention window.
fn clear_old_logs(app_data_dir: &std::path::Path, extra_log_dir: Option<&std::path::Path>) {
    let default_log_dir = app_data_dir.join("logs");
    // Deduplicate directories so we don't scan the same path twice when
    // the override happens to equal the default.
    let mut dirs: Vec<std::path::PathBuf> = vec![default_log_dir];
    if let Some(extra) = extra_log_dir {
        if !extra.as_os_str().is_empty() && !dirs.iter().any(|d| d == extra) {
            dirs.push(extra.to_path_buf());
        }
    }

    // Tracing + activity logs: 7 days retention
    let seven_day_cutoff =
        std::time::SystemTime::now() - std::time::Duration::from_secs(7 * 24 * 60 * 60);
    // Session logs: 30 days retention (longer — they contain forensic
    // data from entries that were trimmed out of the in-memory log).
    let session_cutoff =
        std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 24 * 60 * 60);

    let mut removed = 0u32;
    for log_dir in &dirs {
        let Ok(entries) = std::fs::read_dir(log_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let filename = entry.file_name();
            let name = filename.to_string_lossy();
            let cutoff = if name.starts_with("session-") {
                session_cutoff
            } else {
                // `meedyadl.*` and `activity-*` both get the 7-day window.
                seven_day_cutoff
            };
            if modified < cutoff && std::fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }
    }
    if removed > 0 {
        log::info!(
            "Cleaned up {removed} log file(s) (tracing/activity: >7d, session: >30d)"
        );
    }
}

/// Resolves the directory for the persistent on-disk activity log (#541).
///
/// Honours `AppSettings::activity_log_path_override` when set to a
/// non-empty, non-whitespace value. Falls back to
/// `{app_data_dir}/logs/` on any of the following conditions:
/// * The setting is empty or contains only whitespace.
/// * The override path cannot be created (permission issue, missing
///   mount, invalid characters). We log a warning and fall back rather
///   than crashing — a non-functional log override must not prevent
///   the app from starting.
fn resolve_activity_log_dir(
    app: &tauri::AppHandle,
    app_data_dir: &std::path::Path,
) -> std::path::PathBuf {
    let default_dir = app_data_dir.join("logs");
    let override_path = services::config_service::load_settings(app)
        .ok()
        .map(|s| s.activity_log_path_override)
        .unwrap_or_default();
    let trimmed = override_path.trim();
    if trimmed.is_empty() {
        return default_dir;
    }
    let candidate = std::path::PathBuf::from(trimmed);
    match std::fs::create_dir_all(&candidate) {
        Ok(()) => candidate,
        Err(e) => {
            log::warn!(
                "activity_log: override path {:?} is not writable ({e}); \
                 falling back to default {:?}",
                candidate,
                default_dir
            );
            default_dir
        }
    }
}

/// Installs a custom panic hook that writes structured JSON crash reports
/// to `{app_data_dir}/crashes/` before aborting.
///
/// The crash report includes: panic message, backtrace, app version, OS,
/// architecture, and timestamp. This provides diagnostic information even
/// when the app crashes outside of a debugger.
///
/// The original default hook is preserved and called after writing the
/// crash report, so the standard panic message still appears on stderr.
fn setup_panic_handler() {
    let default_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        // Capture the backtrace
        let backtrace = std::backtrace::Backtrace::force_capture();

        // Extract the panic message
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        // Extract location info
        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));

        // Build the crash report JSON
        let report = serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "app_version": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "source": "rust_panic",
            "panic_message": message,
            "location": location,
            "backtrace": backtrace.to_string(),
            "context": {}
        });

        // Write the crash report to disk
        let crash_dir = dirs::data_dir()
            .map(|d| d.join("io.github.meedyadl").join("crashes"))
            .unwrap_or_else(|| std::env::temp_dir().join("MeedyaDL").join("crashes"));

        if std::fs::create_dir_all(&crash_dir).is_ok() {
            let filename = format!("crash-{}.json", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
            let path = crash_dir.join(&filename);
            if let Ok(json) = serde_json::to_string_pretty(&report) {
                let _ = std::fs::write(&path, json);
                eprintln!("Crash report saved to: {}", path.display());
            }
        }

        // Call the original panic hook so the standard message still appears
        default_hook(panic_info);
    }));
}

/// Creates and configures the system tray icon with its context menu and
/// event handlers.
///
/// Managed state holding the tray icon ID so other modules (e.g., download_queue)
/// can update the tray tooltip to show download progress.
pub struct TrayState(pub tauri::tray::TrayIconId);

/// Updates the system tray icon tooltip with the current queue status.
/// Called from download_queue after state changes.
pub fn update_tray_tooltip(app: &tauri::AppHandle, active: usize, queued: usize, completed: usize) {
    use tauri::Manager;
    if let Some(tray_state) = app.try_state::<TrayState>() {
        if let Some(tray) = app.tray_by_id(&tray_state.0) {
            let tooltip = if active > 0 {
                format!("MeedyaDL — {active} downloading, {queued} queued")
            } else if completed > 0 {
                format!("MeedyaDL — {completed} completed")
            } else {
                "MeedyaDL".to_string()
            };
            let _ = tray.set_tooltip(Some(&tooltip));
        }
    }
}

/// The system tray icon allows the user to interact with the application
/// even when the main window is hidden or minimised. Tauri 2.0's tray API
/// is builder-based: we construct menu items, compose them into a menu,
/// attach the menu to a `TrayIconBuilder`, and register event handlers for
/// clicks.
///
/// # Returns
/// The `TrayIcon` instance, which **must** be kept alive (bound to a
/// variable) for the duration of the application. Dropping the return
/// value would remove the icon from the system tray.
///
/// # Errors
/// Returns an error if any tray menu item or the tray icon itself fails
/// to build (e.g., missing platform support).
///
/// # Reference
/// - System tray guide: <https://v2.tauri.app/develop/system-tray/>
/// - `TrayIcon` API: <https://docs.rs/tauri/latest/tauri/tray/index.html>
/// - Menu API: <https://docs.rs/tauri/latest/tauri/menu/index.html>
fn setup_system_tray(
    app: &tauri::App,
) -> Result<tauri::tray::TrayIcon, Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder};
    // `Manager` trait provides `.get_webview_window()` on AppHandle
    use tauri::Manager;
    // `Emitter` trait provides `.emit()` for sending events to the frontend
    use tauri::Emitter;

    // Build the tray menu items
    // "Show Window" -- brings the main window to focus
    let show_item = MenuItemBuilder::with_id("show", "Show Window").build(app)?;

    // First separator -- visually groups window controls from status info
    let separator1 = PredefinedMenuItem::separator(app)?;

    // "Downloads: None" -- disabled info item that displays current download status.
    // The frontend can update this text via the tray menu API as downloads progress.
    let downloads_item = MenuItemBuilder::with_id("downloads_status", "Downloads: None")
        .enabled(false)
        .build(app)?;

    // Second separator -- visually groups status info from application actions
    let separator2 = PredefinedMenuItem::separator(app)?;

    // "Check for Updates" -- triggers an update check for the application and GAMDL
    let updates_item = MenuItemBuilder::with_id("check_updates", "Check for Updates").build(app)?;

    // "Quit MeedyaDL" -- cleanly exits the application
    let quit_item = MenuItemBuilder::with_id("quit", "Quit MeedyaDL").build(app)?;

    // Assemble the tray context menu from the items defined above
    let tray_menu = MenuBuilder::new(app)
        .item(&show_item)
        .item(&separator1)
        .item(&downloads_item)
        .item(&separator2)
        .item(&updates_item)
        .item(&quit_item)
        .build()?;

    // Build the tray icon, attach the menu, and register event handlers.
    //
    // IMPORTANT: The returned `TrayIcon` must be stored in a named binding
    // in the caller (e.g., `let _tray = ...`). Using `let _ = ...` (without
    // a name) would drop the `TrayIcon` immediately, removing it from the
    // system tray. A leading-underscore named binding keeps the value alive
    // for the lifetime of its enclosing scope.
    //
    // Reference: https://docs.rs/tauri/latest/tauri/tray/struct.TrayIconBuilder.html
    let tray = TrayIconBuilder::new()
        .menu(&tray_menu)
        // Register a handler for clicks on items within the tray
        // context menu. The `event.id()` corresponds to the string
        // ID passed to `MenuItemBuilder::with_id(...)` above.
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                // Show and focus the main window
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                // Trigger an update check by emitting an event to the frontend
                "check_updates" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit("tray-check-updates", ());
                    }
                }
                // Cleanly exit the application.
                // Trigger the shutdown signal first so background tasks
                // stop starting new work before the process exits.
                "quit" => {
                    let shutdown = app.state::<services::download_queue::ShutdownSignal>();
                    shutdown.trigger();
                    log::info!("Tray quit — shutdown signal sent to background tasks");
                    app.exit(0);
                }
                _ => {}
            }
        })
        // Handle direct clicks on the tray icon itself (not the context
        // menu). This uses Rust's pattern matching with struct
        // destructuring to match only left-button-up events, ignoring
        // right-clicks (which open the context menu), double-clicks,
        // and mouse-down events.
        //
        // On macOS, a left-click on the tray icon shows the context
        // menu by default; this handler provides an additional
        // "show window" shortcut on platforms where left-click is
        // separate from menu display (Windows, some Linux DEs).
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..  // Ignore position and other fields via `..` rest pattern
            } = event
            {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    log::info!("System tray icon initialized");

    // Store the tray icon's ID for later tooltip updates from download_queue
    app.manage(TrayState(tray.id().clone()));
    Ok(tray)
}

/// Restores persisted download queue items and schedules delayed processing.
///
/// Loads any persisted queue items from `queue.json` (written after every
/// queue mutation in the previous session). If items exist, they are restored
/// to the queue in `Queued` state and processing is started after a short
/// delay (2 seconds) to give the frontend event listeners time to initialise.
///
/// This provides crash recovery: if the app closes (or crashes) while
/// downloads are queued/active, those items are restored and automatically
/// resumed on next launch.
fn setup_queue_recovery(app: &tauri::App) {
    use tauri::Manager;

    let app_handle = app.handle().clone();
    let persisted_items = services::download_queue::load_queue_from_disk(&app_handle);

    if persisted_items.is_empty() {
        return;
    }

    let count = persisted_items.len();
    let settings = services::config_service::load_settings(&app_handle).unwrap_or_default();

    // Get the queue handle from managed state
    let queue_handle: tauri::State<'_, services::download_queue::QueueHandle> = app.state();
    let queue_arc = queue_handle.inner().clone();

    // Restore items synchronously (we can block briefly in setup).
    // Use `blocking_lock()` instead of `block_on(lock().await)` because
    // the Tokio runtime may not be set as the "current" runtime during
    // the `setup` closure (it runs inside the macOS `did_finish_launching`
    // callback). `blocking_lock()` works without an active Tokio context.
    {
        let mut q = queue_arc.blocking_lock();
        q.restore_items(persisted_items, &settings);
    }

    log::info!("Queue restored: {count} item(s) will resume after frontend initialises");

    // Spawn a delayed task to start processing the restored queue.
    // The 2-second delay ensures the frontend's Tauri event listeners
    // are registered before downloads start emitting events.
    // Use `tauri::async_runtime::spawn` instead of `tokio::spawn` because
    // the Tokio runtime may not be set as "current" during the `setup` closure.
    let queue_for_processing = queue_arc;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        utils::activity_log::emit_app_log(
            &app_handle,
            &format!("Restored {count} item(s) from previous session"),
        );
        services::download_queue::process_queue(app_handle, queue_for_processing).await;
    });
}

/// Parses a `meedyadl://` deep link URL and extracts the download parameters.
///
/// Supported URL format:
///   `meedyadl://download?url=<apple_music_url>[&codec=<codec>]`
///
/// # Arguments
/// * `deep_link_url` -- The full `meedyadl://` URL string to parse.
///
/// # Returns
/// A tuple of `(download_url, optional_codec)` if the URL is valid,
/// or `None` if the URL cannot be parsed or is missing required parameters.
fn parse_deep_link_url(deep_link_url: &str) -> Option<(String, Option<String>)> {
    // Parse the deep link URL. The `url` crate handles percent-decoding
    // and query parameter extraction.
    let parsed = url::Url::parse(deep_link_url).ok()?;

    // Verify the scheme is `meedyadl` and the host/path indicates a download action.
    // Accept both `meedyadl://download?...` and `meedyadl://download/?...`
    if parsed.scheme() != "meedyadl" {
        return None;
    }

    // The "host" in a custom scheme URL is the action (e.g., "download").
    // url::Url parses `meedyadl://download?url=...` with host = "download".
    let action = parsed.host_str().unwrap_or("");
    if action != "download" {
        log::warn!("Unknown deep link action: '{action}' (expected 'download')");
        return None;
    }

    // Extract the `url` query parameter (required).
    let mut download_url: Option<String> = None;
    let mut codec: Option<String> = None;

    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "url" => download_url = Some(value.to_string()),
            "codec" => codec = Some(value.to_string()),
            other => {
                log::debug!("Ignoring unknown deep link parameter: '{other}'");
            }
        }
    }

    let url = download_url?;
    if url.is_empty() {
        return None;
    }

    Some((url, codec))
}

/// Handles a list of deep link URLs by parsing each one and emitting a
/// `deep-link-download` event to the frontend for the first valid URL.
///
/// Also brings the main application window to the foreground so the user
/// can see the pre-filled download form.
///
/// # Arguments
/// * `app` -- The Tauri `AppHandle` for emitting events and accessing windows.
/// * `urls` -- The list of deep link URL strings received from the OS.
fn handle_deep_link_urls(app: &tauri::AppHandle, urls: Vec<url::Url>) {
    use tauri::Emitter;
    use tauri::Manager;

    for deep_url in &urls {
        let url_str = deep_url.as_str();
        log::info!("Deep link received: {url_str}");

        // Handle .meedyadl file associations (file:// URLs)
        if deep_url.scheme() == "file" {
            if let Ok(file_path) = deep_url.to_file_path() {
                if file_path.extension().and_then(|e| e.to_str()) == Some("meedyadl") {
                    log::info!("Manifest file opened: {}", file_path.display());
                    match std::fs::read_to_string(&file_path) {
                        Ok(contents) => {
                            match serde_json::from_str::<models::manifest::ManifestFile>(&contents)
                            {
                                Ok(manifest) => {
                                    let manifest_urls: Vec<String> =
                                        manifest.sources.iter().map(|s| s.url.clone()).collect();
                                    if !manifest_urls.is_empty() {
                                        // Join all URLs with newlines for multi-URL textarea input
                                        let joined_urls = manifest_urls.join("\n");
                                        let payload = serde_json::json!({
                                            "url": joined_urls,
                                            "codec": serde_json::Value::Null,
                                        });
                                        if let Err(e) = app.emit("deep-link-download", payload) {
                                            log::error!(
                                                "Failed to emit manifest download event: {e}"
                                            );
                                        }
                                        if let Some(window) = app.get_webview_window("main") {
                                            let _ = window.show();
                                            let _ = window.set_focus();
                                        }
                                        utils::activity_log::emit_app_log(
                                            app,
                                            &format!(
                                                "Manifest imported: {} source(s)",
                                                manifest_urls.len()
                                            ),
                                        );
                                        return;
                                    }
                                }
                                Err(e) => log::warn!("Failed to parse manifest: {e}"),
                            }
                        }
                        Err(e) => log::warn!("Failed to read manifest file: {e}"),
                    }
                }
            }
        }

        if let Some((download_url, codec)) = parse_deep_link_url(url_str) {
            log::info!(
                "Deep link parsed: url={download_url}, codec={}",
                codec.as_deref().unwrap_or("(default)")
            );

            // Emit a typed event to the frontend with the parsed parameters.
            // The frontend listens for `deep-link-download` and navigates to
            // the Download page with the URL pre-filled.
            let payload = serde_json::json!({
                "url": download_url,
                "codec": codec,
            });
            if let Err(e) = app.emit("deep-link-download", payload) {
                log::error!("Failed to emit deep-link-download event: {e}");
            }

            // Bring the main window to the foreground so the user sees the
            // pre-filled download form.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            // Log to the activity log
            utils::activity_log::emit_app_log(
                app,
                &format!("URL received from deep link: {download_url}"),
            );

            // Only process the first valid deep link URL
            return;
        }

        log::warn!("Failed to parse deep link URL: {url_str}");
    }
}

/// Registers the deep link URL handler for the `meedyadl://` custom scheme.
///
/// Sets up two handlers:
/// 1. **Runtime handler** (`on_open_url`): Called when the app is already running
///    and a deep link URL is opened by an external tool.
/// 2. **Startup handler** (`get_current`): Checks if the app was launched via a
///    deep link URL (i.e., the app was not running when the URL was opened).
///
/// Both handlers parse the URL and emit a `deep-link-download` event to the
/// frontend with the extracted download URL and optional codec parameter.
fn setup_deep_link_handler(app: &tauri::App) {
    use tauri_plugin_deep_link::DeepLinkExt;

    let app_handle = app.handle().clone();

    // Handler for deep links received while the app is already running.
    // The closure captures a clone of the AppHandle for event emission.
    app.deep_link().on_open_url(move |event| {
        let urls = event.urls();
        if !urls.is_empty() {
            handle_deep_link_urls(&app_handle, urls);
        }
    });

    // Check if the app was launched via a deep link URL.
    // This handles the case where the app was not running and the OS started
    // it in response to a `meedyadl://` URL being opened.
    // We delay this check so the frontend event listeners are ready.
    let startup_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        // Wait for frontend to initialise (same delay as queue recovery)
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        match startup_handle.deep_link().get_current() {
            Ok(Some(urls)) if !urls.is_empty() => {
                log::info!("App launched via deep link ({} URL(s))", urls.len());
                handle_deep_link_urls(&startup_handle, urls);
            }
            Ok(_) => {
                // No startup deep link — normal launch
            }
            Err(e) => {
                log::debug!("Could not check startup deep link: {e}");
            }
        }
    });

    log::info!("Deep link handler registered for meedyadl:// scheme");
}

/// Emit a concise summary of key settings at startup.
///
/// Logs the most diagnostically useful settings to the activity log
/// in three compact lines so users (and crash reports) can quickly
/// identify the active configuration. Always runs (not verbose-only).
/// Sensitive fields (credentials, paths, wrapper URLs) are redacted.
fn emit_startup_settings_summary(app: &tauri::AppHandle, s: &models::settings::AppSettings) {
    use utils::activity_log::emit_app_log;

    // Derive a short companion mode label from the serde serialization
    let companion_str = serde_json::to_value(&s.companion_mode)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", s.companion_mode));

    // Derive a short download mode label
    let dl_mode = serde_json::to_value(&s.download_mode)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", s.download_mode));

    // Storefront: show the configured value or "auto" if empty
    let storefront = if s.storefront.is_empty() {
        "auto"
    } else {
        &s.storefront
    };

    // Helper: "on"/"off" for booleans
    let flag = |b: bool| if b { "on" } else { "off" };

    // Line 1: Core download configuration
    emit_app_log(
        app,
        &format!(
            "Config: codec={}, video={}, companions={companion_str}, storefront={storefront}, mode={dl_mode}, auto_start={}",
            s.default_song_codec.to_cli_string(),
            s.default_video_resolution.to_cli_string(),
            flag(s.auto_start_queue),
        ),
    );

    // Line 2: Feature flags
    emit_app_log(
        app,
        &format!(
            "Features: enhanced_lrc={}, advisory_suffixes={}, acoustid={}, replaygain={}, musicbrainz={}, animated_artwork={}",
            flag(s.enhanced_lrc),
            flag(s.content_advisory_in_filenames),
            flag(s.acoustid_enabled),
            flag(s.replaygain_enabled),
            flag(s.musicbrainz_lookup),
            flag(s.animated_artwork_enabled),
        ),
    );

    // Line 3: Authentication status (redacted — no credentials or paths)
    let cookies_status = if s.cookies_path.is_some() { "set" } else { "not set" };
    let musickit_status = if s.musickit_team_id.is_some() && s.musickit_key_id.is_some() {
        "configured"
    } else {
        "not configured"
    };
    emit_app_log(
        app,
        &format!(
            "Auth: wrapper={}, cookies={cookies_status}, musickit={musickit_status}",
            flag(s.use_wrapper),
        ),
    );

    // Line 4: Duplicate-detection configuration.
    //
    // Surfaced explicitly so bug reports can tell at a glance whether
    // dedup ran, what scope it consulted, and which key strategy was in
    // effect. Without this, diagnosing "why did my second album redownload
    // tracks I already had?" requires digging into settings.json by hand.
    let dedup_scope = serde_json::to_value(s.duplicate_detection.scope)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", s.duplicate_detection.scope));
    let dedup_key = serde_json::to_value(s.duplicate_detection.key_strategy)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{:?}", s.duplicate_detection.key_strategy));
    // The preference order is usually long; render it as a `>`-joined
    // CLI-style list so the activity log row stays one line.
    let dedup_pref = s
        .duplicate_detection
        .preference_order
        .iter()
        .map(|m| m.to_cli_string().to_string())
        .collect::<Vec<_>>()
        .join(">");
    emit_app_log(
        app,
        &format!("Dedup: scope={dedup_scope}, key={dedup_key}, preferences={dedup_pref}"),
    );
}

// ---------------------------------------------------------------------------
// Linux WebKitGTK rendering environment
// ---------------------------------------------------------------------------

/// Configures WebKitGTK environment variables on Linux to prevent rendering
/// corruption on Raspberry Pi and remote desktop environments (VNC,
/// Raspberry Pi Connect, etc.).
///
/// On Raspberry Pi specifically, GPU-accelerated compositing in WebKitGTK can
/// produce garbled/corrupted output — especially when viewed over a remote
/// desktop connection. Disabling the DMA-BUF renderer and GPU compositing
/// forces software rendering, which resolves the visual artifacts.
///
/// The detection is conservative: env vars are only set when running on a
/// Raspberry Pi (detected via `/proc/device-tree/model`). Users on capable
/// desktop Linux systems retain GPU-accelerated rendering. Users can also
/// override by setting the env vars themselves before launching the app (we
/// only set them if they are not already defined).
///
/// See: <https://github.com/MWBMPartners/MeedyaDL/issues/150>
#[cfg(target_os = "linux")]
fn setup_linux_rendering_env() {
    // Detect Raspberry Pi by reading the device-tree model string.
    // This file exists on all Raspberry Pi boards running Linux and contains
    // a human-readable model identifier (e.g., "Raspberry Pi 5 Model B").
    let is_raspberry_pi = std::fs::read_to_string("/proc/device-tree/model")
        .map(|model| model.to_lowercase().contains("raspberry"))
        .unwrap_or(false);

    if !is_raspberry_pi {
        return;
    }

    log::info!("Raspberry Pi detected — configuring WebKitGTK for software rendering");

    // WEBKIT_DISABLE_DMABUF_RENDERER: Disables the DMA-BUF renderer, which
    // can cause visual corruption over remote desktop connections and on
    // some ARM GPUs. Falls back to shared-memory rendering. Available in
    // WebKitGTK 2.42+.
    if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
        // SAFETY: Called before any threads are spawned (before tauri::Builder).
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        log::debug!("Set WEBKIT_DISABLE_DMABUF_RENDERER=1");
    }

    // WEBKIT_DISABLE_COMPOSITING_MODE: Disables GPU compositing entirely,
    // forcing CPU-based rendering. Increases CPU usage but eliminates all
    // GPU-related rendering artifacts on Raspberry Pi.
    if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
        // SAFETY: Called before any threads are spawned (before tauri::Builder).
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
        log::debug!("Set WEBKIT_DISABLE_COMPOSITING_MODE=1");
    }
}

/// No-op on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
fn setup_linux_rendering_env() {}

/// Configures and launches the Tauri application.
///
/// This function is the single entry point called from `main.rs`. It uses the
/// Tauri **Builder pattern** to declaratively compose the application from
/// plugins, commands, state, and lifecycle hooks. The builder is consumed by
/// `.run()` at the end, which starts the native event loop and never returns
/// under normal operation.
///
/// # Execution flow
/// 1. Install the panic handler and initialise `tracing` with dual-output
///    logging (stderr + rotating file in `{app_data_dir}/logs/`). If Sentry
///    is enabled in settings, the Sentry SDK is also initialised.
/// 2. Create a `tauri::Builder` and chain configuration calls:
///    - `.manage()` -- inject shared state accessible from any command handler.
///    - `.plugin()` -- register Tauri plugins that bridge native OS APIs.
///    - `.invoke_handler()` -- register `#[tauri::command]` functions for IPC.
///    - `.setup()` -- run one-time initialisation after the webview is ready.
/// 3. `.run(tauri::generate_context!())` starts the event loop. The macro
///    reads `tauri.conf.json` at **compile time** to embed window config,
///    bundle identifiers, and other metadata into the binary.
///
/// # Panics
/// Panics with a descriptive message if the Tauri event loop fails to start
/// (e.g., missing webview runtime, invalid configuration).
///
/// # Reference
/// - Builder pattern: <https://docs.rs/tauri/latest/tauri/struct.Builder.html>
/// - `generate_context!`: <https://docs.rs/tauri/latest/tauri/macro.generate_context.html>
/// - Plugin system: <https://v2.tauri.app/develop/plugins/>
/// - Calling Rust from JS: <https://v2.tauri.app/develop/calling-rust/>
// Allow large_stack_frames: `tauri::generate_context!()` allocates ~740KB on the
// stack at compile time. This is idiomatic Tauri code and cannot be avoided without
// boxing the entire context, which Tauri's API does not support.
#[allow(clippy::large_stack_frames, clippy::too_many_lines)]
pub fn run() {
    // Install the custom panic handler FIRST (before any other initialisation)
    // so that if anything panics during startup, we still get a crash report.
    setup_panic_handler();

    // Load settings early to check if Sentry is enabled. We need this before
    // initialising tracing because the Sentry layer must be part of the
    // subscriber stack from the start. If settings can't be loaded, default
    // to Sentry disabled (safe default -- no data sent without consent).
    let sentry_enabled = services::config_service::load_settings_from_default_path()
        .map(|s| s.sentry_enabled)
        .unwrap_or(false);

    // Initialise Sentry SDK if the user has opted in. The `_sentry_guard`
    // must be kept alive for the app's lifetime; dropping it flushes pending
    // events and shuts down the SDK. We use a compile-time DSN constant
    // (public DSNs are safe to embed per Sentry docs -- they only identify
    // the project, not authenticate requests).
    // Sentry DSN via compile-time env var. Without a real DSN, Sentry is a no-op.
    let sentry_dsn = option_env!("SENTRY_DSN").unwrap_or("");
    let _sentry_guard = if sentry_enabled && !sentry_dsn.is_empty() && !sentry_dsn.contains("examplePublicKey") {
        Some(sentry::init((
            sentry_dsn,
            sentry::ClientOptions {
                release: Some(std::borrow::Cow::Borrowed(env!("CARGO_PKG_VERSION"))),
                environment: Some(std::borrow::Cow::Borrowed(if cfg!(debug_assertions) {
                    "development"
                } else {
                    "production"
                })),
                sample_rate: 1.0,
                traces_sample_rate: 0.2,
                ..Default::default()
            },
        )))
    } else {
        None
    };

    // Initialise the tracing subscriber with dual-output logging (stderr + file).
    // The `_tracing_guard` must be kept alive to ensure the file appender
    // flushes on shutdown. Replaces the previous `env_logger::init()` call.
    // During development, run with `RUST_LOG=debug cargo tauri dev` to see
    // verbose output from all modules, or `RUST_LOG=meedyadl=debug` to
    // restrict output to this crate only.
    let _tracing_guard = setup_tracing(sentry_enabled);

    // On Linux, configure WebKitGTK environment variables before the WebView
    // is created. Fixes rendering corruption on Raspberry Pi and remote
    // desktop environments (VNC, Raspberry Pi Connect). See #150.
    setup_linux_rendering_env();

    // Build and run the Tauri application using the Builder pattern.
    // `Builder::default()` creates a new builder with sensible defaults.
    // Each chained method returns the builder, allowing fluent configuration.
    // Reference: https://docs.rs/tauri/latest/tauri/struct.Builder.html
    let builder = tauri::Builder::default()
        // ---------------------------------------------------------------
        // Managed State
        // ---------------------------------------------------------------
        // `.manage(T)` registers an instance of `T` as application-wide
        // state. Any `#[tauri::command]` handler can receive it by
        // declaring a parameter `State<'_, T>`. Tauri stores the value
        // behind an `Arc` internally, so it is shared safely across threads.
        //
        // Here we register the download queue handle -- an
        // `Arc<Mutex<DownloadQueue>>` -- so that download commands can
        // enqueue, cancel, and inspect downloads without global statics.
        //
        // Reference: https://docs.rs/tauri/latest/tauri/struct.Builder.html#method.manage
        // Reference: https://v2.tauri.app/develop/calling-rust/#accessing-managed-state
        .manage(services::download_queue::new_queue_handle())
        // Register the shutdown signal for graceful background task cleanup.
        // Fire-and-forget tasks (companions, lyrics, enrichment) poll this
        // flag between iterations and exit early when the app is closing.
        .manage(services::download_queue::ShutdownSignal::new())
        // In-process AppSettings cache (#690). Eliminates redundant
        // disk reads on the queue hot path — `load_settings_for_queue`
        // and friends now read from this cache instead of round-
        // tripping `config_service::load_settings()` per call. The
        // `save_settings` IPC refreshes the cache after each disk
        // write so the two stay in sync.
        .manage(services::settings_cache::SettingsCache::new())
        // ---------------------------------------------------------------
        // Plugin Registration
        // ---------------------------------------------------------------
        // Tauri 2.0 uses a plugin system where each native capability
        // (shell access, dialogs, filesystem, etc.) is provided by an
        // opt-in plugin. Plugins must be registered here on the Rust side
        // **and** listed in `tauri.conf.json` under `plugins.` / permissions.
        // The corresponding npm packages (`@tauri-apps/plugin-*`) expose
        // the TypeScript API to the React frontend.
        //
        // Reference: https://v2.tauri.app/develop/plugins/
        // Reference: https://v2.tauri.app/security/permissions/
        // Shell plugin: allows spawning external processes (Python, GAMDL CLI)
        // and opening URLs in the default browser. Used by `gamdl_service`
        // and `python_manager` to execute subprocess commands.
        // Reference: https://v2.tauri.app/plugin/shell/
        .plugin(tauri_plugin_shell::init())
        // Dialog plugin: native OS file/folder picker dialogs and message boxes.
        // Used in the frontend for selecting output directories and cookie files.
        // Reference: https://v2.tauri.app/plugin/dialog/
        .plugin(tauri_plugin_dialog::init())
        // Filesystem plugin: read/write files within permitted scope paths.
        // Tauri 2.0's security model requires explicit path scope grants in
        // `tauri.conf.json` -- the plugin alone does not grant blanket access.
        // Reference: https://v2.tauri.app/plugin/file-system/
        .plugin(tauri_plugin_fs::init())
        // Store plugin: persistent JSON key-value store backed by a file in
        // the app data directory. Used by `config_service` to persist user
        // settings between sessions. `Builder::default().build()` creates a
        // store with default options (auto-save on change).
        // Reference: https://v2.tauri.app/plugin/store/
        .plugin(tauri_plugin_store::Builder::default().build())
        // Process plugin: provides `process.exit()` and `process.relaunch()`
        // APIs so the frontend can cleanly shut down or restart the app.
        // Reference: https://v2.tauri.app/plugin/process/
        .plugin(tauri_plugin_process::init())
        // Updater plugin: cryptographically-verified application self-updates.
        // Downloads signed update binaries from GitHub Releases and applies them
        // in-place. The public key for signature verification is configured in
        // tauri.conf.json. Custom endpoints are set at runtime via UpdaterExt
        // to support both stable and pre-release channels.
        // Reference: https://v2.tauri.app/plugin/updater/
        .plugin(tauri_plugin_updater::Builder::new().build())
        // OS plugin: exposes `os.platform()`, `os.arch()`, `os.version()`, etc.
        // Used to determine which Python/tool binaries to download for the
        // current operating system and CPU architecture.
        // Reference: https://v2.tauri.app/plugin/os/
        .plugin(tauri_plugin_os::init())
        // Notification plugin: sends native OS desktop notifications for
        // download events (completion, failure) when the app window is not
        // focused. Controlled by the `desktop_notifications` setting.
        // Reference: https://v2.tauri.app/plugin/notification/
        .plugin(tauri_plugin_notification::init())
        // Deep Link plugin: registers the `meedyadl://` custom URL scheme so
        // external tools, bookmarklets, and browser extensions can trigger
        // downloads by opening URLs like:
        //   meedyadl://download?url=https://music.apple.com/gb/album/...
        // The plugin generates platform-specific scheme registrations:
        //   - macOS: CFBundleURLTypes in Info.plist (automatic)
        //   - Linux: x-scheme-handler/meedyadl in .desktop file (manual)
        //   - Windows: registry entries (automatic via plugin)
        // Reference: https://v2.tauri.app/plugin/deep-linking/
        .plugin(tauri_plugin_deep_link::init())
        // ---------------------------------------------------------------
        // IPC Command Registration
        // ---------------------------------------------------------------
        // `.invoke_handler()` registers all `#[tauri::command]` functions
        // that the React frontend can call via:
        //   `import { invoke } from '@tauri-apps/api/core';`
        //   `const result = await invoke('command_name', { args });`
        //
        // The `generate_handler!` macro creates a dispatch function that
        // maps the string command name sent over IPC to the corresponding
        // Rust function. Command functions may be sync or async, and can
        // accept `AppHandle`, `State<T>`, `Window`, and custom deserializable
        // parameters.
        //
        // Commands are grouped by module for clarity. The order here does
        // not affect dispatch performance (it's a match, not a linear scan).
        //
        // Reference: https://v2.tauri.app/develop/calling-rust/
        // Reference: https://docs.rs/tauri/latest/tauri/macro.generate_handler.html
        .invoke_handler(tauri::generate_handler![
            // System information and platform detection commands
            commands::system::get_platform_info,
            commands::system::get_app_data_dir,
            commands::system::get_platform_config,
            // Engine registry — full engine/platform config (#316)
            commands::system::get_engine_config,
            // Multi-service URL detection (#315)
            commands::system::detect_service,
            commands::system::save_session_log,
            // Notification diagnostics + backend-pipeline test button (#834).
            commands::system::test_desktop_notification,
            commands::system::get_notification_diagnostics,
            // Output-directory integrity scan (#537 chunk B).
            commands::system::run_integrity_scan,
            // Dependency management commands (Python, GAMDL, tools)
            commands::dependencies::check_python_status,
            commands::dependencies::install_python,
            commands::dependencies::check_gamdl_status,
            commands::dependencies::install_gamdl,
            commands::dependencies::install_gamdl_version,
            commands::dependencies::get_gamdl_support_window,
            commands::dependencies::get_gamdl_capabilities,
            commands::dependencies::check_votify_status,
            commands::dependencies::install_votify,
            commands::dependencies::check_ofscraper_status,
            commands::dependencies::install_ofscraper,
            commands::dependencies::check_all_dependencies,
            commands::dependencies::install_dependency,
            commands::dependencies::get_component_versions,
            // Settings management commands
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::has_embedded_acoustid_key,
            commands::settings::validate_cookies_file,
            commands::settings::check_cookies_before_download,
            commands::settings::check_internet_before_download,
            commands::settings::check_output_path_before_download,
            commands::settings::get_default_output_path,
            commands::settings::test_wrapper_connection,
            // Settings export/import commands
            commands::settings::export_settings,
            commands::settings::import_settings,
            // GAMDL download and queue management commands
            commands::gamdl::start_download,
            commands::gamdl::cancel_download,
            commands::gamdl::retry_download,
            commands::gamdl::retry_failed_bulk,
            commands::gamdl::retry_download_without_wrapper,
            commands::gamdl::move_queue_item_to_top,
            commands::gamdl::move_queue_item_to_bottom,
            commands::gamdl::move_queue_item_up,
            commands::gamdl::move_queue_item_down,
            commands::gamdl::clear_queue,
            commands::gamdl::clear_all_queue,
            commands::gamdl::delete_queue_item,
            commands::gamdl::abort_all_downloads,
            commands::gamdl::get_queue_status,
            commands::gamdl::check_gamdl_update,
            // Queue export/import commands
            commands::gamdl::export_queue,
            commands::gamdl::import_queue,
            // Manual queue processing trigger
            commands::gamdl::process_queue_manual,
            // Activity log export
            commands::gamdl::export_activity_log,
            // Persistent on-disk activity log commands (#541)
            commands::activity_log::export_disk_activity_log,
            commands::activity_log::get_logs_folder_path,
            // Embedded legal docs (ACKNOWLEDGEMENTS.md + THIRD_PARTY_LICENSES.md) — #802
            commands::legal::get_acknowledgements_text,
            commands::legal::get_third_party_licenses_text,
            // Snapshot + restore (#466)
            commands::backup::create_backup,
            commands::backup::list_backups,
            commands::backup::restore_from_backup,
            commands::backup::delete_backup,
            // Manifest import and folder scan
            commands::gamdl::import_manifest,
            commands::gamdl::scan_folder_for_manifests,
            commands::gamdl::scan_enrichment_gaps,
            // Legacy sibling-folder merge for pre-#528 downloads (#789)
            commands::gamdl::detect_legacy_folder_pairs,
            commands::gamdl::preview_legacy_folder_merge,
            commands::gamdl::execute_legacy_folder_merge,
            // Library Scan smart-retry diff per row (Phase 5b, #717)
            commands::gamdl::diff_library_scan_manifest,
            // Library Scan Apple Music lastModifiedDate freshness check (Phase 5c, #717)
            commands::gamdl::check_library_scan_freshness,
            // Smart re-download detection (#263)
            commands::gamdl::check_redownload_status,
            // Syllable-level lyrics fetch (#306)
            commands::gamdl::fetch_syllable_lyrics,
            // Credential storage commands
            commands::credentials::store_credential,
            commands::credentials::get_credential,
            commands::credentials::delete_credential,
            commands::credentials::validate_musickit_credentials,
            commands::credentials::has_embedded_musickit_token,
            commands::credentials::has_webplayer_token,
            commands::credentials::clear_webplayer_token,
            commands::credentials::check_dev_access,
            commands::credentials::activate_dev_access,
            commands::credentials::deactivate_dev_access,
            // Update checking and auto-update commands
            commands::updates::check_all_updates,
            commands::updates::upgrade_gamdl,
            commands::updates::upgrade_pip_engine,
            commands::updates::check_component_update,
            commands::updates::download_and_install_app_update,
            // Cookie management commands (browser detection, auto-import)
            commands::cookies::detect_browsers,
            commands::cookies::import_cookies_from_browser,
            commands::cookies::check_full_disk_access,
            // Embedded Apple Music login window commands
            commands::login_window::open_apple_login,
            commands::login_window::extract_login_cookies,
            commands::login_window::close_apple_login,
            // Animated artwork download command
            commands::artwork::download_animated_artwork,
            // Crash report commands (list, get, delete, export, log, GitHub reporting)
            commands::crash_reports::list_crash_reports,
            commands::crash_reports::get_crash_report,
            commands::crash_reports::delete_crash_report,
            commands::crash_reports::delete_all_crash_reports,
            commands::crash_reports::export_crash_report,
            commands::crash_reports::log_frontend_error,
            commands::crash_reports::get_github_issue_url,
            commands::crash_reports::build_diagnostic_bundle,
            // Download history commands (list, search, clear)
            commands::history::list_history,
            commands::history::clear_history,
            commands::history::search_history,
            commands::history::delete_history_entry,
            commands::history::get_lifetime_stats,
            // API field audit command (diagnostic tool)
            commands::api_audit::audit_api_fields,
            // Clipboard monitoring command
            commands::clipboard::read_clipboard,
            // Service status checking
            commands::service_status::check_service_status,
            // Smart Download cross-platform search
            commands::smart_download::check_cross_platform,
        ]);

    // ---------------------------------------------------------------
    // macOS Application Menu
    // ---------------------------------------------------------------
    // On macOS, override the default app menu so the "About MeedyaDL"
    // item navigates to the in-app Help > About page instead of
    // showing the generic macOS About dialog. Other standard menu
    // items (Edit, Window, etc.) are preserved.
    //
    // On Linux and Windows, no application menu is added — the app
    // uses a custom titlebar/sidebar and the in-window menu bar is
    // unwanted.
    //
    // Reference: https://docs.rs/tauri/latest/tauri/menu/index.html
    #[cfg(target_os = "macos")]
    let builder = builder
        .menu(|app| {
            use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};

            let app_submenu = SubmenuBuilder::new(app, "MeedyaDL")
                .item(&MenuItemBuilder::with_id("about_meedyadl", "About MeedyaDL").build(app)?)
                .separator()
                .item(&PredefinedMenuItem::services(app, None)?)
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;

            let edit_submenu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            let window_submenu = SubmenuBuilder::new(app, "Window")
                .minimize()
                .item(&PredefinedMenuItem::maximize(app, None)?)
                .separator()
                .close_window()
                .build()?;

            MenuBuilder::new(app)
                .item(&app_submenu)
                .item(&edit_submenu)
                .item(&window_submenu)
                .build()
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "about_meedyadl" {
                use tauri::Emitter;
                let _ = app.emit("navigate-help-about", ());
            }
        });

    builder
        // ---------------------------------------------------------------
        // Graceful Shutdown -- window event handler
        // ---------------------------------------------------------------
        // When the main window is destroyed (user closes it or clicks tray
        // "Quit"), trigger the shutdown signal so fire-and-forget background
        // tasks (companion downloads, lyrics companions, enrichment pipeline)
        // stop starting new work at their next check point. This prevents
        // orphaned subprocess spawns and unnecessary I/O during app exit.
        //
        // Reference: https://docs.rs/tauri/latest/tauri/struct.Builder.html#method.on_window_event
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                use tauri::Manager;
                let app = window.app_handle();
                let shutdown = app.state::<services::download_queue::ShutdownSignal>();
                shutdown.trigger();
                log::info!("Window destroyed — shutdown signal sent to background tasks");

                // Best-effort auto-backup on exit (#466). We do this
                // synchronously so the snapshot completes before the
                // process tears down. Errors are logged but do not
                // block shutdown — a missing snapshot is preferable to
                // a hung exit. Typical snapshot is ~10-500 KB and
                // copies in well under 100ms.
                match services::backup_service::create_backup(&app.clone()) {
                    Ok(summary) => {
                        log::info!(
                            "Exit auto-backup wrote {} file(s) to {} ({} total snapshots)",
                            summary.files.len(),
                            summary.snapshot_path,
                            summary.total_snapshots,
                        );
                    }
                    Err(e) => {
                        // Don't log as warn if it's just "nothing to back up"
                        // (fresh install with no state files yet).
                        log::debug!("Exit auto-backup skipped: {e}");
                    }
                }
            }
        })
        // ---------------------------------------------------------------
        // Application Lifecycle -- `.setup()` hook
        // ---------------------------------------------------------------
        // The `.setup()` closure runs **once** after the Tauri runtime and
        // webview are initialised but before the event loop starts processing
        // user input. This is the place for one-time initialisation that needs
        // access to the `App` handle (and therefore to managed state, windows,
        // and the filesystem).
        //
        // The closure receives `&mut App` and must return `Ok(())` to signal
        // that startup succeeded. Returning `Err(...)` would abort the app.
        //
        // Reference: https://docs.rs/tauri/latest/tauri/struct.Builder.html#method.setup
        // Reference: https://v2.tauri.app/develop/#setup
        .setup(|app| {
            // Log application startup information
            log::info!(
                "MeedyaDL v{} starting on {} ({})",
                app.package_info().version,
                std::env::consts::OS,
                std::env::consts::ARCH,
            );

            // Open WebView DevTools in debug builds or when devtools feature is enabled.
            // This allows inspecting the DOM, Console, and Network tabs to diagnose
            // rendering issues. In release builds, devtools are available via the
            // "devtools" Cargo feature flag but not opened automatically.
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            // Ensure the application data directory exists
            // This is where Python, GAMDL, tools, and settings are stored
            let app_data_dir = utils::platform::get_app_data_dir(app.handle());
            if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
                log::error!("Failed to create app data directory: {e}");
            } else {
                log::info!("App data directory: {}", app_data_dir.display());
            }

            // Set up the system tray icon with context menu and event handlers.
            // The `_tray` binding keeps the TrayIcon alive for the app's lifetime;
            // dropping it would remove the icon from the system tray.
            let _tray = setup_system_tray(app)?;

            // Clean up crash reports older than 30 days
            services::crash_report_service::clear_old_reports(app.handle());

            // Resolve the on-disk activity log directory, honouring the
            // user's `activity_log_path_override` setting when set.
            // Falls back to `{app_data_dir}/logs/` on error or when empty.
            let activity_log_dir = resolve_activity_log_dir(app.handle(), &app_data_dir);

            // Clean up log files older than their retention period. The
            // override dir (if set and different from default) is also
            // scanned so `activity-*.log` files there get pruned too.
            let extra_dir = if activity_log_dir != app_data_dir.join("logs") {
                Some(activity_log_dir.as_path())
            } else {
                None
            };
            clear_old_logs(&app_data_dir, extra_dir);

            // Start the persistent on-disk activity log writer (#541).
            // Runs as a background Tokio task consuming events from an
            // unbounded channel. The shared `ShutdownSignal` trips the
            // writer into its flush-and-exit path on window close / tray
            // quit so the final events are not lost.
            use tauri::Manager;
            let shutdown = app
                .state::<services::download_queue::ShutdownSignal>()
                .flag();
            let writer_handle =
                services::activity_log_writer::start(activity_log_dir.clone(), shutdown);
            utils::activity_log::register_disk_writer(writer_handle);
            log::info!(
                "Activity log writer started at {} (retention {}d)",
                activity_log_dir.display(),
                services::activity_log_writer::ACTIVITY_LOG_RETENTION_DAYS
            );

            // Compact any duplicate history rows left by older retry behaviour.
            services::history_service::cleanup_duplicate_entries(app.handle());

            // Restore any persisted queue items and schedule delayed processing
            setup_queue_recovery(app);

            // Spawn the external queue watchdog (#818). Runs on its own
            // top-level tokio task so it can't be killed by a hang in
            // the queue processor's task tree — the failure mode that
            // caused the v1.6.0 silent-stuck-download in #815. Polls
            // every 60s; escalates items with bit-identical progress
            // signal for >10min (WARN) or >20min (Error + release slot).
            {
                use tauri::Manager;
                let queue: tauri::State<'_, services::download_queue::QueueHandle> =
                    app.state();
                let shutdown: tauri::State<'_, services::download_queue::ShutdownSignal> =
                    app.state();
                services::queue_watchdog::spawn_queue_watchdog(
                    app.handle().clone(),
                    queue.inner().clone(),
                    std::sync::Arc::new(shutdown.inner().clone()),
                );
            }

            // Register the deep link URL handler for the `meedyadl://` scheme.
            // When an external tool opens a URL like:
            //   meedyadl://download?url=https://music.apple.com/gb/album/...&codec=alac
            // this handler parses the URL parameters and emits a `deep-link-download`
            // event to the frontend, which navigates to the Download page and pre-fills
            // the URL input. Also brings the main window to the foreground.
            //
            // The handler also checks for any URL that was used to launch the app
            // (e.g., when the app was not running and was started via a deep link).
            setup_deep_link_handler(app);

            // Emit a startup activity log event after a short delay so the
            // frontend event listeners are registered before we send it.
            let startup_handle = app.handle().clone();
            let version = app.package_info().version.to_string();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                utils::activity_log::emit_app_log(
                    &startup_handle,
                    &format!(
                        "MeedyaDL v{version} started on {} ({})",
                        std::env::consts::OS,
                        std::env::consts::ARCH,
                    ),
                );

                // Log concise settings summary at startup for diagnostics
                if let Ok(s) = services::config_service::load_settings(&startup_handle) {
                    emit_startup_settings_summary(&startup_handle, &s);
                }

                // Log component versions to the Activity Log for diagnostics.
                // Runs after a short delay so dependencies have been detected.
                commands::dependencies::log_component_versions_to_activity(&startup_handle).await;
            });

            Ok(())
        })
        // ---------------------------------------------------------------
        // Start the Tauri event loop
        // ---------------------------------------------------------------
        // `.run()` consumes the builder and enters the platform's native
        // event loop (NSApplication on macOS, Win32 message loop on Windows,
        // GTK main loop on Linux). This call **blocks** until the application
        // exits (via `app.exit()`, window close, or OS termination).
        //
        // `tauri::generate_context!()` is a compile-time macro that reads
        // `tauri.conf.json` and embeds configuration (window settings,
        // bundle identifier, icons, permissions) into the binary.
        //
        // Reference: https://docs.rs/tauri/latest/tauri/struct.Builder.html#method.run
        // Reference: https://docs.rs/tauri/latest/tauri/macro.generate_context.html
        .run(tauri::generate_context!())
        .expect("Failed to start MeedyaDL application");
}
