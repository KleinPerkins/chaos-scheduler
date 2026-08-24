//! Envelope encryption for the secret-bearing at-rest fields (ADR 0011).
//!
//! The scheme is classic envelope encryption:
//!
//! - A 256-bit **KEK** (key-encrypting key / master key) lives in the macOS
//!   Keychain (service [`MASTER_KEK_KEYCHAIN_SERVICE`], account
//!   [`MASTER_KEK_KEYCHAIN_ACCOUNT`]), reachable through the reusable
//!   [`crate::keychain::KeyStore`] trait — so tests inject a fake and never
//!   touch the real Keychain (headless CI).
//! - A 256-bit **DEK** (data-encrypting key) encrypts the individual field
//!   values. The DEK is AEAD-wrapped by the KEK and the wrapped DEK is stored
//!   in the `envelope_keys` DB table (created in migration v19). At startup the
//!   KEK is fetched once, the DEK is unwrapped once, and the resulting
//!   [`FieldCipher`] is held in memory for the process lifetime — the KEK is
//!   never re-fetched per operation.
//!
//! Field values are sealed with **XChaCha20-Poly1305** (safe 24-byte random
//! nonces) and bound to their storage location via **AAD** (a stable
//! `table:column`-style context string) so a ciphertext cannot be relocated or
//! swapped between fields. The stored token is
//! `enc:v1:` + base64(nonce ‖ ciphertext ‖ tag); the `enc:v1:` prefix makes
//! both the migration and every write idempotent (an already-encrypted value is
//! never re-encrypted, and a plaintext value is recognisably not yet sealed).
//!
//! **Lost/unreadable KEK ⇒ graceful degradation, never a crash.** If the KEK is
//! absent or unreadable at open the cipher is [`CipherState::Locked`]: encrypted
//! fields decrypt to the distinct [`SECRET_UNAVAILABLE_SENTINEL`] (never the
//! read-scope `__redacted__` sentinel), secret writes are rejected, and all
//! non-secret operation proceeds. A lost KEK means the existing ciphertext is
//! unrecoverable — the operator re-enters secrets through a re-provision path
//! that mints a fresh KEK/DEK without blindly destroying existing ciphertext.

use crate::keychain::KeyStore;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use std::fmt;
use std::sync::{Arc, RwLock};

/// Keychain service for the single master KEK.
pub const MASTER_KEK_KEYCHAIN_SERVICE: &str = "chaos-scheduler-master-kek";
/// Keychain account for the single master KEK.
pub const MASTER_KEK_KEYCHAIN_ACCOUNT: &str = "db-envelope-kek-v1";

/// AEAD algorithm identifier persisted alongside the wrapped DEK.
pub const ENVELOPE_ALGO: &str = "XChaCha20Poly1305";

/// Marker prefix on every sealed value. Lets writes and the migration stay
/// idempotent (already-`enc:v1:` values are passed through unchanged) and lets
/// reads tell ciphertext apart from legacy plaintext.
pub const CIPHERTEXT_PREFIX: &str = "enc:v1:";

/// Distinct sentinel returned when an encrypted field cannot be decrypted
/// because the master key is unavailable (secrets-locked) or the ciphertext no
/// longer verifies (e.g. after a re-provision under a fresh key). Deliberately
/// NOT the read-scope redaction sentinel `__redacted__`, so the UI can tell
/// "hidden on read" apart from "master key unavailable — re-enter this secret".
pub const SECRET_UNAVAILABLE_SENTINEL: &str = "__secret_unavailable__";

/// 256-bit keys, 192-bit (24-byte) XChaCha20 nonces.
const KEY_LEN: usize = 32;
const XNONCE_LEN: usize = 24;

/// AAD binding the wrapped DEK to its purpose, so a wrapped-DEK blob cannot be
/// lifted into another AEAD context.
const DEK_WRAP_AAD: &[u8] = b"chaos-scheduler:envelope:dek-wrap:v1";

/// Failure surface for envelope operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    /// The master key is unavailable (missing/unreadable) — secrets are locked
    /// and a new secret value cannot be sealed.
    SecretsLocked,
    /// AEAD encrypt/decrypt failed (wrong key, tampering, or corruption).
    Crypto(String),
    /// The key store (Keychain) returned an error.
    KeyStore(String),
    /// A stored envelope value (token or wrapped DEK) was malformed.
    Corrupt(String),
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvelopeError::SecretsLocked => write!(
                f,
                "secrets are locked: the master key is unavailable; re-enter secrets to re-provision"
            ),
            EnvelopeError::Crypto(msg) => write!(f, "envelope crypto error: {msg}"),
            EnvelopeError::KeyStore(msg) => write!(f, "envelope key store error: {msg}"),
            EnvelopeError::Corrupt(msg) => write!(f, "envelope corrupt value: {msg}"),
        }
    }
}

impl std::error::Error for EnvelopeError {}

pub type EnvelopeResult<T> = Result<T, EnvelopeError>;

/// Fill an array with cryptographically-strong random bytes. Mirrors the
/// existing API-key/salt generation idiom in `service.rs`.
fn random_bytes<const N: usize>() -> [u8; N] {
    use rand::Rng;
    let mut buf = [0u8; N];
    rand::rng().fill_bytes(&mut buf);
    buf
}

/// Generate a fresh 256-bit key (KEK or DEK).
pub fn generate_key() -> [u8; KEY_LEN] {
    random_bytes::<KEY_LEN>()
}

/// True when `s` is an envelope ciphertext token.
pub fn is_ciphertext(s: &str) -> bool {
    s.starts_with(CIPHERTEXT_PREFIX)
}

/// The in-memory data-key cipher. Holds only the DEK (never the KEK) plus the
/// key version; the AEAD is cheap to (re)construct from the key per call.
pub struct FieldCipher {
    dek: Key,
    version: i64,
}

impl FieldCipher {
    /// Build a cipher from raw DEK bytes and the key version.
    pub fn from_bytes(dek: [u8; KEY_LEN], version: i64) -> Self {
        Self {
            dek: *Key::from_slice(&dek),
            version,
        }
    }

    /// The stored key version (bumped on DEK rotation).
    pub fn version(&self) -> i64 {
        self.version
    }

    /// The raw DEK bytes (used only to re-wrap under a new KEK during KEK
    /// rotation — never persisted or logged in the clear).
    pub fn dek_bytes(&self) -> [u8; KEY_LEN] {
        let mut out = [0u8; KEY_LEN];
        out.copy_from_slice(self.dek.as_slice());
        out
    }

    fn aead(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(&self.dek)
    }

    /// Seal `plaintext`, binding it to `aad`. Empty and already-encrypted values
    /// pass through unchanged so the operation is idempotent.
    pub fn encrypt(&self, aad: &str, plaintext: &str) -> EnvelopeResult<String> {
        if plaintext.is_empty() || is_ciphertext(plaintext) {
            return Ok(plaintext.to_string());
        }
        let nonce_bytes = random_bytes::<XNONCE_LEN>();
        let ciphertext = self
            .aead()
            .encrypt(
                XNonce::from_slice(&nonce_bytes),
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|e| EnvelopeError::Crypto(e.to_string()))?;
        let mut blob = Vec::with_capacity(XNONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);
        Ok(format!("{CIPHERTEXT_PREFIX}{}", B64.encode(blob)))
    }

    /// Open a stored token, binding it to `aad`. A value that is not an envelope
    /// token passes through unchanged (legacy plaintext / already-decrypted).
    pub fn decrypt(&self, aad: &str, stored: &str) -> EnvelopeResult<String> {
        let Some(b64) = stored.strip_prefix(CIPHERTEXT_PREFIX) else {
            return Ok(stored.to_string());
        };
        let blob = B64
            .decode(b64)
            .map_err(|e| EnvelopeError::Corrupt(e.to_string()))?;
        if blob.len() < XNONCE_LEN {
            return Err(EnvelopeError::Corrupt("envelope shorter than nonce".into()));
        }
        let (nonce_bytes, ciphertext) = blob.split_at(XNONCE_LEN);
        let plaintext = self
            .aead()
            .decrypt(
                XNonce::from_slice(nonce_bytes),
                Payload {
                    msg: ciphertext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|e| EnvelopeError::Crypto(e.to_string()))?;
        String::from_utf8(plaintext).map_err(|e| EnvelopeError::Corrupt(e.to_string()))
    }
}

/// AEAD-wrap a DEK under a KEK. Returns `(wrapped_dek, wrap_nonce)`.
pub fn wrap_dek(kek: &[u8; KEY_LEN], dek: &[u8; KEY_LEN]) -> EnvelopeResult<(Vec<u8>, Vec<u8>)> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(kek));
    let nonce_bytes = random_bytes::<XNONCE_LEN>();
    let wrapped = cipher
        .encrypt(
            XNonce::from_slice(&nonce_bytes),
            Payload {
                msg: dek,
                aad: DEK_WRAP_AAD,
            },
        )
        .map_err(|e| EnvelopeError::Crypto(e.to_string()))?;
    Ok((wrapped, nonce_bytes.to_vec()))
}

/// Unwrap a DEK previously produced by [`wrap_dek`]. A wrong KEK or tampered
/// blob fails to verify (returns [`EnvelopeError::Crypto`]).
pub fn unwrap_dek(
    kek: &[u8; KEY_LEN],
    wrapped: &[u8],
    wrap_nonce: &[u8],
) -> EnvelopeResult<[u8; KEY_LEN]> {
    if wrap_nonce.len() != XNONCE_LEN {
        return Err(EnvelopeError::Corrupt("wrap nonce wrong length".into()));
    }
    let cipher = XChaCha20Poly1305::new(Key::from_slice(kek));
    let dek = cipher
        .decrypt(
            XNonce::from_slice(wrap_nonce),
            Payload {
                msg: wrapped,
                aad: DEK_WRAP_AAD,
            },
        )
        .map_err(|e| EnvelopeError::Crypto(e.to_string()))?;
    if dek.len() != KEY_LEN {
        return Err(EnvelopeError::Corrupt("unwrapped DEK wrong length".into()));
    }
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(&dek);
    Ok(out)
}

/// Encode a KEK for storage in the Keychain (which holds UTF-8 strings).
pub fn kek_to_b64(kek: &[u8; KEY_LEN]) -> String {
    B64.encode(kek)
}

/// Decode a KEK read back from the Keychain.
pub fn kek_from_b64(s: &str) -> EnvelopeResult<[u8; KEY_LEN]> {
    let bytes = B64
        .decode(s.trim())
        .map_err(|e| EnvelopeError::Corrupt(e.to_string()))?;
    if bytes.len() != KEY_LEN {
        return Err(EnvelopeError::Corrupt("KEK wrong length".into()));
    }
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// The process-wide cipher state. `Active` holds the unwrapped DEK cipher;
/// `Locked` means the master key was missing/unreadable at open (or a
/// re-provision is pending) so secret writes are rejected and reads of
/// encrypted fields yield [`SECRET_UNAVAILABLE_SENTINEL`].
pub enum CipherState {
    Active(Arc<FieldCipher>),
    Locked,
}

/// Envelope state held by the `Database`: the injected key store (for
/// provisioning/rotation) plus the interior-mutable cipher state so rotation
/// can swap the DEK while the DB is shared behind an `Arc`.
pub struct EnvelopeState {
    key_store: Arc<dyn KeyStore>,
    cipher: RwLock<CipherState>,
}

impl EnvelopeState {
    /// Build unprovisioned state (Locked until `Database::open` provisions it).
    pub fn new(key_store: Arc<dyn KeyStore>) -> Self {
        Self {
            key_store,
            cipher: RwLock::new(CipherState::Locked),
        }
    }

    /// The injected key store (real Keychain in production, fake in tests).
    pub fn key_store(&self) -> &Arc<dyn KeyStore> {
        &self.key_store
    }

    /// Clone out the active cipher, or `None` when locked.
    pub fn cipher(&self) -> Option<Arc<FieldCipher>> {
        match &*self.cipher.read().expect("envelope cipher lock poisoned") {
            CipherState::Active(cipher) => Some(cipher.clone()),
            CipherState::Locked => None,
        }
    }

    /// Whether secrets are currently locked (no usable DEK).
    pub fn is_locked(&self) -> bool {
        self.cipher().is_none()
    }

    /// Install an active cipher (provisioning / rotation / re-provision).
    pub fn set_active(&self, cipher: Arc<FieldCipher>) {
        *self.cipher.write().expect("envelope cipher lock poisoned") = CipherState::Active(cipher);
    }

    /// Drop to the locked state (missing/unreadable master key).
    pub fn set_locked(&self) {
        *self.cipher.write().expect("envelope cipher lock poisoned") = CipherState::Locked;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_encrypt_decrypt_with_aad() {
        let cipher = FieldCipher::from_bytes(generate_key(), 1);
        let token = cipher
            .encrypt("email_config:smtp_password", "hunter2")
            .unwrap();
        assert!(is_ciphertext(&token), "token must carry the enc:v1: prefix");
        assert!(
            !token.contains("hunter2"),
            "plaintext must not appear at rest"
        );
        let plain = cipher
            .decrypt("email_config:smtp_password", &token)
            .unwrap();
        assert_eq!(plain, "hunter2");
    }

    #[test]
    fn wrong_aad_fails_to_decrypt() {
        let cipher = FieldCipher::from_bytes(generate_key(), 1);
        let token = cipher
            .encrypt("workflows:spec_json:secret", "shhh")
            .unwrap();
        // A ciphertext bound to one field location must not open under another.
        assert!(cipher
            .decrypt("workflows:trigger_config:secret", &token)
            .is_err());
    }

    #[test]
    fn empty_and_already_encrypted_pass_through() {
        let cipher = FieldCipher::from_bytes(generate_key(), 1);
        assert_eq!(cipher.encrypt("a:b", "").unwrap(), "");
        let token = cipher.encrypt("a:b", "v").unwrap();
        // Re-encrypting an already-sealed value is a no-op (idempotent writes).
        assert_eq!(cipher.encrypt("a:b", &token).unwrap(), token);
    }

    #[test]
    fn decrypt_passes_through_legacy_plaintext() {
        let cipher = FieldCipher::from_bytes(generate_key(), 1);
        assert_eq!(
            cipher.decrypt("a:b", "legacy-plaintext").unwrap(),
            "legacy-plaintext"
        );
    }

    #[test]
    fn dek_wrap_unwrap_round_trip_and_wrong_kek_fails() {
        let kek = generate_key();
        let dek = generate_key();
        let (wrapped, nonce) = wrap_dek(&kek, &dek).unwrap();
        assert_eq!(unwrap_dek(&kek, &wrapped, &nonce).unwrap(), dek);
        let other = generate_key();
        assert!(unwrap_dek(&other, &wrapped, &nonce).is_err());
    }

    #[test]
    fn kek_b64_round_trips() {
        let kek = generate_key();
        let restored = kek_from_b64(&kek_to_b64(&kek)).unwrap();
        assert_eq!(restored, kek);
    }
}
