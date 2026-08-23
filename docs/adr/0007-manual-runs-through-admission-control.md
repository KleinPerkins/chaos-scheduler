# ADR 0007: All manual runs go through admission control via the `dispatch_manual_run` choke point
Status: accepted — 2026-08-22

## Decision

Every manual run — a fresh on-demand run, an enqueue, and a rerun — is routed through a single
admission-control choke point, `dispatch_manual_run` (`src-tauri/src/scheduler.rs`), which makes the
capacity/mutex/queued-claim/run-state decision inside one `BEGIN IMMEDIATE` transaction. The legacy
`trigger_workflow` command was removed; `rerun_workflow` routes through admission control; and the
SDK/MCP prefer `enqueue_workflow`, with `runWorkflow`/`run_workflow_now` deprecated as aliases that
still pass through admission.

## Why

- **Alternatives considered.** (a) Keep a direct `trigger_workflow` path that bypasses admission —
  rejected: a second entry point can violate capacity/mutex invariants and race the scheduler's own
  admission, producing over-capacity or double-run states. (b) Per-surface admission logic — rejected:
  duplicates the transaction and drifts (see ADR 0001). (c) One `dispatch_manual_run` choke point that
  all manual paths funnel through — chosen (Decision-3).
- **Evidence.** `dispatch_manual_run` was introduced as the admission choke point; `trigger_workflow`
  was removed; rerun was rerouted through admission and surfaces its admission outcome in the UI;
  `enqueue_workflow` is the preferred verb and the old run-now aliases are deprecated.

## Consequences

- **Enables.** A single place where capacity, mutex, queued-claim, and idempotency are enforced
  atomically for all manual runs; consistent admission outcomes across UI/SDK/MCP.
- **Forecloses.** No new manual-run path may bypass `dispatch_manual_run`; no reintroduction of a
  direct trigger command.
- **Invariant to keep true.** Manual runs are admission-gated in one `BEGIN IMMEDIATE` transaction;
  `enqueue` is the canonical verb.
