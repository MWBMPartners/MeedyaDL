// Copyright (c) 2024-2026 MWBM Partners Ltd
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
///   Service: "com.mwbmpartners.gamdl-gui"
///   Account: "wrapper_url" (or whatever key was used)
///
/// See: https://docs.rs/keyring/latest/keyring/struct.Entry.html
const SERVICE_NAME: &str = "com.mwbmpartners.gamdl-gui";

/// Stores a credential securely in the OS keychain.
///
/// **Frontend caller:** `storeCredential(key, value)` in `src/lib/tauri-commands.ts`
///
/// If a credential with the same key already exists, it is overwritten.
/// This is used for storing wrapper URLs, future API keys for
/// YouTube Music / Spotify integrations, and other sensitive data.
///
/// The credential is stored using the OS native secure storage:
/// - macOS: Keychain Services (encrypted, requires user authentication)
/// - Windows: Credential Manager (DPAPI encrypted)
/// - Linux: Secret Service API (GNOME Keyring or KWallet)
///
/// # Arguments
/// * `key` - A unique identifier for the credential (e.g., "wrapper_url").
///   This becomes the "account" field in the keychain entry.
/// * `value` - The secret value to store. Stored as the "password" field.
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
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;

    // Store the credential in the OS keychain.
    // set_password() creates or overwrites the credential atomically.
    entry
        .set_password(&value)
        .map_err(|e| format!("Failed to store credential '{}': {}", key, e))?;

    // Log the key name only (never the value) for debugging and auditing
    log::info!("Credential '{}' stored securely", key);
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
///   (e.g., "wrapper_url").
///
/// # Returns
/// * `Ok(Some(String))` - The credential value was retrieved successfully.
/// * `Ok(None)` - No credential exists for this key (never been stored).
/// * `Err(String)` - Keychain access error (locked, permission denied, etc.).
#[tauri::command]
pub async fn get_credential(key: String) -> Result<Option<String>, String> {
    // Create a keyring entry handle for the lookup
    let entry = keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;

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
        Err(e) => Err(format!("Failed to retrieve credential '{}': {}", key, e)),
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
///   (e.g., "wrapper_url").
///
/// # Returns
/// * `Ok(())` - Credential deleted, or it didn't exist (both are success).
/// * `Err(String)` - Keychain access error (locked, permission denied, etc.).
#[tauri::command]
pub async fn delete_credential(key: String) -> Result<(), String> {
    // Create a keyring entry handle for the deletion
    let entry = keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;

    // Attempt to delete the credential from the OS keychain.
    // We explicitly handle NoEntry as a success case for idempotency.
    match entry.delete_credential() {
        Ok(()) => {
            log::info!("Credential '{}' deleted", key);
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            // Credential didn't exist — this is fine, deletion is idempotent.
            // No log message needed since there was nothing to delete.
            Ok(())
        }
        // Any other error is a real failure (locked keychain, etc.)
        Err(e) => Err(format!("Failed to delete credential '{}': {}", key, e)),
    }
}
