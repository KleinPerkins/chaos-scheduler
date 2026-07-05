import http from "node:http";
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
): Promise<{ baseUrl: string; config: ReturnType<typeof configFromEnv> }> {
  const config = {
    ...configFromEnv({ CHAOS_SCHEDULER_API_KEY: "server.fallback" }),
    httpPort: 0,
    ...overrides,
  };
  const server = createHttpServer(config);
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
});
