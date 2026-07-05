# Hardening Gap Closure — Final Report

**Status:** Complete (all behavior merged on `main` as of 2026-07-05)  
**Supersedes:** the living-ledger canvas `chaos-scheduler-hardening-ledger` (Cursor
project canvas) for gap-closure disposition. That canvas remains a historical
audit trail for the broader Hardening v2 program; this document is the
**authoritative** per-concern closure record for the 11 gap-closure items.

## Version drift (known, not hand-fixed)

| Location                           | Version |
| ---------------------------------- | ------- |
| `packages/sdk-ts/package.json`     | `0.4.0` |
| `packages/mcp-server/package.json` | `0.4.0` |
| `.release-please-manifest.json`    | `0.2.0` |
| `src-tauri/Cargo.toml`             | `0.2.0` |

Release PR **#35** (`chore: release main` → `0.3.0`) reconciles these via
release-please. Versions were **not** hand-edited during gap closure; Conventional
Commits drive the manifest.

## Disposition table

| ID      | Concern                                   | Root cause                                                                                                               | Fix                                                                                                                                                                                                           | PR                                                                                                                                                  | Verification                                                                                             |
| ------- | ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| A1      | Bounded graceful shutdown                 | `quit_app` / dock-quit set `SHUTDOWN` nowhere; workers stuck in poll/retry loops; main-thread drain unsafe               | Single `ExitRequested` choke point: re-entrancy guard, `prevent_exit`, `SHUTDOWN` + interruptible sleep in poll/retry, fixed-grace off-main-thread `exit(0)`                                                  | [#70](https://github.com/KleinPerkins/chaos-scheduler/pull/70)                                                                                      | `claim_exit_shutdown_is_idempotent`; bounded shutdown kills in-flight child; `SHUTDOWN`-during-poll test |
| A2      | SDK canonical inbound webhook signing     | SDK sent raw-body HMAC; backend requires `METHOD\nPATH\nTIMESTAMP\nSHA256(body)` → guaranteed 401 with secret configured | `computeInboundDispatchSignature` / `inboundDispatchHeaders`; `eventId`/`timestamp` on dispatch; `queued_run_id` on `DuplicateDispatch`; pinned `webhook-vectors.v1.json`; INTEGRATION/sdk README/cursor rule | [#69](https://github.com/KleinPerkins/chaos-scheduler/pull/69)                                                                                      | SDK + API unit tests; vectors in `packages/test-fixtures/webhook-vectors.v1.json`                        |
| A3      | MCP fail-closed guardrail + shared budget | Lookup errors swallowed → silent allow; `update_workflow` skipped destination-env check; per-request HTTP budget         | Rethrow lookup errors under active protection; `assertEnvironmentWritable(patch.environment)`; shared in-process `ToolBudget`; callTool smoke tests; MCP README guardrails                                    | [#68](https://github.com/KleinPerkins/chaos-scheduler/pull/68)                                                                                      | `server.test.ts` fail-closed + destination-env; HTTP budget shared across requests                       |
| B2      | Outbound webhook SSRF                     | Literal-only host check; default redirect follow; `::ffff:127.0.0.1` bypass; DNS-rebind TOCTOU                           | `DnsResolver` seam; resolve+pin via `ClientBuilder::resolve`; `Policy::none()`; IPv4-mapped blocklist                                                                                                         | [#71](https://github.com/KleinPerkins/chaos-scheduler/pull/71)                                                                                      | Injected resolver → localhost blocked; redirect/IPv4-mapped unit tests                                   |
| B3      | Child env secret scrub                    | Spawned scripts inherited full app env including scheduler secrets                                                       | Deny-list scrub before spawn: `CURSOR_API_KEY`, `SMTP_PASSWORD`, `CHAOS_SCHEDULER_API_*`, internal `*_SECRET`/`*_TOKEN`; preserve user vars                                                                   | [#72](https://github.com/KleinPerkins/chaos-scheduler/pull/72)                                                                                      | `scrub_scheduler_secrets_from_child_removes_denied_keys`; child `printenv` non-inheritance               |
| B4      | REST + metrics remote-bind gate           | REST/metrics bound `0.0.0.0` without opt-in while MCP required `--allow-remote-http`                                     | `validate_remote_api_bind` + `CHAOS_SCHEDULER_ALLOW_REMOTE_API=1` on REST and metrics startup                                                                                                                 | [#73](https://github.com/KleinPerkins/chaos-scheduler/pull/73)                                                                                      | `validate_remote_api_bind_allows_loopback_and_blocks_remote_without_flag`                                |
| B5a     | Read-scope secrets redaction              | `get_workflow` returned full `spec_json` secrets to `read` scope; MCP/`chaos://` mirrored                                | `READ_SCOPE_SECRET_SENTINEL` (`__redacted__`) in service layer; write/admin round-trip preserved                                                                                                              | [#74](https://github.com/KleinPerkins/chaos-scheduler/pull/74)                                                                                      | Cross-surface no-secret-bytes test; read body contains `__redacted__`                                    |
| D1      | `poll_exhausted` first-class status       | Terminal status collapsed to `failed` at `execute_typed_operator` / `finish_run` / webhook payload                       | Thread `poll_exhausted` through operator outcome, DB row, completion webhook JSON `status` field; UI filter + badge                                                                                           | [#75](https://github.com/KleinPerkins/chaos-scheduler/pull/75) (backend), [#77](https://github.com/KleinPerkins/chaos-scheduler/pull/77) (frontend) | `finish_run_persists_poll_exhausted_run_row_status`; GlobalHistory filter/label                          |
| C1-R    | Rust test hygiene                         | Orphan-recovery branches undertested; no migration fixture test; unbounded `.bak` files                                  | Orphan-recovery branch matrix; migration v(N-1)→v(N) fixture; `.bak` prune (keep last 3)                                                                                                                      | [#76](https://github.com/KleinPerkins/chaos-scheduler/pull/76)                                                                                      | `#[cfg(unix)]` orphan matrix; migration fixture assertion                                                |
| Vectors | Shared webhook vectors cross-check        | Risk of Rust/TS drift on canonical signing                                                                               | Rust `include_str!` test of `webhook-vectors.v1.json` after A2 merged                                                                                                                                         | [#78](https://github.com/KleinPerkins/chaos-scheduler/pull/78)                                                                                      | `cargo test` webhook vector cross-check                                                                  |
| C2      | jsx-a11y + contrast + UX partials         | axe `color-contrast` disabled; muted tokens below WCAG AA; missing poll-error banner, `aria-current`, revoke auto-reset  | `eslint-plugin-jsx-a11y` error; contrast token fixes; axe re-enabled; RunDetail/Dashboard/Integrations UX partials                                                                                            | [#79](https://github.com/KleinPerkins/chaos-scheduler/pull/79)                                                                                      | eslint + e2e axe `color-contrast` passes                                                                 |
| C1-E2E  | Playwright matrix (8 specs)               | Only 2 e2e specs; no feature-flow coverage                                                                               | 8 specs with inline axe; CI trigger scoped to `e2e/**` + `src/components/**`                                                                                                                                  | [#80](https://github.com/KleinPerkins/chaos-scheduler/pull/80)                                                                                      | `npm run test:e2e` green in CI                                                                           |
| D2      | Docs + closure report                     | SECURITY.md stale; no formal disposition record                                                                          | This document + [SECURITY.md](../SECURITY.md) sweep + INTEGRATION coherence                                                                                                                                   | _(this PR)_                                                                                                                                         | Docs review; full CI gate                                                                                |

## Explicit non-goals (unchanged)

- macOS Keychain / OS-ACL secrets-at-rest
- Multi-tenant per-token HTTP rate limiting (folded into MCP in-process budget)
- Repo-wide coverage thresholds
- Real-Tauri-shell Playwright
- Hand-editing `CHANGELOG.md`, `.release-please-manifest.json`, or package versions

## Definition of done — checklist

- [x] All 11 gap-closure concerns on `main` with failing-first regression tests
- [x] Shutdown bounded; no main-thread block
- [x] SDK/MCP/backend agree on inbound signing; shared pinned vectors green
- [x] `poll_exhausted` end-to-end (run row + UI + webhook payload `status`)
- [x] E2E matrix + jsx-a11y + axe `color-contrast`
- [x] Gap-closure report committed
- [ ] Release PR #35 merged → `0.3.0` shipped (operator-gated; out of scope for this PR)

## Next step

Merge Release PR **#35** after confirming `main` CI green, then smoke: auto-update,
MCP stdio, inbound webhook dispatch with configured secret.
