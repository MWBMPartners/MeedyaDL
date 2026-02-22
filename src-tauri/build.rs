// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Tauri build script. This is executed by Cargo before compiling the main
// application. It runs Tauri's code generation to produce the necessary
// bindings between the Rust backend and the web frontend.

fn main() {
    // Ensure bundled-deps.tar.gz exists for Tauri's resource requirement.
    // CI builds create the real archive via download-bundled-deps.sh; for dev
    // builds we create a minimal placeholder so cargo check/build succeeds.
    let archive_path = std::path::Path::new("bundled-deps.tar.gz");
    if !archive_path.exists() {
        // Create a minimal valid gzip file (empty gzip stream)
        let empty_gz: &[u8] = &[
            0x1f, 0x8b, 0x08, 0x00, // magic + method + flags
            0x00, 0x00, 0x00, 0x00, // mtime
            0x00, 0x03, // xfl + os
            0x03, 0x00, // compressed empty payload
            0x00, 0x00, 0x00, 0x00, // crc32
            0x00, 0x00, 0x00, 0x00, // size
        ];
        std::fs::write(archive_path, empty_gz).ok();
    }

    // Run Tauri's build-time code generation
    // This generates the context used by tauri::generate_context!() in main.rs
    tauri_build::build();
}
