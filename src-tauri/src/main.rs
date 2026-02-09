// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Main entry point for the gamdl-GUI desktop application.
// This file configures the Tauri application to hide the console window
// on Windows release builds (users see only the GUI, not a terminal).
// All application setup logic is delegated to lib.rs.

// Prevents an additional console window from appearing on Windows in release mode.
// This attribute has no effect on other platforms or in debug builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Delegate to the library's run() function which sets up the Tauri
    // application with all plugins, commands, and event handlers.
    gamdl_gui::run();
}
