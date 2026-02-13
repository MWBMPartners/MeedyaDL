// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Main entry point for the MeedyaDL desktop application.
// =========================================================
//
// This file is intentionally minimal. In Tauri 2.0, the recommended pattern
// is to keep main.rs as a thin launcher and place all application logic in
// lib.rs. This separation allows the same library code to be used by both
// the desktop binary (main.rs) and by unit/integration tests.
//
// The only responsibilities of main.rs are:
//   1. Suppress the Windows console window in release builds (via the
//      `windows_subsystem` attribute below).
//   2. Call the library's `run()` function to bootstrap the Tauri application.
//
// Reference: https://v2.tauri.app/develop/
// Reference: https://v2.tauri.app/start/create-project/#tauri-entry-point

// ---------------------------------------------------------------------------
// `#![cfg_attr(...)]` is a conditional compilation attribute that applies only
// when the inner condition is true.
//
// `not(debug_assertions)` evaluates to true in release builds (i.e., when
// `cargo build --release` is used). In debug builds, the console stays visible
// so that `println!` and `env_logger` output can be read during development.
//
// `windows_subsystem = "windows"` tells the Windows linker to use the GUI
// subsystem entry point (WinMain) instead of the console subsystem. This
// prevents an empty terminal window from appearing behind the GUI.
// On macOS and Linux this attribute is silently ignored.
//
// Reference: https://doc.rust-lang.org/reference/runtime.html#the-windows_subsystem-attribute
// Reference: https://doc.rust-lang.org/reference/conditional-compilation.html#the-cfg_attr-attribute
// ---------------------------------------------------------------------------
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// The application entry point.
///
/// All application setup -- plugin registration, command handler registration,
/// tray icon construction, managed state injection, and the Tauri event loop --
/// is delegated to [`meedyadl::run()`] in `lib.rs`. Keeping `main()` trivial
/// follows the Tauri 2.0 convention and ensures the library crate remains the
/// single source of truth for application configuration.
fn main() {
    // Delegate to the library's run() function which sets up the Tauri
    // application with all plugins, commands, and event handlers.
    // If run() returns (which only happens on fatal startup errors),
    // the process exits with the default exit code.
    meedyadl::run();
}
