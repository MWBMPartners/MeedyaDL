// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Service status models for the remote kill-switch system.
//
// These types represent the JSON configuration fetched from
// `raw.githubusercontent.com/MWBMPartners/MeedyaDL/main/service-status.json`.
// The app checks this on launch and every 4 hours to determine if any
// media service should be disabled (e.g., due to API breakage or legal issues).
//
// ## Fail-Open Design
//
// If the remote config is unreachable AND no local cache exists, all services
// default to enabled. This ensures the app remains functional even when the
// status endpoint is down.
//
// ## Schema Versioning
//
// The `version` field allows future backward-incompatible changes. The current
// parser only understands version 1; unknown versions are treated as all-enabled.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level service status configuration fetched from the remote endpoint.
///
/// Contains per-service enable/disable flags and an optional global message
/// for announcements visible to all users.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatusConfig {
    /// Schema version (currently 1). Used for future backward-incompatible changes.
    pub version: u32,
    /// ISO 8601 timestamp of the last config update.
    pub updated_at: String,
    /// Per-service status entries keyed by PascalCase service name
    /// (e.g., "AppleMusic", "YouTube", "BBCiPlayer", "Spotify").
    pub services: HashMap<String, ServiceStatusEntry>,
    /// Optional global announcement shown to all users regardless of
    /// which services they use. `None` or `null` means no announcement.
    pub global_message: Option<String>,
}

/// Status entry for a single media service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatusEntry {
    /// Whether the service is currently enabled for downloads.
    /// `false` means the service is remotely disabled (kill-switched).
    pub enabled: bool,
    /// Optional message explaining why the service is disabled or
    /// providing additional context. Shown in the UI banner.
    pub message: Option<String>,
}

impl ServiceStatusConfig {
    /// Returns a default config with all services enabled.
    /// Used as the fail-open fallback when both the remote endpoint
    /// and the local cache are unavailable.
    pub fn all_enabled() -> Self {
        let mut services = HashMap::new();
        for name in &["AppleMusic", "YouTube", "BBCiPlayer", "Spotify"] {
            services.insert(
                name.to_string(),
                ServiceStatusEntry {
                    enabled: true,
                    message: None,
                },
            );
        }
        Self {
            version: 1,
            updated_at: String::new(),
            services,
            global_message: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_enabled_default() {
        let config = ServiceStatusConfig::all_enabled();
        assert_eq!(config.version, 1);
        assert!(config.global_message.is_none());
        assert_eq!(config.services.len(), 4);
        for entry in config.services.values() {
            assert!(entry.enabled);
            assert!(entry.message.is_none());
        }
    }

    #[test]
    fn test_deserialize_json() {
        let json = r#"{
            "version": 1,
            "updated_at": "2026-02-22T00:00:00Z",
            "services": {
                "AppleMusic": { "enabled": false, "message": "Temporarily disabled" },
                "YouTube": { "enabled": true, "message": null }
            },
            "global_message": "Maintenance window"
        }"#;
        let config: ServiceStatusConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.global_message.as_deref(), Some("Maintenance window"));
        assert!(!config.services["AppleMusic"].enabled);
        assert_eq!(
            config.services["AppleMusic"].message.as_deref(),
            Some("Temporarily disabled")
        );
        assert!(config.services["YouTube"].enabled);
    }
}
