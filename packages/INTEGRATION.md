# Chaos Scheduler — integration guide

How external systems and Cursor integrate with the Chaos Scheduler. Two
packages live here:

- [`@chaos-scheduler/sdk`](./sdk-ts) — typed client for the REST API (`/api/v1`).
- [`@chaos-scheduler/mcp-server`](./mcp-server) — MCP server (built on the SDK)
  exposing the scheduler to Cursor and other MCP clients.

The scheduler runs an embedded HTTP API (default `http://127.0.0.1:9618`,
loopback-only) implemented in `src-tauri/src/api.rs`. It is the single source of
truth for the contract; the SDK types are hand-derived from the Rust models
(see the SDK README's _Source of truth_).

## 1. Authentication

Mint a **scoped API key** in the desktop app (Integrations screen). A token is
`"<id>.<secret>"`, sent as `Authorization: Bearer <id>.<secret>`. Scopes:
`read`, `write`, `admin` (admin implies the others). `GET /health` and
`GET /version` are unauthenticated.

> Key creation/listing is not exposed over REST — it is a desktop-app operation.

## 2. Register a workflow (write)

```ts
import { ChaosSchedulerClient } from "@chaos-scheduler/sdk";

const client = new ChaosSchedulerClient({
  baseUrl: "http://127.0.0.1:9618",
  apiKey: process.env.CHAOS_SCHEDULER_API_KEY,
});

const wf = await client.registerWorkflow({
  name: "Nightly digest",
  script_path: "scripts/digest.py",
  cron_schedule: "0 6 * * *",
  environment: "production",
  // optional multi-step execution spec:
  spec: {
    kind: "generic",
    generic: {
      steps: [
        { id: "pull", command: "git pull" },
        { id: "run", script: "scripts/digest.py", depends_on: ["pull"] },
      ],
    },
    on_failure: [{ type: "email" }], // email is always available
  },
});
```

API-registered workflows are `managed_externally=true` (read-only in the UI).
Registration is non-idempotent. MCP agents should read
`chaos://workflows/index` before calling `register_workflow` so an existing
definition is updated rather than duplicated.

## 3. Run or enqueue on demand (write, idempotent)

```ts
import { isDuplicateDispatch } from "@chaos-scheduler/sdk";

const key = crypto.randomUUID();
const res = await client.enqueueWorkflow(wf.id, { idempotencyKey: key });
// Retrying with the SAME key returns { status: "duplicate", run_id, queued_run_id } — safe.
if (!isDuplicateDispatch(res) && res.run_id) {
  const run = await client.waitForRun(res.run_id);
  console.log(run.status, run.exit_code);
}
```

- `enqueueWorkflow` queues a run through admission control (the scheduler manages
  concurrency). `runWorkflow` is a **deprecated** alias: it posts to `/run` but
  takes the same admission-controlled path — it does **not** dispatch immediately
  or bypass the queue — so prefer `enqueueWorkflow`.
- `dispatchWorkflow` triggers a workflow's inbound `webhook` trigger with a raw
  payload (optionally HMAC-signed — see below).

## 4. Event-driven trigger (inbound webhook)

If the scheduler has an inbound webhook secret configured, sign with the
**canonical** scheme (not raw-body HMAC):

```
METHOD\nPATH\nTIMESTAMP\nSHA256_HEX(raw_body)
→ hex(HMAC_SHA256(secret, canonical))
```

Headers: `X-Chaos-Timestamp`, `X-Chaos-Event-Id`, `X-Chaos-Signature: sha256=<hex>`.

```ts
await client.dispatchWorkflow(wf.id, {
  payload: JSON.stringify({ event: "push", ref: "main" }),
  signatureSecret: process.env.INBOUND_SECRET,
  // optional pinned replay fields:
  // timestamp: "1700000000",
  // eventId: "evt-pinned-001",
});
```

`dispatchWorkflow` sets all three headers when `signatureSecret` is provided.
The scheduler's `inbound_webhook_secret` configuration and status are not
available through MCP; the caller must receive the signing secret through an
appropriate external secret channel.
Manual signing:

```ts
import { inboundDispatchHeaders } from "@chaos-scheduler/sdk";

const path = `/api/v1/workflows/${encodeURIComponent(wf.id)}/dispatch`;
const headers = inboundDispatchHeaders({
  path,
  body: payload,
  secret: process.env.INBOUND_SECRET!,
});
```

Verified by `api.rs::verify_inbound_webhook` / `inbound_canonical_payload`.
Cross-language vectors live in `packages/test-fixtures/webhook-vectors.v1.json`.

## 5. Receive result webhooks (outbound)

Configure a `webhook` action on a workflow's `on_success`/`on_failure`. On
completion the scheduler POSTs the run result to your endpoint with:

- `X-Chaos-Event: run.succeeded | run.failed` (binary; `poll_exhausted` runs
  emit `run.failed` but the JSON body carries `"status": "poll_exhausted"`)
- `X-Chaos-Signature: sha256=<hex HMAC-SHA256 of the raw body>`

Verify over the **raw** bytes:

```ts
import { verifyWebhookSignature } from "@chaos-scheduler/sdk";

// express example — use express.raw() so req.body is the raw Buffer
const ok = verifyWebhookSignature(
  req.body,
  req.header("x-chaos-signature"),
  process.env.WEBHOOK_SECRET!,
);
if (!ok) return res.sendStatus(401);
```

Both the signing and verification match `src-tauri/src/actions.rs::sign_payload`
and are covered by cross-implementation test vectors in the SDK.

## 6. Poll runs / read state (read)

`listWorkflows`, `getWorkflow`, `listRuns`, `getRun`, `getRunLogs`,
`getRunTasks`, `getRunMetrics`, `listQueues`, `listQueuedRuns`,
`listEnvironments`, plus `waitForRun` (client-side polling with a default 5-minute
overall timeout; the backend has no long-poll).

For deeper run detail and scheduler state:

- `getRunLogs(id)` — stdout/stderr, exit code, and result URL for a run.
- `getRunTasks(id)` — per-step tasks and retry attempts (step-flow execution).
- `getRunMetrics(id)` — metric samples emitted during the run.
- `listQueues()` — per-environment queue capacity snapshots (active/queued
  counts and caps).
- `listQueuedRuns()` — durable queued runs awaiting or undergoing admission.

**Read-scope redaction** — REST/SDK `getWorkflow` / `listWorkflows` and the MCP
`get_workflow` tool replace nested secret fields (`secret`,
`signature_secret`, `cursor_api_key`, `smtp_password`) with the stable sentinel
`__redacted__` when the caller has **read** scope only. **Write/admin** tool
calls receive full values so PATCH round-trips work. MCP workflow resources
(`chaos://workflows` and `chaos://workflows/{id}`) add a second, scope-independent
projection: they always redact those fields before injecting workflow state
into agent context, bound nested JSON parsing, and replace malformed nested JSON
with `__redacted_invalid_json__`. The versioned
`chaos://workflows/{id}/definition` resource additionally separates parsed
spec/trigger/queue values, completion actions, and parse warnings. It is labeled
`stored_config`: parsing does not prove effective runtime enforcement. See
[SECURITY.md](../SECURITY.md#secrets-storage--read-scope-redaction).

Terminal run `status` values include `success`, `failed`, `cancelled`,
`stale`, and `poll_exhausted` (cloud-agent poll budget exhausted).

`waitForRun` throws once `timeoutMs` (default 300000) elapses without a terminal
status, so a slow run is distinguishable from a failed one.

## 7. Drive it from Cursor (MCP)

Run the MCP server and add it to Cursor (see the
[mcp-server README](./mcp-server/README.md) and the repo's `.cursor/mcp.json`).
Start at `chaos://authoring`. The `chaos://catalog`,
`chaos://guides/{workflows|webhooks|integrations}`, and
`chaos://schemas/{workflow-spec|triggers|queue|integrations}` resources provide
progressive authoring context. These `v1 stored_config` schemas are permissive
mirrors that preserve additive fields at the MCP validation boundary; the
Rust/backend contract and persistence normalization remain authoritative, and
the current backend may drop unsupported fields.

The agent then uses tools like `register_workflow`, `patch_workflow_spec`,
`update_workflow`, `enqueue_workflow`, `rerun_workflow`, `get_run`,
`get_run_logs`, `get_run_tasks`, `get_run_metrics`, `list_queues`,
`list_email_profiles` /
`create_email_profile` / `set_workflow_email_profile`, and `chaos://` resources
— the same operations as above, but conversational.

Run detail is also available selectively through `chaos://runs/{id}/tasks` and
`chaos://runs/{id}/metrics`. Workflow-ID resource templates provide
prefix-filtered deterministic completions with MCP's native 100-value response
cap and truthful pagination metadata, while listing templates remains a static,
backend-free operation. For failure triage, start with the run summary/logs,
load tasks or metrics only when they can explain the failure, and use
`rerun_workflow` with the failed source run for a faithful retry only after
explicit operator confirmation.

MCP workflow/environment writes are **fail-closed** on protected environments
(lookup errors block writes; `update_workflow` checks `patch.environment`;
workflow email-profile assignment resolves the workflow first). Global
email-profile CRUD has no environment target and relies on API scope/auth. A
shared in-process tool-call budget caps runaway loops. Details in the
[mcp-server README guardrails section](./mcp-server/README.md#guardrails).

## 8. Update or rerun a workflow (write)

```ts
await client.updateWorkflow(wf.id, {
  enabled: true,
  cron_schedule: "0 7 * * *",
});

const rerun = await client.rerunWorkflow(wf.id, {
  sourceRunId: failedRunId,
  idempotencyKey: crypto.randomUUID(),
});
```

`updateWorkflow` maps to `PATCH /api/v1/workflows/{id}`; `rerunWorkflow` maps to
`POST /api/v1/workflows/{id}/rerun` and supports the same idempotent-replay shape
as run/enqueue.

For MCP spec changes, read `chaos://workflows/{id}/definition` and prefer
`patch_workflow_spec`. It applies RFC 7396 JSON Merge Patch to the full stored
spec internally, preserves omitted fields and `__redacted__` secret sentinels,
and returns only the redacted stored definition. `set_workflow_spec` remains the
intentional full-replacement tool. This is a read-merge-write flow, so serialize
concurrent spec writers until the backend exposes compare-and-swap semantics. A
sentinel inside a replaced array requires a unique `id` or unchanged webhook
URL; ambiguous edits fail closed, and identity changes require the real secret.

## 9. Email delivery profiles (CRUD + selection)

Named, reusable SMTP profiles let a workflow send failure alerts through a
specific mailbox instead of the single global email config. The same CRUD +
selection surface the desktop app uses is exposed over REST, the SDK, and MCP.

```ts
const profile = await client.createEmailProfile({
  name: "Ops mailbox",
  enabled: true,
  alert_email: "ops@example.com",
  smtp_host: "smtp.example.com",
  smtp_port: 587,
  smtp_user: "mailer",
  smtp_password: "app-password",
  from_address: "alerts@example.com",
  from_name: "Chaos Scheduler",
});

// Point a workflow at it (pass null to clear and fall back to the global config).
await client.setWorkflowEmailProfile(wf.id, profile.id);
```

| SDK method                               | HTTP                                        | Scope |
| ---------------------------------------- | ------------------------------------------- | ----- |
| `listEmailProfiles()`                    | `GET /api/v1/email-profiles`                | read  |
| `createEmailProfile(input)`              | `POST /api/v1/email-profiles`               | write |
| `updateEmailProfile(id, input)`          | `PATCH /api/v1/email-profiles/{id}`         | write |
| `deleteEmailProfile(id)`                 | `DELETE /api/v1/email-profiles/{id}`        | write |
| `setWorkflowEmailProfile(id, profileId)` | `POST /api/v1/workflows/{id}/email-profile` | write |

**Password masking** — `smtp_password` is always returned as the mask
`••••••••` (exported as `MASKED_SECRET`). Echo the mask back on update to keep
the stored password unchanged, or send a new value to replace it. A blank
password means none is stored. The MCP tools (`list_email_profiles`,
`create_email_profile`, `update_email_profile`, `delete_email_profile`,
`set_workflow_email_profile`) and the `chaos://email-profiles` resource wrap the
same calls. Workflow profile assignment uses the protected-environment
guardrail; global profile CRUD is environment-agnostic and relies on API
scope/auth.

## Error handling

Non-2xx responses throw `ChaosApiError` with `.status` and helpers
`.isAuthError` (401/403), `.isRateLimited` (429), `.isNotFound` (404). The API
applies a per-key rate limit and a 256 KiB request-body limit.

## Compatibility

Use `GET /version` (`client.getVersion()`) to check the product version, DB
schema version, and API version (`v1`) before relying on newer behavior.
