// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Core library for the gamdl-GUI Tauri application.
// This module configures the Tauri builder with all plugins, IPC command
// handlers, managed state, and application lifecycle hooks. It serves as
// the central wiring point that connects the Rust backend to the React frontend.

// Declare sub-modules that organize the backend code
pub mod commands;
pub mod models;
pub mod services;
pub mod utils;


/// Configures and launches the Tauri application.
///
/// This function is the single entry point called from main.rs. It:
/// 1. Registers all Tauri plugins (shell, dialog, filesystem, store, process, OS)
/// 2. Registers all IPC command handlers that the React frontend can invoke
/// 3. Sets up the application lifecycle (setup hook for initialization)
/// 4. Starts the Tauri event loop which runs until the application exits
pub fn run() {
    // Initialize the environment logger for debug output
    // Set RUST_LOG=debug to see detailed backend logs during development
    env_logger::init();

    // Build and run the Tauri application
    tauri::Builder::default()
        // --- Managed State ---
        // Register the download queue as Tauri managed state so it can be
        // injected into command handlers via State<'_, QueueHandle>.
        .manage(services::download_queue::new_queue_handle())

        // --- Plugin Registration ---
        // Each plugin extends Tauri with additional capabilities that can be
        // used from both the Rust backend and the TypeScript frontend.

        // Shell plugin: Execute external commands (Python, GAMDL)
        .plugin(tauri_plugin_shell::init())
        // Dialog plugin: Native file/folder picker and message dialogs
        .plugin(tauri_plugin_dialog::init())
        // Filesystem plugin: Read/write files from permitted directories
        .plugin(tauri_plugin_fs::init())
        // Store plugin: Persistent key-value storage for app settings
        .plugin(tauri_plugin_store::Builder::default().build())
        // Process plugin: Exit and restart the application
        .plugin(tauri_plugin_process::init())
        // OS plugin: Detect platform (macOS/Windows/Linux) and architecture
        .plugin(tauri_plugin_os::init())

        // --- IPC Command Registration ---
        // Register all Tauri commands that can be called from the frontend
        // via the invoke() API. Commands are organized by module.
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
            // Credential storage commands
            commands::credentials::store_credential,
            commands::credentials::get_credential,
            commands::credentials::delete_credential,
            // Update checking commands
            commands::updates::check_all_updates,
            commands::updates::upgrade_gamdl,
            commands::updates::check_component_update,
        ])

        // --- Application Lifecycle ---
        .setup(|app| {
            // Log application startup information
            log::info!(
                "gamdl-GUI v{} starting on {} ({})",
                app.package_info().version,
                std::env::consts::OS,
                std::env::consts::ARCH,
            );

            // Ensure the application data directory exists
            // This is where Python, GAMDL, tools, and settings are stored
            let app_data_dir = utils::platform::get_app_data_dir(app.handle());
            if let Err(e) = std::fs::create_dir_all(&app_data_dir) {
                log::error!("Failed to create app data directory: {}", e);
            } else {
                log::info!("App data directory: {}", app_data_dir.display());
            }

            // --- System Tray Setup ---
            // Import tray and menu types at point of use to keep top-level imports minimal
            use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState};
            use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
            use tauri::Manager;
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
            let quit_item = MenuItemBuilder::with_id("quit", "Quit GAMDL")
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
            // The leading underscore keeps the binding alive without triggering an
            // unused-variable warning — dropping the TrayIcon would remove it from
            // the system tray.
            let _tray = TrayIconBuilder::new()
                .menu(&tray_menu)
                // Handle clicks on tray menu items
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
                // Handle direct clicks on the tray icon itself (not the menu).
                // A left-click toggles window visibility: shows and focuses the
                // window if it is hidden, or brings it to focus if already visible.
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
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

            Ok(())
        })
        // Start the Tauri event loop - this blocks until the application exits
        .run(tauri::generate_context!())
        .expect("Failed to start gamdl-GUI application");
}
