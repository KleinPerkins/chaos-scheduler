import { describe, expect, it, vi } from "vitest";
import { ChaosSchedulerClient, type FetchLike } from "../src/client.js";
import { ChaosApiError } from "../src/errors.js";
import { isDuplicateDispatch } from "../src/types.js";

interface Captured {
  url: string;
  method?: string;
  headers?: Record<string, string>;
  body?: string;
}

/** Build a fake fetch returning a fixed JSON body + status, capturing requests. */
function fakeFetch(
  responder: (
    req: Captured,
  ) => { status: number; json: unknown } | { status: number; text: string },
): { fetch: FetchLike; calls: Captured[] } {
  const calls: Captured[] = [];
  const fetch: FetchLike = async (url, init) => {
    const req: Captured = {
      url,
      method: init?.method,
      headers: init?.headers,
      body: init?.body,
    };
    calls.push(req);
    const r = responder(req);
    const text = "json" in r ? JSON.stringify(r.json) : r.text;
    return {
      ok: r.status >= 200 && r.status < 300,
      status: r.status,
      text: async () => text,
    };
  };
  return { fetch, calls };
}

const BASE = "http://127.0.0.1:9618";

describe("ChaosSchedulerClient", () => {
  it("sends the bearer token and unwraps the workflows envelope", async () => {
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: { workflows: [{ id: "w1", name: "A" }] },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    const workflows = await client.listWorkflows();
    expect(workflows).toHaveLength(1);
    expect(workflows[0]!.id).toBe("w1");
    expect(calls[0]!.url).toBe(`${BASE}/api/v1/workflows`);
    expect(calls[0]!.headers?.["authorization"]).toBe("Bearer id.secret");
  });

  it("does not send auth for health/version", async () => {
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: { status: "ok" },
    }));
    const client = new ChaosSchedulerClient({ baseUrl: BASE, fetch });
    await client.getHealth();
    expect(calls[0]!.headers?.["authorization"]).toBeUndefined();
  });

  it("throws ChaosApiError (with status + message) on non-2xx", async () => {
    const { fetch } = fakeFetch(() => ({
      status: 403,
      json: { error: "API key lacks scope 'write'" },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    await expect(
      client.createEnvironment({ name: "prod" }),
    ).rejects.toMatchObject({
      name: "ChaosApiError",
      status: 403,
      message: "API key lacks scope 'write'",
    });
    try {
      await client.createEnvironment({ name: "prod" });
    } catch (e) {
      expect(e).toBeInstanceOf(ChaosApiError);
      expect((e as ChaosApiError).isAuthError).toBe(true);
    }
  });

  it("throws locally (401) when an authed call has no api key", async () => {
    const { fetch, calls } = fakeFetch(() => ({ status: 200, json: {} }));
    const client = new ChaosSchedulerClient({ baseUrl: BASE, fetch });
    await expect(client.listWorkflows()).rejects.toMatchObject({ status: 401 });
    expect(calls).toHaveLength(0); // never hit the network
  });

  it("serializes the register body and passes environment through", async () => {
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: {
        workflow: {
          id: "w2",
          environment: "production",
          managed_externally: true,
        },
      },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    const wf = await client.registerWorkflow({
      name: "API WF",
      script_path: "scripts/x.py",
      cron_schedule: "0 0 * * *",
      environment: "production",
    });
    expect(wf.managed_externally).toBe(true);
    const sent = JSON.parse(calls[0]!.body!);
    expect(sent.name).toBe("API WF");
    expect(sent.environment).toBe("production");
    expect(calls[0]!.headers?.["content-type"]).toBe("application/json");
  });

  it("attaches the Idempotency-Key header on run", async () => {
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: {
        workflow_id: "w1",
        status: "admitted",
        run_id: "r1",
        queue_name: "production-default",
      },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    const res = await client.runWorkflow("w1", { idempotencyKey: "abc-123" });
    expect(calls[0]!.headers?.["idempotency-key"]).toBe("abc-123");
    expect(isDuplicateDispatch(res)).toBe(false);
    if (!isDuplicateDispatch(res)) expect(res.status).toBe("admitted");
  });

  it("recognizes the duplicate-dispatch replay shape", async () => {
    const { fetch } = fakeFetch(() => ({
      status: 200,
      json: { status: "duplicate", run_id: "r1" },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    const res = await client.enqueueWorkflow("w1", {
      idempotencyKey: "abc-123",
    });
    expect(isDuplicateDispatch(res)).toBe(true);
  });

  it("signs the inbound dispatch payload from a secret", async () => {
    const { computeInboundDispatchSignature } =
      await import("../src/webhook.js");
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: {
        workflow_id: "w1",
        status: "queued",
        run_id: null,
        queue_name: "production-default",
      },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    const path = "/api/v1/workflows/w1/dispatch";
    const timestamp = "1700000000";
    const eventId = "evt-test";
    await client.dispatchWorkflow("w1", {
      payload: '{"a":1}',
      signatureSecret: "topsecret",
      timestamp,
      eventId,
    });
    expect(calls[0]!.body).toBe('{"a":1}');
    const expected = computeInboundDispatchSignature(
      "POST",
      path,
      timestamp,
      '{"a":1}',
      "topsecret",
    );
    expect(calls[0]!.headers?.["x-chaos-signature"]).toBe(`sha256=${expected}`);
    expect(calls[0]!.headers?.["x-chaos-timestamp"]).toBe(timestamp);
    expect(calls[0]!.headers?.["x-chaos-event-id"]).toBe(eventId);
  });

  it("polls waitForRun until a terminal status", async () => {
    vi.useFakeTimers();
    const statuses = ["running", "running", "success"];
    let i = 0;
    const { fetch } = fakeFetch(() => ({
      status: 200,
      json: {
        run: {
          id: "r1",
          workflow_id: "w1",
          status: statuses[i++] ?? "success",
        },
      },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    const promise = client.waitForRun("r1", {
      intervalMs: 10,
      timeoutMs: 10_000,
    });
    await vi.runAllTimersAsync();
    const run = await promise;
    expect(run.status).toBe("success");
    vi.useRealTimers();
  });

  it("calls read endpoints with the expected paths", async () => {
    const { fetch, calls } = fakeFetch((req) => {
      if (req.url.endsWith("/logs")) {
        return {
          status: 200,
          json: {
            run_id: "r1",
            status: "success",
            exit_code: 0,
            stdout: "out",
            stderr: "",
            result_url: null,
          },
        };
      }
      if (req.url.endsWith("/tasks")) {
        return { status: 200, json: { tasks: [], attempts: [] } };
      }
      if (req.url.endsWith("/metrics")) {
        return { status: 200, json: { metrics: [] } };
      }
      if (req.url.endsWith("/queues")) {
        return { status: 200, json: { queues: [] } };
      }
      if (req.url.endsWith("/queued-runs")) {
        return { status: 200, json: { queued_runs: [] } };
      }
      return { status: 404, json: { error: "missing" } };
    });
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    await client.getRunLogs("r1");
    await client.getRunTasks("r1");
    await client.getRunMetrics("r1");
    await client.listQueues();
    await client.listQueuedRuns();
    expect(calls.map((c) => c.url)).toEqual([
      `${BASE}/api/v1/runs/r1/logs`,
      `${BASE}/api/v1/runs/r1/tasks`,
      `${BASE}/api/v1/runs/r1/metrics`,
      `${BASE}/api/v1/queues`,
      `${BASE}/api/v1/queued-runs`,
    ]);
  });
  it("patches a workflow via PATCH", async () => {
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: {
        workflow: { id: "w1", enabled: false, cron_schedule: "0 1 * * *" },
      },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    const wf = await client.updateWorkflow("w1", {
      enabled: false,
      cron_schedule: "0 1 * * *",
    });
    expect(wf.enabled).toBe(false);
    expect(calls[0]!.method).toBe("PATCH");
    expect(calls[0]!.url).toBe(`${BASE}/api/v1/workflows/w1`);
  });

  it("lists and creates email profiles, unwrapping the envelopes", async () => {
    const masked = {
      id: "ep1",
      name: "Primary",
      enabled: true,
      alert_email: "a@e.com",
      smtp_host: "smtp.e.com",
      smtp_port: 587,
      smtp_user: "u",
      smtp_password: "••••••••",
      from_address: "f@e.com",
      from_name: "N",
      created_at: "",
      updated_at: "",
    };
    const { fetch, calls } = fakeFetch((req) => {
      if (req.method === "POST") {
        return {
          status: 200,
          json: { email_profile: { ...masked, id: "ep2" } },
        };
      }
      return { status: 200, json: { email_profiles: [masked] } };
    });
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });

    const listed = await client.listEmailProfiles();
    expect(listed).toHaveLength(1);
    expect(listed[0]!.smtp_password).toBe("••••••••");
    expect(calls[0]!.url).toBe(`${BASE}/api/v1/email-profiles`);

    const created = await client.createEmailProfile({
      name: "Primary",
      enabled: true,
      alert_email: "a@e.com",
      smtp_host: "smtp.e.com",
      smtp_port: 587,
      smtp_user: "u",
      smtp_password: "realpw",
      from_address: "f@e.com",
      from_name: "N",
    });
    expect(created.id).toBe("ep2");
    expect(calls[1]!.method).toBe("POST");
    expect(JSON.parse(calls[1]!.body!).smtp_password).toBe("realpw");
  });

  it("updates and deletes an email profile and selects it onto a workflow", async () => {
    const { fetch, calls } = fakeFetch((req) => {
      if (req.method === "DELETE")
        return { status: 200, json: { deleted: "ep1" } };
      if (req.url.endsWith("/email-profile")) {
        return {
          status: 200,
          json: { workflow_id: "w1", email_profile_id: "ep1" },
        };
      }
      return {
        status: 200,
        json: {
          email_profile: {
            id: "ep1",
            name: "Renamed",
            smtp_password: "••••••••",
          },
        },
      };
    });
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });

    await client.updateEmailProfile("ep1", {
      name: "Renamed",
      enabled: true,
      alert_email: "a@e.com",
      smtp_host: "smtp.e.com",
      smtp_port: 587,
      smtp_user: "u",
      smtp_password: "••••••••",
      from_address: "f@e.com",
      from_name: "N",
    });
    expect(calls[0]!.method).toBe("PATCH");
    expect(calls[0]!.url).toBe(`${BASE}/api/v1/email-profiles/ep1`);

    const sel = await client.setWorkflowEmailProfile("w1", "ep1");
    expect(sel.email_profile_id).toBe("ep1");
    expect(calls[1]!.method).toBe("POST");
    expect(calls[1]!.url).toBe(`${BASE}/api/v1/workflows/w1/email-profile`);
    expect(JSON.parse(calls[1]!.body!).profile_id).toBe("ep1");

    const del = await client.deleteEmailProfile("ep1");
    expect(del.deleted).toBe("ep1");
    expect(calls[2]!.method).toBe("DELETE");
  });

  it("reruns a workflow with source run and idempotency key", async () => {
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: {
        workflow_id: "w1",
        status: "admitted",
        run_id: "r2",
        queued_run_id: null,
        queue_name: "production-default",
      },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    const res = await client.rerunWorkflow("w1", {
      sourceRunId: "r1",
      idempotencyKey: "rerun-1",
    });
    expect(calls[0]!.method).toBe("POST");
    expect(calls[0]!.url).toBe(`${BASE}/api/v1/workflows/w1/rerun`);
    expect(calls[0]!.headers?.["idempotency-key"]).toBe("rerun-1");
    if (!isDuplicateDispatch(res)) expect(res.run_id).toBe("r2");
  });

  it("waitForRun throws when timeout elapses before terminal status", async () => {
    vi.useFakeTimers();
    const { fetch } = fakeFetch(() => ({
      status: 200,
      json: { run: { id: "r1", workflow_id: "w1", status: "running" } },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    const promise = client.waitForRun("r1", {
      intervalMs: 100,
      timeoutMs: 250,
    });
    const assertion = expect(promise).rejects.toThrow(/timed out after 250ms/);
    await vi.advanceTimersByTimeAsync(300);
    await assertion;
    vi.useRealTimers();
  });

  it("trims one or more trailing slashes from baseUrl", async () => {
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: { status: "ok" },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: `${BASE}///`,
      fetch,
    });
    await client.getHealth();
    expect(calls[0]!.url).toBe(`${BASE}/api/v1/health`);
  });

  it("handles a long run of trailing slashes correctly (regression for ReDoS)", async () => {
    // Deterministic correctness check standing in for manual timing evidence
    // (see PR description): a backtracking regex here would make this test
    // hang rather than fail, so completion itself is the signal.
    const longSlashes = "/".repeat(50_000);
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: { status: "ok" },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: `${BASE}${longSlashes}`,
      fetch,
    });
    await client.getHealth();
    expect(calls[0]!.url).toBe(`${BASE}/api/v1/health`);
  });

  it("leaves a baseUrl with no trailing slash unchanged", async () => {
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: { status: "ok" },
    }));
    const client = new ChaosSchedulerClient({ baseUrl: BASE, fetch });
    await client.getHealth();
    expect(calls[0]!.url).toBe(`${BASE}/api/v1/health`);
  });

  it("wraps a network failure as a ChaosApiError with status 0", async () => {
    const fetch: FetchLike = async () => {
      throw new Error("connection refused");
    };
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    await expect(client.getRun("r1")).rejects.toMatchObject({
      name: "ChaosApiError",
      status: 0,
    });
  });
});
