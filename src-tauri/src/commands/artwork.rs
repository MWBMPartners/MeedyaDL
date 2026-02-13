// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Animated artwork IPC command.
// ==============================
//
// Provides a Tauri command for manually triggering animated artwork
// downloads for completed albums. This allows the frontend to offer a
// "Download Artwork" button on completed queue items or in the settings.
//
// The command delegates to `services::animated_artwork_service`, which
// handles credential loading, Apple Music API queries, and FFmpeg HLS
// download.
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command                | TypeScript Function               |
// |-----------------------------|-----------------------------------|
// | download_animated_artwork   | downloadAnimatedArtwork(urls, dir)|
//
// ## References
//
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - animated_artwork_service: src-tauri/src/services/animated_artwork_service.rs

use tauri::AppHandle;

use crate::services::animated_artwork_service::{self, ArtworkResult};

/// Manually download animated artwork for an album.
///
/// **Frontend caller:** `downloadAnimatedArtwork(urls, outputDir)` in
/// `src/lib/tauri-commands.ts`
///
/// This command is used when the user explicitly requests artwork download
/// for a specific album (e.g., via a button on a completed queue item).
/// It runs the same logic as the automatic post-download hook but can be
/// triggered independently.
///
/// # Arguments
/// * `app` - Tauri AppHandle for accessing settings, keychain, and FFmpeg path
/// * `urls` - Apple Music URL(s) for the album (used to extract storefront/ID)
/// * `output_dir` - The album directory where `FrontCover.mp4` and
///   `PortraitCover.mp4` should be saved
///
/// # Returns
/// * `Ok(ArtworkResult)` - Which artwork types were downloaded
/// * `Err(String)` - Error message if the process failed
#[tauri::command]
pub async fn download_animated_artwork(
    app: AppHandle,
    urls: Vec<String>,
    output_dir: String,
) -> Result<ArtworkResult, String> {
    animated_artwork_service::process_album_artwork(&app, &urls, &output_dir).await
}
