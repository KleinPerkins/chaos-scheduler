/**
 * `@chaos-scheduler/mcp-server` — the Chaos MCP server exposing the scheduler to
 * MCP clients (Cursor, Cloud Agents) as tools, resources, and prompts.
 *
 * Programmatic entry points; the executable lives in `cli.ts` (`chaos-mcp-server`).
 */

export { buildServer, SERVER_NAME, SERVER_VERSION } from "./server.js";
export type { ServerDeps } from "./server.js";
export { runStdio } from "./stdio.js";
export { runHttp, createHttpServer } from "./http.js";
export { makeClient } from "./factory.js";
export {
  configFromEnv,
  applyCliOverrides,
  type ChaosMcpConfig,
  type McpTransport,
} from "./config.js";
export {
  ToolBudget,
  GuardrailError,
  assertEnvironmentWritable,
} from "./guardrails.js";
