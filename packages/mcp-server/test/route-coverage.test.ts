import { readFileSync } from "node:fs";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { ChaosSchedulerClient, type FetchLike } from "@chaos-scheduler/sdk";
import { describe, expect, it } from "vitest";
import { configFromEnv } from "../src/config.js";
import { buildServer } from "../src/server.js";

/**
 * Exact contract matrix: MCP tool name → SDK method(s) → REST route(s).
 * Keeps the public integration surface aligned with `src-tauri/src/api.rs`.
 */
const TOOL_ROUTE_MATRIX = [
  ["get_version", ["getVersion"], ["GET /api/v1/version"]],
  ["health_check", ["getHealth"], ["GET /api/v1/health"]],
  ["list_environments", ["listEnvironments"], ["GET /api/v1/environments"]],
  ["create_environment", ["createEnvironment"], ["POST /api/v1/environments"]],
  ["list_workflows", ["listWorkflows"], ["GET /api/v1/workflows"]],
  ["get_workflow", ["getWorkflow"], ["GET /api/v1/workflows/{id}"]],
  ["register_workflow", ["registerWorkflow"], ["POST /api/v1/workflows"]],
  [
    "set_workflow_spec",
    ["setWorkflowSpec"],
    ["POST /api/v1/workflows/{id}/spec"],
  ],
  [
    "patch_workflow_spec",
    ["getWorkflow", "setWorkflowSpec"],
    ["GET /api/v1/workflows/{id}", "POST /api/v1/workflows/{id}/spec"],
  ],
  ["update_workflow", ["updateWorkflow"], ["PATCH /api/v1/workflows/{id}"]],
  ["rerun_workflow", ["rerunWorkflow"], ["POST /api/v1/workflows/{id}/rerun"]],
  ["delete_workflow", ["deleteWorkflow"], ["DELETE /api/v1/workflows/{id}"]],
  ["run_workflow_now", ["runWorkflow"], ["POST /api/v1/workflows/{id}/run"]],
  [
    "enqueue_workflow",
    ["enqueueWorkflow"],
    ["POST /api/v1/workflows/{id}/enqueue"],
  ],
  [
    "dispatch_workflow",
    ["dispatchWorkflow"],
    ["POST /api/v1/workflows/{id}/dispatch"],
  ],
  ["list_workflow_runs", ["listRuns"], ["GET /api/v1/workflows/{id}/runs"]],
  ["get_run", ["getRun"], ["GET /api/v1/runs/{id}"]],
  ["get_run_logs", ["getRunLogs"], ["GET /api/v1/runs/{id}/logs"]],
  ["get_run_tasks", ["getRunTasks"], ["GET /api/v1/runs/{id}/tasks"]],
  ["get_run_metrics", ["getRunMetrics"], ["GET /api/v1/runs/{id}/metrics"]],
  ["list_queues", ["listQueues"], ["GET /api/v1/queues"]],
  ["list_queued_runs", ["listQueuedRuns"], ["GET /api/v1/queued-runs"]],
  [
    "list_email_profiles",
    ["listEmailProfiles"],
    ["GET /api/v1/email-profiles"],
  ],
  [
    "create_email_profile",
    ["createEmailProfile"],
    ["POST /api/v1/email-profiles"],
  ],
  [
    "update_email_profile",
    ["updateEmailProfile"],
    ["PATCH /api/v1/email-profiles/{id}"],
  ],
  [
    "delete_email_profile",
    ["deleteEmailProfile"],
    ["DELETE /api/v1/email-profiles/{id}"],
  ],
  [
    "set_workflow_email_profile",
    ["setWorkflowEmailProfile"],
    ["POST /api/v1/workflows/{id}/email-profile"],
  ],
] as const;

const EMAIL_PROFILE_INPUT = {
  name: "Test",
  enabled: true,
  alert_email: "alerts@example.com",
  smtp_host: "smtp.example.com",
  smtp_port: 587,
  smtp_user: "mailer",
  smtp_password: "secret",
  from_address: "alerts@example.com",
  from_name: "Chaos Scheduler",
};

const TOOL_ARGUMENTS: Record<string, Record<string, unknown>> = {
  get_version: {},
  health_check: {},
  list_environments: {},
  create_environment: { name: "test-environment" },
  list_workflows: {},
  get_workflow: { id: "test-id" },
  register_workflow: {
    name: "Test",
    script_path: "/tmp/test.sh",
    cron_schedule: "0 0 * * *",
  },
  set_workflow_spec: {
    id: "test-id",
    spec: {
      kind: "generic",
      generic: { steps: [{ id: "run", command: "echo ok" }] },
    },
  },
  patch_workflow_spec: { id: "test-id", patch: {} },
  update_workflow: { id: "test-id", enabled: true },
  rerun_workflow: {
    id: "test-id",
    source_run_id: "run-id",
    idempotency_key: "route-contract",
  },
  delete_workflow: { id: "test-id" },
  run_workflow_now: {
    id: "test-id",
    idempotency_key: "route-contract",
  },
  enqueue_workflow: {
    id: "test-id",
    idempotency_key: "route-contract",
  },
  dispatch_workflow: {
    id: "test-id",
    payload: "{}",
    idempotency_key: "route-contract",
  },
  list_workflow_runs: { id: "test-id" },
  get_run: { id: "test-id" },
  get_run_logs: { id: "test-id" },
  get_run_tasks: { id: "test-id" },
  get_run_metrics: { id: "test-id" },
  list_queues: {},
  list_queued_runs: {},
  list_email_profiles: {},
  create_email_profile: EMAIL_PROFILE_INPUT,
  update_email_profile: { id: "test-id", ...EMAIL_PROFILE_INPUT },
  delete_email_profile: { id: "test-id" },
  set_workflow_email_profile: {
    workflow_id: "test-id",
    profile_id: "profile-id",
  },
};

type SdkRouteContract = {
  route: string;
  invoke: (client: ChaosSchedulerClient) => Promise<unknown>;
};

const SDK_ROUTE_CONTRACTS: Record<string, SdkRouteContract> = {
  getVersion: {
    route: "GET /api/v1/version",
    invoke: (client) => client.getVersion(),
  },
  getHealth: {
    route: "GET /api/v1/health",
    invoke: (client) => client.getHealth(),
  },
  listEnvironments: {
    route: "GET /api/v1/environments",
    invoke: (client) => client.listEnvironments(),
  },
  createEnvironment: {
    route: "POST /api/v1/environments",
    invoke: (client) => client.createEnvironment({ name: "test" }),
  },
  listWorkflows: {
    route: "GET /api/v1/workflows",
    invoke: (client) => client.listWorkflows(),
  },
  getWorkflow: {
    route: "GET /api/v1/workflows/{id}",
    invoke: (client) => client.getWorkflow("test-id"),
  },
  registerWorkflow: {
    route: "POST /api/v1/workflows",
    invoke: (client) =>
      client.registerWorkflow({
        name: "Test",
        script_path: "/tmp/test.sh",
        cron_schedule: "0 0 * * *",
      }),
  },
  setWorkflowSpec: {
    route: "POST /api/v1/workflows/{id}/spec",
    invoke: (client) =>
      client.setWorkflowSpec("test-id", {
        kind: "generic",
        generic: { steps: [{ id: "run", command: "echo ok" }] },
      }),
  },
  updateWorkflow: {
    route: "PATCH /api/v1/workflows/{id}",
    invoke: (client) => client.updateWorkflow("test-id", { enabled: true }),
  },
  rerunWorkflow: {
    route: "POST /api/v1/workflows/{id}/rerun",
    invoke: (client) =>
      client.rerunWorkflow("test-id", {
        sourceRunId: "run-id",
        idempotencyKey: "route-contract",
      }),
  },
  deleteWorkflow: {
    route: "DELETE /api/v1/workflows/{id}",
    invoke: (client) => client.deleteWorkflow("test-id"),
  },
  runWorkflow: {
    route: "POST /api/v1/workflows/{id}/run",
    invoke: (client) =>
      client.runWorkflow("test-id", { idempotencyKey: "route-contract" }),
  },
  enqueueWorkflow: {
    route: "POST /api/v1/workflows/{id}/enqueue",
    invoke: (client) =>
      client.enqueueWorkflow("test-id", { idempotencyKey: "route-contract" }),
  },
  dispatchWorkflow: {
    route: "POST /api/v1/workflows/{id}/dispatch",
    invoke: (client) =>
      client.dispatchWorkflow("test-id", {
        payload: "{}",
        idempotencyKey: "route-contract",
      }),
  },
  listRuns: {
    route: "GET /api/v1/workflows/{id}/runs",
    invoke: (client) => client.listRuns("test-id"),
  },
  getRun: {
    route: "GET /api/v1/runs/{id}",
    invoke: (client) => client.getRun("test-id"),
  },
  getRunLogs: {
    route: "GET /api/v1/runs/{id}/logs",
    invoke: (client) => client.getRunLogs("test-id"),
  },
  getRunTasks: {
    route: "GET /api/v1/runs/{id}/tasks",
    invoke: (client) => client.getRunTasks("test-id"),
  },
  getRunMetrics: {
    route: "GET /api/v1/runs/{id}/metrics",
    invoke: (client) => client.getRunMetrics("test-id"),
  },
  listQueues: {
    route: "GET /api/v1/queues",
    invoke: (client) => client.listQueues(),
  },
  listQueuedRuns: {
    route: "GET /api/v1/queued-runs",
    invoke: (client) => client.listQueuedRuns(),
  },
  listEmailProfiles: {
    route: "GET /api/v1/email-profiles",
    invoke: (client) => client.listEmailProfiles(),
  },
  createEmailProfile: {
    route: "POST /api/v1/email-profiles",
    invoke: (client) => client.createEmailProfile(EMAIL_PROFILE_INPUT),
  },
  updateEmailProfile: {
    route: "PATCH /api/v1/email-profiles/{id}",
    invoke: (client) =>
      client.updateEmailProfile("test-id", EMAIL_PROFILE_INPUT),
  },
  deleteEmailProfile: {
    route: "DELETE /api/v1/email-profiles/{id}",
    invoke: (client) => client.deleteEmailProfile("test-id"),
  },
  setWorkflowEmailProfile: {
    route: "POST /api/v1/workflows/{id}/email-profile",
    invoke: (client) => client.setWorkflowEmailProfile("test-id", "profile-id"),
  },
};

const API_ROUTE_SEGMENTS = readFileSync(
  new URL("../../../src-tauri/src/api.rs", import.meta.url),
  "utf8",
)
  .split("let router = Router::new()")[1]!
  .split(".layer(RequestBodyLimitLayer")[0]!
  .split(/(?=\.route\()/);

const EXPECTED_RESOURCES = [
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
];

const EXPECTED_RESOURCE_TEMPLATES = [
  "chaos://runs/{id}",
  "chaos://runs/{id}/logs",
  "chaos://runs/{id}/metrics",
  "chaos://runs/{id}/tasks",
  "chaos://workflows/{id}",
  "chaos://workflows/{id}/definition",
  "chaos://workflows/{id}/runs",
];

const EXPECTED_PROMPTS = [
  "register_workflow_for_repo",
  "safely_update_workflow",
  "summarize_workflow_health",
  "triage_failed_run",
];

const WORKFLOW_FIXTURE = {
  id: "test-id",
  name: "Test",
  environment: "sandbox",
  spec_json: JSON.stringify({
    kind: "generic",
    generic: { steps: [{ id: "run", command: "echo ok" }] },
  }),
};

function backendFixture(method: string, path: string): unknown {
  if (path === "/api/v1/environments") {
    return method === "GET"
      ? { environments: [] }
      : { environment: { id: "test-environment" } };
  }
  if (path === "/api/v1/workflows") {
    return method === "GET"
      ? { workflows: [] }
      : { workflow: WORKFLOW_FIXTURE };
  }
  if (path === "/api/v1/workflows/test-id") {
    if (method === "DELETE") return { deleted: "test-id" };
    return { workflow: WORKFLOW_FIXTURE };
  }
  if (path === "/api/v1/workflows/test-id/spec") {
    return { workflow: WORKFLOW_FIXTURE };
  }
  if (path === "/api/v1/workflows/test-id/runs") return { runs: [] };
  if (path === "/api/v1/runs/test-id") return { run: { id: "test-id" } };
  if (path === "/api/v1/runs/test-id/tasks") {
    return { tasks: [], attempts: [] };
  }
  if (path === "/api/v1/runs/test-id/metrics") return { metrics: [] };
  if (path === "/api/v1/queues") return { queues: [] };
  if (path === "/api/v1/queued-runs") return { queued_runs: [] };
  if (path === "/api/v1/email-profiles") {
    return method === "GET"
      ? { email_profiles: [] }
      : { email_profile: { id: "profile-id" } };
  }
  if (path === "/api/v1/email-profiles/test-id") {
    return method === "DELETE"
      ? { deleted: "test-id" }
      : { email_profile: { id: "test-id" } };
  }
  return {};
}

async function connectedPair(fetchOverride?: FetchLike) {
  const fetch: FetchLike =
    fetchOverride ??
    (async () => ({
      ok: true,
      status: 200,
      text: async () => "{}",
    }));
  const config = {
    ...configFromEnv({ CHAOS_SCHEDULER_API_KEY: "id.secret" }),
    protectedEnvironments: [],
  };
  const sdk = new ChaosSchedulerClient({
    baseUrl: config.baseUrl,
    apiKey: config.apiKey,
    fetch,
  });
  const server = buildServer({ client: sdk, config });
  const [clientTransport, serverTransport] =
    InMemoryTransport.createLinkedPair();
  const client = new Client({ name: "manifest-test", version: "0.0.0" });
  await Promise.all([
    server.connect(serverTransport),
    client.connect(clientTransport),
  ]);
  return client;
}

describe("SDK/MCP route coverage matrix", () => {
  it("maps every registered MCP tool to existing SDK methods and documented routes", async () => {
    const client = new ChaosSchedulerClient({
      baseUrl: "http://127.0.0.1:9618",
      apiKey: "id.secret",
      fetch: async () => ({
        ok: true,
        status: 200,
        text: async () => "{}",
      }),
    });
    for (const [, sdkMethods, routes] of TOOL_ROUTE_MATRIX) {
      expect(routes).toEqual(
        sdkMethods.map((sdkMethod) => SDK_ROUTE_CONTRACTS[sdkMethod]?.route),
      );
      for (const sdkMethod of sdkMethods) {
        expect(
          typeof (client as unknown as Record<string, unknown>)[sdkMethod],
        ).toBe("function");
      }
    }

    const mcp = await connectedPair();
    const { tools } = await mcp.listTools();
    expect(tools.map((tool) => tool.name).sort()).toEqual(
      TOOL_ROUTE_MATRIX.map(([tool]) => tool).sort(),
    );
    expect(Object.keys(TOOL_ARGUMENTS).sort()).toEqual(
      TOOL_ROUTE_MATRIX.map(([tool]) => tool).sort(),
    );
    expect(Object.keys(SDK_ROUTE_CONTRACTS).sort()).toEqual(
      [
        ...new Set(
          TOOL_ROUTE_MATRIX.flatMap(([, sdkMethods]) => [...sdkMethods]),
        ),
      ].sort(),
    );
  });

  it("executes every MCP tool through its declared REST routes", async () => {
    for (const [tool, , routes] of TOOL_ROUTE_MATRIX) {
      const requests: string[] = [];
      const fetch: FetchLike = async (url, init) => {
        const method = init?.method ?? "GET";
        const path = new URL(url).pathname;
        requests.push(`${method} ${path}`);
        return {
          ok: true,
          status: 200,
          text: async () => JSON.stringify(backendFixture(method, path)),
        };
      };
      const client = await connectedPair(fetch);

      const result = await client.callTool({
        name: tool,
        arguments: TOOL_ARGUMENTS[tool],
      });

      expect(result.isError, tool).toBeFalsy();
      expect(requests, tool).toEqual(
        routes.map((route) => route.replace("{id}", "test-id")),
      );
      await client.close();
    }
  });

  it("executes every declared SDK route contract", async () => {
    for (const [sdkMethod, contract] of Object.entries(SDK_ROUTE_CONTRACTS)) {
      const requests: string[] = [];
      const client = new ChaosSchedulerClient({
        baseUrl: "http://127.0.0.1:9618",
        apiKey: "id.secret",
        fetch: async (url, init) => {
          requests.push(`${init?.method ?? "GET"} ${new URL(url).pathname}`);
          return {
            ok: true,
            status: 200,
            text: async () => "{}",
          };
        },
      });

      await contract.invoke(client);

      expect(requests, sdkMethod).toEqual([
        contract.route.replace("{id}", "test-id"),
      ]);

      const [method, path] = contract.route.split(" ");
      const routeSegment = API_ROUTE_SEGMENTS.find((segment) =>
        segment.includes(`"${path}"`),
      );
      expect(routeSegment, `${sdkMethod} Rust route`).toBeDefined();
      expect(routeSegment, `${sdkMethod} Rust method`).toMatch(
        new RegExp(`\\b${method!.toLowerCase()}\\(`),
      );
    }
  });

  it("locks exact resource, template, and prompt identifier manifests", async () => {
    const client = await connectedPair();
    const [{ resources }, { resourceTemplates }, { prompts }] =
      await Promise.all([
        client.listResources(),
        client.listResourceTemplates(),
        client.listPrompts(),
      ]);

    expect(resources.map((resource) => resource.uri).sort()).toEqual(
      [...EXPECTED_RESOURCES].sort(),
    );
    expect(
      resourceTemplates.map((template) => template.uriTemplate).sort(),
    ).toEqual([...EXPECTED_RESOURCE_TEMPLATES].sort());
    expect(prompts.map((prompt) => prompt.name).sort()).toEqual(
      [...EXPECTED_PROMPTS].sort(),
    );
  });
});
