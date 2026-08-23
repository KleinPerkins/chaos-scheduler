# ADR 0005: Bundle `mcp-server` with tsup but keep `@chaos-scheduler/sdk` external
Status: accepted — 2026-08-22

## Decision

`@chaos-scheduler/mcp-server` is built with tsup and bundles its transitive dependencies (e.g.
`@modelcontextprotocol/sdk`, `zod`) inline for a zero-npm-footprint artifact. `@chaos-scheduler/sdk`
MUST remain `external` in the tsup config (never listed in `noExternal`) so it is resolved as a normal
dependency at install time rather than inlined into the mcp-server bundle.

## Why

- **Alternatives considered.** (a) Bundle everything including the SDK — rejected: inlining the SDK
  freezes a copy of it inside every mcp-server build, so an independent SDK release would not take
  effect and the two packages' versions would silently diverge. (b) Externalize everything (no
  bundling) — rejected: it reintroduces a large transitive install footprint for the provisioned
  mcp-server, which the app wants to keep minimal. (c) Bundle transitive deps but keep the SDK
  external — chosen.
- **Evidence.** The mcp-server consumes the SDK's `ChaosSchedulerClient` (see ADR 0001's layering);
  keeping the SDK external preserves independent SDK releases while still yielding a small mcp-server
  artifact.

## Consequences

- **Enables.** Independent versioning/release of the SDK; a compact, dependency-inlined mcp-server.
- **Forecloses.** `@chaos-scheduler/sdk` must never be added to tsup `noExternal`; doing so breaks
  independent SDK releases.
- **Invariant to keep true.** SDK external, transitive deps inlined — verified as part of the release
  ordering (sdk → mcp-server → clean-install CLI smoke → desktop).
