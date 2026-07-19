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
    Ok(health_check_service::wrapper_v2_login(&url, &username, &password).await)
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
