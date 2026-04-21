// Copyright (c) 2026 MeedyaDL
// Licensed under the MIT License. See the LICENSE file in the project
// root for full licence text.
//
//! Version-aware capability flags for the installed `GAMDL` CLI.
//!
//! GAMDL has been evolving rapidly, and some CLI options / config.ini keys
//! are only available on a subset of supported releases. Two concrete cases
//! motivate this module:
//!
//! * `--fetch-extra-tags` / `fetch_extra_tags` was present in every v2.x
//!   release but **removed in v3.0** (upstream commit
//!   [`61ea24b`](https://github.com/glomatico/gamdl/commit/61ea24b), "Remove
//!   extra tags fetching and preview parsing"). Passing the flag to v3.0
//!   causes Click to reject the CLI invocation with `no such option`.
//! * `--database-path` / `database_path` and `--playlist-folder-template` /
//!   `playlist_folder_template` were **added in v3.0**. Passing either to
//!   a v2.x release causes the same kind of Click error.
//!
//! Rather than pinning `MeedyaDL` to a single GAMDL line, we detect the
//! installed version at runtime and only emit flags / INI keys the
//! installed release actually understands. Because `MeedyaDL` still
//! supports GAMDL `>= 2.9.1` (the first release with native
//! `--song-codec-priority` album support), every capability gate is
//! version-range-aware rather than a simple "is v3+?" check.
//!
//! # Threading model
//!
//! Several concurrent subsystems consume capability flags (the download
//! queue, the config.ini writer, the CLI arg builder, etc.), so the
//! detected version lives behind a process-global [`RwLock`]. Writes
//! happen only on startup (dependency probe), when the user reinstalls
//! GAMDL, or in tests — all extremely rare events. Reads happen on every
//! download. An `RwLock` keeps the read path lock-free in practice while
//! still allowing occasional updates without an explicit message passing
//! setup.
//!
//! # Unknown-version defaults
//!
//! When the version has not yet been probed (e.g. GAMDL isn't installed,
//! or the dependency check hasn't run), every capability query returns
//! `false`. This is deliberately the safest default: we never emit
//! options that a future reader of the INI / CLI might reject. The only
//! downside is that users on v2.x who haven't completed setup won't get
//! `fetch_extra_tags` until the version is probed — which happens on the
//! very next dependency check or download attempt.

use std::sync::{LazyLock, RwLock};

use super::gamdl_service::is_version_at_least;

/// Process-global cache of the last detected GAMDL version string.
///
/// Populated by [`set_detected_version`] from:
/// * [`crate::services::gamdl_service::install_gamdl`] after a successful
///   `pip install --upgrade gamdl`, and
/// * [`crate::services::gamdl_service::get_gamdl_version`] whenever the
///   version is probed (startup dependency check, first download, etc.).
///
/// `None` means "we haven't probed yet or GAMDL isn't installed". See the
/// module-level docs for how unknown versions are handled.
static DETECTED_VERSION: LazyLock<RwLock<Option<String>>> =
    LazyLock::new(|| RwLock::new(None));

/// Records the currently installed GAMDL version.
///
/// Passing `None` explicitly clears the cache (used when GAMDL is
/// uninstalled or the probe fails). Poisoned locks are recovered
/// transparently — a panicking reader should never brick the whole
/// capability subsystem.
pub fn set_detected_version(version: Option<String>) {
    let mut guard = match DETECTED_VERSION.write() {
        Ok(g) => g,
        // If another thread panicked while holding the write lock, recover
        // the inner value anyway. Version probing is idempotent so the
        // state is safe to overwrite.
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = version;
}

/// Returns the currently cached GAMDL version string, if any.
#[must_use]
pub fn detected_version() -> Option<String> {
    DETECTED_VERSION
        .read()
        .ok()
        .and_then(|guard| guard.clone())
}

/// Individual CLI / config.ini features whose availability differs across
/// GAMDL releases.
///
/// Keep variants narrow: each variant represents **one** user-facing flag
/// or INI key. Bundling multiple unrelated features into a single variant
/// makes regressions harder to diagnose when upstream tightens or loosens
/// support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamdlFeature {
    /// `--fetch-extra-tags` CLI flag and `fetch_extra_tags` INI key.
    ///
    /// Present in every v2.x release; removed in v3.0 alongside the
    /// preview-parsing code path.
    FetchExtraTags,

    /// Native album-level `--song-codec-priority` (pass the full fallback
    /// chain in a single GAMDL invocation).
    ///
    /// Introduced in v2.9.1. Older releases require `MeedyaDL` to spawn
    /// one subprocess per codec via `try_fallback`.
    NativeCodecPriority,
}

impl GamdlFeature {
    /// Returns `true` when `version` is known to support this feature.
    ///
    /// Version comparison uses [`is_version_at_least`] which tolerates
    /// two-part ("2.9") and unparseable version strings gracefully.
    fn is_available_on(self, version: &str) -> bool {
        match self {
            // Removed in v3.0, so it's available on everything below.
            Self::FetchExtraTags => !is_version_at_least(version, "3.0"),
            // Added in v2.9.1.
            Self::NativeCodecPriority => is_version_at_least(version, "2.9.1"),
        }
    }
}

/// Reports whether the currently installed GAMDL release supports
/// `feature`.
///
/// Returns `false` when the version has not been detected yet — see the
/// "Unknown-version defaults" section in the module docs for rationale.
#[must_use]
pub fn supports(feature: GamdlFeature) -> bool {
    match detected_version() {
        Some(ver) => feature.is_available_on(&ver),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capability cache is process-global, so tests that mutate it
    /// must serialise to avoid interfering with one another. A single
    /// `Mutex` is cheaper than an `RwLock` here and makes the intent
    /// explicit.
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn fetch_extra_tags_is_available_on_v2x() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(Some("2.9.3".to_string()));
        assert!(supports(GamdlFeature::FetchExtraTags));
        set_detected_version(None);
    }

    #[test]
    fn fetch_extra_tags_is_not_available_on_v3() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(Some("3.0".to_string()));
        assert!(!supports(GamdlFeature::FetchExtraTags));
        set_detected_version(Some("3.0.0".to_string()));
        assert!(!supports(GamdlFeature::FetchExtraTags));
        set_detected_version(Some("3.1.2".to_string()));
        assert!(!supports(GamdlFeature::FetchExtraTags));
        set_detected_version(None);
    }

    #[test]
    fn native_codec_priority_requires_v291() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(Some("2.9.0".to_string()));
        assert!(!supports(GamdlFeature::NativeCodecPriority));
        set_detected_version(Some("2.9.1".to_string()));
        assert!(supports(GamdlFeature::NativeCodecPriority));
        set_detected_version(Some("3.0".to_string()));
        assert!(supports(GamdlFeature::NativeCodecPriority));
        set_detected_version(None);
    }

    #[test]
    fn unknown_version_reports_no_capabilities() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(None);
        assert!(!supports(GamdlFeature::FetchExtraTags));
        assert!(!supports(GamdlFeature::NativeCodecPriority));
    }

    #[test]
    fn set_detected_version_roundtrip() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(Some("2.9.3".to_string()));
        assert_eq!(detected_version(), Some("2.9.3".to_string()));
        set_detected_version(None);
        assert_eq!(detected_version(), None);
    }
}
