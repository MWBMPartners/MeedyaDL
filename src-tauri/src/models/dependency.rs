// Copyright (c) 2024-2026 MWBM Partners Ltd
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

    /// Whether this update is compatible with the current gamdl-GUI version.
    /// Set to `false` if a major version bump in the dependency would
    /// require changes to the GUI's integration code. When `false`, the
    /// update UI shows a warning and may block the update until the GUI
    /// itself is updated.
    pub compatible: bool,
}
