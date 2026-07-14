import {
  McpServer,
  ResourceTemplate,
} from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  ErrorCode,
  McpError,
  type ReadResourceResult,
} from "@modelcontextprotocol/sdk/types.js";
import {
  ChaosApiError,
  type ChaosSchedulerClient,
  type Workflow,
} from "@chaos-scheduler/sdk";
import { z, type ZodType } from "zod";
import {
  GitPullOperatorConfigSchema,
  IntegrationSchema,
  QueueConfigSchema,
  TriggerConfigSchema,
  WorkflowSpecSchema,
  CursorAgentOperatorConfigSchema,
} from "./authoring-schemas.js";
import { projectStoredJson } from "./resource-projection.js";

const RESOURCE_NOT_FOUND = -32002;
const CONTRACT = { version: "v1", view: "stored_config" } as const;

export function jsonResource(uri: URL, data: unknown): ReadResourceResult {
  return {
    contents: [
      {
        uri: uri.href,
        mimeType: "application/json",
        text: JSON.stringify(data, null, 2),
      },
    ],
  };
}

function mapResourceError(uri: URL, err: unknown): McpError {
  if (err instanceof McpError) return err;
  if (err instanceof ChaosApiError && err.isNotFound) {
    return new McpError(RESOURCE_NOT_FOUND, "Resource not found", {
      uri: uri.href,
    });
  }
  if (err instanceof ChaosApiError) {
    return new McpError(
      ErrorCode.InternalError,
      "Scheduler resource read failed",
      { uri: uri.href, status: err.status },
    );
  }
  return new McpError(
    ErrorCode.InternalError,
    "Scheduler resource read failed",
    { uri: uri.href },
  );
}

export async function readJsonResource<T>(
  uri: URL,
  load: () => Promise<T>,
  project: (data: T) => unknown = (data) => data,
): Promise<ReadResourceResult> {
  try {
    return jsonResource(uri, project(await load()));
  } catch (err) {
    throw mapResourceError(uri, err);
  }
}

export function resourceIdentifier(uri: URL, value: unknown): string {
  const id = Array.isArray(value) ? value.join("/") : String(value ?? "");
  if (!id.trim()) {
    throw new McpError(ErrorCode.InvalidParams, "Invalid resource identifier", {
      uri: uri.href,
    });
  }
  return id;
}

function schemaPayload(schema: ZodType, extra: Record<string, unknown> = {}) {
  return {
    ...CONTRACT,
    authority:
      "Permissive MCP authoring mirror; Rust/backend validation and persistence normalization remain authoritative.",
    schema: z.toJSONSchema(schema),
    ...extra,
  };
}

const AUTHORING_RESOURCE = {
  ...CONTRACT,
  purpose:
    "Progressive-disclosure entrypoint for authoring and updating scheduler workflows.",
  start_here: [
    "chaos://workflows/index",
    "chaos://catalog",
    "chaos://guides/workflows",
  ],
  safe_flow: [
    "Read chaos://workflows/index before registering; registration is non-idempotent.",
    "Read the relevant guide and schema before drafting stored configuration.",
    "Read chaos://workflows/{id}/definition before updates.",
    "Prefer patch_workflow_spec for spec updates that may contain redacted secrets.",
    "Confirm the proposed write with the operator.",
  ],
};

const CATALOG_RESOURCE = {
  ...CONTRACT,
  resources: {
    guides: [
      "chaos://guides/workflows",
      "chaos://guides/webhooks",
      "chaos://guides/integrations",
    ],
    schemas: [
      "chaos://schemas/workflow-spec",
      "chaos://schemas/triggers",
      "chaos://schemas/queue",
      "chaos://schemas/integrations",
    ],
    state: ["chaos://workflows/index", "chaos://workflows/{id}/definition"],
  },
  known_types: {
    workflow_kinds: ["generic", "typed"],
    trigger_kinds: ["cron", "file_arrival", "asset_update", "on_completion"],
    on_completion_status_filter:
      "Optional non-empty subset of success|failed; defaults to success.",
    inbound_dispatch:
      "dispatch_workflow is an API trigger, not a stored trigger_config kind.",
    completion_actions: [
      "email",
      "webhook",
      "run_workflow",
      "desktop_notification",
    ],
    typed_operators: ["git_pull", "cursor_agent"],
  },
};

const WORKFLOW_GUIDE = {
  ...CONTRACT,
  title: "Workflow authoring",
  steps: [
    "Read chaos://workflows/index and reuse/update an existing workflow when possible.",
    "Choose generic for step DAGs or typed for a known built-in operator.",
    "Keep trigger_config and queue_config serialized as JSON strings at tool/REST boundaries.",
    "Use patch_workflow_spec for partial spec changes; use set_workflow_spec only for intentional full replacement.",
  ],
  cautions: [
    "register_workflow is non-idempotent and can create duplicates.",
    "Schemas describe stored configuration, not effective configuration or proof of runtime enforcement.",
    "MCP validation preserves additive fields, but the current backend may normalize fields it does not support.",
    "patch_workflow_spec is read-merge-write; serialize concurrent writers until backend compare-and-swap support exists.",
    "Redacted secrets in replaced arrays require a unique id or unchanged webhook URL; ambiguous restoration fails closed.",
    "The backend performs authoritative validation and may reject a schema-valid draft.",
  ],
};

const WEBHOOK_GUIDE = {
  ...CONTRACT,
  title: "Inbound dispatch and outbound completion webhooks",
  inbound: {
    tool: "dispatch_workflow",
    signature:
      "HMAC-SHA256 over METHOD\nPATH\nTIMESTAMP\nSHA256(body), sent with X-Chaos-Timestamp, X-Chaos-Event-Id, and X-Chaos-Signature.",
    retry_fields: ["idempotency_key", "event_id", "timestamp"],
    boundary:
      "inbound_webhook_secret configuration and status are unavailable through MCP.",
  },
  outbound: {
    location: "WorkflowSpec on_success/on_failure webhook action",
    signature:
      "HMAC-SHA256 over the raw POST body with X-Chaos-Event; this differs from inbound dispatch signing.",
    secret_handling:
      "Resource definitions always redact webhook secret values as __redacted__.",
  },
};

const INTEGRATION_GUIDE = {
  ...CONTRACT,
  title: "Email and typed-operator integrations",
  email: {
    tools: [
      "list_email_profiles",
      "create_email_profile",
      "update_email_profile",
      "set_workflow_email_profile",
    ],
    secret_rule:
      "smtp_password is masked on reads; echo the mask to preserve it on profile updates.",
  },
  typed_operators: {
    git_pull:
      "Requires path; can clone with repo_url or pull an existing repository.",
    cursor_agent:
      "Requires prompt; cloud mode also requires repository. API credentials are resolved by named secret.",
  },
  boundary:
    "This catalog mirrors known stored fields; backend validation and runtime availability are authoritative.",
};

function workflowIndex(workflows: Workflow[]) {
  return {
    ...CONTRACT,
    workflows: workflows.map((workflow) => ({
      id: workflow.id,
      name: workflow.name,
      environment: workflow.environment,
      enabled: workflow.enabled,
      kind: workflow.kind,
      managed_externally: workflow.managed_externally,
      updated_at: workflow.updated_at,
    })),
  };
}

function definitionPart(value: unknown) {
  const projected = projectStoredJson(value);
  return {
    parse_status: projected.status,
    value: projected.value,
  };
}

function objectValue(value: unknown): Record<string, unknown> | undefined {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

export function workflowDefinition(workflow: Workflow) {
  const source = workflow as unknown as Record<string, unknown>;
  const spec = definitionPart(workflow.spec_json);
  const triggers = definitionPart(workflow.trigger_config);
  const queue = definitionPart(workflow.queue_config);
  const specObject =
    spec.parse_status === "parsed" ? objectValue(spec.value) : undefined;
  const completionActions = {
    parse_status: spec.parse_status,
    on_success: specObject?.on_success ?? null,
    on_failure: specObject?.on_failure ?? null,
  };
  const warnings: string[] = [];
  if (spec.parse_status === "invalid") {
    warnings.push(
      "Stored workflow spec JSON is invalid or exceeds safe limits.",
    );
  }
  if (triggers.parse_status === "invalid") {
    warnings.push(
      "Stored trigger configuration JSON is invalid or exceeds safe limits.",
    );
  }
  if (queue.parse_status === "invalid") {
    warnings.push(
      "Stored queue configuration JSON is invalid or exceeds safe limits.",
    );
  }

  return {
    ...CONTRACT,
    workflow: {
      id: workflow.id,
      name: workflow.name,
      description: workflow.description,
      script_path: workflow.script_path,
      cron_schedule: workflow.cron_schedule,
      enabled: workflow.enabled,
      async_mode: workflow.async_mode,
      email_on_failure: workflow.email_on_failure,
      environment: workflow.environment,
      managed_externally: workflow.managed_externally,
      kind: workflow.kind,
      domain: workflow.domain,
      timezone: workflow.timezone,
      email_profile_id: source.email_profile_id ?? null,
      updated_at: workflow.updated_at,
    },
    stored_config: {
      spec,
      triggers,
      queue,
      completion_actions: completionActions,
    },
    warnings,
    boundaries: [
      "This is stored configuration, not effective configuration or proof of runtime enforcement.",
      "Inbound webhook secret configuration and status are unavailable through MCP.",
      "Inbound dispatch and outbound completion webhooks use different signature schemes.",
      "Workflow registration is non-idempotent; check chaos://workflows/index first.",
      "MCP validation preserves additive fields, but the current backend may normalize unsupported fields.",
      "Spec patching is read-merge-write; concurrent writers must be serialized.",
    ],
  };
}

function registerStaticResource(
  server: McpServer,
  name: string,
  uri: string,
  title: string,
  description: string,
  data: unknown,
) {
  server.registerResource(
    name,
    uri,
    { title, description, mimeType: "application/json" },
    async (requestedUri) => jsonResource(requestedUri, data),
  );
}

export function registerAuthoringResources(
  server: McpServer,
  client: ChaosSchedulerClient,
) {
  registerStaticResource(
    server,
    "authoring",
    "chaos://authoring",
    "Authoring entrypoint",
    "Start here before registering or updating workflows",
    AUTHORING_RESOURCE,
  );
  registerStaticResource(
    server,
    "catalog",
    "chaos://catalog",
    "Authoring catalog",
    "Known workflow, trigger, action, and integration capabilities",
    CATALOG_RESOURCE,
  );
  registerStaticResource(
    server,
    "guide-workflows",
    "chaos://guides/workflows",
    "Workflow authoring guide",
    "Safe workflow registration and update flow",
    WORKFLOW_GUIDE,
  );
  registerStaticResource(
    server,
    "guide-webhooks",
    "chaos://guides/webhooks",
    "Webhook guide",
    "Inbound and outbound webhook contracts",
    WEBHOOK_GUIDE,
  );
  registerStaticResource(
    server,
    "guide-integrations",
    "chaos://guides/integrations",
    "Integration guide",
    "Email profiles and typed operator integrations",
    INTEGRATION_GUIDE,
  );
  registerStaticResource(
    server,
    "schema-workflow-spec",
    "chaos://schemas/workflow-spec",
    "Workflow spec schema",
    "Permissive MCP authoring schema for stored workflow specs",
    schemaPayload(WorkflowSpecSchema),
  );
  registerStaticResource(
    server,
    "schema-triggers",
    "chaos://schemas/triggers",
    "Trigger schema",
    "Permissive MCP authoring schema for stored trigger configuration",
    schemaPayload(TriggerConfigSchema),
  );
  registerStaticResource(
    server,
    "schema-queue",
    "chaos://schemas/queue",
    "Queue schema",
    "Permissive MCP authoring schema for stored queue configuration",
    schemaPayload(QueueConfigSchema),
  );
  registerStaticResource(
    server,
    "schema-integrations",
    "chaos://schemas/integrations",
    "Integration schema",
    "Completion action, email profile, and typed operator schemas",
    schemaPayload(IntegrationSchema, {
      known_operator_schemas: {
        git_pull: z.toJSONSchema(GitPullOperatorConfigSchema),
        cursor_agent: z.toJSONSchema(CursorAgentOperatorConfigSchema),
      },
    }),
  );

  server.registerResource(
    "workflow-index",
    "chaos://workflows/index",
    {
      title: "Workflow index",
      description: "Lightweight workflow identities for duplicate checks",
      mimeType: "application/json",
    },
    async (uri) =>
      readJsonResource(uri, () => client.listWorkflows(), workflowIndex),
  );

  server.registerResource(
    "workflow-definition",
    new ResourceTemplate("chaos://workflows/{id}/definition", {
      list: undefined,
    }),
    {
      title: "Workflow stored definition",
      description:
        "Stored spec, triggers, queue, completion actions, and parse warnings",
      mimeType: "application/json",
    },
    async (uri, variables) => {
      const id = resourceIdentifier(uri, variables.id);
      return readJsonResource(
        uri,
        () => client.getWorkflow(id),
        workflowDefinition,
      );
    },
  );
}
