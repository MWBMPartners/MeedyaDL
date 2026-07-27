// Copyright (c) 2026 MeedyaSuite
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

/// Single canonical User-Agent string for every outbound HTTP request sent
/// to a **third party** that identifies itself as MeedyaDL — GitHub, PyPI,
/// MusicBrainz, Odesli, the Apple Music JWT paths, raw.githubusercontent —
/// (i.e. everything except the deliberate Apple-browser impersonation in
/// `apple_music_api::APPLE_BROWSER_USER_AGENT`, which must stay untouched —
/// Apple 403s non-browser UAs on that codepath).
///
/// This is a compile-time `const` (not `full_user_agent()` below) precisely
/// because third parties get NO platform detail: OS/arch/OS-version is a
/// fingerprinting surface that buys a third-party API nothing, so it is
/// deliberately omitted here. See `full_user_agent()` for the
/// platform-bearing counterpart, reserved for MeedyaDL's own backend.
///
/// Three things this constant fixes by construction:
///
/// (a) "MeedyaDL/" MUST stay first — MWBM-IntAppsAPI's `AuthMiddleware` does
///     a `str_starts_with()` prefix match against a registered
///     `user_agent_prefix`, so reordering or removing this prefix silently
///     breaks that integration's request identification.
/// (b) The version is resolved at compile time via `env!("CARGO_PKG_VERSION")`
///     so it can never drift out of sync with the shipped app version — the
///     bug this constant replaces was a hardcoded `MusicBrainz` UA pinned at
///     "0.6" while the app itself had moved on to the 1.12.x line.
/// (c) The `(+https://...)` comment token satisfies MusicBrainz's Terms of
///     Service requirement that outbound UAs identify the application and
///     provide a way to reach its maintainers — the `+URL` convention (as
///     used by e.g. Googlebot's UA string) marks the parenthesised segment
///     as a contact/info link rather than a platform-detail comment.
pub const APP_USER_AGENT: &str = concat!(
    "MeedyaDL/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/MWBMPartners/MeedyaDL)"
);

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
}
