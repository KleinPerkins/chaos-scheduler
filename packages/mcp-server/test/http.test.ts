import http from "node:http";
import type { FetchLike } from "@chaos-scheduler/sdk";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StreamableHTTPClientTransport } from "@modelcontextprotocol/sdk/client/streamableHttp.js";
import { afterEach, describe, expect, it } from "vitest";
import { configFromEnv } from "../src/config.js";
import { createHttpServer, runHttp } from "../src/http.js";

const servers: http.Server[] = [];

afterEach(async () => {
  await Promise.all(
    servers.splice(0).map(
      (server) =>
        new Promise<void>((resolve, reject) => {
          server.close((err) => (err ? reject(err) : resolve()));
        }),
    ),
  );
});

async function startServer(
  overrides: Partial<ReturnType<typeof configFromEnv>> = {},
  backendFetch?: FetchLike,
): Promise<{ baseUrl: string; config: ReturnType<typeof configFromEnv> }> {
  const config = {
    ...configFromEnv({ CHAOS_SCHEDULER_API_KEY: "server.fallback" }),
    httpPort: 0,
    ...overrides,
  };
  const server = createHttpServer(config, undefined, backendFetch);
  await new Promise<void>((resolve) => {
    server.listen(0, "127.0.0.1", resolve);
  });
  servers.push(server);
  const address = server.address();
  if (typeof address !== "object" || address === null) {
    throw new Error("server did not bind to a TCP port");
  }
  return { baseUrl: `http://127.0.0.1:${address.port}`, config };
}

async function request(
  baseUrl: string,
  options: {
    method?: string;
    path?: string;
    body?: string;
    authorization?: string;
    host?: string;
  } = {},
): Promise<{ status: number; body: string; allow?: string }> {
  const url = new URL(options.path ?? "/mcp", baseUrl);
  return new Promise((resolve, reject) => {
    const req = http.request(
      url,
      {
        method: options.method ?? "POST",
        headers: {
          accept: "application/json, text/event-stream",
          "content-type": "application/json",
          ...(options.authorization
            ? { authorization: options.authorization }
            : {}),
          ...(options.host ? { host: options.host } : {}),
        },
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on("data", (chunk) => chunks.push(Buffer.from(chunk)));
        res.on("end", () =>
          resolve({
            status: res.statusCode ?? 0,
            body: Buffer.concat(chunks).toString("utf8"),
            allow: res.headers.allow,
          }),
        );
      },
    );
    req.on("error", reject);
    if (options.body) req.write(options.body);
    req.end();
  });
}

describe("HTTP MCP transport guardrails", () => {
  it("requires per-request bearer auth and ignores the configured server key", async () => {
    const { baseUrl } = await startServer();

    const res = await request(baseUrl, {
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

    expect(res.status).toBe(401);
    expect(JSON.parse(res.body).error.message).toMatch(/missing bearer/i);
  });

  it("rejects non-POST MCP requests", async () => {
    const { baseUrl } = await startServer();

    const res = await request(baseUrl, {
      method: "GET",
      authorization: "Bearer user.key",
    });

    expect(res.status).toBe(405);
    expect(res.allow).toBe("POST");
  });

  it("rejects oversized request bodies before dispatch", async () => {
    const { baseUrl } = await startServer({ httpMaxBodyBytes: 8 });

    const res = await request(baseUrl, {
      authorization: "Bearer user.key",
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

    expect(res.status).toBe(413);
    expect(JSON.parse(res.body).error.message).toMatch(/too large/i);
  });

  it("rejects non-loopback Host headers unless remote HTTP is enabled", async () => {
    const { baseUrl } = await startServer();

    const res = await request(baseUrl, {
      authorization: "Bearer user.key",
      host: "evil.example:9700",
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

    expect(res.status).toBe(403);
    expect(JSON.parse(res.body).error.message).toMatch(/host not allowed/i);
  });

  it("accepts a mixed-case Bearer scheme", async () => {
    const { baseUrl } = await startServer();

    const res = await request(baseUrl, {
      authorization: "bEaReR user.key",
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

    expect(res.status).not.toBe(401);
  });

  it("accepts a tab separator between scheme and token", async () => {
    const { baseUrl } = await startServer();

    const res = await request(baseUrl, {
      authorization: "Bearer\tuser.key",
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

    expect(res.status).not.toBe(401);
  });

  it("accepts extra whitespace before the token", async () => {
    const { baseUrl } = await startServer();

    const res = await request(baseUrl, {
      authorization: "Bearer     user.key",
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

    expect(res.status).not.toBe(401);
  });

  it("rejects a non-Bearer scheme", async () => {
    const { baseUrl } = await startServer();

    const res = await request(baseUrl, {
      authorization: "Basic dXNlcjpwYXNz",
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

    expect(res.status).toBe(401);
    expect(JSON.parse(res.body).error.message).toMatch(/missing bearer/i);
  });

  it("rejects a Bearer scheme with no token", async () => {
    const { baseUrl } = await startServer();

    const res = await request(baseUrl, {
      authorization: "Bearer",
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

    expect(res.status).toBe(401);
    expect(JSON.parse(res.body).error.message).toMatch(/missing bearer/i);
  });

  it("rejects a Bearer scheme with only whitespace as the token", async () => {
    const { baseUrl } = await startServer();

    const res = await request(baseUrl, {
      authorization: "Bearer    ",
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

    expect(res.status).toBe(401);
    expect(JSON.parse(res.body).error.message).toMatch(/missing bearer/i);
  });

  it("returns quickly for a long malformed header (regression for ReDoS)", async () => {
    const { baseUrl } = await startServer();

    // Stays under Node's default HTTP header size limit (~16KB) so the
    // connection isn't reset before reaching the (now regex-free) parser.
    const res = await request(baseUrl, {
      authorization: `Bearer${" ".repeat(8_000)}`,
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
    });

    // All whitespace, no token: still a well-formed rejection, not a hang.
    expect(res.status).toBe(401);
    expect(JSON.parse(res.body).error.message).toMatch(/missing bearer/i);
  });

  it("refuses remote binds without explicit opt-in", async () => {
    await expect(
      runHttp({
        ...configFromEnv({ CHAOS_SCHEDULER_API_KEY: "server.fallback" }),
        transport: "http",
        httpHost: "0.0.0.0",
        httpPort: 0,
      }),
    ).rejects.toThrow(/allow-remote-http/);
  });

  it("runs an authenticated initialize-to-completion-to-resource-read flow", async () => {
    const fetchedPaths: string[] = [];
    const backendAuthorizations: Array<string | null> = [];
    const fetch: FetchLike = async (url, init) => {
      const path = url.replace("http://127.0.0.1:9618", "");
      fetchedPaths.push(path);
      backendAuthorizations.push(
        new Headers(init?.headers).get("authorization"),
      );
      const body =
        path === "/api/v1/workflows"
          ? {
              workflows: [
                { id: "wf-alpha", name: "Alpha", environment: "sandbox" },
                { id: "wf-beta", name: "Beta", environment: "sandbox" },
              ],
            }
          : path === "/api/v1/runs/run-1/tasks"
            ? {
                tasks: [
                  {
                    id: "task-row-1",
                    run_id: "run-1",
                    task_id: "build",
                    status: "success",
                  },
                ],
                attempts: [],
              }
            : {};
      return {
        ok: true,
        status: 200,
        text: async () => JSON.stringify(body),
      };
    };
    const { baseUrl } = await startServer({}, fetch);
    const transport = new StreamableHTTPClientTransport(
      new URL("/mcp", baseUrl),
      {
        requestInit: {
          headers: { authorization: "Bearer user.scoped-key" },
        },
      },
    );
    const client = new Client({
      name: "http-contract-test",
      version: "0.0.0",
    });
    await client.connect(transport);

    const { resourceTemplates } = await client.listResourceTemplates();
    const listedTemplates = resourceTemplates.map(
      (template) => template.uriTemplate,
    );
    expect(listedTemplates.sort()).toEqual(
      [
        "chaos://runs/{id}",
        "chaos://runs/{id}/logs",
        "chaos://runs/{id}/metrics",
        "chaos://runs/{id}/tasks",
        "chaos://workflows/{id}",
        "chaos://workflows/{id}/definition",
        "chaos://workflows/{id}/runs",
      ].sort(),
    );
    expect(fetchedPaths).toEqual([]);

    const completed = await client.complete({
      ref: {
        type: "ref/resource",
        uri: "chaos://workflows/{id}/definition",
      },
      argument: { name: "id", value: "wf-a" },
    });
    expect(completed.completion.values).toEqual(["wf-alpha"]);
    expect(fetchedPaths).toEqual(["/api/v1/workflows"]);

    const resource = await client.readResource({
      uri: "chaos://runs/run-1/tasks",
    });
    const resourceText = (resource.contents[0] as { text: string }).text;
    expect(JSON.parse(resourceText).tasks[0].task_id).toBe("build");
    expect(fetchedPaths).toEqual([
      "/api/v1/workflows",
      "/api/v1/runs/run-1/tasks",
    ]);
    expect(backendAuthorizations).toEqual([
      "Bearer user.scoped-key",
      "Bearer user.scoped-key",
    ]);
    await client.close();
  });
});
