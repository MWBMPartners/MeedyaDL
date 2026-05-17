// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// IPC command rate limiter.
// =========================
//
// Simple sliding-window rate limiter for Tauri IPC commands. Prevents
// abuse from a compromised or buggy frontend by limiting how frequently
// expensive commands can be called.
//
// ## Usage
//
// ```rust
// use crate::utils::rate_limiter::check_rate_limit;
//
// #[tauri::command]
// pub async fn start_download(...) -> Result<..., String> {
//     check_rate_limit("start_download", 10, 60)?; // 10 calls per 60 seconds
//     // ... command logic
// }
// ```
//
// ## Design
//
// Uses a global `LazyLock<Mutex<HashMap>>` to track per-command call
// timestamps. The sliding window approach removes expired timestamps
// on each check, so memory usage is bounded by the window size.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

/// Global rate limit state: command name → list of call timestamps.
static RATE_LIMITS: LazyLock<Mutex<HashMap<&'static str, Vec<Instant>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Check whether a command is within its rate limit.
///
/// Returns `Ok(())` if the call is allowed, or `Err(String)` with a
/// human-readable message if the limit has been exceeded.
///
/// # Arguments
/// * `command` - Command name (used as the rate limit key)
/// * `max_calls` - Maximum number of calls allowed within the window
/// * `window_secs` - Sliding window duration in seconds
///
/// # Example
///
/// Allow at most 10 calls to "start_download" per 60 seconds:
/// ```rust,no_run
/// # use meedyadl::utils::rate_limiter::check_rate_limit;
/// check_rate_limit("start_download", 10, 60).unwrap();
/// ```
pub fn check_rate_limit(
    command: &'static str,
    max_calls: usize,
    window_secs: u64,
) -> Result<(), String> {
    let now = Instant::now();
    let window = std::time::Duration::from_secs(window_secs);

    let mut limits = RATE_LIMITS
        .lock()
        .map_err(|_| "Rate limiter lock poisoned".to_string())?;

    let timestamps = limits.entry(command).or_default();

    // Remove expired timestamps (outside the sliding window)
    timestamps.retain(|t| now.duration_since(*t) < window);

    if timestamps.len() >= max_calls {
        let oldest = timestamps.first().copied().unwrap_or(now);
        let retry_after = window
            .checked_sub(now.duration_since(oldest))
            .unwrap_or_default();

        log::warn!(
            "Rate limit exceeded for '{}': {}/{} calls in {}s window (retry after {:.0}s)",
            command,
            timestamps.len(),
            max_calls,
            window_secs,
            retry_after.as_secs_f64(),
        );

        return Err(format!(
            "Too many requests. Please wait {:.0} seconds before trying again.",
            retry_after.as_secs_f64().ceil(),
        ));
    }

    // Record this call
    timestamps.push(now);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_calls_within_limit() {
        // Use a unique command name to avoid test interference
        let result = check_rate_limit("test_allow", 5, 60);
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_calls_exceeding_limit() {
        for _ in 0..3 {
            check_rate_limit("test_reject", 3, 60).unwrap();
        }
        let result = check_rate_limit("test_reject", 3, 60);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Too many requests"));
    }
}
