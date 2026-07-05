#!/usr/bin/env node
/**
 * `chaos-mcp-server` executable.
 *
 * Usage:
 *   chaos-mcp-server                 # stdio (default), for local Cursor use
 *   chaos-mcp-server --http          # Streamable HTTP, for remote/team use
 *   chaos-mcp-server --http --port 9700 --host 0.0.0.0 --allow-remote-http
 *
 * Configuration via CHAOS_SCHEDULER_* env vars (see README). At minimum set
 * CHAOS_SCHEDULER_URL and CHAOS_SCHEDULER_API_KEY.
 */
import { applyCliOverrides, configFromEnv } from "./config.js";
import { runHttp } from "./http.js";
import { runStdio } from "./stdio.js";

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  if (argv.includes("--help") || argv.includes("-h")) {
    process.stdout.write(
      [
        "chaos-mcp-server — Chaos Scheduler MCP server",
        "",
        "Options:",
        "  --stdio                     Use stdio transport (default)",
        "  --http                      Use Streamable HTTP transport",
        "  --host <host>               HTTP bind host (default 127.0.0.1)",
        "  --port <port>               HTTP bind port (default 9700)",
        "  --allow-remote-http         Permit non-loopback HTTP binds",
        "  --max-http-body-bytes <n>   HTTP MCP body cap (default 1048576)",
        "  --url <baseUrl>             Scheduler API base URL (default http://127.0.0.1:9618)",
        "  --allow-protected-writes    Permit writes to protected environments",
        "",
        "Env: CHAOS_SCHEDULER_URL, CHAOS_SCHEDULER_API_KEY,",
        "     CHAOS_SCHEDULER_MCP_TRANSPORT, CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS, ...",
        "",
      ].join("\n"),
    );
    return;
  }

  const config = applyCliOverrides(configFromEnv(), argv);
  if (config.transport === "http") {
    await runHttp(config);
  } else {
    await runStdio(config);
  }
}

main().catch((err) => {
  process.stderr.write(`[chaos-mcp] fatal: ${String(err)}\n`);
  process.exit(1);
});
