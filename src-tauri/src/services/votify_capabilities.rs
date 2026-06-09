// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See the LICENSE file in the project
// root for full licence text.
//
//! Version-aware capability flags for the installed `votify` CLI.
//!
//! Mirrors the [`super::gamdl_capabilities`] pattern. Votify is a pip-
//! installed Python CLI from the same author as GAMDL and follows the
//! same evolutionary cadence — CLI flags appear, get renamed, or get
//! removed across releases. Rather than pinning MeedyaDL to a single
//! votify line, we detect the installed version at runtime and only
//! emit flags / INI keys the installed release actually understands.
//!
//! # PR M9-1 status
//!
//! This file lands as the **scaffolding only** for the multi-PR Spotify
//! integration EPIC (#101). The support window + version cache + classify
//! + pip spec functions are fully wired so they can be consumed by:
//!
//!   * the startup version probe in `commands::dependencies` (lands in
//!     this PR alongside this file),
//!   * the future `votify_service::install_votify()` flow (M9-2), and
//!   * the future Spotify download dispatch (M9-2 / M9-4).
//!
//! The [`VotifyFeature`] enum currently carries a single placeholder
//! variant ([`VotifyFeature::Placeholder`]). Real version-conditional
//! feature gates land in PR M9-2 alongside the votify CLI flag audit
//! that maps each `VotifyOptions` field to a CLI flag — at that point
//! the placeholder is replaced with concrete variants for any flag
//! whose availability has changed across the support window
//! (`1.9.0..=1.9.9`). The placeholder pattern matches GAMDL's early
//! capability rollout (#604 lifecycle).
//!
//! # Threading model
//!
//! Same shape as `gamdl_capabilities`: the detected version lives behind
//! a process-global [`RwLock`]. Writes happen on startup (dependency
//! probe), reinstall, or in tests. Reads happen on every download. The
//! `RwLock` keeps the read path lock-free in practice.
//!
//! # Unknown-version defaults
//!
//! When the version has not yet been probed, every capability query
//! returns `false`. Safest default: never emit options that a future
//! reader of the CLI might reject. Once PR M9-2's audit populates real
//! feature gates, this rule continues to apply.

use std::sync::{LazyLock, RwLock};

use serde::Deserialize;

// ============================================================
// Support window (compiled from tool-versions.toml)
// ============================================================

/// Raw TOML body of `src-tauri/tool-versions.toml`, embedded at compile
/// time. Mirrors the pattern used by `gamdl_capabilities`.
const TOOL_VERSIONS_TOML: &str = include_str!("../../tool-versions.toml");

/// Deserialisation shape for the `[votify]` section of
/// `tool-versions.toml`. Kept private — public API is
/// [`VotifySupportWindow`].
#[derive(Debug, Deserialize)]
struct VotifySupportToml {
    minimum_version: String,
    maximum_tested_version: String,
    recommended_version: String,
}

#[derive(Debug, Deserialize)]
struct ToolVersionsToml {
    votify: VotifySupportToml,
}

/// The validated votify version range baked into this MeedyaDL build.
///
/// Same shape and semantics as
/// [`super::gamdl_capabilities::GamdlSupportWindow`]:
///
/// * `minimum` — oldest release MeedyaDL still supports. Below this
///   threshold, features silently degrade.
/// * `maximum_tested` — highest release we have actually exercised in
///   CI / manual testing. Above this threshold we surface an "Untested"
///   badge but don't block.
/// * `recommended` — what the installer resolves to by default. Always
///   within `[minimum, maximum_tested]`.
#[derive(Debug, Clone)]
pub struct VotifySupportWindow {
    pub minimum: String,
    pub maximum_tested: String,
    pub recommended: String,
}

/// Parses the embedded `tool-versions.toml` once and caches the result.
/// A parse error is a programmer error — the TOML ships in our repo
/// and is covered by the `support_window_parses` test — so we unwrap
/// with a descriptive panic rather than surfacing an `Option`.
static SUPPORT_WINDOW: LazyLock<VotifySupportWindow> = LazyLock::new(|| {
    let parsed: ToolVersionsToml = toml::from_str(TOOL_VERSIONS_TOML)
        .expect("tool-versions.toml must contain a valid [votify] section");
    VotifySupportWindow {
        minimum: parsed.votify.minimum_version,
        maximum_tested: parsed.votify.maximum_tested_version,
        recommended: parsed.votify.recommended_version,
    }
});

/// Returns the support window compiled into this MeedyaDL build.
#[must_use]
pub fn support_window() -> &'static VotifySupportWindow {
    &SUPPORT_WINDOW
}

/// Classification of the installed votify version relative to the
/// support window.
///
/// Mirrors [`super::gamdl_capabilities::VersionSupport`]. Each variant
/// carries the context the UI needs to render a helpful message — we
/// avoid returning a bare `bool` because callers need to display
/// specific numbers ("votify 2.0.0 is newer than the tested 1.9.9
/// ceiling"), not yes/no.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionSupport {
    /// Votify is not installed at all (e.g., setup wizard hasn't run,
    /// or pip is broken).
    NotInstalled,
    /// Installed version is below `minimum`. Some MeedyaDL features
    /// will silently degrade. Surface a prominent warning.
    Unsupported { installed: String, minimum: String },
    /// Installed version is inside `[minimum, maximum_tested]`. Normal
    /// operation.
    Supported { installed: String },
    /// Installed version is above `maximum_tested`. MeedyaDL hasn't
    /// validated this release — proceed but warn, and offer a downgrade
    /// to `recommended`.
    Untested {
        installed: String,
        maximum_tested: String,
        recommended: String,
    },
}

impl VersionSupport {
    /// Is the installed version inside the supported range?
    #[must_use]
    pub const fn is_supported(&self) -> bool {
        matches!(self, Self::Supported { .. })
    }
}

/// Classifies `installed` against the compiled support window.
///
/// A `None` installed version means votify is not installed; we
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

/// Should the "votify update available" notice be shown to the user?
///
/// Returns `false` only when `latest_available` is not a parseable
/// semver string. Above-ceiling versions are still surfaced so users
/// learn about new votify releases as soon as upstream ships them —
/// the "tested vs untested" distinction is communicated via
/// [`is_above_tested_ceiling`] and the frontend's amber "Untested"
/// badge. Mirrors [`super::gamdl_capabilities::should_offer_upgrade`].
#[must_use]
pub fn should_offer_upgrade(latest_available: &str) -> bool {
    is_parseable_semver(latest_available)
}

/// Is `version` strictly above this MeedyaDL build's
/// `maximum_tested_version`? Mirrors
/// [`super::gamdl_capabilities::is_above_tested_ceiling`].
#[must_use]
pub fn is_above_tested_ceiling(version: &str) -> bool {
    if !is_parseable_semver(version) {
        return false;
    }
    let window = support_window();
    !is_version_at_least(&window.maximum_tested, version)
}

/// Returns `true` if `version` is a leading-numeric semver-ish string.
fn is_parseable_semver(version: &str) -> bool {
    let mut parts = version.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    first.parse::<u32>().is_ok()
}

/// Hand-rolled version comparison. Returns `true` iff `version >= minimum`
/// per a relaxed semver order.
///
/// Mirrors the helper imported from `gamdl_service` for the GAMDL
/// capability module. Kept private to this module instead of shared
/// because votify and GAMDL might one day want different relaxations
/// (e.g. pre-release suffix handling) and the helper is 8 lines.
///
/// Splits on `.`, parses each part as `u32`, substitutes `0` for
/// missing or unparseable parts. Tolerant of two-part inputs ("1.9")
/// and unparseable strings (`"unknown"` → `(0, 0, 0)`, fails most
/// comparisons safely).
fn is_version_at_least(version: &str, minimum: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let mut parts = v.split('.');
        let part = |it: &mut std::str::Split<char>| -> u32 {
            it.next().and_then(|p| p.parse::<u32>().ok()).unwrap_or(0)
        };
        (part(&mut parts), part(&mut parts), part(&mut parts))
    };
    parse(version) >= parse(minimum)
}

/// Pip version specifier string for `pip install --upgrade`.
///
/// Example: `votify>=1.9.0,<=1.9.9`. Mirrors
/// [`super::gamdl_capabilities::pip_version_spec`].
#[must_use]
pub fn pip_version_spec() -> String {
    let window = support_window();
    format!(
        "votify>={minimum},<={maximum}",
        minimum = window.minimum,
        maximum = window.maximum_tested,
    )
}

/// Pip version specifier pinning votify to a single explicit version.
///
/// Used when the user has consciously opted into installing an
/// **above-ceiling, untested** votify release. Mirrors
/// [`super::gamdl_capabilities::pip_target_spec`].
#[must_use]
pub fn pip_target_spec(target: &str) -> String {
    format!("votify=={target}")
}

/// Process-global cache of the last detected votify version string.
///
/// Populated by [`set_detected_version`] from:
///
/// * `commands::dependencies::get_component_versions` at startup
///   (lands in this PR alongside this file).
/// * Future `votify_service::install_votify` after a successful
///   `pip install --upgrade votify` (PR M9-2).
/// * Future `votify_service::get_votify_version` whenever the version
///   is probed (first Spotify download attempt, settings page refresh,
///   etc. — also PR M9-2).
///
/// `None` means "we haven't probed yet or votify isn't installed". See
/// the module-level docs for how unknown versions are handled.
static DETECTED_VERSION: LazyLock<RwLock<Option<String>>> =
    LazyLock::new(|| RwLock::new(None));

/// Records the currently installed votify version.
///
/// Passing `None` explicitly clears the cache (used when votify is
/// uninstalled or the probe fails). Poisoned locks are recovered
/// transparently — a panicking reader should never brick the whole
/// capability subsystem.
pub fn set_detected_version(version: Option<String>) {
    let mut guard = match DETECTED_VERSION.write() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = version;
}

/// Returns the currently cached votify version string, if any.
#[must_use]
pub fn detected_version() -> Option<String> {
    DETECTED_VERSION
        .read()
        .ok()
        .and_then(|guard| guard.clone())
}

/// Individual CLI / config features whose availability differs across
/// votify releases.
///
/// **PR M9-1 placeholder.** Real variants land in PR M9-2 when the
/// votify CLI audit produces a per-flag version table. The placeholder
/// pattern matches GAMDL's early capability rollout (#604 lifecycle)
/// where the enum existed but only one or two real variants were
/// declared until the upstream audit caught up.
///
/// To add a real feature gate:
///
/// 1. Add a variant here documenting which votify version(s) carry the
///    feature.
/// 2. Add the matching branch to [`VotifyFeature::is_available_on`]
///    using [`is_version_at_least`].
/// 3. Add the variant to [`active_capabilities_summary`]'s table.
/// 4. Add a unit test pinning the version → feature mapping.
///
/// (Same workflow as adding a [`super::gamdl_capabilities::GamdlFeature`]
/// variant — see e.g. #605 `WrapperM3u8Ip` for the canonical PR shape.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VotifyFeature {
    /// Placeholder variant kept so the enum compiles before PR M9-2
    /// lands real version-conditional gates.
    ///
    /// `is_available_on` returns `true` for any version that parses as
    /// `>= minimum_version`. This is a deliberate no-op — it makes the
    /// enum and the supporting [`supports`] / [`active_capabilities_summary`]
    /// scaffolding exercise-able by the M9-1 tests without committing
    /// to specific votify version semantics that haven't been audited
    /// yet.
    ///
    /// Will be removed when the first real gate lands in M9-2.
    Placeholder,
}

impl VotifyFeature {
    /// Returns `true` when `version` is known to support this feature.
    ///
    /// Version comparison uses [`is_version_at_least`] which tolerates
    /// two-part ("1.9") and unparseable version strings gracefully.
    fn is_available_on(self, version: &str) -> bool {
        match self {
            Self::Placeholder => {
                let window = support_window();
                is_version_at_least(version, &window.minimum)
            }
        }
    }
}

/// Reports whether the currently installed votify release supports
/// `feature`.
///
/// Returns `false` when the version has not been detected yet — see
/// the "Unknown-version defaults" section in the module docs for
/// rationale. Mirrors [`super::gamdl_capabilities::supports`].
#[must_use]
pub fn supports(feature: VotifyFeature) -> bool {
    match detected_version() {
        Some(ver) => feature.is_available_on(&ver),
        None => false,
    }
}

/// Compact comma-separated list of capability flags active on the
/// currently installed votify release.
///
/// Used by the per-download activity-log line so users (and crash
/// reports) can see at a glance which feature gates were active when
/// an item ran. Returns `"unknown"` when the version cache hasn't
/// been populated yet, mirroring [`supports`]'s safe default.
///
/// While only [`VotifyFeature::Placeholder`] exists, the output is
/// either `"placeholder"` (version known + within support window) or
/// `"(none)"` (version known but below `minimum`). PR M9-2 grows the
/// list as real gates land.
#[must_use]
pub fn active_capabilities_summary() -> String {
    let Some(ver) = detected_version() else {
        return "unknown".to_string();
    };

    let all = [(VotifyFeature::Placeholder, "placeholder")];

    let active: Vec<&str> = all
        .iter()
        .filter_map(|(feat, name)| feat.is_available_on(&ver).then_some(*name))
        .collect();

    if active.is_empty() {
        "(none)".to_string()
    } else {
        active.join(", ")
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Process-global mutex serialising tests that mutate the detected-
    /// version cache. Mirrors the pattern used by gamdl_capabilities so
    /// tests across modules don't race each other on the shared cache.
    static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    #[test]
    fn support_window_parses() {
        let window = support_window();
        // Trivial sanity: the embedded TOML produced something non-empty
        // matching the expected shape. The actual values are tested in
        // `support_window_has_recommended_inside_range` below.
        assert!(!window.minimum.is_empty(), "minimum should parse");
        assert!(!window.maximum_tested.is_empty(), "maximum_tested should parse");
        assert!(!window.recommended.is_empty(), "recommended should parse");
    }

    #[test]
    fn support_window_has_recommended_inside_range() {
        let window = support_window();
        assert!(
            is_version_at_least(&window.recommended, &window.minimum),
            "recommended ({}) must be >= minimum ({})",
            window.recommended,
            window.minimum,
        );
        assert!(
            is_version_at_least(&window.maximum_tested, &window.recommended),
            "recommended ({}) must be <= maximum_tested ({})",
            window.recommended,
            window.maximum_tested,
        );
    }

    #[test]
    fn classify_none_returns_not_installed() {
        let _lock = test_lock();
        assert_eq!(classify(None), VersionSupport::NotInstalled);
    }

    #[test]
    fn classify_below_minimum_returns_unsupported() {
        let _lock = test_lock();
        let window = support_window();
        let result = classify(Some("0.1.0"));
        assert!(matches!(result, VersionSupport::Unsupported { ref minimum, .. } if minimum == &window.minimum));
    }

    #[test]
    fn classify_within_window_returns_supported() {
        let _lock = test_lock();
        let window = support_window();
        // Use the minimum itself — boundary case that should always pass.
        let result = classify(Some(&window.minimum));
        assert!(result.is_supported(), "minimum ({}) should be supported, got {result:?}", window.minimum);
    }

    #[test]
    fn classify_above_ceiling_returns_untested() {
        let _lock = test_lock();
        // 999.0.0 is way above any plausible upstream votify release.
        let result = classify(Some("999.0.0"));
        assert!(matches!(result, VersionSupport::Untested { .. }));
    }

    #[test]
    fn is_above_tested_ceiling_true_for_above() {
        assert!(is_above_tested_ceiling("999.0.0"));
    }

    #[test]
    fn is_above_tested_ceiling_false_for_within_window() {
        let window = support_window();
        assert!(!is_above_tested_ceiling(&window.maximum_tested));
        assert!(!is_above_tested_ceiling(&window.minimum));
    }

    #[test]
    fn is_above_tested_ceiling_false_for_unparseable() {
        // Garbage strings can't be reasoned about — preserve pre-#101
        // pass-through behaviour by returning false.
        assert!(!is_above_tested_ceiling("not-a-version"));
        assert!(!is_above_tested_ceiling(""));
    }

    #[test]
    fn should_offer_upgrade_accepts_parseable() {
        assert!(should_offer_upgrade("1.9.9"));
        assert!(should_offer_upgrade("2.0"));
        assert!(should_offer_upgrade("3"));
    }

    #[test]
    fn should_offer_upgrade_rejects_garbage() {
        assert!(!should_offer_upgrade("not-a-version"));
        assert!(!should_offer_upgrade(""));
        assert!(!should_offer_upgrade("v1.9"));  // leading "v" breaks the first.parse::<u32>() guard
    }

    #[test]
    fn pip_version_spec_contains_bounds() {
        let spec = pip_version_spec();
        let window = support_window();
        assert!(spec.contains(&format!(">={}", window.minimum)), "spec '{spec}' should bound below");
        assert!(spec.contains(&format!("<={}", window.maximum_tested)), "spec '{spec}' should bound above");
        assert!(spec.starts_with("votify"));
    }

    #[test]
    fn pip_target_spec_pins_exact_version() {
        assert_eq!(pip_target_spec("1.9.5"), "votify==1.9.5");
    }

    #[test]
    fn detected_version_round_trips() {
        let _lock = test_lock();
        set_detected_version(Some("1.9.5".to_string()));
        assert_eq!(detected_version(), Some("1.9.5".to_string()));
        set_detected_version(None);
        assert_eq!(detected_version(), None);
    }

    #[test]
    fn supports_returns_false_when_version_unknown() {
        let _lock = test_lock();
        set_detected_version(None);
        assert!(!supports(VotifyFeature::Placeholder));
    }

    #[test]
    fn supports_placeholder_when_version_within_window() {
        let _lock = test_lock();
        let window = support_window();
        set_detected_version(Some(window.minimum.clone()));
        assert!(supports(VotifyFeature::Placeholder));
    }

    #[test]
    fn supports_placeholder_false_below_minimum() {
        let _lock = test_lock();
        set_detected_version(Some("0.1.0".to_string()));
        assert!(!supports(VotifyFeature::Placeholder));
    }

    #[test]
    fn active_capabilities_summary_unknown_when_no_version() {
        let _lock = test_lock();
        set_detected_version(None);
        assert_eq!(active_capabilities_summary(), "unknown");
    }

    #[test]
    fn active_capabilities_summary_lists_active() {
        let _lock = test_lock();
        let window = support_window();
        set_detected_version(Some(window.maximum_tested.clone()));
        let summary = active_capabilities_summary();
        // Placeholder is the only declared feature in M9-1 and is active for
        // any version in the support window.
        assert!(summary.contains("placeholder"), "summary '{summary}' should include placeholder");
    }

    #[test]
    fn active_capabilities_summary_none_when_below_minimum() {
        let _lock = test_lock();
        set_detected_version(Some("0.1.0".to_string()));
        assert_eq!(active_capabilities_summary(), "(none)");
    }

    #[test]
    fn is_version_at_least_basic_orderings() {
        assert!(is_version_at_least("1.9.9", "1.9.0"));
        assert!(is_version_at_least("1.9.0", "1.9.0"));  // equality
        assert!(is_version_at_least("2.0.0", "1.9.9"));
        assert!(!is_version_at_least("1.8.9", "1.9.0"));
    }

    #[test]
    fn is_version_at_least_tolerates_two_parts() {
        assert!(is_version_at_least("1.9", "1.9.0"));
        assert!(is_version_at_least("1.9", "1.9"));
    }

    #[test]
    fn is_version_at_least_unparseable_collapses_to_zero() {
        // Garbage strings parse to (0, 0, 0) — they fail most real
        // comparisons safely.
        assert!(!is_version_at_least("not-a-version", "1.0.0"));
        assert!(is_version_at_least("not-a-version", "0.0.0"));
    }

    #[test]
    fn placeholder_feature_aware_of_minimum() {
        // The placeholder consults the support window's minimum. Any future
        // edit to `minimum_version` in tool-versions.toml should still leave
        // this passing — but if someone changes the placeholder semantics
        // without updating the test, the regression surfaces here.
        let window = support_window();
        assert!(VotifyFeature::Placeholder.is_available_on(&window.minimum));
        assert!(!VotifyFeature::Placeholder.is_available_on("0.0.1"));
    }
}
