# ADR 0004: MCP/SDK are app-owned provisioned artifacts version-pinned to the build (not DMG-bundled, not floating `npx`)
Status: accepted — 2026-08-22

## Decision

The desktop app is the lifecycle owner of the MCP server and SDK. It provisions the existing published
npm packages (`@chaos-scheduler/mcp-server`, `@chaos-scheduler/sdk`) into an app-managed directory at
a version pinned to the build, first-run/opt-in from the Integrations screen. Node is the only external
runtime dependency, and the app degrades gracefully if Node is absent. The provisioner
(`src-tauri/src/mcp.rs`) owns install/repair/uninstall, mutex recovery, orphaned-directory cleanup, and
`mcp.json` merge/backup.

## Why

- **Alternatives considered.** (a) Bundle the MCP/SDK inside the DMG — rejected: a macOS DMG is a
  drag-install with no installer script, and bundling couples MCP/SDK release cadence to the desktop
  build. (b) Invoke via floating `npx @chaos-scheduler/mcp-server` — rejected: an unpinned `npx`
  resolves to whatever version is latest at run time, breaking reproducibility and letting a bad
  upstream publish silently change local behavior. (c) App-managed dir, version pinned to the build,
  opt-in provisioning — chosen.
- **Evidence.** `mcp.rs` is the ~2200-line managed provisioner; the build stamps the pinned MCP
  version; distribution is opt-in from the Integrations screen because there is no DMG install hook.

## Consequences

- **Enables.** Reproducible, version-locked MCP/SDK per build; a single owner for install/repair/
  uninstall and for safe `mcp.json` merges; clean degradation when Node is missing.
- **Forecloses.** No DMG-bundled MCP/SDK; no floating `npx`; no provisioning path that is not
  version-pinned to the build.
- **Invariant to keep true.** The app owns the managed directory and the pinned version; `mcp.json`
  is merged/backed-up, never blindly overwritten.
