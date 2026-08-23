# ADR 0001: SchedulerService is the single business-logic boundary; IPC/REST/SDK/MCP are thin adapters
Status: accepted — 2026-08-22

## Decision

All scheduling business logic and governance live in one GUI-agnostic Rust type, `SchedulerService`
(`src-tauri/src/service.rs`). The four external surfaces — Tauri IPC (`commands.rs`), REST `/api/v1`
(`api.rs`), the npm SDK (`packages/sdk-ts`), and the MCP server (`packages/mcp-server`) — are thin
adapters that call the same service methods. The public layering is MCP → SDK → REST → service:
`mcp-server` consumes `@chaos-scheduler/sdk`'s `ChaosSchedulerClient`, which speaks REST, which
delegates to `SchedulerService`. Side effects are injected via traits (`Notifier`, `Clock`,
`ProcessRunner`) so the service is testable without a GUI or a real clock/process.

## Why

- **Alternatives considered.** (a) Duplicate validation/governance in each adapter — rejected: four
  copies of admission, redaction, and environment rules inevitably drift, and a fix in one surface
  silently misses the others. (b) A shared library of helpers without a single owning type — rejected:
  helpers do not enforce a boundary; callers can still reach around them. (c) One service type that
  every governed path must call — chosen.
- **Evidence.** `service.rs` holds no `tauri::AppHandle` dependency and is exercised by unit tests
  with injected traits; `api.rs` delegates to `.service.` consistently. A deliberate CQRS split
  remains: governed writes/validation route through `state.service`, while analytical read-model
  queries (dashboard KPIs, history, metrics) read `state.db` directly — acceptable because there is
  no governance to duplicate on a pure read path and scoped redaction still lives on the service.

## Consequences

- **Enables.** New surfaces are cheap and safe (they wrap existing methods); governance is fixed once.
- **Forecloses.** Adapters must not embed business rules or reach past the service for governed
  writes. A new write path that bypasses `SchedulerService` is a regression against this ADR.
- **Invariant to keep true.** The MCP → SDK → REST → service layering is one-directional; the service
  never depends on an adapter. Read-path DB access stays read-only and redaction-aware.
