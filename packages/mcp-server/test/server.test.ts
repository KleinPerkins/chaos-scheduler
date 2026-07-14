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
    environments: [
      { id: "e1", name: "production" },
      { id: "e2", name: "sandbox" },
    ],
  },
  "GET /api/v1/workflows": {
    workflows: [
      {
        id: "w1",
        name: "A",
        environment: "sandbox",
        kind: "generic",
        spec_json: JSON.stringify({
          kind: "generic",
          generic: {
            steps: [{ id: "run", command: "echo ok" }],
          },
          on_failure: [
            {
              type: "webhook",
              url: "https://example.com/hook",
              secret: "outbound-secret",
            },
          ],
          nested: {
            cursor_api_key: "cursor-secret",
            smtp_password: "smtp-secret",
          },
        }),
        trigger_config: JSON.stringify([
          {
            kind: "file_arrival",
            path: "inbox/*.json",
            signature_secret: "trigger-secret",
          },
        ]),
        queue_config: JSON.stringify({
          queue: "sandbox-default",
          metadata: { secret: "queue-secret" },
        }),
        internal_only: "must-not-cross-resource-boundary",
      },
      { id: "w2", name: "Prod", environment: "production" },
      {
        id: "w3",
        name: "Malformed",
        environment: "sandbox",
        spec_json: '{"secret":"malformed-secret"',
      },
    ],
  },
  "GET /api/v1/workflows/w1": {
    workflow: {
      id: "w1",
      name: "A",
      environment: "sandbox",
      kind: "generic",
      spec_json: JSON.stringify({
        kind: "generic",
        generic: {
          steps: [{ id: "run", command: "echo ok" }],
        },
        on_failure: [
          {
            type: "webhook",
            url: "https://example.com/hook",
            secret: "outbound-secret",
          },
        ],
        nested: {
          cursor_api_key: "cursor-secret",
          smtp_password: "smtp-secret",
        },
      }),
      trigger_config: JSON.stringify([
        {
          kind: "file_arrival",
          path: "inbox/*.json",
          signature_secret: "trigger-secret",
        },
      ]),
      queue_config: JSON.stringify({
        queue: "sandbox-default",
        metadata: { secret: "queue-secret" },
      }),
      internal_only: "must-not-cross-resource-boundary",
    },
  },
  "GET /api/v1/workflows/w2": {
    workflow: {
      id: "w2",
      name: "Prod",
      environment: "production",
    },
  },
  "GET /api/v1/workflows/w3": {
    workflow: {
      id: "w3",
      name: "Malformed",
      environment: "sandbox",
      spec_json: '{"secret":"malformed-secret"',
      trigger_config: null,
      queue_config: null,
    },
  },
  "POST /api/v1/workflows/w1/run": {
    workflow_id: "w1",
    status: "admitted",
    run_id: "r1",
    queued_run_id: null,
    queue_name: "sandbox-default",
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
        name: "production-default",
        environment: "production",
        capacity: 4,
        active_count: 0,
        queued_count: 0,
        global_parallelism_cap: 8,
        updated_at: "2026-01-01T00:00:00Z",
      },
      {
        name: "sandbox-default",
        environment: "sandbox",
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
        queue_name: "sandbox-default",
        environment: "sandbox",
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
    queue_name: "sandbox-default",
  },
  "PATCH /api/v1/workflows/w1": {
    workflow: {
      id: "w1",
      name: "A",
      environment: "sandbox",
    },
  },
  "POST /api/v1/workflows/w1/rerun": {
    workflow_id: "w1",
    status: "admitted",
    run_id: "r2",
    queued_run_id: null,
    queue_name: "sandbox-default",
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
  "POST /api/v1/workflows": {
    workflow: {
      id: "w-reg",
      name: "Registered",
      environment: "sandbox",
      script_path: "demo.sh",
      cron_schedule: "0 0 * * *",
      managed_externally: true,
    },
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
        "patch_workflow_spec",
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
    expect(resources.map((r) => r.uri).sort()).toEqual(
      [
        "chaos://authoring",
        "chaos://catalog",
        "chaos://email-profiles",
        "chaos://environments",
        "chaos://guides/integrations",
        "chaos://guides/webhooks",
        "chaos://guides/workflows",
        "chaos://queued-runs",
        "chaos://queues",
        "chaos://schemas/integrations",
        "chaos://schemas/queue",
        "chaos://schemas/triggers",
        "chaos://schemas/workflow-spec",
        "chaos://version",
        "chaos://workflows",
        "chaos://workflows/index",
      ].sort(),
    );

    const { resourceTemplates } = await client.listResourceTemplates();
    expect(resourceTemplates.map((r) => r.uriTemplate).sort()).toEqual(
      [
        "chaos://runs/{id}",
        "chaos://runs/{id}/logs",
        "chaos://workflows/{id}",
        "chaos://workflows/{id}/definition",
        "chaos://workflows/{id}/runs",
      ].sort(),
    );

    const { prompts } = await client.listPrompts();
    expect(prompts.map((p) => p.name).sort()).toEqual(
      [
        "register_workflow_for_repo",
        "safely_update_workflow",
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

  it("marks run_workflow_now deprecated in its description (steers to enqueue_workflow)", async () => {
    const { client } = await connectedPair();
    const { tools } = await client.listTools();
    const runNow = tools.find((t) => t.name === "run_workflow_now");
    expect(runNow).toBeDefined();
    // Manual runs are admission-controlled; run_workflow_now is a deprecated
    // alias that still works but should steer callers to enqueue_workflow.
    expect(runNow!.description ?? "").toMatch(/deprecated/i);
    expect(runNow!.description ?? "").toMatch(/enqueue_workflow/);
  });

  it("blocks writes to a protected environment (guardrail)", async () => {
    const { client } = await connectedPair({
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "production",
    });
    const result = (await client.callTool({
      name: "run_workflow_now",
      arguments: { id: "w2" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBe(true);
    expect(textOf(result)).toMatch(/protected/i);
  });

  it("register_workflow injects the MCP default environment when omitted", async () => {
    const bodies: string[] = [];
    const fetch: FetchLike = async (url, init) => {
      if (init?.method === "POST" && url.endsWith("/api/v1/workflows")) {
        bodies.push(init.body as string);
      }
      return routedFetch(ROUTES)(url, init);
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
      name: "register_workflow",
      arguments: {
        name: "Registered",
        script_path: "demo.sh",
        cron_schedule: "0 0 * * *",
      },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };
    expect(result.isError).toBeFalsy();
    expect(bodies).toHaveLength(1);
    expect(JSON.parse(bodies[0]!).environment).toBe("sandbox");
  });

  it("validates workflow specs before registration", async () => {
    const { client } = await connectedPair();
    const result = (await client.callTool({
      name: "register_workflow",
      arguments: {
        name: "Invalid",
        script_path: "demo.sh",
        cron_schedule: "0 0 * * *",
        spec: { kind: "generic", generic: { steps: [] } },
      },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };

    expect(result.isError).toBe(true);
    expect(textOf(result)).toMatch(/at least one step/i);
  });

  it("reads the workflows resource", async () => {
    const { client } = await connectedPair();
    const res = await client.readResource({ uri: "chaos://workflows" });
    const text = (res.contents[0] as { text: string }).text;
    expect(JSON.parse(text)[0].id).toBe("w1");
  });

  it("serves backend-independent authoring discovery resources", async () => {
    const { client } = await connectedPair();

    const authoring = await client.readResource({ uri: "chaos://authoring" });
    const authoringBody = JSON.parse(
      (authoring.contents[0] as { text: string }).text,
    );
    expect(authoringBody).toMatchObject({
      version: "v1",
      view: "stored_config",
    });
    expect(authoringBody.start_here).toContain("chaos://workflows/index");

    const schema = await client.readResource({
      uri: "chaos://schemas/workflow-spec",
    });
    const schemaBody = JSON.parse(
      (schema.contents[0] as { text: string }).text,
    );
    expect(schemaBody).toMatchObject({
      version: "v1",
      view: "stored_config",
    });
    expect(schemaBody.schema).toHaveProperty("$schema");

    const catalog = await client.readResource({ uri: "chaos://catalog" });
    const catalogBody = JSON.parse(
      (catalog.contents[0] as { text: string }).text,
    );
    expect(catalogBody.known_types.trigger_kinds).toContain("on_completion");
    expect(catalogBody.known_types.trigger_kinds).not.toContain(
      "inbound_webhook",
    );
    expect(catalogBody.known_types.inbound_dispatch).toMatch(
      /not a stored trigger_config kind/i,
    );

    const webhookGuide = await client.readResource({
      uri: "chaos://guides/webhooks",
    });
    const webhookBody = JSON.parse(
      (webhookGuide.contents[0] as { text: string }).text,
    );
    expect(JSON.stringify(webhookBody)).toMatch(
      /inbound_webhook_secret.*unavailable/i,
    );
    expect(webhookBody.inbound.signature).toContain(
      "METHOD\nPATH\nTIMESTAMP\nSHA256(body)",
    );
  });

  it("serves a lightweight workflow index without nested configuration", async () => {
    const { client } = await connectedPair();
    const result = await client.readResource({
      uri: "chaos://workflows/index",
    });
    const body = JSON.parse((result.contents[0] as { text: string }).text);

    expect(body).toMatchObject({
      version: "v1",
      view: "stored_config",
    });
    expect(body.workflows[0]).toMatchObject({
      id: "w1",
      name: "A",
      environment: "sandbox",
      kind: "generic",
    });
    expect(body.workflows[0]).not.toHaveProperty("spec_json");
    expect(JSON.stringify(body)).not.toContain("outbound-secret");
  });

  it("consolidates redacted stored configuration in workflow definitions", async () => {
    const { client } = await connectedPair();
    const result = await client.readResource({
      uri: "chaos://workflows/w1/definition",
    });
    const text = (result.contents[0] as { text: string }).text;
    const body = JSON.parse(text);

    expect(body).toMatchObject({
      version: "v1",
      view: "stored_config",
      workflow: { id: "w1", environment: "sandbox" },
      stored_config: {
        spec: { parse_status: "parsed" },
        triggers: { parse_status: "parsed" },
        queue: { parse_status: "parsed" },
        completion_actions: {
          parse_status: "parsed",
        },
      },
    });
    expect(body.stored_config.completion_actions.on_failure[0].secret).toBe(
      "__redacted__",
    );
    expect(text).not.toMatch(
      /outbound-secret|cursor-secret|smtp-secret|trigger-secret|queue-secret/,
    );
    expect(body.boundaries).toEqual(
      expect.arrayContaining([
        expect.stringMatching(/not effective/i),
        expect.stringMatching(/inbound.*unavailable/i),
      ]),
    );
  });

  it("reports invalid stored configuration without echoing raw JSON", async () => {
    const { client } = await connectedPair();
    const result = await client.readResource({
      uri: "chaos://workflows/w3/definition",
    });
    const text = (result.contents[0] as { text: string }).text;
    const body = JSON.parse(text);

    expect(body.stored_config.spec).toEqual({
      parse_status: "invalid",
      value: null,
    });
    expect(body.warnings).toEqual(
      expect.arrayContaining([expect.stringMatching(/spec.*invalid/i)]),
    );
    expect(text).not.toContain("malformed-secret");
  });

  it("steers registration and updates through discovery-first prompts", async () => {
    const { client } = await connectedPair();
    const register = await client.getPrompt({
      name: "register_workflow_for_repo",
      arguments: { repo_path: "/tmp/example" },
    });
    const registerText = JSON.stringify(register.messages);
    expect(registerText).toContain("chaos://workflows/index");
    expect(registerText).toMatch(/non-idempotent/i);

    const update = await client.getPrompt({
      name: "safely_update_workflow",
      arguments: { workflow_id: "w1" },
    });
    const updateText = JSON.stringify(update.messages);
    expect(updateText).toContain("chaos://workflows/w1/definition");
    expect(updateText).toContain("patch_workflow_spec");
    expect(updateText).toMatch(/redacted/i);
  });

  it("redacts nested secrets and allowlists workflow resource fields", async () => {
    const { client } = await connectedPair();

    const listResult = await client.readResource({ uri: "chaos://workflows" });
    const listText = (listResult.contents[0] as { text: string }).text;
    const workflows = JSON.parse(listText) as Array<Record<string, unknown>>;
    const listed = workflows.find((workflow) => workflow.id === "w1");
    expect(listed).toBeDefined();
    expect(listed).not.toHaveProperty("internal_only");
    expect(listText).not.toMatch(
      /outbound-secret|cursor-secret|smtp-secret|trigger-secret|queue-secret|must-not-cross/,
    );

    const listedSpec = JSON.parse(String(listed!.spec_json));
    expect(listedSpec.on_failure[0].secret).toBe("__redacted__");
    expect(listedSpec.nested.cursor_api_key).toBe("__redacted__");
    expect(listedSpec.nested.smtp_password).toBe("__redacted__");
    const listedTriggers = JSON.parse(String(listed!.trigger_config));
    expect(listedTriggers[0].signature_secret).toBe("__redacted__");
    const listedQueue = JSON.parse(String(listed!.queue_config));
    expect(listedQueue.metadata.secret).toBe("__redacted__");

    const singleResult = await client.readResource({
      uri: "chaos://workflows/w1",
    });
    const singleText = (singleResult.contents[0] as { text: string }).text;
    expect(singleText).not.toMatch(
      /outbound-secret|cursor-secret|smtp-secret|trigger-secret|queue-secret|must-not-cross/,
    );
    expect(JSON.parse(singleText).id).toBe("w1");
  });

  it("never echoes malformed stored JSON through workflow resources", async () => {
    const { client } = await connectedPair();
    const result = await client.readResource({ uri: "chaos://workflows" });
    const text = (result.contents[0] as { text: string }).text;
    const workflows = JSON.parse(text) as Array<Record<string, unknown>>;
    const malformed = workflows.find((workflow) => workflow.id === "w3");

    expect(malformed?.spec_json).toBe("__redacted_invalid_json__");
    expect(text).not.toContain("malformed-secret");
  });

  it("maps missing workflow resources to a sanitized resource error", async () => {
    const { client } = await connectedPair();

    await expect(
      client.readResource({ uri: "chaos://workflows/missing" }),
    ).rejects.toThrow(/resource not found/i);
  });

  it("does not expose backend error bodies through resource errors", async () => {
    const fetch: FetchLike = async () => ({
      ok: false,
      status: 500,
      text: async () =>
        JSON.stringify({ error: "backend-secret-response-body" }),
    });
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

    let message = "";
    try {
      await client.readResource({ uri: "chaos://workflows/w1" });
    } catch (err) {
      message = String(err);
    }
    expect(message).toMatch(/scheduler resource read failed/i);
    expect(message).not.toContain("backend-secret-response-body");
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
    expect(JSON.parse(textOf(result))[0].name).toBe("production-default");
  });

  it("fail-closed when getWorkflow returns 404 under active protection", async () => {
    const routes = { ...ROUTES };
    delete routes["GET /api/v1/workflows/w1"];
    const { client } = await connectedPair(
      { CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "production" },
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
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "production",
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
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "production",
    });
    const result = (await client.callTool({
      name: "update_workflow",
      arguments: { id: "w1", environment: "production" },
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

  it("blocks email-profile assignment for a protected workflow", async () => {
    const { client } = await connectedPair({
      CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS: "production",
    });
    const result = (await client.callTool({
      name: "set_workflow_email_profile",
      arguments: { workflow_id: "w2", profile_id: "ep1" },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };

    expect(result.isError).toBe(true);
    expect(textOf(result)).toMatch(/protected/i);
  });

  it("reads the email-profiles resource", async () => {
    const { client } = await connectedPair();
    const res = await client.readResource({ uri: "chaos://email-profiles" });
    const text = (res.contents[0] as { text: string }).text;
    expect(JSON.parse(text)[0].id).toBe("ep1");
  });

  it("patches a full stored spec while preserving redacted secrets and unknown fields", async () => {
    let writtenSpec: Record<string, unknown> | undefined;
    const fetch: FetchLike = async (url, init) => {
      const path = url.replace("http://127.0.0.1:9618", "");
      const key = `${init?.method ?? "GET"} ${path}`;
      if (key === "POST /api/v1/workflows/w1/spec") {
        writtenSpec = JSON.parse(String(init?.body));
        const current = (
          ROUTES["GET /api/v1/workflows/w1"] as {
            workflow: Record<string, unknown>;
          }
        ).workflow;
        return {
          ok: true,
          status: 200,
          text: async () =>
            JSON.stringify({
              workflow: {
                ...current,
                spec_json: JSON.stringify(writtenSpec),
              },
            }),
        };
      }
      return routedFetch(ROUTES)(url, init);
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
      name: "patch_workflow_spec",
      arguments: {
        id: "w1",
        patch: {
          generic: {
            steps: [{ id: "run", command: "echo updated" }],
          },
          on_failure: [
            {
              type: "webhook",
              url: "https://example.com/hook",
              secret: "__redacted__",
            },
          ],
          future_contract: { enabled: true },
        },
      },
    })) as {
      content: Array<{ type: string; text?: string }>;
      isError?: boolean;
    };

    expect(result.isError).toBeFalsy();
    expect(writtenSpec).toMatchObject({
      kind: "generic",
      generic: {
        steps: [{ id: "run", command: "echo updated" }],
      },
      on_failure: [
        {
          type: "webhook",
          secret: "outbound-secret",
        },
      ],
      nested: {
        cursor_api_key: "cursor-secret",
        smtp_password: "smtp-secret",
      },
      future_contract: { enabled: true },
    });
    const responseText = textOf(result);
    expect(responseText).not.toMatch(
      /outbound-secret|cursor-secret|smtp-secret/,
    );
    expect(JSON.parse(responseText)).toMatchObject({
      version: "v1",
      view: "stored_config",
      stored_config: {
        spec: { parse_status: "parsed" },
      },
    });
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
        event_id: "event-123",
        timestamp: "1767225600",
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
    expect(dispatchCall?.headers?.["x-chaos-event-id"]).toBe("event-123");
    expect(dispatchCall?.headers?.["x-chaos-timestamp"]).toBe("1767225600");
  });
});
