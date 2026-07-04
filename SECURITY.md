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

## Scope & sensitive material

Chaos Scheduler runs locally and (in later phases) exposes a localhost REST API,
webhooks, and a Cursor integration. Take particular care with:

- **API keys** — the scheduler's own API keys and the Cursor service-account key
  are stored in local settings, never in workflow specs or committed to git.
- **Updater signing key** — the minisign private key
  (`TAURI_SIGNING_PRIVATE_KEY`) is a critical secret; see
  [docs/RELEASING.md](docs/RELEASING.md) for custody.
- **Webhook secrets** — inbound/outbound webhooks are HMAC-signed; treat secrets
  as credentials.
- **Network binding** — the API binds `127.0.0.1` by default. Only expose it on a
  LAN deliberately, with API-key auth enabled.

Never commit secrets. `.env` files are not read by the Tauri build; CI secrets
live in GitHub Actions secrets (scoped to the `release` Environment for signing
material).
