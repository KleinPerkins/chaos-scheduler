/**
 * Runtime configuration for the Chaos MCP server, resolved from environment
 * variables (and, for the CLI, overridable by flags).
 *
 * All env vars use the `CHAOS_SCHEDULER_` prefix to match the backend branding
 * module (`src-tauri/src/branding.rs`).
 */

export type McpTransport = "stdio" | "http";

export interface ChaosMcpConfig {
  /** Base URL of the scheduler REST API. */
  baseUrl: string;
  /** Default scoped API key (used for stdio and as the HTTP fallback). */
  apiKey?: string;
  /** Selected transport. */
  transport: McpTransport;
  /** HTTP bind host (http transport only). */
  httpHost: string;
  /** HTTP bind port (http transport only). */
  httpPort: number;
  /**
   * Environments treated as protected: write tools targeting them are blocked
   * unless {@link allowProtectedWrites} is set. A guardrail for prod safety.
   */
  protectedEnvironments: string[];
  /** Allow writes to protected environments. */
  allowProtectedWrites: boolean;
  /** Max tool invocations per server instance (0 = unlimited). */
  maxToolCalls: number;
  /** Per-request timeout forwarded to the SDK client (ms). */
  requestTimeoutMs: number;
}

function envList(value: string | undefined): string[] {
  if (!value) return [];
  return value
    .split(",")
    .map((s) => s.trim().toLowerCase())
    .filter((s) => s.length > 0);
}

function envBool(value: string | undefined): boolean {
  if (!value) return false;
  return ["1", "true", "yes", "on"].includes(value.trim().toLowerCase());
}

function envInt(value: string | undefined, fallback: number): number {
  if (!value) return fallback;
  const n = Number.parseInt(value, 10);
  return Number.isFinite(n) ? n : fallback;
}

const DEFAULTS = {
  baseUrl: "http://127.0.0.1:9618",
  httpHost: "127.0.0.1",
  httpPort: 9700,
  protectedEnvironments: ["prod", "production"],
  requestTimeoutMs: 30_000,
};

/** Resolve configuration from `process.env` (with sensible defaults). */
export function configFromEnv(
  env: NodeJS.ProcessEnv = process.env,
): ChaosMcpConfig {
  const protectedRaw = env.CHAOS_SCHEDULER_MCP_PROTECTED_ENVIRONMENTS;
  return {
    baseUrl: env.CHAOS_SCHEDULER_URL?.trim() || DEFAULTS.baseUrl,
    apiKey: env.CHAOS_SCHEDULER_API_KEY?.trim() || undefined,
    transport:
      (env.CHAOS_SCHEDULER_MCP_TRANSPORT?.trim().toLowerCase() as McpTransport) ||
      "stdio",
    httpHost: env.CHAOS_SCHEDULER_MCP_HTTP_HOST?.trim() || DEFAULTS.httpHost,
    httpPort: envInt(env.CHAOS_SCHEDULER_MCP_HTTP_PORT, DEFAULTS.httpPort),
    protectedEnvironments:
      protectedRaw === undefined
        ? DEFAULTS.protectedEnvironments
        : envList(protectedRaw),
    allowProtectedWrites: envBool(
      env.CHAOS_SCHEDULER_MCP_ALLOW_PROTECTED_WRITES,
    ),
    maxToolCalls: envInt(env.CHAOS_SCHEDULER_MCP_MAX_TOOL_CALLS, 0),
    requestTimeoutMs: envInt(
      env.CHAOS_SCHEDULER_MCP_REQUEST_TIMEOUT_MS,
      DEFAULTS.requestTimeoutMs,
    ),
  };
}

/** Apply parsed CLI flags on top of a base config. */
export function applyCliOverrides(
  base: ChaosMcpConfig,
  argv: string[],
): ChaosMcpConfig {
  const cfg = { ...base };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    switch (arg) {
      case "--http":
        cfg.transport = "http";
        break;
      case "--stdio":
        cfg.transport = "stdio";
        break;
      case "--host":
        cfg.httpHost = argv[++i] ?? cfg.httpHost;
        break;
      case "--port":
        cfg.httpPort = envInt(argv[++i], cfg.httpPort);
        break;
      case "--url":
        cfg.baseUrl = argv[++i] ?? cfg.baseUrl;
        break;
      case "--allow-protected-writes":
        cfg.allowProtectedWrites = true;
        break;
      default:
        break;
    }
  }
  return cfg;
}
