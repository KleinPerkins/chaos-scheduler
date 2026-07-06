import { describe, expect, it } from "vitest";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { ChaosSchedulerClient, type FetchLike } from "@chaos-scheduler/sdk";
import { configFromEnv } from "../src/config.js";
import { buildServer } from "../src/server.js";

/** Canned backend responses keyed by "METHOD path". */
function routedFetch(routes: Record<string, unknown>): FetchLike {
  return async (url, init) => {
    const path = url.replace("http://127.0.0.1:9618", "");
    const key = `${init?.method ?? "GET"} ${path}`;
    const body = routes[key];
    if (body === undefined) {
      return {
        ok: false,
        status: 404,
        text: async () => JSON.stringify({ error: `no route ${key}` }),
      };
    }
    return { ok: true, status: 200, text: async () => JSON.stringify(body) };
  };
}

const ROUTES: Record<string, unknown> = {
  "GET /api/v1/version": {
    product: "Chaos Scheduler",
    version: "0.1.0",
    schema_version: 4,
    api: "v1",
  },
  "GET /api/v1/environments": {
    environments: [{ id: "e1", name: "instance" }],
  },
  "GET /api/v1/workflows": {
    workflows: [{ id: "w1", name: "A", environment: "instance" }],
  },
  "GET /api/v1/workflows/w1": {
    workflow: {
      id: "w1",
      name: "A",
      environment: "instance",
    },
  },
  "POST /api/v1/workflows/w1/run": {
    workflow_id: "w1",
    status: "admitted",
    run_id: "r1",
    queued_run_id: null,
    queue_name: "instance-default",
  },
  "GET /api/v1/runs/r1": {
    run: { id: "r1", workflow_id: "w1", status: "success", exit_code: 0 },
  },
  "GET /api/v1/runs/r1/logs": {
    run_id: "r1",
    status: "success",
    exit_code: 0,
    stdout: "ok",
    stderr: "",
    result_url: null,
  },
  "GET /api/v1/runs/r1/tasks": {
    tasks: [{ id: "t1", run_id: "r1", task_id: "step1", status: "success" }],
    attempts: [],
  },
  "GET /api/v1/runs/r1/metrics": {
    metrics: [
      {
        id: "m1",
        run_id: "r1",
        task_id: null,
        metric_name: "duration_ms",
        metric_value: 42,
        metric_unit: "ms",
        emitted_at: "2026-01-01T00:00:00Z",
      },
    ],
  },
  "GET /api/v1/queues": {
    queues: [
      {
        name: "instance-default",
        environment: "instance",
        capacity: 4,
        active_count: 0,
        queued_count: 0,
        global_parallelism_cap: 8,
        updated_at: "2026-01-01T00:00:00Z",
      },
    ],
  },
  "GET /api/v1/queued-runs": {
    queued_runs: [
      {
        id: "q1",
        run_id: null,
        workflow_id: "w1",
        queue_name: "instance-default",
        environment: "instance",
        status: "queued",
        queued_at: "2026-01-01T00:00:00Z",
      },
    ],
  },
  "POST /api/v1/workflows/w1/dispatch": {
    workflow_id: "w1",
    status: "queued",
    run_id: null,
    queued_run_id: "q2",
    queue_name: "instance-default",
  },
  "PATCH /api/v1/workflows/w1": {
    workflow: {
      id: "w1",
      name: "A",
      environment: "instance",
    },
  },
  "POST /api/v1/workflows/w1/rerun": {
    workflow_id: "w1",
    status: "admitted",
    run_id: "r2",
    queued_run_id: null,
    queue_name: "instance-default",
  },
  "GET /api/v1/email-profiles": {
    email_profiles: [
      {
        id: "ep1",
        name: "Primary",
        enabled: true,
        alert_email: "alerts@example.com",
        smtp_host: "smtp.example.com",
        smtp_port: 587,
        smtp_user: "mailer",
        smtp_password: "••••••••",
        from_address: "from@example.com",
        from_name: "Chaos",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      },
    ],
  },
  "POST /api/v1/email-profiles": {
    email_profile: {
      id: "ep2",
      name: "Primary",
      enabled: true,
      alert_email: "alerts@example.com",
      smtp_host: "smtp.example.com",
      smtp_port: 587,
      smtp_user: "mailer",
      smtp_password: "••••••••",
      from_address: "from@example.com",
      from_name: "Chaos",
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
    },
  },
  "PATCH /api/v1/email-profiles/ep1": {
    email_profile: {
      id: "ep1",
      name: "Renamed",
      enabled: true,
      alert_email: "alerts@example.com",
      smtp_host: "smtp.example.com",
      smtp_port: 587,
      smtp_user: "mailer",
      smtp_password: "••••••••",
      from_address: "from@example.com",
      from_name: "Chaos",
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
    },
  },
  "DELETE /api/v1/email-profiles/ep1": { deleted: "ep1" },
  "POST /api/v1/workflows/w1/email-profile": {
    workflow_id: "w1",
    email_profile_id: "ep1",
  },
};

async function connectedPair(
  envOverrides: Record<string, string> = {},
  routes: Record<string, unknown> = ROUTES,
) {
  const config = configFromEnv({
    CHAOS_SCHEDULER_API_KEY: "id.secret",
    ...envOverrides,
  });
  const sdk = new ChaosSchedulerClient({
    baseUrl: config.baseUrl,
    apiKey: config.apiKey,
    fetch: routedFetch(routes),
  });
  const server = buildServer({ client: sdk, config });
  const [clientTransport, serverTransport] =
    InMemoryTransport.createLinkedPair();
  const client = new Client({ name: "test-client", version: "0.0.0" });
  await Promise.all([
    server.connect(serverTransport),
    client.connect(clientTransport),
  ]);
  return { client, server };
}

function textOf(result: {
  content: Array<{ type: string; text?: string }>;
}): string {
  return result.content
    .filter((c) => c.type === "text")
    .map((c) => c.text ?? "")
    .join("\n");
}

describe("Chaos MCP server", () => {
  it("registers the expected tools", async () => {
    const { client } = await connectedPair();
    const { tools } = await client.listTools();
    const names = tools.map((t) => t.name).sort();
    expect(names).toEqual(
      [
        "create_email_profile",
        "create_environment",
        "delete_email_profile",
        "delete_workflow",
        "dispatch_workflow",
        "enqueue_workflow",
        "get_run",
        "get_run_logs",
        "get_run_metrics",
        "get_run_tasks",
        "get_version",
        "get_workflow",
        "health_check",
        "list_email_profiles",
        "list_environments",
        "list_queued_runs",
        "list_queues",
        "list_workflow_runs",
        "list_workflows",
        "register_workflow",
        "run_workflow_now",
        "set_workflow_email_profile",
        "set_workflow_spec",
        "rerun_workflow",
        "update_email_profile",
        "update_workflow",
      ].sort(),
    );
  });

  it("registers resources and prompts", async () => {
    const { client } = await connectedPair();
    const { resources } = await client.listResources();
    expect(resources.map((r) => r.uri)).toContain("chaos://environments");
    expect(resources.map((r) => r.uri)).toContain("chaos://workflows");
    expect(resources.map((r) => r.uri)).toContain("chaos://email-profiles");

    const { prompts } = await client.listPrompts();
    expect(prompts.map((p) => p.name).sort()).toEqual(
      [
        "register_workflow_for_repo",
        "summarize_workflow_health",
        "triage_failed_run",
      ].sort(),
    );
  });

  it("get_version tool proxies the backend", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "get_version",
      arguments: {},
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    expect(JSON.parse(textOf(result)).product).toBe("Chaos Scheduler");
  });

  it("run_workflow_now dispatches through the SDK", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "run_workflow_now",
      arguments: { id: "w1", idempotency_key: "k1" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    expect(JSON.parse(textOf(result)).run_id).toBe("r1");
  });

  it("blocks writes to a protected environment (guardrail)", async () => {
    // Make the workflow's environment (instance) protected.
    const { client } = await connectedPair({
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "instance",
    });
    const result = (await client.callTool({
      name: "run_workflow_now",
      arguments: { id: "w1" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBe(true);
    expect(textOf(result)).toMatch(/protected/i);
  });

  it("reads the workflows resource", async () => {
    const { client } = await connectedPair();
    const res = await client.readResource({ uri: "chaos://workflows" });
    const text = (res.contents[0] as { text: string }).text;
    expect(JSON.parse(text)[0].id).toBe("w1");
  });

  it("reads a templated run resource", async () => {
    const { client } = await connectedPair();
    const res = await client.readResource({ uri: "chaos://runs/r1" });
    const text = (res.contents[0] as { text: string }).text;
    expect(JSON.parse(text).status).toBe("success");
  });

  it("get_run_logs tool proxies the backend", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "get_run_logs",
      arguments: { id: "r1" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    expect(JSON.parse(textOf(result)).stdout).toBe("ok");
  });

  it("list_queues tool proxies the backend", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "list_queues",
      arguments: {},
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    expect(JSON.parse(textOf(result))[0].name).toBe("instance-default");
  });

  it("fail-closed when getWorkflow returns 404 under active protection", async () => {
    const routes = { ...ROUTES };
    delete routes["GET /api/v1/workflows/w1"];
    const { client } = await connectedPair(
      { CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "instance" },
      routes,
    );
    const result = (await client.callTool({
      name: "run_workflow_now",
      arguments: { id: "w1" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBe(true);
    expect(textOf(result)).toMatch(/could not resolve workflow/i);
  });

  it("fail-closed when getWorkflow returns 500 under active protection", async () => {
    const fetch: FetchLike = async (url, init) => {
      const path = url.replace("http://127.0.0.1:9618", "");
      const key = `${init?.method ?? "GET"} ${path}`;
      if (key === "GET /api/v1/workflows/w1") {
        return {
          ok: false,
          status: 500,
          text: async () => JSON.stringify({ error: "internal error" }),
        };
      }
      return routedFetch(ROUTES)(url, init);
    };
    const config = configFromEnv({
      CHAOS_SCHEDULER_API_KEY: "id.secret",
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "instance",
    });
    const sdk = new ChaosSchedulerClient({
      baseUrl: config.baseUrl,
      apiKey: config.apiKey,
      fetch,
    });
    const server = buildServer({ client: sdk, config });
    const [clientTransport, serverTransport] =
      InMemoryTransport.createLinkedPair();
    const client = new Client({ name: "test-client", version: "0.0.0" });
    await Promise.all([
      server.connect(serverTransport),
      client.connect(clientTransport),
    ]);
    const result = (await client.callTool({
      name: "run_workflow_now",
      arguments: { id: "w1" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBe(true);
    expect(textOf(result)).toMatch(/could not resolve workflow/i);
  });

  it("blocks update_workflow when destination environment is protected", async () => {
    const { client } = await connectedPair({
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "prod",
    });
    const result = (await client.callTool({
      name: "update_workflow",
      arguments: { id: "w1", environment: "prod" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBe(true);
    expect(textOf(result)).toMatch(/protected/i);
  });

  it("update_workflow proxies through the SDK", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "update_workflow",
      arguments: { id: "w1", name: "Renamed" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    expect(JSON.parse(textOf(result)).id).toBe("w1");
  });

  it("rerun_workflow proxies through the SDK", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "rerun_workflow",
      arguments: { id: "w1", idempotency_key: "rk1" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    expect(JSON.parse(textOf(result)).run_id).toBe("r2");
  });

  it("get_run_tasks proxies through the SDK", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "get_run_tasks",
      arguments: { id: "r1" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    expect(JSON.parse(textOf(result)).tasks[0].task_id).toBe("step1");
  });

  it("list_queued_runs proxies through the SDK", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "list_queued_runs",
      arguments: {},
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    expect(JSON.parse(textOf(result))[0].id).toBe("q1");
  });

  it("list_email_profiles proxies through the SDK (masked)", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "list_email_profiles",
      arguments: {},
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    const profiles = JSON.parse(textOf(result));
    expect(profiles[0].id).toBe("ep1");
    expect(profiles[0].smtp_password).toBe("••••••••");
  });

  it("create_email_profile proxies through the SDK", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "create_email_profile",
      arguments: {
        name: "Primary",
        enabled: true,
        alert_email: "alerts@example.com",
        smtp_host: "smtp.example.com",
        smtp_port: 587,
        smtp_user: "mailer",
        smtp_password: "realpw",
        from_address: "from@example.com",
        from_name: "Chaos",
      },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    const created = JSON.parse(textOf(result));
    expect(created.id).toBe("ep2");
    expect(created.smtp_password).toBe("••••••••");
  });

  it("set_workflow_email_profile proxies through the SDK", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "set_workflow_email_profile",
      arguments: { workflow_id: "w1", profile_id: "ep1" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    expect(JSON.parse(textOf(result)).email_profile_id).toBe("ep1");
  });

  it("reads the email-profiles resource", async () => {
    const { client } = await connectedPair();
    const res = await client.readResource({ uri: "chaos://email-profiles" });
    const text = (res.contents[0] as { text: string }).text;
    expect(JSON.parse(text)[0].id).toBe("ep1");
  });

  it("dispatch_workflow proxies through the SDK with signature header", async () => {
    const calls: Array<{ headers?: Record<string, string> }> = [];
    const fetch: FetchLike = async (url, init) => {
      calls.push({ headers: init?.headers as Record<string, string> });
      const path = url.replace("http://127.0.0.1:9618", "");
      const key = `${init?.method ?? "GET"} ${path}`;
      const body = ROUTES[key];
      return {
        ok: true,
        status: 200,
        text: async () => JSON.stringify(body),
      };
    };
    const config = configFromEnv({ CHAOS_SCHEDULER_API_KEY: "id.secret" });
    const sdk = new ChaosSchedulerClient({
      baseUrl: config.baseUrl,
      apiKey: config.apiKey,
      fetch,
    });
    const server = buildServer({ client: sdk, config });
    const [clientTransport, serverTransport] =
      InMemoryTransport.createLinkedPair();
    const client = new Client({ name: "test-client", version: "0.0.0" });
    await Promise.all([
      server.connect(serverTransport),
      client.connect(clientTransport),
    ]);
    const result = (await client.callTool({
      name: "dispatch_workflow",
      arguments: {
        id: "w1",
        payload: "{}",
        signature_secret: "hook-secret",
      },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    const dispatchCall = calls.find((c) =>
      c.headers?.["x-chaos-signature"]?.startsWith("sha256="),
    );
    expect(dispatchCall?.headers?.["x-chaos-signature"]).toMatch(
      /^sha256=[0-9a-f]{64}$/,
    );
  });
});
