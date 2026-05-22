// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Service status checking service.
//
// Fetches the remote service-status.json configuration from GitHub to
// determine if any media download service should be remotely disabled.
// Implements a three-tier fallback: remote → local cache → all-enabled default.
//
// ## Remote Endpoint
//
// The config is hosted at:
//   https://raw.githubusercontent.com/MWBMPartners/MeedyaDL/main/service-status.json
//
// ## Caching
//
// On successful fetch, the response is cached to `{app_data_dir}/service-status-cache.json`.
// If the remote endpoint is unreachable, the cached version is used. If neither is
// available, all services default to enabled (fail-open design).
//
// ## References
//
// - Reqwest HTTP client: https://docs.rs/reqwest/latest/reqwest/
// - Tauri AppHandle: https://docs.rs/tauri/latest/tauri/struct.AppHandle.html

use tauri::AppHandle;

use crate::models::service_status::ServiceStatusConfig;
use crate::models::media_service::MediaServiceId;
use crate::utils::platform;

/// URL for the remote service status configuration file.
const SERVICE_STATUS_URL: &str =
    "https://raw.githubusercontent.com/MWBMPartners/MeedyaDL/main/service-status.json";

/// Filename for the local cache of the service status config.
const CACHE_FILENAME: &str = "service-status-cache.json";

/// HTTP request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 10;

/// Fetches the service status configuration from the remote endpoint.
///
/// Falls back to cached config on network failure, and to an all-enabled
/// default if no cache exists. This fail-open design ensures the app
/// remains functional even when the status endpoint is unavailable.
///
/// # Arguments
/// * `app` - Tauri AppHandle for resolving the app data directory.
///
/// # Returns
/// * `Ok(ServiceStatusConfig)` - The resolved service status config.
/// * `Err(String)` - Only if something unexpected goes wrong (should not
///   happen in practice due to the all-enabled fallback).
pub async fn fetch_service_status(app: &AppHandle) -> Result<ServiceStatusConfig, String> {
    // Try fetching from the remote endpoint first
    match fetch_remote().await {
        Ok(config) => {
            // Cache the successful response for offline use
            if let Err(e) = save_cache(app, &config) {
                log::warn!("Failed to cache service status: {}", e);
            }
            log::info!("Service status fetched from remote (version {})", config.version);
            Ok(config)
        }
        Err(e) => {
            log::warn!("Failed to fetch remote service status: {}. Trying cache...", e);
            // Fall back to cached config
            match load_cache(app) {
                Ok(config) => {
                    log::info!("Using cached service status (updated_at: {})", config.updated_at);
                    Ok(config)
                }
                Err(cache_err) => {
                    log::warn!(
                        "No cached service status available: {}. Using all-enabled default.",
                        cache_err
                    );
                    // Fail-open: all services enabled
                    Ok(ServiceStatusConfig::all_enabled())
                }
            }
        }
    }
}

/// Fetches the service status config from the remote GitHub endpoint.
async fn fetch_remote() -> Result<ServiceStatusConfig, String> {
    let client = crate::utils::http_client::build_simple(REQUEST_TIMEOUT_SECS)?;

    let response = client
        .get(SERVICE_STATUS_URL)
        .header("User-Agent", "MeedyaDL")
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} from service status endpoint", response.status()));
    }

    let config: ServiceStatusConfig = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse service status JSON: {}", e))?;

    // Only accept version 1 configs; unknown versions fall through to default
    if config.version != 1 {
        return Err(format!("Unknown service status schema version: {}", config.version));
    }

    Ok(config)
}

/// Saves the service status config to the local cache file.
fn save_cache(app: &AppHandle, config: &ServiceStatusConfig) -> Result<(), String> {
    let cache_path = platform::get_app_data_dir(app).join(CACHE_FILENAME);
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize cache: {}", e))?;
    std::fs::write(&cache_path, json)
        .map_err(|e| format!("Failed to write cache file: {}", e))?;
    Ok(())
}

/// Loads the service status config from the local cache file.
fn load_cache(app: &AppHandle) -> Result<ServiceStatusConfig, String> {
    let cache_path = platform::get_app_data_dir(app).join(CACHE_FILENAME);
    let json = std::fs::read_to_string(&cache_path)
        .map_err(|e| format!("Failed to read cache file: {}", e))?;
    let config: ServiceStatusConfig = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse cached JSON: {}", e))?;
    Ok(config)
}

/// Loads the cached service status from disk (synchronous, for use in the
/// download queue gate where we don't want to make a network call).
///
/// Returns `None` if the cache doesn't exist or can't be parsed.
pub fn load_cached_status(app: &AppHandle) -> Option<ServiceStatusConfig> {
    load_cache(app).ok()
}

/// Checks whether a specific service is disabled in the given config.
///
/// Maps the `MediaServiceId` enum to the PascalCase key used in the
/// remote JSON config. Returns `true` if the service is explicitly
/// disabled, `false` otherwise (including when the service key is missing).
pub fn is_service_disabled(config: &ServiceStatusConfig, service: &MediaServiceId) -> bool {
    let key = service_id_to_key(service);
    config
        .services
        .get(key)
        .map(|entry| !entry.enabled)
        .unwrap_or(false) // Missing key = not disabled (fail-open)
}

/// Returns the user-facing message for a disabled service, if any.
pub fn get_service_message(config: &ServiceStatusConfig, service: &MediaServiceId) -> Option<String> {
    let key = service_id_to_key(service);
    config
        .services
        .get(key)
        .and_then(|entry| entry.message.clone())
}

/// Maps a `MediaServiceId` to its PascalCase key in the JSON config.
fn service_id_to_key(service: &MediaServiceId) -> &'static str {
    match service {
        MediaServiceId::AppleMusic => "AppleMusic",
        MediaServiceId::YouTube => "YouTube",
        MediaServiceId::YouTubeMusic => "YouTubeMusic",
        MediaServiceId::BBCiPlayer => "BBCiPlayer",
        MediaServiceId::Spotify => "Spotify",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::models::service_status::ServiceStatusEntry;

    fn make_config(apple_enabled: bool, message: Option<&str>) -> ServiceStatusConfig {
        let mut services = HashMap::new();
        services.insert(
            "AppleMusic".to_string(),
            ServiceStatusEntry {
                enabled: apple_enabled,
                message: message.map(String::from),
            },
        );
        services.insert(
            "YouTube".to_string(),
            ServiceStatusEntry {
                enabled: true,
                message: None,
            },
        );
        ServiceStatusConfig {
            version: 1,
            updated_at: "2026-02-22T00:00:00Z".to_string(),
            services,
            global_message: None,
        }
    }

    #[test]
    fn test_service_disabled() {
        let config = make_config(false, Some("API down"));
        assert!(is_service_disabled(&config, &MediaServiceId::AppleMusic));
        assert!(!is_service_disabled(&config, &MediaServiceId::YouTube));
        // Missing key = not disabled (fail-open)
        assert!(!is_service_disabled(&config, &MediaServiceId::Spotify));
    }

    #[test]
    fn test_service_message() {
        let config = make_config(false, Some("API down"));
        assert_eq!(
            get_service_message(&config, &MediaServiceId::AppleMusic),
            Some("API down".to_string())
        );
        assert_eq!(get_service_message(&config, &MediaServiceId::YouTube), None);
        assert_eq!(get_service_message(&config, &MediaServiceId::Spotify), None);
    }

    #[test]
    fn test_service_id_to_key() {
        assert_eq!(service_id_to_key(&MediaServiceId::AppleMusic), "AppleMusic");
        assert_eq!(service_id_to_key(&MediaServiceId::YouTube), "YouTube");
        assert_eq!(service_id_to_key(&MediaServiceId::BBCiPlayer), "BBCiPlayer");
        assert_eq!(service_id_to_key(&MediaServiceId::Spotify), "Spotify");
    }
}
