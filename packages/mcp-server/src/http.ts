/**
 * Remote/team transport: a single Streamable HTTP endpoint (`/mcp`), stateless.
 *
 * Each request builds a fresh server + transport (no session store), so this
 * scales horizontally and is safe behind a load balancer. The per-request API
 * key is taken from the incoming `Authorization: Bearer` header (falling back to
 * the server's configured key), so a team deployment can pass each user's own
 * scoped scheduler key straight through.
 */
import http from "node:http";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import type { ChaosMcpConfig } from "./config.js";
import { makeClient } from "./factory.js";
import { buildServer } from "./server.js";

const MCP_PATH = "/mcp";

function bearer(header: string | undefined): string | undefined {
  if (!header) return undefined;
  const m = /^Bearer\s+(.+)$/i.exec(header.trim());
  return m ? m[1]!.trim() : undefined;
}

async function readBody(req: http.IncomingMessage): Promise<unknown> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) chunks.push(chunk as Buffer);
  if (chunks.length === 0) return undefined;
  const text = Buffer.concat(chunks).toString("utf8");
  if (text.trim().length === 0) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

function notFound(res: http.ServerResponse): void {
  res.writeHead(404, { "content-type": "application/json" });
  res.end(
    JSON.stringify({
      jsonrpc: "2.0",
      error: { code: -32601, message: "not found" },
      id: null,
    }),
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
    const parsedBody = req.method === "POST" ? await readBody(req) : undefined;
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
  const httpServer = createHttpServer(config);
  await new Promise<void>((resolve) => {
    httpServer.listen(config.httpPort, config.httpHost, resolve);
  });
  process.stderr.write(
    `[chaos-mcp] Streamable HTTP transport on http://${config.httpHost}:${config.httpPort}${MCP_PATH} → ${config.baseUrl}\n`,
  );
  return httpServer;
}
