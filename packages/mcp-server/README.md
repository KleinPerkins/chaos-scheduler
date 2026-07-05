# @chaos-scheduler/mcp-server

The **Chaos MCP server** — a [Model Context Protocol](https://modelcontextprotocol.io)
server that exposes the Chaos Scheduler to MCP clients (Cursor, Cursor Cloud
Agents, and others) as **tools**, **resources**, and **prompts**. It is built on
[`@chaos-scheduler/sdk`](../sdk-ts) and mirrors exactly what the scheduler's REST
API (`/api/v1`) exposes — no duplicated business logic.

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
`list_queues`, `list_queued_runs`, `get_version`, `health_check`.

Write: `create_environment`, `register_workflow`, `update_workflow`,
`set_workflow_spec`, `delete_workflow`, `run_workflow_now`, `enqueue_workflow`,
`dispatch_workflow`, `rerun_workflow`.

Each write tool passes through the protected-environment guardrail; all tools
count against the optional tool-call budget. Dispatch tools forward `idempotency_key`;
replays return the original `run_id` or `queued_run_id`.

### Resources

`chaos://version`, `chaos://environments`, `chaos://workflows`,
`chaos://workflows/{id}`, `chaos://workflows/{id}/runs`, `chaos://runs/{id}`,
`chaos://runs/{id}/logs`, `chaos://queues`, `chaos://queued-runs`.
Freshness is pull-based (Cursor does not document resource subscriptions).

### Prompts

`triage_failed_run(run_id)`, `summarize_workflow_health(environment)`,
`register_workflow_for_repo(repo_path[, environment])`.

## Guardrails

- **Protected environments** — writes targeting a protected env (default
  `prod`/`production`) are rejected with a clear message unless
  `--allow-protected-writes` / `CHAOS_SCHEDULER_MCP_ALLOW_PROTECTED_WRITES=1`.
  The Cursor hook adds a second, client-side confirmation layer.
- **Tool budget** — set `CHAOS_SCHEDULER_MCP_MAX_TOOL_CALLS` to cap tool calls
  per server instance and stop runaway agent loops.
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
