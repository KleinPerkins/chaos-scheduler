# Security Policy

## Supported versions

Chaos Scheduler is pre-1.0. Only the latest released version receives security
fixes. Auto-update is forward-only — stay on the newest release.

## Reporting a vulnerability

**Do not open a public issue for security vulnerabilities.**

Please report privately via GitHub's
[private vulnerability reporting](https://github.com/KleinPerkins/chaos-scheduler/security/advisories/new)
(Security → Report a vulnerability). Include:

- a description and impact assessment,
- reproduction steps or a proof of concept,
- affected version(s) and platform.

We aim to acknowledge reports within **5 business days** and to provide a
remediation timeline after triage. Please allow reasonable time for a fix before
any public disclosure (coordinated disclosure).

## Scope & threat model

Chaos Scheduler is a **personal, loopback-first** desktop app. It runs locally,
stores state in SQLite, exposes a localhost REST API (`/api/v1`), optional
metrics, inbound/outbound webhooks, and a Cursor MCP integration. The design
assumes a **single trusted operator on the same machine** — not multi-tenant
hosting on a shared network.

Take particular care with:

- **API keys** — scoped keys for REST/MCP are minted in the desktop app;
  secrets are hashed at rest (salted SHA-256); the plaintext token is shown
  once at creation.
- **Cursor service-account key** (`CURSOR_API_KEY`) — stored in local settings,
  never in workflow specs or git.
- **Webhook secrets** — inbound dispatch and outbound completion webhooks use
  HMAC-SHA256; treat secrets as credentials.
- **Updater signing key** — the minisign private key
  (`TAURI_SIGNING_PRIVATE_KEY`) is a critical secret; see
  [docs/RELEASING.md](docs/RELEASING.md) for custody.
- **SMTP credentials** — optional alert email password in local settings.

Never commit secrets. `.env` files are not read by the Tauri build; CI secrets
live in GitHub Actions secrets (scoped to the `release` Environment for signing
material).

### MCP config and Git history

The `.cursor/mcp.json` attributes are presentation aids, not security
boundaries. `linguist-generated` is a default-collapsed presentation hint in
GitHub's changed-files UI, and `binary` suppresses ordinary local CLI text
diffs. Neither is redaction: reviewers can expand generated files, patch and
compare API endpoints may return their text, and Git history retains every
committed blob.

The authoritative MCP credential controls are therefore independent of those
attributes, and as of #292 they are enforced in the repository:

- no live credential may remain in a tracked blob;
- this security follow-up (#292) landed the requirement to stop tracking project-local
  `.cursor/mcp.json`: it is removed from the Git index and git-ignored, while the
  app-managed user config at `~/.cursor/mcp.json` stays outside Git. The managed
  scheduler key now lives in the macOS Keychain (never in either file) and is
  resolved at spawn time by an app-owned launcher script — see
  [ADR 0010](docs/adr/0010-keychain-managed-mcp-key.md);
- a focused credential guard (`scripts/check-mcp-config-secret-free.mjs`, wired into
  the `ci-required` aggregation and the lefthook pre-commit hook) now prevents tracked
  MCP configs from reintroducing scheduler API-key or bearer material. The committed
  example configs (`.cursor/mcp.example.json`, `.cursor/mcp.remote.example.json`) carry
  placeholders only and are excluded from the guard; and
- credential revocation plus GitHub Support cleanup/history handling remain
  separate incident-response work. Attributes do not remove existing history or
  prevent all disclosure.

## Network binding

### REST API (default `127.0.0.1:9618`)

The embedded API binds `CHAOS_SCHEDULER_API_ADDR` (default loopback). Binding to
a **non-loopback** address requires an explicit operator opt-in:

```bash
export CHAOS_SCHEDULER_ALLOW_REMOTE_API=1
```

Without that flag, startup refuses the bind with a clear log message. Loopback
addresses (`127.0.0.1`, `[::1]`) are always permitted.

### Metrics endpoint (default `127.0.0.1:9617`)

The Prometheus-style metrics listener uses the **same** remote-bind gate as the
REST API. Non-loopback `METRICS_ADDR` requires `CHAOS_SCHEDULER_ALLOW_REMOTE_API=1`.

### MCP Streamable HTTP (default `127.0.0.1:9700`)

MCP HTTP mode has a separate flag: `CHAOS_SCHEDULER_MCP_ALLOW_REMOTE_HTTP=1`
(or `--allow-remote-http`). See the
[mcp-server README](packages/mcp-server/README.md).

When exposing any surface beyond loopback, use scoped API keys, TLS where
practical, and firewall rules appropriate to your LAN.

## Inbound webhook signing (canonical)

Workflow dispatch (`POST /api/v1/workflows/{id}/dispatch`) verifies HMAC over a
**canonical** payload when a webhook secret is configured:

```
METHOD\nPATH\nTIMESTAMP\nSHA256_HEX(raw_body)
→ hex(HMAC_SHA256(secret, canonical))
```

Required headers:

- `X-Chaos-Timestamp` — Unix seconds; must fall within a **5-minute** replay
  window.
- `X-Chaos-Event-Id` — unique per event; duplicates within the TTL are rejected
  (`409 Conflict`).
- `X-Chaos-Signature` — `sha256=<hex>`.

The SDK's `dispatchWorkflow` / `inboundDispatchHeaders` implement this scheme.
**Raw-body HMAC alone is rejected** (legacy callers receive `401`).

Cross-language test vectors:
`packages/test-fixtures/webhook-vectors.v1.json` (verified in Rust and TypeScript).

Outbound completion webhooks use a **different** scheme: HMAC-SHA256 over the
**raw POST body** with `X-Chaos-Event: run.succeeded | run.failed`. See
[packages/INTEGRATION.md](packages/INTEGRATION.md) §4–5.

## Secrets storage & read redaction

| Material                                    | At rest                                                                                                              | Over REST (read scope)                |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| API key secrets                             | Salted hash in SQLite; plaintext shown once at mint                                                                  | Keys are not listable over REST       |
| Managed MCP scheduler key (#292)            | macOS Keychain (service `chaos-scheduler-managed-mcp`, account `managed-mcp-api-key`); never in `~/.cursor/mcp.json` | Not exposed over REST                 |
| Webhook / operator secrets in workflow JSON | **AEAD envelope-encrypted** (`enc:v1:` tokens) inside `spec_json` / `trigger_config` / `queue_config`                | Replaced with `__redacted__` sentinel |
| `inbound_webhook_secret` (scheduler config) | **AEAD envelope-encrypted** (`enc:v1:` token) in the `scheduler_config` value column                                 | Not exposed over REST                 |
| SMTP password (global + per-profile)        | **AEAD envelope-encrypted** (`enc:v1:` token) in the `email_config` / `email_profiles` column                        | Desktop IPC only (not REST); masked   |
| Cursor / other local settings               | Local SQLite settings                                                                                                | Desktop IPC only (not REST)           |

**At rest, secret-bearing fields are AEAD envelope-encrypted by default** (see the
envelope-encryption section below and [ADR 0011](docs/adr/0011-envelope-encryption-secrets-at-rest.md)).
Decryption happens at the `db.rs` read boundary, so the service and every adapter see plaintext and
the read-scope / MCP redaction below still applies unchanged above the decrypt.

**Service-layer read-scope redaction** (REST/SDK): at `read` scope, nested
fields named `secret`, `signature_secret`, `cursor_api_key`, or `smtp_password`
in workflow spec/trigger JSON are replaced with the stable sentinel
`__redacted__` (distinct from empty/unset). A `write` or `admin` REST/SDK caller
instead receives full secrets so a direct edit/PATCH round-trip keeps working.

**MCP redaction is unconditional — every workflow-returning tool and
resource.** Every MCP surface that can place workflow state into agent/LLM
context applies the projection redaction **regardless of API-key scope**, on
BOTH the read and write paths:

- **read tools** — `list_workflows`, `get_workflow`;
- **write tools that echo the stored workflow** — `register_workflow`,
  `set_workflow_spec`, `update_workflow`. A create/replace/no-op-update returns
  the full stored row, so without redaction a write-scoped key could read a raw
  secret back out through a write side-door;
- **`patch_workflow_spec`** — returns only the redacted stored definition; and
- **resources** — `chaos://workflows`, `chaos://workflows/{id}`,
  `chaos://workflows/{id}/definition`, `chaos://workflows/index`.

The managed Cursor MCP integration key is write-scoped, so this scope-independent
boundary (not the service-layer scope check) is what keeps workflow secrets out
of agent context. Known secret fields are always redacted across spec, trigger,
and queue JSON; parsing is bounded; malformed or oversized nested JSON is
replaced with `__redacted_invalid_json__`.

**Secret-preserving edits never hand a secret to the caller.** MCP tool and
resource reads are not write round-trip payloads. To edit a spec that still
contains secrets, `patch_workflow_spec` re-fetches the stored value
**server-side** and preserves it through the `__redacted__` sentinel, so the
real secret is never returned to the MCP caller. A `write`/`admin` REST/SDK
caller that needs the raw values for a round-trip must read them directly over
REST, never through an MCP tool or resource.

## Envelope encryption of secrets at rest (on by default)

As of [ADR 0011](docs/adr/0011-envelope-encryption-secrets-at-rest.md), the
secret-bearing at-rest fields are **AEAD envelope-encrypted by default** — a
layer above the FileVault-stack file hardening (`0600` / `secure_delete` /
Time-Machine exclusion), not a replacement for it.

**Scheme.** A 256-bit **KEK** (master key) lives in the macOS Keychain (service
`chaos-scheduler-master-kek`, account `db-envelope-kek-v1`), minted once if
absent, behind the same `KeyStore` trait as the managed MCP key. A 256-bit
**DEK** (data key) encrypts the field values; the DEK is AEAD-wrapped by the KEK
and the wrapped DEK is stored in the `envelope_keys` table. At startup the KEK
is fetched once and the DEK unwrapped once, then held in memory — the KEK is
never re-fetched per operation.

**Cipher.** Field values are sealed with **XChaCha20-Poly1305** (random 24-byte
nonces) and bound to their storage location via **AAD** (a `table:column`-style
context) so a ciphertext cannot be relocated or swapped between fields. The
stored token is `enc:v1:` + base64(nonce ‖ ciphertext ‖ tag); the `enc:v1:`
prefix keeps writes and the migration idempotent.

**Scope.** SMTP password (global + per-profile), `inbound_webhook_secret`, and
the workflow `spec_json` / `trigger_config` / `queue_config` secret fields.
API-key hashes/salts are already one-way hashed and are **not** encrypted.

**Encrypt-on-write / decrypt-on-read** happens at the `db.rs` boundary, so all
adapters transparently receive plaintext from the service and the read-scope /
MCP redaction above still replaces secrets with `__redacted__`. The offboarding
purge blanks the (now ciphertext) fields exactly as before.

**Rotation.** The KEK can be rotated (mint a new KEK, re-wrap the DEK; field
data untouched) and the DEK can be rotated (mint a new DEK, decrypt-all +
re-encrypt, bump the key version). Both are available as IPC commands
(`rotate_master_key`, `rotate_data_key`).

**Lost / unavailable master key.** If the KEK is missing or unreadable at
startup the app runs in a **secrets-locked** state rather than crashing:
encrypted fields read back as the distinct `__secret_unavailable__` sentinel
(**not** `__redacted__`), new secret **writes** are rejected with a clear error,
and all non-secret operation proceeds. A re-provision action
(`reprovision_secrets`) mints a fresh KEK/DEK so the operator can re-enter
secrets without destroying the existing ciphertext blindly.

> **A lost KEK means the existing encrypted secrets are unrecoverable.** This is
> inherent to encrypting at rest and is accepted by design: there is no in-app
> KEK escrow. The mitigation is the explicit secrets-locked state plus the
> re-enter/re-provision path — you re-enter secrets, you do not recover them.

## Child-process environment scrubbing

Workflow child processes inherit the app's environment (personal scripts may rely
on `PATH`, `SSH_AUTH_SOCK`, proxies, venv vars, etc.). Before spawn, the
scheduler strips a **deny-list** of scheduler-internal secrets:

- `CURSOR_API_KEY`, `SMTP_PASSWORD`
- `CHAOS_SCHEDULER_API_*`
- `CHAOS_SCHEDULER_*_SECRET` and `CHAOS_SCHEDULER_*_TOKEN`

User credentials (e.g. `GITHUB_TOKEN`) are **not** stripped.

## Outbound webhook SSRF protections

Outbound `webhook` completion actions apply defense-in-depth before connecting:

1. **Literal host/IP blocklist** — loopback, unspecified, link-local, ULA, and
   `localhost` hostnames are rejected at URL parse time.
2. **DNS resolve + pin** — at send time the host is resolved; if **any** address
   is blocked, the request is refused. The validated address is pinned via
   `ClientBuilder::resolve` so a second DNS lookup cannot race back to a private
   IP (rebind TOCTOU).
3. **IPv4-mapped IPv6** — addresses like `::ffff:127.0.0.1` are treated as
   blocked.
4. **No redirects** — `redirect::Policy::none()`; 3xx responses are not
   followed.

## Graceful shutdown

All quit paths (Cmd+Q, dock, tray, `quit_app`, restart) route through a single
`RunEvent::ExitRequested` handler:

1. **Re-entrancy guard** — `claim_exit_shutdown()` ensures the handler runs once.
2. **`prevent_exit()`** — Tauri defers process exit while workers wind down.
3. **`SHUTDOWN` flag** — poll loops and retry backoffs observe the flag via
   interruptible sleep and stop promptly.
4. **Fixed grace, off main thread** — after ~5 s (3 s child SIGTERM→SIGKILL
   window + 2 s margin), `app.exit(0)` runs on a background thread so the event
   loop is never blocked.

Runs not finished within the grace window are recovered as stale/orphaned on the
next boot via `recover_orphaned_runs` (PID + start-time verification).

## MCP guardrails (summary)

The MCP server enforces **fail-closed** protected-environment writes, a shared
in-process tool-call budget, and per-request bearer auth in HTTP mode. Cursor
hooks remain **fail-open** (confirm/warn). Details:
[packages/mcp-server/README.md](packages/mcp-server/README.md#guardrails).

## Gap-closure audit

The post-hardening gap-closure ship (11 concerns, PRs #68–#80) is documented in
[docs/hardening-gap-closure-report.md](docs/hardening-gap-closure-report.md).

## Transitive dependency advisories (upstream-blocked)

Dependabot may still flag the following **transitive** Rust crates. We track
them, bump when upstream releases permit, and dismiss with documented rationale
when blocked.

### `glib` &lt; 0.20 (GTK3 / Tauri Linux stack)

- **Source:** Tauri 2's Linux webview stack pulls `gtk` 0.18 → `glib` 0.18.x.
  The gtk3-rs 0.18 line is EOL and pins `glib` ^0.18; there is no in-tree upgrade
  path without a Tauri major platform shift.
- **Exposure:** Chaos Scheduler ships **macOS-only** desktop binaries. Linux
  GTK/glib code is compile-time transitive baggage from `wry`/`tauri`, not a
  supported runtime surface for this project.
- **Mitigation:** Stay on latest Tauri patch releases; re-evaluate when Tauri
  moves the Linux stack beyond gtk3-rs 0.18.

### `rand` 0.7.x (PHF 0.8 build codegen)

- **Source:** `selectors` 0.24 (Tauri HTML/CSS parsing) → `phf_codegen` 0.8 →
  `phf_generator` 0.8 → `rand` 0.7.3. This chain is **build-time only** (PHF
  table generation during `cargo build`).
- **Exposure:** Runtime `rand` is 0.8.6+ / 0.9.x after lockfile updates. The
  0.7.x advisory (custom logger unsoundness) does not affect shipped binaries.
- **Mitigation:** `cargo update -p rand@0.8` on each security pass; dismiss
  the 0.7.x alert until `selectors`/`phf` 0.8 codegen is upgraded upstream.

### NPM `esbuild` (dev-server, Windows-only advisory)

- **Source:** `tsup` and other dev tooling pin `esbuild` ^0.27. Patched in
  `>= 0.28.1` via root and package `overrides`.
- **Exposure:** Dev/build tooling only; advisory targets the esbuild **dev
  server on Windows**, not production bundles or macOS operator workflows.
