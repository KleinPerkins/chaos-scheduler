# ADR 0002: `environment` + `managed_externally` replace the overloaded `corpus` concept
Status: accepted — 2026-08-22

## Decision

The core workflow model uses two orthogonal fields — `environment` (which named environment a
workflow belongs to, e.g. `production`, `sandbox`) and `managed_externally` (whether an external
owner such as an MCP client controls the workflow) — replacing the earlier single overloaded
`corpus` field. Environment is single-select and scopes all metrics/data; the default environments
are `production` and `sandbox`.

## Why

- **Alternatives considered.** (a) Keep `corpus` and encode both meanings positionally — rejected:
  one column conflated "where does this run" with "who owns this," so queries and UI controls could
  not express either cleanly (the old All/Instance/Source control was the symptom). (b) Add a second
  boolean but keep the `corpus` name — rejected: the name no longer described the field. (c) Rename
  to `environment` and add an explicit `managed_externally` boolean — chosen, with a schema migration
  chain (drop-corpus, then the production/sandbox rename) that also normalizes legacy `source`/
  `instance` names away.
- **Evidence.** The migration chain in `src-tauri/src/db.rs` carries the `corpus` drop and the
  production/sandbox environment rename; the UI environment selector became a single-select that
  scopes all data, each environment rendered as a unique colored-dot badge.

## Consequences

- **Enables.** Clear per-environment scoping of workflows/runs/metrics; an explicit externally-managed
  flag that governance (e.g. MCP protection of `production`) can key off.
- **Forecloses.** No new code may reintroduce a single field that conflates ownership and environment.
- **Invariant to keep true.** Workflows are registered per environment; environment is the scoping
  key for all metrics and data surfaces.
