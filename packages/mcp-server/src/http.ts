/**
 * Remote/team transport: a single Streamable HTTP endpoint (`/mcp`), stateless.
 *
 * Each request builds a fresh server + transport (no session store), so this
 * scales horizontally and is safe behind a load balancer. HTTP mode requires a
 * per-request `Authorization: Bearer` header; it never falls back to the
 * server process key.
 */
import http from "node:http";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import type { ChaosMcpConfig } from "./config.js";
import { makeClient } from "./factory.js";
import { ToolBudget } from "./guardrails.js";
import { buildServer } from "./server.js";

const MCP_PATH = "/mcp";

class HttpRequestError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

function bearer(header: string | undefined): string | undefined {
  if (!header) return undefined;
  const m = /^Bearer\s+(.+)$/i.exec(header.trim());
  const token = m ? m[1]!.trim() : undefined;
  return token && token.length > 0 ? token : undefined;
}

function normalizeHost(host: string | undefined): string | undefined {
  if (!host) return undefined;
  const value = host.trim().toLowerCase();
  if (!value) return undefined;
  if (value.startsWith("[")) return value.slice(1, value.indexOf("]"));
  return value.split(":")[0];
}

function isLoopbackHost(host: string | undefined): boolean {
  const normalized = normalizeHost(host);
  return (
    normalized === "localhost" ||
    normalized === "::1" ||
    normalized === "0:0:0:0:0:0:0:1" ||
    normalized === "127.0.0.1" ||
    normalized?.startsWith("127.") === true
  );
}

function ensureLocalBind(config: ChaosMcpConfig): void {
  if (!config.allowRemoteHttp && !isLoopbackHost(config.httpHost)) {
    throw new Error(
      `Refusing to bind HTTP MCP to non-loopback host ${config.httpHost}; pass --allow-remote-http to opt in`,
    );
  }
}

function hostAllowed(
  req: http.IncomingMessage,
  config: ChaosMcpConfig,
): boolean {
  return config.allowRemoteHttp || isLoopbackHost(req.headers.host);
}

async function readBody(
  req: http.IncomingMessage,
  maxBytes: number,
): Promise<unknown> {
  const chunks: Buffer[] = [];
  let total = 0;
  for await (const chunk of req) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    total += buffer.length;
    if (total > maxBytes) {
      throw new HttpRequestError(413, "request body too large");
    }
    chunks.push(buffer);
  }
  if (chunks.length === 0) return undefined;
  const text = Buffer.concat(chunks).toString("utf8");
  if (text.trim().length === 0) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    throw new HttpRequestError(400, "invalid JSON body");
  }
}

function sendJsonRpcError(
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
  sendJsonRpcError(res, 404, -32601, "not found");
}

/** Create (but do not start) the HTTP server. Exposed for tests. */
export function createHttpServer(
  config: ChaosMcpConfig,
  sharedBudget?: ToolBudget,
): http.Server {
  const budget = sharedBudget ?? new ToolBudget(config.maxToolCalls);
  return http.createServer((req, res) => {
    void handle(req, res, config, budget);
  });
}

async function handle(
  req: http.IncomingMessage,
  res: http.ServerResponse,
  config: ChaosMcpConfig,
  sharedBudget: ToolBudget,
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

  if (req.method !== "POST") {
    res.writeHead(405, {
      "content-type": "application/json",
      allow: "POST",
    });
    res.end(JSON.stringify({ error: "method not allowed" }));
    return;
  }

  if (!hostAllowed(req, config)) {
    sendJsonRpcError(res, 403, -32003, "host not allowed");
    return;
  }

  const apiKey = bearer(req.headers.authorization);
  if (!apiKey) {
    sendJsonRpcError(res, 401, -32001, "missing bearer token");
    return;
  }

  try {
    const parsedBody = await readBody(req, config.httpMaxBodyBytes);
    const client = makeClient(config, apiKey);
    const server = buildServer({ client, config, budget: sharedBudget });
    // Stateless: a new transport per request (sessionIdGenerator: undefined).
    const transport = new StreamableHTTPServerTransport({
      sessionIdGenerator: undefined,
    });
    res.on("close", () => {
      void transport.close();
      void server.close();
    });

    await server.connect(transport);
    await transport.handleRequest(req, res, parsedBody);
  } catch (err) {
    if (err instanceof HttpRequestError) {
      sendJsonRpcError(res, err.status, -32000, err.message);
      return;
    }
    process.stderr.write(`[chaos-mcp] request error: ${String(err)}\n`);
    if (!res.headersSent) {
      sendJsonRpcError(res, 500, -32603, "internal error");
    }
  }
}

export async function runHttp(config: ChaosMcpConfig): Promise<http.Server> {
  ensureLocalBind(config);
  const sharedBudget = new ToolBudget(config.maxToolCalls);
  const httpServer = createHttpServer(config, sharedBudget);
  await new Promise<void>((resolve) => {
    httpServer.listen(config.httpPort, config.httpHost, resolve);
  });
  process.stderr.write(
    `[chaos-mcp] Streamable HTTP transport on http://${config.httpHost}:${config.httpPort}${MCP_PATH} -> ${config.baseUrl}\n`,
  );
  return httpServer;
}
