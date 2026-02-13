// Copyright (c) 2024-2026 MeedyaDL
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Dependency information models.
// Defines structures for tracking the installation status and version
// information of external dependencies (Python, GAMDL, FFmpeg, etc.).
//
// ## Why external dependencies?
//
// GAMDL is a Python CLI tool that relies on several external binaries to
// download, decrypt, and remux Apple Music content. This application
// manages those dependencies automatically so the user does not need to
// install them manually. The dependency system handles:
//
// - **Python 3.12+** -- runtime for GAMDL itself (installed via standalone
//   Python builds or system Python).
// - **GAMDL** -- the core Apple Music downloader, installed via pip into
//   a managed virtual environment.
// - **FFmpeg** -- required for audio/video remuxing and format conversion.
//   Downloaded as a pre-built binary from GitHub releases.
// - **mp4decrypt** (Bento4) -- optional, used to decrypt Widevine DRM
//   protected content.
// - **N_m3u8DL-RE** -- optional, alternative HLS stream downloader that
//   can be faster than yt-dlp.
// - **MP4Box** (GPAC) -- optional, alternative remuxer.
//
// ## Architecture
//
// The data structures in this module are pure data models. The actual
// installation, version checking, and update logic lives in
// `commands/dependency.rs`. These models are:
//
// 1. Stored in the Tauri app state as the current dependency status.
// 2. Sent to the React frontend over IPC so the setup/status UI can
//    display installation progress and version information.
// 3. Used by the download manager to locate binary paths before
//    spawning GAMDL subprocesses.
//
// ## References
//
// - serde: <https://docs.rs/serde/latest/serde/>
// - GAMDL installation: <https://github.com/glomatico/gamdl#installation>

use serde::{Deserialize, Serialize};

/// Information about an external dependency required or used by GAMDL.
///
/// Each dependency in the system is represented by one `DependencyInfo`
/// instance, stored in a `Vec<DependencyInfo>` in the Tauri app state.
/// The React frontend's setup wizard and status page use this data to
/// show which dependencies are installed, their versions, and whether
/// updates are available.
///
/// ## Required vs. optional dependencies
///
/// | Dependency    | Required | Purpose                                      |
/// |---------------|----------|----------------------------------------------|
/// | Python 3.12+  | Yes      | Runtime for GAMDL CLI                        |
/// | GAMDL         | Yes      | Core Apple Music downloader                  |
/// | FFmpeg        | Yes      | Audio/video remuxing and conversion          |
/// | mp4decrypt    | No       | Widevine DRM content decryption              |
/// | N_m3u8DL-RE   | No       | Alternative (faster) HLS stream downloader   |
/// | MP4Box        | No       | Alternative remuxer (GPAC)                   |
///
/// ## Serialization
///
/// Serialized to JSON for the Tauri IPC bridge. The frontend TypeScript
/// type mirrors this struct field-for-field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    /// Human-readable name displayed in the setup wizard and status page
    /// (e.g., `"Python 3.12"`, `"FFmpeg"`, `"mp4decrypt"`).
    pub name: String,

    /// Whether this dependency is required for basic functionality.
    ///
    /// - `true` -- the app cannot download content without it. The setup
    ///   wizard blocks until all required dependencies are installed.
    /// - `false` -- the dependency unlocks optional features (e.g.,
    ///   alternative download mode or DRM decryption). The app functions
    ///   without it, but some settings may be unavailable.
    pub required: bool,

    /// Current installation status. Drives the UI badge colour and the
    /// setup wizard's progress indicators. See `DependencyInstallStatus`
    /// for the possible states.
    pub status: DependencyInstallStatus,

    /// Installed version string (e.g., `"3.12.4"`, `"7.1"`, `"2024.05.20"`).
    /// `None` when the dependency is not installed or the version could not
    /// be determined (e.g., the binary exists but `--version` failed).
    pub version: Option<String>,

    /// Absolute filesystem path to the installed binary or virtual environment.
    /// Used by `GamdlOptions` path fields (see `gamdl_options.rs`) when
    /// constructing CLI arguments. `None` when not installed.
    pub path: Option<String>,

    /// Latest available version as reported by the update check (PyPI for
    /// Python packages, GitHub releases for binaries). `None` when the
    /// update check has not been performed or is not applicable.
    pub latest_version: Option<String>,

    /// Whether an update is available. Computed by comparing `version`
    /// with `latest_version` using semver or date-based comparison.
    /// The frontend shows an "Update available" badge when `true`.
    pub update_available: bool,
}

/// The installation status of a dependency.
///
/// Models the lifecycle of a dependency installation as a simple state
/// machine:
///
/// ```text
///   ┌──────────────┐     ┌────────────┐     ┌───────────┐
///   │ NotInstalled │────>│ Installing │────>│ Installed │
///   └──────────────┘     └────────────┘     └───────────┘
///                              │
///                              ▼
///                        ┌───────────┐
///                        │   Error   │
///                        └───────────┘
/// ```
///
/// `NotInstalled` is the initial state. `Installing` is transient (only
/// active while a download/install task is running). `Installed` and
/// `Error` are the two possible outcomes.
///
/// ## Serialization
///
/// `#[serde(rename_all = "snake_case")]` produces `"not_installed"`,
/// `"installing"`, `"installed"`, and `"error"` in JSON -- matching the
/// TypeScript union type in the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyInstallStatus {
    /// The dependency is not present on the system. The setup wizard
    /// shows a "Download" button in this state.
    NotInstalled,

    /// The dependency is currently being downloaded and/or installed.
    /// The setup wizard shows a progress spinner in this state. This
    /// is a transient state that automatically transitions to either
    /// `Installed` or `Error`.
    Installing,

    /// The dependency is installed and ready to use. The binary at
    /// `DependencyInfo::path` has been verified to exist and return
    /// a valid version string.
    Installed,

    /// Installation failed. The error details are typically logged to
    /// the application log. The setup wizard shows a "Retry" button
    /// and may display the error message.
    Error,
}

/// Information about an available update for a dependency.
///
/// Returned by the `check_updates` Tauri command (see `commands/dependency.rs`)
/// when a newer version of a dependency is detected. The React frontend
/// displays this in an "Updates available" notification panel, allowing
/// the user to review release notes and decide whether to update.
///
/// ## Update sources
///
/// - **GAMDL**: checked via PyPI JSON API (`https://pypi.org/pypi/gamdl/json`).
/// - **FFmpeg**: checked via GitHub releases API.
/// - **mp4decrypt, N_m3u8DL-RE, MP4Box**: checked via their respective
///   GitHub release pages.
/// - **Python**: checked via the `python-build-standalone` GitHub releases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// Name of the component (e.g., `"GAMDL"`, `"Python"`, `"FFmpeg"`).
    /// Matches the `DependencyInfo::name` field for correlation.
    pub name: String,

    /// Currently installed version string. Displayed alongside the latest
    /// version so the user can see what they are upgrading from.
    pub current_version: String,

    /// Latest available version string from the upstream source.
    pub latest_version: String,

    /// Direct download URL for the update binary or package. `None` for
    /// pip-managed packages (which are updated via `pip install --upgrade`).
    /// `Some(url)` for binary dependencies that are downloaded directly.
    pub download_url: Option<String>,

    /// Release notes or changelog excerpt for the update. Displayed in
    /// the update notification panel so the user can assess the update's
    /// importance. `None` when release notes are not available.
    pub release_notes: Option<String>,

    /// Whether this update is compatible with the current MeedyaDL version.
    /// Set to `false` if a major version bump in the dependency would
    /// require changes to the GUI's integration code. When `false`, the
    /// update UI shows a warning and may block the update until the GUI
    /// itself is updated.
    pub compatible: bool,
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // DependencyInstallStatus serde serialization
    // ----------------------------------------------------------

    /// Verifies that `DependencyInstallStatus::NotInstalled` serializes
    /// to `"not_installed"` as expected by the React frontend's
    /// TypeScript union type.
    #[test]
    fn dependency_status_not_installed_serializes_correctly() {
        let json = serde_json::to_string(&DependencyInstallStatus::NotInstalled).unwrap();
        assert_eq!(json, "\"not_installed\"");
    }

    /// Verifies that `DependencyInstallStatus::Installing` serializes
    /// to `"installing"` for the setup wizard's progress spinner.
    #[test]
    fn dependency_status_installing_serializes_correctly() {
        let json = serde_json::to_string(&DependencyInstallStatus::Installing).unwrap();
        assert_eq!(json, "\"installing\"");
    }

    /// Verifies that `DependencyInstallStatus::Installed` serializes
    /// to `"installed"` for the setup wizard's completion badge.
    #[test]
    fn dependency_status_installed_serializes_correctly() {
        let json = serde_json::to_string(&DependencyInstallStatus::Installed).unwrap();
        assert_eq!(json, "\"installed\"");
    }

    /// Verifies that `DependencyInstallStatus::Error` serializes
    /// to `"error"` for the setup wizard's error/retry display.
    #[test]
    fn dependency_status_error_serializes_correctly() {
        let json = serde_json::to_string(&DependencyInstallStatus::Error).unwrap();
        assert_eq!(json, "\"error\"");
    }

    /// Verifies that all `DependencyInstallStatus` variants survive a
    /// full serde roundtrip (serialize then deserialize) without
    /// data loss, ensuring consistent IPC communication.
    #[test]
    fn dependency_status_serde_roundtrip_all_variants() {
        let variants = vec![
            DependencyInstallStatus::NotInstalled,
            DependencyInstallStatus::Installing,
            DependencyInstallStatus::Installed,
            DependencyInstallStatus::Error,
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: DependencyInstallStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(
                deserialized, variant,
                "Roundtrip failed for {:?} (json: {})",
                variant, json
            );
        }
    }

    // ----------------------------------------------------------
    // DependencyInfo serde roundtrip
    // ----------------------------------------------------------

    /// Verifies that a fully populated `DependencyInfo` with all
    /// optional fields set survives a serde roundtrip, ensuring the
    /// setup wizard receives complete dependency information.
    #[test]
    fn dependency_info_serde_roundtrip_all_fields_populated() {
        let info = DependencyInfo {
            name: "Python 3.12".to_string(),
            required: true,
            status: DependencyInstallStatus::Installed,
            version: Some("3.12.4".to_string()),
            path: Some("/opt/python/bin/python3".to_string()),
            latest_version: Some("3.12.5".to_string()),
            update_available: true,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: DependencyInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "Python 3.12");
        assert!(deserialized.required);
        assert_eq!(deserialized.status, DependencyInstallStatus::Installed);
        assert_eq!(deserialized.version, Some("3.12.4".to_string()));
        assert_eq!(deserialized.path, Some("/opt/python/bin/python3".to_string()));
        assert_eq!(deserialized.latest_version, Some("3.12.5".to_string()));
        assert!(deserialized.update_available);
    }

    /// Verifies that a `DependencyInfo` with all optional fields set
    /// to `None` (representing a not-yet-installed dependency) survives
    /// a serde roundtrip correctly, as the setup wizard must handle
    /// this state for initial installation.
    #[test]
    fn dependency_info_serde_roundtrip_optional_fields_none() {
        let info = DependencyInfo {
            name: "FFmpeg".to_string(),
            required: true,
            status: DependencyInstallStatus::NotInstalled,
            version: None,
            path: None,
            latest_version: None,
            update_available: false,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: DependencyInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "FFmpeg");
        assert!(deserialized.required);
        assert_eq!(deserialized.status, DependencyInstallStatus::NotInstalled);
        assert!(deserialized.version.is_none());
        assert!(deserialized.path.is_none());
        assert!(deserialized.latest_version.is_none());
        assert!(!deserialized.update_available);
    }

    /// Verifies that an optional dependency (not required for basic
    /// functionality) serializes with `required: false` and the
    /// correct status, ensuring the setup wizard can distinguish
    /// required from optional dependencies.
    #[test]
    fn dependency_info_optional_dependency() {
        let info = DependencyInfo {
            name: "mp4decrypt".to_string(),
            required: false,
            status: DependencyInstallStatus::NotInstalled,
            version: None,
            path: None,
            latest_version: None,
            update_available: false,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: DependencyInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "mp4decrypt");
        assert!(!deserialized.required);
    }

    // ----------------------------------------------------------
    // UpdateInfo serde roundtrip
    // ----------------------------------------------------------

    /// Verifies that a fully populated `UpdateInfo` with download URL
    /// and release notes survives a serde roundtrip, ensuring the
    /// update notification panel receives all information needed to
    /// display and execute the update.
    #[test]
    fn update_info_serde_roundtrip_all_fields() {
        let info = UpdateInfo {
            name: "GAMDL".to_string(),
            current_version: "2024.05.20".to_string(),
            latest_version: "2024.06.15".to_string(),
            download_url: Some("https://github.com/glomatico/gamdl/releases/download/v2024.06.15/gamdl-2024.06.15.tar.gz".to_string()),
            release_notes: Some("Bug fixes and performance improvements.".to_string()),
            compatible: true,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: UpdateInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "GAMDL");
        assert_eq!(deserialized.current_version, "2024.05.20");
        assert_eq!(deserialized.latest_version, "2024.06.15");
        assert!(deserialized.download_url.is_some());
        assert!(deserialized.release_notes.is_some());
        assert!(deserialized.compatible);
    }

    /// Verifies that an `UpdateInfo` for a pip-managed package (where
    /// `download_url` and `release_notes` are `None`) survives a serde
    /// roundtrip correctly.
    #[test]
    fn update_info_serde_roundtrip_optional_fields_none() {
        let info = UpdateInfo {
            name: "Python".to_string(),
            current_version: "3.12.4".to_string(),
            latest_version: "3.12.5".to_string(),
            download_url: None,
            release_notes: None,
            compatible: true,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: UpdateInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "Python");
        assert_eq!(deserialized.current_version, "3.12.4");
        assert_eq!(deserialized.latest_version, "3.12.5");
        assert!(deserialized.download_url.is_none());
        assert!(deserialized.release_notes.is_none());
        assert!(deserialized.compatible);
    }

    /// Verifies that an incompatible `UpdateInfo` (where a major version
    /// bump would break GUI integration) correctly preserves the
    /// `compatible: false` flag through a serde roundtrip.
    #[test]
    fn update_info_incompatible_update() {
        let info = UpdateInfo {
            name: "FFmpeg".to_string(),
            current_version: "6.1".to_string(),
            latest_version: "7.0".to_string(),
            download_url: Some("https://example.com/ffmpeg-7.0.tar.gz".to_string()),
            release_notes: Some("Major version bump with breaking API changes.".to_string()),
            compatible: false,
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: UpdateInfo = serde_json::from_str(&json).unwrap();

        assert!(!deserialized.compatible);
        assert_eq!(deserialized.latest_version, "7.0");
    }
}
