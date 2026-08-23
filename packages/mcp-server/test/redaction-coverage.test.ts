/**
 * Redaction coverage matrix.
 *
 * Enumerates every workflow-state READ surface an MCP client can reach and
 * asserts none emits a raw workflow secret for a fixture workflow, regardless
 * of API-key scope.
 *
 * The whole point of this test is the write-scoped-key condition: the managed
 * Cursor MCP integration mints a `read,write` key (`src-tauri/src/mcp.rs`), so
 * the Rust service layer's scope-based redaction is bypassed and the REST/SDK
 * response the MCP server consumes carries FULL secrets. The upstream backend
 * fixture below therefore returns UNREDACTED secrets on purpose — every
 * assertion here proves the MCP egress boundary redacts on its own, not because
 * REST happened to redact upstream.
 *
 * Read-surface matrix (the four classes named in the hardening plan):
 *   - REST / SDK  — the untrusted upstream. At `write`/`admin` scope it returns
 *                   full secrets by design (round-trip edits); exercised here as
 *                   the adversarial source feeding every MCP surface, and
 *                   asserted non-vacuous (it really does carry the secrets).
 *   - Desktop IPC — a Rust/Tauri channel outside this package. It intentionally
 *                   returns full secrets to the local operator's own authoring
 *                   UI, is never agent/LLM context, and is covered by Rust-side
 *                   `service.rs` scope tests. Not executable from this package.
 *   - MCP tools   — `list_workflows`, `get_workflow`: MUST always redact.
 *   - MCP resources — `chaos://workflows`, `chaos://workflows/{id}`,
 *                   `chaos://workflows/{id}/definition`, `chaos://workflows/index`:
 *                   MUST always redact.
 *
 * Pre-fix, the two MCP tools returned the SDK result verbatim and this test
 * fails; after wrapping them in the resource projection it passes.
 */
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { ChaosSchedulerClient, type FetchLike } from "@chaos-scheduler/sdk";
import { describe, expect, it } from "vitest";
import { configFromEnv } from "../src/config.js";
import { buildServer } from "../src/server.js";

/**
 * Distinct, greppable secret values planted in every secret-bearing field the
 * projection is meant to strip (`secret`, `signature_secret`, `cursor_api_key`,
 * `smtp_password`), plus an internal-only column the resource allowlist drops.
 */
const SECRETS = {
  outboundWebhook: "outbound-webhook-hmac-SECRET-a1",
  inboundSignature: "inbound-signature-SECRET-b2",
  cursorApiKey: "cursor-api-key-SECRET-c3",
  smtpPassword: "smtp-password-SECRET-d4",
  queueMetadata: "queue-metadata-SECRET-e5",
} as const;
const SECRET_VALUES = Object.values(SECRETS);
const INTERNAL_ONLY = "internal-only-must-not-cross-boundary-f6";

const WORKFLOW_ID = "wf-secret";

/** A full, unredacted workflow exactly as a write-scoped REST read returns it. */
const RAW_WORKFLOW = {
  id: WORKFLOW_ID,
  name: "Has Secrets",
  description: "workflow with credentials in every stored-JSON field",
  script_path: "run.sh",
  cron_schedule: "0 0 * * *",
  enabled: true,
  async_mode: false,
  email_on_failure: true,
  environment: "sandbox",
  managed_externally: true,
  kind: "generic",
  spec_json: JSON.stringify({
    kind: "generic",
    generic: { steps: [{ id: "run", command: "echo ok" }] },
    on_failure: [
      {
        type: "webhook",
        url: "https://example.com/hook",
        secret: SECRETS.outboundWebhook,
      },
    ],
    nested: {
      cursor_api_key: SECRETS.cursorApiKey,
      smtp_password: SECRETS.smtpPassword,
    },
  }),
  domain: null,
  timezone: "UTC",
  trigger_config: JSON.stringify([
    {
      kind: "file_arrival",
      path: "inbox/*.json",
      signature_secret: SECRETS.inboundSignature,
    },
  ]),
  queue_config: JSON.stringify({
    queue: "sandbox-default",
    metadata: { secret: SECRETS.queueMetadata },
  }),
  email_profile_id: null,
  last_run_at: null,
  created_at: "2026-01-01T00:00:00Z",
  updated_at: "2026-01-01T00:00:00Z",
  // Not in the resource allowlist — must never cross the projection boundary.
  internal_only: INTERNAL_ONLY,
};

const BACKEND_ROUTES: Record<string, unknown> = {
  "GET /api/v1/workflows": { workflows: [RAW_WORKFLOW] },
  [`GET /api/v1/workflows/${WORKFLOW_ID}`]: { workflow: RAW_WORKFLOW },
};

function rawBackendFetch(): FetchLike {
  return async (url, init) => {
    const path = url.replace("http://127.0.0.1:9618", "");
    const key = `${init?.method ?? "GET"} ${path}`;
    const body = BACKEND_ROUTES[key];
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

function rawSdk(): ChaosSchedulerClient {
  return new ChaosSchedulerClient({
    baseUrl: "http://127.0.0.1:9618",
    // A write-scoped key is exactly what the managed integration uses; the mock
    // backend returns full secrets regardless, which is the point.
    apiKey: "managed.write-scoped-secret",
    fetch: rawBackendFetch(),
  });
}

async function connectedClient(): Promise<Client> {
  const config = configFromEnv({
    CHAOS_SCHEDULER_API_KEY: "managed.write-scoped-secret",
  });
  const server = buildServer({ client: rawSdk(), config });
  const [clientTransport, serverTransport] =
    InMemoryTransport.createLinkedPair();
  const client = new Client({ name: "redaction-coverage", version: "0.0.0" });
  await Promise.all([
    server.connect(serverTransport),
    client.connect(clientTransport),
  ]);
  return client;
}

function toolText(result: {
  content: Array<{ type: string; text?: string }>;
  isError?: boolean;
}): string {
  return result.content
    .filter((entry) => entry.type === "text")
    .map((entry) => entry.text ?? "")
    .join("\n");
}

async function callToolText(
  client: Client,
  name: string,
  args: Record<string, unknown>,
): Promise<string> {
  const result = (await client.callTool({ name, arguments: args })) as {
    content: Array<{ type: string; text?: string }>;
    isError?: boolean;
  };
  expect(result.isError, `${name} returned an error`).toBeFalsy();
  return toolText(result);
}

async function readResourceText(client: Client, uri: string): Promise<string> {
  const result = await client.readResource({ uri });
  return result.contents
    .map((entry) => (entry as { text?: string }).text ?? "")
    .join("\n");
}

interface ReadSurface {
  id: string;
  kind: "mcp-tool" | "mcp-resource";
  read: (client: Client) => Promise<string>;
}

const READ_SURFACES: ReadSurface[] = [
  {
    id: "mcp-tool:list_workflows",
    kind: "mcp-tool",
    read: (client) => callToolText(client, "list_workflows", {}),
  },
  {
    id: "mcp-tool:get_workflow",
    kind: "mcp-tool",
    read: (client) => callToolText(client, "get_workflow", { id: WORKFLOW_ID }),
  },
  {
    id: "mcp-resource:chaos://workflows",
    kind: "mcp-resource",
    read: (client) => readResourceText(client, "chaos://workflows"),
  },
  {
    id: "mcp-resource:chaos://workflows/{id}",
    kind: "mcp-resource",
    read: (client) =>
      readResourceText(client, `chaos://workflows/${WORKFLOW_ID}`),
  },
  {
    id: "mcp-resource:chaos://workflows/{id}/definition",
    kind: "mcp-resource",
    read: (client) =>
      readResourceText(client, `chaos://workflows/${WORKFLOW_ID}/definition`),
  },
  {
    id: "mcp-resource:chaos://workflows/index",
    kind: "mcp-resource",
    read: (client) => readResourceText(client, "chaos://workflows/index"),
  },
];

describe("redaction coverage matrix", () => {
  it("upstream REST/SDK write-scope read really carries the raw secrets (non-vacuous)", async () => {
    // If this ever stops holding, the matrix below would pass vacuously.
    const sdk = rawSdk();
    const upstream =
      JSON.stringify(await sdk.listWorkflows()) +
      JSON.stringify(await sdk.getWorkflow(WORKFLOW_ID));
    for (const secret of SECRET_VALUES) {
      expect(upstream, `upstream missing planted secret ${secret}`).toContain(
        secret,
      );
    }
    expect(upstream).toContain(INTERNAL_ONLY);
  });

  for (const surface of READ_SURFACES) {
    it(`${surface.id} emits no raw secret for any key scope`, async () => {
      const client = await connectedClient();
      const text = await surface.read(client);
      for (const secret of SECRET_VALUES) {
        expect(text, `${surface.id} leaked ${secret}`).not.toContain(secret);
      }
      // The allowlisted projection also drops non-allowlisted columns.
      expect(text, `${surface.id} leaked internal_only`).not.toContain(
        INTERNAL_ONLY,
      );
    });
  }

  it("keeps every chaos://workflows* read surface inside the covered matrix", async () => {
    const client = await connectedClient();
    const [{ resources }, { resourceTemplates }, { tools }] = await Promise.all(
      [
        client.listResources(),
        client.listResourceTemplates(),
        client.listTools(),
      ],
    );

    // Every workflow resource either projects workflow config (and is asserted
    // above) or returns run rows only. A new chaos://workflows* resource forces
    // an author to categorize it here rather than silently skipping redaction.
    const projectedResources = new Set(
      READ_SURFACES.filter((surface) => surface.kind === "mcp-resource").map(
        (surface) => surface.id.replace("mcp-resource:", ""),
      ),
    );
    const runsOnlyResources = new Set(["chaos://workflows/{id}/runs"]);
    const workflowResourceUris = [
      ...resources.map((resource) => resource.uri),
      ...resourceTemplates.map((template) => template.uriTemplate),
    ].filter((uri) => uri.startsWith("chaos://workflows"));

    for (const uri of workflowResourceUris) {
      expect(
        projectedResources.has(uri) || runsOnlyResources.has(uri),
        `uncovered workflow read resource: ${uri}`,
      ).toBe(true);
    }

    // Annotations must be present so the read-tool derivation below is real.
    expect(tools.every((tool) => tool.annotations !== undefined)).toBe(true);
    const workflowReadTools = tools
      .filter((tool) => tool.annotations?.readOnlyHint === true)
      .map((tool) => tool.name)
      .filter((name) => name.includes("workflow") && !name.includes("runs"));
    // Any new read-only workflow tool must be added to READ_SURFACES above.
    expect(new Set(workflowReadTools)).toEqual(
      new Set(["list_workflows", "get_workflow"]),
    );
  });
});
