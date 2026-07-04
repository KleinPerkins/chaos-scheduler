/** Local stdio transport: one Cursor-managed process, one long-lived server. */
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import type { ChaosMcpConfig } from "./config.js";
import { makeClient } from "./factory.js";
import { buildServer } from "./server.js";

export async function runStdio(config: ChaosMcpConfig): Promise<void> {
  const client = makeClient(config);
  const server = buildServer({ client, config });
  const transport = new StdioServerTransport();
  await server.connect(transport);
  // Log to stderr only — stdout is the JSON-RPC channel and must stay clean.
  process.stderr.write(
    `[chaos-mcp] stdio transport connected → ${config.baseUrl}\n`,
  );
}
