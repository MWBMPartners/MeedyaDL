// Copyright (c) 2026 MeedyaDL
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

use std::time::Duration;

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
            .user_agent("MeedyaDL/1.0")
            .connect_timeout(5);
        assert_eq!(cfg.timeout, Duration::from_secs(10));
        assert_eq!(cfg.user_agent, Some("MeedyaDL/1.0"));
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
            .user_agent("MeedyaDL/test")
            .connect_timeout(5);
        assert!(build_client(cfg).is_ok());
    }
}
