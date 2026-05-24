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
            // Added in v3.1, REMOVED in v3.6 (replaced by wrapper-v2 single URL).
            Self::WrapperM3u8Ip => {
                is_version_at_least(version, "3.1") && !is_version_at_least(version, "3.6")
            }
            // Added in v3.0 — v2.9.x rejects it at CLI parse time.
            Self::PlaylistFolderTemplate => is_version_at_least(version, "3.0"),
            // No-op starting v3.1 — flag is accepted but ignored.
            Self::NoExceptionsFlag => !is_version_at_least(version, "3.1"),
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
        (GamdlFeature::FetchExtraTags, "fetch_extra_tags"),
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
        assert!(!supports(GamdlFeature::PlaylistFolderTemplate));
        assert!(!supports(GamdlFeature::NoExceptionsFlag));
    }

    #[test]
    fn playlist_folder_template_requires_v30() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
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
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(Some("3.5.0".to_string()));
        let summary = active_capabilities_summary();
        // v3.5 supports: NativeCodecPriority, PlaylistFolderTemplate,
        // WrapperM3u8Ip. Does NOT support FetchExtraTags (removed in 3.0)
        // or NoExceptionsFlag (no-op since 3.1).
        assert!(summary.contains("native_codec_priority"));
        assert!(summary.contains("playlist_folder_template"));
        assert!(summary.contains("wrapper_m3u8_ip"));
        assert!(!summary.contains("fetch_extra_tags"));
        assert!(!summary.contains("no_exceptions_flag"));
        set_detected_version(None);
    }

    #[test]
    fn active_capabilities_summary_unknown_when_uncached() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(None);
        assert_eq!(active_capabilities_summary(), "unknown");
    }

    #[test]
    fn active_capabilities_summary_lists_v2x_features() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        set_detected_version(Some("2.9.3".to_string()));
        let summary = active_capabilities_summary();
        // v2.9.3 supports: NativeCodecPriority, FetchExtraTags,
        // NoExceptionsFlag. Does NOT support PlaylistFolderTemplate
        // (added in 3.0) or WrapperM3u8Ip (added in 3.1).
        assert!(summary.contains("native_codec_priority"));
        assert!(summary.contains("fetch_extra_tags"));
        assert!(summary.contains("no_exceptions_flag"));
        assert!(!summary.contains("playlist_folder_template"));
        assert!(!summary.contains("wrapper_m3u8_ip"));
        set_detected_version(None);
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

    // -- GAMDL v3.6 capability gates (#853) -------------------------------

    #[test]
    fn wrapper_url_requires_v36() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
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
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
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
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
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
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
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
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
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
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());

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
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());

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
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());

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
}
