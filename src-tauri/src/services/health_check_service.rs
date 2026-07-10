// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Pre-flight health check service.
// =================================
//
// Provides reusable health check functions that run before queue processing
// begins. These checks warn the user about potential issues (no internet,
// expired cookies, wrapper service down) without blocking the download queue.
//
// ## Architecture
//
// This service extracts validation logic that was previously inline in
// `commands/settings.rs` into reusable functions. Both the pre-flight
// checks in `download_queue.rs` and the existing Tauri commands use
// these shared functions.
//
// ## Checks
//
// 1. **Internet connectivity** — HTTP GET to `https://www.apple.com/` (5s timeout)
// 2. **Cookie validation** — Netscape format parsing, expiry check, Apple domain check
// 3. **Wrapper health** — HTTP GET to the wrapper service URL (5s timeout)
//
// ## Design
//
// Each check function returns `Option<PreflightWarning>`:
// - `None` = check passed, no issues found
// - `Some(warning)` = issue detected, warning should be shown to the user
//
// Warnings are non-blocking — the download queue proceeds regardless.
// The frontend displays them as persistent yellow toasts.

use serde::Serialize;

use crate::commands::settings::CookieValidation;

// ============================================================
// Types
// ============================================================

/// Identifies which pre-flight check produced a warning.
///
/// Serialized as snake_case strings for the frontend TypeScript types.
/// Used in the `"preflight-warning"` Tauri event payload.
///
/// Mirrors: `PreflightCheck` in `src/types/index.ts`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightCheck {
    /// Internet connectivity check (HTTP GET to apple.com)
    Internet,
    /// Cookie file validation (format, expiry, Apple domains)
    Cookies,
    /// Wrapper service health check (HTTP GET to wrapper URL)
    Wrapper,
    /// Wrapper m3u8 socket reachability (TCP connect to `wrapper_m3u8_ip`),
    /// required by GAMDL v3.1+ when `--use-wrapper` is set.
    WrapperM3u8,
    /// Wrapper decryption socket reachability (TCP connect to
    /// `wrapper_decrypt_ip`), required by GAMDL whenever
    /// `--use-wrapper` is set. Catches the common remote-wrapper
    /// misconfiguration where the user has set the account URL +
    /// m3u8 IP to a LAN host but left decrypt at the loopback
    /// default — see #743.
    WrapperDecrypt,
    /// Wrapper-v2 daemon health check (#853) — HTTP `GET /health`
    /// on the configured `wrapper_url`. Replaces the three v1 socket
    /// preflights above on GAMDL ≥ 3.6. Surfaces as a yellow toast
    /// when the wrapper-v2 container isn't running, isn't bound to
    /// the configured URL, or returns a non-200 status.
    WrapperV2Health,
    /// Wrapper-v2 daemon login state (#853) — HTTP `GET /me` to
    /// determine whether the wrapper has an active Apple Music
    /// session, plus an automatic `POST /login` retry when
    /// credentials are present. Required because GAMDL ≥ 3.6 would
    /// otherwise interactively prompt for credentials on stdin
    /// (which MeedyaDL cannot answer from a subprocess context),
    /// silently hanging the download. Surfaces as a yellow toast
    /// when the wrapper is reachable but logged out and no
    /// credentials are available.
    WrapperV2Auth,
    /// Output directory writability check (filesystem probe)
    OutputPath,
}

/// Payload emitted to the frontend via the `"preflight-warning"` Tauri event.
///
/// Each warning represents a single pre-flight check that detected an issue.
/// The frontend displays these as persistent yellow toast notifications.
///
/// Mirrors: `PreflightWarning` in `src/types/index.ts`
#[derive(Debug, Clone, Serialize)]
pub struct PreflightWarning {
    /// Which health check produced this warning
    pub check: PreflightCheck,
    /// Human-readable warning message for the user
    pub message: String,
}

// ============================================================
// Internet Connectivity Check
// ============================================================

/// Checks internet connectivity using a multi-provider, multi-tier approach.
///
/// ## Strategy
///
/// The check runs in two tiers to differentiate three scenarios:
///
/// **Tier 1 — General internet connectivity** (provider-neutral):
/// Tests Cloudflare (`1.1.1.1`) and Google (`google.com`) in sequence.
/// These are among the most reliable endpoints on the internet and are
/// independent of Apple's infrastructure. If either responds, the internet
/// is working. This avoids false "no internet" warnings when Apple alone
/// is experiencing an outage (which has happened with global CDN failures).
///
/// **Tier 2 — Apple Music API reachability** (service-specific):
/// Only runs if Tier 1 passes. Tests `amp-api.music.apple.com`, the actual
/// API endpoint GAMDL connects to. Even a 401 Unauthorized response counts
/// as "reachable" (proves TCP/TLS works to Apple's servers).
///
/// ## Outcomes
///
/// | Tier 1 | Tier 2 | Result |
/// |--------|--------|--------|
/// | Pass   | Pass   | No warning |
/// | Pass   | Fail   | "Apple Music API unreachable (internet is working)" |
/// | Fail   | —      | "No internet connectivity" (Tier 2 skipped) |
///
/// ## Performance
///
/// Happy path (everything works): two sequential HTTP GETs (~10ms each).
/// Cloudflare is tested first because it's the fastest global anycast network.
/// Each request has a 5-second timeout. Worst case (all fail): ~15 seconds
/// (3 endpoints × 5s timeout), but this only happens when offline.
///
/// ## Future-proofing
///
/// Tier 1 uses provider-neutral endpoints so it works for any service
/// (Apple Music, Spotify, YouTube, BBC iPlayer — see planned milestones).
/// Tier 2 can be extended per-service when additional services are added.
///
/// # Returns
/// - `None` if the Apple Music API is reachable
/// - `Some(PreflightWarning)` with a message differentiating the failure mode
pub async fn check_internet_connectivity() -> Option<PreflightWarning> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    // === Tier 1: General internet connectivity ===
    // Test provider-neutral endpoints. If any responds, internet is working.
    // Order: Cloudflare (fastest anycast) → Google (most reliable fallback).
    // Each endpoint's result is logged individually for diagnostics.
    log::info!("Pre-flight internet check: starting Tier 1 (general connectivity)");

    let cloudflare_result = try_reach(&client, "Cloudflare", "https://1.1.1.1/").await;
    let has_internet = if cloudflare_result {
        // Cloudflare passed — skip Google (short-circuit)
        log::info!("Pre-flight internet check: Google (google.com) → skipped (Cloudflare passed)");
        true
    } else {
        // Cloudflare failed — try Google as fallback
        try_reach(&client, "Google", "https://www.google.com/").await
    };

    if !has_internet {
        log::warn!("Pre-flight internet check: Tier 1 FAILED — no general internet connectivity");
        log::info!("Pre-flight internet check: Tier 2 (Apple Music API) → skipped (no internet)");
        return Some(PreflightWarning {
            check: PreflightCheck::Internet,
            message: "No internet connectivity — could not reach Cloudflare or Google. \
                      Check your network connection."
                .to_string(),
        });
    }

    log::info!("Pre-flight internet check: Tier 1 PASSED — general internet is working");

    // === Tier 2: Apple Music API reachability ===
    // Internet works, but can we reach the specific API endpoint GAMDL uses?
    // Any HTTP response (including 401, 403) = reachable.
    log::info!("Pre-flight internet check: starting Tier 2 (Apple Music API)");
    if try_reach(
        &client,
        "Apple Music API",
        "https://amp-api.music.apple.com/",
    )
    .await
    {
        log::info!("Pre-flight internet check: all tiers PASSED");
        return None; // Everything is reachable
    }

    // Internet works but Apple Music API doesn't — service-specific issue
    log::warn!("Pre-flight internet check: Tier 2 FAILED — Apple Music API unreachable (internet is working)");
    Some(PreflightWarning {
        check: PreflightCheck::Internet,
        message: "Apple Music API is unreachable (internet is working) — \
                  Apple's servers may be temporarily unavailable or blocked by your network"
            .to_string(),
    })
}

/// Attempts a single HTTP GET and returns `true` if any response was received.
///
/// Any HTTP status (200, 401, 403, 5xx) counts as "reachable" — we only care
/// about network-level connectivity (DNS, TCP, TLS), not HTTP-level success.
/// Logs the result with the endpoint name, HTTP status, and response time.
async fn try_reach(client: &reqwest::Client, name: &str, url: &str) -> bool {
    let start = std::time::Instant::now();
    match client.get(url).send().await {
        Ok(response) => {
            let elapsed = start.elapsed();
            log::info!(
                "Pre-flight internet check: {} ({}) → reachable ({}, {:.0?})",
                name,
                url,
                response.status(),
                elapsed,
            );
            true
        }
        Err(e) => {
            let elapsed = start.elapsed();
            // Categorise the failure for clearer diagnostics
            let reason = if e.is_timeout() {
                "timeout".to_string()
            } else if e.is_connect() {
                "connection refused".to_string()
            } else {
                format!("{e}")
            };
            log::info!(
                "Pre-flight internet check: {} ({}) → unreachable ({}, {:.0?})",
                name,
                url,
                reason,
                elapsed,
            );
            false
        }
    }
}

// ============================================================
// Cookie Validation
// ============================================================

/// Parses a Netscape-format cookies file and returns detailed validation.
///
/// This is the shared implementation used by both:
/// - `commands::settings::validate_cookies_file` (frontend cookie picker)
/// - `validate_cookies` below (pre-flight check)
///
/// # Netscape Cookie Format
/// Each cookie line is tab-separated with 7 fields:
/// `domain \t subdomains \t path \t secure \t expiry \t name \t value`
/// Lines starting with `#` are comments. Empty lines are skipped.
/// See: <https://curl.se/docs/http-cookies.html>
///
/// # Arguments
/// * `path` - Absolute path to the Netscape-format cookies file
///
/// # Returns
/// * `Ok(CookieValidation)` - Detailed validation result with counts and warnings
/// * `Err(String)` - File read error (not found, permission denied, etc.)
pub fn parse_cookies_file(path: &str) -> Result<CookieValidation, String> {
    // Read the entire cookie file into memory.
    // Cookie files are typically small (< 100KB) so reading all at once is fine.
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read cookie file: {e}"))?;

    // Tracking variables for the validation scan
    let mut cookie_count = 0;
    let mut domains = std::collections::HashSet::new();
    let mut apple_music_cookies = 0;
    let mut expired = false;
    let mut warnings = Vec::new();
    // Current UTC timestamp for comparing against cookie expiry times
    let now = chrono::Utc::now().timestamp();

    // Parse each line of the Netscape cookie file
    for line in contents.lines() {
        // Skip comments (lines starting with #) and empty lines.
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Split the line by tabs into the 7 expected fields.
        // Fields[0] = domain (may have leading dot for subdomain matching)
        // Fields[4] = expiry (Unix timestamp, 0 means session cookie)
        let fields: Vec<&str> = trimmed.split('\t').collect();
        if fields.len() >= 7 {
            cookie_count += 1;
            // Strip the leading dot from domain for consistent deduplication
            let domain = fields[0].trim_start_matches('.');
            domains.insert(domain.to_string());

            // Check for Apple Music related domains
            if domain.contains("apple.com") || domain.contains("mzstatic.com") {
                apple_music_cookies += 1;

                // Parse the expiry timestamp to check validity
                if let Ok(expiry) = fields[4].parse::<i64>() {
                    if expiry > 0 && expiry < now {
                        expired = true;
                    } else if expiry > 0 {
                        let days_until_expiry = (expiry - now) / 86400;
                        if days_until_expiry < 7 {
                            warnings.push(format!(
                                "Apple Music cookies expire in {days_until_expiry} day(s)"
                            ));
                        }
                    }
                }
            }
        }
    }

    let valid = cookie_count > 0;

    if apple_music_cookies == 0 {
        warnings.push("No Apple Music cookies found in file".to_string());
    }

    if expired {
        warnings.push(
            "Some Apple Music cookies have expired - you may need to re-export them".to_string(),
        );
    }

    Ok(CookieValidation {
        valid,
        cookie_count,
        domains: domains.into_iter().collect(),
        apple_music_cookies,
        expired,
        warnings,
    })
}

/// Validates a cookies file and returns a pre-flight warning if issues are found.
///
/// Wraps `parse_cookies_file()` and converts the detailed `CookieValidation`
/// into a simple `PreflightWarning` for the toast notification system.
///
/// # Arguments
/// * `cookies_path` - Absolute path to the Netscape-format cookies file
///
/// # Returns
/// - `None` if cookies are valid and contain non-expired Apple Music entries
/// - `Some(PreflightWarning)` if any issue is detected
pub fn validate_cookies(cookies_path: &str) -> Option<PreflightWarning> {
    match parse_cookies_file(cookies_path) {
        Ok(validation) => {
            if !validation.valid {
                Some(PreflightWarning {
                    check: PreflightCheck::Cookies,
                    message: "Cookies file is invalid or empty".to_string(),
                })
            } else if validation.apple_music_cookies == 0 {
                Some(PreflightWarning {
                    check: PreflightCheck::Cookies,
                    message: "No Apple Music cookies found in cookies file".to_string(),
                })
            } else if validation.expired {
                Some(PreflightWarning {
                    check: PreflightCheck::Cookies,
                    message: "Apple Music cookies have expired — re-export from your browser"
                        .to_string(),
                })
            } else {
                None
            }
        }
        Err(e) => Some(PreflightWarning {
            check: PreflightCheck::Cookies,
            message: format!("Cannot read cookies file: {e}"),
        }),
    }
}

// ============================================================
// Wrapper Health Check
// ============================================================

/// Pings the wrapper service URL with an HTTP GET and 5-second timeout.
///
/// Any HTTP response (even 404 or 500) counts as "reachable" — the wrapper
/// is running but may need configuration. Only network-level failures
/// (timeout, connection refused, DNS failure) produce a warning.
///
/// # Arguments
/// * `wrapper_url` - The wrapper service URL (e.g., `http://127.0.0.1:30020`)
///
/// # Returns
/// - `None` if the wrapper responded (any HTTP status)
/// - `Some(PreflightWarning)` if the wrapper is unreachable
pub async fn check_wrapper_health(wrapper_url: &str) -> Option<PreflightWarning> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    match client.get(wrapper_url).send().await {
        Ok(_) => None,
        Err(e) => {
            let message = if e.is_timeout() {
                format!("Wrapper service at {wrapper_url} timed out — check that it is running")
            } else if e.is_connect() {
                format!("Cannot connect to wrapper at {wrapper_url} — is the service running?")
            } else {
                format!("Wrapper health check failed: {e}")
            };
            Some(PreflightWarning {
                check: PreflightCheck::Wrapper,
                message,
            })
        }
    }
}

/// Probes the wrapper's decryption service via a 3-second TCP connect
/// (#743). Mirrors `check_wrapper_m3u8_health` exactly — same host:port
/// shape, same 3-second timeout, same warning surface — only the
/// failure message differs.
///
/// **Why this matters:** GAMDL opens an outbound TCP connection to the
/// configured `wrapper_decrypt_ip` (default `"127.0.0.1:10020"`) for
/// every encrypted sample it needs to decrypt. Cookie-mode downloads
/// never hit this socket; wrapper-mode downloads always do. If the
/// user has set `wrapper_account_url` + `wrapper_m3u8_ip` to a LAN
/// host but left `wrapper_decrypt_ip` at the loopback default — the
/// classic "I followed the wrapper docs but ALAC downloads still
/// fail" symptom — this preflight surfaces the mistake at queue time
/// instead of mid-download.
///
/// Returns:
/// - `None` if the socket accepts the connection.
/// - `Some(PreflightWarning)` on parse error, connect refusal, or timeout.
pub async fn check_wrapper_decrypt_health(
    wrapper_decrypt_ip: &str,
) -> Option<PreflightWarning> {
    let (host, port_str) = match wrapper_decrypt_ip.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !p.is_empty() => (h, p),
        _ => {
            return Some(PreflightWarning {
                check: PreflightCheck::WrapperDecrypt,
                message: format!(
                    "Wrapper decryption address '{wrapper_decrypt_ip}' is malformed — expected host:port (e.g. 127.0.0.1:10020)"
                ),
            });
        }
    };
    let port: u16 = match port_str.parse() {
        Ok(p) => p,
        Err(_) => {
            return Some(PreflightWarning {
                check: PreflightCheck::WrapperDecrypt,
                message: format!(
                    "Wrapper decryption port '{port_str}' in '{wrapper_decrypt_ip}' is not a valid number"
                ),
            });
        }
    };

    let connect = tokio::net::TcpStream::connect((host, port));
    match tokio::time::timeout(std::time::Duration::from_secs(3), connect).await {
        Ok(Ok(_stream)) => None,
        Ok(Err(err)) => Some(PreflightWarning {
            check: PreflightCheck::WrapperDecrypt,
            message: format!(
                "Wrapper decryption socket at {wrapper_decrypt_ip} is unreachable — \
                 GAMDL needs your wrapper to expose its decryption service here ({err})"
            ),
        }),
        Err(_) => Some(PreflightWarning {
            check: PreflightCheck::WrapperDecrypt,
            message: format!(
                "Wrapper decryption socket at {wrapper_decrypt_ip} timed out — \
                 check that your wrapper's decryption service is running"
            ),
        }),
    }
}

/// Probes the wrapper's m3u8 service (GAMDL v3.1+) via a 3-second TCP
/// connect.
///
/// Format: `"host:port"` (e.g. `"127.0.0.1:20020"`). The split is strict —
/// the wrapper m3u8 protocol opens a raw TCP socket, so there is no URL
/// parsing to do.
///
/// Returns:
/// - `None` if the socket accepts the connection.
/// - `Some(PreflightWarning)` on parse error, connect refusal, or timeout.
pub async fn check_wrapper_m3u8_health(wrapper_m3u8_ip: &str) -> Option<PreflightWarning> {
    // Parse `host:port`. Reject empty, missing colon, or un-parseable port.
    let (host, port_str) = match wrapper_m3u8_ip.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !p.is_empty() => (h, p),
        _ => {
            return Some(PreflightWarning {
                check: PreflightCheck::WrapperM3u8,
                message: format!(
                    "Wrapper m3u8 address '{wrapper_m3u8_ip}' is malformed — expected host:port (e.g. 127.0.0.1:20020)"
                ),
            });
        }
    };
    let port: u16 = match port_str.parse() {
        Ok(p) => p,
        Err(_) => {
            return Some(PreflightWarning {
                check: PreflightCheck::WrapperM3u8,
                message: format!(
                    "Wrapper m3u8 port '{port_str}' in '{wrapper_m3u8_ip}' is not a valid number"
                ),
            });
        }
    };

    let connect =
        tokio::net::TcpStream::connect((host, port));
    match tokio::time::timeout(std::time::Duration::from_secs(3), connect).await {
        Ok(Ok(_stream)) => None,
        Ok(Err(err)) => Some(PreflightWarning {
            check: PreflightCheck::WrapperM3u8,
            message: format!(
                "Wrapper m3u8 socket at {wrapper_m3u8_ip} is unreachable — GAMDL 3.1+ needs your wrapper to expose an m3u8 service here ({err})"
            ),
        }),
        Err(_) => Some(PreflightWarning {
            check: PreflightCheck::WrapperM3u8,
            message: format!(
                "Wrapper m3u8 socket at {wrapper_m3u8_ip} timed out — check that your wrapper's m3u8 service is running"
            ),
        }),
    }
}

// ============================================================
// Wrapper-v2 Health Check (#853, GAMDL ≥ 3.6)
// ============================================================

/// Wrapper-v2 daemon `/me` payload subset used by MeedyaDL.
///
/// Matches the shape returned by [wrapper-v2](https://github.com/glomatico/wrapper-v2):
///
/// ```json
/// {
///   "version": "0.0.2",
///   "runtime": { "playback_ready": true },
///   "auth":    { "state": "authenticated" | "logged_out" | "logging_in" }
/// }
/// ```
///
/// We deserialise the fields we act on (`auth.state` for the login gate
/// and `runtime.playback_ready` for the FairPlay-ready signal) plus the
/// daemon `version` (captured for diagnostics). Everything else is dropped.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WrapperV2Me {
    pub auth: WrapperV2AuthBlock,
    #[serde(default)]
    pub runtime: WrapperV2RuntimeBlock,
    /// Wrapper-v2 daemon version string (e.g. `"0.0.2"`), if reported.
    /// Captured for diagnostics only — GAMDL 3.8.2 hard-requires `0.0.2`
    /// (an exact-match check at CLI startup), but MeedyaDL's ceiling is
    /// GAMDL 3.8.1 so no version preflight is enforced here yet. The
    /// enforcing preflight lands when GAMDL 3.8.2 is admitted.
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WrapperV2AuthBlock {
    /// `"authenticated"`, `"logged_out"`, `"logging_in"`, `"awaiting_2fa"`.
    pub state: String,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct WrapperV2RuntimeBlock {
    /// True when FairPlay decrypt is available — required for the
    /// non-`aac-web` codec families. `aac-web` works without this.
    #[serde(default)]
    pub playback_ready: bool,
}

/// Wrapper-v2 daemon `GET /health` preflight (#853).
///
/// Issues a 3-second HTTP GET against `{wrapper_url}/health`. The
/// wrapper-v2 daemon's `/health` returns `200 OK` with the runtime
/// flags JSON when the supervisor + worker are both up; any other
/// response — including connection refused — surfaces as a yellow
/// toast on the queue page.
///
/// Returns:
/// - `None` when the daemon responds 200.
/// - `Some(PreflightWarning)` on non-200 status, connection error,
///   or 3-second timeout.
pub async fn check_wrapper_v2_health(wrapper_url: &str) -> Option<PreflightWarning> {
    let url = format!("{}/health", wrapper_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(err) => {
            return Some(PreflightWarning {
                check: PreflightCheck::WrapperV2Health,
                message: format!("Failed to build HTTP client for wrapper-v2 preflight: {err}"),
            });
        }
    };
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => None,
        Ok(resp) => Some(PreflightWarning {
            check: PreflightCheck::WrapperV2Health,
            message: format!(
                "Wrapper-v2 daemon at {wrapper_url} returned HTTP {} from GET /health — check the container logs",
                resp.status()
            ),
        }),
        Err(err) if err.is_timeout() => Some(PreflightWarning {
            check: PreflightCheck::WrapperV2Health,
            message: format!(
                "Wrapper-v2 daemon at {wrapper_url} timed out after 3s — is the container running?"
            ),
        }),
        Err(err) => Some(PreflightWarning {
            check: PreflightCheck::WrapperV2Health,
            message: format!(
                "Wrapper-v2 daemon at {wrapper_url} unreachable — {err}. \
                 GAMDL ≥ 3.6 needs wrapper-v2 for non-aac-web codecs."
            ),
        }),
    }
}

/// Fetches the wrapper-v2 `/me` payload to determine auth state and
/// runtime readiness (#853). Returns `Ok(WrapperV2Me)` on success;
/// the caller decides whether to fail the preflight or auto-login.
pub async fn fetch_wrapper_v2_me(wrapper_url: &str) -> Result<WrapperV2Me, String> {
    let url = format!("{}/me", wrapper_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GET {url} failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GET {url} returned HTTP {}", resp.status()));
    }
    resp.json::<WrapperV2Me>()
        .await
        .map_err(|e| format!("Failed to parse /me JSON: {e}"))
}

/// Wrapper-v2 auth-state preflight (#853).
///
/// Calls [`fetch_wrapper_v2_me`] and inspects `auth.state`. Surfaces
/// a yellow toast when the wrapper is reachable but logged-out — the
/// user must complete a `POST /login` flow before the next download
/// will succeed (otherwise GAMDL ≥ 3.6 would interactively prompt
/// on stdin, deadlocking the subprocess).
pub async fn check_wrapper_v2_auth(wrapper_url: &str) -> Option<PreflightWarning> {
    match fetch_wrapper_v2_me(wrapper_url).await {
        Ok(me) => {
            // Capture the daemon version for diagnostics (no enforcement
            // yet — MeedyaDL's ceiling is GAMDL 3.8.1). GAMDL 3.8.2 will
            // hard-require wrapper-v2 0.0.2, so surfacing it now helps
            // correlate version-skew failures ahead of that admission.
            log::debug!(
                "wrapper-v2 /me reports version {:?} (auth state: {})",
                me.version,
                me.auth.state
            );
            if me.auth.state == "authenticated" {
                None
            } else {
                Some(PreflightWarning {
                    check: PreflightCheck::WrapperV2Auth,
                    message: format!(
                        "Wrapper-v2 daemon at {wrapper_url} is reachable but not signed in (state: {}). \
                         Use Settings > Wrapper > Sign In before queueing downloads — GAMDL 3.6 would \
                         otherwise prompt for credentials on stdin and hang the subprocess.",
                        me.auth.state
                    ),
                })
            }
        }
        Err(err) => Some(PreflightWarning {
            check: PreflightCheck::WrapperV2Auth,
            message: format!(
                "Could not query wrapper-v2 auth state at {wrapper_url}: {err}"
            ),
        }),
    }
}

// ============================================================
// Output Path Writability Check
// ============================================================

/// Verifies that the resolved output directory exists and is writable.
///
/// This catches common issues before the download starts:
/// - Cloud storage mount disconnected (CloudMounter, rclone, SSHFS)
/// - Disk full or quota exceeded
/// - Permissions changed since settings were saved
/// - Network drive unreachable
///
/// Uses `tokio::task::spawn_blocking` + `tokio::time::timeout(5s)` to avoid
/// blocking the async runtime on unresponsive mounts (e.g., disconnected
/// CloudMounter volumes where file operations block for minutes before
/// returning ETIMEDOUT).
///
/// # Arguments
/// * `output_path` - The resolved output directory path
///
/// # Returns
/// - `None` if the directory is writable
/// - `Some(PreflightWarning)` if any issue is detected
pub async fn check_output_path(output_path: &str) -> Option<PreflightWarning> {
    let path_owned = output_path.to_string();

    let probe_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(move || probe_output_directory(&path_owned)),
    )
    .await;

    match probe_result {
        Ok(Ok(warning)) => warning, // Probe completed — None = writable, Some = issue
        Ok(Err(e)) => Some(PreflightWarning {
            check: PreflightCheck::OutputPath,
            message: format!("Output path check failed unexpectedly: {e}"),
        }),
        Err(_) => {
            // Timeout — likely an unresponsive network mount
            Some(PreflightWarning {
                check: PreflightCheck::OutputPath,
                message: format!(
                    "Output directory timed out (5s) — the path may be a \
                     disconnected cloud mount or unresponsive network drive: {output_path}"
                ),
            })
        }
    }
}

/// Synchronous filesystem probe for output directory writability.
///
/// Called from `check_output_path()` via `spawn_blocking()` to avoid
/// blocking the async runtime on potentially slow filesystem operations.
fn probe_output_directory(path: &str) -> Option<PreflightWarning> {
    let dir = std::path::Path::new(path);

    // Step 1: Check if the directory exists. If not, check if the parent
    // exists (the directory may be created on first download by GAMDL).
    if !dir.exists() {
        if let Some(parent) = dir.parent() {
            if !parent.exists() {
                return Some(PreflightWarning {
                    check: PreflightCheck::OutputPath,
                    message: format!(
                        "Output directory does not exist and its parent is also \
                         missing: {path}"
                    ),
                });
            }
            // Parent exists but target dir doesn't — GAMDL will create it.
            // Probe the parent instead.
            return probe_write_access(parent, path);
        }
        return Some(PreflightWarning {
            check: PreflightCheck::OutputPath,
            message: format!("Output directory does not exist: {path}"),
        });
    }

    // Step 2: Directory exists — verify it's actually a directory
    if !dir.is_dir() {
        return Some(PreflightWarning {
            check: PreflightCheck::OutputPath,
            message: format!("Output path exists but is not a directory: {path}"),
        });
    }

    // Step 3: Probe write access
    probe_write_access(dir, path)
}

/// Attempts to create and immediately delete a temporary probe file
/// to verify write access to the directory.
///
/// OS-specific error detection:
/// - `PermissionDenied` → explicit "not writable (permission denied)" message
/// - `ENOSPC` (errno 28) → "full disk"
/// - `EROFS` (errno 30) → "read-only file system" (macOS)
/// - `ETIMEDOUT` (errno 60) → "disconnected cloud mount" (macOS CloudMounter)
/// - `ESTALE` (errno 116) → "stale NFS handle" (Linux)
fn probe_write_access(dir: &std::path::Path, display_path: &str) -> Option<PreflightWarning> {
    let probe_file = dir.join(".meedyadl_write_probe");
    match std::fs::write(&probe_file, b"probe") {
        Ok(()) => {
            // Write succeeded — clean up the probe file
            let _ = std::fs::remove_file(&probe_file);

            // Check available disk space (warn if < 500 MB)
            if let Ok(available) = fs2::available_space(dir) {
                let available_mb = available / (1024 * 1024);
                if available_mb < 500 {
                    let available_display = if available_mb < 1024 {
                        format!("{available_mb} MB")
                    } else {
                        format!("{:.1} GB", available_mb as f64 / 1024.0)
                    };
                    return Some(PreflightWarning {
                        check: PreflightCheck::OutputPath,
                        message: format!(
                            "Low disk space on output directory ({available_display} remaining): {display_path}. \
                             Downloads may fail if the disk fills up during a large album."
                        ),
                    });
                }
            }

            None
        }
        Err(e) => {
            let message = if e.kind() == std::io::ErrorKind::PermissionDenied {
                format!("Output directory is not writable (permission denied): {display_path}")
            } else if e.raw_os_error() == Some(28) {
                // ENOSPC on macOS/Linux
                format!("Output directory is on a full disk: {display_path}")
            } else if e.raw_os_error() == Some(30) {
                // EROFS: read-only file system (macOS)
                format!("Output directory is on a read-only file system: {display_path}")
            } else if e.raw_os_error() == Some(60) {
                // ETIMEDOUT on macOS (disconnected CloudMounter mount)
                format!(
                    "Output directory timed out — cloud storage mount may be \
                     disconnected: {display_path}"
                )
            } else if e.raw_os_error() == Some(116) {
                // ESTALE on Linux (NFS stale handle)
                format!(
                    "Output directory has a stale file handle — network mount may \
                     need remounting: {display_path}"
                )
            } else {
                format!("Output directory is not writable: {display_path} ({e})")
            };
            Some(PreflightWarning {
                check: PreflightCheck::OutputPath,
                message,
            })
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_existing_writable_directory() {
        let dir = std::env::temp_dir();
        let result = probe_output_directory(dir.to_str().unwrap());
        assert!(result.is_none(), "Temp dir should be writable");
    }

    #[test]
    fn probe_nonexistent_directory_with_valid_parent() {
        // Use a unique name (PID) to avoid collisions with concurrent test runs
        // and clean up any stale directory from a previous run.
        let unique_name = format!("meedyadl_test_nonexistent_{}", std::process::id());
        let path = std::env::temp_dir().join(unique_name);
        let _ = std::fs::remove_dir_all(&path);
        let result = probe_output_directory(path.to_str().unwrap());
        // Parent (/tmp or equivalent) exists, so the probe should succeed on the parent.
        // On some CI runners the temp dir may have restricted write permissions —
        // in that case the function correctly returns a warning, so we only assert
        // that no "does not exist" warning is returned (the parent DOES exist).
        if let Some(ref warning) = result {
            assert!(
                !warning.message.contains("does not exist"),
                "Parent exists, so should not report 'does not exist': {}",
                warning.message
            );
        }
    }

    #[test]
    fn probe_nonexistent_directory_with_missing_parent() {
        let result = probe_output_directory("/nonexistent/deeply/nested/path");
        assert!(result.is_some());
        let warning = result.unwrap();
        assert!(matches!(warning.check, PreflightCheck::OutputPath));
        assert!(warning.message.contains("does not exist"));
    }

    #[test]
    fn probe_file_instead_of_directory() {
        // Create a temp file, then probe it as if it were a directory
        let file = std::env::temp_dir().join("meedyadl_test_probe_file");
        std::fs::write(&file, b"test").unwrap();
        let result = probe_output_directory(file.to_str().unwrap());
        std::fs::remove_file(&file).unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().message.contains("not a directory"));
    }
}
