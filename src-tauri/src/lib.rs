// Copyright (c) 2024-2026 MeedyaDL
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

/// Initialises the `tracing` subscriber with dual-output logging:
///
/// 1. **stderr** -- Coloured, human-readable output for development. Controlled
///    by the `RUST_LOG` environment variable (e.g., `RUST_LOG=debug`).
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
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("meedyadl=info,warn"));

    // Build the layered subscriber
    let registry = tracing_subscriber::registry()
        .with(env_filter)
        // stderr layer: coloured, with timestamps, for dev console
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(false),
        )
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
        registry
            .with(sentry_tracing::layer())
            .init();
    } else {
        registry.init();
    }

    guard
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
        let location = panic_info.location().map(|loc| {
            format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
        });

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
            let filename = format!(
                "crash-{}.json",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            );
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
fn setup_system_tray(app: &tauri::App) -> Result<tauri::tray::TrayIcon, Box<dyn std::error::Error>> {
    use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState};
    use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
    // `Manager` trait provides `.get_webview_window()` on AppHandle
    use tauri::Manager;
    // `Emitter` trait provides `.emit()` for sending events to the frontend
    use tauri::Emitter;

    // Build the tray menu items
    // "Show Window" -- brings the main window to focus
    let show_item = MenuItemBuilder::with_id("show", "Show Window")
        .build(app)?;

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
    let updates_item = MenuItemBuilder::with_id("check_updates", "Check for Updates")
        .build(app)?;

    // "Quit MeedyaDL" -- cleanly exits the application
    let quit_item = MenuItemBuilder::with_id("quit", "Quit MeedyaDL")
        .build(app)?;

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
                // Cleanly exit the application
                "quit" => {
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
    let settings = services::config_service::load_settings(&app_handle)
        .unwrap_or_default();

    // Get the queue handle from managed state
    let queue_handle: tauri::State<'_, services::download_queue::QueueHandle> =
        app.state();
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

    log::info!(
        "Queue restored: {count} item(s) will resume after frontend initialises"
    );

    // Spawn a delayed task to start processing the restored queue.
    // The 2-second delay ensures the frontend's Tauri event listeners
    // are registered before downloads start emitting events.
    // Use `tauri::async_runtime::spawn` instead of `tokio::spawn` because
    // the Tokio runtime may not be set as "current" during the `setup` closure.
    let queue_for_processing = queue_arc;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        services::download_queue::process_queue(
            app_handle,
            queue_for_processing,
        )
        .await;
    });
}


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
    let _sentry_guard = if sentry_enabled {
        Some(sentry::init((
            "https://examplePublicKey@o0.ingest.sentry.io/0",
            sentry::ClientOptions {
                release: Some(std::borrow::Cow::Borrowed(env!("CARGO_PKG_VERSION"))),
                environment: Some(std::borrow::Cow::Borrowed(if cfg!(debug_assertions) {
                    "development"
                } else {
                    "production"
                })),
                // Capture 100% of errors, 20% of transactions (performance)
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

    // Build and run the Tauri application using the Builder pattern.
    // `Builder::default()` creates a new builder with sensible defaults.
    // Each chained method returns the builder, allowing fluent configuration.
    // Reference: https://docs.rs/tauri/latest/tauri/struct.Builder.html
    tauri::Builder::default()
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
            // Dependency management commands (Python, GAMDL, tools)
            commands::dependencies::check_python_status,
            commands::dependencies::install_python,
            commands::dependencies::check_gamdl_status,
            commands::dependencies::install_gamdl,
            commands::dependencies::check_all_dependencies,
            commands::dependencies::install_dependency,
            // Settings management commands
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::validate_cookies_file,
            commands::settings::get_default_output_path,
            commands::settings::test_wrapper_connection,
            // GAMDL download and queue management commands
            commands::gamdl::start_download,
            commands::gamdl::cancel_download,
            commands::gamdl::retry_download,
            commands::gamdl::retry_download_without_wrapper,
            commands::gamdl::clear_queue,
            commands::gamdl::get_queue_status,
            commands::gamdl::check_gamdl_update,
            // Queue export/import commands
            commands::gamdl::export_queue,
            commands::gamdl::import_queue,
            // Manual queue processing trigger
            commands::gamdl::process_queue_manual,
            // Activity log export
            commands::gamdl::export_activity_log,
            // Credential storage commands
            commands::credentials::store_credential,
            commands::credentials::get_credential,
            commands::credentials::delete_credential,
            // Update checking and auto-update commands
            commands::updates::check_all_updates,
            commands::updates::upgrade_gamdl,
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
            // Crash report commands (list, get, delete, export, log frontend errors)
            commands::crash_reports::list_crash_reports,
            commands::crash_reports::get_crash_report,
            commands::crash_reports::delete_crash_report,
            commands::crash_reports::export_crash_report,
            commands::crash_reports::log_frontend_error,
        ])

        // ---------------------------------------------------------------
        // macOS Application Menu
        // ---------------------------------------------------------------
        // On macOS, override the default app menu so the "About MeedyaDL"
        // item navigates to the in-app Help > About page instead of
        // showing the generic macOS About dialog. Other standard menu
        // items (Edit, Window, etc.) are preserved.
        //
        // Reference: https://docs.rs/tauri/latest/tauri/menu/index.html
        .menu(|app| {
            use tauri::menu::{
                MenuBuilder, SubmenuBuilder, MenuItemBuilder, PredefinedMenuItem,
            };

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

            // Restore any persisted queue items and schedule delayed processing
            setup_queue_recovery(app);

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
