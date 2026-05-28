// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Spotify download service (votify wrapper).
// ============================================
//
// This module will provide the integration with votify for downloading
// Spotify content (tracks, albums, playlists, artist discographies).
// It follows the same subprocess execution pattern as `gamdl_service.rs`.
//
// ## Status: Stub (Coming Soon)
//
// This service is part of the multi-service architecture preparation.
// All public functions currently return "not yet implemented" errors.
// Full implementation is planned for a future milestone.
//
// ## Planned Capabilities
//
// - Track, album, and playlist downloads
// - Artist discography batch downloads
// - Ogg Vorbis quality selection (high/medium/low)
// - Cover art saving (embedded + separate file)
// - Lyrics embedding
// - Cookie-based Spotify authentication
//
// ## References
//
// - votify: <https://github.com/glomatico/votify>

use crate::models::votify_options::VotifyOptions;

/// Checks the installed votify version.
///
/// # Returns
/// * `Ok(String)` - The installed version string.
/// * `Err(String)` - votify is not installed or version check failed.
pub async fn check_votify_version(
    _app: &tauri::AppHandle,
) -> Result<String, String> {
    Err("Spotify downloads are not yet implemented. Coming soon!".to_string())
}

/// Installs votify via pip into the managed Python environment.
///
/// # Returns
/// * `Ok(())` - votify was installed successfully.
/// * `Err(String)` - Installation failed.
pub async fn install_votify(
    _app: &tauri::AppHandle,
) -> Result<(), String> {
    Err("Spotify downloads are not yet implemented. Coming soon!".to_string())
}

/// Builds the votify command with the specified options.
///
/// Constructs a `std::process::Command` with the appropriate arguments
/// based on the provided `VotifyOptions`. The command targets the managed
/// Python environment's votify installation.
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving the managed Python path.
/// * `urls` - The Spotify URL(s) to download.
/// * `options` - The votify options to apply.
///
/// # Returns
/// * `Ok(Command)` - The configured command ready to spawn.
/// * `Err(String)` - Failed to build the command.
pub fn build_votify_command(
    _app: &tauri::AppHandle,
    _urls: &[String],
    _options: &VotifyOptions,
) -> Result<std::process::Command, String> {
    Err("Spotify downloads are not yet implemented. Coming soon!".to_string())
}

/// Checks the latest votify version available on PyPI.
///
/// # Returns
/// * `Ok(String)` - The latest version string.
/// * `Err(String)` - Network error or PyPI API parsing failure.
pub async fn check_latest_votify_version() -> Result<String, String> {
    Err("Spotify downloads are not yet implemented. Coming soon!".to_string())
}
