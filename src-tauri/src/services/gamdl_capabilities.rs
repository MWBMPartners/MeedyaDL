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

use serde::Deserialize;

use super::gamdl_service::is_version_at_least;

// ============================================================
// Support window (compiled from tool-versions.toml)
// ============================================================
//
// The support window bounds which GAMDL releases *this MeedyaDL
// build* has been validated against. It is the single source of
// truth consumed by:
//
//   * `install_gamdl` — caps `pip install --upgrade` to the tested
//     range, so setup-wizard / "Update GAMDL" flows never pull a
//     release we haven't validated.
//   * `check_latest_gamdl_version` / the update banner — suppresses
//     the update prompt when upstream ships beyond the ceiling.
//     Users who already have a newer version still see their
//     installed value; they just don't get nudged upward.
//   * `get_component_versions` / startup diagnostics — classifies
//     the installed version as Unsupported / Supported / Untested /
//     NotInstalled for the activity log and component dashboard.
//
// The window is compiled into the binary from `tool-versions.toml`
// via `include_str!()` (same pattern as the external-tool pinning),
// so bumping it is a one-file PR. The parse happens exactly once
// (lazy static) — no runtime TOML cost on the hot path.

/// Raw TOML body of `src-tauri/tool-versions.toml`, embedded at compile
/// time. Mirrors the pattern used by `dependency_manager.rs` for the
/// external-tool pins.
const TOOL_VERSIONS_TOML: &str = include_str!("../../tool-versions.toml");

/// Deserialisation shape for the `[gamdl]` section of `tool-versions.toml`.
///
/// Kept private because the public API is [`GamdlSupportWindow`] — we
/// want callers to depend on the semantic struct, not on the on-disk
/// layout.
#[derive(Debug, Deserialize)]
struct GamdlSupportToml {
    minimum_version: String,
    maximum_tested_version: String,
    recommended_version: String,
}

#[derive(Debug, Deserialize)]
struct ToolVersionsToml {
    gamdl: GamdlSupportToml,
}

/// The validated GAMDL version range baked into this MeedyaDL build.
///
/// * `minimum` — oldest release MeedyaDL still supports. Below this
///   threshold, features like native `--song-codec-priority` silently
///   degrade.
/// * `maximum_tested` — highest release we have actually exercised in
///   CI / manual testing. Above this threshold we suppress update
///   prompts; users manually running newer builds are warned but not
///   blocked.
/// * `recommended` — what the installer resolves to by default.
///   Always within `[minimum, maximum_tested]`.
#[derive(Debug, Clone)]
pub struct GamdlSupportWindow {
    pub minimum: String,
    pub maximum_tested: String,
    pub recommended: String,
}

/// Parses the embedded `tool-versions.toml` once and caches the
/// result. A parse error is a programmer error — the TOML ships in
/// our repo and is covered by the `support_window_parses` test — so
/// we unwrap with a descriptive panic rather than surfacing an
/// `Option` that every caller would have to unwrap anyway.
static SUPPORT_WINDOW: LazyLock<GamdlSupportWindow> = LazyLock::new(|| {
    let parsed: ToolVersionsToml = toml::from_str(TOOL_VERSIONS_TOML)
        .expect("tool-versions.toml must contain a valid [gamdl] section");
    GamdlSupportWindow {
        minimum: parsed.gamdl.minimum_version,
        maximum_tested: parsed.gamdl.maximum_tested_version,
        recommended: parsed.gamdl.recommended_version,
    }
});

/// Returns the support window compiled into this MeedyaDL build.
#[must_use]
pub fn support_window() -> &'static GamdlSupportWindow {
    &SUPPORT_WINDOW
}

/// Classification of the installed GAMDL version relative to the
/// support window.
///
/// Each variant carries the context needed to render a helpful
/// message — we avoid returning a bare `bool` because the UI needs
/// to show specific numbers ("GAMDL 3.1 is newer than the tested
/// 3.0 ceiling"), not a yes/no.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionSupport {
    /// GAMDL is not installed at all (e.g., setup wizard hasn't run,
    /// or pip is broken).
    NotInstalled,
    /// Installed version is below `minimum`. Some MeedyaDL features
    /// will silently degrade. Surface a prominent warning.
    Unsupported { installed: String, minimum: String },
    /// Installed version is inside `[minimum, maximum_tested]`. Normal
    /// operation.
    Supported { installed: String },
    /// Installed version is above `maximum_tested`. MeedyaDL hasn't
    /// validated this release — proceed but warn, and offer a
    /// downgrade to `recommended`.
    Untested {
        installed: String,
        maximum_tested: String,
        recommended: String,
    },
}

impl VersionSupport {
    /// Is the installed version inside the supported range?
    ///
    /// Convenience for call sites that only need a boolean (e.g.
    /// deciding whether to suppress a warning toast).
    #[must_use]
    pub const fn is_supported(&self) -> bool {
        matches!(self, Self::Supported { .. })
    }
}

/// Classifies `installed` against the compiled support window.
///
/// A `None` installed version means GAMDL is not installed; we
/// return [`VersionSupport::NotInstalled`] rather than picking an
/// arbitrary default.
#[must_use]
pub fn classify(installed: Option<&str>) -> VersionSupport {
    let Some(installed) = installed else {
        return VersionSupport::NotInstalled;
    };
    let window = support_window();

    if !is_version_at_least(installed, &window.minimum) {
        return VersionSupport::Unsupported {
            installed: installed.to_string(),
            minimum: window.minimum.clone(),
        };
    }

    // `is_version_at_least(a, b)` returns `a >= b`. The "untested"
    // case is `installed > maximum_tested`, i.e. installed is at
    // least one patch ahead of the ceiling. We phrase it as
    // `!is_version_at_least(maximum_tested, installed)` — true iff
    // `maximum_tested < installed`.
    if !is_version_at_least(&window.maximum_tested, installed) {
        return VersionSupport::Untested {
            installed: installed.to_string(),
            maximum_tested: window.maximum_tested.clone(),
            recommended: window.recommended.clone(),
        };
    }

    VersionSupport::Supported {
        installed: installed.to_string(),
    }
}

/// Should the "GAMDL update available" notice be shown to the user?
///
/// Returns `false` when:
/// * `latest_available` is above the `maximum_tested_version` baked
///   into this MeedyaDL build — we don't want to push users into
///   untested territory. They can still upgrade manually if they
///   want to.
/// * `latest_available` is not a parseable semver string (we won't
///   offer upgrades to a version we can't understand — `1.2.3`,
///   `2.9` are fine; `invalid`, `v3-rc` are not).
#[must_use]
pub fn should_offer_upgrade(latest_available: &str) -> bool {
    if !is_parseable_semver(latest_available) {
        return false;
    }
    let window = support_window();
    // Cap the suggested upgrade at `maximum_tested`: offer only when
    // the latest PyPI release is still inside our validated window.
    is_version_at_least(&window.maximum_tested, latest_available)
}

/// Returns `true` if `version` is a leading-numeric semver-ish string
/// (e.g. `2.9`, `2.9.1`, `3.0.0`).
///
/// The existing `is_version_at_least` helper silently substitutes `0`
/// for unparseable parts, which is convenient for comparisons but
/// unsafe here: "garbage" strings would compare equal to `(0, 0, 0)`
/// and falsely pass the ceiling check. This guard keeps
/// `should_offer_upgrade` strict.
fn is_parseable_semver(version: &str) -> bool {
    let mut parts = version.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    first.parse::<u32>().is_ok()
}

/// Pip version specifier string for `pip install --upgrade`.
///
/// Example output: `gamdl>=2.9.1,<=3.0`. Consumers pass this to
/// `pip install --upgrade {spec}` so the resolver can pick the
/// newest validated release without jumping to an untested major.
#[must_use]
pub fn pip_version_spec() -> String {
    let window = support_window();
    format!(
        "gamdl>={minimum},<={maximum}",
        minimum = window.minimum,
        maximum = window.maximum_tested,
    )
}

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

    /// `--wrapper-m3u8-ip` CLI flag and `wrapper_m3u8_ip` INI key.
    ///
    /// Introduced in v3.1. When `--use-wrapper` is set, v3.1+ fetches the
    /// HLS master playlist URL from a TCP socket on this address instead
    /// of the Apple Music API response. Older releases do not recognise
    /// the flag; emitting it would either be dropped silently
    /// (GAMDL v3.0's `cleanup_unknown_params()`) or — at the CLI layer —
    /// cause a click argument-parse error. Gate all emission on this
    /// feature to stay self-consistent with the detected CLI.
    WrapperM3u8Ip,

    /// `--no-exceptions` CLI flag has an observable effect.
    ///
    /// Present in every release, but v3.1 (`dc6f2e8`, "Use
    /// ExceptionPrettyPrinter and .exception logging") removed every
    /// consumer of the flag — `cli.py` no longer calls
    /// `traceback.print_exc()` and unconditionally routes exceptions
    /// through structlog's `ExceptionPrettyPrinter`. The flag is still
    /// accepted by the CLI parser but has no effect on output.
    ///
    /// Returns `true` for v2.x and v3.0 only. MeedyaDL continues to set
    /// the field on `GamdlOptions`; the actual CLI emission is gated by
    /// this capability in `to_cli_args()`.
    NoExceptionsFlag,
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
            // Added in v3.1.
            Self::WrapperM3u8Ip => is_version_at_least(version, "3.1"),
            // No-op starting v3.1 — flag is accepted but ignored.
            Self::NoExceptionsFlag => !is_version_at_least(version, "3.1"),
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
    fn no_exceptions_flag_is_effective_below_v31() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(Some("2.9.3".to_string()));
        assert!(supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.0".to_string()));
        assert!(supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.0.5".to_string()));
        assert!(supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.1".to_string()));
        assert!(!supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.2.0".to_string()));
        assert!(!supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(None);
    }

    #[test]
    fn wrapper_m3u8_ip_requires_v31() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(Some("3.0".to_string()));
        assert!(!supports(GamdlFeature::WrapperM3u8Ip));
        set_detected_version(Some("3.0.1".to_string()));
        assert!(!supports(GamdlFeature::WrapperM3u8Ip));
        set_detected_version(Some("3.1".to_string()));
        assert!(supports(GamdlFeature::WrapperM3u8Ip));
        set_detected_version(Some("3.1.2".to_string()));
        assert!(supports(GamdlFeature::WrapperM3u8Ip));
        set_detected_version(None);
    }

    #[test]
    fn unknown_version_reports_no_capabilities() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(None);
        assert!(!supports(GamdlFeature::FetchExtraTags));
        assert!(!supports(GamdlFeature::NativeCodecPriority));
        assert!(!supports(GamdlFeature::WrapperM3u8Ip));
        assert!(!supports(GamdlFeature::NoExceptionsFlag));
    }

    #[test]
    fn set_detected_version_roundtrip() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(Some("2.9.3".to_string()));
        assert_eq!(detected_version(), Some("2.9.3".to_string()));
        set_detected_version(None);
        assert_eq!(detected_version(), None);
    }

    // ----------------------------------------------------------------
    // Support window
    // ----------------------------------------------------------------

    #[test]
    fn support_window_parses() {
        // Fails loudly if tool-versions.toml's [gamdl] section is
        // malformed or missing — the parse is lazy, so this test is
        // what catches regressions in the TOML.
        let window = support_window();
        assert!(!window.minimum.is_empty());
        assert!(!window.maximum_tested.is_empty());
        assert!(!window.recommended.is_empty());
    }

    #[test]
    fn support_window_has_recommended_inside_range() {
        // Defensive: `recommended` must be between `minimum` and
        // `maximum_tested`, otherwise the installer could pin to a
        // version we've explicitly declared out-of-range.
        let window = support_window();
        assert!(
            is_version_at_least(&window.recommended, &window.minimum),
            "recommended ({}) must be >= minimum ({})",
            window.recommended,
            window.minimum
        );
        assert!(
            is_version_at_least(&window.maximum_tested, &window.recommended),
            "recommended ({}) must be <= maximum_tested ({})",
            window.recommended,
            window.maximum_tested
        );
    }

    #[test]
    fn classify_not_installed() {
        assert_eq!(classify(None), VersionSupport::NotInstalled);
    }

    #[test]
    fn classify_below_minimum_is_unsupported() {
        let result = classify(Some("2.8.4"));
        match result {
            VersionSupport::Unsupported { installed, minimum } => {
                assert_eq!(installed, "2.8.4");
                assert_eq!(minimum, support_window().minimum);
            }
            other => panic!("Expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn classify_inside_window_is_supported() {
        let supported = classify(Some(&support_window().minimum));
        assert!(
            supported.is_supported(),
            "Exact minimum must be supported, got {supported:?}"
        );

        let at_ceiling = classify(Some(&support_window().maximum_tested));
        assert!(
            at_ceiling.is_supported(),
            "Exact ceiling must be supported, got {at_ceiling:?}"
        );
    }

    #[test]
    fn classify_above_ceiling_is_untested() {
        // Bump the installed major past whatever ceiling we ship with,
        // so the test stays green as we bump `maximum_tested`.
        let result = classify(Some("99.0.0"));
        match result {
            VersionSupport::Untested {
                installed,
                maximum_tested,
                recommended,
            } => {
                assert_eq!(installed, "99.0.0");
                assert_eq!(maximum_tested, support_window().maximum_tested);
                assert_eq!(recommended, support_window().recommended);
            }
            other => panic!("Expected Untested, got {other:?}"),
        }
    }

    #[test]
    fn should_offer_upgrade_inside_window() {
        // Latest available equals our ceiling → offer.
        assert!(should_offer_upgrade(&support_window().maximum_tested));
        // Latest available equals our floor → still inside the window
        // (user might be on an older patch), offer.
        assert!(should_offer_upgrade(&support_window().minimum));
    }

    #[test]
    fn should_not_offer_upgrade_above_ceiling() {
        // Upstream shipped something newer than we've validated —
        // don't push users into it.
        assert!(!should_offer_upgrade("99.0.0"));
    }

    #[test]
    fn should_not_offer_upgrade_for_unparseable_version() {
        // Without the semver guard, garbage strings would coerce to
        // (0, 0, 0) and pass the ceiling check. Reject them instead.
        assert!(!should_offer_upgrade("invalid"));
        assert!(!should_offer_upgrade(""));
        assert!(!should_offer_upgrade("v3-rc"));
    }

    #[test]
    fn pip_version_spec_bounds_the_range() {
        let spec = pip_version_spec();
        let window = support_window();
        assert!(spec.starts_with("gamdl>="));
        assert!(spec.contains(&format!(">={}", window.minimum)));
        assert!(spec.contains(&format!("<={}", window.maximum_tested)));
    }
}
