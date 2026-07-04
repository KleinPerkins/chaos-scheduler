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
          environment: "instance",
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
      environment: "instance",
    });
    expect(wf.managed_externally).toBe(true);
    const sent = JSON.parse(calls[0]!.body!);
    expect(sent.name).toBe("API WF");
    expect(sent.environment).toBe("instance");
    expect(calls[0]!.headers?.["content-type"]).toBe("application/json");
  });

  it("attaches the Idempotency-Key header on run", async () => {
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: {
        workflow_id: "w1",
        status: "admitted",
        run_id: "r1",
        queue_name: "instance-default",
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
    const { fetch, calls } = fakeFetch(() => ({
      status: 200,
      json: {
        workflow_id: "w1",
        status: "queued",
        run_id: null,
        queue_name: "instance-default",
      },
    }));
    const client = new ChaosSchedulerClient({
      baseUrl: BASE,
      apiKey: "id.secret",
      fetch,
    });
    await client.dispatchWorkflow("w1", {
      payload: '{"a":1}',
      signatureSecret: "topsecret",
    });
    expect(calls[0]!.body).toBe('{"a":1}');
    expect(calls[0]!.headers?.["x-chaos-signature"]).toBe(
      "sha256=bf1e6501b7fa928ec2391fea9dd90af3c9ad1b7b1ef6ff319c25940cec746bf8",
    );
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
});
