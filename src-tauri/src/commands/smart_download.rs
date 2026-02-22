// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Smart Download IPC command.
//
// Exposes the cross-platform Smart Download search to the frontend.
// Called when a user pastes a URL and has Smart Download enabled in settings.
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command          | TypeScript Function        |
// |-----------------------|----------------------------|
// | check_cross_platform  | checkCrossPlatform()       |

use tauri::AppHandle;

use crate::models::content_match::SmartDownloadResult;
use crate::models::media_service::MediaServiceId;
use crate::services::smart_download;

/// Searches all enabled services for cross-platform matches of the given content.
///
/// **Frontend caller:** `checkCrossPlatform()` in `src/lib/tauri-commands.ts`
///
/// Extracts content identifiers (ISRC/UPC) from the source service and
/// searches other enabled services for the same content, returning matches
/// ranked by quality tier.
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving settings and credentials.
/// * `url` - The user's pasted URL.
/// * `service_id` - The detected source service as a string (e.g., "AppleMusic").
///
/// # Returns
/// * `Ok(SmartDownloadResult)` - Cross-platform search results.
/// * `Err(String)` - Error message if the search fails.
#[tauri::command]
pub async fn check_cross_platform(
    app: AppHandle,
    url: String,
    service_id: String,
) -> Result<SmartDownloadResult, String> {
    log::info!(
        "Smart Download: checking cross-platform for {} (service: {})",
        url,
        service_id
    );

    // Parse the service_id string to a MediaServiceId enum.
    // The frontend sends the PascalCase name (e.g., "AppleMusic").
    let parsed_service = match service_id.as_str() {
        "AppleMusic" => MediaServiceId::AppleMusic,
        "YouTube" => MediaServiceId::YouTube,
        "BBCiPlayer" => MediaServiceId::BBCiPlayer,
        "Spotify" => MediaServiceId::Spotify,
        _ => return Err(format!("Unknown service: {}", service_id)),
    };

    smart_download::check_cross_platform(&app, &url, parsed_service).await
}
