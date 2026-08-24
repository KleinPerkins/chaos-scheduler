# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records for `chaos-scheduler`.

**Governance:** `~/dev/_ops/doc-warden/POLICY.md` §2 (taxonomy), §9 (archival).
ADRs are **byte-identical-forever** once their Status line reads `accepted` — no in-place edits
after acceptance, not even a status-bullet edit. Supersession is recorded only via a new ADR's
`Supersedes:` pointer and this index file's row update.

## Index

| ADR                                                       | Title                                                                                                                     | Status   | Date       |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- | -------- | ---------- |
| [0001](0001-scheduler-service-boundary.md)                | SchedulerService is the single business-logic boundary; IPC/REST/SDK/MCP are thin adapters                                | accepted | 2026-08-22 |
| [0002](0002-environment-managed-externally-core-model.md) | `environment` + `managed_externally` replace the overloaded `corpus` concept                                              | accepted | 2026-08-22 |
| [0003](0003-append-only-sqlite-migrations.md)             | SQLite migrations are append-only, transactional, and never mutated after shipping                                        | accepted | 2026-08-22 |
| [0004](0004-app-owned-mcp-sdk-provisioning.md)            | MCP/SDK are app-owned provisioned artifacts version-pinned to the build (not DMG-bundled, not floating `npx`)             | accepted | 2026-08-22 |
| [0005](0005-sdk-external-mcp-bundled.md)                  | Bundle `mcp-server` with tsup but keep `@chaos-scheduler/sdk` external                                                    | accepted | 2026-08-22 |
| [0006](0006-passwordless-minisign-updater-signing.md)     | Passwordless minisign updater signing, separate from and additional to Apple Developer-ID signing                         | accepted | 2026-08-22 |
| [0007](0007-manual-runs-through-admission-control.md)     | All manual runs go through admission control via the `dispatch_manual_run` choke point                                    | accepted | 2026-08-22 |
| [0008](0008-bespoke-svg-chart-primitives.md)              | Charts use bespoke in-repo SVG primitives (d3-scale/d3-shape), not a charting library                                     | accepted | 2026-08-22 |
| [0009](0009-d05-fix-agent-propose-only.md)                | The D05 fix-agent is propose-only — never auto-merged or auto-applied (born-draft PR, trusted-local-tool threat model)    | accepted | 2026-08-23 |
| [0010](0010-keychain-managed-mcp-key.md)                  | Managed MCP scheduler key lives in the macOS Keychain, resolved by an app-owned launcher — never at rest in a config file | accepted | 2026-08-24 |
| [0011](0011-envelope-encryption-secrets-at-rest.md)       | Secret-bearing at-rest fields are envelope-encrypted (KEK in Keychain, wrapped DEK in the DB), on by default              | accepted | 2026-08-24 |

## Conventions

Format (Nygard):

```markdown
# ADR <NNNN>: <title>

Status: <draft|accepted|superseded> — <date>

## Decision

## Why

## Consequences
```

- Numbering: 4-digit zero-padded sequential (`0001`, `0002`, …)
- Filename: `<NNNN>-<slug>.md`
- Status values: `draft` → `accepted` → `superseded`
- `Supersedes: ADR-NNNN` line added to the new ADR; old ADR is never modified
- The index row for a superseded ADR is updated to `superseded — <date>` (this index is mutable;
  the ADR file itself is not)
