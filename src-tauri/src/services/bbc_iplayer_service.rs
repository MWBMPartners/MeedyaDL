// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// BBC iPlayer download service (get_iplayer + yt-dlp fallback).
// ===============================================================
//
// This module will provide the integration with get_iplayer (primary)
// and yt-dlp (fallback) for downloading BBC iPlayer and BBC Sounds
// content. It follows the same subprocess execution pattern as
// `gamdl_service.rs`.
//
// ## Status: Stub (Coming Soon)
//
// This service is part of the multi-service architecture preparation.
// All public functions currently return "not yet implemented" errors.
// Full implementation is planned for a future milestone.
//
// ## Planned Capabilities
//
// - TV programme and episode downloads
// - Radio/podcast (BBC Sounds) downloads
// - Series batch downloads
// - Subtitle download and embedding
// - Quality selection (SD, HD, Full HD)
// - get_iplayer as primary engine with yt-dlp fallback
// - Region restriction handling (UK VPN may be required)
//
// ## References
//
// - get_iplayer: <https://github.com/get-iplayer/get_iplayer>
// - yt-dlp (fallback): <https://github.com/yt-dlp/yt-dlp>

use crate::models::get_iplayer_options::GetIplayerOptions;

/// Checks the installed get_iplayer version.
///
/// # Returns
/// * `Ok(String)` - The installed version string.
/// * `Err(String)` - get_iplayer is not installed or version check failed.
pub async fn check_get_iplayer_version(
    _app: &tauri::AppHandle,
) -> Result<String, String> {
    Err("BBC iPlayer downloads are not yet implemented. Coming soon!".to_string())
}

/// Builds the get_iplayer command with the specified options.
///
/// Constructs a `std::process::Command` with the appropriate arguments
/// based on the provided `GetIplayerOptions`.
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving paths.
/// * `urls` - The BBC iPlayer/Sounds URL(s) to download.
/// * `options` - The get_iplayer options to apply.
///
/// # Returns
/// * `Ok(Command)` - The configured command ready to spawn.
/// * `Err(String)` - Failed to build the command.
pub fn build_get_iplayer_command(
    _app: &tauri::AppHandle,
    _urls: &[String],
    _options: &GetIplayerOptions,
) -> Result<std::process::Command, String> {
    Err("BBC iPlayer downloads are not yet implemented. Coming soon!".to_string())
}

/// Builds a yt-dlp fallback command for BBC iPlayer content.
///
/// Used when get_iplayer fails or is unavailable. Constructs a yt-dlp
/// command configured for BBC iPlayer URLs.
///
/// # Returns
/// * `Ok(Command)` - The configured yt-dlp fallback command.
/// * `Err(String)` - Failed to build the command.
pub fn build_ytdlp_fallback_command(
    _app: &tauri::AppHandle,
    _urls: &[String],
    _options: &GetIplayerOptions,
) -> Result<std::process::Command, String> {
    Err("BBC iPlayer downloads are not yet implemented. Coming soon!".to_string())
}
