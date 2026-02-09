// Copyright (c) 2024-2026 MWBM Partners Ltd
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Dependency information models.
// Defines structures for tracking the installation status and version
// information of external dependencies (Python, GAMDL, FFmpeg, etc.).

use serde::{Deserialize, Serialize};

/// Information about an external dependency required or used by GAMDL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    /// Human-readable name (e.g., "Python 3.12", "FFmpeg")
    pub name: String,

    /// Whether this dependency is required for basic functionality.
    /// Required: Python, GAMDL, FFmpeg
    /// Optional: mp4decrypt, N_m3u8DL-RE, MP4Box
    pub required: bool,

    /// Current installation status
    pub status: DependencyInstallStatus,

    /// Installed version string, if available
    pub version: Option<String>,

    /// Absolute path to the installed binary
    pub path: Option<String>,

    /// Latest available version (from update check), if known
    pub latest_version: Option<String>,

    /// Whether an update is available
    pub update_available: bool,
}

/// The installation status of a dependency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyInstallStatus {
    /// Not installed; needs to be downloaded
    NotInstalled,

    /// Currently being downloaded/installed
    Installing,

    /// Installed and ready to use
    Installed,

    /// Installation failed with an error
    Error,
}

/// Information about an available update for a dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// Name of the component (e.g., "GAMDL", "Python")
    pub name: String,

    /// Currently installed version
    pub current_version: String,

    /// Latest available version
    pub latest_version: String,

    /// URL to download the update (if applicable)
    pub download_url: Option<String>,

    /// Release notes or changelog for the update
    pub release_notes: Option<String>,

    /// Whether this update is compatible with the current UI version
    pub compatible: bool,
}
