# ADR 0006: Passwordless minisign updater signing, separate from and additional to Apple Developer-ID signing

Status: accepted — 2026-08-22

## Decision

The Tauri updater artifacts are signed with a passwordless minisign key so releases and auto-updates
run unattended (no key-password prompt in CI). This minisign signing is separate from, and in addition
to, Apple Developer-ID signing + notarization of the DMG — both are performed for a shipped release;
neither substitutes for the other. The minisign public key lives in `src-tauri/tauri.conf.json`; the
private key is held in the required-reviewer `release` GitHub Environment.

## Why

- **Alternatives considered.** (a) A password-protected minisign key — rejected: it forces an
  interactive prompt, which breaks unattended CI releases and the reusable release workflow. (b) Rely
  on Apple signing alone and skip the updater signature — rejected: the Tauri updater requires its own
  minisign signature to verify update artifacts; Apple notarization does not cover the updater channel.
  (c) A passwordless minisign key stored in the gated `release` environment, used in addition to Apple
  signing — chosen.
- **Evidence.** The release pipeline signs+notarizes the DMG (Apple) and separately produces
  minisign-signed updater artifacts served via `latest.json`; the public key is committed in
  `tauri.conf.json`.

## Consequences

- **Enables.** Fully unattended signed releases and auto-update; both trust chains (Apple + minisign)
  intact for every shipped release.
- **Forecloses.** Neither signature is skipped for a shipped release. The key being passwordless means
  its confidentiality rests entirely on the gated `release` environment.
- **Invariant to keep true (critical).** Losing the minisign private key permanently breaks
  auto-update for all installed clients — there is no recovery path. Protect and back up that key.
