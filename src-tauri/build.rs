// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Tauri build script. This is executed by Cargo before compiling the main
// application. It runs Tauri's code generation to produce the necessary
// bindings between the Rust backend and the web frontend.

fn main() {
    // On Windows (MSVC), put the application manifest into every program
    // this package links -- the app AND the `cargo test` programs -- instead
    // of letting Tauri put it into the app only.
    //
    // Why: the manifest asks Windows for Common Controls version 6. Tauri's
    // menu and dialog code calls functions that only exist in version 6.
    // tauri-build embeds its manifest through a resource that is linked into
    // binaries only (`cargo:rustc-link-arg-bins`), so the library's unit-test
    // program had no manifest. As soon as a test reached code that keeps a
    // copy of the app handle (and so, through it, Tauri's menu code), Windows
    // refused to start the whole test program with STATUS_ENTRYPOINT_NOT_FOUND
    // (0xc0000139) before a single test ran. Giving every program the same
    // manifest means no future test can trip this again. This is the approach
    // Tauri's own documentation and examples use.
    //
    // Other platforms and Windows GNU builds are unchanged.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if target_os == "windows" && target_env == "msvc" {
        embed_windows_manifest_everywhere();
        let attributes = tauri_build::Attributes::new().windows_attributes(
            // Tauri must not add its own copy as well: two manifests in one
            // program is an error.
            tauri_build::WindowsAttributes::new_without_app_manifest(),
        );
        tauri_build::try_build(attributes).expect("failed to run tauri-build");
    } else {
        // Run Tauri's build-time code generation
        // This generates the context used by tauri::generate_context!() in main.rs
        tauri_build::build();
    }
}

/// Tells the MSVC linker to embed `windows-app-manifest.xml` into every
/// program this package links (app, tests, examples, benches).
/// `cargo:rustc-link-arg` (no `-bins` suffix) is what makes it reach the
/// test programs.
fn embed_windows_manifest_everywhere() {
    let manifest = std::path::Path::new(
        &std::env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"),
    )
    .join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
