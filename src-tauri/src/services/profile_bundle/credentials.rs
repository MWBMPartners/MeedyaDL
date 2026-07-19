// Copyright (c) 2026 MeedyaSuite
// Licensed under the MIT License.
//
// .meedyabundle credential encryption-at-rest (#876 P4).
//
// Provides the AES-256-GCM + PBKDF2-HMAC-SHA256 crypto primitives
// used by the export / import IPC to seal user credentials
// (cookies, MusicKit JWT, web-player developer token) into the
// bundle without leaking them on disk.
//
// Threat model: someone gets the bundle file. They MUST NOT be
// able to recover the plaintext credentials without the password
// the exporting user set. The GCM authentication tag also catches
// tamper / corruption on import — the wrong password fails with
// `CredentialError::AuthenticationFailed` rather than producing
// garbage plaintext.
//
// Parameters per the EPIC body:
//   * Salt: 32 random bytes (CSPRNG)
//   * KDF: PBKDF2-HMAC-SHA256, 600 000 iterations (OWASP 2026)
//   * Cipher: AES-256-GCM (16-byte authentication tag)
//   * Nonce: 12 random bytes (CSPRNG)
//
// The on-disk shape is two files inside the archive:
//   * `credentials.encrypted` — the raw ciphertext + 16-byte
//     GCM tag concatenated (matches the `aes-gcm` crate's
//     `Aes256Gcm::encrypt` output).
//   * `credentials.kdf-params.json` — { algorithm, iterations,
//     salt_b64, nonce_b64 } so the importer can reconstruct the
//     same key + nonce from the user's password.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;

/// OWASP 2026 minimum for PBKDF2-HMAC-SHA256. Bump on a future
/// `KdfParams.version` if/when guidance moves.
pub const PBKDF2_ITERATIONS: u32 = 600_000;
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32; // AES-256

/// Errors surfaced by the encrypt/decrypt path. Deliberately
/// narrow so the caller can show the user a precise message
/// (especially "wrong password" vs "corrupt bundle").
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("credential encryption error: {0}")]
    Cipher(String),

    /// The GCM authentication tag did not verify on decrypt. This
    /// is the canonical "wrong password OR corrupt bundle" path —
    /// the cipher cannot distinguish the two cases (by design).
    #[error("credentials could not be decrypted: wrong password or corrupt bundle")]
    AuthenticationFailed,

    #[error("kdf params parse error: {0}")]
    KdfParams(String),

    #[error("base64 decode error: {0}")]
    Base64(String),
}

/// On-disk KDF-params shape (serialised as
/// `credentials.kdf-params.json` inside the bundle).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KdfParams {
    /// Stable string so future migrations can fork on it. Always
    /// `"pbkdf2-hmac-sha256+aes-256-gcm"` for now.
    pub algorithm: String,
    pub iterations: u32,
    pub salt_b64: String,
    pub nonce_b64: String,
}

impl KdfParams {
    fn new(iterations: u32, salt: &[u8], nonce: &[u8]) -> Self {
        let b64 = base64::engine::general_purpose::STANDARD;
        Self {
            algorithm: "pbkdf2-hmac-sha256+aes-256-gcm".to_string(),
            iterations,
            salt_b64: b64.encode(salt),
            nonce_b64: b64.encode(nonce),
        }
    }

    fn salt(&self) -> Result<Vec<u8>, CredentialError> {
        base64::engine::general_purpose::STANDARD
            .decode(&self.salt_b64)
            .map_err(|e| CredentialError::Base64(e.to_string()))
    }

    fn nonce(&self) -> Result<Vec<u8>, CredentialError> {
        base64::engine::general_purpose::STANDARD
            .decode(&self.nonce_b64)
            .map_err(|e| CredentialError::Base64(e.to_string()))
    }
}

/// Output of [`encrypt_credentials`] — both files the caller
/// needs to add to the bundle.
#[derive(Debug, Clone)]
pub struct EncryptedCredentials {
    /// AES-256-GCM ciphertext including the 16-byte authentication
    /// tag (the `aes-gcm` crate appends it automatically).
    pub ciphertext: Vec<u8>,
    /// Parameters needed by the importer to reconstruct the key +
    /// nonce. Caller serialises this with `serde_json::to_vec`.
    pub kdf_params: KdfParams,
}

/// Encrypt `plaintext` under `password`. Generates a fresh
/// 32-byte salt + 12-byte nonce per call; both are returned in
/// the `KdfParams` so the importer can derive the same key.
///
/// `plaintext` is expected to be the JSON-serialised credential
/// blob (cookies file content + MusicKit JWT + web-player token).
/// This module is intentionally agnostic about the schema so it
/// can wrap anything.
pub fn encrypt_credentials(
    password: &str,
    plaintext: &[u8],
) -> Result<EncryptedCredentials, CredentialError> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

    let key = derive_key(password, &salt, PBKDF2_ITERATIONS);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| CredentialError::Cipher(e.to_string()))?;

    Ok(EncryptedCredentials {
        ciphertext,
        kdf_params: KdfParams::new(PBKDF2_ITERATIONS, &salt, &nonce_bytes),
    })
}

/// Decrypt `ciphertext` under `password` using the stored
/// `params`. Returns `Err(AuthenticationFailed)` on wrong password
/// or tampered ciphertext.
pub fn decrypt_credentials(
    password: &str,
    ciphertext: &[u8],
    params: &KdfParams,
) -> Result<Vec<u8>, CredentialError> {
    let salt = params.salt()?;
    let nonce_bytes = params.nonce()?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(CredentialError::KdfParams(format!(
            "nonce length {} bytes is invalid (expected {})",
            nonce_bytes.len(),
            NONCE_LEN
        )));
    }
    // Denial-of-service guard: `params.iterations` comes straight from
    // the untrusted bundle file (`credentials.kdf-params.json`). A
    // crafted bundle could declare an absurd iteration count to turn a
    // routine import into a multi-minute (or effectively unbounded)
    // PBKDF2 hang before the wrong-password/right-password outcome is
    // even known. Clamp to a sane range — below this floor is a
    // trivially weak KDF (not our concern to accept), above this ceiling
    // is almost certainly a hostile value rather than a legitimate
    // future security bump — and reject anything outside it before key
    // derivation starts.
    const MIN_ITERATIONS: u32 = 10_000;
    const MAX_ITERATIONS: u32 = 5_000_000;
    if !(MIN_ITERATIONS..=MAX_ITERATIONS).contains(&params.iterations) {
        return Err(CredentialError::KdfParams(format!(
            "iteration count {} is outside the accepted range ({MIN_ITERATIONS}..={MAX_ITERATIONS})",
            params.iterations
        )));
    }
    let key = derive_key(password, &salt, params.iterations);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CredentialError::AuthenticationFailed)
}

fn derive_key(password: &str, salt: &[u8], iterations: u32) -> [u8; KEY_LEN] {
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_succeeds_with_correct_password() {
        let plaintext = b"{\"cookies\":\"...\",\"musickit_jwt\":\"...\"}";
        let enc = encrypt_credentials("hunter2", plaintext).unwrap();
        let decrypted =
            decrypt_credentials("hunter2", &enc.ciphertext, &enc.kdf_params).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_password_fails_with_authentication_failed() {
        let enc = encrypt_credentials("right", b"payload").unwrap();
        let err = decrypt_credentials("WRONG", &enc.ciphertext, &enc.kdf_params)
            .expect_err("must reject wrong password");
        match err {
            CredentialError::AuthenticationFailed => {}
            other => panic!("expected AuthenticationFailed, got {other:?}"),
        }
    }

    #[test]
    fn tampered_ciphertext_fails_with_authentication_failed() {
        let mut enc = encrypt_credentials("hunter2", b"payload").unwrap();
        // Flip a single bit in the middle of the ciphertext.
        let mid = enc.ciphertext.len() / 2;
        enc.ciphertext[mid] ^= 0x01;
        let err = decrypt_credentials("hunter2", &enc.ciphertext, &enc.kdf_params)
            .expect_err("tampered ciphertext must fail auth");
        assert!(matches!(err, CredentialError::AuthenticationFailed));
    }

    #[test]
    fn two_encryptions_of_same_plaintext_produce_different_ciphertexts() {
        // Fresh salt + nonce per call means ciphertexts must NOT
        // be identical even when the password and plaintext match.
        // This is what protects against rainbow-table attacks.
        let a = encrypt_credentials("pw", b"same").unwrap();
        let b = encrypt_credentials("pw", b"same").unwrap();
        assert_ne!(a.ciphertext, b.ciphertext);
        assert_ne!(a.kdf_params.salt_b64, b.kdf_params.salt_b64);
        assert_ne!(a.kdf_params.nonce_b64, b.kdf_params.nonce_b64);
    }

    #[test]
    fn kdf_params_round_trip_through_json() {
        // The KdfParams blob is serialised + stored in the bundle
        // as `credentials.kdf-params.json`. JSON round-trip must
        // preserve every field bit-identical or import breaks.
        let enc = encrypt_credentials("pw", b"payload").unwrap();
        let json = serde_json::to_string(&enc.kdf_params).unwrap();
        let parsed: KdfParams = serde_json::from_str(&json).unwrap();
        let decrypted = decrypt_credentials("pw", &enc.ciphertext, &parsed).unwrap();
        assert_eq!(decrypted, b"payload");
    }

    #[test]
    fn kdf_params_advertises_correct_algorithm_string() {
        let enc = encrypt_credentials("pw", b"payload").unwrap();
        assert_eq!(
            enc.kdf_params.algorithm,
            "pbkdf2-hmac-sha256+aes-256-gcm"
        );
        assert_eq!(enc.kdf_params.iterations, PBKDF2_ITERATIONS);
    }

    // ----------------------------------------------------------
    // Iteration-count DoS guard (security audit F8)
    // ----------------------------------------------------------

    #[test]
    fn decrypt_rejects_absurdly_high_iteration_count() {
        let mut enc = encrypt_credentials("pw", b"payload").unwrap();
        // A crafted bundle could declare billions of iterations to hang
        // the import on PBKDF2 before the password is even checked.
        enc.kdf_params.iterations = 50_000_000;
        let err = decrypt_credentials("pw", &enc.ciphertext, &enc.kdf_params)
            .expect_err("must reject implausible iteration count");
        match err {
            CredentialError::KdfParams(msg) => {
                assert!(msg.contains("iteration count"), "unexpected message: {msg}");
            }
            other => panic!("expected KdfParams error, got {other:?}"),
        }
    }

    #[test]
    fn decrypt_rejects_suspiciously_low_iteration_count() {
        let mut enc = encrypt_credentials("pw", b"payload").unwrap();
        enc.kdf_params.iterations = 1;
        let err = decrypt_credentials("pw", &enc.ciphertext, &enc.kdf_params)
            .expect_err("must reject a trivially weak iteration count");
        assert!(matches!(err, CredentialError::KdfParams(_)));
    }

    #[test]
    fn decrypt_accepts_default_iteration_count() {
        // Sanity check: the standard round trip (PBKDF2_ITERATIONS) must
        // not be affected by the new range guard.
        let enc = encrypt_credentials("pw", b"payload").unwrap();
        assert!(decrypt_credentials("pw", &enc.ciphertext, &enc.kdf_params).is_ok());
    }
}
