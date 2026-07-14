# @chaos-scheduler/mcp-server

The **Chaos MCP server** — a [Model Context Protocol](https://modelcontextprotocol.io)
server that exposes the Chaos Scheduler to MCP clients (Cursor, Cursor Cloud
Agents, and others) as **tools**, **resources**, and **prompts**. It is built on
[`@chaos-scheduler/sdk`](../sdk-ts): tools use the scheduler's REST API
(`/api/v1`), while versioned discovery resources derive safe `stored_config`
views without duplicating runtime business logic.

Direction: **Cursor → Scheduler** (the reverse direction, Scheduler → Cursor via
Cloud Agents, lives in the Rust backend's `cursor_agent` operator).

## Transports

| Transport           | Use                        | How                          |
| ------------------- | -------------------------- | ---------------------------- |
| **stdio**           | Local, Cursor-managed proc | `chaos-mcp-server` (default) |
| **Streamable HTTP** | Remote / team, single URL  | `chaos-mcp-server --http`    |

Both advertise the same tools/resources/prompts. HTTP mode is **stateless** (a
fresh server per request), so it scales horizontally.

## Configuration

All via `CHAOS_SCHEDULER_*` env vars (CLI flags override):

| Env var                                      | Default                 | Meaning                                           |
| -------------------------------------------- | ----------------------- | ------------------------------------------------- |
| `CHAOS_SCHEDULER_URL`                        | `http://127.0.0.1:9618` | Scheduler REST base URL                           |
| `CHAOS_SCHEDULER_API_KEY`                    | —                       | Scoped API key (`<id>.<secret>`)                  |
| `CHAOS_SCHEDULER_MCP_TRANSPORT`              | `stdio`                 | `stdio` \| `http`                                 |
| `CHAOS_SCHEDULER_MCP_HTTP_HOST`              | `127.0.0.1`             | HTTP bind host                                    |
| `CHAOS_SCHEDULER_MCP_HTTP_PORT`              | `9700`                  | HTTP bind port                                    |
| `CHAOS_SCHEDULER_MCP_ALLOW_REMOTE_HTTP`      | `false`                 | Permit non-loopback HTTP binds                    |
| `CHAOS_SCHEDULER_MCP_HTTP_MAX_BODY_BYTES`    | `1048576`               | HTTP request body cap                             |
| `CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS` | `prod,production`       | Env names whose writes are blocked                |
| `CHAOS_SCHEDULER_MCP_ALLOW_PROTECTED_WRITES` | `false`                 | Permit writes to protected environments           |
| `CHAOS_SCHEDULER_MCP_MAX_TOOL_CALLS`         | `0` (unlimited)         | Per-process tool-call budget (runaway-loop guard) |
| `CHAOS_SCHEDULER_MCP_REQUEST_TIMEOUT_MS`     | `30000`                 | Per-request SDK timeout                           |

CLI flags: `--stdio`, `--http`, `--host <h>`, `--port <p>`,
`--allow-remote-http`, `--http-max-body-bytes <n>`, `--url <baseUrl>`,
`--allow-protected-writes`, `--help`.

## Run it

Local (stdio):

```bash
CHAOS_SCHEDULER_URL=http://127.0.0.1:9618 \
CHAOS_SCHEDULER_API_KEY=<id.secret> \
npx -y @chaos-scheduler/mcp-server
```

Remote/team (Streamable HTTP):

```bash
CHAOS_SCHEDULER_URL=http://127.0.0.1:9618 \
CHAOS_SCHEDULER_MCP_ALLOW_REMOTE_HTTP=1 \
npx -y @chaos-scheduler/mcp-server --http --allow-remote-http --host 0.0.0.0 --port 9700
# → POST http://<host>:9700/mcp   (GET /health for a liveness probe)
```

In HTTP mode the per-request API key is required in the incoming
`Authorization: Bearer` header. The server does not fall back to
`CHAOS_SCHEDULER_API_KEY`; that key is for local stdio mode.

## Add to Cursor

The repo ships a working local config at [`.cursor/mcp.json`](../../.cursor/mcp.json)
(stdio, pointing at the built server) and a remote HTTP template at
[`.cursor/mcp.remote.example.json`](../../.cursor/mcp.remote.example.json).
Minimal stdio entry:

```json
{
  "mcpServers": {
    "chaos-scheduler": {
      "command": "npx",
      "args": ["-y", "@chaos-scheduler/mcp-server"],
      "env": {
        "CHAOS_SCHEDULER_URL": "http://127.0.0.1:9618",
        "CHAOS_SCHEDULER_API_KEY": "<id.secret>"
      }
    }
  }
}
```

A Project Rule ([`.cursor/rules/chaos-scheduler.mdc`](../../.cursor/rules/chaos-scheduler.mdc))
teaches the agent when and how to call these tools, and local hooks
([`.cursor/hooks.json`](../../.cursor/hooks.json)) add a confirm-on-protected-write
guard plus an audit log.

## Capabilities

### Tools

Read: `list_environments`, `list_workflows`, `get_workflow`,
`list_workflow_runs`, `get_run`, `get_run_logs`, `get_run_tasks`, `get_run_metrics`,
`list_queues`, `list_queued_runs`, `list_email_profiles`, `get_version`,
`health_check`.

Write: `create_environment`, `register_workflow`, `update_workflow`,
`set_workflow_spec`, `patch_workflow_spec`, `delete_workflow`,
`run_workflow_now`, `enqueue_workflow`, `dispatch_workflow`, `rerun_workflow`,
`create_email_profile`,
`update_email_profile`, `delete_email_profile`, `set_workflow_email_profile`.

Email-profile `smtp_password` values are masked (`••••••••`) on read; echo the
mask back on update to keep the stored secret.

Workflow/environment-scoped write tools pass through the
protected-environment guardrail; workflow email-profile assignment is included.
Global email-profile CRUD has no environment target and relies on API scope/auth.
All tools count against the optional tool-call budget. Dispatch tools forward
`idempotency_key`; replays return the original `run_id` or `queued_run_id`.

`patch_workflow_spec` applies RFC 7396 JSON Merge Patch to the full stored spec
internally. Omitted fields and `__redacted__` secret sentinels preserve their
stored values; the tool validates the merged authoring shape, writes through the
SDK, and returns only the redacted definition. Use `set_workflow_spec` only for
an intentional full replacement. The patch flow is read-merge-write, so
concurrent spec writers must be serialized until a backend compare-and-swap
contract exists. A sentinel inside a replaced array requires a unique `id` or an
unchanged webhook URL; ambiguous edits fail closed, and identity changes require
the real secret. `dispatch_workflow` also accepts SDK-supported `event_id` and
`timestamp` fields for deterministic signed replays.

`run_workflow_now` is **deprecated** — manual runs are admission-controlled, so it
is an alias of `enqueue_workflow` (same queued path, same result). It keeps working
unchanged, but prefer `enqueue_workflow`.

### Resources

Discovery: `chaos://authoring`, `chaos://catalog`,
`chaos://guides/{workflows|webhooks|integrations}`, and
`chaos://schemas/{workflow-spec|triggers|queue|integrations}`.

Workflow state: `chaos://workflows/index`, `chaos://workflows/{id}/definition`,
`chaos://workflows`, `chaos://workflows/{id}`, and
`chaos://workflows/{id}/runs`.

Other state: `chaos://version`, `chaos://environments`, `chaos://runs/{id}`,
`chaos://runs/{id}/logs`, `chaos://runs/{id}/tasks`,
`chaos://runs/{id}/metrics`, `chaos://queues`, `chaos://queued-runs`, and
`chaos://email-profiles`. Workflow-ID resource templates offer prefix-filtered,
deterministic completions using MCP's native 100-value response cap and truthful
`total` / `hasMore` metadata; template listing stays static and does not fetch
backend records.
Freshness is pull-based (Cursor does not document resource subscriptions).

Workflow resources are safe context projections, not write round-trip payloads.
They always redact known nested secret fields regardless of API-key scope,
bound parsing of `spec_json` / `trigger_config` / `queue_config`, and replace
malformed nested JSON with `__redacted_invalid_json__` rather than exposing the
raw value. Use workflow write tools, not a resource payload, for mutations.
Derived discovery/index/definition payloads are versioned `v1` and explicitly
labeled `stored_config`. Their permissive Zod schemas preserve additive fields
at the MCP validation boundary, but Rust/backend validation and persistence
normalization remain authoritative; the current backend may drop unsupported
fields. A parsed definition is not effective configuration or proof of runtime
enforcement. Inbound webhook secret configuration/status is not available
through MCP.

### Prompts

`triage_failed_run(run_id)`, `summarize_workflow_health(environment)`,
`register_workflow_for_repo(repo_path[, environment])`, and
`safely_update_workflow(workflow_id)`.

Failure triage starts with the run summary/logs, reads task or metric detail only
when needed, and asks for explicit operator confirmation before any retry. After
confirmation, use `rerun_workflow` with the failed source run for a faithful
retry.

## Guardrails

- **Protected environments (fail-closed)** — when
  `CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS` is set and protected writes are
  not allowed, write tools resolve the workflow environment via `get_workflow`
  and **refuse** if lookup fails (no silent allow). `update_workflow` also checks
  `patch.environment` against the protected list. Mirror the backend list with
  `CHAOS_SCHEDULER_PROTECTED_ENVIRONMENTS` (example in `.cursor/mcp.json`).
- **Tool budget** — `CHAOS_SCHEDULER_MCP_MAX_TOOL_CALLS` caps tool calls per MCP
  process. Stdio = one budget per Cursor session; Streamable HTTP shares one
  in-process budget across requests on that server instance.
- **Cursor hooks vs MCP** — `.cursor/hooks/guard-writes.sh` stays **fail-open**
  (warn/confirm); MCP guardrails are **fail-closed** for protected env writes.
- **HTTP auth and bind safety** — Streamable HTTP requires per-request bearer
  auth, rejects DNS-rebinding-style Host headers on loopback binds, caps request
  bodies, and requires `--allow-remote-http` before binding outside loopback.
- **Scoped auth** — the server never mints keys; it forwards a scoped API key to
  the REST API, which enforces `read`/`write`/`admin` scopes per endpoint.

## Development

```bash
npm install
npm run build      # tsup → dist (ESM + d.ts); bundles @chaos-scheduler/sdk
npm test           # vitest (in-memory MCP client ↔ server, fake-fetch SDK)
npm run typecheck
```

> Note: the local SDK is referenced via `file:../sdk-ts`; build the SDK first
> (`npm --prefix ../sdk-ts run build`) or run the repo-level build.

## License

MIT
