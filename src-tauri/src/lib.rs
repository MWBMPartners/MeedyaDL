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
/// Each sub-module groups related commands (system, dependencies, settings,
/// gamdl, credentials, updates). Commands are thin wrappers that validate
/// inputs and delegate to the `services` layer.
pub mod commands;

/// Shared data models used across commands, services, and IPC payloads.
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


/// Configures and launches the Tauri application.
///
/// This function is the single entry point called from `main.rs`. It uses the
/// Tauri **Builder pattern** to declaratively compose the application from
/// plugins, commands, state, and lifecycle hooks. The builder is consumed by
/// `.run()` at the end, which starts the native event loop and never returns
/// under normal operation.
///
/// # Execution flow
/// 1. Initialise the `env_logger` crate so `log::info!` / `log::debug!` etc.
///    print to stderr (controlled by the `RUST_LOG` environment variable).
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
pub fn run() {
    // Initialise the `env_logger` crate for structured logging.
    // During development, run with `RUST_LOG=debug cargo tauri dev` to see
    // verbose output from all modules, or `RUST_LOG=meedyadl=debug` to
    // restrict output to this crate only.
    // Reference: https://docs.rs/env_logger/latest/env_logger/
    env_logger::init();

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
            // GAMDL download and queue management commands
            commands::gamdl::start_download,
            commands::gamdl::cancel_download,
            commands::gamdl::retry_download,
            commands::gamdl::clear_queue,
            commands::gamdl::get_queue_status,
            commands::gamdl::check_gamdl_update,
            // Queue export/import commands
            commands::gamdl::export_queue,
            commands::gamdl::import_queue,
            // Credential storage commands
            commands::credentials::store_credential,
            commands::credentials::get_credential,
            commands::credentials::delete_credential,
            // Update checking commands
            commands::updates::check_all_updates,
            commands::updates::upgrade_gamdl,
            commands::updates::check_component_update,
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
        ])

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
                log::error!("Failed to create app data directory: {}", e);
            } else {
                log::info!("App data directory: {}", app_data_dir.display());
            }

            // -------------------------------------------------------
            // System Tray Setup
            // -------------------------------------------------------
            // The system tray icon allows the user to interact with the
            // application even when the main window is hidden or minimised.
            // Tauri 2.0's tray API is builder-based: we construct menu
            // items, compose them into a menu, attach the menu to a
            // TrayIconBuilder, and register event handlers for clicks.
            //
            // We import tray/menu types here (inside `.setup()`) rather
            // than at file scope to keep the top-level namespace clean --
            // these types are only needed during one-time initialisation.
            //
            // Reference: https://v2.tauri.app/develop/system-tray/
            // Reference: https://docs.rs/tauri/latest/tauri/tray/index.html
            // Reference: https://docs.rs/tauri/latest/tauri/menu/index.html
            use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState};
            use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
            // `Manager` trait provides `.get_webview_window()` on AppHandle
            use tauri::Manager;
            // `Emitter` trait provides `.emit()` for sending events to the frontend
            use tauri::Emitter;

            // Build the tray menu items
            // "Show Window" — brings the main window to focus
            let show_item = MenuItemBuilder::with_id("show", "Show Window")
                .build(app)?;

            // First separator — visually groups window controls from status info
            let separator1 = PredefinedMenuItem::separator(app)?;

            // "Downloads: None" — disabled info item that displays current download status.
            // The frontend can update this text via the tray menu API as downloads progress.
            let downloads_item = MenuItemBuilder::with_id("downloads_status", "Downloads: None")
                .enabled(false)
                .build(app)?;

            // Second separator — visually groups status info from application actions
            let separator2 = PredefinedMenuItem::separator(app)?;

            // "Check for Updates" — triggers an update check for the application and GAMDL
            let updates_item = MenuItemBuilder::with_id("check_updates", "Check for Updates")
                .build(app)?;

            // "Quit GAMDL" — cleanly exits the application
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
            // IMPORTANT: The `_tray` binding uses a leading underscore to
            // suppress the "unused variable" warning, but the variable is
            // NOT dropped -- it remains alive for the lifetime of the
            // `.setup()` closure's scope (i.e., the entire app lifetime).
            // If we used `let _ = ...` (without a name), the TrayIcon would
            // be dropped immediately and disappear from the system tray.
            //
            // Reference: https://docs.rs/tauri/latest/tauri/tray/struct.TrayIconBuilder.html
            let _tray = TrayIconBuilder::new()
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

            // -------------------------------------------------------
            // Queue Persistence: Restore on Startup
            // -------------------------------------------------------
            // Load any persisted queue items from `queue.json` (written
            // after every queue mutation in the previous session). If items
            // exist, restore them to the queue in Queued state and start
            // processing after a short delay (2 seconds) to give the
            // frontend event listeners time to initialise.
            //
            // This provides crash recovery: if the app closes (or crashes)
            // while downloads are queued/active, those items are restored
            // and automatically resumed on next launch.
            {
                let app_handle = app.handle().clone();
                let persisted_items = services::download_queue::load_queue_from_disk(&app_handle);
                if !persisted_items.is_empty() {
                    let count = persisted_items.len();
                    let settings = services::config_service::load_settings(&app_handle)
                        .unwrap_or_default();

                    // Get the queue handle from managed state
                    use tauri::Manager;
                    let queue_handle: tauri::State<'_, services::download_queue::QueueHandle> =
                        app.state();
                    let queue_arc = queue_handle.inner().clone();

                    // Restore items synchronously (we can block briefly in setup)
                    {
                        let rt = tokio::runtime::Handle::current();
                        rt.block_on(async {
                            let mut q = queue_arc.lock().await;
                            q.restore_items(persisted_items, &settings);
                        });
                    }

                    log::info!(
                        "Queue restored: {} item(s) will resume after frontend initialises",
                        count
                    );

                    // Spawn a delayed task to start processing the restored queue.
                    // The 2-second delay ensures the frontend's Tauri event listeners
                    // are registered before downloads start emitting events.
                    let queue_for_processing = queue_arc;
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        services::download_queue::process_queue(
                            app_handle,
                            queue_for_processing,
                        )
                        .await;
                    });
                }
            }

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
