// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// API field audit command — diagnostic tool for discovering new API fields
// ========================================================================
//
// Exposes the API field audit service as a Tauri IPC command. Used from the
// Settings > Metadata tab to compare Apple Music API responses against the
// known tag definitions in tags.toml.
//
// @see services/api_audit_service.rs — Business logic
// @see models/tag_registry.rs — Tag definitions from tags.toml

use tauri::AppHandle;

use crate::services::api_audit_service::{self, ApiAuditResult};

/// Audit an Apple Music album's API fields against the tag registry.
///
/// Fetches the album from the Apple Music API, flattens all JSON attribute
/// paths, and compares them against the known tag definitions in tags.toml.
/// Reports known (mapped), unknown (new/unmapped), and missing (expected
/// but absent) fields.
///
/// Requires MusicKit credentials (Team ID, Key ID, private key in keychain).
///
/// # Arguments
/// * `album_url` - Apple Music album URL (e.g., "https://music.apple.com/us/album/.../1234567890")
///
/// # Returns
/// * `Ok(ApiAuditResult)` - Audit results with field breakdowns
/// * `Err(String)` - Credential, URL, or API error
#[tauri::command]
pub async fn audit_api_fields(app: AppHandle, album_url: String) -> Result<ApiAuditResult, String> {
    api_audit_service::audit_album_fields(&app, album_url).await
}
