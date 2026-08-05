// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See the LICENSE file in the project
// root for full licence text.
//
//! Version-aware capability flags for the installed `GAMDL` CLI.
//!
//! GAMDL has been evolving rapidly, and some CLI options / config.ini keys
//! are only available on a subset of supported releases. Two concrete cases
//! motivate this module:
//!
//! * `--wrapper-m3u8-ip` / `wrapper_m3u8_ip` was **added in v3.1** and
//!   **removed again in v3.6** (superseded by the single-URL wrapper-v2
//!   `--wrapper-url`). Passing it to a release outside `[3.1, 3.5.x]`
//!   causes Click to reject the CLI invocation with `no such option`.
//! * `--database-path` / `database_path` and `--playlist-folder-template` /
//!   `playlist_folder_template` were **added in v3.0**. Passing either to
//!   a pre-3.0 release causes the same kind of Click error.
//!
//! (Historical note: a third motivating case, `--fetch-extra-tags` /
//! `fetch_extra_tags`, was present in every v2.x release and removed in
//! v3.0. It was the original reason this module exists, but the plumbing
//! for it was removed in #1000 once GAMDL v2 support itself was dropped
//! — the gate had gone permanently inert inside the v3-only support
//! window. See git history for the pre-#1000 shape if resurrecting a
//! similar version-scoped flag gate.)
//!
//! Rather than pinning `MeedyaDL` to a single GAMDL line, we detect the
//! installed version at runtime and only emit flags / INI keys the
//! installed release actually understands. `MeedyaDL` supports the GAMDL
//! `v3.x` line only (`>= 3.0`; v2 support was dropped 2026-07-03) — split
//! across two wrapper generations (v3.0–v3.5.x wrapper-v1, v3.6+
//! wrapper-v2). Every capability gate stays version-range-aware rather
//! than a simple "is v3+?" check because behaviour still varies WITHIN
//! the v3 line (e.g. `--no-exceptions` effective on <3.1 and >=3.8 but a
//! no-op between; native muxing / wrapper-v2 from 3.6; assets-API
//! non-web-codec unlock from 3.8). Some gates keyed on the old `2.9.1`
//! floor (native codec priority, classical-host rewrite, storefront INI
//! strip) are now always-true inside the window but keep their exact
//! version-math predicates — still correct and unknown-version-safe.
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
//! downside is that users who haven't completed setup won't get a
//! version-gated flag like `wrapper_m3u8_ip` until the version is
//! probed — which happens on the very next dependency check or
//! download attempt.

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
    /// Optional `[gamdl.platform_ceilings]` sub-table (#1014): per-platform
    /// overrides of `maximum_tested_version`, keyed by the same canonical
    /// platform IDs [`current_platform_id`] returns (e.g. `"linux-armv7"`).
    /// Absent for every platform that tracks the global ceiling — only
    /// listed when a platform genuinely can't install what the rest of
    /// the fleet can (e.g. GAMDL 3.8.2+ ships no Linux ARMv7 wheel, so
    /// that platform's *effective* ceiling trails the global one).
    /// `#[serde(default)]` keeps the table fully optional so an older or
    /// hand-edited `tool-versions.toml` without it still parses.
    #[serde(default)]
    platform_ceilings: std::collections::HashMap<String, String>,
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
/// * `platform_ceilings` — per-platform overrides of `maximum_tested`
///   (#1014). See [`effective_maximum_tested`] / [`classify_for_platform`].
#[derive(Debug, Clone)]
pub struct GamdlSupportWindow {
    pub minimum: String,
    pub maximum_tested: String,
    pub recommended: String,
    pub platform_ceilings: std::collections::HashMap<String, String>,
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
        platform_ceilings: parsed.gamdl.platform_ceilings,
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

// ============================================================
// Per-platform ceiling overrides (#1014)
// ============================================================
//
// `support_window()` above is intentionally global — one ceiling for
// every platform this build ships. That is the right default: bumping
// `maximum_tested_version` is meant to lift the ceiling for everyone at
// once. It breaks down only when a GAMDL release ships a compiled
// extension that a platform genuinely has no wheel for (Linux ARMv7 as
// of GAMDL 3.8.2+, per `update_checker::wheel_platform_tags()`'s own
// per-platform PyPI wheel probe) — on that one platform, reporting the
// global ceiling as "Supported" is misleading: the version is only
// reachable there by building the compiled extension from source,
// which MeedyaDL's managed Python environment can't do.
//
// The functions below layer a narrow, additive per-platform override on
// top of the existing global window WITHOUT changing `classify()` or
// `support_window()` themselves, so every existing caller and test
// keeps its current behaviour untouched. A platform with no entry in
// `[gamdl.platform_ceilings]` — which is every platform except Linux
// ARMv7 today — sees byte-identical results from the platform-aware
// functions as from the plain ones.

/// Returns a canonical platform identifier used to key
/// `[gamdl.platform_ceilings]` overrides in `tool-versions.toml`.
///
/// Mirrors the OS/arch dispatch in
/// `update_checker::wheel_platform_tags()` (same `cfg!()` conditions),
/// but produces stable dash-joined IDs instead of PyPI wheel-tag
/// substrings — these are our own config keys, not filename fragments.
#[must_use]
pub fn current_platform_id() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            "windows-aarch64"
        } else {
            "windows-x86_64"
        }
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "aarch64") {
            "linux-aarch64"
        } else if cfg!(target_arch = "arm") {
            "linux-armv7"
        } else {
            "linux-x86_64"
        }
    } else {
        "unknown"
    }
}

/// Returns the effective `maximum_tested_version` for `platform_id`
/// (#1014): the platform-specific override from
/// `[gamdl.platform_ceilings]` when one exists, otherwise this build's
/// global `maximum_tested_version`.
///
/// For every platform without an override (all of them today except
/// Linux ARMv7) this returns exactly `support_window().maximum_tested`.
#[must_use]
pub fn effective_maximum_tested(platform_id: &str) -> String {
    support_window()
        .platform_ceilings
        .get(platform_id)
        .cloned()
        .unwrap_or_else(|| support_window().maximum_tested.clone())
}

/// Platform-aware counterpart to [`classify`] (#1014): classifies
/// `installed` using the effective ceiling for `platform_id` — i.e.
/// [`effective_maximum_tested`] — instead of the global one.
///
/// Identical to `classify(installed)` for any `platform_id` without a
/// `[gamdl.platform_ceilings]` entry (every platform except Linux
/// ARMv7 today). On Linux ARMv7, a version above the ARMv7-specific
/// ceiling (but still within the global window) correctly classifies
/// as [`VersionSupport::Untested`] instead of `Supported` — the global
/// ceiling reflects what most platforms can install, not what ARMv7
/// can.
#[must_use]
pub fn classify_for_platform(installed: Option<&str>, platform_id: &str) -> VersionSupport {
    let Some(installed) = installed else {
        return VersionSupport::NotInstalled;
    };
    let window = support_window();
    let maximum_tested = effective_maximum_tested(platform_id);

    if !is_version_at_least(installed, &window.minimum) {
        return VersionSupport::Unsupported {
            installed: installed.to_string(),
            minimum: window.minimum.clone(),
        };
    }

    if !is_version_at_least(&maximum_tested, installed) {
        return VersionSupport::Untested {
            installed: installed.to_string(),
            maximum_tested,
            recommended: window.recommended.clone(),
        };
    }

    VersionSupport::Supported {
        installed: installed.to_string(),
    }
}

/// Should the "GAMDL update available" notice be shown to the user?
///
/// Returns `false` only when `latest_available` is not a parseable semver
/// string (e.g. `invalid`, `v3-rc`, empty). Above-ceiling versions are
/// **still surfaced** so users learn about new GAMDL releases as soon as
/// upstream ships them — the "tested vs untested" distinction is
/// communicated separately via [`is_above_tested_ceiling`] and the
/// frontend's amber "Untested" badge.
///
/// # History
///
/// This previously hard-capped at `maximum_tested_version`, which silently
/// hid every upstream release until MeedyaDL bumped its support window.
/// In practice that meant the Updates page kept saying "All components up
/// to date" while a new GAMDL release sat unnoticed on PyPI for days,
/// blocking us from validating the new build's compatibility against real
/// downloads. Surfacing the version (with a warning badge) is the
/// MeedyaDL-side fix; the install path now also handles untested targets
/// via [`pip_target_spec`].
#[must_use]
pub fn should_offer_upgrade(latest_available: &str) -> bool {
    is_parseable_semver(latest_available)
}

/// Is `version` strictly above this MeedyaDL build's
/// `maximum_tested_version`?
///
/// Used by [`crate::services::update_checker`] to set the `is_untested`
/// flag on a `ComponentUpdate`, which in turn drives the frontend's
/// "Untested" warning badge. `false` for unparseable strings — we can't
/// reason about them, so we don't claim they're untested.
#[must_use]
pub fn is_above_tested_ceiling(version: &str) -> bool {
    if !is_parseable_semver(version) {
        return false;
    }
    let window = support_window();
    // `is_version_at_least(a, b)` ⇔ `a >= b`. Above-ceiling means
    // `version > maximum_tested`, i.e. `!(maximum_tested >= version)`.
    !is_version_at_least(&window.maximum_tested, version)
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
/// Example output: `gamdl>=3.0,<=3.8.1`. Consumers pass this to
/// `pip install --upgrade {spec}` so the resolver can pick the
/// newest validated release without jumping to an untested version
/// (and never resolves down to a dropped v2 release).
#[must_use]
pub fn pip_version_spec() -> String {
    let window = support_window();
    format!(
        "gamdl>={minimum},<={maximum}",
        minimum = window.minimum,
        maximum = window.maximum_tested,
    )
}

/// Pip version specifier pinning GAMDL to a single explicit version.
///
/// Used when the user has consciously opted into installing an
/// **above-ceiling, untested** GAMDL release from the Updates page (the
/// amber "Untested" badge route). The bounded [`pip_version_spec`] would
/// silently downgrade their click on "Upgrade to v3.4" into "install
/// v3.3" — confusing UX and not what the user asked for. This helper
/// pins to exactly the version the frontend showed in the banner.
///
/// `target` should be a parseable semver. We don't sanity-check it here
/// because the caller (`install_gamdl`) has already obtained it from
/// `check_latest_gamdl_version` (PyPI) and surfaced it to the user.
#[must_use]
pub fn pip_target_spec(target: &str) -> String {
    format!("gamdl=={target}")
}

// ============================================================
// Wrapper-aware v2 → v3 upgrade target (#1001)
// ============================================================
//
// A user still running GAMDL v2.x on a v2-support-dropped MeedyaDL
// build (`classify(installed) == Unsupported`) needs a guided upgrade.
// The right target depends on whether they run the wrapper: GAMDL v3.6
// switched the wrapper protocol from v1 (three local sockets) to v2 (a
// single HTTP daemon requiring a manual Docker + Apple `.so`-extraction
// setup). Auto-jumping a wrapper-v1 user straight to this build's fully
// tested `recommended` version (currently on the wrapper-v2 line) would
// silently break their working wrapper. `recommended_upgrade_target`
// encodes the safe target for each case; see #1001 for the full
// migration-flow design (the surrounding modal/notice UI is a separate,
// not-yet-decided follow-up — this function is the backend primitive).

/// Last GAMDL release still on the wrapper-v1 protocol (three local
/// sockets: `--wrapper-account-url` HTTP, `--wrapper-m3u8-ip` TCP,
/// `--wrapper-decrypt-ip` TCP). GAMDL v3.6 replaced wrapper-v1 with
/// wrapper-v2 (`GamdlFeature::WrapperUrl`), a single HTTP daemon that
/// requires a manual Docker + Apple `.so`-extraction setup — not a
/// drop-in replacement for a user who already has wrapper-v1 running.
pub const LAST_WRAPPER_V1_VERSION: &str = "3.5.2";

/// Returns the recommended upgrade target for a user currently on
/// `installed`, taking their wrapper usage into account (#1001).
///
/// | `installed` state         | `use_wrapper` | Target                              |
/// |----------------------------|---------------|--------------------------------------|
/// | below this build's floor (v2.x) | `true`  | [`LAST_WRAPPER_V1_VERSION`] ("3.5.2") — the newest release that doesn't require migrating to wrapper-v2 |
/// | below this build's floor (v2.x) | `false` | `support_window().recommended` — no wrapper to protect, so the best-tested release |
/// | already `>=` this build's floor (v3.x+), or `None` (nothing installed yet) | either | `support_window().recommended` — the v2→v3 wrapper-protocol concern doesn't apply once already on v3, or when there's no prior install to protect |
///
/// This function only *encodes* the target table above — it does not
/// itself decide whether an upgrade is warranted. Callers should gate
/// on `classify(installed) == VersionSupport::Unsupported` (i.e.
/// `installed` is still on the pre-floor v2.x line) before consulting
/// it for the v2→v3 migration flow; called with an already-v3+
/// `installed`, it safely degrades to the ordinary `recommended`
/// target rather than ever recommending a downgrade.
#[must_use]
pub fn recommended_upgrade_target(installed: Option<&str>, use_wrapper: bool) -> &'static str {
    // `None` (nothing installed yet, e.g. a fresh setup) is NOT the
    // "still on v2.x" case — there's no existing wrapper-v1 setup to
    // protect, so it falls straight through to `recommended` just like
    // an already-v3+ install. Only an explicitly known pre-floor (v2.x)
    // version triggers the wrapper-v1-preserving branch.
    let is_pre_floor_v2 =
        installed.is_some_and(|v| !is_version_at_least(v, &support_window().minimum));

    if use_wrapper && is_pre_floor_v2 {
        LAST_WRAPPER_V1_VERSION
    } else {
        support_window().recommended.as_str()
    }
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

    /// `--playlist-folder-template` CLI flag and `playlist_folder_template`
    /// INI key.
    ///
    /// Introduced in GAMDL v3.0. v2.9.x accepts the other folder templates
    /// (`--album-folder-template`, `--compilation-folder-template`,
    /// `--no-album-folder-template`) but does NOT recognise
    /// `--playlist-folder-template` — passing it crashes Click with
    /// "no such option". Emission must therefore be gated the same way
    /// [`Self::WrapperM3u8Ip`] is.
    ///
    /// On v2.9.x the flag's absence just means GAMDL falls back to its
    /// own built-in default (`"Playlists/{playlist_artist}"`), so skipping
    /// the emission is safe — the user may just not see the custom
    /// template they configured. MeedyaDL's Settings UI greys the input
    /// with a tooltip referencing this fact when the detected version is
    /// below 3.0.
    PlaylistFolderTemplate,

    /// `--no-exceptions` CLI flag has an observable effect.
    ///
    /// Three-era history (mirrors [`Self::FFmpegPath`]):
    ///
    /// - `< 3.1` — **effective** (original era). Upstream `cli.py`
    ///   honours the flag on its trace-suppression path.
    /// - `3.1 .. 3.7.4` — **no-op**. Upstream commit `dc6f2e8`
    ///   ("Use ExceptionPrettyPrinter and .exception logging")
    ///   removed every consumer of the flag; the CLI parser accepts
    ///   it, but `structlog`'s `ExceptionPrettyPrinter` is added to
    ///   the processor list unconditionally so tracebacks always
    ///   surface regardless.
    /// - `>= 3.8` — **effective again**. Upstream commit `58f4548`
    ///   ("Respect no exceptions option") gates the
    ///   `ExceptionPrettyPrinter` on `not config.no_exceptions`,
    ///   restoring the suppression behaviour to what it was on the
    ///   pre-3.1 code path.
    ///
    /// So the predicate is `true` on either the pre-3.1 or the >=3.8
    /// eras, `false` on the 3.1..3.7.4 no-op window. Same three-era
    /// shape as [`Self::FFmpegPath`] which was removed in v3.6 then
    /// reinstated in v3.7.
    ///
    /// MeedyaDL continues to set the field on `GamdlOptions` on
    /// every version so the value survives the capability-cache
    /// warm-up window; the actual CLI emission (via `to_cli_args()`)
    /// and the pre-emission dance in `download_queue::merge_options`
    /// are both gated by this capability. Downstream
    /// `is_python_traceback_noise` (#660) suppresses the console
    /// noise regardless of the flag, so the visible effect for users
    /// is mostly a shorter, cleaner activity log on 3.8+.
    NoExceptionsFlag,

    // ---------------------------------------------------------------------
    // GAMDL v3.6 capability gates (#853)
    // ---------------------------------------------------------------------
    /// `--wrapper-url` CLI flag and `wrapper_url` INI key (wrapper-v2).
    ///
    /// Introduced in v3.6. Replaces the three v1 wrapper sockets
    /// ([`Self::WrapperM3u8Ip`], `--wrapper-account-url`, `--wrapper-decrypt-ip`)
    /// with a single HTTP base URL pointing at the
    /// [wrapper-v2 daemon](https://github.com/glomatico/wrapper-v2)'s
    /// HTTP API (`/health`, `/me`, `/playback`, `/decrypt`, `/login`).
    /// Default value in upstream: `http://127.0.0.1` (port 80 from the
    /// shipped `compose.yaml`).
    ///
    /// MeedyaDL emits exactly one of the v1 or v2 wrapper flag families
    /// per CLI invocation, gated on this feature.
    WrapperUrl,

    /// `aac-legacy` / `aac-he-legacy` codec identifiers RENAMED to
    /// `aac-web` / `aac-he-web` in v3.6 (`gamdl/interface/enums.py`).
    ///
    /// The underlying codec is identical — just the on-the-wire string
    /// changed. Both `SongCodec::AacLegacy` and `SongCodec::AacHeLegacy`
    /// Rust enum variants are kept (settings file backwards-compat); the
    /// CLI / INI serialisation site consults this capability to pick
    /// `aac-legacy` (<3.6) or `aac-web` (>=3.6).
    ///
    /// `LEGACY_SONG_CODECS = {"aac-legacy", "aac-he-legacy"}` constant
    /// also removed; replaced with `SONG_CODEC_FLAVOR_MAP` + new
    /// `SongCodec.is_web` / `is_cenc` / `flavor` properties on the
    /// Python side.
    AacWebCodecRename,

    /// `--ffmpeg-path` / `--mp4box-path` / `--mp4decrypt-path` CLI
    /// options REMOVED in v3.6 alongside native muxing + decryption
    /// for music videos (upstream "Dropped FFmpeg, MP4Box and
    /// mp4decrypt with native muxing and decryption for music videos").
    ///
    /// Songs already used native pipelines pre-3.6; music videos were
    /// the last consumer of these external tools. On 3.6+ MeedyaDL
    /// must NOT pass any of the three path options (would crash Click
    /// with "no such option") and must NOT include the corresponding
    /// INI keys.
    ///
    /// The tools are still required for MeedyaDL's own pipeline
    /// (FFmpeg for ReplayGain / BPM analysis; MP4Box / mp4decrypt only
    /// previously needed by GAMDL itself) — see [`tool-versions.toml`]
    /// for which we still ship.
    NativeMuxing,

    /// `--music-video-remux-mode` CLI option REMOVED in v3.6
    /// (collateral damage from native muxing — there's only one
    /// remux strategy now).
    ///
    /// MeedyaDL's `GamdlOptions::music_video_remux_mode` field is kept
    /// for backwards-compat with saved queues / settings; the CLI arg
    /// builder gates emission behind this capability.
    MusicVideoRemuxMode,

    // ---------------------------------------------------------------------
    // GAMDL v3.7 capability gates (#867)
    // ---------------------------------------------------------------------
    /// `--ffmpeg-path` CLI option + `ffmpeg_path` INI key.
    ///
    /// v3.6 removed all three tool-path CLI options (`--ffmpeg-path`,
    /// `--mp4box-path`, `--mp4decrypt-path`) when GAMDL switched to native
    /// muxing + decryption for music videos (covered by
    /// [`Self::NativeMuxing`]). v3.7 **reinstated `--ffmpeg-path`**
    /// because N_m3u8DL-RE depends on FFmpeg at HLS download time; the
    /// other two stay removed. See `.github/audits/gamdl-v3.7-audit.md`
    /// for the full upstream commit chain (`92b8220c` + `bd59bb7c`).
    ///
    /// Three-version classification:
    /// * `< 3.6`: ✓ (original tool-path era — emitted)
    /// * `3.6.x`: ✗ (native muxing era — all three suppressed)
    /// * `>= 3.7`: ✓ (FFmpeg path back; mp4box / mp4decrypt still gone)
    ///
    /// In other words: `true` when ANY of `<3.6` OR `>=3.7` — only `false`
    /// on the `3.6` line. The `Self::NativeMuxing` gate is now used ONLY
    /// for the two STILL-removed tool paths (`--mp4box-path`,
    /// `--mp4decrypt-path`).
    FFmpegPath,

    /// GAMDL's URL regex (`gamdl/.../constants.py::VALID_URL_PATTERN`)
    /// accepts `https://(?:classical\.)?music.apple.com/...` but rejects
    /// the bare legacy `https://classical.apple.com/...` form. Apple
    /// Music Classical originally lived at the bare host before migrating
    /// to `classical.music.apple.com`; the legacy form is still in the
    /// wild from older builds + bookmarks. MeedyaDL's frontend URL parser
    /// accepts both, but when this capability is `true` the legacy form
    /// must be rewritten to `classical.music.apple.com` before being
    /// handed to GAMDL or the subprocess immediately exits with
    /// "Could not parse URL" (#880).
    ///
    /// Three-version classification:
    /// * `< 2.9.1`: `false` (regex was even stricter — `r"https://music\.apple\.com"`,
    ///   no classical prefix accepted at all; rewriting wouldn't help, the
    ///   URL still fails). MeedyaDL doesn't support pre-2.9.1 so this branch
    ///   is theoretical — pre-#880 unconditional-no-op behaviour is preserved.
    /// * `>= 2.9.1`: `true` (regex relaxed to `r"https://(?:classical\.)?music\.apple\.com"`
    ///   — the classical.music.apple.com host is accepted but the bare
    ///   classical.apple.com is not).
    ///
    /// Effective for the entire MeedyaDL support window. The gate exists
    /// so the unknown-version default (`false`) preserves the pre-#880
    /// pass-through behaviour rather than producing a rewrite that might
    /// be wrong on an unaudited future GAMDL.
    ClassicalMusicHostRequired,

    /// GAMDL strips unknown INI keys from its `config.ini` via
    /// `gamdl/cli/config_file.py::cleanup_unknown_params()` on every load.
    /// `storefront` is NOT in GAMDL's `CliConfig` on any release in our
    /// support window (2.9.1+) — the storefront is derived from the URL
    /// path itself (`/us/album/...` → "us") by the URL regex, not from
    /// the INI. MeedyaDL has been writing `storefront = us` into the INI
    /// for legacy reasons; the value is silently discarded on read.
    ///
    /// When this capability is `true` MeedyaDL omits the dead INI write
    /// to keep the config tidy + avoid forcing GAMDL's cleanup to do
    /// unnecessary work (#881).
    ///
    /// Three-version classification:
    /// * `>= 2.9.1`: `true` (every supported release strips `storefront`)
    /// * unknown / out-of-window: `false` — MeedyaDL preserves the
    ///   pre-#881 behaviour of emitting the key, on the principle that
    ///   we never know what an unaudited GAMDL is keying off. If a
    ///   future GAMDL re-adds storefront as a real CLI/INI option we
    ///   adjust the gate then.
    StorefrontIniKeyStripped,

    /// `--wrapper-decrypt-host` / `--wrapper-decrypt-port` CLI flags and
    /// `wrapper_decrypt_host` / `wrapper_decrypt_port` INI keys.
    ///
    /// Introduced in GAMDL v3.8.2. Wrapper-v2 decryption moved from an
    /// HTTP `POST /decrypt` call riding `--wrapper-url` to a native TCP
    /// protocol on a **separate** host/port, splitting the combined
    /// wrapper-v1 `--wrapper-decrypt-ip` (`host:port` string) into two
    /// discrete options. GAMDL 3.8.2 also hard-requires wrapper-v2
    /// `0.0.2` — it exact-matches the `version` field returned by
    /// `GET /me` at CLI startup and aborts on any mismatch.
    ///
    /// This capability applies only inside the wrapper-v2
    /// ([`Self::WrapperUrl`]) family — wrapper-v1 keeps using the
    /// combined `--wrapper-decrypt-ip` flag unconditionally. Emitting
    /// `--wrapper-decrypt-host` / `--wrapper-decrypt-port` on a
    /// wrapper-v2 release older than 3.8.2 (3.6 .. 3.8.1) would crash
    /// Click with "no such option" — the flags didn't exist yet.
    WrapperDecryptHostPort,

    // ---------------------------------------------------------------------
    // GAMDL v3.8 capability gates (#962, #963, #1002)
    // ---------------------------------------------------------------------
    /// Upstream commit [`a7d141b7`](https://github.com/glomatico/gamdl/commit/a7d141b7)
    /// (GAMDL v3.8) added a new `POST /v1/play/assets` HLS endpoint that
    /// unlocked every non-web `SongCodec` **except ALAC** for wrapper-less
    /// downloads (aac, aac-he, aac-binaural, aac-downmix, aac-he-binaural,
    /// aac-he-downmix, atmos, ac3). Companion commit
    /// [`4d2988b3`](https://github.com/glomatico/gamdl/commit/4d2988b3)
    /// narrowed GAMDL's own CLI startup warning + README wording to say
    /// only ALAC still needs wrapper — confirming the API behaviour.
    ///
    /// Consumed by [`crate::models::gamdl_options::SongCodec::is_wrapper_dependent_runtime`]
    /// so `download_queue::build_gapfill_priority_chain()` no longer
    /// pre-emptively strips Atmos/AC3 out of a wrapper-less gap-fill retry
    /// chain on 3.8+, where they now succeed. Below 3.8 (and on an
    /// unprobed / unknown version) the conservative, version-agnostic
    /// `SongCodec::is_wrapper_dependent()` predicate still applies —
    /// `Atmos` and `Ac3` are treated as wrapper-dependent.
    ///
    /// Deliberately does **not** change `SongCodec::display_name()` — the
    /// `(Experimental)` labels stay unconditional across every GAMDL
    /// version per the maintainer decision on #965. Version-aware prose
    /// belongs in the frontend, driven by the `assets_api_unlocks_lossy_codecs`
    /// field on the `GamdlCapabilities` DTO (`commands::dependencies`) via
    /// the `useGamdlCapabilities` hook — not in the codec label itself.
    AssetsApiUnlocksLossyCodecs,
}

impl GamdlFeature {
    /// Returns `true` when `version` is known to support this feature.
    ///
    /// Version comparison uses [`is_version_at_least`] which tolerates
    /// two-part ("2.9") and unparseable version strings gracefully.
    fn is_available_on(self, version: &str) -> bool {
        match self {
            // Added in v2.9.1.
            Self::NativeCodecPriority => is_version_at_least(version, "2.9.1"),
            // Added in v3.1, REMOVED in v3.6 (replaced by wrapper-v2 single URL).
            Self::WrapperM3u8Ip => {
                is_version_at_least(version, "3.1") && !is_version_at_least(version, "3.6")
            }
            // Added in v3.0 — v2.9.x rejects it at CLI parse time.
            Self::PlaylistFolderTemplate => is_version_at_least(version, "3.0"),
            // Three-era predicate — see the [`NoExceptionsFlag`]
            // variant's doc comment for the full history. Effective
            // on either the pre-3.1 era (original) or the >= 3.8 era
            // (reinstated by upstream `58f4548`); no-op on the
            // 3.1..3.7.4 window because `structlog`'s
            // `ExceptionPrettyPrinter` was in the processor list
            // unconditionally. Same shape as `FFmpegPath` above.
            Self::NoExceptionsFlag => {
                !is_version_at_least(version, "3.1") || is_version_at_least(version, "3.8")
            }
            // GAMDL v3.6 family (#853):
            // Added in v3.6 — single HTTP URL replacing the three wrapper-v1 sockets.
            Self::WrapperUrl => is_version_at_least(version, "3.6"),
            // Codec identifier rename in v3.6 (aac-legacy → aac-web).
            Self::AacWebCodecRename => is_version_at_least(version, "3.6"),
            // Native muxing in v3.6 — no external FFmpeg/MP4Box/mp4decrypt path
            // options accepted by the CLI.
            Self::NativeMuxing => is_version_at_least(version, "3.6"),
            // --music-video-remux-mode REMOVED in v3.6.
            // Returns true when the option is STILL available (i.e. on <3.6).
            Self::MusicVideoRemuxMode => !is_version_at_least(version, "3.6"),
            // GAMDL v3.7 family (#867):
            // --ffmpeg-path was REMOVED in v3.6 (with mp4box-path /
            // mp4decrypt-path) then REINSTATED in v3.7 because N_m3u8DL-RE
            // depends on FFmpeg. The other two stay removed. So the gate
            // is true when EITHER < 3.6 (original tool-path era) OR >= 3.7
            // (FFmpeg-only reinstatement era). Only false on the 3.6.x line.
            Self::FFmpegPath => {
                !is_version_at_least(version, "3.6") || is_version_at_least(version, "3.7")
            }
            // GAMDL >= 2.9.1 (#880): classical.apple.com URLs must be
            // rewritten to classical.music.apple.com before being handed
            // to GAMDL. Effective on the entire support window.
            Self::ClassicalMusicHostRequired => is_version_at_least(version, "2.9.1"),
            // GAMDL >= 2.9.1 (#881): `storefront` INI key is stripped on
            // every release via `cleanup_unknown_params()`. Effective on
            // the entire support window.
            Self::StorefrontIniKeyStripped => is_version_at_least(version, "2.9.1"),
            // Added in v3.8.2 — wrapper-v2 decrypt moved from HTTP
            // (riding `--wrapper-url`) to a separate TCP host/port.
            Self::WrapperDecryptHostPort => is_version_at_least(version, "3.8.2"),
            // GAMDL v3.8 family (#962, #963, #1002):
            // `/v1/play/assets` unlocked every non-web codec except ALAC
            // for wrapper-less downloads.
            Self::AssetsApiUnlocksLossyCodecs => is_version_at_least(version, "3.8"),
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

/// Compact comma-separated list of capability flags active on the
/// currently installed GAMDL release (e.g.
/// `"native_codec_priority, wrapper_m3u8_ip, no_exceptions_flag"`).
///
/// Used by the per-download activity-log line so users (and crash
/// reports) can see at a glance which feature gates were active when an
/// item ran. Returns `"unknown"` when the version cache hasn't been
/// populated yet, mirroring [`supports`]'s safe default.
#[must_use]
pub fn active_capabilities_summary() -> String {
    let Some(ver) = detected_version() else {
        return "unknown".to_string();
    };

    let all = [
        (GamdlFeature::NativeCodecPriority, "native_codec_priority"),
        (GamdlFeature::PlaylistFolderTemplate, "playlist_folder_template"),
        (GamdlFeature::WrapperM3u8Ip, "wrapper_m3u8_ip"),
        (GamdlFeature::NoExceptionsFlag, "no_exceptions_flag"),
        (GamdlFeature::WrapperUrl, "wrapper_url"),
        (GamdlFeature::AacWebCodecRename, "aac_web_codec_rename"),
        (GamdlFeature::NativeMuxing, "native_muxing"),
        (GamdlFeature::MusicVideoRemuxMode, "music_video_remux_mode"),
        (GamdlFeature::FFmpegPath, "ffmpeg_path"),
        (GamdlFeature::ClassicalMusicHostRequired, "classical_music_host_required"),
        (GamdlFeature::StorefrontIniKeyStripped, "storefront_ini_key_stripped"),
        (GamdlFeature::WrapperDecryptHostPort, "wrapper_decrypt_host_port"),
        (
            GamdlFeature::AssetsApiUnlocksLossyCodecs,
            "assets_api_unlocks_lossy_codecs",
        ),
    ];

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

/// Process-global mutex that tests across multiple modules share to
/// serialise mutation of [`set_detected_version`]. Without it,
/// parallel tests in different files (e.g.,
/// `services::gamdl_capabilities::tests`, `models::gamdl_options::tests`,
/// `services::config_service::tests`, `services::apple_music_api::tests`)
/// can flip the cache between another test's set + read and cause
/// intermittent assertion failures.
///
/// All test modules that touch the capability cache MUST acquire
/// this lock before calling `set_detected_version` or any
/// `supports(...)` that depends on the version. Tests within
/// `gamdl_capabilities::tests` itself also use this same lock.
///
/// Public-but-gated so production code can't accidentally hold it.
#[cfg(test)]
pub fn capability_cache_test_lock(
) -> std::sync::MutexGuard<'static, ()> {
    static SHARED: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SHARED.lock().unwrap_or_else(|p| p.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience wrapper around [`capability_cache_test_lock`].
    /// Kept under the same `TEST_LOCK` name so the existing tests in
    /// this module need no renaming.
    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        capability_cache_test_lock()
    }

    #[test]
    fn native_codec_priority_requires_v291() {
        let _lock = test_lock();
        set_detected_version(Some("2.9.0".to_string()));
        assert!(!supports(GamdlFeature::NativeCodecPriority));
        set_detected_version(Some("2.9.1".to_string()));
        assert!(supports(GamdlFeature::NativeCodecPriority));
        set_detected_version(Some("3.0".to_string()));
        assert!(supports(GamdlFeature::NativeCodecPriority));
        set_detected_version(None);
    }

    #[test]
    fn no_exceptions_flag_three_era_predicate() {
        // Three-era predicate: effective on < 3.1 (original era) and
        // >= 3.8 (upstream `58f4548` reinstated), no-op on 3.1..3.7.4.
        // Same shape as the `FFmpegPath` gate (removed in v3.6,
        // reinstated in v3.7). See the variant's doc comment for
        // the full incident history.
        let _lock = test_lock();

        // Era 1 — effective (< 3.1).
        set_detected_version(Some("2.9.3".to_string()));
        assert!(supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.0".to_string()));
        assert!(supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.0.5".to_string()));
        assert!(supports(GamdlFeature::NoExceptionsFlag));

        // Era 2 — no-op (3.1..3.7.4). Upstream removed every consumer;
        // MeedyaDL must NOT emit the flag on this range or the
        // spawned command line lies about intent.
        set_detected_version(Some("3.1".to_string()));
        assert!(!supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.2.0".to_string()));
        assert!(!supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.5.2".to_string()));
        assert!(!supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.7.4".to_string()));
        assert!(!supports(GamdlFeature::NoExceptionsFlag));

        // Era 3 — effective again (>= 3.8). Upstream `58f4548` gated
        // `ExceptionPrettyPrinter` on `not config.no_exceptions`, so
        // emitting the flag once more suppresses tracebacks.
        set_detected_version(Some("3.8".to_string()));
        assert!(supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.8.1".to_string()));
        assert!(supports(GamdlFeature::NoExceptionsFlag));
        set_detected_version(Some("3.9.0".to_string()));
        assert!(supports(GamdlFeature::NoExceptionsFlag));

        set_detected_version(None);
    }

    #[test]
    fn wrapper_m3u8_ip_requires_v31() {
        let _lock = test_lock();
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
        let _lock = test_lock();
        set_detected_version(None);
        assert!(!supports(GamdlFeature::NativeCodecPriority));
        assert!(!supports(GamdlFeature::WrapperM3u8Ip));
        assert!(!supports(GamdlFeature::PlaylistFolderTemplate));
        assert!(!supports(GamdlFeature::NoExceptionsFlag));
    }

    #[test]
    fn playlist_folder_template_requires_v30() {
        let _lock = test_lock();
        set_detected_version(Some("2.9.1".to_string()));
        assert!(!supports(GamdlFeature::PlaylistFolderTemplate));
        set_detected_version(Some("2.9.3".to_string()));
        assert!(!supports(GamdlFeature::PlaylistFolderTemplate));
        set_detected_version(Some("3.0".to_string()));
        assert!(supports(GamdlFeature::PlaylistFolderTemplate));
        set_detected_version(Some("3.0.1".to_string()));
        assert!(supports(GamdlFeature::PlaylistFolderTemplate));
        set_detected_version(Some("3.2".to_string()));
        assert!(supports(GamdlFeature::PlaylistFolderTemplate));
        set_detected_version(None);
    }

    #[test]
    fn active_capabilities_summary_lists_v3_5_features() {
        let _lock = test_lock();
        set_detected_version(Some("3.5.0".to_string()));
        let summary = active_capabilities_summary();
        // v3.5 supports: NativeCodecPriority, PlaylistFolderTemplate,
        // WrapperM3u8Ip. Does NOT support NoExceptionsFlag (no-op on
        // the 3.1..3.7.4 window).
        assert!(summary.contains("native_codec_priority"));
        assert!(summary.contains("playlist_folder_template"));
        assert!(summary.contains("wrapper_m3u8_ip"));
        assert!(!summary.contains("no_exceptions_flag"));
        set_detected_version(None);
    }

    #[test]
    fn active_capabilities_summary_lists_v3_8_features() {
        // v3.8 reinstated `NoExceptionsFlag` (upstream `58f4548`) — the
        // summary output must reflect it so operators reading the
        // startup log can confirm the flag is being emitted again.
        let _lock = test_lock();
        set_detected_version(Some("3.8".to_string()));
        let summary = active_capabilities_summary();
        assert!(summary.contains("native_codec_priority"));
        assert!(summary.contains("playlist_folder_template"));
        assert!(summary.contains("no_exceptions_flag"));
        // v3.8's new /v1/play/assets endpoint unlocked every non-web
        // codec except ALAC for wrapper-less downloads (#963).
        assert!(summary.contains("assets_api_unlocks_lossy_codecs"));
        set_detected_version(None);
    }

    #[test]
    fn active_capabilities_summary_unknown_when_uncached() {
        let _lock = test_lock();
        set_detected_version(None);
        assert_eq!(active_capabilities_summary(), "unknown");
    }

    #[test]
    fn active_capabilities_summary_lists_v2x_features() {
        let _lock = test_lock();
        set_detected_version(Some("2.9.3".to_string()));
        let summary = active_capabilities_summary();
        // v2.9.3 supports: NativeCodecPriority, NoExceptionsFlag. Does
        // NOT support PlaylistFolderTemplate (added in 3.0) or
        // WrapperM3u8Ip (added in 3.1). (FetchExtraTags plumbing was
        // removed in #1000 once GAMDL v2 support itself was dropped —
        // this test still exercises a hypothetical pre-3.0 version
        // string since `is_version_at_least`/the version-math gates
        // remain unconditionally correct outside the support window.)
        assert!(summary.contains("native_codec_priority"));
        assert!(summary.contains("no_exceptions_flag"));
        assert!(!summary.contains("playlist_folder_template"));
        assert!(!summary.contains("wrapper_m3u8_ip"));
        set_detected_version(None);
    }

    #[test]
    fn set_detected_version_roundtrip() {
        let _lock = test_lock();
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
    fn should_offer_upgrade_above_ceiling() {
        // Above-ceiling versions ARE now surfaced to the user (with an
        // "Untested" warning badge). Hiding them silently caused real
        // upgrades to sit unnoticed on PyPI for days. The is_above_tested_ceiling
        // helper communicates the warning state separately.
        assert!(should_offer_upgrade("99.0.0"));
    }

    #[test]
    fn should_not_offer_upgrade_for_unparseable_version() {
        // Without the semver guard, garbage strings would coerce to
        // (0, 0, 0) and pass downstream is_newer comparisons. Reject
        // them instead — we won't surface a version we can't reason about.
        assert!(!should_offer_upgrade("invalid"));
        assert!(!should_offer_upgrade(""));
        assert!(!should_offer_upgrade("v3-rc"));
    }

    #[test]
    fn is_above_tested_ceiling_flags_future_versions() {
        // Above the ceiling → flagged.
        assert!(is_above_tested_ceiling("99.0.0"));
        // At the ceiling → not flagged (it's the highest tested).
        assert!(!is_above_tested_ceiling(&support_window().maximum_tested));
        // Below the ceiling → not flagged.
        assert!(!is_above_tested_ceiling(&support_window().minimum));
        // Unparseable → not flagged (we can't reason about it).
        assert!(!is_above_tested_ceiling("garbage"));
        assert!(!is_above_tested_ceiling(""));
    }

    #[test]
    fn pip_version_spec_bounds_the_range() {
        let spec = pip_version_spec();
        let window = support_window();
        assert!(spec.starts_with("gamdl>="));
        assert!(spec.contains(&format!(">={}", window.minimum)));
        assert!(spec.contains(&format!("<={}", window.maximum_tested)));
    }

    #[test]
    fn pip_target_spec_pins_exact_version() {
        // Explicit-target installs (untested upgrade flow) must pin to
        // an exact version so pip can't silently resolve to something
        // else under the user's `gamdl>=…,<=…` cap.
        assert_eq!(pip_target_spec("3.3"), "gamdl==3.3");
        assert_eq!(pip_target_spec("4.0.1"), "gamdl==4.0.1");
    }

    // ----------------------------------------------------------------
    // Wrapper-aware v2 → v3 upgrade target (#1001)
    // ----------------------------------------------------------------

    #[test]
    fn last_wrapper_v1_version_predates_wrapper_v2_threshold() {
        // wrapper-v2 landed in GAMDL 3.6 (`GamdlFeature::WrapperUrl`).
        // The recommended v2.x-with-wrapper migration target must stay
        // strictly below that threshold, or the whole point of the
        // table (don't break a working wrapper-v1 setup) is defeated.
        assert!(!is_version_at_least(LAST_WRAPPER_V1_VERSION, "3.6"));
    }

    #[test]
    fn recommended_upgrade_target_v2_with_wrapper_stays_on_wrapper_v1() {
        // A v2.x user running the wrapper must be offered the last
        // wrapper-v1 release, not this build's fully-tested
        // (wrapper-v2) `recommended` version.
        let target = recommended_upgrade_target(Some("2.9.3"), true);
        assert_eq!(target, LAST_WRAPPER_V1_VERSION);
    }

    #[test]
    fn recommended_upgrade_target_v2_without_wrapper_gets_recommended() {
        // No wrapper to protect — offer the best-tested release.
        let target = recommended_upgrade_target(Some("2.9.3"), false);
        assert_eq!(target, support_window().recommended);
    }

    #[test]
    fn recommended_upgrade_target_already_v3_ignores_wrapper_flag() {
        // Once already on v3.x (>= this build's floor), the v2->v3
        // wrapper-protocol migration concern doesn't apply — both
        // branches degrade to `recommended` regardless of `use_wrapper`.
        let installed = support_window().minimum.clone();
        assert_eq!(
            recommended_upgrade_target(Some(&installed), true),
            support_window().recommended
        );
        assert_eq!(
            recommended_upgrade_target(Some(&installed), false),
            support_window().recommended
        );
    }

    #[test]
    fn recommended_upgrade_target_none_installed_gets_recommended() {
        // No installed version at all (fresh setup) — nothing to
        // protect, offer the best-tested release regardless of the
        // wrapper toggle's current setting.
        assert_eq!(
            recommended_upgrade_target(None, true),
            support_window().recommended
        );
        assert_eq!(
            recommended_upgrade_target(None, false),
            support_window().recommended
        );
    }

    // ----------------------------------------------------------------
    // Per-platform ceiling overrides (#1014)
    // ----------------------------------------------------------------

    #[test]
    fn current_platform_id_is_a_known_value() {
        // Pure smoke test: whatever this build target is, the ID must
        // be one of the canonical set (or "unknown" for an
        // unrecognised OS) — never empty.
        let id = current_platform_id();
        assert!(!id.is_empty());
        assert!([
            "macos",
            "windows-x86_64",
            "windows-aarch64",
            "linux-x86_64",
            "linux-aarch64",
            "linux-armv7",
            "unknown",
        ]
        .contains(&id));
    }

    #[test]
    fn effective_maximum_tested_falls_back_without_override() {
        // No platform in the *shipped* tool-versions.toml is expected to
        // have an override except "linux-armv7" (as of #1014's initial
        // ARMv7 entry) — every other platform ID must see the global
        // ceiling unchanged.
        for platform_id in ["macos", "windows-x86_64", "windows-aarch64", "linux-x86_64", "linux-aarch64"] {
            assert_eq!(
                effective_maximum_tested(platform_id),
                support_window().maximum_tested,
                "{platform_id}: expected no override, got a different effective ceiling"
            );
        }
    }

    #[test]
    fn effective_maximum_tested_uses_armv7_override_when_present() {
        // This test is intentionally tolerant of the override being
        // absent (e.g. a future edit removes it once upstream ships an
        // ARMv7 wheel) — it only asserts the override, when present, is
        // actually honoured rather than silently ignored.
        let window = support_window();
        if let Some(armv7_ceiling) = window.platform_ceilings.get("linux-armv7") {
            assert_eq!(&effective_maximum_tested("linux-armv7"), armv7_ceiling);
            // And it must differ from (be below) the global ceiling —
            // otherwise the override is a no-op and shouldn't exist.
            assert_ne!(armv7_ceiling, &window.maximum_tested);
        }
    }

    #[test]
    fn classify_for_platform_matches_classify_without_override() {
        // For a platform with no override, classify_for_platform must
        // be byte-identical to the plain classify() — this is the
        // "zero risk for every other platform" invariant #1014 relies
        // on.
        for v in [
            "2.8.0",
            &support_window().minimum,
            &support_window().maximum_tested,
            "99.0.0",
        ] {
            assert_eq!(
                classify_for_platform(Some(v), "linux-x86_64"),
                classify(Some(v)),
                "linux-x86_64 (no override) must match classify() for {v}"
            );
        }
        assert_eq!(
            classify_for_platform(None, "linux-x86_64"),
            classify(None)
        );
    }

    #[test]
    fn classify_for_platform_armv7_respects_override_when_present() {
        let window = support_window();
        let Some(armv7_ceiling) = window.platform_ceilings.get("linux-armv7").cloned() else {
            // No override configured — nothing to assert (see the
            // tolerant comment on `effective_maximum_tested_uses_armv7_override_when_present`).
            return;
        };

        // Exactly at the ARMv7 ceiling: Supported.
        let at_ceiling = classify_for_platform(Some(&armv7_ceiling), "linux-armv7");
        assert!(
            at_ceiling.is_supported(),
            "ARMv7 at its own ceiling ({armv7_ceiling}) must be Supported, got {at_ceiling:?}"
        );

        // Above the ARMv7 ceiling but still within (or at) the global
        // ceiling: Untested on ARMv7 specifically, even though the same
        // version is Supported globally.
        let global_ceiling = window.maximum_tested.clone();
        if is_version_at_least(&global_ceiling, &armv7_ceiling)
            && global_ceiling != armv7_ceiling
        {
            let armv7_result = classify_for_platform(Some(&global_ceiling), "linux-armv7");
            assert!(
                matches!(armv7_result, VersionSupport::Untested { .. }),
                "global ceiling ({global_ceiling}) exceeds the ARMv7 ceiling \
                 ({armv7_ceiling}) so ARMv7 must classify it Untested, got {armv7_result:?}"
            );
            assert!(
                classify(Some(&global_ceiling)).is_supported(),
                "the same version must still be Supported on the global (non-ARMv7) window"
            );
        }
    }

    // -- GAMDL v3.6 capability gates (#853) -------------------------------

    #[test]
    fn wrapper_url_requires_v36() {
        let _lock = test_lock();
        for v in ["2.9.3", "3.0", "3.3", "3.5.2"] {
            set_detected_version(Some(v.to_string()));
            assert!(
                !supports(GamdlFeature::WrapperUrl),
                "WrapperUrl must NOT be available on {v} (wrapper-v1 only)"
            );
        }
        for v in ["3.6", "3.6.0", "3.6.1", "4.0"] {
            set_detected_version(Some(v.to_string()));
            assert!(
                supports(GamdlFeature::WrapperUrl),
                "WrapperUrl must be available on {v}"
            );
        }
        set_detected_version(None);
    }

    #[test]
    fn wrapper_m3u8_ip_removed_in_v36() {
        let _lock = test_lock();
        // The old wrapper-v1 m3u8 IP flag was added in v3.1 but REMOVED
        // in v3.6 alongside the wrapper-v2 single-endpoint redesign.
        set_detected_version(Some("3.5.2".to_string()));
        assert!(supports(GamdlFeature::WrapperM3u8Ip));
        set_detected_version(Some("3.6".to_string()));
        assert!(!supports(GamdlFeature::WrapperM3u8Ip));
        set_detected_version(Some("3.6.0".to_string()));
        assert!(!supports(GamdlFeature::WrapperM3u8Ip));
        set_detected_version(None);
    }

    #[test]
    fn aac_web_codec_rename_requires_v36() {
        let _lock = test_lock();
        set_detected_version(Some("3.5.2".to_string()));
        assert!(
            !supports(GamdlFeature::AacWebCodecRename),
            "on v3.5.2 we must emit 'aac-legacy' / 'aac-he-legacy'"
        );
        set_detected_version(Some("3.6".to_string()));
        assert!(
            supports(GamdlFeature::AacWebCodecRename),
            "on v3.6+ we must emit 'aac-web' / 'aac-he-web'"
        );
        set_detected_version(None);
    }

    #[test]
    fn native_muxing_requires_v36() {
        let _lock = test_lock();
        // <3.6: external FFmpeg/MP4Box/mp4decrypt path options still
        // accepted by the CLI parser; we emit them.
        set_detected_version(Some("3.5.2".to_string()));
        assert!(!supports(GamdlFeature::NativeMuxing));
        // >=3.6: options removed; we must NOT emit them.
        set_detected_version(Some("3.6".to_string()));
        assert!(supports(GamdlFeature::NativeMuxing));
        set_detected_version(None);
    }

    #[test]
    fn music_video_remux_mode_removed_in_v36() {
        let _lock = test_lock();
        // Returns true when the option is STILL AVAILABLE.
        set_detected_version(Some("3.5.2".to_string()));
        assert!(supports(GamdlFeature::MusicVideoRemuxMode));
        set_detected_version(Some("3.6".to_string()));
        assert!(!supports(GamdlFeature::MusicVideoRemuxMode));
        set_detected_version(None);
    }

    /// `--ffmpeg-path` has a UNIQUE three-version life: present on <3.6,
    /// REMOVED on 3.6 alongside the other tool-paths, REINSTATED on 3.7
    /// because N_m3u8DL-RE depends on FFmpeg. So the gate is true on
    /// either side of the 3.6 valley — only false on the 3.6.x line.
    ///
    /// Pre-release v3.7.1 (currently on upstream `main`) inherits >=3.7
    /// behaviour cleanly because `is_version_at_least("3.7.1", "3.7")`
    /// is true.
    #[test]
    fn ffmpeg_path_gate_three_version_classification() {
        let _lock = test_lock();

        // Era 1: <3.6 (original tool-path era — flag accepted)
        set_detected_version(Some("2.9.3".to_string()));
        assert!(
            supports(GamdlFeature::FFmpegPath),
            "<3.6: --ffmpeg-path is accepted, must emit"
        );
        set_detected_version(Some("3.5.2".to_string()));
        assert!(
            supports(GamdlFeature::FFmpegPath),
            "3.5.2: --ffmpeg-path is accepted, must emit"
        );

        // Era 2: 3.6.x (native muxing era — flag REMOVED, would crash Click)
        set_detected_version(Some("3.6".to_string()));
        assert!(
            !supports(GamdlFeature::FFmpegPath),
            "3.6: --ffmpeg-path was REMOVED, must suppress"
        );
        set_detected_version(Some("3.6.5".to_string()));
        assert!(
            !supports(GamdlFeature::FFmpegPath),
            "3.6.5: still on 3.6 line, --ffmpeg-path still removed"
        );

        // Era 3: >=3.7 (REINSTATED — N_m3u8DL-RE needs FFmpeg)
        set_detected_version(Some("3.7".to_string()));
        assert!(
            supports(GamdlFeature::FFmpegPath),
            "3.7: --ffmpeg-path REINSTATED, must emit again"
        );
        // v3.7.1 prep is already on upstream main; we'll admit it to the
        // support window after release, but the gate logic must already
        // handle it transparently.
        set_detected_version(Some("3.7.1".to_string()));
        assert!(
            supports(GamdlFeature::FFmpegPath),
            "3.7.1: still on the >=3.7 line, --ffmpeg-path still emitted"
        );

        set_detected_version(None);
    }

    /// `ClassicalMusicHostRequired` returns `true` on every supported
    /// GAMDL version (>= 2.9.1) so MeedyaDL rewrites legacy
    /// `classical.apple.com` URLs to `classical.music.apple.com`. On
    /// unknown / unparseable versions the gate is `false` so MeedyaDL
    /// preserves the pre-#880 pass-through behaviour — never rewrite
    /// when we can't audit the target version (#880).
    #[test]
    fn classical_music_host_required_gate_covers_support_window() {
        let _lock = test_lock();

        for v in ["2.9.1", "2.9.3", "3.0", "3.6", "3.7.1"] {
            set_detected_version(Some(v.to_string()));
            assert!(
                supports(GamdlFeature::ClassicalMusicHostRequired),
                "{v}: classical.music.apple.com host required, rewrite must engage"
            );
        }

        // Pre-2.9.1 — out of support window, gate must be off.
        set_detected_version(Some("2.9".to_string()));
        assert!(
            !supports(GamdlFeature::ClassicalMusicHostRequired),
            "2.9: pre-support-window, rewrite must NOT engage"
        );

        // Unknown / unparseable — gate off, pass-through preserved.
        set_detected_version(None);
        assert!(
            !supports(GamdlFeature::ClassicalMusicHostRequired),
            "None: unknown version, rewrite must NOT engage"
        );

        set_detected_version(None);
    }

    /// `StorefrontIniKeyStripped` returns `true` on every supported
    /// GAMDL version (>= 2.9.1) — `cleanup_unknown_params()` has been
    /// silently stripping the key since 2.9.1 and there's no version
    /// in our support window where it's a real CLI/INI option. On
    /// unknown versions the gate is `false` so MeedyaDL keeps emitting
    /// the key, preserving pre-#881 behaviour for unaudited installs
    /// (#881).
    #[test]
    fn storefront_ini_key_stripped_gate_covers_support_window() {
        let _lock = test_lock();

        for v in ["2.9.1", "2.9.3", "3.0", "3.6", "3.7.1"] {
            set_detected_version(Some(v.to_string()));
            assert!(
                supports(GamdlFeature::StorefrontIniKeyStripped),
                "{v}: storefront INI key is stripped by GAMDL, MeedyaDL must omit"
            );
        }

        set_detected_version(Some("2.9".to_string()));
        assert!(
            !supports(GamdlFeature::StorefrontIniKeyStripped),
            "2.9: out of window, omit-storefront-INI optimisation must NOT engage"
        );

        set_detected_version(None);
        assert!(
            !supports(GamdlFeature::StorefrontIniKeyStripped),
            "None: unknown version, omit-storefront-INI optimisation must NOT engage"
        );

        set_detected_version(None);
    }

    /// `WrapperDecryptHostPort` requires GAMDL v3.8.2 — the release that
    /// split wrapper-v2's combined decrypt address into separate
    /// `--wrapper-decrypt-host` / `--wrapper-decrypt-port` flags. Must be
    /// `false` on every earlier wrapper-v2 release (3.6 .. 3.8.1) since
    /// the flags didn't exist yet and would crash Click, and `false` on
    /// the unknown/None version per the safe-default policy.
    #[test]
    fn wrapper_decrypt_host_port_requires_v382() {
        let _lock = test_lock();

        for v in ["3.6", "3.7.4", "3.8", "3.8.1"] {
            set_detected_version(Some(v.to_string()));
            assert!(
                !supports(GamdlFeature::WrapperDecryptHostPort),
                "{v}: split decrypt host/port flags must NOT be emitted (added in 3.8.2)"
            );
        }

        for v in ["3.8.2", "3.8.3", "3.8.4", "3.8.5", "3.9", "4.0"] {
            set_detected_version(Some(v.to_string()));
            assert!(
                supports(GamdlFeature::WrapperDecryptHostPort),
                "{v}: split decrypt host/port flags must be emitted"
            );
        }

        set_detected_version(None);
        assert!(
            !supports(GamdlFeature::WrapperDecryptHostPort),
            "None: unknown version, split decrypt flags must NOT be emitted"
        );

        set_detected_version(None);
    }

    /// `AssetsApiUnlocksLossyCodecs` requires GAMDL v3.8 — the release
    /// that added `/v1/play/assets`, unlocking every non-web codec
    /// except ALAC for wrapper-less downloads (#963, #1002). Must be
    /// `false` below 3.8 and on the unknown/None version, per the
    /// safe-default policy.
    #[test]
    fn assets_api_unlocks_lossy_codecs_requires_v38() {
        let _lock = test_lock();

        for v in ["3.0", "3.6", "3.7.4"] {
            set_detected_version(Some(v.to_string()));
            assert!(
                !supports(GamdlFeature::AssetsApiUnlocksLossyCodecs),
                "{v}: assets API unlock must NOT be active (added in 3.8)"
            );
        }

        for v in ["3.8", "3.8.1", "3.8.5", "3.9", "4.0"] {
            set_detected_version(Some(v.to_string()));
            assert!(
                supports(GamdlFeature::AssetsApiUnlocksLossyCodecs),
                "{v}: assets API unlock must be active"
            );
        }

        set_detected_version(None);
        assert!(
            !supports(GamdlFeature::AssetsApiUnlocksLossyCodecs),
            "None: unknown version, assets API unlock must NOT be active"
        );

        set_detected_version(None);
    }
}
