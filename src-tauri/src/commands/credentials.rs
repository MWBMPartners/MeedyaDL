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

/// The service name used as the namespace in the OS keychain.
/// All credentials stored by this app use this identifier.
const SERVICE_NAME: &str = "com.mwbmpartners.gamdl-gui";

/// Stores a credential securely in the OS keychain.
///
/// If a credential with the same key already exists, it is overwritten.
/// This is used for storing wrapper URLs, future API keys for
/// YouTube Music / Spotify integrations, and other sensitive data.
///
/// # Arguments
/// * `key` - A unique identifier for the credential (e.g., "wrapper_url")
/// * `value` - The secret value to store
#[tauri::command]
pub async fn store_credential(key: String, value: String) -> Result<(), String> {
    // Create a keyring entry using the app's service name and the provided key
    let entry = keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;

    // Store the credential in the OS keychain
    entry
        .set_password(&value)
        .map_err(|e| format!("Failed to store credential '{}': {}", key, e))?;

    log::info!("Credential '{}' stored securely", key);
    Ok(())
}

/// Retrieves a credential from the OS keychain.
///
/// Returns None if the credential doesn't exist (never been stored).
/// Returns an error if the keychain is locked or access is denied.
///
/// # Arguments
/// * `key` - The unique identifier used when storing the credential
#[tauri::command]
pub async fn get_credential(key: String) -> Result<Option<String>, String> {
    // Create a keyring entry to look up
    let entry = keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;

    // Attempt to retrieve the stored password
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        // NoEntry means the credential was never stored - this is not an error
        Err(keyring::Error::NoEntry) => Ok(None),
        // Any other error (locked keychain, permission denied, etc.)
        Err(e) => Err(format!("Failed to retrieve credential '{}': {}", key, e)),
    }
}

/// Deletes a credential from the OS keychain.
///
/// Returns Ok(()) even if the credential didn't exist (idempotent).
///
/// # Arguments
/// * `key` - The unique identifier of the credential to delete
#[tauri::command]
pub async fn delete_credential(key: String) -> Result<(), String> {
    // Create a keyring entry to delete
    let entry = keyring::Entry::new(SERVICE_NAME, &key)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;

    // Delete the credential; ignore "not found" errors
    match entry.delete_credential() {
        Ok(()) => {
            log::info!("Credential '{}' deleted", key);
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            // Credential didn't exist - that's fine
            Ok(())
        }
        Err(e) => Err(format!("Failed to delete credential '{}': {}", key, e)),
    }
}
