// Copyright (c) 2024-2026 MeedyaSuite
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
    /// Installed version is on the [`KNOWN_BAD_VERSIONS`] list: a
    /// release we have specifically checked and found broken, not
    /// merely one we haven't got round to testing.
    ///
    /// This is a stronger statement than [`Self::Untested`] and takes
    /// precedence over every other classification, including being
    /// below the supported floor — "this exact release is broken, here
    /// is what to move to" is more useful to a user than "this release
    /// is old" or "this release is new".
    KnownBad {
        installed: String,
        /// Plain-English description of what is broken, ready to show
        /// to a user. Comes from [`KnownBadVersion::reason`].
        reason: String,
        /// The version to move to instead. From
        /// [`KnownBadVersion::fixed_in`].
        fixed_in: String,
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

// ============================================================
// Known-bad releases — versions we refuse on purpose
// ============================================================
//
// The support window answers "how old / how new is this?". It cannot
// answer "is this particular release broken?", because a broken
// release usually sits comfortably INSIDE the window: it is newer than
// the floor, older than the ceiling, and looks perfectly ordinary to
// every version comparison we make. GAMDL 3.9 is exactly that case.
//
// So this is a short, hand-maintained list of releases we have checked
// and found broken in a way that matters to MeedyaDL. It is short by
// design: an entry here is a claim we have verified, not a suspicion.
// Every entry must say, in words a user can act on, what is broken and
// which version fixes it.
//
// The list is enforced in three places, and all three matter:
//   * the explicit-version install paths refuse it outright, so nobody
//     can land on it by clicking an "install this version" button;
//   * `classify` / `classify_for_platform` report it, so a user who
//     already has it installed is told at startup rather than being
//     left to work out why downloads fail;
//   * the routine bounded install never picks it anyway, because a
//     fixed release with a higher version number exists — but that is
//     luck, not enforcement, and it would stop being true if a broken
//     release were ever the newest one.

/// One release we refuse on purpose, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownBadVersion {
    /// The exact release, written the way upstream writes it
    /// (e.g. `"3.9"`). Matched on the numbers only, so `"3.9"` and
    /// `"3.9.0"` are the same release — see [`known_bad_version`].
    pub version: &'static str,
    /// What is broken, in plain English, ready to put in front of a
    /// user. No code identifiers, no issue numbers.
    pub reason: &'static str,
    /// The release that fixes it — what we tell the user to move to.
    pub fixed_in: &'static str,
}

/// Releases MeedyaDL refuses to install, and will warn about if it
/// finds one already installed.
///
/// GAMDL 3.9 is the first and, at the time of writing, only entry.
///
/// What went wrong: 3.9 changed how it finds the copy-protection key
/// for a track, looking it up by a label on the key. Apple's web AAC
/// streams present a key carrying no such label, so the lookup found
/// nothing and the download stopped outright. That is the `aac-web`
/// and `aac-he-web` formats — the two at the very bottom of MeedyaDL's
/// default audio fallback chain, so they are what every other format
/// falls back TO when it is unavailable. Losing them means losing the
/// safety net rather than one option among many, and the failure looks
/// to a user like "this track just isn't available". That is why this
/// is a refusal and not a warning. Upstream fixed it the next day in
/// 3.9.1, which restores the old behaviour whenever the labelled
/// lookup comes back empty.
pub const KNOWN_BAD_VERSIONS: &[KnownBadVersion] = &[KnownBadVersion {
    version: "3.9",
    reason: "GAMDL 3.9 cannot download Apple's web AAC formats at all — it looks for the \
             track's copy-protection key by a label that Apple's web streams do not put on \
             it, and stops when it finds none. Those formats are the last resort in MeedyaDL's \
             fallback chain, so downloads that would otherwise have recovered simply fail.",
    fixed_in: "3.9.1",
}];

/// Splits a version string into its first three numbers, or `None` if
/// any of them is not a plain number.
///
/// Deliberately stricter than [`is_version_at_least`], which silently
/// substitutes `0` for anything it cannot parse. That leniency is fine
/// for "is this at least X?" comparisons but wrong here: under it,
/// `"3.9.1rc1"` would read as `(3, 9, 0)` and match the known-bad
/// entry for 3.9 — refusing a pre-release of the very fix we are
/// telling people to install. Returning `None` for anything that is
/// not plainly numeric means a string we cannot read with confidence
/// is never treated as a known-bad release.
///
/// A missing minor or patch counts as zero, so `"3.9"` and `"3.9.0"`
/// both give `(3, 9, 0)` — upstream writes the same release both ways.
fn numeric_version_parts(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = match parts.next() {
        Some(p) => p.parse().ok()?,
        None => 0,
    };
    let patch: u32 = match parts.next() {
        Some(p) => p.parse().ok()?,
        None => 0,
    };
    Some((major, minor, patch))
}

/// Returns the [`KNOWN_BAD_VERSIONS`] entry matching `version`, or
/// `None` when the version is fine (or is a string we cannot read).
///
/// Matching is on the numbers only, so `"3.9"` and `"3.9.0"` both
/// match the 3.9 entry. Anything with a non-numeric part — a
/// pre-release like `"3.9.1rc1"`, or plain garbage — matches nothing.
#[must_use]
pub fn known_bad_version(version: &str) -> Option<&'static KnownBadVersion> {
    let parts = numeric_version_parts(version)?;
    KNOWN_BAD_VERSIONS
        .iter()
        .find(|bad| numeric_version_parts(bad.version) == Some(parts))
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

    // Checked before anything else: a release on the known-bad list is
    // broken in a specific, describable way, and saying so is more
    // useful than any statement about how it compares with the window.
    if let Some(bad) = known_bad_version(installed) {
        return VersionSupport::KnownBad {
            installed: installed.to_string(),
            reason: bad.reason.to_string(),
            fixed_in: bad.fixed_in.to_string(),
        };
    }

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
// The functions below layer a per-platform override on top of the
// global window. `support_window()` and `classify()` still report the
// global picture; everything that decides what to INSTALL or what to
// TELL THE USER about their own machine goes through the per-platform
// ceiling — `pip_version_spec()`, `is_above_tested_ceiling()`,
// `classify_for_platform()`, and the explicit-version install path in
// `gamdl_service`.
//
// That was not always so. The table originally affected labelling
// only, on the assumption that pip would resolve down by itself when a
// platform had no installable package. That assumption held for Linux
// ARMv7, where the missing package is GAMDL's own, and broke for
// Windows ARM64 at GAMDL 3.9, where the missing package belongs to a
// dependency two levels down and pip therefore tries to compile it
// instead. Relying on a resolver noticing a problem is not the same as
// refusing outright, and only one of the two can explain itself to the
// user.
//
// A platform with no entry in `[gamdl.platform_ceilings]` sees
// byte-identical results from the platform-aware functions as from the
// plain ones — which is every platform except Linux ARMv7 and Windows
// ARM64 today.

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

/// Returns a human-readable name for a platform ID, for messages shown
/// to users ("Windows on ARM", not `windows-aarch64`).
///
/// An ID we don't recognise is handed back unchanged rather than
/// replaced with something vague like "your platform" — if this ever
/// reaches a user it should at least be searchable.
#[must_use]
pub fn platform_display_name(platform_id: &str) -> &str {
    match platform_id {
        "macos" => "macOS",
        "windows-x86_64" => "64-bit Windows",
        "windows-aarch64" => "Windows on ARM",
        "linux-x86_64" => "64-bit Linux",
        "linux-aarch64" => "Linux on 64-bit ARM",
        "linux-armv7" => "Linux on 32-bit ARM",
        other => other,
    }
}

/// Returns the `[gamdl.platform_ceilings]` entry for `platform_id`, or
/// `None` when that platform simply tracks the global ceiling.
///
/// Callers use this to tell the two cases apart, which matters: a
/// version above the GLOBAL ceiling is merely untested, and a user is
/// allowed to install it deliberately. A version above a HELD-BACK
/// platform's ceiling cannot be installed there at all, so it is
/// refused rather than warned about.
#[must_use]
pub fn platform_ceiling_override(platform_id: &str) -> Option<&'static str> {
    support_window()
        .platform_ceilings
        .get(platform_id)
        .map(String::as_str)
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

/// Returns the version to recommend on `platform_id`: this build's
/// `recommended_version`, brought down to that platform's ceiling when
/// the recommendation is above what the platform can install.
///
/// Without this, the "Install recommended" button in Settings > Tools
/// would point a held-back platform at a version the install path then
/// refuses — a button that exists only to produce an error message.
/// The ceilings are always real, published releases, so the version
/// this hands back is always installable.
#[must_use]
pub fn recommended_for_platform(platform_id: &str) -> String {
    let window = support_window();
    let ceiling = effective_maximum_tested(platform_id);
    if is_version_at_least(&ceiling, &window.recommended) {
        window.recommended.clone()
    } else {
        ceiling
    }
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

    // Same precedence as `classify`: a known-bad release is broken
    // everywhere, so it outranks any per-platform ceiling question.
    if let Some(bad) = known_bad_version(installed) {
        return VersionSupport::KnownBad {
            installed: installed.to_string(),
            reason: bad.reason.to_string(),
            fixed_in: bad.fixed_in.to_string(),
        };
    }

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
            // Clamped to what this platform can actually install, NOT
            // the general recommendation. An independent review caught
            // the unclamped version: on Windows on ARM (held at 3.8.5)
            // the startup log said "consider downgrading to 3.9.1" —
            // naming, as the thing to downgrade TO, a release that
            // cannot be installed there at all, and which the install
            // path then refuses. A suggestion the app itself will not
            // carry out is worse than no suggestion.
            recommended: recommended_for_platform(platform_id),
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

/// Is `version` above the ceiling for `platform_id`?
///
/// The pure, platform-explicit half of [`is_above_tested_ceiling`] —
/// takes the platform as an argument so it can be tested for a
/// platform other than the one the test happens to be running on.
///
/// `false` for unparseable strings: we can't reason about them, so we
/// don't claim they're untested.
#[must_use]
pub fn is_above_ceiling_for_platform(version: &str, platform_id: &str) -> bool {
    if !is_parseable_semver(version) {
        return false;
    }
    let maximum_tested = effective_maximum_tested(platform_id);
    // `is_version_at_least(a, b)` ⇔ `a >= b`. Above-ceiling means
    // `version > maximum_tested`, i.e. `!(maximum_tested >= version)`.
    !is_version_at_least(&maximum_tested, version)
}

/// Is `version` above the highest release this machine has been
/// validated up to?
///
/// Used by [`crate::services::update_checker`] to set the `is_untested`
/// flag on a `ComponentUpdate`, which in turn drives the frontend's
/// amber "Untested" warning badge.
///
/// **This asks about the machine it is running on, not about the build
/// in general.** It used to compare against the global
/// `maximum_tested_version` only, which was wrong for any platform held
/// below that global ceiling: on Windows ARM64, where GAMDL 3.9.1
/// cannot be installed at all, the user was shown 3.9.1 as an ordinary,
/// fully tested upgrade with no warning — and clicking it produced a
/// wall of compiler output. It now compares against that platform's own
/// effective ceiling, so the warning appears where it is deserved.
#[must_use]
pub fn is_above_tested_ceiling(version: &str) -> bool {
    is_above_ceiling_for_platform(version, current_platform_id())
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
/// Example output: `gamdl>=3.0,<=3.9.1`. Consumers pass this to
/// `pip install --upgrade {spec}` so the resolver can pick the
/// newest validated release without jumping to an untested version
/// (and never resolves down to a dropped v2 release).
///
/// **The upper bound is this machine's ceiling, not the global one.**
/// A platform listed in `[gamdl.platform_ceilings]` gets its own,
/// lower bound here. Before that, the per-platform table governed only
/// what the app SAID about a version, never what it installed — the
/// install was left to pip resolving down by itself when no package
/// existed. That worked for Linux ARMv7, where the missing package is
/// GAMDL's own and `--only-binary=gamdl` makes the newer releases
/// unsatisfiable. It does not work for Windows ARM64, where the
/// missing package belongs to a dependency two levels down: pip has no
/// reason to resolve down there, so it tries to compile from source
/// and fails. Putting the real ceiling in the range makes the cap
/// deterministic on both.
#[must_use]
pub fn pip_version_spec() -> String {
    pip_version_spec_for_platform(current_platform_id())
}

/// The pure, platform-explicit half of [`pip_version_spec`] — takes the
/// platform as an argument so it can be tested for a platform other
/// than the one the test happens to be running on.
#[must_use]
pub fn pip_version_spec_for_platform(platform_id: &str) -> String {
    let window = support_window();
    format!(
        "gamdl>={minimum},<={maximum}",
        minimum = window.minimum,
        maximum = effective_maximum_tested(platform_id),
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
/// `target` should be a parseable semver. This helper only formats the
/// string — it deliberately makes no judgement about whether the
/// version is a sensible one to install. That judgement belongs to
/// [`crate::services::gamdl_service::refuse_unsupported_target`], which
/// every explicit-version install path calls first: it turns away a
/// release on the known-bad list, and one above this platform's
/// ceiling, with a reason the user can act on.
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
/// `installed` on THIS machine's platform, taking their wrapper usage
/// into account (#1001).
///
/// | `installed` state         | `use_wrapper` | Target                              |
/// |----------------------------|---------------|--------------------------------------|
/// | below this build's floor (v2.x) | `true`  | [`LAST_WRAPPER_V1_VERSION`] ("3.5.2") — the newest release that doesn't require migrating to wrapper-v2 |
/// | below this build's floor (v2.x) | `false` | [`recommended_for_platform`] for the current platform — no wrapper to protect, so the best-tested release this machine can actually install |
/// | already `>=` this build's floor (v3.x+), or `None` (nothing installed yet) | either | [`recommended_for_platform`] for the current platform — the v2→v3 wrapper-protocol concern doesn't apply once already on v3, or when there's no prior install to protect |
///
/// This function only *encodes* the target table above — it does not
/// itself decide whether an upgrade is warranted. Callers should gate
/// on `classify(installed) == VersionSupport::Unsupported` (i.e.
/// `installed` is still on the pre-floor v2.x line) before consulting
/// it for the v2→v3 migration flow; called with an already-v3+
/// `installed`, it safely degrades to the ordinary recommended-for-
/// platform target rather than ever recommending a downgrade.
///
/// Thin wrapper over [`recommended_upgrade_target_for_platform`] that
/// fixes `platform_id` to [`current_platform_id`]. Kept `String` (not
/// `&'static str`) even though [`LAST_WRAPPER_V1_VERSION`] is a
/// compile-time constant, because the other branch —
/// [`recommended_for_platform`] — is itself platform-clamped and
/// therefore computed, not static; a caller can't tell the two
/// branches apart from the return type anyway, so one owned `String`
/// return type is simpler than a `Cow`.
#[must_use]
pub fn recommended_upgrade_target(installed: Option<&str>, use_wrapper: bool) -> String {
    recommended_upgrade_target_for_platform(installed, use_wrapper, current_platform_id())
}

/// The pure, platform-explicit half of [`recommended_upgrade_target`] —
/// takes the platform as an argument so it can be tested for a platform
/// other than the one the test happens to be running on. Mirrors the
/// `_for_platform` pattern used by [`classify_for_platform`] /
/// [`pip_version_spec_for_platform`] / [`is_above_ceiling_for_platform`]
/// elsewhere in this module.
///
/// **Why this exists**: before this split, the v2→v3 "no wrapper to
/// protect" and "already on v3" branches both returned the GLOBAL
/// `support_window().recommended` — on a platform held below that
/// (Windows on ARM, held at 3.8.5 while the global recommended tracks
/// 3.9.1), this named a version the platform-aware install path
/// (`refuse_unsupported_target`) then refused outright. Both branches
/// now go through [`recommended_for_platform`], which is already
/// clamped to whatever `platform_id` can install — exactly the
/// guarantee [`recommended_for_platform`]'s own doc comment describes
/// for the "Install recommended" button in Settings > Tools. The
/// [`LAST_WRAPPER_V1_VERSION`] branch needs no clamping: it is a fixed
/// historical release (3.5.2) that predates every per-platform ceiling
/// entry in `[gamdl.platform_ceilings]` today, so it is installable
/// everywhere regardless of platform.
#[must_use]
pub fn recommended_upgrade_target_for_platform(
    installed: Option<&str>,
    use_wrapper: bool,
    platform_id: &str,
) -> String {
    // `None` (nothing installed yet, e.g. a fresh setup) is NOT the
    // "still on v2.x" case — there's no existing wrapper-v1 setup to
    // protect, so it falls straight through to `recommended` just like
    // an already-v3+ install. Only an explicitly known pre-floor (v2.x)
    // version triggers the wrapper-v1-preserving branch.
    let is_pre_floor_v2 =
        installed.is_some_and(|v| !is_version_at_least(v, &support_window().minimum));

    if use_wrapper && is_pre_floor_v2 {
        LAST_WRAPPER_V1_VERSION.to_string()
    } else {
        recommended_for_platform(platform_id)
    }
}

/// Returns the version to tell a user to move to when `installed` is a
/// [`KNOWN_BAD_VERSIONS`] release, already brought down to whatever
/// `platform_id` can actually install. Returns `None` when `installed`
/// is not on the known-bad list.
///
/// [`KnownBadVersion::fixed_in`] names the release that fixes the
/// specific fault (e.g. `"3.9.1"` for the GAMDL 3.9 entry), but that
/// release can sit ABOVE a held-back platform's ceiling — pointing such
/// a user at a version their own platform's install path then refuses
/// (see [`crate::services::gamdl_service::refuse_unsupported_target`])
/// would just move the same confusion one step later. When
/// `fixed_in` is out of reach for `platform_id`, this falls back to
/// [`recommended_for_platform`], which is always installable there.
#[must_use]
pub fn known_bad_upgrade_target_for_platform(installed: &str, platform_id: &str) -> Option<String> {
    let bad = known_bad_version(installed)?;
    if is_version_at_least(&effective_maximum_tested(platform_id), bad.fixed_in) {
        Some(bad.fixed_in.to_string())
    } else {
        Some(recommended_for_platform(platform_id))
    }
}

// ============================================================
// Known-bad advice sentence (independent-review fix, 2026-09-22)
// ============================================================
//
// What went wrong before this existed: three separate call sites each
// hand-built their own sentence about a known-bad release, and every
// one of them assumed the release named in `KnownBadVersion::fixed_in`
// could always be installed on the user's own machine. That is true on
// most platforms, and was false on exactly the platform GAMDL 3.9
// created the first real test of: Windows on ARM, held at 3.8.5 while
// the fix for 3.9 is 3.9.1. On that platform the old wording read
// "Update to GAMDL 3.8.5 or newer" — a straight downgrade dressed up as
// an update, AND "or newer" re-permits the very 3.9 release the
// sentence exists to move the user away from, AND (in two of the three
// sites) the literal, un-installable "3.9.1" was named outright, which
// this platform's own install path then refuses the moment it's tried.
//
// This is the one place that sentence is built now. Every call site
// passes in the same four facts (why it's broken, what fixes it, what
// is actually installed, which platform) and gets back the same words.

/// Builds the plain-English sentence telling a user what to do about a
/// GAMDL release on the [`KNOWN_BAD_VERSIONS`] list, worded correctly
/// for `platform_id`.
///
/// `reason` and `fixed_in` come from the matching [`KnownBadVersion`]
/// entry (`bad.reason`, `bad.fixed_in`). `installed` is the version
/// actually detected or requested — it does not need to be spelled
/// identically to [`KnownBadVersion::version`] (pip might report
/// `"3.9.0"` where the table says `"3.9"`; [`known_bad_version`]
/// matches those numerically as the same release).
///
/// Three cases, and why a single "name the fix, add 'or newer'"
/// template cannot serve all of them:
///
/// 1. **This platform can install the release that fixes it.** The
///    ordinary case. "or newer" is safe here because every entry in
///    [`KNOWN_BAD_VERSIONS`] is checked
///    (`nothing_we_recommend_or_install_is_on_the_known_bad_list`) to
///    never point `fixed_in` at another broken release — so everything
///    from `fixed_in` upward is fine to recommend.
/// 2. **This platform's own ceiling sits BELOW both the fix and what is
///    already installed.** Today's live case: Windows on ARM, held at
///    3.8.5, with GAMDL 3.9 installed and 3.9.1 (the fix) needing a
///    package with no Windows ARM64 build. Naming the fix here would
///    send the user straight into a refusal from
///    [`crate::services::gamdl_service::refuse_unsupported_target`], so
///    instead this says to move BACK to the platform's own ceiling —
///    worded as a downgrade, because it is one — and says in plain
///    words why the fix itself is out of reach.
/// 3. **This platform's own ceiling sits below the fix, but at or above
///    what is installed.** Not seen on any platform this build ships
///    today, but handled for correctness: the platform can still move
///    forward, just not all the way to the fix, so this is worded as an
///    ordinary (if capped) install rather than a downgrade.
///
/// Never says "or newer" outside case 1, and never names a version
/// [`crate::services::gamdl_service::refuse_unsupported_target`] would
/// then refuse.
///
/// Cases 2 and 3 deliberately say only that no working version has been
/// published for the platform, without naming what is missing. An
/// earlier wording said "one of its required packages" — true of
/// Windows on ARM, where it is a dependency with no ARM64 build, but
/// false of Linux on 32-bit ARM, where it is GAMDL's own package that
/// stops being published. The reason differs per platform and would
/// have to be maintained alongside `[gamdl.platform_ceilings]`, which a
/// future ceiling entry would quietly get wrong. The user's options are
/// the same either way, so the sentence stays true for all of them.
#[must_use]
pub fn known_bad_advice(reason: &str, fixed_in: &str, installed: &str, platform_id: &str) -> String {
    let ceiling = effective_maximum_tested(platform_id);

    if is_version_at_least(&ceiling, fixed_in) {
        // Case 1 — the fix itself is installable here. Every later
        // release is fine too (the known-bad table's own invariant),
        // so "or newer" is safe.
        return format!("{reason} Update to GAMDL {fixed_in} or newer.");
    }

    let platform_name = platform_display_name(platform_id);

    if !is_version_at_least(&ceiling, installed) {
        // Case 2 — this platform's ceiling is even below what's
        // already installed. This is a downgrade, and must be said as
        // one, not dressed up as an "update".
        format!(
            "{reason} {platform_name} can only install GAMDL up to {ceiling} — move back to \
             that version. GAMDL {fixed_in} fixes this, but no version of it that works on \
             {platform_name} has been published."
        )
    } else {
        // Case 3 — capped below the fix, but still a genuine step
        // forward from what's installed.
        format!(
            "{reason} {platform_name} can install GAMDL up to {ceiling} — install that \
             version. GAMDL {fixed_in} fixes this, but no version of it that works on \
             {platform_name} has been published."
        )
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

    /// GAMDL 3.9 added a second way of unlocking copy-protected tracks,
    /// called PlayReady, alongside the built-in Widevine one. It is offered
    /// through two options, `--drm-backend` and `--prd-path`, and needs a
    /// `.prd` device file the user supplies themselves.
    ///
    /// Available from **3.9**. In practice the lowest release that can ever
    /// reach this is 3.9.1, because 3.9 itself is on the known-bad list
    /// (see [`KNOWN_BAD_VERSIONS`]) and MeedyaDL refuses to install it. The
    /// threshold is still written as 3.9, because that is the release that
    /// added the feature — tying it to 3.9.1 would be recording the wrong
    /// fact, and would silently become wrong if the known-bad entry were
    /// ever lifted.
    ///
    /// Older GAMDL releases reject an option they do not know about
    /// outright rather than ignoring it, so passing either option to them
    /// would end the download before it started. That is why this gate
    /// exists, and why the Settings screen hides the choice entirely
    /// unless this is true.
    PlayReadyDrmBackend,
}

impl GamdlFeature {
    /// Returns `true` when `version` is known to support this feature.
    ///
    /// Version comparison uses [`is_version_at_least`] which tolerates
    /// two-part ("2.9") and unparseable version strings gracefully.
    pub(crate) fn is_available_on(self, version: &str) -> bool {
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
            Self::PlayReadyDrmBackend => is_version_at_least(version, "3.9"),
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
        (GamdlFeature::PlayReadyDrmBackend, "play_ready_drm_backend"),
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

    // ----------------------------------------------------------------
    // Known-bad releases (GAMDL 3.9)
    // ----------------------------------------------------------------

    #[test]
    fn known_bad_matches_the_same_release_written_either_way() {
        // Upstream calls it 3.9; pip will happily report 3.9.0. They
        // are the same release and both must match — and, more than
        // just both being present, both spellings must resolve to the
        // SAME table entry (not e.g. two different releases that both
        // happen to be listed). Asserting on the whole struct, rather
        // than restating one hardcoded field, means this actually
        // fails if the numeric-matching logic ever starts treating
        // "3.9" and "3.9.0" as different lookups.
        let by_short_form = known_bad_version("3.9");
        let by_long_form = known_bad_version("3.9.0");
        assert!(by_short_form.is_some());
        assert_eq!(by_short_form, by_long_form);

        let entry = by_short_form.expect("3.9 must be on the known-bad list");
        assert!(
            entry.reason.contains("AAC"),
            "the reason must say what is broken in the user's terms, got: {}",
            entry.reason
        );
    }

    #[test]
    fn known_bad_does_not_catch_neighbouring_releases() {
        // The release that FIXES the problem must not be caught by the
        // entry describing the problem.
        assert!(known_bad_version("3.9.1").is_none());
        assert!(known_bad_version("3.8.5").is_none());
        assert!(known_bad_version("3.10").is_none());

        // Nor must a pre-release of the fix. This is the reason the
        // matcher refuses to guess at non-numeric parts: the lenient
        // comparison used elsewhere reads "3.9.1rc1" as 3.9.0, which
        // would have blocked a release carrying the very repair we are
        // telling people to install.
        assert!(known_bad_version("3.9.1rc1").is_none());

        // And nothing unreadable is ever treated as a known-bad
        // release.
        assert!(known_bad_version("").is_none());
        assert!(known_bad_version("garbage").is_none());
        assert!(known_bad_version("v3.9").is_none());
    }

    #[test]
    fn classify_reports_a_known_bad_release_with_what_to_do_about_it() {
        match classify(Some("3.9")) {
            VersionSupport::KnownBad {
                installed,
                reason,
                fixed_in,
            } => {
                assert_eq!(installed, "3.9");
                // Cross-checked against `known_bad_version` itself rather
                // than restating the "3.9.1" literal a second time — this
                // fails if `classify`'s `KnownBad` arm ever drifts from
                // the table it is supposed to be reporting (e.g. a
                // copy-paste of the wrong field), which a hardcoded
                // literal comparison would not catch since both call
                // sites would happen to agree by coincidence.
                let table_entry =
                    known_bad_version("3.9").expect("3.9 must be on the known-bad list");
                assert_eq!(fixed_in, table_entry.fixed_in);
                assert_eq!(reason, table_entry.reason);
            }
            other => panic!("Expected KnownBad, got {other:?}"),
        }

        // Same answer on every platform — a broken release is broken
        // everywhere, so the per-platform ceiling never gets a look in.
        for platform_id in [
            "macos",
            "windows-aarch64",
            "windows-x86_64",
            "linux-armv7",
            "linux-x86_64",
        ] {
            assert!(
                matches!(
                    classify_for_platform(Some("3.9.0"), platform_id),
                    VersionSupport::KnownBad { .. }
                ),
                "{platform_id} must report 3.9.0 as known-bad"
            );
        }

        // And the classification is NOT applied to anything else.
        assert!(!matches!(
            classify(Some("3.9.1")),
            VersionSupport::KnownBad { .. }
        ));
    }

    #[test]
    fn nothing_we_recommend_or_install_is_on_the_known_bad_list() {
        // The load-bearing guard. A known-bad release sitting inside
        // the support window is invisible to every version comparison
        // we make, so the only thing stopping us shipping people onto
        // one is this check. It covers the general ceiling, the
        // recommended version, the floor, and every per-platform
        // ceiling.
        let window = support_window();
        for (label, version) in [
            ("minimum_version", &window.minimum),
            ("maximum_tested_version", &window.maximum_tested),
            ("recommended_version", &window.recommended),
        ] {
            assert!(
                known_bad_version(version).is_none(),
                "{label} is set to {version}, which is on the known-bad list"
            );
        }
        for (platform_id, ceiling) in &window.platform_ceilings {
            assert!(
                known_bad_version(ceiling).is_none(),
                "the ceiling for {platform_id} is {ceiling}, which is on the known-bad list"
            );
        }

        // Every fix named on the list must itself be clean, or we
        // would be sending users from one broken release to another.
        for bad in KNOWN_BAD_VERSIONS {
            assert!(
                known_bad_version(bad.fixed_in).is_none(),
                "{} points at {} as the fix, but that is itself on the list",
                bad.version,
                bad.fixed_in
            );
            assert!(
                is_version_at_least(bad.fixed_in, bad.version),
                "{} names {} as its fix, which is an older release",
                bad.version,
                bad.fixed_in
            );
        }
    }

    #[test]
    fn known_bad_upgrade_target_names_the_fix_on_a_platform_that_can_install_it() {
        // macOS tracks the global ceiling (3.9.1), which is itself the
        // fix for the 3.9 entry — so the fix is directly reachable and
        // should be named as-is, not swapped for something else.
        let target = known_bad_upgrade_target_for_platform("3.9", "macos")
            .expect("3.9 must be on the known-bad list");
        assert_eq!(target, "3.9.1");
    }

    #[test]
    fn known_bad_upgrade_target_falls_back_when_the_fix_is_out_of_reach() {
        // Windows on ARM is held at 3.8.5 — BELOW the 3.9 entry's own
        // `fixed_in` (3.9.1). Naming 3.9.1 here would send a user
        // straight into a refusal from `refuse_unsupported_target`, so
        // this must fall back to whatever that platform can actually
        // install instead.
        let window = support_window();
        assert!(
            window.platform_ceilings.contains_key("windows-aarch64"),
            "this test only proves something if windows-aarch64 is genuinely held back"
        );

        let target = known_bad_upgrade_target_for_platform("3.9", "windows-aarch64")
            .expect("3.9 must be on the known-bad list");
        assert_eq!(target, effective_maximum_tested("windows-aarch64"));
        assert_ne!(
            target, "3.9.1",
            "windows-aarch64 cannot install 3.9.1 — must not be told to"
        );
    }

    #[test]
    fn known_bad_upgrade_target_is_none_for_a_clean_release() {
        assert_eq!(known_bad_upgrade_target_for_platform("3.9.1", "macos"), None);
        assert_eq!(
            known_bad_upgrade_target_for_platform(&support_window().minimum, "macos"),
            None
        );
    }

    // ----------------------------------------------------------------
    // known_bad_advice (independent-review fix, 2026-09-22)
    // ----------------------------------------------------------------

    #[test]
    fn known_bad_advice_advises_the_fix_with_or_newer_where_it_can_be_installed() {
        // A platform with no ceiling of its own can install the fix
        // outright — "or newer" is correct, and the fix must be named.
        let bad = known_bad_version("3.9").expect("3.9 must be on the known-bad list");
        for platform_id in ["macos", "linux-x86_64", "linux-aarch64", "windows-x86_64"] {
            let advice = known_bad_advice(bad.reason, bad.fixed_in, "3.9", platform_id);
            assert!(
                advice.contains("3.9.1"),
                "{platform_id}: must advise the fixed-in release, got: {advice}"
            );
            assert!(
                advice.contains("or newer"),
                "{platform_id}: safe to say 'or newer' when the fix is installable, got: {advice}"
            );
            assert!(
                advice.contains(bad.reason),
                "{platform_id}: must still say what is broken, got: {advice}"
            );
        }
    }

    #[test]
    fn known_bad_advice_on_a_held_back_platform_never_says_or_newer() {
        // Windows on ARM: held at 3.8.5, below the 3.9 entry's own fix
        // (3.9.1). This is the exact case an independent review found
        // broken — the message used to say "Update to GAMDL 3.8.5 or
        // newer", which named a downgrade AND re-permitted the broken
        // 3.9 release in the same sentence.
        //
        // The fix does NOT mean hiding "3.9.1" from the message — the
        // user needs to be told it exists and why it isn't reachable
        // here — it means the fix is never framed as something to
        // install ("Update to ... 3.9.1" / "... 3.9.1 or newer").
        let window = support_window();
        if !window.platform_ceilings.contains_key("windows-aarch64") {
            return; // Entry removed — nothing left to prove.
        }
        let bad = known_bad_version("3.9").expect("3.9 must be on the known-bad list");
        let advice = known_bad_advice(bad.reason, bad.fixed_in, "3.9", "windows-aarch64");

        assert!(
            !advice.contains("or newer"),
            "must never say 'or newer' when the fix cannot be installed here, got: {advice}"
        );
        assert!(
            !advice.contains("Update to GAMDL 3.9.1") && !advice.contains("Install GAMDL 3.9.1"),
            "3.9.1 must never be framed as something to install here, got: {advice}"
        );
        assert!(
            advice.contains("move back"),
            "3.8.5 is BELOW the installed 3.9 — must be said as a downgrade, not an update, \
             got: {advice}"
        );
        assert!(
            advice.contains(&effective_maximum_tested("windows-aarch64")),
            "must still say what CAN be installed here, got: {advice}"
        );
        assert!(
            advice.contains("3.9.1"),
            "must still name 3.9.1 as part of explaining why it's unavailable, got: {advice}"
        );
        assert!(
            advice.contains("has been published"),
            "must say why 3.9.1 specifically is unreachable on this platform, got: {advice}"
        );
    }

    #[test]
    fn known_bad_advice_on_every_held_back_platform_never_offers_the_fix_as_an_update() {
        // Generalised across whichever platforms `[gamdl.platform_ceilings]`
        // actually lists today, so this doesn't quietly stop proving
        // anything if the table's membership changes (mirrors the style
        // of `nothing_we_recommend_or_install_is_on_the_known_bad_list`
        // and `pip_version_spec_caps_held_back_platforms_lower` above).
        let window = support_window();
        let bad = known_bad_version("3.9").expect("3.9 must be on the known-bad list");

        for platform_id in window.platform_ceilings.keys() {
            let advice = known_bad_advice(bad.reason, bad.fixed_in, "3.9", platform_id);
            let ceiling = effective_maximum_tested(platform_id);
            if is_version_at_least(&ceiling, bad.fixed_in) {
                // This platform's ceiling has caught up to the fix —
                // nothing left to prove about it being held back.
                continue;
            }
            assert!(
                !advice.contains("or newer"),
                "{platform_id}: 'or newer' would re-permit the broken release, got: {advice}"
            );
            assert!(
                !advice.contains(&format!("Update to GAMDL {}", bad.fixed_in))
                    && !advice.contains(&format!("Install GAMDL {}", bad.fixed_in)),
                "{platform_id}: must not offer {} as something to install, got: {advice}",
                bad.fixed_in
            );
        }
    }

    #[test]
    fn known_bad_advice_says_install_not_move_back_when_the_ceiling_is_still_forward() {
        // The rarer, third case: a platform's ceiling is below the fix
        // but still AT OR ABOVE what's installed — a genuine (if capped)
        // step forward, not a downgrade.
        //
        // No platform this build ships is in that position today, and an
        // earlier version of this test papered over that by passing
        // macOS, which returns at case 1 and so never reached the branch
        // the test is named after. An independent review caught it. The
        // helper is pure, so the honest way to reach case 3 is to call
        // it directly with a broken release BELOW a held-back platform's
        // ceiling — i.e. the shape a future known-bad entry would have.
        let held_back = support_window()
            .platform_ceilings
            .iter()
            .find(|(_, ceiling)| !is_version_at_least(ceiling, &support_window().recommended))
            .map(|(platform_id, ceiling)| (platform_id.clone(), ceiling.clone()));
        let Some((platform_id, ceiling)) = held_back else {
            return; // No platform is capped below the fix any more.
        };

        // Installed is deliberately the supported floor, which is below
        // every ceiling in the table — so the ceiling is a step forward.
        let floor = support_window().minimum.clone();
        assert!(
            is_version_at_least(&ceiling, &floor),
            "test premise: {ceiling} must be at or above the floor {floor}"
        );
        let advice = known_bad_advice(
            "A made-up fault, used only to reach the third branch.",
            &support_window().recommended,
            &floor,
            &platform_id,
        );

        assert!(
            !advice.contains("move back"),
            "{platform_id}: {floor} is below the ceiling — a step forward must not be worded \
             as a downgrade, got: {advice}"
        );
        assert!(
            advice.contains(&format!("install GAMDL up to {ceiling} — install that version")),
            "{platform_id}: must name the capped version as the thing to install, got: {advice}"
        );
        assert!(
            !advice.contains("or newer"),
            "{platform_id}: 'or newer' would reach past the ceiling, got: {advice}"
        );
    }

    #[test]
    fn untested_never_suggests_a_version_this_platform_cannot_install() {
        // Found by an independent review: `classify_for_platform`'s
        // `Untested` arm handed back the GENERAL recommendation while
        // clamping everything else to the platform. The startup
        // activity-log line renders that as "consider downgrading to
        // {recommended}", so a Windows-on-ARM user running 3.9.1 was
        // told to downgrade to 3.9.1 — and anyone above it was pointed
        // at a version `refuse_unsupported_target` then refuses.
        let window = support_window();

        // A version comfortably above any real ceiling, built by keeping
        // the major number (so it stays above the supported floor) and
        // pushing the minor far past anything upstream has shipped. A
        // fourth component (`3.8.5.99`) is NOT usable here: the version
        // comparison reads three numbers, so a four-part string is not
        // the "clearly newer" value it looks like.
        let far_future = |version: &str| {
            format!("{}.999.0", version.split('.').next().unwrap_or("3"))
        };

        for (platform_id, ceiling) in &window.platform_ceilings {
            let above = far_future(ceiling);
            let VersionSupport::Untested { recommended, .. } =
                classify_for_platform(Some(&above), platform_id)
            else {
                panic!("{platform_id}: {above} is above the ceiling, so it must be Untested");
            };
            assert!(
                is_version_at_least(ceiling, &recommended),
                "{platform_id} is pointed at {recommended}, above its own ceiling of {ceiling}"
            );
            assert!(
                known_bad_version(&recommended).is_none(),
                "{platform_id} is pointed at {recommended}, which is a known-bad release"
            );
        }

        // A platform with no ceiling of its own is unaffected: above the
        // global ceiling it still gets the general recommendation.
        let above_global = far_future(&window.maximum_tested);
        let VersionSupport::Untested { recommended, .. } =
            classify_for_platform(Some(&above_global), "macos")
        else {
            panic!("{above_global} is above the global ceiling, so it must be Untested");
        };
        assert_eq!(recommended, window.recommended);
    }

    #[test]
    fn play_ready_gate_starts_at_3_9() {
        // GAMDL 3.9 is the release that added PlayReady, so that is the
        // threshold, even though 3.9 itself is refused as known-bad and
        // 3.9.1 is the lowest version anybody can actually be running.
        // Recording 3.9.1 here would be recording the wrong fact.
        for older in ["3.0", "3.6", "3.8", "3.8.5"] {
            assert!(
                !GamdlFeature::PlayReadyDrmBackend.is_available_on(older),
                "{older} predates PlayReady and rejects options it does not know"
            );
        }
        for newer in ["3.9", "3.9.1", "3.10", "4.0"] {
            assert!(
                GamdlFeature::PlayReadyDrmBackend.is_available_on(newer),
                "{newer} understands PlayReady"
            );
        }
    }

    #[test]
    fn play_ready_is_off_when_no_version_has_been_detected() {
        // The whole point of the gate is never to send an option a
        // release might reject, so "we do not know yet" must mean "do not
        // send it" — the same conservative default every other gate has.
        set_detected_version(None);
        assert!(!supports(GamdlFeature::PlayReadyDrmBackend));
    }

    #[test]
    fn recommended_for_platform_never_names_a_version_that_cannot_be_installed() {
        let window = support_window();

        // A platform with no ceiling of its own gets the general
        // recommendation unchanged.
        assert_eq!(recommended_for_platform("macos"), window.recommended);
        assert_eq!(recommended_for_platform("linux-x86_64"), window.recommended);

        // A held-back platform is never told to install something
        // above its own ceiling — which is exactly what the install
        // path would refuse.
        for (platform_id, ceiling) in &window.platform_ceilings {
            let recommended = recommended_for_platform(platform_id);
            assert!(
                is_version_at_least(ceiling, &recommended),
                "{platform_id} is told to install {recommended}, above its ceiling of {ceiling}"
            );
            assert!(
                is_version_at_least(&recommended, &window.minimum),
                "{platform_id} is told to install {recommended}, below the supported floor"
            );
            assert!(
                known_bad_version(&recommended).is_none(),
                "{platform_id} is told to install {recommended}, which is on the known-bad list"
            );
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
        // Everything here is judged against the ceiling for the
        // machine the test is running on, which is what the function
        // now asks. On a platform held below the general ceiling
        // (Windows on ARM, 32-bit ARM Linux) the general ceiling is
        // itself above the local one, so the test reads the local one
        // rather than hard-coding the general figure.
        let local_ceiling = effective_maximum_tested(current_platform_id());

        // Above the ceiling → flagged.
        assert!(is_above_tested_ceiling("99.0.0"));
        // At the ceiling → not flagged (it's the highest tested here).
        assert!(!is_above_tested_ceiling(&local_ceiling));
        // Below the ceiling → not flagged.
        assert!(!is_above_tested_ceiling(&support_window().minimum));
        // Unparseable → not flagged (we can't reason about it).
        assert!(!is_above_tested_ceiling("garbage"));
        assert!(!is_above_tested_ceiling(""));
    }

    #[test]
    fn is_above_ceiling_for_platform_warns_only_where_the_ceiling_is_lower() {
        // The case this exists for: a platform held below the general
        // ceiling must read the general ceiling as "above the ceiling",
        // because that is what puts the amber warning on the Updates
        // page instead of offering the version as an ordinary upgrade.
        // Windows on ARM is the live example — held at 3.8.5 because
        // GAMDL 3.9 onwards needs a package with no build for it.
        let general = &support_window().maximum_tested;
        for platform_id in support_window().platform_ceilings.keys() {
            assert!(
                is_above_ceiling_for_platform(general, platform_id),
                "{general} must be flagged as above {platform_id}'s own, lower ceiling"
            );
        }

        // A platform with no entry of its own tracks the general
        // ceiling, so the same version is perfectly ordinary there.
        assert!(!is_above_ceiling_for_platform(general, "macos"));
        assert!(!is_above_ceiling_for_platform(general, "linux-x86_64"));
        assert!(!is_above_ceiling_for_platform(general, "windows-x86_64"));

        // The held-back platform's own ceiling is not above itself.
        assert!(!is_above_ceiling_for_platform(
            &effective_maximum_tested("windows-aarch64"),
            "windows-aarch64"
        ));

        // Unreadable strings are never claimed to be above anything.
        assert!(!is_above_ceiling_for_platform("garbage", "windows-aarch64"));
        assert!(!is_above_ceiling_for_platform("", "windows-aarch64"));
    }

    #[test]
    fn pip_version_spec_bounds_the_range() {
        let spec = pip_version_spec();
        let window = support_window();
        assert!(spec.starts_with("gamdl>="));
        assert!(spec.contains(&format!(">={}", window.minimum)));
        // Upper bound is this machine's ceiling, which equals the
        // general one on every platform without an entry of its own.
        assert!(spec.contains(&format!(
            "<={}",
            effective_maximum_tested(current_platform_id())
        )));
    }

    #[test]
    fn pip_version_spec_caps_held_back_platforms_lower() {
        // The point of the change: the per-platform table governs what
        // gets INSTALLED, not just what the app says about it. A
        // held-back platform must be offered a range that stops at its
        // own ceiling, while an ordinary platform's range reaches the
        // general one.
        let window = support_window();
        let ordinary = pip_version_spec_for_platform("macos");
        assert_eq!(
            ordinary,
            format!("gamdl>={},<={}", window.minimum, window.maximum_tested)
        );

        // Every held-back platform in the shipped table, whichever
        // they happen to be — written this way so removing an entry
        // (upstream finally publishes the missing package) doesn't
        // leave a test asserting a version nobody holds any more.
        for (platform_id, ceiling) in &window.platform_ceilings {
            assert_eq!(
                pip_version_spec_for_platform(platform_id),
                format!("gamdl>={},<={ceiling}", window.minimum),
                "{platform_id} must be capped at its own ceiling, not the general one"
            );
            assert_ne!(
                pip_version_spec_for_platform(platform_id),
                ordinary,
                "{platform_id} is held below the general ceiling, so its range must differ"
            );
        }

        // And the two we hold today are actually in that table — a
        // guard against an entry being dropped or renamed by accident,
        // since a missing entry silently means "no cap at all".
        assert!(
            window.platform_ceilings.contains_key("windows-aarch64"),
            "Windows on ARM must stay capped: GAMDL 3.9+ needs a package with no build for it"
        );
        assert!(
            window.platform_ceilings.contains_key("linux-armv7"),
            "32-bit ARM Linux must stay capped: GAMDL 3.8.2+ publishes no package for it"
        );
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
        // No wrapper to protect — offer the best-tested release THIS
        // machine can install. Compared against `recommended_for_platform`
        // (not the raw global `support_window().recommended`) so this
        // stays correct on a held-back platform too, not just on a
        // machine with no `[gamdl.platform_ceilings]` override.
        let target = recommended_upgrade_target(Some("2.9.3"), false);
        assert_eq!(target, recommended_for_platform(current_platform_id()));
    }

    #[test]
    fn recommended_upgrade_target_already_v3_ignores_wrapper_flag() {
        // Once already on v3.x (>= this build's floor), the v2->v3
        // wrapper-protocol migration concern doesn't apply — both
        // branches degrade to the platform-clamped recommended version
        // regardless of `use_wrapper`.
        let installed = support_window().minimum.clone();
        let expected = recommended_for_platform(current_platform_id());
        assert_eq!(
            recommended_upgrade_target(Some(&installed), true),
            expected
        );
        assert_eq!(
            recommended_upgrade_target(Some(&installed), false),
            expected
        );
    }

    #[test]
    fn recommended_upgrade_target_none_installed_gets_recommended() {
        // No installed version at all (fresh setup) — nothing to
        // protect, offer the best-tested release this machine can
        // install, regardless of the wrapper toggle's current setting.
        let expected = recommended_for_platform(current_platform_id());
        assert_eq!(recommended_upgrade_target(None, true), expected);
        assert_eq!(recommended_upgrade_target(None, false), expected);
    }

    #[test]
    fn recommended_upgrade_target_never_names_a_version_a_held_back_platform_refuses() {
        // Regression test for the bug an independent review caught: the
        // "no wrapper to protect" and "already on v3" branches used to
        // return the GLOBAL `support_window().recommended` regardless of
        // platform. On Windows ARM64 (held at 3.8.5 while the global
        // recommended tracks 3.9.1), that named a version
        // `refuse_unsupported_target` then turns around and refuses —
        // a button whose only possible outcome is an error message.
        //
        // Exercised via the pure `_for_platform` half so this is
        // provable for "windows-aarch64" regardless of which platform
        // actually runs this test suite.
        let window = support_window();
        assert!(
            window.platform_ceilings.contains_key("windows-aarch64"),
            "this test only proves something if windows-aarch64 is genuinely held back"
        );

        for (installed, use_wrapper) in [
            (None, true),
            (None, false),
            (Some("2.9.3"), false),
            (Some(window.minimum.as_str()), true),
            (Some(window.minimum.as_str()), false),
        ] {
            let target =
                recommended_upgrade_target_for_platform(installed, use_wrapper, "windows-aarch64");
            assert!(
                is_version_at_least(&effective_maximum_tested("windows-aarch64"), &target),
                "recommended_upgrade_target_for_platform(installed={installed:?}, \
                 use_wrapper={use_wrapper}) returned {target}, which windows-aarch64's own \
                 install path cannot install"
            );
            // And it must actually equal what the platform-aware helper
            // says is installable there — not merely "some version
            // below the ceiling" by coincidence.
            assert_eq!(target, effective_maximum_tested("windows-aarch64"));
        }
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
        // Two platforms in the shipped tool-versions.toml are held
        // below the general ceiling: "linux-armv7" (#1014 — GAMDL
        // itself publishes nothing for it from 3.8.2) and
        // "windows-aarch64" (GAMDL 3.9+ needs a package that has no
        // Windows ARM64 build). Every OTHER platform must see the
        // general ceiling unchanged.
        for platform_id in ["macos", "windows-x86_64", "linux-x86_64", "linux-aarch64"] {
            assert_eq!(
                effective_maximum_tested(platform_id),
                support_window().maximum_tested,
                "{platform_id}: expected no override, got a different effective ceiling"
            );
        }
    }

    #[test]
    fn effective_maximum_tested_uses_windows_arm_override_when_present() {
        // Mirrors the ARMv7 test below: tolerant of the entry being
        // removed one day (if `pyplayready` relaxes its requirement, or
        // a Windows ARM64 build of what it needs appears), but while it
        // IS there it must be honoured and must actually be lower than
        // the general ceiling — an override equal to the general
        // ceiling would be a no-op pretending to be a safeguard.
        let window = support_window();
        if let Some(ceiling) = window.platform_ceilings.get("windows-aarch64") {
            assert_eq!(&effective_maximum_tested("windows-aarch64"), ceiling);
            assert_ne!(ceiling, &window.maximum_tested);
            assert!(
                is_version_at_least(&window.maximum_tested, ceiling),
                "the Windows-on-ARM ceiling ({ceiling}) must be BELOW the general one ({}), \
                 not above it",
                window.maximum_tested
            );
        }
    }

    #[test]
    fn classify_for_platform_windows_arm_is_untested_at_the_general_ceiling() {
        // A Windows-on-ARM machine running the general ceiling version
        // is in untested territory even though the same version is
        // perfectly ordinary everywhere else. This is what the startup
        // activity-log line reads from.
        let window = support_window();
        let Some(ceiling) = window.platform_ceilings.get("windows-aarch64") else {
            return; // Entry removed — nothing to assert (see above).
        };

        let at_own_ceiling = classify_for_platform(Some(ceiling), "windows-aarch64");
        assert!(
            at_own_ceiling.is_supported(),
            "Windows on ARM at its own ceiling ({ceiling}) must be Supported, got \
             {at_own_ceiling:?}"
        );

        let at_general = classify_for_platform(Some(&window.maximum_tested), "windows-aarch64");
        assert!(
            matches!(at_general, VersionSupport::Untested { .. }),
            "the general ceiling ({}) is above the Windows-on-ARM ceiling ({ceiling}), so it \
             must classify as Untested there, got {at_general:?}",
            window.maximum_tested
        );
        assert!(
            classify(Some(&window.maximum_tested)).is_supported(),
            "the same version must still be Supported on the general window"
        );
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
