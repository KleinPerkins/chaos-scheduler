//! Reusable macOS Keychain-backed key storage for the managed MCP integration.
//!
//! The managed Cursor/MCP API key must never live at rest in a plaintext file
//! (see `SECURITY.md` "MCP config and Git history" + ADR 0010). This module
//! abstracts a single generic-password item behind the [`KeyStore`] trait so
//! the provisioning/offboarding flows can be exercised against an in-memory
//! [`FakeKeyStore`] in tests and NEVER touch the real login Keychain — real
//! Keychain access hangs or fails in headless CI, so injecting a fake is
//! mandatory for those tests.
//!
//! A single managed key is stored at a time (service +
//! account are stable constants); re-minting overwrites it in place.

use std::fmt;

/// Keychain service for the single managed MCP API key.
pub const MANAGED_MCP_KEYCHAIN_SERVICE: &str = "chaos-scheduler-managed-mcp";
/// Keychain account for the single managed MCP API key.
pub const MANAGED_MCP_KEYCHAIN_ACCOUNT: &str = "managed-mcp-api-key";

/// Outcome of a delete that distinguishes a proven removal from an
/// unverifiable one. Offboarding needs "proven absent" to claim a removal, so a
/// backend error that leaves the real state unknown must never be reported as a
/// successful removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    /// The item existed and was deleted.
    Deleted,
    /// The item was already absent — also counts as proven-absent.
    AlreadyAbsent,
    /// The delete could not be verified (an unexpected backend error). Removal
    /// is NOT proven; callers must treat this as needs-attention.
    Unknown,
}

impl DeleteOutcome {
    /// Whether the item is PROVABLY absent after the delete (either removed now
    /// or already gone). [`DeleteOutcome::Unknown`] is deliberately not proven.
    pub fn proven_absent(self) -> bool {
        matches!(self, DeleteOutcome::Deleted | DeleteOutcome::AlreadyAbsent)
    }
}

/// Error surface for a [`KeyStore`]. `Unavailable` is only constructed on
/// non-macOS builds (the compile fallback); `Backend` carries a real
/// platform-keystore error.
#[allow(dead_code)] // `Unavailable` is constructed only in the non-macOS fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyStoreError {
    /// The platform key store is unavailable (e.g. non-macOS builds).
    Unavailable(String),
    /// The backend returned an unexpected error.
    Backend(String),
}

impl fmt::Display for KeyStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyStoreError::Unavailable(msg) => write!(f, "key store unavailable: {msg}"),
            KeyStoreError::Backend(msg) => write!(f, "key store error: {msg}"),
        }
    }
}

impl std::error::Error for KeyStoreError {}

pub type KeyStoreResult<T> = Result<T, KeyStoreError>;

/// Abstraction over a single generic-password item store, so the managed-MCP
/// flows can inject a fake in tests and use the real macOS Keychain in
/// production.
pub trait KeyStore: Send + Sync {
    /// Store (or overwrite in place) the secret for `service`/`account`.
    fn set(&self, service: &str, account: &str, secret: &str) -> KeyStoreResult<()>;
    /// Read the secret for `service`/`account`. `Ok(None)` means proven-absent;
    /// `Err` means the read could not be completed.
    fn get(&self, service: &str, account: &str) -> KeyStoreResult<Option<String>>;
    /// Delete the item, distinguishing `Deleted` / `AlreadyAbsent` from
    /// `Unknown` (an unverifiable removal).
    fn delete(&self, service: &str, account: &str) -> KeyStoreResult<DeleteOutcome>;
}

/// The production key store for the current platform.
#[cfg(target_os = "macos")]
pub fn default_key_store() -> Box<dyn KeyStore> {
    Box::new(SecurityFrameworkKeyStore)
}

/// The production key store for the current platform (non-macOS fallback).
#[cfg(not(target_os = "macos"))]
pub fn default_key_store() -> Box<dyn KeyStore> {
    Box::new(UnavailableKeyStore)
}

// ---------------------------------------------------------------------------
// Real macOS implementation (security-framework `passwords` API)
// ---------------------------------------------------------------------------

/// Apple `OSStatus` for "no matching Keychain item" (`errSecItemNotFound`).
/// Stable OS constant; compared against `security_framework::base::Error::code`.
#[cfg(target_os = "macos")]
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

/// Real macOS Keychain implementation backed by the `security-framework`
/// crate's generic-password API. Operates on the login Keychain.
#[cfg(target_os = "macos")]
#[derive(Debug, Default, Clone, Copy)]
pub struct SecurityFrameworkKeyStore;

#[cfg(target_os = "macos")]
impl KeyStore for SecurityFrameworkKeyStore {
    fn set(&self, service: &str, account: &str, secret: &str) -> KeyStoreResult<()> {
        security_framework::passwords::set_generic_password(service, account, secret.as_bytes())
            .map_err(|e| KeyStoreError::Backend(e.to_string()))
    }

    fn get(&self, service: &str, account: &str) -> KeyStoreResult<Option<String>> {
        match security_framework::passwords::get_generic_password(service, account) {
            Ok(bytes) => String::from_utf8(bytes).map(Some).map_err(|e| {
                KeyStoreError::Backend(format!("stored secret is not valid UTF-8: {e}"))
            }),
            Err(err) if err.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
            Err(err) => Err(KeyStoreError::Backend(err.to_string())),
        }
    }

    fn delete(&self, service: &str, account: &str) -> KeyStoreResult<DeleteOutcome> {
        match security_framework::passwords::delete_generic_password(service, account) {
            Ok(()) => Ok(DeleteOutcome::Deleted),
            Err(err) if err.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(DeleteOutcome::AlreadyAbsent),
            // Any other backend error means we cannot prove the item is gone.
            Err(_) => Ok(DeleteOutcome::Unknown),
        }
    }
}

// ---------------------------------------------------------------------------
// Non-macOS compile fallback (keeps the crate `cargo check`-able on Linux CI)
// ---------------------------------------------------------------------------

/// Placeholder key store for non-macOS targets: the app ships macOS-only, but
/// the crate must still compile on Linux CI. Every operation reports the store
/// as unavailable rather than pretending to succeed.
#[cfg(not(target_os = "macos"))]
#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableKeyStore;

#[cfg(not(target_os = "macos"))]
impl KeyStore for UnavailableKeyStore {
    fn set(&self, _service: &str, _account: &str, _secret: &str) -> KeyStoreResult<()> {
        Err(KeyStoreError::Unavailable(
            "the macOS Keychain is not available on this platform".into(),
        ))
    }

    fn get(&self, _service: &str, _account: &str) -> KeyStoreResult<Option<String>> {
        Err(KeyStoreError::Unavailable(
            "the macOS Keychain is not available on this platform".into(),
        ))
    }

    fn delete(&self, _service: &str, _account: &str) -> KeyStoreResult<DeleteOutcome> {
        Err(KeyStoreError::Unavailable(
            "the macOS Keychain is not available on this platform".into(),
        ))
    }
}

// ---------------------------------------------------------------------------
// In-memory fake (test-only) — provisioning/offboarding tests inject this so
// they never touch the real Keychain.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[derive(Debug, Default)]
pub struct FakeKeyStore {
    items: std::sync::Mutex<std::collections::HashMap<(String, String), String>>,
    /// When set, `delete` reports [`DeleteOutcome::Unknown`] without removing
    /// the item — simulates a real Keychain hiccup where removal can't be
    /// verified, so offboarding must report removal-unproven.
    delete_unverifiable: std::sync::atomic::AtomicBool,
    /// When set, `get` returns a backend error — simulates a locked Keychain or
    /// a denied access prompt so provisioning can prove it treats an
    /// unreadable Keychain as "unavailable" (leave the key intact) rather than
    /// "absent" (revoke + remint). See issue #292 review Finding 3.
    get_unavailable: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
impl FakeKeyStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Force the next `delete` calls to report `Unknown` (unverifiable).
    pub fn set_delete_unverifiable(&self, on: bool) {
        self.delete_unverifiable
            .store(on, std::sync::atomic::Ordering::SeqCst);
    }

    /// Force the next `get` calls to fail with a backend error, simulating a
    /// Keychain that is locked or whose access was denied (the item may still
    /// be present — the read simply could not be completed).
    pub fn set_get_unavailable(&self, on: bool) {
        self.get_unavailable
            .store(on, std::sync::atomic::Ordering::SeqCst);
    }

    /// Whether an item is currently stored (test assertions).
    pub fn contains(&self, service: &str, account: &str) -> bool {
        self.items
            .lock()
            .unwrap()
            .contains_key(&(service.to_string(), account.to_string()))
    }
}

#[cfg(test)]
impl KeyStore for FakeKeyStore {
    fn set(&self, service: &str, account: &str, secret: &str) -> KeyStoreResult<()> {
        self.items.lock().unwrap().insert(
            (service.to_string(), account.to_string()),
            secret.to_string(),
        );
        Ok(())
    }

    fn get(&self, service: &str, account: &str) -> KeyStoreResult<Option<String>> {
        if self
            .get_unavailable
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(KeyStoreError::Backend(
                "simulated unreadable Keychain (locked or access denied)".to_string(),
            ));
        }
        Ok(self
            .items
            .lock()
            .unwrap()
            .get(&(service.to_string(), account.to_string()))
            .cloned())
    }

    fn delete(&self, service: &str, account: &str) -> KeyStoreResult<DeleteOutcome> {
        if self
            .delete_unverifiable
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            // Leave the item in place: an unverifiable delete has not proven removal.
            return Ok(DeleteOutcome::Unknown);
        }
        let existed = self
            .items
            .lock()
            .unwrap()
            .remove(&(service.to_string(), account.to_string()))
            .is_some();
        Ok(if existed {
            DeleteOutcome::Deleted
        } else {
            DeleteOutcome::AlreadyAbsent
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_key_store_round_trips_set_get_delete() {
        let ks = FakeKeyStore::new();
        assert_eq!(ks.get("svc", "acct").unwrap(), None);

        ks.set("svc", "acct", "tok-1").unwrap();
        assert_eq!(ks.get("svc", "acct").unwrap().as_deref(), Some("tok-1"));
        assert!(ks.contains("svc", "acct"));

        // Overwrite in place.
        ks.set("svc", "acct", "tok-2").unwrap();
        assert_eq!(ks.get("svc", "acct").unwrap().as_deref(), Some("tok-2"));

        assert_eq!(ks.delete("svc", "acct").unwrap(), DeleteOutcome::Deleted);
        assert_eq!(ks.get("svc", "acct").unwrap(), None);
        // A second delete is AlreadyAbsent — still proven-absent.
        assert_eq!(
            ks.delete("svc", "acct").unwrap(),
            DeleteOutcome::AlreadyAbsent
        );
    }

    #[test]
    fn delete_outcome_proven_absent_semantics() {
        assert!(DeleteOutcome::Deleted.proven_absent());
        assert!(DeleteOutcome::AlreadyAbsent.proven_absent());
        assert!(!DeleteOutcome::Unknown.proven_absent());
    }

    #[test]
    fn fake_delete_unverifiable_reports_unknown_and_keeps_item() {
        let ks = FakeKeyStore::new();
        ks.set("svc", "acct", "tok").unwrap();
        ks.set_delete_unverifiable(true);
        assert_eq!(ks.delete("svc", "acct").unwrap(), DeleteOutcome::Unknown);
        // Item is left in place because removal was not proven.
        assert!(ks.contains("svc", "acct"));
    }
}
