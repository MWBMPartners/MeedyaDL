// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Perl runtime manager service.
// Manages the bundled portable Perl runtime used by get_iplayer for
// BBC iPlayer downloads. Unlike Python which is downloaded at runtime,
// Perl is bundled into the installer by CI (via download-bundled-deps.sh)
// and extracted at first launch by bundled_deps_service.rs.
//
// ## Architecture Overview
//
// The Perl runtime comes from skaji/relocatable-perl, which provides
// portable, self-contained Perl distributions for macOS and Linux.
// Windows support uses Strawberry Perl Portable (not yet implemented).
//
// The Perl binary is used by bbc_iplayer_service.rs to run get_iplayer
// as a subprocess: `perl /path/to/get_iplayer <args>`
//
// ## Flow Summary
//
// 1. `check_perl_status()` - Check if bundled Perl exists and runs
// 2. `get_perl_binary_path()` - Resolve the Perl binary location
// 3. `get_get_iplayer_path()` - Resolve the get_iplayer script location
//
// ## References
//
// - relocatable-perl: https://github.com/skaji/relocatable-perl
// - get_iplayer: https://github.com/get-iplayer/get_iplayer
// - Strawberry Perl Portable: https://strawberryperl.com/

use std::path::PathBuf;
use tauri::AppHandle;

use crate::utils::platform;

/// Returns the path to the bundled Perl directory.
///
/// The Perl runtime is extracted to `{app_data}/perl/` at first launch
/// by `bundled_deps_service::extract_bundled_deps()`.
pub fn get_perl_dir(app: &AppHandle) -> PathBuf {
    platform::get_app_data_dir(app).join("perl")
}

/// Returns the path to the bundled Perl binary.
///
/// Platform-specific:
///   - macOS/Linux: `{app_data}/perl/bin/perl`
///   - Windows: `{app_data}/perl/perl/bin/perl.exe` (Strawberry Perl layout)
pub fn get_perl_binary_path(app: &AppHandle) -> PathBuf {
    let perl_dir = get_perl_dir(app);

    if cfg!(target_os = "windows") {
        // Strawberry Perl Portable layout (not yet implemented)
        perl_dir.join("perl").join("bin").join("perl.exe")
    } else {
        // skaji/relocatable-perl layout
        perl_dir.join("bin").join("perl")
    }
}

/// Returns the path to the get_iplayer script.
///
/// The script is stored in the tools directory alongside other binaries,
/// but it's a Perl script (not a standalone binary) and must be invoked
/// via the bundled Perl: `perl /path/to/get_iplayer <args>`
pub fn get_get_iplayer_path(app: &AppHandle) -> PathBuf {
    platform::get_tools_dir(app)
        .join("get_iplayer")
        .join("get_iplayer")
}

/// Checks whether the bundled Perl runtime is available and functional.
///
/// Returns `Ok(version_string)` if Perl is installed and runs successfully,
/// or `Err(message)` if Perl is not found or not working.
pub async fn check_perl_status(app: &AppHandle) -> Result<String, String> {
    let perl_bin = get_perl_binary_path(app);

    if !perl_bin.exists() {
        return Err("Bundled Perl not found. It will be extracted from the installer on first launch.".to_string());
    }

    // Run `perl --version` to verify the binary works
    let output = tokio::process::Command::new(&perl_bin)
        .arg("--version")
        .output()
        .await
        .map_err(|e| format!("Failed to run bundled Perl: {}", e))?;

    if !output.status.success() {
        return Err("Bundled Perl binary exists but failed to run".to_string());
    }

    // Extract version from output (first line typically contains "This is perl 5, version XX")
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout
        .lines()
        .find(|line| line.contains("perl"))
        .unwrap_or("perl (version unknown)")
        .trim()
        .to_string();

    Ok(version)
}

/// Checks whether get_iplayer is available (script exists and Perl can run it).
///
/// Returns `Ok(version_string)` if get_iplayer is installed and runs,
/// or `Err(message)` if not found or not working.
pub async fn check_get_iplayer_status(app: &AppHandle) -> Result<String, String> {
    let perl_bin = get_perl_binary_path(app);
    let script_path = get_get_iplayer_path(app);

    if !perl_bin.exists() {
        return Err("Bundled Perl not found — get_iplayer requires Perl".to_string());
    }

    if !script_path.exists() {
        return Err("get_iplayer script not found".to_string());
    }

    // Run `perl get_iplayer --version` to verify it works
    let output = tokio::process::Command::new(&perl_bin)
        .args([script_path.to_string_lossy().as_ref(), "--version"])
        .output()
        .await
        .map_err(|e| format!("Failed to run get_iplayer: {}", e))?;

    // get_iplayer may return non-zero if modules are missing, but still print a version
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    let version = combined
        .lines()
        .find(|line| line.contains("get_iplayer"))
        .unwrap_or("get_iplayer (version unknown)")
        .trim()
        .to_string();

    if output.status.success() {
        Ok(version)
    } else {
        // Partial success — Perl ran but get_iplayer had an issue (likely missing CPAN modules)
        Err(format!(
            "get_iplayer ran but reported errors (may need CPAN modules): {}",
            version
        ))
    }
}
