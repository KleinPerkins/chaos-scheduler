/**
 * Typed client for the Chaos Scheduler REST API (`/api/v1`).
 *
 * Mirrors the exact contract implemented in `src-tauri/src/api.rs`:
 *   - Auth: `Authorization: Bearer <id>.<secret>` (scoped API key).
 *   - Idempotency: `Idempotency-Key` header on run/enqueue/dispatch.
 *   - Errors: non-2xx render as `{ "error": "<message>" }`.
 *   - Response envelopes: `{ environments }`, `{ workflows }`, `{ workflow }`,
 *     `{ runs }`, `{ run }`, `{ deleted }`; dispatch returns a bare
 *     `DispatchOutcome` (or `{ status: "duplicate", run_id, queued_run_id }` on replay).
 */

import { ChaosApiError } from "./errors.js";
import type {
  CreateEnvironmentInput,
  DispatchResult,
  EmailProfile,
  EmailProfileInput,
  Environment,
  QueueInfo,
  QueuedRun,
  RegisterWorkflowInput,
  RerunWorkflowOptions,
  Run,
  UpdateWorkflowInput,
  RunLogs,
  RunMetric,
  RunTasksResult,
  VersionInfo,
  Workflow,
  WorkflowSpec,
} from "./types.js";

/** Minimal fetch signature so tests can inject a stub. */
export type FetchLike = (
  input: string,
  init?: {
    method?: string;
    headers?: Record<string, string>;
    body?: string;
    signal?: AbortSignal;
  },
) => Promise<{
  ok: boolean;
  status: number;
  text: () => Promise<string>;
}>;

export interface ChaosSchedulerClientOptions {
  /** Base URL of the running scheduler, e.g. `http://127.0.0.1:9618`. */
  baseUrl: string;
  /** Scoped API-key token (`<id>.<secret>`). Optional for health/version. */
  apiKey?: string;
  /** Injectable fetch (defaults to global `fetch`). */
  fetch?: FetchLike;
  /** Per-request timeout in milliseconds (default 30000). */
  timeoutMs?: number;
  /** Extra headers merged into every request. */
  defaultHeaders?: Record<string, string>;
}

export interface DispatchOptions {
  /** Value for the `Idempotency-Key` header (safe replay). */
  idempotencyKey?: string;
}

export interface InboundDispatchOptions extends DispatchOptions {
  /** Raw request body forwarded to the workflow's webhook trigger. */
  payload?: string;
  /**
   * Value for the `X-Chaos-Signature` header (canonical inbound scheme).
   * If `signatureSecret` is provided, the client computes canonical HMAC
   * (`METHOD\nPATH\nTIMESTAMP\nSHA256(body)`) and sets timestamp/event-id headers.
   */
  signature?: string;
  /** Shared secret used to sign the inbound dispatch (canonical scheme). */
  signatureSecret?: string;
  /** Unix seconds for `X-Chaos-Timestamp` (default: now). */
  timestamp?: string;
  /** Value for `X-Chaos-Event-Id` (default: random UUID). */
  eventId?: string;
}

/** Run statuses treated as terminal by {@link ChaosSchedulerClient.waitForRun}. */
export const TERMINAL_RUN_STATUSES: ReadonlySet<string> = new Set([
  "success",
  "succeeded",
  "completed",
  "failed",
  "error",
  "cancelled",
  "canceled",
  "timeout",
  "skipped",
]);

export interface WaitForRunOptions {
  /** Poll interval in ms (default 2000). */
  intervalMs?: number;
  /** Overall timeout in ms (default 300000). */
  timeoutMs?: number;
  /** Predicate deciding whether a run is terminal (default: status set). */
  isDone?: (run: Run) => boolean;
  /** Abort signal to cancel polling. */
  signal?: AbortSignal;
}

function trimTrailingSlash(url: string): string {
  return url.replace(/\/+$/, "");
}

async function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) return reject(new Error("aborted"));
    const t = setTimeout(resolve, ms);
    signal?.addEventListener(
      "abort",
      () => {
        clearTimeout(t);
        reject(new Error("aborted"));
      },
      { once: true },
    );
  });
}

export class ChaosSchedulerClient {
  private readonly baseUrl: string;
  private readonly apiKey?: string;
  private readonly fetchImpl: FetchLike;
  private readonly timeoutMs: number;
  private readonly defaultHeaders: Record<string, string>;

  constructor(options: ChaosSchedulerClientOptions) {
    if (!options.baseUrl)
      throw new Error("ChaosSchedulerClient requires a baseUrl");
    this.baseUrl = trimTrailingSlash(options.baseUrl);
    this.apiKey = options.apiKey;
    const globalFetch = (globalThis as { fetch?: FetchLike }).fetch;
    const resolved = options.fetch ?? globalFetch;
    if (!resolved) {
      throw new Error(
        "No fetch implementation available; pass `fetch` in options (Node < 18) ",
      );
    }
    this.fetchImpl = resolved;
    this.timeoutMs = options.timeoutMs ?? 30_000;
    this.defaultHeaders = options.defaultHeaders ?? {};
  }

  // --- Unauthenticated ---

  /** `GET /api/v1/health` — liveness probe (no auth required). */
  async getHealth(): Promise<{ status: string }> {
    return this.request<{ status: string }>("GET", "/api/v1/health", {
      auth: false,
    });
  }

  /** `GET /api/v1/version` — product/version/schema info (no auth required). */
  async getVersion(): Promise<VersionInfo> {
    return this.request<VersionInfo>("GET", "/api/v1/version", { auth: false });
  }

  // --- Environments (read | write) ---

  /** `GET /api/v1/environments` — list environments (scope: read). */
  async listEnvironments(): Promise<Environment[]> {
    const res = await this.request<{ environments: Environment[] }>(
      "GET",
      "/api/v1/environments",
    );
    return res.environments;
  }

  /** `POST /api/v1/environments` — create an environment (scope: write). */
  async createEnvironment(input: CreateEnvironmentInput): Promise<Environment> {
    const res = await this.request<{ environment: Environment }>(
      "POST",
      "/api/v1/environments",
      { body: input },
    );
    return res.environment;
  }

  // --- Workflows (read | write) ---

  /** `GET /api/v1/workflows` — list workflows (scope: read). */
  async listWorkflows(): Promise<Workflow[]> {
    const res = await this.request<{ workflows: Workflow[] }>(
      "GET",
      "/api/v1/workflows",
    );
    return res.workflows;
  }

  /** `GET /api/v1/workflows/{id}` — fetch a workflow (scope: read). */
  async getWorkflow(id: string): Promise<Workflow> {
    const res = await this.request<{ workflow: Workflow }>(
      "GET",
      `/api/v1/workflows/${encodeURIComponent(id)}`,
    );
    return res.workflow;
  }

  /**
   * `POST /api/v1/workflows` — register a workflow (scope: write).
   * API-registered workflows are marked `managed_externally=true`. If `spec` is
   * provided it is applied via the spec endpoint after creation.
   */
  async registerWorkflow(input: RegisterWorkflowInput): Promise<Workflow> {
    const res = await this.request<{ workflow: Workflow }>(
      "POST",
      "/api/v1/workflows",
      {
        body: input,
      },
    );
    return res.workflow;
  }

  /**
   * `PATCH /api/v1/workflows/{id}` — update workflow metadata/runtime prefs (scope: write).
   * API callers may edit externally-managed definitions they registered.
   */
  async updateWorkflow(
    id: string,
    input: UpdateWorkflowInput,
  ): Promise<Workflow> {
    const res = await this.request<{ workflow: Workflow }>(
      "PATCH",
      `/api/v1/workflows/${encodeURIComponent(id)}`,
      { body: input },
    );
    return res.workflow;
  }

  /**
   * `POST /api/v1/workflows/{id}/rerun` — rerun from a prior run (scope: write).
   * Supports `idempotencyKey` for safe retries.
   */
  async rerunWorkflow(
    id: string,
    options: RerunWorkflowOptions = {},
  ): Promise<DispatchResult> {
    const body: Record<string, unknown> = {};
    if (options.sourceRunId) body.source_run_id = options.sourceRunId;
    if (options.inputOverride !== undefined) {
      body.input_override = options.inputOverride;
    }
    return this.request<DispatchResult>(
      "POST",
      `/api/v1/workflows/${encodeURIComponent(id)}/rerun`,
      { body, idempotencyKey: options.idempotencyKey },
    );
  }

  /** `DELETE /api/v1/workflows/{id}` — deregister a workflow (scope: write). */
  async deleteWorkflow(id: string): Promise<{ deleted: string }> {
    return this.request<{ deleted: string }>(
      "DELETE",
      `/api/v1/workflows/${encodeURIComponent(id)}`,
    );
  }

  /** `POST /api/v1/workflows/{id}/spec` — set the execution spec (scope: write). */
  async setWorkflowSpec(id: string, spec: WorkflowSpec): Promise<Workflow> {
    const res = await this.request<{ workflow: Workflow }>(
      "POST",
      `/api/v1/workflows/${encodeURIComponent(id)}/spec`,
      { body: spec },
    );
    return res.workflow;
  }

  // --- Dispatch (write) ---

  /**
   * `POST /api/v1/workflows/{id}/run` — dispatch a run now (scope: write).
   * Supply `idempotencyKey` for safe retries; a reused key returns
   * `{ status: "duplicate", run_id }`.
   */
  async runWorkflow(
    id: string,
    options: DispatchOptions = {},
  ): Promise<DispatchResult> {
    return this.request<DispatchResult>(
      "POST",
      `/api/v1/workflows/${encodeURIComponent(id)}/run`,
      { idempotencyKey: options.idempotencyKey },
    );
  }

  /** `POST /api/v1/workflows/{id}/enqueue` — queue a run (scope: write). */
  async enqueueWorkflow(
    id: string,
    options: DispatchOptions = {},
  ): Promise<DispatchResult> {
    return this.request<DispatchResult>(
      "POST",
      `/api/v1/workflows/${encodeURIComponent(id)}/enqueue`,
      { idempotencyKey: options.idempotencyKey },
    );
  }

  /**
   * `POST /api/v1/workflows/{id}/dispatch` — inbound webhook trigger (scope:
   * write). Sends the raw `payload` as the request body; if the workflow's
   * inbound secret is configured, provide `signature` or `signatureSecret` to
   * set canonical `X-Chaos-Timestamp`, `X-Chaos-Event-Id`, and `X-Chaos-Signature`.
   */
  async dispatchWorkflow(
    id: string,
    options: InboundDispatchOptions = {},
  ): Promise<DispatchResult> {
    const payload = options.payload ?? "";
    const path = `/api/v1/workflows/${encodeURIComponent(id)}/dispatch`;
    const headers: Record<string, string> = {};
    if (options.signature) {
      headers["x-chaos-signature"] = options.signature;
      if (options.timestamp) {
        headers["x-chaos-timestamp"] = options.timestamp;
      }
      if (options.eventId) {
        headers["x-chaos-event-id"] = options.eventId;
      }
    } else if (options.signatureSecret) {
      const { inboundDispatchHeaders } = await import("./webhook.js");
      Object.assign(
        headers,
        inboundDispatchHeaders({
          path,
          body: payload,
          secret: options.signatureSecret,
          timestamp: options.timestamp,
          eventId: options.eventId,
        }),
      );
    }
    return this.request<DispatchResult>("POST", path, {
      rawBody: payload,
      headers,
      idempotencyKey: options.idempotencyKey,
    });
  }

  // --- Runs (read) ---

  /** `GET /api/v1/workflows/{id}/runs` — recent run history (scope: read). */
  async listRuns(id: string): Promise<Run[]> {
    const res = await this.request<{ runs: Run[] }>(
      "GET",
      `/api/v1/workflows/${encodeURIComponent(id)}/runs`,
    );
    return res.runs;
  }

  /** `GET /api/v1/runs/{id}` — fetch a single run (scope: read). */
  async getRun(id: string): Promise<Run> {
    const res = await this.request<{ run: Run }>(
      "GET",
      `/api/v1/runs/${encodeURIComponent(id)}`,
    );
    return res.run;
  }

  /** `GET /api/v1/runs/{id}/logs` — stdout/stderr/exit for a run (scope: read). */
  async getRunLogs(id: string): Promise<RunLogs> {
    return this.request<RunLogs>(
      "GET",
      `/api/v1/runs/${encodeURIComponent(id)}/logs`,
    );
  }

  /** `GET /api/v1/runs/{id}/tasks` — step tasks + retry attempts (scope: read). */
  async getRunTasks(id: string): Promise<RunTasksResult> {
    return this.request<RunTasksResult>(
      "GET",
      `/api/v1/runs/${encodeURIComponent(id)}/tasks`,
    );
  }

  /** `GET /api/v1/runs/{id}/metrics` — emitted metric samples (scope: read). */
  async getRunMetrics(id: string): Promise<RunMetric[]> {
    const res = await this.request<{ metrics: RunMetric[] }>(
      "GET",
      `/api/v1/runs/${encodeURIComponent(id)}/metrics`,
    );
    return res.metrics;
  }

  /** `GET /api/v1/queues` — queue capacity snapshots (scope: read). */
  async listQueues(): Promise<QueueInfo[]> {
    const res = await this.request<{ queues: QueueInfo[] }>(
      "GET",
      "/api/v1/queues",
    );
    return res.queues;
  }

  /** `GET /api/v1/queued-runs` — durable queued runs (scope: read). */
  async listQueuedRuns(): Promise<QueuedRun[]> {
    const res = await this.request<{ queued_runs: QueuedRun[] }>(
      "GET",
      "/api/v1/queued-runs",
    );
    return res.queued_runs;
  }

  /**
   * `GET /api/v1/email-profiles` — list named SMTP delivery profiles
   * (scope: read). Each profile's `smtp_password` is masked.
   */
  async listEmailProfiles(): Promise<EmailProfile[]> {
    const res = await this.request<{ email_profiles: EmailProfile[] }>(
      "GET",
      "/api/v1/email-profiles",
    );
    return res.email_profiles;
  }

  /**
   * `POST /api/v1/email-profiles` — create a new email profile (scope: write).
   * The server assigns the id. Returns the created profile (password masked).
   */
  async createEmailProfile(input: EmailProfileInput): Promise<EmailProfile> {
    const res = await this.request<{ email_profile: EmailProfile }>(
      "POST",
      "/api/v1/email-profiles",
      { body: input },
    );
    return res.email_profile;
  }

  /**
   * `PATCH /api/v1/email-profiles/{id}` — update an existing profile
   * (scope: write). Echo the masked password (`MASKED_SECRET`) to keep the
   * stored secret, or pass a new value to replace it. Returns the updated
   * profile (password masked).
   */
  async updateEmailProfile(
    id: string,
    input: EmailProfileInput,
  ): Promise<EmailProfile> {
    const res = await this.request<{ email_profile: EmailProfile }>(
      "PATCH",
      `/api/v1/email-profiles/${encodeURIComponent(id)}`,
      { body: input },
    );
    return res.email_profile;
  }

  /** `DELETE /api/v1/email-profiles/{id}` — delete a profile (scope: write). */
  async deleteEmailProfile(id: string): Promise<{ deleted: string }> {
    return this.request<{ deleted: string }>(
      "DELETE",
      `/api/v1/email-profiles/${encodeURIComponent(id)}`,
    );
  }

  /**
   * `POST /api/v1/workflows/{id}/email-profile` — select (or clear, with
   * `null`) the email profile a workflow uses for failure alerts (scope:
   * write).
   */
  async setWorkflowEmailProfile(
    workflowId: string,
    profileId: string | null,
  ): Promise<{ workflow_id: string; email_profile_id: string | null }> {
    return this.request<{
      workflow_id: string;
      email_profile_id: string | null;
    }>(
      "POST",
      `/api/v1/workflows/${encodeURIComponent(workflowId)}/email-profile`,
      { body: { profile_id: profileId } },
    );
  }

  /**
   * Poll `getRun` until the run reaches a terminal status (or timeout).
   * Convenience over the read endpoints; the backend has no long-poll.
   */
  async waitForRun(
    runId: string,
    options: WaitForRunOptions = {},
  ): Promise<Run> {
    const interval = options.intervalMs ?? 2_000;
    const timeout = options.timeoutMs ?? 300_000;
    const isDone =
      options.isDone ??
      ((run: Run) => TERMINAL_RUN_STATUSES.has(run.status.toLowerCase()));
    const deadline = Date.now() + timeout;
    for (;;) {
      const run = await this.getRun(runId);
      if (isDone(run)) return run;
      if (Date.now() + interval > deadline) {
        throw new Error(
          `waitForRun timed out after ${timeout}ms (run ${runId})`,
        );
      }
      await sleep(interval, options.signal);
    }
  }

  // --- Internals ---

  private async request<T>(
    method: string,
    path: string,
    opts: {
      body?: unknown;
      rawBody?: string;
      headers?: Record<string, string>;
      idempotencyKey?: string;
      auth?: boolean;
    } = {},
  ): Promise<T> {
    const url = `${this.baseUrl}${path}`;
    const headers: Record<string, string> = {
      accept: "application/json",
      ...this.defaultHeaders,
      ...(opts.headers ?? {}),
    };
    const requireAuth = opts.auth !== false;
    if (requireAuth) {
      if (!this.apiKey) {
        throw new ChaosApiError({
          status: 401,
          url,
          method,
          message: `An API key is required to call ${method} ${path}`,
        });
      }
      headers["authorization"] = `Bearer ${this.apiKey}`;
    }
    if (opts.idempotencyKey) headers["idempotency-key"] = opts.idempotencyKey;

    let body: string | undefined;
    if (opts.rawBody !== undefined) {
      body = opts.rawBody;
    } else if (opts.body !== undefined) {
      body = JSON.stringify(opts.body);
      headers["content-type"] = "application/json";
    }

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);
    let raw: { ok: boolean; status: number; text: () => Promise<string> };
    try {
      raw = await this.fetchImpl(url, {
        method,
        headers,
        body,
        signal: controller.signal,
      });
    } catch (err) {
      clearTimeout(timer);
      throw new ChaosApiError({
        status: 0,
        url,
        method,
        message:
          err instanceof Error
            ? `network error calling ${method} ${path}: ${err.message}`
            : `network error calling ${method} ${path}`,
      });
    }
    clearTimeout(timer);

    const text = await raw.text();
    let parsed: unknown = undefined;
    if (text.length > 0) {
      try {
        parsed = JSON.parse(text);
      } catch {
        parsed = text;
      }
    }

    if (!raw.ok) {
      const message =
        (parsed && typeof parsed === "object" && "error" in parsed
          ? String((parsed as { error: unknown }).error)
          : undefined) ?? `${method} ${path} failed with status ${raw.status}`;
      throw new ChaosApiError({
        status: raw.status,
        url,
        method,
        message,
        body: parsed,
      });
    }

    return parsed as T;
  }
}
