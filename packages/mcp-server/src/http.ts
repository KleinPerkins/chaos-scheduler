/**
 * Remote/team transport: a single Streamable HTTP endpoint (`/mcp`), stateless.
 *
 * Each request builds a fresh server + transport (no session store), so this
 * scales horizontally and is safe behind a load balancer. The per-request API
 * key is taken from the incoming `Authorization: Bearer` header; HTTP mode never
 * falls back to the server's configured key.
 */
import http from "node:http";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import type { ChaosMcpConfig } from "./config.js";
import { makeClient } from "./factory.js";
import { buildServer } from "./server.js";

const MCP_PATH = "/mcp";
const LOOPBACK_HOSTS = new Set(["127.0.0.1", "::1", "localhost"]);

function bearer(header: string | undefined): string | undefined {
  if (!header) return undefined;
  const m = /^Bearer\s+(.+)$/i.exec(header.trim());
  return m ? m[1]!.trim() : undefined;
}

class BodyTooLargeError extends Error {}

async function readBody(
  req: http.IncomingMessage,
  maxBytes: number,
): Promise<unknown> {
  const chunks: Buffer[] = [];
  let total = 0;
  for await (const chunk of req) {
    const buf = chunk as Buffer;
    total += buf.byteLength;
    if (total > maxBytes) {
      throw new BodyTooLargeError("HTTP MCP request body too large");
    }
    chunks.push(buf);
  }
  if (chunks.length === 0) return undefined;
  const text = Buffer.concat(chunks).toString("utf8");
  if (text.trim().length === 0) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

function jsonError(
  res: http.ServerResponse,
  status: number,
  code: number,
  message: string,
): void {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(
    JSON.stringify({
      jsonrpc: "2.0",
      error: { code, message },
      id: null,
    }),
  );
}

function notFound(res: http.ServerResponse): void {
  jsonError(res, 404, -32601, "not found");
}

function isLoopbackHost(host: string): boolean {
  const normalized = host.trim().toLowerCase();
  if (LOOPBACK_HOSTS.has(normalized)) return true;
  if (normalized.startsWith("127.")) return true;
  return normalized === "[::1]";
}

export function assertHttpBindAllowed(config: ChaosMcpConfig): void {
  if (config.allowRemoteHttp || isLoopbackHost(config.httpHost)) return;
  throw new Error(
    `Refusing to bind HTTP MCP to ${config.httpHost}; pass --allow-remote-http to opt in`,
  );
}

/** Create (but do not start) the HTTP server. Exposed for tests. */
export function createHttpServer(config: ChaosMcpConfig): http.Server {
  return http.createServer((req, res) => {
    void handle(req, res, config);
  });
}

async function handle(
  req: http.IncomingMessage,
  res: http.ServerResponse,
  config: ChaosMcpConfig,
): Promise<void> {
  const url = new URL(
    req.url ?? "/",
    `http://${req.headers.host ?? "localhost"}`,
  );

  if (url.pathname === "/health") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ status: "ok", transport: "streamable-http" }));
    return;
  }

  if (url.pathname !== MCP_PATH) {
    notFound(res);
    return;
  }

  const apiKey = bearer(req.headers["authorization"]);
  if (!apiKey) {
    jsonError(res, 401, -32001, "missing Authorization bearer token");
    return;
  }

  let parsedBody: unknown;
  try {
    parsedBody =
      req.method === "POST"
        ? await readBody(req, config.maxHttpBodyBytes)
        : undefined;
  } catch (err) {
    if (err instanceof BodyTooLargeError) {
      jsonError(res, 413, -32002, "request body too large");
      return;
    }
    throw err;
  }

  const client = makeClient(config, apiKey);
  const server = buildServer({ client, config });
  // Stateless: a new transport per request (sessionIdGenerator: undefined).
  const transport = new StreamableHTTPServerTransport({
    sessionIdGenerator: undefined,
  });
  res.on("close", () => {
    void transport.close();
    void server.close();
  });

  try {
    await server.connect(transport);
    await transport.handleRequest(req, res, parsedBody);
  } catch (err) {
    process.stderr.write(`[chaos-mcp] request error: ${String(err)}\n`);
    if (!res.headersSent) {
      res.writeHead(500, { "content-type": "application/json" });
      res.end(
        JSON.stringify({
          jsonrpc: "2.0",
          error: { code: -32603, message: "internal error" },
          id: null,
        }),
      );
    }
  }
}

export async function runHttp(config: ChaosMcpConfig): Promise<http.Server> {
  assertHttpBindAllowed(config);
  const httpServer = createHttpServer(config);
  await new Promise<void>((resolve) => {
    httpServer.listen(config.httpPort, config.httpHost, resolve);
  });
  process.stderr.write(
    `[chaos-mcp] Streamable HTTP transport on http://${config.httpHost}:${config.httpPort}${MCP_PATH} → ${config.baseUrl}\n`,
  );
  return httpServer;
}
