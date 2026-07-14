/**
 * Builds the Chaos MCP server: the tools, resources, and prompts that let an
 * MCP client (Cursor's agent, a Cloud Agent, etc.) drive the Chaos Scheduler.
 *
 * Everything is implemented on top of `@chaos-scheduler/sdk`, so the MCP surface
 * never duplicates REST/business logic — it mirrors exactly what the API
 * exposes. Write tools are wrapped with tool-budget + protected-environment
 * guardrails.
 */

import {
  McpServer,
  ResourceTemplate,
} from "@modelcontextprotocol/sdk/server/mcp.js";
import type { CallToolResult } from "@modelcontextprotocol/sdk/types.js";
import { ChaosApiError, ChaosSchedulerClient } from "@chaos-scheduler/sdk";
import { z } from "zod";
import { WorkflowSpecSchema } from "./authoring-schemas.js";
import type { ChaosMcpConfig } from "./config.js";
import {
  assertEnvironmentWritable,
  GuardrailError,
  ToolBudget,
} from "./guardrails.js";
import { SERVER_ICONS, SERVER_WEBSITE_URL } from "./icon.js";
import { applyWorkflowSpecMergePatch } from "./json-merge-patch.js";
import {
  projectWorkflowForResource,
  projectWorkflowsForResource,
} from "./resource-projection.js";
import {
  readJsonResource,
  registerAuthoringResources,
  resourceIdentifier,
  workflowDefinition,
  workflowResourceTemplate,
} from "./resources.js";

export const SERVER_NAME = "chaos-scheduler";
export const SERVER_VERSION = "0.1.0";

export interface ServerDeps {
  client: ChaosSchedulerClient;
  config: ChaosMcpConfig;
  /** Shared budget (defaults to one derived from config). */
  budget?: ToolBudget;
}

function jsonResult(data: unknown): CallToolResult {
  return { content: [{ type: "text", text: JSON.stringify(data, null, 2) }] };
}

function errorResult(err: unknown): CallToolResult {
  let message: string;
  if (err instanceof ChaosApiError) {
    message = `Scheduler API error (${err.status}): ${err.message}`;
  } else if (err instanceof GuardrailError) {
    message = `Guardrail blocked this call: ${err.message}`;
  } else if (err instanceof Error) {
    message = err.message;
  } else {
    message = String(err);
  }
  return { content: [{ type: "text", text: message }], isError: true };
}

/**
 * Construct a fully-registered {@link McpServer}. Callers connect it to a
 * transport (stdio or Streamable HTTP).
 */
export function buildServer(deps: ServerDeps): McpServer {
  const { client, config } = deps;
  const budget = deps.budget ?? new ToolBudget(config.maxToolCalls);

  // Protection only incurs an extra workflow fetch when actually configured.
  const protectionActive =
    config.protectedEnvironments.length > 0 && !config.allowProtectedWrites;
  const assertWorkflowWritable = async (id: string): Promise<void> => {
    if (!protectionActive) return;
    let env: string | undefined;
    try {
      const wf = await client.getWorkflow(id);
      env = wf.environment;
    } catch (err) {
      // Fail closed: if we cannot resolve the workflow's environment while
      // protection is active, refuse the write rather than silently
      // treating an unresolvable lookup as "unprotected" (the prior
      // behavior returned `undefined`, which made the guardrail a no-op).
      throw new GuardrailError(
        `could not resolve workflow '${id}' environment; refusing write while environment protection is active (${err instanceof Error ? err.message : String(err)})`,
      );
    }
    assertEnvironmentWritable(env, config);
  };

  const server = new McpServer(
    {
      name: SERVER_NAME,
      version: SERVER_VERSION,
      title: "Chaos Scheduler",
      websiteUrl: SERVER_WEBSITE_URL,
      icons: SERVER_ICONS,
    },
    {
      instructions:
        "Chaos Scheduler MCP server. Use these tools to register/inspect environments and " +
        "workflows, dispatch runs on demand (with idempotency keys), and read run results. " +
        "Prefer `enqueue_workflow` for manual runs; `run_workflow_now` is a deprecated alias " +
        "that also goes through admission control. Read-only state is also available as " +
        "`chaos://` resources. Start workflow authoring at `chaos://authoring`.",
    },
  );

  /** Wrap a tool handler with budget accounting + uniform error rendering. */
  const tool = <A extends z.ZodRawShape>(
    name: string,
    config_: {
      title: string;
      description: string;
      inputSchema?: A;
      readOnly?: boolean;
      destructive?: boolean;
    },
    handler: (args: z.infer<z.ZodObject<A>>) => Promise<CallToolResult>,
  ) => {
    server.registerTool(
      name,
      {
        title: config_.title,
        description: config_.description,
        inputSchema: config_.inputSchema,
        annotations: {
          title: config_.title,
          readOnlyHint: config_.readOnly ?? false,
          destructiveHint: config_.destructive ?? false,
        },
      },
      // The wrapper's runtime shape is correct; cast satisfies the SDK's
      // heavily-overloaded ToolCallback generic (which varies by inputSchema).
      (async (args: unknown) => {
        try {
          budget.consume(name);
          return await handler(args as z.infer<z.ZodObject<A>>);
        } catch (err) {
          return errorResult(err);
        }
      }) as never,
    );
  };

  // ---- Meta ----

  tool(
    "get_version",
    {
      title: "Get scheduler version",
      description:
        "Return the scheduler product name, version, DB schema version, and API version.",
      readOnly: true,
    },
    async () => jsonResult(await client.getVersion()),
  );

  tool(
    "health_check",
    {
      title: "Health check",
      description: "Liveness probe for the scheduler API.",
      readOnly: true,
    },
    async () => jsonResult(await client.getHealth()),
  );

  // ---- Environments ----

  tool(
    "list_environments",
    {
      title: "List environments",
      description: "List all execution environments (partitions/queue scopes).",
      readOnly: true,
    },
    async () => jsonResult(await client.listEnvironments()),
  );

  tool(
    "create_environment",
    {
      title: "Create environment",
      description:
        "Create a new execution environment. Blocked for protected environment names.",
      inputSchema: {
        name: z.string().describe("Unique environment name"),
        description: z.string().optional(),
        working_dir: z
          .string()
          .optional()
          .describe("Default working directory for the environment"),
        default_queue_capacity: z.number().int().optional(),
        default_tag_cap: z.number().int().optional(),
        default_max_queued: z.number().int().optional(),
      },
    },
    async (args) => {
      assertEnvironmentWritable(args.name, config);
      return jsonResult(await client.createEnvironment(args));
    },
  );

  // ---- Workflows ----

  tool(
    "list_workflows",
    {
      title: "List workflows",
      description: "List all registered workflows across environments.",
      readOnly: true,
    },
    async () => jsonResult(await client.listWorkflows()),
  );

  tool(
    "get_workflow",
    {
      title: "Get workflow",
      description: "Fetch a single workflow by id.",
      inputSchema: { id: z.string().describe("Workflow id") },
      readOnly: true,
    },
    async (args) => jsonResult(await client.getWorkflow(args.id)),
  );

  tool(
    "register_workflow",
    {
      title: "Register workflow",
      description:
        "Register an externally-managed workflow (marked managed_externally=true). Optionally " +
        "include a full execution `spec` (generic step-flow or typed operator).",
      inputSchema: {
        name: z.string(),
        script_path: z.string().describe("Entry script/command path"),
        cron_schedule: z
          .string()
          .describe("Cron expression; required even for event workflows"),
        description: z.string().optional(),
        environment: z
          .string()
          .optional()
          .describe("Target environment (default: sandbox)"),
        async_mode: z.boolean().optional(),
        email_on_failure: z.boolean().optional(),
        timezone: z.string().optional(),
        domain: z.string().optional(),
        trigger_config: z.string().optional().describe("JSON string"),
        queue_config: z.string().optional().describe("JSON string"),
        spec: WorkflowSpecSchema.optional().describe(
          "WorkflowSpec object (generic|typed); backend validation remains authoritative",
        ),
      },
    },
    async (args) => {
      const environment = args.environment ?? config.defaultEnvironment;
      assertEnvironmentWritable(environment, config);
      return jsonResult(
        await client.registerWorkflow({
          ...(args as Parameters<typeof client.registerWorkflow>[0]),
          environment,
        }),
      );
    },
  );

  tool(
    "set_workflow_spec",
    {
      title: "Set workflow spec",
      description:
        "Replace a workflow's execution spec (generic step-flow or typed operator).",
      inputSchema: {
        id: z.string(),
        spec: WorkflowSpecSchema.describe("Complete WorkflowSpec object"),
      },
    },
    async (args) => {
      await assertWorkflowWritable(args.id);
      return jsonResult(
        await client.setWorkflowSpec(
          args.id,
          args.spec as Parameters<typeof client.setWorkflowSpec>[1],
        ),
      );
    },
  );

  tool(
    "patch_workflow_spec",
    {
      title: "Patch workflow spec",
      description:
        "Safely apply an RFC 7396 JSON Merge Patch to the full stored workflow spec. " +
        "Omitted fields preserve stored values; __redacted__ sentinels in arrays require " +
        "a unique id or unchanged webhook URL. Ambiguous secret restoration fails closed. " +
        "Returns only the redacted stored definition. This is read-merge-write; " +
        "serialize concurrent writers.",
      inputSchema: {
        id: z.string().describe("Workflow id"),
        patch: z
          .record(z.string(), z.unknown())
          .describe("JSON Merge Patch object for the stored WorkflowSpec"),
      },
    },
    async (args) => {
      const currentWorkflow = await client.getWorkflow(args.id);
      if (protectionActive) {
        assertEnvironmentWritable(currentWorkflow.environment, config);
      }
      let currentSpec: unknown = {};
      if (currentWorkflow.spec_json?.trim()) {
        try {
          currentSpec = JSON.parse(currentWorkflow.spec_json) as unknown;
        } catch {
          throw new Error(
            "Stored workflow spec is invalid JSON; use set_workflow_spec for an intentional full replacement.",
          );
        }
      }
      const merged = applyWorkflowSpecMergePatch(currentSpec, args.patch);
      const spec = WorkflowSpecSchema.parse(merged);
      const updated = await client.setWorkflowSpec(
        args.id,
        spec as Parameters<typeof client.setWorkflowSpec>[1],
      );
      return jsonResult(workflowDefinition(updated));
    },
  );

  tool(
    "update_workflow",
    {
      title: "Update workflow",
      description:
        "Patch workflow metadata/runtime preferences (cron, enabled, environment, etc.).",
      inputSchema: {
        id: z.string(),
        name: z.string().optional(),
        description: z.string().nullable().optional(),
        script_path: z.string().optional(),
        cron_schedule: z.string().optional(),
        enabled: z.boolean().optional(),
        async_mode: z.boolean().optional(),
        email_on_failure: z.boolean().optional(),
        timezone: z.string().optional(),
        environment: z.string().optional(),
        domain: z.string().nullable().optional(),
        trigger_config: z.string().nullable().optional(),
        queue_config: z.string().nullable().optional(),
      },
    },
    async (args) => {
      const { id, ...patch } = args;
      await assertWorkflowWritable(id);
      if (patch.environment) {
        assertEnvironmentWritable(patch.environment, config);
      }
      return jsonResult(await client.updateWorkflow(id, patch));
    },
  );

  tool(
    "rerun_workflow",
    {
      title: "Rerun workflow",
      description:
        "Rerun a workflow from a prior run, optionally overriding input JSON. Supports idempotency_key.",
      inputSchema: {
        id: z.string().describe("Workflow id"),
        source_run_id: z
          .string()
          .optional()
          .describe("Run id to copy inputs from"),
        input_override: z.unknown().optional().describe("JSON input override"),
        idempotency_key: z.string().optional(),
      },
    },
    async (args) => {
      await assertWorkflowWritable(args.id);
      return jsonResult(
        await client.rerunWorkflow(args.id, {
          sourceRunId: args.source_run_id,
          inputOverride: args.input_override,
          idempotencyKey: args.idempotency_key,
        }),
      );
    },
  );

  tool(
    "delete_workflow",
    {
      title: "Delete workflow",
      description: "Deregister a workflow by id.",
      inputSchema: { id: z.string() },
      destructive: true,
    },
    async (args) => {
      await assertWorkflowWritable(args.id);
      return jsonResult(await client.deleteWorkflow(args.id));
    },
  );

  // ---- Dispatch ----

  tool(
    "run_workflow_now",
    {
      title: "Run workflow now (deprecated)",
      description:
        "DEPRECATED — use enqueue_workflow instead. Manual runs are admission-controlled: this " +
        "posts to /run but takes the same queued admission path as enqueue_workflow (it does NOT " +
        "execute immediately or bypass the queue) and returns the same result. Pass an " +
        "idempotency_key for safe retries; a reused key returns the original run as " +
        "{status:'duplicate'}.",
      inputSchema: {
        id: z.string(),
        idempotency_key: z.string().optional(),
      },
    },
    async (args) => {
      await assertWorkflowWritable(args.id);
      return jsonResult(
        await client.runWorkflow(args.id, {
          idempotencyKey: args.idempotency_key,
        }),
      );
    },
  );

  tool(
    "enqueue_workflow",
    {
      title: "Enqueue workflow",
      description:
        "Queue a run (scheduler manages concurrency/admission). Supports idempotency_key.",
      inputSchema: {
        id: z.string(),
        idempotency_key: z.string().optional(),
      },
    },
    async (args) => {
      await assertWorkflowWritable(args.id);
      return jsonResult(
        await client.enqueueWorkflow(args.id, {
          idempotencyKey: args.idempotency_key,
        }),
      );
    },
  );

  tool(
    "dispatch_workflow",
    {
      title: "Dispatch workflow (webhook trigger)",
      description:
        "Trigger a workflow's inbound webhook with a raw payload. Provide signature_secret to " +
        "sign the payload if the scheduler requires it.",
      inputSchema: {
        id: z.string(),
        payload: z
          .string()
          .optional()
          .describe("Raw request body forwarded to the trigger"),
        signature_secret: z.string().optional(),
        idempotency_key: z.string().optional(),
        event_id: z
          .string()
          .max(160)
          .refine(
            (value) =>
              [...value].every((character) => {
                const codePoint = character.codePointAt(0) ?? 0;
                return codePoint >= 32 && codePoint !== 127;
              }),
            {
              message: "event_id must not contain control characters",
            },
          )
          .optional()
          .describe("Stable inbound event id used for replay protection"),
        timestamp: z
          .string()
          .regex(/^-?\d+$/, "timestamp must be Unix seconds")
          .optional()
          .describe("Unix timestamp used for deterministic signed replays"),
      },
    },
    async (args) => {
      await assertWorkflowWritable(args.id);
      return jsonResult(
        await client.dispatchWorkflow(args.id, {
          payload: args.payload,
          signatureSecret: args.signature_secret,
          idempotencyKey: args.idempotency_key,
          eventId: args.event_id,
          timestamp: args.timestamp,
        }),
      );
    },
  );

  // ---- Runs ----

  tool(
    "list_workflow_runs",
    {
      title: "List workflow runs",
      description:
        "Fetch recent run history for a workflow (most recent first).",
      inputSchema: { id: z.string() },
      readOnly: true,
    },
    async (args) => jsonResult(await client.listRuns(args.id)),
  );

  tool(
    "get_run",
    {
      title: "Get run",
      description:
        "Fetch a single run (status, exit code, stdout/stderr, result).",
      inputSchema: { id: z.string().describe("Run id") },
      readOnly: true,
    },
    async (args) => jsonResult(await client.getRun(args.id)),
  );

  tool(
    "get_run_logs",
    {
      title: "Get run logs",
      description:
        "Fetch stdout/stderr/exit metadata for a run (lighter than get_run).",
      inputSchema: { id: z.string().describe("Run id") },
      readOnly: true,
    },
    async (args) => jsonResult(await client.getRunLogs(args.id)),
  );

  tool(
    "get_run_tasks",
    {
      title: "Get run tasks",
      description: "Fetch per-step task rows and retry attempts for a run.",
      inputSchema: { id: z.string().describe("Run id") },
      readOnly: true,
    },
    async (args) => jsonResult(await client.getRunTasks(args.id)),
  );

  tool(
    "get_run_metrics",
    {
      title: "Get run metrics",
      description: "Fetch emitted metric samples for a run.",
      inputSchema: { id: z.string().describe("Run id") },
      readOnly: true,
    },
    async (args) => jsonResult(await client.getRunMetrics(args.id)),
  );

  tool(
    "list_queues",
    {
      title: "List queues",
      description: "List queue capacity snapshots across environments.",
      readOnly: true,
    },
    async () => jsonResult(await client.listQueues()),
  );

  tool(
    "list_queued_runs",
    {
      title: "List queued runs",
      description: "List durable queued runs awaiting admission.",
      readOnly: true,
    },
    async () => jsonResult(await client.listQueuedRuns()),
  );

  // ---- Email profiles ----

  const emailProfileFields = {
    name: z.string().describe("Display name for the profile"),
    enabled: z.boolean().describe("Whether the profile is active"),
    alert_email: z.string().describe("Recipient address for alerts"),
    smtp_host: z.string().describe("SMTP server host"),
    smtp_port: z.number().int().describe("SMTP server port (e.g. 587)"),
    smtp_user: z.string().describe("SMTP username"),
    smtp_password: z
      .string()
      .describe(
        "SMTP password. On update, pass '••••••••' to keep the stored secret.",
      ),
    from_address: z.string().describe("Envelope from address"),
    from_name: z.string().describe("Display name for the sender"),
  };

  tool(
    "list_email_profiles",
    {
      title: "List email profiles",
      description:
        "List named SMTP delivery profiles. Passwords are masked ('••••••••').",
      readOnly: true,
    },
    async () => jsonResult(await client.listEmailProfiles()),
  );

  tool(
    "create_email_profile",
    {
      title: "Create email profile",
      description:
        "Create a named SMTP delivery profile. The server assigns the id.",
      inputSchema: emailProfileFields,
    },
    async (args) => jsonResult(await client.createEmailProfile(args)),
  );

  tool(
    "update_email_profile",
    {
      title: "Update email profile",
      description:
        "Update an existing email profile. Echo the masked password ('••••••••') " +
        "to keep the stored secret, or pass a new value to replace it.",
      inputSchema: {
        id: z.string().describe("Email profile id"),
        ...emailProfileFields,
      },
    },
    async (args) => {
      const { id, ...input } = args;
      return jsonResult(await client.updateEmailProfile(id, input));
    },
  );

  tool(
    "delete_email_profile",
    {
      title: "Delete email profile",
      description:
        "Delete an email profile by id. Workflows using it fall back to the global config.",
      inputSchema: { id: z.string().describe("Email profile id") },
      destructive: true,
    },
    async (args) => jsonResult(await client.deleteEmailProfile(args.id)),
  );

  tool(
    "set_workflow_email_profile",
    {
      title: "Select workflow email profile",
      description:
        "Select the email profile a workflow uses for failure alerts. Pass profile_id: null to clear " +
        "(the workflow then uses the global email config).",
      inputSchema: {
        workflow_id: z.string().describe("Workflow id"),
        profile_id: z
          .string()
          .nullable()
          .describe("Email profile id, or null to clear the selection"),
      },
    },
    async (args) => {
      await assertWorkflowWritable(args.workflow_id);
      return jsonResult(
        await client.setWorkflowEmailProfile(args.workflow_id, args.profile_id),
      );
    },
  );

  // ---- Resources (read-only state for @-referencing) ----

  registerAuthoringResources(server, client);

  server.registerResource(
    "version",
    "chaos://version",
    {
      title: "Scheduler version",
      description: "Product/version/schema info",
      mimeType: "application/json",
    },
    async (uri) => readJsonResource(uri, () => client.getVersion()),
  );

  server.registerResource(
    "environments",
    "chaos://environments",
    {
      title: "Environments",
      description: "All execution environments",
      mimeType: "application/json",
    },
    async (uri) => readJsonResource(uri, () => client.listEnvironments()),
  );

  server.registerResource(
    "workflows",
    "chaos://workflows",
    {
      title: "Workflows",
      description: "All registered workflows",
      mimeType: "application/json",
    },
    async (uri) =>
      readJsonResource(
        uri,
        () => client.listWorkflows(),
        projectWorkflowsForResource,
      ),
  );

  server.registerResource(
    "email-profiles",
    "chaos://email-profiles",
    {
      title: "Email profiles",
      description: "All named SMTP delivery profiles (passwords masked)",
      mimeType: "application/json",
    },
    async (uri) => readJsonResource(uri, () => client.listEmailProfiles()),
  );

  server.registerResource(
    "workflow",
    workflowResourceTemplate("chaos://workflows/{id}", client),
    {
      title: "Workflow",
      description: "A single workflow by id",
      mimeType: "application/json",
    },
    async (uri, variables) => {
      const id = resourceIdentifier(uri, variables.id);
      return readJsonResource(
        uri,
        () => client.getWorkflow(id),
        projectWorkflowForResource,
      );
    },
  );

  server.registerResource(
    "workflow-runs",
    workflowResourceTemplate("chaos://workflows/{id}/runs", client),
    {
      title: "Workflow runs",
      description: "Recent runs for a workflow",
      mimeType: "application/json",
    },
    async (uri, variables) => {
      const id = resourceIdentifier(uri, variables.id);
      return readJsonResource(uri, () => client.listRuns(id));
    },
  );

  server.registerResource(
    "run",
    new ResourceTemplate("chaos://runs/{id}", { list: undefined }),
    {
      title: "Run",
      description: "A single run (result + logs)",
      mimeType: "application/json",
    },
    async (uri, variables) => {
      const id = resourceIdentifier(uri, variables.id);
      return readJsonResource(uri, () => client.getRun(id));
    },
  );

  server.registerResource(
    "run-logs",
    new ResourceTemplate("chaos://runs/{id}/logs", { list: undefined }),
    {
      title: "Run logs",
      description: "Stdout/stderr/exit metadata for a run",
      mimeType: "application/json",
    },
    async (uri, variables) => {
      const id = resourceIdentifier(uri, variables.id);
      return readJsonResource(uri, () => client.getRunLogs(id));
    },
  );

  server.registerResource(
    "run-tasks",
    new ResourceTemplate("chaos://runs/{id}/tasks", { list: undefined }),
    {
      title: "Run tasks",
      description: "Per-step task rows and retry attempts for a run",
      mimeType: "application/json",
    },
    async (uri, variables) => {
      const id = resourceIdentifier(uri, variables.id);
      return readJsonResource(uri, () => client.getRunTasks(id));
    },
  );

  server.registerResource(
    "run-metrics",
    new ResourceTemplate("chaos://runs/{id}/metrics", { list: undefined }),
    {
      title: "Run metrics",
      description: "Metric samples emitted during a run",
      mimeType: "application/json",
    },
    async (uri, variables) => {
      const id = resourceIdentifier(uri, variables.id);
      return readJsonResource(uri, () => client.getRunMetrics(id));
    },
  );

  server.registerResource(
    "queues",
    "chaos://queues",
    {
      title: "Queues",
      description: "Queue capacity snapshots",
      mimeType: "application/json",
    },
    async (uri) => readJsonResource(uri, () => client.listQueues()),
  );

  server.registerResource(
    "queued-runs",
    "chaos://queued-runs",
    {
      title: "Queued runs",
      description: "Durable queued runs awaiting admission",
      mimeType: "application/json",
    },
    async (uri) => readJsonResource(uri, () => client.listQueuedRuns()),
  );

  // ---- Prompts (triage/reporting templates) ----

  server.registerPrompt(
    "triage_failed_run",
    {
      title: "Triage a failed run",
      description: "Investigate a failed run and propose a fix.",
      argsSchema: { run_id: z.string().describe("The failed run id") },
    },
    ({ run_id }) => ({
      messages: [
        {
          role: "user",
          content: {
            type: "text",
            text:
              `Investigate Chaos Scheduler run \`${run_id}\`.\n\n` +
              `1. Read the run via the \`get_run\` tool (or the \`chaos://runs/${run_id}\` resource).\n` +
              `2. Summarize why it failed (exit code, stderr tail). Use \`get_run_logs\` when you only need stdout/stderr.\n` +
              `3. Use \`get_run_tasks\` only for step/retry detail and \`get_run_metrics\` only when emitted measurements can explain the failure.\n` +
              `4. Inspect the owning workflow with \`get_workflow\`.\n` +
              `5. Propose a concrete fix. If the failure looks transient, ask the operator for explicit confirmation before any retry.\n` +
              `6. Only after confirmation, call \`rerun_workflow\` with \`source_run_id: ${run_id}\` and a fresh \`idempotency_key\` for a faithful retry.`,
          },
        },
      ],
    }),
  );

  server.registerPrompt(
    "summarize_workflow_health",
    {
      title: "Summarize workflow health",
      description: "Summarize recent run health for an environment.",
      argsSchema: { environment: z.string().describe("Environment name") },
    },
    ({ environment }) => ({
      messages: [
        {
          role: "user",
          content: {
            type: "text",
            text:
              `Summarize workflow health for the \`${environment}\` environment.\n\n` +
              `1. Call \`list_workflows\` and filter to environment \`${environment}\`.\n` +
              `2. For each, call \`list_workflow_runs\` and compute recent success/failure counts.\n` +
              `3. Highlight failing or stale workflows and recommend next actions.`,
          },
        },
      ],
    }),
  );

  server.registerPrompt(
    "register_workflow_for_repo",
    {
      title: "Register a workflow for this repo",
      description:
        "Draft and register a scheduler workflow for the current repository.",
      argsSchema: {
        repo_path: z.string().describe("Absolute path to the repo"),
        environment: z
          .string()
          .optional()
          .describe("Target environment (default: sandbox)"),
      },
    },
    ({ repo_path, environment }) => ({
      messages: [
        {
          role: "user",
          content: {
            type: "text",
            text:
              `Help me register a Chaos Scheduler workflow for the repo at \`${repo_path}\`` +
              (environment ? ` in the \`${environment}\` environment` : "") +
              `.\n\n` +
              `1. Read \`chaos://workflows/index\` and stop if an existing workflow should be updated; ` +
              `registration is non-idempotent.\n` +
              `2. Read \`chaos://guides/workflows\` and the relevant \`chaos://schemas/*\` resources.\n` +
              `3. Inspect the repo to find the entry script/command and a sensible schedule.\n` +
              `4. Draft a WorkflowSpec (generic step-flow) if there are multiple steps.\n` +
              `5. Confirm the plan with me, then call \`register_workflow\` with name, script_path, ` +
              `cron_schedule, environment, and the spec.`,
          },
        },
      ],
    }),
  );

  server.registerPrompt(
    "safely_update_workflow",
    {
      title: "Safely update a workflow",
      description:
        "Inspect a redacted stored definition and draft a secret-preserving patch.",
      argsSchema: {
        workflow_id: z.string().describe("Workflow id"),
      },
    },
    ({ workflow_id }) => ({
      messages: [
        {
          role: "user",
          content: {
            type: "text",
            text:
              `Safely update Chaos Scheduler workflow \`${workflow_id}\`.\n\n` +
              `1. Read \`chaos://workflows/${workflow_id}/definition\` and relevant authoring guides/schemas.\n` +
              `2. Treat it as redacted stored configuration, not effective runtime configuration.\n` +
              `3. Draft the smallest RFC 7396 patch. Keep \`__redacted__\` sentinels; array items need a unique id or unchanged webhook URL, while identity changes need the real secret.\n` +
              `4. Confirm the proposed change with me and ensure no concurrent writer is editing the spec.\n` +
              `5. Call \`patch_workflow_spec\`; use \`set_workflow_spec\` only for an intentional full replacement.`,
          },
        },
      ],
    }),
  );

  return server;
}
