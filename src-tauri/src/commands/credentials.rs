// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License. See LICENSE file in the project root.
//
// Secure credential storage IPC commands.
// Provides the frontend with the ability to securely store, retrieve,
// and delete sensitive credentials (API keys, wrapper URLs, etc.)
// using the operating system's native keychain/keyring.
//
// Platform backends:
// - macOS: Keychain
// - Windows: Windows Credential Manager
// - Linux: Secret Service (GNOME Keyring / KWallet)
//
// ## Architecture
//
// This module uses the `keyring` crate (https://docs.rs/keyring/latest/keyring/)
// to provide a cross-platform abstraction over OS-level secure storage.
// All credentials are stored under a single service name (SERVICE_NAME)
// with different keys for different secrets. This keeps credentials out of
// plain-text config files and leverages OS-level encryption and access control.
//
// Currently used for:
// - Wrapper URL (for GAMDL's Apple Music API wrapper)
// - Future: YouTube Music / Spotify API keys
//
// ## Frontend Mapping (src/lib/tauri-commands.ts)
//
// | Rust Command       | TypeScript Function       | Line |
// |--------------------|---------------------------|------|
// | store_credential   | storeCredential(k, v)     | ~133 |
// | get_credential     | getCredential(k)          | ~138 |
// | delete_credential  | deleteCredential(k)       | ~143 |
//
// ## References
//
// - keyring crate: https://docs.rs/keyring/latest/keyring/
// - Tauri IPC commands: https://v2.tauri.app/develop/calling-rust/
// - macOS Keychain Services: https://developer.apple.com/documentation/security/keychain_services
// - Windows Credential Manager: https://learn.microsoft.com/en-us/windows/win32/secauthn/credential-manager

use crate::context_err;

/// The service name used as the namespace in the OS keychain.
/// All credentials stored by this app use this identifier.
///
/// In keychain terminology, credentials are stored as (service, account) pairs:
/// - **service** = this constant (identifies our app)
/// - **account** = the `key` parameter passed to each command
///
/// This means credentials appear in Keychain Access (macOS) as:
///   Service: "io.github.meedyadl"
///   Account: "`wrapper_url`" (or whatever key was used)
///
/// See: <https://docs.rs/keyring/latest/keyring/struct.Entry.html>
const SERVICE_NAME: &str = "io.github.meedyadl";

/// Keychain accounts the universal `store_credential` / `get_credential` /
/// `delete_credential` IPCs are allowed to touch.
///
/// **Adding to this list MUST be reviewed for post-XSS exfiltration
/// risk** — high-sensitivity entries should ideally have dedicated
/// per-secret IPCs (e.g. `has_webplayer_token`, `clear_webplayer_token`,
/// `check_dev_access`) rather than ride the universal accessor.
///
/// Pre-#938, these three IPCs accepted **any** key string, so any
/// post-XSS attacker (hostile help-content markdown, compromised npm
/// dep, future stored-XSS) could `invoke('get_credential', { key:
/// 'webplayer_developer_token' })` or worse — `'musickit_private_key'`
/// — and exfiltrate Apple Developer credentials from the OS keychain.
/// The allowlist closes that escalation path while preserving the only
/// legitimate frontend caller in
/// `src/components/settings/tabs/AdvancedTab.tsx`.
///
/// Current entries:
/// * `musickit_private_key` — set/get by the AdvancedTab MusicKit
///   credentials editor.
///
/// Other keychain accounts stay reachable only via their dedicated
/// IPCs:
/// * `webplayer_developer_token` → `has_webplayer_token`,
///   `clear_webplayer_token`.
/// * `dev_access_token` → `check_dev_access`, `activate_dev_access`,
///   `deactivate_dev_access`.
const ALLOWED_CREDENTIAL_KEYS: &[&str] = &["musickit_private_key"];

/// Verify that the supplied credential key is on the allowlist.
///
/// The error message is deliberately generic — it does NOT disclose
/// which keys are on the allowlist or what's gated elsewhere, so an
/// attacker can't use the IPC error response as a key-enumeration
/// oracle.
fn check_credential_key_allowed(key: &str) -> Result<(), String> {
    if ALLOWED_CREDENTIAL_KEYS.contains(&key) {
        Ok(())
    } else {
        // Logged with WARN so an operator inspecting the tracing log
        // can see brute-force probes after the fact, but the IPC
        // response stays opaque.
        log::warn!(
            "Credential IPC rejected disallowed key '{key}' (allowlist enforcement, #938)"
        );
        Err(format!("Unknown credential key: {key}"))
    }
}

/// Stores a credential securely in the OS keychain.
///
/// **Frontend caller:** `storeCredential(key, value)` in `src/lib/tauri-commands.ts`
///
/// If a credential with the same key already exists, it is overwritten.
/// This is used for storing wrapper URLs, future API keys for
/// `YouTube` Music / Spotify integrations, and other sensitive data.
///
/// The credential is stored using the OS native secure storage:
/// - macOS: Keychain Services (encrypted, requires user authentication)
/// - Windows: Credential Manager (DPAPI encrypted)
/// - Linux: Secret Service API (GNOME Keyring or `KWallet`)
///
/// # Arguments
/// * `key` - A unique identifier for the credential (e.g., "`wrapper_url`").
///   This becomes the "account" field in the keychain entry.
/// * `value` - The secret value to store. Stored as the "password" field.
///
/// # Errors
///
/// Returns `Err(String)` if the OS keychain is inaccessible (locked, permission
/// denied, or backend unavailable).
///
/// # Returns
/// * `Ok(())` - Credential stored (or overwritten) successfully.
/// * `Err(String)` - Keychain access error (locked keychain, permission denied, etc.).
///
/// # Security Note
/// The value is never logged — only the key name is logged for auditability.
#[tauri::command]
pub async fn store_credential(key: String, value: String) -> Result<(), String> {
    // #938 — reject any key not on the allowlist BEFORE creating a
    // keyring handle, so an attacker probing the IPC can't enumerate
    // valid keys via keyring-error vs allowlist-error response shapes.
    check_credential_key_allowed(&key)?;

    // Create a keyring entry handle for the (service, key) pair.
    // Entry::new() can fail if the OS keychain backend is unavailable.
    // See: https://docs.rs/keyring/latest/keyring/struct.Entry.html#method.new
    let entry = context_err!(
        keyring::Entry::new(SERVICE_NAME, &key),
        "Failed to create keyring entry"
    )?;

    // Store the credential in the OS keychain.
    // set_password() creates or overwrites the credential atomically.
    context_err!(
        entry.set_password(&value),
        "Failed to store credential '{key}'"
    )?;

    // Log the key name only (never the value) for debugging and auditing
    log::info!("Credential '{key}' stored securely");
    Ok(())
}

/// Retrieves a credential from the OS keychain.
///
/// **Frontend caller:** `getCredential(key)` in `src/lib/tauri-commands.ts`
///
/// Returns `Some(value)` if the credential exists, `None` if it was never stored.
/// Returns an `Err` if the keychain is locked, access is denied, or the
/// keychain backend encounters an unexpected error.
///
/// The distinction between `Ok(None)` (not found) and `Err(...)` (access error)
/// is important: the frontend treats `None` as "not yet configured" (show setup
/// prompt) vs. an error which indicates a system-level problem.
///
/// # Arguments
/// * `key` - The unique identifier used when storing the credential
///   (e.g., "`wrapper_url`").
///
/// # Errors
///
/// Returns `Err(String)` if the OS keychain is inaccessible (locked, permission
/// denied, or backend unavailable).
///
/// # Returns
/// * `Ok(Some(String))` - The credential value was retrieved successfully.
/// * `Ok(None)` - No credential exists for this key (never been stored).
/// * `Err(String)` - Keychain access error (locked, permission denied, etc.).
#[tauri::command]
pub async fn get_credential(key: String) -> Result<Option<String>, String> {
    // #938 — reject any key not on the allowlist. CRITICAL: return
    // `Err` (not `Ok(None)`) so the response distinguishes
    // "you're not allowed to read this" from "this key is unset". An
    // `Ok(None)` would let an attacker probe whether a sensitive key
    // exists via the response shape.
    check_credential_key_allowed(&key)?;

    // Create a keyring entry handle for the lookup
    let entry = context_err!(
        keyring::Entry::new(SERVICE_NAME, &key),
        "Failed to create keyring entry"
    )?;

    // Attempt to retrieve the stored password.
    // The keyring crate distinguishes between "not found" and other errors,
    // which we map to Ok(None) and Err() respectively.
    match entry.get_password() {
        // Credential found — return the secret value
        Ok(password) => Ok(Some(password)),
        // NoEntry means the credential was never stored — this is expected
        // behavior on first run, not an error condition.
        // See: https://docs.rs/keyring/latest/keyring/enum.Error.html#variant.NoEntry
        Err(keyring::Error::NoEntry) => Ok(None),
        // Any other error indicates a system-level problem:
        // locked keychain, permission denied, backend unavailable, etc.
        Err(e) => Err(format!("Failed to retrieve credential '{key}': {e}")),
    }
}

/// Deletes a credential from the OS keychain.
///
/// **Frontend caller:** `deleteCredential(key)` in `src/lib/tauri-commands.ts`
///
/// This operation is idempotent — returns `Ok(())` even if the credential
/// didn't exist. This simplifies frontend logic: the caller doesn't need
/// to check existence before deletion.
///
/// # Arguments
/// * `key` - The unique identifier of the credential to delete
///   (e.g., "`wrapper_url`").
///
/// # Errors
///
/// Returns `Err(String)` if the OS keychain is inaccessible (locked, permission
/// denied, or backend unavailable).
///
/// # Returns
/// * `Ok(())` - Credential deleted, or it didn't exist (both are success).
/// * `Err(String)` - Keychain access error (locked, permission denied, etc.).
#[tauri::command]
pub async fn delete_credential(key: String) -> Result<(), String> {
    // #938 — reject any key not on the allowlist before touching the
    // keyring. Same threat model as store_credential: an attacker
    // shouldn't be able to delete dedicated-IPC keys
    // (`webplayer_developer_token`, `dev_access_token`) via the
    // universal accessor.
    check_credential_key_allowed(&key)?;

    // Create a keyring entry handle for the deletion
    let entry = context_err!(
        keyring::Entry::new(SERVICE_NAME, &key),
        "Failed to create keyring entry"
    )?;

    // Attempt to delete the credential from the OS keychain.
    // We explicitly handle NoEntry as a success case for idempotency.
    match entry.delete_credential() {
        Ok(()) => {
            log::info!("Credential '{key}' deleted");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            // Credential didn't exist — this is fine, deletion is idempotent.
            // No log message needed since there was nothing to delete.
            Ok(())
        }
        // Any other error is a real failure (locked keychain, etc.)
        Err(e) => Err(format!("Failed to delete credential '{key}': {e}")),
    }
}

/// Validates MusicKit credentials by generating a JWT and testing it against
/// the Apple Music API.
///
/// **Frontend caller:** `validateMusicKitCredentials()` in `src/lib/tauri-commands.ts`
///
/// Performs a lightweight test by fetching metadata for a known public album.
/// This verifies that the Team ID, Key ID, and private key are all valid and
/// that the JWT can authenticate with Apple's API.
///
/// # Returns
/// * `Ok(String)` - Success message confirming credentials are valid.
/// * `Err(String)` - Descriptive error with guidance on what to check.
#[tauri::command]
pub async fn validate_musickit_credentials(
    app: tauri::AppHandle,
    team_id: Option<String>,
    key_id: Option<String>,
) -> Result<String, String> {
    use std::sync::LazyLock;

    use crate::services::{apple_music_api, config_service};
    use regex::Regex;

    static ID_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[A-Z0-9]{10}$").expect("Invalid MusicKit ID regex"));

    // Resolve from current UI input first, then persisted settings as fallback.
    let settings = config_service::load_settings(&app).unwrap_or_default();
    let team_id = team_id
        .as_deref()
        .or(settings.musickit_team_id.as_deref())
        .ok_or(
            "MusicKit Team ID not configured. Enter your 10-character Team ID in Settings > Advanced > API Credentials.",
        )?;
    let key_id = key_id
        .as_deref()
        .or(settings.musickit_key_id.as_deref())
        .ok_or(
            "MusicKit Key ID not configured. Enter your 10-character Key ID in Settings > Advanced > API Credentials.",
        )?;

    let team_id = team_id.trim().to_ascii_uppercase();
    let key_id = key_id.trim().to_ascii_uppercase();

    if !ID_RE.is_match(&team_id) {
        return Err(
            "MusicKit Team ID must be exactly 10 uppercase letters/numbers (A-Z, 0-9).".to_string(),
        );
    }
    if !ID_RE.is_match(&key_id) {
        return Err(
            "MusicKit Key ID must be exactly 10 uppercase letters/numbers (A-Z, 0-9).".to_string(),
        );
    }

    // 2. Get private key from OS keychain
    let private_key = apple_music_api::get_private_key_from_keychain()
        .map_err(|e| format!("Keychain error: {e}"))?
        .ok_or(
            "MusicKit private key not found in OS keychain. \
             Paste your .p8 private key content in Settings > Advanced > API Credentials and click 'Save to Keychain'.",
        )?;

    // 3. Generate JWT
    let jwt =
        apple_music_api::generate_musickit_jwt(&team_id, &key_id, &private_key).map_err(|e| {
            format!(
                "JWT generation failed: {e}. Check that your private key is a valid .p8 PEM file."
            )
        })?;

    log::info!("MusicKit validation: JWT generated (iss={team_id}, kid={key_id})");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    // 4. Probe both known Apple Music API hostnames.
    // A 200/404 response on either host proves auth success.
    let test_urls = [
        "https://amp-api.music.apple.com/v1/catalog/us/albums/1441164495",
        "https://api.music.apple.com/v1/catalog/us/albums/1441164495",
    ];

    let mut statuses: Vec<(String, u16)> = Vec::new();
    let mut network_errors: Vec<String> = Vec::new();

    for url in test_urls {
        let mut req = client
            .get(url)
            .header("Authorization", format!("Bearer {jwt}"))
            .header("User-Agent", "meedyadl");
        // amp-api is Apple's web player API and expects an Origin header;
        // the official MusicKit API (api.music.apple.com) does not need it.
        if url.contains("amp-api") {
            req = req.header("Origin", "https://music.apple.com");
        }
        match req.send().await {
            Ok(resp) => {
                statuses.push((url.to_string(), resp.status().as_u16()));
            }
            Err(err) => {
                network_errors.push(format!("{url}: {err}"));
            }
        }
    }

    if statuses.iter().any(|(_, s)| *s == 200 || *s == 404) {
        log::info!("MusicKit validation: credentials valid (statuses={statuses:?})");
        return Ok("MusicKit credentials are valid! API authentication successful.".to_string());
    }

    // If ANY host returned 401, the credentials are likely invalid (#161).
    // Previously used .all() which missed cases where one host had a network
    // error and the other returned 401 — falling through to a generic message.
    if statuses.iter().any(|(_, s)| *s == 401) {
        log::warn!("MusicKit validation: authentication failed (HTTP 401, statuses={statuses:?})");
        return Err(
            "Authentication failed (HTTP 401). Your MusicKit credentials may be invalid. \
             Check on developer.apple.com that: (1) Team ID is correct, (2) Key ID is active, \
             (3) private key (.p8) matches the Key ID, (4) key has MusicKit/Media Services access."
                .to_string(),
        );
    }

    if statuses.iter().any(|(_, s)| *s == 403) {
        return Err(
            "Forbidden (HTTP 403). Your MusicKit key likely lacks required permissions. \
             Ensure MusicKit (Media Services) is enabled for that key in Apple Developer."
                .to_string(),
        );
    }

    if statuses.iter().any(|(_, s)| *s == 429) {
        return Err(
            "Rate limited (HTTP 429). Apple Music API is temporarily limiting requests. Try again in a few minutes."
                .to_string(),
        );
    }

    if statuses.is_empty() {
        return Err(format!(
            "Network error while contacting Apple Music API. {}",
            network_errors.join(" | ")
        ));
    }

    Err(format!(
        "Unexpected response while validating MusicKit credentials: {:?}.",
        statuses
    ))
}

/// Returns true if a build-time MusicKit developer token is embedded.
#[tauri::command]
pub fn has_embedded_musickit_token() -> bool {
    crate::services::apple_music_api::has_embedded_musickit_developer_token()
}

/// Returns true if a web player developer token is stored in the OS keychain.
///
/// **Frontend caller:** `hasWebplayerToken()` in `src/lib/tauri-commands.ts`
///
/// The web player token is extracted opportunistically from the Apple Music
/// login window during cookie import. It serves as a last-resort fallback for
/// premium API features when the user has no MusicKit credentials.
#[tauri::command]
pub fn has_webplayer_token() -> bool {
    crate::services::apple_music_api::has_webplayer_token()
}

/// Deletes the web player developer token from the OS keychain.
///
/// **Frontend caller:** `clearWebplayerToken()` in `src/lib/tauri-commands.ts`
///
/// Idempotent — returns `Ok(())` even if no token was stored.
#[tauri::command]
pub async fn clear_webplayer_token() -> Result<(), String> {
    crate::services::apple_music_api::clear_webplayer_token_from_keychain()
}

// ============================================================
// Developer Access
// ============================================================

/// Compile-time developer-access passphrase hash (SHA-256), injected via a CI
/// secret (`DEV_ACCESS_HASH`) in production builds. The plaintext passphrase
/// never appears in the binary. `None` in local dev builds, where
/// `activate_dev_access` falls back to the SHA-256 of the empty string —
/// effectively disabled (only an empty passphrase would match). Kept as an
/// `Option` rather than a hardcoded fallback hash so no hash literal sits in
/// source (which otherwise trips a false-positive secret-scanning match).
const DEV_ACCESS_HASH: Option<&str> = option_env!("DEV_ACCESS_HASH");

/// Keychain account name for the developer access sentinel.
const DEV_ACCESS_KEYCHAIN_KEY: &str = "dev_access_token";

/// Sentinel value stored in keychain when dev access is active.
const DEV_ACCESS_SENTINEL: &str = "meedya-dev-active";

/// Checks whether developer access is currently active.
///
/// **Frontend caller:** `checkDevAccess()` in `src/lib/tauri-commands.ts`
///
/// Checks both the OS keychain sentinel and the `dev_access_enabled`
/// settings field. Either being active is sufficient.
#[tauri::command]
pub fn check_dev_access(app: tauri::AppHandle) -> bool {
    // Fast path: check settings first (in-memory, no keychain I/O)
    if let Ok(settings) = crate::services::config_service::load_settings(&app) {
        if settings.dev_access_enabled {
            return true;
        }
    }

    // Slow path: check keychain sentinel
    let Ok(entry) = keyring::Entry::new(SERVICE_NAME, DEV_ACCESS_KEYCHAIN_KEY) else {
        return false;
    };
    matches!(entry.get_password(), Ok(val) if val == DEV_ACCESS_SENTINEL)
}

/// Activates developer access after validating the passphrase.
///
/// **Frontend caller:** `activateDevAccess(passphrase)` in `src/lib/tauri-commands.ts`
///
/// The passphrase is hashed (SHA-256) and compared against the compile-time
/// embedded hash. On success, stores a keychain sentinel and enables
/// `dev_access_enabled` in settings. Returns whether activation succeeded.
///
/// On failure, returns `false` silently (no error hint to prevent brute-forcing).
///
/// **Rate-limited** at 5 attempts per 60-second sliding window (#940). Plain
/// SHA-256 with no salt and no throttle is wordlist-trivial for any code with
/// IPC access (post-XSS, hostile markdown, compromised npm dep); the limit
/// turns thousands-of-attempts-per-second brute force into ≤5/min. Legit users
/// type the passphrase by hand so the cap has zero UX cost. Mirrors the
/// existing rate-limiter pattern used by `start_download`,
/// `import_cookies_from_browser`, and `check_all_updates`.
#[tauri::command]
pub async fn activate_dev_access(app: tauri::AppHandle, passphrase: String) -> bool {
    use sha2::{Digest, Sha256};

    // Reject after 5 attempts in the rolling 60-second window. The IPC
    // returns `-> bool` (not Result) so we map the rate-limit Err to
    // a plain `false` — visually indistinguishable to the attacker from
    // a wrong-passphrase rejection, denying them a usable oracle.
    if crate::utils::rate_limiter::check_rate_limit("activate_dev_access", 5, 60).is_err() {
        log::warn!("activate_dev_access: rate-limited (possible brute-force attempt)");
        return false;
    }

    // Hash the provided passphrase and compare against the embedded hash.
    let hash = format!("{:x}", Sha256::digest(passphrase.as_bytes()));
    // Fall back to SHA-256("") when no CI hash was injected (local dev builds):
    // dev access is effectively disabled, and no hash literal sits in source.
    let expected =
        DEV_ACCESS_HASH.map_or_else(|| format!("{:x}", Sha256::digest(b"")), str::to_owned);
    if hash != expected {
        return false;
    }

    // Store keychain sentinel
    if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, DEV_ACCESS_KEYCHAIN_KEY) {
        let _ = entry.set_password(DEV_ACCESS_SENTINEL);
    }

    // Enable in settings
    if let Ok(mut settings) = crate::services::config_service::load_settings(&app) {
        settings.dev_access_enabled = true;
        let _ = crate::services::config_service::save_settings(&app, &settings);
    }

    log::info!("Developer access activated");
    true
}

/// Deactivates developer access.
///
/// **Frontend caller:** `deactivateDevAccess()` in `src/lib/tauri-commands.ts`
///
/// Removes the keychain sentinel and disables `dev_access_enabled` in settings.
#[tauri::command]
pub async fn deactivate_dev_access(app: tauri::AppHandle) -> Result<(), String> {
    // Remove keychain sentinel
    if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, DEV_ACCESS_KEYCHAIN_KEY) {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(format!("Failed to remove dev access sentinel: {e}")),
        }
    }

    // Disable in settings
    if let Ok(mut settings) = crate::services::config_service::load_settings(&app) {
        settings.dev_access_enabled = false;
        let _ = crate::services::config_service::save_settings(&app, &settings);
    }

    log::info!("Developer access deactivated");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn musickit_id_normalization_logic() {
        let team = " ab12cd34ef ".trim().to_ascii_uppercase();
        let key = " 99zz88yy77 ".trim().to_ascii_uppercase();
        assert_eq!(team, "AB12CD34EF");
        assert_eq!(key, "99ZZ88YY77");
        assert_eq!(team.len(), 10);
        assert_eq!(key.len(), 10);
    }

    // ----------------------------------------------------------
    // Credential allowlist tests (#938)
    // ----------------------------------------------------------
    //
    // Pin the allowlist contract — if a future contributor adds a key
    // here, they need to deliberately update these tests (which forces
    // the post-XSS-risk review the allowlist's whole purpose).

    #[test]
    fn allowlist_accepts_musickit_private_key() {
        // The single legitimate frontend caller (AdvancedTab.tsx).
        assert!(check_credential_key_allowed("musickit_private_key").is_ok());
    }

    #[test]
    fn allowlist_rejects_webplayer_developer_token() {
        // Critical: this key MUST stay off the allowlist — it has a
        // dedicated IPC (`has_webplayer_token` / `clear_webplayer_token`).
        // If a future PR adds it here, the post-XSS escalation primitive
        // returns.
        assert!(check_credential_key_allowed("webplayer_developer_token").is_err());
    }

    #[test]
    fn allowlist_rejects_dev_access_token() {
        // Critical: this key MUST stay off the allowlist — it's the
        // dev-access sentinel set/cleared by `activate_dev_access` /
        // `deactivate_dev_access`. Allowlist'ing it would let any
        // post-XSS script read the unlock sentinel.
        assert!(check_credential_key_allowed("dev_access_token").is_err());
    }

    #[test]
    fn allowlist_rejects_arbitrary_keys() {
        // Defence against probing — random keys, fuzzing payloads,
        // common credential-naming guesses all rejected.
        for key in &[
            "",
            "wrapper_url",
            "musickit_private_key_v2",
            "../../../etc/passwd",
            "musickit_private_key\0extra",
            "MusicKit_Private_Key", // case-sensitive — exact match required
            "musickit_private_key ", // trailing space — exact match required
        ] {
            assert!(
                check_credential_key_allowed(key).is_err(),
                "key '{}' should be rejected by allowlist",
                key
            );
        }
    }

    #[test]
    fn allowlist_error_message_is_generic_not_enumerable() {
        // Verify the error response doesn't leak allowlist contents —
        // an attacker shouldn't be able to use the error string to
        // enumerate which keys are valid.
        let err1 = check_credential_key_allowed("alpha_key").unwrap_err();
        let err2 = check_credential_key_allowed("beta_key").unwrap_err();

        // Both errors should be the same generic shape (just the key
        // echoed back), NOT something like "alpha_key — try
        // 'musickit_private_key' instead".
        assert!(err1.starts_with("Unknown credential key:"));
        assert!(err2.starts_with("Unknown credential key:"));

        // And critically, MUST NOT mention any allowed key.
        for allowed in ALLOWED_CREDENTIAL_KEYS {
            assert!(
                !err1.contains(allowed),
                "error must not enumerate allowed key '{}'",
                allowed
            );
            assert!(
                !err2.contains(allowed),
                "error must not enumerate allowed key '{}'",
                allowed
            );
        }
    }
}
