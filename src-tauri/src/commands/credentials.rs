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

/// The developer-access passphrase record, injected at build time through the
/// `DEV_ACCESS_HASH` secret. The passphrase itself never appears in the
/// binary; only this record does.
///
/// **When this is `None` (or empty), developer access cannot be unlocked at
/// all.** It used to fall back to the hash of an empty string, which turned
/// "not configured" into "pressing the button without typing anything works"
/// (#1162). No configured record now means no way in.
///
/// **Format** (see `scripts/make-dev-access-hash.mjs`, which writes it):
///
/// ```text
/// pbkdf2-sha256$<rounds>$<salt as hex>$<derived key as hex>
/// ```
///
/// Why a slow, salted check rather than one SHA-256 hash: this record ships
/// inside every copy of the app, so anyone can pull it out and guess offline,
/// where the five-a-minute limit in [`activate_dev_access`] does not apply.
/// One SHA-256 hash lets a graphics card try billions of guesses a second;
/// PBKDF2 with hundreds of thousands of rounds makes each guess that many
/// times more expensive. The salt stops one precomputed list working against
/// every build. A record in the old single-SHA-256 form is refused outright,
/// so a fast hash can never quietly stay in use (CodeQL `rust/weak-sensitive-
/// data-hashing`).
const DEV_ACCESS_HASH: Option<&str> = option_env!("DEV_ACCESS_HASH");

/// The only scheme a passphrase record may use. See [`DEV_ACCESS_HASH`].
const DEV_ACCESS_SCHEME: &str = "pbkdf2-sha256";

/// The fewest rounds a passphrase record is allowed to ask for. A record with
/// fewer is refused, so a weak record cannot be configured by mistake. The
/// script that writes records uses 600,000, OWASP's figure for PBKDF2-SHA256.
const DEV_ACCESS_MIN_ROUNDS: u32 = 100_000;

/// The most rounds a passphrase record may ask for. A record with more is
/// refused rather than run: a mistyped build secret with a few extra digits
/// would otherwise hold a thread for minutes or hours on every unlock attempt,
/// which looks like a hang instead of a refusal.
const DEV_ACCESS_MAX_ROUNDS: u32 = 10_000_000;

/// The shortest salt, in bytes, a passphrase record may carry.
const DEV_ACCESS_MIN_SALT_BYTES: usize = 16;

/// The length, in bytes, of the derived key a passphrase record stores.
const DEV_ACCESS_KEY_BYTES: usize = 32;

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

/// A passphrase record read out of [`DEV_ACCESS_HASH`].
struct DevAccessRecord {
    /// How many PBKDF2 rounds to run.
    rounds: u32,
    /// The random salt the record was made with.
    salt: Vec<u8>,
    /// The key PBKDF2 produced from the real passphrase.
    key: Vec<u8>,
}

/// Turns a string of hexadecimal digits into bytes.
///
/// Returns `None` for an odd length or anything that is not a hex digit, so a
/// damaged record is refused rather than half-read. The digit check is needed:
/// `u8::from_str_radix` on its own would accept a pair such as `+1`.
fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(digits, 16).ok()
        })
        .collect()
}

/// Reads a passphrase record, or returns `None` if it is not one this build
/// accepts: wrong scheme (including the old single-SHA-256 form), too few or
/// too many rounds, too short a salt, a key of the wrong length, or damaged
/// text.
fn parse_dev_access_record(text: &str) -> Option<DevAccessRecord> {
    let mut parts = text.trim().split('$');
    let (Some(scheme), Some(rounds), Some(salt), Some(key), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return None;
    };
    if scheme != DEV_ACCESS_SCHEME {
        return None;
    }
    let rounds: u32 = rounds.parse().ok()?;
    let salt = decode_hex(salt)?;
    let key = decode_hex(key)?;
    if !(DEV_ACCESS_MIN_ROUNDS..=DEV_ACCESS_MAX_ROUNDS).contains(&rounds)
        || salt.len() < DEV_ACCESS_MIN_SALT_BYTES
        || key.len() != DEV_ACCESS_KEY_BYTES
    {
        return None;
    }
    Some(DevAccessRecord { rounds, salt, key })
}

/// Compares two byte strings without stopping at the first difference, so the
/// time taken does not hint at how much of a guess was right.
fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// Decides whether a typed passphrase unlocks developer access.
///
/// Split out from [`activate_dev_access`] so the rules can be tested on their
/// own — the command itself needs a running app and writes to the OS keychain,
/// neither of which belongs in a unit test.
///
/// Refused, in this order:
///
/// 1. **An empty entry.** Blank, or nothing but whitespace. The prompt's text
///    box starts empty, so accepting an empty value would mean the gate opens
///    by pressing the button (#1162).
/// 2. **A build with no passphrase configured.** No way in at all (#1162).
/// 3. **A record this build does not accept** — see
///    [`parse_dev_access_record`]. Logged as a warning, because it means the
///    build secret needs regenerating, not that someone typed it wrong.
/// 4. **A passphrase whose PBKDF2 key does not match the record.**
///
/// Slow on purpose (see [`DEV_ACCESS_HASH`]), so call it off the async
/// runtime's worker threads.
///
/// # Arguments
///
/// * `passphrase` -- What the user typed, compared exactly (not trimmed).
/// * `configured` -- The passphrase record built into this binary, if any.
fn passphrase_is_accepted(passphrase: &str, configured: Option<&str>) -> bool {
    // Rule 1: nothing typed means nothing unlocked.
    if passphrase.trim().is_empty() {
        return false;
    }

    // Rule 2: no passphrase configured for this build means no way in. An
    // uncreated build secret arrives as an empty string, so treat that the same.
    let Some(configured) = configured.filter(|c| !c.trim().is_empty()) else {
        log::debug!("dev access: no passphrase configured for this build — refused");
        return false;
    };

    // Rule 3: only a record in the current, slow form is accepted.
    let Some(record) = parse_dev_access_record(configured) else {
        log::warn!(
            "dev access: the passphrase record built into this app is not in the \
             expected form, so developer access cannot be unlocked. Regenerate the \
             DEV_ACCESS_HASH build secret with scripts/make-dev-access-hash.mjs."
        );
        return false;
    };

    // Rule 4: derive a key from what was typed and compare it with the record.
    let mut derived = vec![0u8; record.key.len()];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
        passphrase.as_bytes(),
        &record.salt,
        record.rounds,
        &mut derived,
    );
    same_bytes(&derived, &record.key)
}

/// Activates developer access after validating the passphrase.
///
/// **Frontend caller:** `activateDevAccess(passphrase)` in `src/lib/tauri-commands.ts`
///
/// The passphrase is checked by [`passphrase_is_accepted`]. On success, stores
/// a keychain sentinel and enables `dev_access_enabled` in settings. Returns
/// whether activation succeeded.
///
/// On failure, returns `false` silently (no error hint to prevent brute-forcing).
///
/// **Rate-limited** at 5 attempts per 60-second sliding window (#940), which
/// slows guessing through the app itself (post-XSS, hostile markdown, a
/// compromised npm dependency). Guessing offline against the record pulled out
/// of the binary is slowed by the record's PBKDF2 rounds instead. Legit users
/// type the passphrase by hand so the cap has zero UX cost. Mirrors the
/// existing rate-limiter pattern used by `start_download`,
/// `import_cookies_from_browser`, and `check_all_updates`.
#[tauri::command]
pub async fn activate_dev_access(app: tauri::AppHandle, passphrase: String) -> bool {
    // Reject after 5 attempts in the rolling 60-second window. The IPC
    // returns `-> bool` (not Result) so we map the rate-limit Err to
    // a plain `false` — visually indistinguishable to the attacker from
    // a wrong-passphrase rejection, denying them a usable oracle.
    if crate::utils::rate_limiter::check_rate_limit("activate_dev_access", 5, 60).is_err() {
        log::warn!("activate_dev_access: rate-limited (possible brute-force attempt)");
        return false;
    }

    // The check runs hundreds of thousands of hash rounds on purpose, which can
    // take a second or two on a slow machine, so it runs on a blocking thread
    // rather than holding up the async runtime. A failed thread counts as "no".
    let accepted = tauri::async_runtime::spawn_blocking(move || {
        passphrase_is_accepted(&passphrase, DEV_ACCESS_HASH)
    })
    .await
    .unwrap_or(false);
    if !accepted {
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

    // ---- Developer access passphrase ----

    /// A record for the passphrase `a-real-passphrase`, made OUTSIDE Rust with
    /// Node's own PBKDF2 — the same calculation `scripts/make-dev-access-hash.mjs`
    /// does (fixed salt here so the test is repeatable, and the minimum rounds
    /// so it stays quick):
    ///
    /// ```text
    /// node -e "const c=require('crypto');const s=Buffer.from('000102030405060708090a0b0c0d0e0f','hex');
    ///   console.log(c.pbkdf2Sync('a-real-passphrase',s,100000,32,'sha256').toString('hex'))"
    /// ```
    ///
    /// If this test ever fails, the script and the app no longer agree.
    const REAL_RECORD: &str = "pbkdf2-sha256$100000$000102030405060708090a0b0c0d0e0f$\
                               e1127b2a122aebc5472aca32a3d89fbceaf7acf51884ac5e9f9a8d7f219e6967";

    /// Builds a record in Rust with a chosen number of rounds and salt, for the
    /// tests that check the record rules rather than the calculation.
    fn record_for(passphrase: &str, rounds: u32, salt: &[u8]) -> String {
        let mut key = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(passphrase.as_bytes(), salt, rounds, &mut key);
        let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        format!("pbkdf2-sha256${rounds}${}${}", hex(salt), hex(&key))
    }

    #[test]
    fn empty_passphrase_is_refused_when_nothing_is_configured() {
        // The #1162 hole: a build with no configured passphrase, and a user who
        // pressed the button without typing anything.
        assert!(!passphrase_is_accepted("", None));
    }

    #[test]
    fn whitespace_only_passphrase_is_refused() {
        // A box containing only spaces is still an empty box.
        assert!(!passphrase_is_accepted("   ", None));
        assert!(!passphrase_is_accepted("\t \n", None));
        assert!(!passphrase_is_accepted("   ", Some(REAL_RECORD)));
    }

    #[test]
    fn nothing_unlocks_a_build_with_no_configured_passphrase() {
        // Not just the empty case: no record built in means no way in at all.
        assert!(!passphrase_is_accepted(
            "correct horse battery staple",
            None
        ));
        assert!(!passphrase_is_accepted("a", None));
    }

    #[test]
    fn an_empty_build_secret_counts_as_not_configured() {
        // A build secret that was never created arrives as an empty string.
        assert!(!passphrase_is_accepted("a-real-passphrase", Some("")));
        assert!(!passphrase_is_accepted("a-real-passphrase", Some("  ")));
    }

    #[test]
    fn the_configured_passphrase_is_accepted() {
        // The feature still works when it is set up properly, using a record
        // made by Node, so the script and the app are proven to agree.
        assert!(passphrase_is_accepted(
            "a-real-passphrase",
            Some(REAL_RECORD)
        ));
    }

    #[test]
    fn a_wrong_passphrase_is_refused() {
        assert!(!passphrase_is_accepted(
            "not-the-passphrase",
            Some(REAL_RECORD)
        ));
        // Case matters, and so does surrounding whitespace: only the empty
        // check trims, the comparison itself does not.
        assert!(!passphrase_is_accepted(
            "A-Real-Passphrase",
            Some(REAL_RECORD)
        ));
        assert!(!passphrase_is_accepted(
            " a-real-passphrase ",
            Some(REAL_RECORD)
        ));
    }

    #[test]
    fn the_old_fast_hash_form_is_refused_even_with_the_right_passphrase() {
        // A record in the old single-SHA-256 form must never unlock anything,
        // so a fast hash cannot quietly stay in use after this change.
        use sha2::{Digest, Sha256};
        let old_form = format!("{:x}", Sha256::digest(b"a-real-passphrase"));
        assert!(!passphrase_is_accepted(
            "a-real-passphrase",
            Some(&old_form)
        ));
    }

    #[test]
    fn a_record_with_too_few_rounds_is_refused() {
        let salt = [7u8; 16];
        let weak = record_for("a-real-passphrase", DEV_ACCESS_MIN_ROUNDS - 1, &salt);
        assert!(parse_dev_access_record(&weak).is_none());
        assert!(!passphrase_is_accepted("a-real-passphrase", Some(&weak)));
    }

    #[test]
    fn a_record_with_too_many_rounds_is_refused() {
        // Built by hand rather than derived: running this many rounds is
        // exactly the hang the ceiling exists to prevent.
        let too_many = format!(
            "pbkdf2-sha256${}$000102030405060708090a0b0c0d0e0f${}",
            u64::from(DEV_ACCESS_MAX_ROUNDS) + 1,
            "00".repeat(32)
        );
        assert!(parse_dev_access_record(&too_many).is_none());
        let at_ceiling = format!(
            "pbkdf2-sha256${DEV_ACCESS_MAX_ROUNDS}$000102030405060708090a0b0c0d0e0f${}",
            "00".repeat(32)
        );
        assert!(parse_dev_access_record(&at_ceiling).is_some());
    }

    #[test]
    fn a_record_with_a_short_salt_is_refused() {
        let short = record_for("a-real-passphrase", DEV_ACCESS_MIN_ROUNDS, &[7u8; 8]);
        assert!(parse_dev_access_record(&short).is_none());
    }

    #[test]
    fn damaged_or_foreign_records_are_refused() {
        let key = "e1127b2a122aebc5472aca32a3d89fbceaf7acf51884ac5e9f9a8d7f219e6967";
        let salt = "000102030405060708090a0b0c0d0e0f";
        for bad in [
            // Wrong scheme name.
            format!("pbkdf2-sha512$100000${salt}${key}"),
            // Missing field.
            format!("pbkdf2-sha256$100000${key}"),
            // Extra field.
            format!("pbkdf2-sha256$100000${salt}${key}$extra"),
            // Rounds not a number.
            format!("pbkdf2-sha256$lots${salt}${key}"),
            // Key one byte short.
            format!("pbkdf2-sha256$100000${salt}${}", &key[..62]),
            // Not hexadecimal.
            format!("pbkdf2-sha256$100000${salt}${}zz", &key[..62]),
            // Odd number of hex digits.
            format!("pbkdf2-sha256$100000${salt}${key}0"),
        ] {
            assert!(parse_dev_access_record(&bad).is_none(), "accepted: {bad}");
        }
        assert!(parse_dev_access_record(REAL_RECORD).is_some());
    }

    #[test]
    fn hex_decoding_and_byte_comparison() {
        assert_eq!(decode_hex("00ff10"), Some(vec![0x00, 0xff, 0x10]));
        assert_eq!(decode_hex("ABcd"), Some(vec![0xab, 0xcd]));
        assert_eq!(decode_hex(""), Some(vec![]));
        assert_eq!(decode_hex("abc"), None);
        assert_eq!(decode_hex("+1"), None);
        assert!(same_bytes(b"abc", b"abc"));
        assert!(!same_bytes(b"abc", b"abd"));
        assert!(!same_bytes(b"abc", b"ab"));
    }
}
