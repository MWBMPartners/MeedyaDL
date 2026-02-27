// Copyright (c) 2024-2026 MeedyaDL
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

/// Checks internet connectivity by making an HTTP GET request to
/// `https://www.apple.com/` with a 5-second timeout.
///
/// Apple.com is used as the target because it's the service MeedyaDL
/// interacts with — if Apple's servers are unreachable, downloads will
/// fail regardless of general internet connectivity.
///
/// # Returns
/// - `None` if the request succeeds (any HTTP response = internet works)
/// - `Some(PreflightWarning)` if the request fails (timeout, DNS, etc.)
pub async fn check_internet_connectivity() -> Option<PreflightWarning> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    match client.get("https://www.apple.com/").send().await {
        Ok(_) => None,
        Err(e) => {
            let message = if e.is_timeout() {
                "Internet connectivity check timed out — Apple Music servers may be unreachable".to_string()
            } else if e.is_connect() {
                "Cannot connect to Apple Music servers — check your internet connection".to_string()
            } else {
                format!("Internet connectivity check failed: {e}")
            };
            Some(PreflightWarning {
                check: PreflightCheck::Internet,
                message,
            })
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
                format!(
                    "Wrapper service at {wrapper_url} timed out — check that it is running"
                )
            } else if e.is_connect() {
                format!(
                    "Cannot connect to wrapper at {wrapper_url} — is the service running?"
                )
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
