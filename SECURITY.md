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
attributes:

- no live credential may remain in a tracked blob;
- the final security follow-up must stop tracking project-local
  `.cursor/mcp.json` and keep the app-managed user config at
  `~/.cursor/mcp.json` outside Git;
- a focused credential guard must prevent tracked MCP configs from reintroducing
  scheduler API-key or bearer material; and
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

## Secrets storage & read-scope redaction

| Material                                    | At rest                                             | Over REST/MCP (read scope)            |
| ------------------------------------------- | --------------------------------------------------- | ------------------------------------- |
| API key secrets                             | Salted hash in SQLite; plaintext shown once at mint | Keys are not listable over REST       |
| Webhook / operator secrets in workflow JSON | Stored in `spec_json` / `trigger_config`            | Replaced with `__redacted__` sentinel |
| Cursor / SMTP settings                      | Local SQLite settings                               | Desktop IPC only (not REST)           |

**Read-scope redaction** (`read` scope and MCP tools): nested fields named
`secret`, `signature_secret`, `cursor_api_key`, or `smtp_password` in workflow
spec/trigger JSON are replaced with the stable sentinel `__redacted__`
(distinct from empty/unset). Applied in the service layer so REST/SDK/tool reads
inherit identical behavior.

**Resource projection** (`chaos://workflows` and
`chaos://workflows/{id}`): MCP applies an additional scope-independent boundary
before workflow state enters agent context. Known secret fields are always
redacted across spec, trigger, and queue JSON; parsing is bounded; malformed or
oversized nested JSON is replaced with `__redacted_invalid_json__`.

**Write/admin round-trip**: REST/SDK and MCP tool callers with `write` or `admin`
scope receive full secrets so edit and PATCH round-trips keep working. Workflow
resources are intentionally not write round-trip payloads.

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
