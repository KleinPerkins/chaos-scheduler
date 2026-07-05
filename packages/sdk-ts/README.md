# @chaos-scheduler/sdk

Typed TypeScript client for the **Chaos Scheduler** REST API (`/api/v1`), plus
webhook signature helpers. It is the foundation for the
[`@chaos-scheduler/mcp-server`](../mcp-server) and for any external system that
registers workflows, triggers runs on demand, polls results, or receives result
webhooks.

- Zero runtime dependencies (uses the global `fetch` and `node:crypto`).
- Ships ESM + CJS + `.d.ts`.
- Wire types are hand-derived from the Rust backend — see
  [Source of truth](#source-of-truth).

## Install

```bash
npm install @chaos-scheduler/sdk
```

Requires Node.js ≥ 18 (for global `fetch`). Pass a `fetch` implementation in the
options for older runtimes.

## Quick start

```ts
import { ChaosSchedulerClient } from "@chaos-scheduler/sdk";

const client = new ChaosSchedulerClient({
  baseUrl: "http://127.0.0.1:9618", // the scheduler's loopback API
  apiKey: process.env.CHAOS_SCHEDULER_API_KEY, // "<id>.<secret>"
});

// 1) Register an externally-managed workflow (scope: write)
const wf = await client.registerWorkflow({
  name: "Nightly digest",
  script_path: "scripts/digest.py",
  cron_schedule: "0 6 * * *",
  environment: "instance",
});

// 2) Run it now, safely retryable via an idempotency key (scope: write)
const outcome = await client.runWorkflow(wf.id, {
  idempotencyKey: crypto.randomUUID(),
});

// 3) Poll the run to completion (read)
if (!("workflow_id" in outcome)) {
  // duplicate replay — nothing new started
} else if (outcome.run_id) {
  const run = await client.waitForRun(outcome.run_id);
  console.log(run.status, run.exit_code);
}
```

## Authentication

The API authenticates with **scoped API keys** minted in the Scheduler UI
(Integrations screen) or via Tauri commands. A token is `"<id>.<secret>"` and is
sent as `Authorization: Bearer <id>.<secret>`. Scopes:

| Scope   | Grants                                       |
| ------- | -------------------------------------------- |
| `read`  | list/get environments, workflows, runs       |
| `write` | create/register/delete, run/enqueue/dispatch |
| `admin` | superuser (implies `read` + `write`)         |

`write` and `admin` keys are local-code-execution credentials: a holder can
register or run workflows that execute commands on the scheduler host. Store
them in a secret manager, rotate/revoke aggressively, and avoid putting them in
logs, prompts, or issue trackers.

Protected environments default to `prod,production`. Backend write paths refuse
to create, edit, delete, or execute workflows in those environments unless the
scheduler process is started with
`CHAOS_SCHEDULER_ALLOW_PROTECTED_WRITES=1`. Override the protected-name list with
`CHAOS_SCHEDULER_PROTECTED_ENVIRONMENTS=prod,production,...`.

`getHealth()` and `getVersion()` need no key.

> **Note:** API-key creation/listing is **not** exposed over REST; keys are
> managed inside the desktop app. The SDK therefore has no key-management
> methods.

## Idempotency

`runWorkflow`, `enqueueWorkflow`, and `dispatchWorkflow` accept an
`idempotencyKey`. Reusing a key returns the original result as
`{ status: "duplicate", run_id, queued_run_id }`. Queued dispatches replay with `queued_run_id`; admitted dispatches replay with `run_id`. Use the `isDuplicateDispatch` guard:

```ts
import { isDuplicateDispatch } from "@chaos-scheduler/sdk";

const res = await client.runWorkflow(id, { idempotencyKey: key });
if (isDuplicateDispatch(res)) {
  // replay: res.run_id or res.queued_run_id points at the first request
} else {
  // fresh dispatch: res.status is admitted/queued/skipped, res.run_id is new
}
```

## Inbound webhook trigger (signed)

`POST /api/v1/workflows/{id}/dispatch` accepts a raw body forwarded to the
workflow's `webhook` trigger. If an inbound secret is configured on the
scheduler, sign the raw body:

```ts
await client.dispatchWorkflow(id, {
  payload: JSON.stringify({ event: "push", ref: "main" }),
  signatureSecret: process.env.INBOUND_SECRET, // → X-Chaos-Signature: sha256=<hmac>
});
```

## Verifying outbound result webhooks

The scheduler's `webhook` action POSTs the run result to your endpoint with:

- `X-Chaos-Event: run.succeeded | run.failed`
- `X-Chaos-Signature: sha256=<hex HMAC-SHA256 of the raw body>`

Verify it over the **raw** request body (never a re-serialized object):

```ts
import { verifyWebhookSignature } from "@chaos-scheduler/sdk";
import express from "express";

const app = express();
app.post(
  "/chaos-webhook",
  express.raw({ type: "application/json" }),
  (req, res) => {
    const ok = verifyWebhookSignature(
      req.body, // Buffer of raw bytes
      req.header("x-chaos-signature"),
      process.env.WEBHOOK_SECRET!,
    );
    if (!ok) return res.status(401).end();
    const result = JSON.parse(req.body.toString("utf8"));
    // handle result…
    res.status(200).end();
  },
);
```

The signature scheme (`hex(HMAC_SHA256(secret, raw_body))`) is verified in the
SDK's test suite against a cross-implementation vector shared with the backend.

## API surface

Client methods (all return typed models):

| Method                       | Endpoint                               | Scope |
| ---------------------------- | -------------------------------------- | ----- |
| `getHealth()`                | `GET /api/v1/health`                   | —     |
| `getVersion()`               | `GET /api/v1/version`                  | —     |
| `listEnvironments()`         | `GET /api/v1/environments`             | read  |
| `createEnvironment(input)`   | `POST /api/v1/environments`            | write |
| `listWorkflows()`            | `GET /api/v1/workflows`                | read  |
| `getWorkflow(id)`            | `GET /api/v1/workflows/{id}`           | read  |
| `registerWorkflow(input)`    | `POST /api/v1/workflows`               | write |
| `deleteWorkflow(id)`         | `DELETE /api/v1/workflows/{id}`        | write |
| `setWorkflowSpec(id, spec)`  | `POST /api/v1/workflows/{id}/spec`     | write |
| `runWorkflow(id, opts)`      | `POST /api/v1/workflows/{id}/run`      | write |
| `enqueueWorkflow(id, opts)`  | `POST /api/v1/workflows/{id}/enqueue`  | write |
| `dispatchWorkflow(id, opts)` | `POST /api/v1/workflows/{id}/dispatch` | write |
| `listRuns(id)`               | `GET /api/v1/workflows/{id}/runs`      | read  |
| `getRun(id)`                 | `GET /api/v1/runs/{id}`                | read  |
| `waitForRun(runId, opts)`    | polls `GET /api/v1/runs/{id}`          | read  |

Webhook helpers: `computeWebhookSignature`, `webhookSignatureHeader`,
`verifyWebhookSignature`. Errors: `ChaosApiError` (`.status`, `.isAuthError`,
`.isRateLimited`, `.isNotFound`).

## Source of truth

There is no generated OpenAPI document yet. The wire types in `src/types.ts` are
hand-derived from the Rust backend and must be kept in sync with:

- `src-tauri/src/db.rs` — `Workflow`, `Run`, `Environment`
- `src-tauri/src/workflow_spec.rs` — `WorkflowSpec` and friends
- `src-tauri/src/actions.rs` — `ActionSpec`, HMAC `sign_payload`
- `src-tauri/src/scheduler.rs` — `DispatchOutcome`
- `src-tauri/src/api.rs` — routes, request bodies, response envelopes

## Development

```bash
npm install
npm run build      # tsup → dist (ESM + CJS + d.ts)
npm test           # vitest
npm run typecheck  # tsc --noEmit
```

## License

MIT
