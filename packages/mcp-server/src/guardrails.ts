/**
 * Tool-budgeting and prod-safety guardrails.
 *
 * These sit between the MCP client (e.g. Cursor's agent) and the scheduler so a
 * runaway agent loop cannot spam the API and destructive writes cannot silently
 * hit a protected environment.
 */

import type { ChaosMcpConfig } from "./config.js";

/** Thrown by a guardrail; surfaced to the MCP client as a tool error. */
export class GuardrailError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "GuardrailError";
  }
}

/** Simple per-instance call counter enforcing {@link ChaosMcpConfig.maxToolCalls}. */
export class ToolBudget {
  private used = 0;
  constructor(private readonly max: number) {}

  /** Count one invocation; throws once the budget is exhausted. */
  consume(toolName: string): void {
    if (this.max <= 0) return; // unlimited
    this.used += 1;
    if (this.used > this.max) {
      throw new GuardrailError(
        `tool-call budget exhausted (${this.max}); refusing '${toolName}'. ` +
          `Raise CHAOS_SCHEDULER_MCP_MAX_TOOL_CALLS or start a new session.`,
      );
    }
  }

  get remaining(): number {
    return this.max <= 0
      ? Number.POSITIVE_INFINITY
      : Math.max(0, this.max - this.used);
  }
}

/**
 * Reject writes targeting a protected environment unless explicitly allowed.
 * `env` is the environment name a write would affect.
 */
export function assertEnvironmentWritable(
  env: string | undefined,
  config: ChaosMcpConfig,
): void {
  if (!env) return;
  if (config.allowProtectedWrites) return;
  if (config.protectedEnvironments.includes(env.trim().toLowerCase())) {
    throw new GuardrailError(
      `environment '${env}' is protected; refusing the write. ` +
        `Set CHAOS_SCHEDULER_MCP_ALLOW_PROTECTED_WRITES=1 (or pass --allow-protected-writes) to override.`,
    );
  }
}
