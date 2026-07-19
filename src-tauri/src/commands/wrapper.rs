// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Wrapper-v2 interactive sign-in IPC commands (#1029).
// =====================================================
//
// wrapper-v2 is a self-authenticating daemon — it runs Apple's own sign-in
// flow and mints its own tokens. Its only credential input is `POST /login`
// (Apple ID + password) followed by `POST /login/2fa` for the two-factor
// code; it accepts no cookie, Music-User-Token, or Keychain handoff (see the
// investigation in #1029). These thin commands surface that flow to the
// Settings > Advanced > Wrapper "Sign in to wrapper" modal so users never
// touch the out-of-band `wrapper-account.sh` terminal helper.
//
// Credential hygiene: the Apple password / 2FA code arrive as command
// arguments, are forwarded straight to the daemon over the loopback wrapper
// URL, and are never logged or persisted. The wrapper daemon persists its
// own session in its container volume, so MeedyaDL never needs to re-send.

use crate::services::config_service;
use crate::services::health_check_service::{
    self, WrapperV2LoginResult,
};
use tauri::AppHandle;

/// Resolves the configured wrapper-v2 HTTP base URL from settings.
///
/// Falls back to defaults on any load error — the subsequent HTTP call
/// then surfaces an unreachable-wrapper message rather than panicking.
fn resolve_wrapper_url(app: &AppHandle) -> String {
    config_service::load_settings(app)
        .unwrap_or_default()
        .wrapper_url
}

/// Signs the wrapper-v2 daemon in to Apple with an Apple ID + password
/// (`POST /login`). Returns a [`WrapperV2LoginResult`] whose `status` is
/// `"authenticated"`, `"awaiting_2fa"`, `"failed"`, or `"error"`.
///
/// Rate-limited to 5 attempts/minute to blunt credential-stuffing against a
/// mistakenly LAN-exposed wrapper.
#[tauri::command]
pub async fn wrapper_sign_in(
    app: AppHandle,
    username: String,
    password: String,
) -> Result<WrapperV2LoginResult, String> {
    crate::utils::rate_limiter::check_rate_limit("wrapper_sign_in", 5, 60)?;
    let url = resolve_wrapper_url(&app);

    // Defense-in-depth: `wrapper_url` can no longer be hijacked via a
    // settings/bundle import (see `commands::settings::import_settings`
    // and `commands::profile_bundle::import_profile`), but a user could
    // still manually point Settings > Advanced > Wrapper at a remote
    // host. wrapper-v2 has no network-layer authentication of its own,
    // so POSTing an Apple ID + password to a non-local host would hand
    // Apple credentials to whoever controls that host. Refuse outright
    // unless the resolved host is loopback or a private/LAN address.
    if !wrapper_host_is_local_or_private(&url).await {
        return Err(format!(
            "Refusing to send credentials to a non-local wrapper URL ({url}). Set the wrapper to a local/LAN address in Settings."
        ));
    }

    Ok(health_check_service::wrapper_v2_login(&url, &username, &password).await)
}

/// Classifies whether a wrapper URL's host is safe to send Apple
/// credentials to: loopback (`127.0.0.1` / `::1` / `localhost`) or a
/// private/LAN address (RFC 1918 IPv4, IPv4 link-local, or IPv6
/// unique-local `fc00::/7`). DNS names are resolved and EVERY resolved
/// address must be local/private. Malformed URLs, unresolvable
/// hostnames, and public addresses all fail closed (`false`) — this is
/// a hard block, not an advisory hint (compare the softer frontend
/// classifier in `src/lib/wrapper-url-classifier.ts`, which only
/// surfaces a UI warning and never blocks).
async fn wrapper_host_is_local_or_private(wrapper_url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(wrapper_url) else {
        return false;
    };
    match parsed.host() {
        Some(url::Host::Ipv4(v4)) => is_local_or_private_ipv4(&v4),
        Some(url::Host::Ipv6(v6)) => is_local_or_private_ipv6(&v6),
        Some(url::Host::Domain(domain)) => {
            if domain.eq_ignore_ascii_case("localhost") {
                return true;
            }
            // Resolve the hostname and require ALL resolved addresses
            // to be local/private — a hostname that round-robins
            // between a LAN address and a public one must not sneak
            // through.
            match tokio::net::lookup_host((domain, 0u16)).await {
                Ok(addrs) => {
                    let mut saw_any = false;
                    for addr in addrs {
                        saw_any = true;
                        let ok = match addr.ip() {
                            std::net::IpAddr::V4(v4) => is_local_or_private_ipv4(&v4),
                            std::net::IpAddr::V6(v6) => is_local_or_private_ipv6(&v6),
                        };
                        if !ok {
                            return false;
                        }
                    }
                    saw_any
                }
                Err(_) => false,
            }
        }
        None => false,
    }
}

fn is_local_or_private_ipv4(v4: &std::net::Ipv4Addr) -> bool {
    v4.is_loopback() || v4.is_private() || v4.is_link_local()
}

fn is_local_or_private_ipv6(v6: &std::net::Ipv6Addr) -> bool {
    if let Some(mapped) = v6.to_ipv4_mapped() {
        return is_local_or_private_ipv4(&mapped);
    }
    if v6.is_loopback() {
        return true;
    }
    // fc00::/7 — Unique Local Address range (first byte 0xFC or 0xFD).
    let first_segment = v6.segments()[0];
    (0xfc00..=0xfdff).contains(&first_segment)
}

/// Submits the Apple two-factor code (`POST /login/2fa`) after
/// [`wrapper_sign_in`] returned `"awaiting_2fa"`. Same result contract.
#[tauri::command]
pub async fn wrapper_submit_2fa(
    app: AppHandle,
    code: String,
) -> Result<WrapperV2LoginResult, String> {
    crate::utils::rate_limiter::check_rate_limit("wrapper_submit_2fa", 10, 60)?;
    let url = resolve_wrapper_url(&app);
    Ok(health_check_service::wrapper_v2_submit_2fa(&url, &code).await)
}

/// Clears the wrapper-v2 session (`DELETE /login`).
#[tauri::command]
pub async fn wrapper_sign_out(app: AppHandle) -> Result<(), String> {
    let url = resolve_wrapper_url(&app);
    health_check_service::wrapper_v2_logout(&url).await
}

/// Returns the wrapper-v2 daemon's current `auth.state` (`GET /me`) so the
/// UI can show a "signed in / not signed in" indicator without a full
/// preflight. One of: `logged_out | in_progress | awaiting_2fa |
/// authenticated | failed`. Errors when the daemon is unreachable.
#[tauri::command]
pub async fn wrapper_auth_status(app: AppHandle) -> Result<String, String> {
    let url = resolve_wrapper_url(&app);
    let me = health_check_service::fetch_wrapper_v2_me(&url).await?;
    Ok(me.auth.state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_v4_is_local() {
        assert!(is_local_or_private_ipv4(&"127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn rfc1918_ranges_are_private() {
        assert!(is_local_or_private_ipv4(&"10.1.2.3".parse().unwrap()));
        assert!(is_local_or_private_ipv4(&"172.16.0.5".parse().unwrap()));
        assert!(is_local_or_private_ipv4(&"172.31.255.254".parse().unwrap()));
        assert!(is_local_or_private_ipv4(&"192.168.1.50".parse().unwrap()));
        assert!(is_local_or_private_ipv4(&"169.254.1.1".parse().unwrap()));
    }

    #[test]
    fn public_v4_is_not_local() {
        assert!(!is_local_or_private_ipv4(&"8.8.8.8".parse().unwrap()));
        assert!(!is_local_or_private_ipv4(&"1.1.1.1".parse().unwrap()));
        // Just outside the 172.16.0.0/12 private range.
        assert!(!is_local_or_private_ipv4(&"172.32.0.1".parse().unwrap()));
    }

    #[test]
    fn ipv6_loopback_and_ula_are_local() {
        assert!(is_local_or_private_ipv6(&"::1".parse().unwrap()));
        assert!(is_local_or_private_ipv6(&"fd00::1".parse().unwrap()));
        assert!(is_local_or_private_ipv6(&"fc00::1".parse().unwrap()));
    }

    #[test]
    fn ipv6_public_is_not_local() {
        assert!(!is_local_or_private_ipv6(
            &"2001:4860:4860::8888".parse().unwrap()
        ));
    }

    #[test]
    fn ipv6_mapped_v4_delegates_to_v4_rules() {
        // ::ffff:8.8.8.8 — public
        assert!(!is_local_or_private_ipv6(
            &"::ffff:8.8.8.8".parse().unwrap()
        ));
        // ::ffff:192.168.1.1 — private
        assert!(is_local_or_private_ipv6(
            &"::ffff:192.168.1.1".parse().unwrap()
        ));
    }

    #[tokio::test]
    async fn wrapper_host_rejects_public_ip_literal() {
        assert!(!wrapper_host_is_local_or_private("http://8.8.8.8:80").await);
    }

    #[tokio::test]
    async fn wrapper_host_accepts_loopback_and_private_literals() {
        assert!(wrapper_host_is_local_or_private("http://127.0.0.1").await);
        assert!(wrapper_host_is_local_or_private("http://localhost:30020").await);
        assert!(wrapper_host_is_local_or_private("http://192.168.1.5").await);
    }

    #[tokio::test]
    async fn wrapper_host_rejects_malformed_url() {
        assert!(!wrapper_host_is_local_or_private("not a url").await);
        assert!(!wrapper_host_is_local_or_private("").await);
    }
}
