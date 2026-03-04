// Copyright (c) 2024-2026 MeedyaDL
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
    // Create a keyring entry handle for the (service, key) pair.
    // Entry::new() can fail if the OS keychain backend is unavailable.
    // See: https://docs.rs/keyring/latest/keyring/struct.Entry.html#method.new
    let entry = keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| format!("Failed to create keyring entry: {e}"))?;

    // Store the credential in the OS keychain.
    // set_password() creates or overwrites the credential atomically.
    entry
        .set_password(&value)
        .map_err(|e| format!("Failed to store credential '{key}': {e}"))?;

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
    // Create a keyring entry handle for the lookup
    let entry = keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| format!("Failed to create keyring entry: {e}"))?;

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
    // Create a keyring entry handle for the deletion
    let entry = keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| format!("Failed to create keyring entry: {e}"))?;

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
) -> Result<String, String> {
    use crate::services::{apple_music_api, config_service};

    // 1. Load settings for Team ID and Key ID
    let settings = config_service::load_settings(&app).unwrap_or_default();

    let team_id = settings
        .musickit_team_id
        .as_ref()
        .filter(|s| !s.is_empty())
        .ok_or("MusicKit Team ID not configured. Enter your 10-character Team ID in Settings > Cover Art.")?;

    let key_id = settings
        .musickit_key_id
        .as_ref()
        .filter(|s| !s.is_empty())
        .ok_or("MusicKit Key ID not configured. Enter your 10-character Key ID in Settings > Cover Art.")?;

    // 2. Get private key from OS keychain
    let private_key = apple_music_api::get_private_key_from_keychain()
        .map_err(|e| format!("Keychain error: {e}"))?
        .ok_or(
            "MusicKit private key not found in OS keychain. \
             Paste your .p8 private key content in Settings > Cover Art and click 'Save to Keychain'.",
        )?;

    // 3. Generate JWT
    let jwt = apple_music_api::generate_musickit_jwt(team_id, key_id, &private_key)
        .map_err(|e| format!("JWT generation failed: {e}. Check that your private key is a valid .p8 PEM file."))?;

    log::info!("MusicKit validation: JWT generated (iss={team_id}, kid={key_id})");

    // 4. Make a lightweight test API call to a known public album
    // Using "Abbey Road" by The Beatles — a universally available album
    let test_url = "https://amp-api.music.apple.com/v1/catalog/us/albums/1441164495";

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    let response = client
        .get(test_url)
        .header("Authorization", format!("Bearer {jwt}"))
        .header("User-Agent", "meedyadl")
        .header("Origin", "https://music.apple.com")
        .send()
        .await
        .map_err(|e| format!("Network error: {e}. Check your internet connection."))?;

    let status = response.status().as_u16();
    match status {
        200 => {
            log::info!("MusicKit validation: credentials valid (HTTP 200)");
            Ok("MusicKit credentials are valid! API authentication successful.".to_string())
        }
        401 => {
            log::warn!("MusicKit validation: authentication failed (HTTP 401)");
            Err(
                "Authentication failed (HTTP 401). Your MusicKit credentials are invalid. \
                 Check on developer.apple.com that: (1) your Team ID matches, \
                 (2) your Key ID has not been revoked, (3) your private key (.p8) \
                 matches the Key ID."
                    .to_string(),
            )
        }
        403 => {
            log::warn!("MusicKit validation: forbidden (HTTP 403)");
            Err(
                "Forbidden (HTTP 403). Your MusicKit key may not have the required \
                 permissions. Ensure the MusicKit service is enabled for your key \
                 on developer.apple.com."
                    .to_string(),
            )
        }
        429 => Err("Rate limited (HTTP 429). Apple Music API is temporarily limiting requests. Try again in a few minutes.".to_string()),
        404 => {
            // 404 means the JWT authenticated successfully (didn't get 401),
            // but the test album wasn't found. Credentials are valid.
            log::info!("MusicKit validation: credentials valid (HTTP 404 — test album not found, auth OK)");
            Ok("MusicKit credentials are valid! API authentication successful.".to_string())
        }
        _ => Err(format!("Unexpected HTTP {status} from Apple Music API. Try again later.")),
    }
}
