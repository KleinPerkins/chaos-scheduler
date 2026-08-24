# ADR 0011: Secret-bearing at-rest fields are envelope-encrypted (KEK in Keychain, wrapped DEK in the DB), on by default

Status: accepted — 2026-08-24

Supersedes: the "Option B / PR-E deferred" posture of
[docs/plans/credential-security-hardening-v1.md](../plans/credential-security-hardening-v1.md)

## Decision

The secret-bearing at-rest fields are now **AEAD envelope-encrypted by default**, sealed at the
`db.rs` read/write boundary so every adapter above the service still sees transparent plaintext.

- **Two-key envelope.** A 256-bit **KEK** (master key) lives in the **macOS Keychain** as a
  generic-password item (service `chaos-scheduler-master-kek`, account `db-envelope-kek-v1`) behind
  the reusable `KeyStore` trait from [ADR 0010](0010-keychain-managed-mcp-key.md)
  (`src-tauri/src/keychain.rs`) — real `SecurityFrameworkKeyStore` on macOS, `UnavailableKeyStore`
  off it, and an in-memory `FakeKeyStore` so **every** test injects a fake and never touches the
  real Keychain. A 256-bit **DEK** (data key) encrypts the field values; the DEK is AEAD-wrapped by
  the KEK (wrap-AAD `chaos-scheduler:envelope:dek-wrap:v1`) and the wrapped DEK is stored in a new
  `envelope_keys(id, version, algo, wrapped_dek, wrap_nonce, created_at)` table. At `Database::open`
  the KEK is fetched **once**, the DEK is unwrapped **once**, and the resulting cipher is held in
  memory for the process lifetime — the KEK is never re-fetched per operation.
- **AEAD + AAD + token format.** Field values are sealed with **XChaCha20-Poly1305** (`algo` =
  `XChaCha20Poly1305`) using random 24-byte nonces, and bound to their storage location by **AAD** —
  a stable `table:column` (or `table:column:field` for a JSON leaf) context string — so a ciphertext
  cannot be relocated or swapped between fields. The stored token is
  `enc:v1:` + base64(nonce ‖ ciphertext ‖ tag), written in place of the plaintext (inside the JSON
  for the spec/trigger/queue secret leaves, and in the column for `smtp_password` /
  `inbound_webhook_secret`). The `enc:v1:` prefix makes the migration and every write **idempotent**:
  an already-sealed value is passed through untouched, and a plaintext value is recognisably not yet
  sealed.
- **Exact scope (unchanged from the redaction/purge scope).** email `smtp_password` (global
  `email_config` **and** per-profile `email_profiles`), `scheduler_config.inbound_webhook_secret`
  (via `is_secret_scheduler_config_key`), and the workflow `spec_json` / `trigger_config` /
  `queue_config` secret leaves enumerated by `SECRET_SPEC_FIELD_NAMES`. API-key hashes/salts (already
  one-way hashed) and non-secret fields are **not** encrypted.
- **Seam at the single db.rs chokepoint.** Encrypt in-scope fields on write, decrypt on read at the
  `db.rs` read/write functions, so all adapters (IPC/REST/SDK/MCP) transparently receive plaintext
  from the service ([ADR 0001](0001-scheduler-service-boundary.md)) and the existing read-scope /
  unconditional-MCP **redaction still runs ABOVE the decrypt** (it replaces the decrypted plaintext
  with `__redacted__`, unchanged). The JSON-leaf enumeration reuses `SECRET_SPEC_FIELD_NAMES` /
  `is_secret_scheduler_config_key` — no second secret-field list.
- **v19 migration (append-only, transactional, idempotent).** [ADR 0003](0003-append-only-sqlite-migrations.md)
  is honoured: the shipped migrations are untouched. The registered v19 migration creates the
  `envelope_keys` table; envelope-key **provisioning** (which needs Keychain access a
  `fn(&Connection)` migration lacks) happens at `Database::open` with a threaded `KeyStore`, and the
  in-place plaintext→ciphertext sweep runs in a transaction right after the migration chain, skipping
  any value already `enc:v1:`. Pre-v19 `.bak` sidecars are securely wiped only **after** a sweep that
  actually sealed plaintext, so a copied pre-migration backup does not leave cleartext behind.
- **Rotation.** (a) **KEK rotation** mints a new KEK, re-wraps the DEK, and updates the Keychain item
  and the stored wrapped DEK — field ciphertext is untouched. (b) **DEK rotation** mints a new DEK,
  decrypts-all + re-encrypts every in-scope field, and bumps `envelope_keys.version`; a value that
  fails to decrypt aborts the rotation rather than silently corrupting data. Both are exposed as
  service methods and IPC commands (`rotate_master_key`, `rotate_data_key`).
- **Graceful missing/unreadable master key — never a crash or silent brick.** If the KEK is
  absent/unreadable at open, the DB runs in a **secrets-locked** state: in-scope encrypted fields
  decrypt to the **distinct** `__secret_unavailable__` sentinel (never the read-scope `__redacted__`,
  so the UI can tell "hidden on read" apart from "master key unavailable"), secret **writes** are
  rejected with a clear error, and all non-secret operation proceeds. A `reprovision_secrets` IPC
  path mints a fresh KEK/DEK so the operator can re-enter secrets **without** blindly overwriting or
  deleting the existing ciphertext. `secrets_locked` exposes the state.

## Why

- **Decision reversal — envelope encryption is now MANDATED and on by default.** The
  credential-security-hardening plan previously **decided (2026-08-23)** on Option A (the
  FileVault-stack file hardening of PR-C) and **deferred** Option B / PR-E ("only if Affirm IT/
  security mandates application-level encryption beyond FileVault"). That deferral is **reversed**:
  PR-E ships now, on by default. The added protection over FileVault alone is meaningful for a
  managed-laptop threat model — a copied or backed-up DB file is now ciphertext, and malware running
  as the same user on an unlocked Mac must also pull the ACL-bound Keychain KEK to read secrets —
  and it composes with, rather than replaces, the PR-C at-rest hardening.
- **Reuse the vetted Keychain foundation.** ADR 0010 already established the `KeyStore` trait, the
  macOS `security-framework` implementation, the non-macOS fallback, and the `FakeKeyStore` that
  keeps headless CI off the real Keychain. Envelope encryption is the natural second consumer: a
  dedicated KEK service/account under the same trait, unwrapped once at startup.
- **Alternatives considered.** (a) Whole-DB SQLCipher — rejected: a heavier dependency and key-at-
  rest problem that still reduces to protecting a master key, with no field-level AAD binding. (b)
  Encrypt every field directly under the Keychain KEK — rejected: re-wrapping/rotation would require
  touching the Keychain per field and per rotation; a wrapped DEK lets the KEK stay unwrapped-once
  and makes DEK rotation a local re-encrypt. (c) A build-time or on-disk app key — rejected: reduces
  to protecting yet another at-rest key and reinvents the Keychain. XChaCha20-Poly1305 was chosen
  over AES-GCM specifically for its 24-byte random nonces (no per-key nonce-reuse bookkeeping).
- **Evidence.** Each behaviour carries a fails-first test, all injecting `FakeKeyStore`: the v19
  fixture sweep (`migration_v18_to_v19_encrypts_existing_plaintext_in_place_idempotently`), the
  transparent round-trip across every in-scope field (`envelope_round_trips_every_in_scope_field_via_db_api`),
  redaction-still-redacts on sealed data (`read_scope_redaction_still_redacts_secret_encrypted_at_rest`),
  offboard blanks the ciphertext (`offboard_blanks_encrypted_ciphertext_at_rest`), KEK rotation
  (`kek_rotation_rewraps_dek_and_leaves_data_intact`), DEK rotation
  (`dek_rotation_reencrypts_fields_and_bumps_version`), and the secrets-locked graceful state +
  recovery (`missing_master_key_locks_secrets_and_reprovision_recovers`).

## Consequences

- **Enables.** Secret-bearing fields that are AEAD ciphertext at rest under an OS-Keychain-held
  master key; a copied/backed-up DB that is opaque without the Keychain KEK; field-location-bound
  ciphertext (AAD) that cannot be swapped; KEK and DEK rotation; and a defined, non-destructive
  recovery path when the master key is unavailable.
- **Lost-key recovery posture (documented explicitly).** The KEK is the root of recoverability. **A
  lost KEK ⇒ the existing encrypted secrets are unrecoverable** — this is inherent to encrypting at
  rest and is accepted, not a bug. It is mitigated operationally: the secrets-locked state is
  explicit (`__secret_unavailable__`, not a crash and not confused with redaction), and
  `reprovision_secrets` lets the operator mint a fresh KEK/DEK and re-enter secrets without destroying
  the old ciphertext blindly. There is intentionally **no KEK escrow/backup** in-app; a lost master
  key means re-entering secrets, not recovering them.
- **Forecloses.** No adapter may bypass the db.rs seam to read or write an in-scope secret; no test
  may touch the real Keychain (always inject a `KeyStore`); a shipped migration is never mutated
  (v19 is additive, and provisioning lives at `open`); and pre-migration `.bak` cleartext is
  securely wiped only after a verified encrypting sweep.
- **Invariant to keep true.** KEK-in-Keychain + wrapped-DEK-in-DB + unwrap-once-at-open +
  encrypt-on-write/decrypt-on-read at the single db.rs chokepoint + `enc:v1:`-idempotent tokens with
  `table:column` AAD + graceful secrets-locked degradation + non-destructive re-provision — each with
  its failing-first test. This ADR **adds to, and must not regress**, the shipped v1.7.0 hardening:
  read-scope + unconditional-MCP redaction still replaces decrypted plaintext with `__redacted__`;
  the offboarding purge still blanks the (now ciphertext) secret-bearing fields, including
  whole-column blanking of unparseable JSON; the audit log records only non-secret metadata; the
  at-rest `0600` / `secure_delete` / Time-Machine-exclusion file hardening still holds; and the D05
  fix-agent guarantees of [ADR 0009](0009-d05-fix-agent-propose-only.md) are untouched.
