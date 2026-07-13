import type { Workflow } from "@chaos-scheduler/sdk";

export const REDACTED_SECRET = "__redacted__";
export const INVALID_STORED_JSON = "__redacted_invalid_json__";

const MAX_STORED_JSON_BYTES = 256 * 1024;
const MAX_STORED_JSON_DEPTH = 32;
const MAX_STORED_JSON_NODES = 10_000;

const SENSITIVE_KEYS = new Set([
  "secret",
  "signature_secret",
  "cursor_api_key",
  "smtp_password",
]);

const STORED_JSON_FIELDS = new Set([
  "spec_json",
  "trigger_config",
  "queue_config",
]);

const WORKFLOW_RESOURCE_FIELDS = [
  "id",
  "name",
  "description",
  "script_path",
  "cron_schedule",
  "enabled",
  "async_mode",
  "email_on_failure",
  "environment",
  "managed_externally",
  "kind",
  "spec_json",
  "domain",
  "timezone",
  "trigger_config",
  "queue_config",
  "email_profile_id",
  "last_run_at",
  "created_at",
  "updated_at",
] as const;

export type StoredJsonProjection =
  | { status: "absent"; value: null }
  | { status: "parsed"; value: unknown }
  | { status: "invalid"; value: null };

class ProjectionLimitError extends Error {}

interface ProjectionState {
  nodes: number;
}

function redactJsonValue(
  value: unknown,
  depth: number,
  state: ProjectionState,
): unknown {
  state.nodes += 1;
  if (depth > MAX_STORED_JSON_DEPTH || state.nodes > MAX_STORED_JSON_NODES) {
    throw new ProjectionLimitError("stored JSON exceeds projection limits");
  }

  if (Array.isArray(value)) {
    return value.map((item) => redactJsonValue(item, depth + 1, state));
  }
  if (value !== null && typeof value === "object") {
    // Null-prototype output keeps hostile-but-valid JSON keys such as
    // `__proto__` as inert data instead of invoking Object.prototype setters.
    const projected = Object.create(null) as Record<string, unknown>;
    for (const [key, item] of Object.entries(
      value as Record<string, unknown>,
    )) {
      projected[key] = SENSITIVE_KEYS.has(key.toLowerCase())
        ? REDACTED_SECRET
        : redactJsonValue(item, depth + 1, state);
    }
    return projected;
  }
  return value;
}

/**
 * Parse a stored JSON blob with deterministic size/shape limits and redact
 * known secret-bearing keys. Invalid content is never returned to the caller.
 */
export function projectStoredJson(value: unknown): StoredJsonProjection {
  if (value === null || value === undefined || value === "") {
    return { status: "absent", value: null };
  }
  if (
    typeof value !== "string" ||
    Buffer.byteLength(value, "utf8") > MAX_STORED_JSON_BYTES
  ) {
    return { status: "invalid", value: null };
  }

  try {
    const parsed = JSON.parse(value) as unknown;
    return {
      status: "parsed",
      value: redactJsonValue(parsed, 0, { nodes: 0 }),
    };
  } catch {
    return { status: "invalid", value: null };
  }
}

function projectStoredJsonString(value: unknown): unknown {
  if (value === null || value === undefined || value === "") return value;
  const projection = projectStoredJson(value);
  return projection.status === "parsed"
    ? JSON.stringify(projection.value)
    : INVALID_STORED_JSON;
}

/**
 * Stable, allowlisted workflow DTO for MCP resources. The existing resource
 * array/object envelope remains unchanged while nested configuration is made
 * safe for read-only context injection.
 */
export function projectWorkflowForResource(
  workflow: Workflow | Record<string, unknown>,
): Record<string, unknown> {
  const source = workflow as unknown as Record<string, unknown>;
  const projected: Record<string, unknown> = {};

  for (const field of WORKFLOW_RESOURCE_FIELDS) {
    if (!Object.prototype.hasOwnProperty.call(source, field)) continue;
    const value = source[field];
    projected[field] = STORED_JSON_FIELDS.has(field)
      ? projectStoredJsonString(value)
      : value;
  }

  return projected;
}

export function projectWorkflowsForResource(
  workflows: Workflow[],
): Array<Record<string, unknown>> {
  return workflows.map(projectWorkflowForResource);
}
