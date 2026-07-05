import http from "node:http";
import type { AddressInfo } from "node:net";
import { afterEach, describe, expect, it } from "vitest";
import type { ChaosMcpConfig } from "./config.js";
import { assertHttpBindAllowed, createHttpServer } from "./http.js";

const servers: http.Server[] = [];

function config(overrides: Partial<ChaosMcpConfig> = {}): ChaosMcpConfig {
  return {
    baseUrl: "http://127.0.0.1:9618",
    apiKey: "server-key",
    transport: "http",
    httpHost: "127.0.0.1",
    httpPort: 9700,
    allowRemoteHttp: false,
    maxHttpBodyBytes: 1024,
    protectedEnvironments: ["prod", "production"],
    allowProtectedWrites: false,
    maxToolCalls: 0,
    requestTimeoutMs: 30_000,
    ...overrides,
  };
}

async function listen(server: http.Server): Promise<number> {
  servers.push(server);
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  return (server.address() as AddressInfo).port;
}

async function close(server: http.Server): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    server.close((err) => (err ? reject(err) : resolve()));
  });
}

async function request(
  server: http.Server,
  options: http.RequestOptions,
  body?: string,
): Promise<{ status: number; body: string }> {
  const port = await listen(server);
  return await new Promise((resolve, reject) => {
    const req = http.request(
      {
        host: "127.0.0.1",
        port,
        path: "/mcp",
        method: "POST",
        ...options,
      },
      (res) => {
        const chunks: Buffer[] = [];
        res.on("data", (chunk) => chunks.push(chunk as Buffer));
        res.on("end", () =>
          resolve({
            status: res.statusCode ?? 0,
            body: Buffer.concat(chunks).toString("utf8"),
          }),
        );
      },
    );
    req.on("error", reject);
    if (body) req.write(body);
    req.end();
  });
}

afterEach(async () => {
  await Promise.all(servers.splice(0).map(close));
});

describe("HTTP MCP guardrails", () => {
  it("requires per-request bearer auth and does not fall back to the server key", async () => {
    const server = createHttpServer(config({ apiKey: "must-not-fallback" }));

    const res = await request(server, {});

    expect(res.status).toBe(401);
    expect(res.body).toContain("missing Authorization bearer token");
  });

  it("rejects oversized MCP request bodies before tool dispatch", async () => {
    const server = createHttpServer(config({ maxHttpBodyBytes: 8 }));

    const res = await request(
      server,
      { headers: { authorization: "Bearer user-key" } },
      JSON.stringify({ jsonrpc: "2.0", method: "tools/list", id: 1 }),
    );

    expect(res.status).toBe(413);
    expect(res.body).toContain("request body too large");
  });

  it("requires an explicit opt-in for non-loopback HTTP binds", () => {
    expect(() =>
      assertHttpBindAllowed(config({ httpHost: "0.0.0.0" })),
    ).toThrow("--allow-remote-http");

    expect(() =>
      assertHttpBindAllowed(
        config({ httpHost: "0.0.0.0", allowRemoteHttp: true }),
      ),
    ).not.toThrow();
  });
});
