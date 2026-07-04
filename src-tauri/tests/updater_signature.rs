//! Unit-level verification of the Tauri auto-updater signature scheme.
//!
//! The updater ships a `latest.json` whose `signature` field is a Tauri-wrapped
//! (base64-of-minisign) detached signature over the update artifact, verified in
//! the installed app against the `plugins.updater.pubkey` from `tauri.conf.json`
//! (also a Tauri-wrapped base64 minisign public key). This test proves that
//! pipeline end-to-end with a real key/signature fixture — no running app, no
//! network — using the same `minisign-verify` crate the updater relies on.
//!
//! Fixture provenance: generated once with `tauri signer generate` +
//! `tauri signer sign` (empty password) over `ARTIFACT`. Regenerate the trio
//! together if you ever rotate it.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};

/// The exact bytes that were signed (stands in for the update artifact).
const ARTIFACT: &[u8] = b"chaos-scheduler updater artifact v9.9.9\n";

/// Tauri-format public key, exactly as it appears in
/// `tauri.conf.json > plugins.updater.pubkey` (base64 of the minisign `.pub`).
const TAURI_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc4NjQzMDZFMEQ0MDI5NDgKUldSSUtVQU5iakJrZU1JN1JXKzZ1V1RiYThhbFY3OVhvMXMvY1JTdTlueTZGdG9wV1VoeWt2dUMK";

/// Tauri-format signature, exactly as it appears in `latest.json > signature`
/// (base64 of the minisign `.sig`).
const LATEST_JSON_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSSUtVQU5iakJrZUsvaXVZb1RyOTZjWWw0TC94QzcvQlVKYjF6bW1BUUhQbXkxQzNSYjZWMG9iNnp0SWwvbkZhUm90SFhNQ3lMcnA2aVNkV2FMc0RXbHJFSE9tZzFxVFE4PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzgzMTY2NzM4CWZpbGU6YXJ0aWZhY3QudHh0CmF6K3llcG9zYjBzOFlNVjRMQ21kd2FYNHNCRkNpM3hxRGVFRHNUbml5d3NuODVsSWtWT0lPazllR280MEhZWW05aDJjaDM0ZG9rWGVLMURCRjhOL0FBPT0K";

/// Decode a Tauri-wrapped base64 blob back into the underlying minisign text.
fn tauri_unwrap(b64: &str) -> String {
    String::from_utf8(STANDARD.decode(b64.trim()).expect("valid base64"))
        .expect("minisign text is UTF-8")
}

fn fixture_public_key() -> PublicKey {
    // A minisign `.pub` is: `untrusted comment: ...\n<base64 key>`.
    let text = tauri_unwrap(TAURI_PUBKEY);
    let key_line = text.lines().nth(1).expect("public key line present").trim();
    PublicKey::from_base64(key_line).expect("parse minisign public key")
}

fn fixture_signature() -> Signature {
    Signature::decode(&tauri_unwrap(LATEST_JSON_SIGNATURE)).expect("parse minisign signature")
}

#[test]
fn latest_json_signature_verifies_against_configured_pubkey() {
    let pk = fixture_public_key();
    let sig = fixture_signature();
    pk.verify(ARTIFACT, &sig, false)
        .expect("a genuine signature must verify against the updater pubkey");
}

#[test]
fn tampered_artifact_fails_verification() {
    let pk = fixture_public_key();
    let sig = fixture_signature();
    let mut tampered = ARTIFACT.to_vec();
    tampered[0] ^= 0x01; // flip one bit
    assert!(
        pk.verify(&tampered, &sig, false).is_err(),
        "a modified artifact must NOT verify — otherwise updates could be spoofed"
    );
}

#[test]
fn signature_from_a_different_key_is_rejected() {
    // A structurally-valid signature verified against an unrelated public key
    // must fail (guards against accepting any well-formed signature).
    let other_pub = "untrusted comment: minisign public key\nRWTgUyj6qX8b0m0Q0h3s1vT0m9q8f8b6qk8x2r5u1p0w6n3t7c9d2e4f";
    // If this crafted key ever parses, verification must still fail; if it does
    // not parse we simply skip (the important guarantee is the two tests above).
    if let Ok(pk) = PublicKey::from_base64(other_pub.lines().nth(1).unwrap()) {
        let sig = fixture_signature();
        assert!(pk.verify(ARTIFACT, &sig, false).is_err());
    }
}
