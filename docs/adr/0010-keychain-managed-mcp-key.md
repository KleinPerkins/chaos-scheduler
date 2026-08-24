# ADR 0010: The managed MCP scheduler key lives in the macOS Keychain, resolved by an app-owned launcher — never at rest in a config file

Status: accepted — 2026-08-24

## Decision

The app-managed Cursor/MCP integration no longer writes the minted scheduler API key as plaintext
into `~/.cursor/mcp.json`. Instead:

- The key is stored in the **macOS Keychain** as a generic-password item (service
  `chaos-scheduler-managed-mcp`, account `managed-mcp-api-key`; a single managed key at a time,
  re-mint overwrites in place) behind a reusable `KeyStore` trait (`src-tauri/src/keychain.rs`).
  The real implementation (`SecurityFrameworkKeyStore`) uses the `security-framework` crate's
  `passwords` API and is gated `#[cfg(target_os = "macos")]`; a non-macOS `UnavailableKeyStore`
  keeps the crate compiling on Linux CI; an in-memory `FakeKeyStore` lets every
  provisioning/offboarding test inject a fake and **never** touch the real Keychain.
- The managed `mcp.json` entry's `command` becomes an **app-owned launcher script**
  (`<app_data_dir>/mcp/launch-managed.sh`, mode `0700`) that resolves the key from the Keychain at
  spawn time (`/usr/bin/security find-generic-password`) and execs the absolute node + absolute CLI.
  The launcher contains **no secret**; the entry's `env` keeps `CHAOS_SCHEDULER_URL` and the
  ownership markers but **never** `CHAOS_SCHEDULER_API_KEY`.
- A **one-time, transactional migration** runs on provision and on the startup re-provision hook:
  if a managed entry still carries an inline `CHAOS_SCHEDULER_API_KEY`, the token is read →
  `keystore.set(...)` → verified by `keystore.get(...) == token` → the entry is rewritten to the
  launcher form via the existing backup+atomic-write path → only then is the plaintext gone. If any
  step fails the inline token is left intact (the working key is never lost) and the state surfaces
  as needs-attention/unproven.
- **Offboarding is Keychain-aware.** It deletes the Keychain item; `OffboardReport` gains a
  `keychain_item_removed` field; `managed_integration_fully_removed` requires the Keychain item
  **proven-absent** (`Deleted`/`AlreadyAbsent`) as well as the manifest file and Cursor entry — a
  delete that can't be verified (`Unknown`) reports removal-unproven, never a false "removed".

Separately, the **tracked** project file `.cursor/mcp.json` is removed from Git and git-ignored, a
secret-free `.cursor/mcp.example.json` is added, and a focused credential guard
(`scripts/check-mcp-config-secret-free.mjs`, wired into `ci-required` + lefthook) fails if any
tracked `.cursor/**/mcp*.json` (excluding `*.example.json`) reintroduces scheduler API-key or bearer
material.

## Why

- **Remove token-at-rest in files.** The pre-#292 design kept a live-shaped scheduler key in
  plaintext in `~/.cursor/mcp.json` (and a live-shaped value in the tracked `.cursor/mcp.json`
  blob). The `SECURITY.md` "MCP config and Git history" charter requires that no live credential
  remain in a tracked blob and that the managed key move off plaintext files; the Keychain is the
  OS-provided secret store for exactly this.
- **Alternatives considered.** (a) Keep the token in `~/.cursor/mcp.json` but rely on file
  permissions/`0600` — rejected: still token-at-rest in a plaintext file that other tooling, backups,
  and support bundles routinely read, and it does nothing for the tracked-blob exposure. (b) Encrypt
  the token with an app-held key on disk — rejected: reduces to protecting yet another at-rest key
  and reinvents the Keychain. (c) A Keychain-backed **launcher** that resolves the key at spawn and a
  trait abstraction for testability — chosen: the config file and Git history carry no secret, the
  OS guards the key, and the trait keeps headless CI off the real Keychain (which hangs/fails there).
  mcp.rs's own module doc already named this the intended (previously deferred) design.
- **Evidence.** Each new behavior carries a fails-first test: the offboarding refcount guard
  (`overlapping_offboards_keep_minting_disabled_until_all_complete`), the unparseable-`mcp.json`
  backup-and-replace (`offboard_backs_up_and_replaces_unparseable_config_scrubbing_the_token`), the
  Keychain-aware tri-state (`offboard_reports_not_removed_when_keychain_delete_is_unverifiable`), and
  the credential guard (`scripts/check-mcp-config-secret-free.test.mjs`). Provision/remove/offboard
  tests inject `FakeKeyStore` and assert the key is stored in the Keychain and absent from the config
  `env`.

## Consequences

- **Enables.** A managed MCP integration whose secret is held by the OS Keychain and whose tracked
  and user config files are secret-free; offboarding that provably removes the Keychain item; and a
  test suite that exercises the whole flow without ever touching the real login Keychain.
- **Forecloses.** No provisioning path may write `CHAOS_SCHEDULER_API_KEY` into the Cursor config
  `env`, and no test may hit the real Keychain (always inject a `KeyStore`). A Keychain delete that
  can't be verified must never be reported as a completed removal. If the Keychain item is lost
  (e.g. deleted out of band), the integration re-provisions and re-mints rather than resurrecting a
  plaintext token.
- **Invariant to keep true.** Key-in-Keychain + launcher-with-no-secret + `env` without the API key
  - transactional migration that never loses the working key + offboarding that requires the
    Keychain item proven-absent + tests that inject a fake store — each retaining its failing-first
    test. This ADR adds to, and must not regress, the shipped v1.7.0 hardening (MCP read redaction,
    gitleaks gate, at-rest `0600`/secure-delete, audit-log view, the offboarding purge + tri-state) or
    the D05 fix-agent guarantees of [ADR 0009](0009-d05-fix-agent-propose-only.md).
