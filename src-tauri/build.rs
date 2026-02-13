// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Tauri build script. This is executed by Cargo before compiling the main
// application. It runs Tauri's code generation to produce the necessary
// bindings between the Rust backend and the web frontend.

fn main() {
    // Run Tauri's build-time code generation
    // This generates the context used by tauri::generate_context!() in main.rs
    tauri_build::build();
}
