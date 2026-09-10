// Copyright (c) 2024-2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Centralised reqwest::Client construction.
// ===========================================
//
// Codebase audit (#716 finding #2) identified 13+ `reqwest::Client::builder()`
// instances across services/ + utils/ + commands/, each rebuilding the
// builder with a timeout and constructing the same error message
// ("Failed to create HTTP client: {e}") on failure. Most use a 5-30 sec
// request timeout; one uses connect_timeout instead (archive.rs, where
// large binary downloads need an unbounded read budget but a short
// connect window); two add a User-Agent header in the builder.
//
// This module provides a single `build_client(ClientConfig)` primitive
// plus convenience wrappers for the two canonical configurations.
// Future emission rules — retry policies, request logging, redaction
// of sensitive headers, multi-service rate-limiting — only need to
// touch this module.
//
// Migration is opt-in per callsite (#716 follow-ups). The helper does
// NOT change behaviour for any existing call: the timeout, UA, and
// error-message string match what the existing sites produce.

use std::sync::LazyLock;
use std::time::Duration;

/// User-Agent string identifying MeedyaDL by name. This is Group A of the
/// four-way outbound UA policy (see `.claude/CLAUDE.md`'s "Outbound
/// User-Agent" bullet for the full table): sent to (1) first-party
/// endpoints — anything belonging to MWBM Partners / MWBM Partners Ltd /
/// MeedyaSuite / Meedya / Scriptkey, including the `MWBMPartners`,
/// `Skriptey`, and `MeedyaSuite` GitHub orgs (e.g. `service_status.rs`'s
/// `raw.githubusercontent.com/MWBMPartners/...` reads) — and (2)
/// integrations that specifically require identification: the GitHub API
/// (GitHub's own guidance asks integrations to identify themselves, and
/// it's the channel GitHub uses to contact maintainers of a misbehaving
/// integration rather than silently blocking it) and MusicBrainz (a
/// licensed API whose Terms of Service require an accurate, identifying
/// UA). Every other third party gets a different UA — see
/// [`SAFARI_MACOS_USER_AGENT`] for Apple Music specifically, and
/// [`browser_user_agent()`] for everything else (Odesli, PyPI, generic
/// asset downloads).
///
/// This is a compile-time `const` (not `full_user_agent()` below) precisely
/// because these destinations get NO platform detail: OS/arch/OS-version is
/// a fingerprinting surface that buys none of them anything beyond the
/// identity they actually need, so it is deliberately omitted here. See
/// `full_user_agent()` for the platform-bearing counterpart, reserved for
/// MeedyaDL's own backend (MWBM-IntAppsAPI).
///
/// Two things this constant fixes by construction:
///
/// (a) The version is resolved at compile time via `env!("CARGO_PKG_VERSION")`
///     so it can never drift out of sync with the shipped app version — the
///     bug this constant replaces was a hardcoded `MusicBrainz` UA pinned at
///     "0.6" while the app itself had moved on to the 1.12.x line.
/// (b) The `(+https://...)` comment token satisfies MusicBrainz's Terms of
///     Service requirement that outbound UAs identify the application and
///     provide a way to reach its maintainers — the `+URL` convention (as
///     used by e.g. Googlebot's UA string) marks the parenthesised segment
///     as a contact/info link rather than a platform-detail comment.
pub const APP_USER_AGENT: &str = concat!(
    "MeedyaDL/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/MWBMPartners/MeedyaDL)"
);

/// Fixed macOS Safari User-Agent string reserved for **Apple's own
/// endpoints only** (`apple_music_api.rs`, `commands/credentials.rs`,
/// `animated_artwork_service.rs`'s `ffmpeg -user_agent`). This is
/// deliberately sent regardless of the host OS — a Windows, Linux or
/// Raspberry Pi install still presents as macOS Safari here.
///
/// WHY THIS IS CROSS-PLATFORM ON PURPOSE — DO NOT "FIX" IT.
///
/// There are two reasons, and the second is the important one that keeps
/// getting forgotten:
///
/// 1. It is the identity Apple Music's own servers expect, so it gets a
///    normal response rather than a refusal.
///
/// 2. Safari implies a Mac, and Apple Music serves its FULLEST feature set
///    to a Mac running Safari. Presenting as anything else — even something
///    genuine and platform-appropriate like Chrome on Windows — can mean a
///    reduced experience: fewer or lower-quality assets and features that
///    simply are not offered to a non-Apple client. Sending Safari from
///    every platform is how a Windows or Raspberry Pi user gets the same
///    Apple Music experience a Mac user does.
///
/// So this is NOT accidental impersonation left over from an earlier
/// design, and it is NOT an oversight that Windows and Linux do not use
/// `browser_user_agent()` here. Making this "honest per platform" would
/// quietly degrade the app for every non-Mac user. If you are reading this
/// because a lint, an audit or an instinct says the user agent should match
/// the host operating system: it should not, for Apple Music, and this
/// paragraph is why. See issue #1072.
///
/// The VERSION NUMBER in the string is a separate matter and does need
/// keeping current — see [`SAFARI_VERSION`] further down for how it is kept
/// current. Do not reach for this constant outside the Apple Music paths; every
/// other "needs to look like a real browser" call site wants
/// [`browser_user_agent()`] instead, which is genuine for the host OS.
///
/// This was previously named `APPLE_BROWSER_USER_AGENT` and lived in
/// `apple_music_api.rs` as an Apple-specific impersonation string, then
/// briefly promoted (as `BROWSER_USER_AGENT`) to a shared third-party-wide
/// constant sent to every non-MusicBrainz destination. The maintainer
/// refined that policy: GitHub went back to identifying itself via
/// `APP_USER_AGENT` (GitHub's own guidance asks integrations to identify
/// themselves, and it's the channel GitHub uses to contact maintainers of a
/// misbehaving integration rather than silently blocking it), and every
/// remaining non-Apple, non-first-party destination now gets a
/// platform-appropriate browser UA via `browser_user_agent()` instead of
/// this fixed Safari string — a Safari UA arriving from a Windows host is
/// itself an implausible, anomaly-signalling combination. This constant is
/// scoped down to what it always should have been: Apple Music's own
/// storefront, which genuinely expects Safari regardless of host OS. The
/// Safari string value itself is unchanged across all of these moves,
/// except for the `Version/…` number — see [`SAFARI_VERSION`] below for how
/// that one number is kept current while every other byte of this string
/// stays exactly as written here.
pub static SAFARI_MACOS_USER_AGENT: LazyLock<String> = LazyLock::new(|| {
    format!(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{} Safari/605.1.15",
        sanitise_safari_version(SAFARI_VERSION)
    )
});

/// Returns a genuine, platform-appropriate browser User-Agent string for
/// the current host OS. Used by every third party that isn't first-party,
/// isn't an identification-required integration (GitHub, MusicBrainz), and
/// isn't Apple Music (which always gets [`SAFARI_MACOS_USER_AGENT`]
/// regardless of host OS) — currently Odesli and PyPI.
///
/// Selection is by **OS family only, never architecture**: real Chrome
/// running on Windows-on-ARM or on ARM Linux commonly still reports the
/// x64/x86_64 UA token anyway (Chrome's own UA-reduction policy collapses
/// architecture detail), so branching on `std::env::consts::ARCH` here
/// would be both less genuine (most real ARM browsers don't do it either)
/// and needless extra fingerprint surface for no accuracy gain.
///
/// A macOS host gets the same fixed Safari string as
/// [`SAFARI_MACOS_USER_AGENT`] — on macOS, "genuine browser" and "the
/// Apple-specific string" happen to coincide. Windows and Linux hosts get a
/// genuine desktop Chrome UA for their platform; any other/unknown OS falls
/// back to the Linux string (closest generic "desktop browser on some
/// Unix-like" shape).
///
/// **Maintenance note**: the Chrome build number embedded in the Windows
/// and Linux strings below is resolved at packaging time — see
/// [`CHROME_MAJOR`] and its surrounding constants for the mechanism.
pub fn browser_user_agent() -> &'static str {
    match std::env::consts::OS {
        "macos" => SAFARI_MACOS_USER_AGENT.as_str(),
        "windows" => WINDOWS_CHROME_UA.as_str(),
        // "linux" and every other/unknown OS share the same generic
        // desktop-Linux Chrome string — see the doc comment above.
        _ => LINUX_CHROME_UA.as_str(),
    }
}

/// Fallback Chrome major version number, compiled in for every build that
/// doesn't inject [`MEEDYADL_CHROME_MAJOR`](CHROME_MAJOR) at packaging time
/// — i.e. every local dev build, every fork build, and every CI/PR build
/// (`ci.yml` deliberately never sets the env var; see that workflow for
/// why determinism/cache-hygiene wins there). Bump this by hand
/// periodically to track a recent stable Chrome release; it only matters
/// for builds that never talk to the packaging-time resolver.
///
/// A stale value here is not merely untidy: an aged Chrome major is itself
/// an anomaly signal, which defeats the whole point of sending a genuine
/// browser UA. Last refreshed 2026-07-27 against the stable channel.
const CHROME_MAJOR_FALLBACK: &str = "151";

/// The Chrome major version number (e.g. `"131"`) baked into
/// [`browser_user_agent()`]'s Windows and Linux strings. This is a
/// **build-time, OS-agnostic, major-only** injection point — three
/// deliberate narrowings versus "just refresh the UA strings periodically":
///
/// 1. **Major only, never a full version string.** Chrome's own UA
///    reduction policy (shipped fleet-wide since Chrome 110) freezes the
///    minor/build/patch segments at `.0.0.0` in the UA header regardless of
///    the browser's real build — a real-looking full version there would
///    actually look *less* genuine than the frozen shape every real Chrome
///    install now sends. So there is nothing to gain by resolving more than
///    the major, and resolving more would be actively counter-signal.
/// 2. **One number for every desktop platform, never per-OS.** The Chrome
///    major release train is identical across Windows/macOS/Linux — Google
///    ships one version number fleet-wide — so a single build-time constant
///    correctly serves both [`WINDOWS_CHROME_UA`] and [`LINUX_CHROME_UA`].
///    Nothing platform-specific is ever injected: `std::env::consts::OS` is
///    compile-time-constant per target and the platform *tokens*
///    ("Windows NT 10.0", "X11; Linux x86_64") stay hardcoded Rust string
///    literals, never sourced from an env var. This makes it structurally
///    impossible for a Windows build to ever ship a macOS (or any other
///    platform's) UA token — there is no code path where OS selection and
///    version injection could cross-contaminate, because they're two
///    entirely separate mechanisms (a `match` on a compile-time constant,
///    and a `LazyLock<String>` format!).
/// 3. **Absent injection is a fully valid, zero-config, zero-network
///    state**, not a degraded one. `option_env!` reads a variable that may
///    not exist at compile time; when it doesn't (every local dev build,
///    every fork, every CI/PR build), this resolves to
///    [`CHROME_MAJOR_FALLBACK`] with no build-script, no network call, and
///    no behavioural difference beyond which digits appear in the UA
///    string. Only `release.yml`'s packaging step (see that workflow) ever
///    sets `MEEDYADL_CHROME_MAJOR`, and it does so best-effort — a failed
///    fetch there simply leaves the env var unset, which lands right back
///    on this same fallback.
///
/// [`SAFARI_MACOS_USER_AGENT`] is deliberately **not** wired into this same
/// network-fetch mechanism — it has its own, described at [`SAFARI_VERSION`]
/// below, which resolves the version a different way for a reason that
/// still stands: that string is the Group B UA Apple Music's own edges must
/// accept, and a broken/rate-limited packaging-time network request could
/// ship a build that Apple Music's servers reject outright, which is far
/// too high a blast radius to accept for what a network fetch would buy.
const CHROME_MAJOR: &str = match option_env!("MEEDYADL_CHROME_MAJOR") {
    Some(v) => v,
    None => CHROME_MAJOR_FALLBACK,
};

/// Defence-in-depth validation for [`CHROME_MAJOR`]. `option_env!` reads
/// whatever string a CI workflow happened to put in the environment at
/// compile time — normally a clean 2-4 digit number resolved from Google's
/// VersionHistory API (see `release.yml`), but this function exists so that
/// even a bad workflow edit (a typo, an unexpected API response shape, an
/// accidental multi-line value) can never make it into a shipped UA string.
/// A UA header containing a malformed "version" is a worse anomaly signal
/// than simply falling back to the last-known-good compiled-in major, so on
/// any validation failure this silently substitutes
/// [`CHROME_MAJOR_FALLBACK`] rather than propagating the bad value or
/// panicking.
fn sanitise_chrome_major(raw: &'static str) -> &'static str {
    let ok = (2..=4).contains(&raw.len()) && raw.bytes().all(|b| b.is_ascii_digit());
    if ok { raw } else { CHROME_MAJOR_FALLBACK }
}

/// Fallback Safari version number, compiled in for every build that doesn't
/// inject [`MEEDYADL_SAFARI_VERSION`](SAFARI_VERSION) at packaging time —
/// i.e. every local dev build, every fork build, and every CI/PR build (only
/// `release.yml`'s macOS job ever sets the env var — see [`SAFARI_VERSION`]
/// for why). Bump this by hand from time to time so it tracks a real,
/// current Safari release; it only matters for builds that never go through
/// that macOS job.
///
/// A stale value here is not merely untidy — an old Safari version number is
/// itself an anomaly signal, which undercuts the entire reason
/// [`SAFARI_MACOS_USER_AGENT`] exists (see that constant's doc comment for
/// why presenting as Safari matters in the first place). Last refreshed
/// 2026-09-08 against a real installed Safari (26.6.2) on the maintainer's
/// Mac; kept here as the major-and-minor pair only, matching the shape most
/// real Safari installs report.
const SAFARI_VERSION_FALLBACK: &str = "26.6";

/// The Safari version string (e.g. `"26.6"` or `"26.6.2"`) baked into
/// [`SAFARI_MACOS_USER_AGENT`]'s `Version/…` field. Same `option_env!`-with-
/// compiled-in-fallback shape as [`CHROME_MAJOR`], but resolved a
/// deliberately DIFFERENT way — this is the one place in the four-way UA
/// policy where the two mechanisms diverge on purpose, so read this
/// alongside [`CHROME_MAJOR`]'s doc comment rather than assuming they match:
///
/// 1. **No network call, ever.** Chrome's number is fetched live from
///    Google's public VersionHistory API because a stale Chrome major is a
///    low-stakes anomaly signal at worst. Safari is different: a failed,
///    timed-out, or rate-limited network fetch at packaging time could ship
///    a build carrying a broken or nonsensical Apple Music identity, and
///    Apple Music's own edges are the one destination this app genuinely
///    cannot afford to be turned away by. That objection is on record (see
///    [`SAFARI_MACOS_USER_AGENT`]'s doc comment) and this mechanism honours
///    it by never touching the network for this value.
/// 2. **Read off the actual build machine instead.** `release.yml` runs
///    `plutil -extract CFBundleShortVersionString raw -o - \
///    /Applications/Safari.app/Contents/Info.plist` on the macOS release
///    runner — reading the version of the real Safari that ships with that
///    runner's own macOS image, not a guess and not a fetch — and exports
///    the result as `MEEDYADL_SAFARI_VERSION`.
/// 3. **Resolved once, shared with every platform's build.** Only a macOS
///    machine can read a Safari version at all, but this string ships in
///    every platform's build (Windows and Linux included — see
///    [`SAFARI_MACOS_USER_AGENT`]'s doc comment for why). So `release.yml`
///    resolves it exactly once, in a small `resolve-safari-version` job that
///    runs on a macOS runner ahead of the per-platform build matrix, and
///    passes the result down to every platform's build job as a job output
///    — the same "resolve once, fan out" shape `MEEDYADL_CHROME_MAJOR` would
///    need if Chrome's number were platform-exclusive too, except Chrome's
///    happens to be fetchable identically from any OS so it never needed
///    this extra job.
///
/// NEVER-FAIL CONTRACT, identical in spirit to Chrome's: if the read fails
/// for any reason — Safari absent from the runner image, an unexpected
/// plist shape, the job output not making it to a platform's job — this
/// resolves to [`SAFARI_VERSION_FALLBACK`] with no build-script requirement,
/// no build failure, and no behavioural difference beyond which digits
/// appear in the UA string. [`sanitise_safari_version()`] is the second line
/// of defence for a value that arrived non-empty but malformed.
const SAFARI_VERSION: &str = match option_env!("MEEDYADL_SAFARI_VERSION") {
    Some(v) => v,
    None => SAFARI_VERSION_FALLBACK,
};

/// Defence-in-depth validation for [`SAFARI_VERSION`], mirroring
/// [`sanitise_chrome_major()`]. Accepts the two shapes Safari's own
/// `CFBundleShortVersionString` actually uses — `"26.6"` (major.minor) or
/// `"26.6.2"` (major.minor.patch), each component 1-3 digits — and
/// substitutes [`SAFARI_VERSION_FALLBACK`] for anything else (empty,
/// non-numeric, wrong number of parts, an implausibly long component), so a
/// bad workflow edit or an unexpected plist read can never reach a shipped
/// UA string.
fn sanitise_safari_version(raw: &'static str) -> &'static str {
    let parts: Vec<&str> = raw.split('.').collect();
    let ok = (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()));
    if ok { raw } else { SAFARI_VERSION_FALLBACK }
}

/// Lazily-built Windows Chrome User-Agent string. Built once per process;
/// see [`CHROME_MAJOR`] for how the embedded major version is resolved and
/// [`browser_user_agent()`]'s doc comment for why Windows presents as
/// Chrome rather than Edge: Chrome's real-world share on Windows is several
/// times Edge's, making it the less remarkable (more genuine-looking)
/// client to present as, and Edge's UA string is Chrome's plus an
/// additional `Edg/<version>` token — strictly more identifying, and a
/// second version number this module would have to keep in sync for no
/// benefit.
static WINDOWS_CHROME_UA: LazyLock<String> = LazyLock::new(|| {
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36",
        sanitise_chrome_major(CHROME_MAJOR)
    )
});

/// Lazily-built Linux Chrome User-Agent string — see [`WINDOWS_CHROME_UA`]
/// for the shared major-version-resolution mechanism. Used for Linux hosts
/// and as the generic fallback for any other/unknown host OS (see
/// [`browser_user_agent()`]).
static LINUX_CHROME_UA: LazyLock<String> = LazyLock::new(|| {
    format!(
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36",
        sanitise_chrome_major(CHROME_MAJOR)
    )
});

/// Lazily-built, platform-bearing User-Agent string. Built once per process
/// (subsequent calls just dereference the cached `String`) because
/// `tauri_plugin_os::version()` does a small amount of OS work that has no
/// reason to repeat on every call — the platform can't change mid-process.
///
/// Format: `"MeedyaDL/{version} ({OSName} {Arch}/{OSVersion})"`, e.g.
/// `"MeedyaDL/1.12.0-alpha.42 (MacOS ARM64/26.6)"`.
///
/// **Not** for third-party requests — see `APP_USER_AGENT`'s doc comment
/// for why platform detail is a fingerprinting surface that has no business
/// leaving the machine except to an endpoint MeedyaDL itself controls.
/// Reserved for the future MWBM-IntAppsAPI client (no caller in this
/// package yet); the platform detail lets that backend do coarse
/// OS/arch-targeted diagnostics and rollout decisions.
static APP_USER_AGENT_FULL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "MeedyaDL/{} ({} {}/{})",
        env!("CARGO_PKG_VERSION"),
        os_name(std::env::consts::OS),
        arch_name(std::env::consts::ARCH),
        tauri_plugin_os::version(),
    )
});

/// Returns the platform-bearing User-Agent string described on
/// [`APP_USER_AGENT_FULL`]. A reference into a `static LazyLock<String>` is
/// itself `&'static str` (the backing `String`'s allocation lives for the
/// process lifetime), so this slots into `ClientConfig::user_agent:
/// Option<&'static str>` with no shape change to `ClientConfig` needed.
pub fn full_user_agent() -> &'static str {
    APP_USER_AGENT_FULL.as_str()
}

/// Maps `std::env::consts::OS` to a closed, human-readable vocabulary for
/// the platform-bearing UA. Deliberately closed (not a passthrough of the
/// raw Rust target-triple OS string, and never a Linux distro name) so the
/// UA's shape stays predictable for whatever parses it downstream.
fn os_name(os: &str) -> &'static str {
    match os {
        "macos" => "MacOS",
        "windows" => "Windows",
        "linux" => "Linux",
        _ => "Unknown",
    }
}

/// Maps `std::env::consts::ARCH` to the short architecture label used in
/// the platform-bearing UA. Unknown architectures fall back to the raw
/// `std::env::consts::ARCH` value rather than "Unknown" — unlike `os_name`,
/// an arch we don't have a friendly label for is still useful raw (e.g. a
/// future `riscv64`), whereas an unrecognised OS string usually indicates
/// something has gone wrong upstream in `std::env::consts::OS` itself.
fn arch_name(arch: &str) -> &str {
    match arch {
        "aarch64" => "ARM64",
        "x86_64" => "x64",
        "arm" => "ARMv7",
        other => other,
    }
}

/// Construction parameters for an HTTP client. Defaults match the
/// most-common shape (15-second request timeout, no user-agent, no
/// connect_timeout override) — the caller only specifies fields that
/// differ from default.
///
/// Use `ClientConfig::default()` directly when 15s + no UA is fine.
/// Use the builder-style setters when finer control is needed.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Total request timeout (from connect through response body).
    /// Defaults to 15 seconds — the value most service sites use.
    /// Set to a longer duration for streaming downloads (mirror the
    /// `archive.rs` convention) or a shorter one for health checks.
    pub timeout: Duration,
    /// Optional connect-only timeout. When `Some`, separate from the
    /// total `timeout` and used for the initial TCP/TLS handshake.
    /// `archive.rs` uses this with `timeout = unbounded` so large
    /// binary downloads don't time out mid-stream on slow links.
    pub connect_timeout: Option<Duration>,
    /// Optional User-Agent header. Required by some APIs (MusicBrainz
    /// rate-limits non-UA requests; AcoustID requires a registered
    /// app name). Note: reqwest's default UA is a generic "reqwest/X.Y";
    /// setting an explicit UA is good citizenship for our outbound
    /// requests regardless.
    pub user_agent: Option<&'static str>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            connect_timeout: None,
            user_agent: None,
        }
    }
}

impl ClientConfig {
    /// Constructs a config with a custom request timeout.
    #[must_use]
    pub const fn with_timeout(secs: u64) -> Self {
        Self {
            timeout: Duration::from_secs(secs),
            connect_timeout: None,
            user_agent: None,
        }
    }

    /// Adds a User-Agent header to the config.
    #[must_use]
    pub const fn user_agent(mut self, ua: &'static str) -> Self {
        self.user_agent = Some(ua);
        self
    }

    /// Adds a connect-only timeout (separate from the total request
    /// timeout). Used by streaming-download paths where the read
    /// budget is unbounded but the initial handshake should fail fast.
    #[must_use]
    pub const fn connect_timeout(mut self, secs: u64) -> Self {
        self.connect_timeout = Some(Duration::from_secs(secs));
        self
    }
}

/// Builds a `reqwest::Client` from a [`ClientConfig`]. Returns the
/// canonical error message used across the codebase
/// (`"Failed to create HTTP client: {e}"`) so existing call sites can
/// be migrated without touching their error-handling.
pub fn build_client(cfg: ClientConfig) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().timeout(cfg.timeout);
    if let Some(ct) = cfg.connect_timeout {
        builder = builder.connect_timeout(ct);
    }
    if let Some(ua) = cfg.user_agent {
        builder = builder.user_agent(ua);
    }
    builder
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))
}

/// Convenience wrapper for the most-common case: a request timeout
/// in seconds, no UA, no connect_timeout. Matches what 9+ of the
/// 13+ existing call sites do.
pub fn build_simple(timeout_secs: u64) -> Result<reqwest::Client, String> {
    build_client(ClientConfig::with_timeout(timeout_secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_15_second_timeout_no_ua() {
        let cfg = ClientConfig::default();
        assert_eq!(cfg.timeout, Duration::from_secs(15));
        assert_eq!(cfg.connect_timeout, None);
        assert_eq!(cfg.user_agent, None);
    }

    #[test]
    fn with_timeout_sets_only_timeout() {
        let cfg = ClientConfig::with_timeout(30);
        assert_eq!(cfg.timeout, Duration::from_secs(30));
        assert_eq!(cfg.user_agent, None);
        assert_eq!(cfg.connect_timeout, None);
    }

    #[test]
    fn builder_chains_compose() {
        let cfg = ClientConfig::with_timeout(10)
            .user_agent(APP_USER_AGENT)
            .connect_timeout(5);
        assert_eq!(cfg.timeout, Duration::from_secs(10));
        assert_eq!(cfg.user_agent, Some(APP_USER_AGENT));
        assert_eq!(cfg.connect_timeout, Some(Duration::from_secs(5)));
    }

    #[test]
    fn build_simple_succeeds_with_a_reasonable_timeout() {
        let client = build_simple(15);
        assert!(
            client.is_ok(),
            "build_simple(15) should succeed: {:?}",
            client.err()
        );
    }

    #[test]
    fn build_client_with_full_config_succeeds() {
        let cfg = ClientConfig::with_timeout(30)
            .user_agent(APP_USER_AGENT)
            .connect_timeout(5);
        assert!(build_client(cfg).is_ok());
    }

    #[test]
    fn app_user_agent_has_the_expected_shape() {
        // (a) the prefix MWBM-IntAppsAPI's AuthMiddleware matches against.
        assert!(APP_USER_AGENT.starts_with("MeedyaDL/"));
        // (b) the version is compile-time-derived, never hardcoded/stale.
        assert!(APP_USER_AGENT.contains(env!("CARGO_PKG_VERSION")));
        // (c) an identifying contact URL, per MusicBrainz's ToS requirement.
        assert!(APP_USER_AGENT.contains("https://"));
    }

    #[test]
    fn safari_macos_user_agent_looks_like_genuine_macos_safari() {
        // Apple Music always gets a macOS Safari UA regardless of host OS
        // — see the constant's doc comment.
        assert!(SAFARI_MACOS_USER_AGENT.contains("Macintosh"));
        assert!(SAFARI_MACOS_USER_AGENT.contains("Safari"));
        // The version number is the one part of the string that's expected
        // to change over time (see SAFARI_VERSION) — everything else must
        // stay byte-identical to what a real macOS Safari sends.
        assert!(SAFARI_MACOS_USER_AGENT.starts_with(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/"
        ));
        assert!(SAFARI_MACOS_USER_AGENT.ends_with(" Safari/605.1.15"));
    }

    #[test]
    fn safari_version_fallback_is_well_formed() {
        // The compiled-in fallback is what every local/fork/CI build (and
        // any non-macOS release job) actually ships, so it must
        // independently satisfy the same shape sanitise_safari_version()
        // enforces on the (possibly build-injected) value.
        assert_eq!(
            sanitise_safari_version(SAFARI_VERSION_FALLBACK),
            SAFARI_VERSION_FALLBACK
        );
    }

    #[test]
    fn sanitise_safari_version_accepts_valid_and_rejects_garbage() {
        // Real Safari CFBundleShortVersionString shapes: major.minor and
        // major.minor.patch both occur in the wild.
        assert_eq!(sanitise_safari_version("26.6"), "26.6");
        assert_eq!(sanitise_safari_version("26.6.2"), "26.6.2");
        assert_eq!(sanitise_safari_version("9.1"), "9.1");
        // Anything else — empty, a single component, too many components,
        // non-numeric, an absurdly long component — falls back to the
        // compiled-in constant rather than propagating a malformed value
        // into a shipped UA string.
        assert_eq!(sanitise_safari_version(""), SAFARI_VERSION_FALLBACK);
        assert_eq!(sanitise_safari_version("26"), SAFARI_VERSION_FALLBACK);
        assert_eq!(sanitise_safari_version("26.6.2.1"), SAFARI_VERSION_FALLBACK);
        assert_eq!(sanitise_safari_version("26.x"), SAFARI_VERSION_FALLBACK);
        assert_eq!(sanitise_safari_version("26."), SAFARI_VERSION_FALLBACK);
        assert_eq!(sanitise_safari_version("2600.6"), SAFARI_VERSION_FALLBACK);
    }

    #[test]
    fn browser_user_agent_is_a_genuine_mozilla_ua() {
        let ua = browser_user_agent();
        assert!(ua.starts_with("Mozilla/5.0"));
    }

    #[test]
    fn browser_user_agent_is_platform_appropriate_on_this_build_target() {
        let ua = browser_user_agent();
        // cfg! reads the actual compile target, so this assertion is
        // meaningful (not vacuously true) on every platform CI builds for.
        if cfg!(target_os = "macos") {
            assert!(ua.contains("Macintosh"), "expected a macOS token: {ua}");
        } else if cfg!(target_os = "windows") {
            assert!(ua.contains("Windows"), "expected a Windows token: {ua}");
        } else if cfg!(target_os = "linux") {
            assert!(ua.contains("Linux"), "expected a Linux token: {ua}");
        }
    }

    #[test]
    fn full_user_agent_has_the_expected_shape() {
        let ua = full_user_agent();
        // Same "MeedyaDL/" + version prefix as the reduced constant.
        assert!(ua.starts_with("MeedyaDL/"));
        assert!(ua.contains(env!("CARGO_PKG_VERSION")));
        // Platform detail is wrapped in "(OSName Arch/OSVersion)".
        assert!(ua.contains('('));
        assert!(ua.contains('/'));
        assert!(ua.contains(')'));
        // Deliberately NOT a contact URL — this string never leaves a
        // MeedyaDL-controlled endpoint, so no `+URL` contact token is
        // needed, and platform detail must never carry the third-party
        // contact convention that would make it look like APP_USER_AGENT.
        assert!(!ua.contains("https://"));
    }

    #[test]
    fn os_name_maps_the_closed_vocabulary() {
        assert_eq!(os_name("macos"), "MacOS");
        assert_eq!(os_name("windows"), "Windows");
        assert_eq!(os_name("linux"), "Linux");
        assert_eq!(os_name("freebsd"), "Unknown");
        assert_eq!(os_name(""), "Unknown");
    }

    #[test]
    fn arch_name_maps_known_architectures_and_passes_through_unknown() {
        assert_eq!(arch_name("aarch64"), "ARM64");
        assert_eq!(arch_name("x86_64"), "x64");
        assert_eq!(arch_name("arm"), "ARMv7");
        // Unknown architectures pass through raw rather than becoming
        // "Unknown" — see the doc comment on arch_name for why this
        // differs from os_name's fallback behaviour.
        assert_eq!(arch_name("riscv64"), "riscv64");
    }

    #[test]
    fn chrome_major_fallback_is_well_formed() {
        // The compiled-in fallback is what every local/fork/CI build
        // actually ships, so it must independently satisfy the same shape
        // sanitise_chrome_major() enforces on the (possibly build-injected)
        // value.
        assert!((2..=4).contains(&CHROME_MAJOR_FALLBACK.len()));
        assert!(CHROME_MAJOR_FALLBACK.bytes().all(|b| b.is_ascii_digit()));
    }

    #[test]
    fn sanitise_chrome_major_accepts_valid_and_rejects_garbage() {
        // Valid 2-4 digit numeric strings pass through unchanged.
        assert_eq!(sanitise_chrome_major("131"), "131");
        assert_eq!(sanitise_chrome_major("99"), "99");
        assert_eq!(sanitise_chrome_major("1310"), "1310");
        // Anything else — empty, too short, non-numeric, decimal-pointed,
        // too long — falls back to the compiled-in constant rather than
        // propagating a malformed value into a shipped UA string.
        assert_eq!(sanitise_chrome_major(""), CHROME_MAJOR_FALLBACK);
        assert_eq!(sanitise_chrome_major("1"), CHROME_MAJOR_FALLBACK);
        assert_eq!(sanitise_chrome_major("13x"), CHROME_MAJOR_FALLBACK);
        assert_eq!(sanitise_chrome_major("131.0"), CHROME_MAJOR_FALLBACK);
        assert_eq!(sanitise_chrome_major("13111"), CHROME_MAJOR_FALLBACK);
    }

    #[test]
    fn chrome_uas_have_the_reduced_version_shape() {
        // Assert SHAPE only, never a specific major number — pinning a
        // literal major here would be a calendar bomb that breaks the day
        // MEEDYADL_CHROME_MAJOR (or the compiled-in fallback) is bumped.
        for ua in [WINDOWS_CHROME_UA.as_str(), LINUX_CHROME_UA.as_str()] {
            assert!(ua.starts_with("Mozilla/5.0"), "unexpected prefix: {ua}");
            assert!(ua.ends_with("Safari/537.36"), "unexpected suffix: {ua}");

            // Chrome's UA-reduction policy freezes minor/build/patch at
            // ".0.0.0" — extract the "Chrome/<version>" token and assert
            // its shape is "<digits>.0.0.0".
            let chrome_token = ua
                .split("Chrome/")
                .nth(1)
                .expect("UA must contain a Chrome/ token")
                .split(' ')
                .next()
                .expect("Chrome/ token must be followed by a version");
            let mut parts = chrome_token.split('.');
            let major = parts.next().expect("version must have a major segment");
            assert!(
                !major.is_empty() && major.bytes().all(|b| b.is_ascii_digit()),
                "major segment should be all-digits: {chrome_token}"
            );
            assert_eq!(
                parts.collect::<Vec<_>>(),
                vec!["0", "0", "0"],
                "expected the reduced .0.0.0 shape: {chrome_token}"
            );
        }
    }
}
