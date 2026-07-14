import { z } from "zod";

const nonEmptyString = z
  .string()
  .refine((value) => value.trim().length > 0, "must not be empty");
const nonNegativeInteger = z.number().int().nonnegative();

export const RetryPolicySchema = z
  .object({
    max_retries: nonNegativeInteger.optional(),
    backoff_seconds: nonNegativeInteger.optional(),
  })
  .passthrough();

export const StepSpecSchema = z
  .object({
    id: nonEmptyString,
    command: nonEmptyString.optional(),
    script: nonEmptyString.optional(),
    args: z.array(z.string()).optional(),
    working_dir: z.string().optional(),
    depends_on: z.array(nonEmptyString).optional(),
    retry: RetryPolicySchema.optional(),
    timeout_seconds: nonNegativeInteger.optional(),
    continue_on_error: z.boolean().optional(),
  })
  .passthrough()
  .superRefine((step, ctx) => {
    if (Boolean(step.command) === Boolean(step.script)) {
      ctx.addIssue({
        code: "custom",
        message: "step must specify exactly one of command or script",
      });
    }
  });

export const GenericSpecSchema = z
  .object({
    steps: z
      .array(StepSpecSchema)
      .min(1, "generic workflow requires at least one step"),
  })
  .passthrough();

export const WebhookActionSchema = z
  .object({
    type: z.literal("webhook"),
    url: z.url(),
    secret: z.string().optional(),
    max_retries: nonNegativeInteger.optional(),
  })
  .passthrough();

export const ActionSpecSchema = z.discriminatedUnion("type", [
  z
    .object({
      type: z.literal("email"),
      to: z.string().optional(),
    })
    .passthrough(),
  WebhookActionSchema,
  z
    .object({
      type: z.literal("run_workflow"),
      workflow_id: nonEmptyString,
      wait: z.boolean().optional(),
    })
    .passthrough(),
  z
    .object({
      type: z.literal("desktop_notification"),
      title: z.string().optional(),
    })
    .passthrough(),
]);

export const GitPullOperatorConfigSchema = z
  .object({
    path: nonEmptyString,
    repo_url: nonEmptyString.optional(),
    branch: nonEmptyString.optional(),
    depth: nonNegativeInteger.optional(),
    rebase: z.boolean().optional(),
  })
  .passthrough();

export const CursorAgentOperatorConfigSchema = z
  .object({
    mode: z.enum(["cloud", "cli"]).optional(),
    prompt: nonEmptyString,
    repository: nonEmptyString.optional(),
    repo: nonEmptyString.optional(),
    ref: z.string().optional(),
    model: z.string().optional(),
    auto_create_pr: z.boolean().optional(),
    api_key_secret: z.string().optional(),
    poll_attempts: nonNegativeInteger.optional(),
    poll_interval_ms: nonNegativeInteger.optional(),
    cli_path: z.string().optional(),
  })
  .passthrough()
  .superRefine((config, ctx) => {
    if (
      (config.mode ?? "cloud") === "cloud" &&
      !config.repository &&
      !config.repo
    ) {
      ctx.addIssue({
        code: "custom",
        path: ["repository"],
        message: "cursor_agent cloud mode requires repository",
      });
    }
  });

export const TypedSpecSchema = z
  .object({
    operator_type: nonEmptyString,
    config: z.record(z.string(), z.unknown()).optional(),
  })
  .passthrough()
  .superRefine((typed, ctx) => {
    const knownSchema =
      typed.operator_type === "git_pull"
        ? GitPullOperatorConfigSchema
        : typed.operator_type === "cursor_agent"
          ? CursorAgentOperatorConfigSchema
          : undefined;
    if (!knownSchema) return;

    const result = knownSchema.safeParse(typed.config ?? {});
    if (!result.success) {
      for (const issue of result.error.issues) {
        ctx.addIssue({
          code: "custom",
          path: ["config", ...issue.path],
          message: issue.message,
        });
      }
    }
  });

const workflowSpecCommon = {
  environment: z.string().optional(),
  on_success: z.array(ActionSpecSchema).optional(),
  on_failure: z.array(ActionSpecSchema).optional(),
};

export const WorkflowSpecSchema = z.discriminatedUnion("kind", [
  z
    .object({
      kind: z.literal("generic"),
      ...workflowSpecCommon,
      generic: GenericSpecSchema,
      typed: TypedSpecSchema.optional(),
    })
    .passthrough(),
  z
    .object({
      kind: z.literal("typed"),
      ...workflowSpecCommon,
      generic: GenericSpecSchema.optional(),
      typed: TypedSpecSchema,
    })
    .passthrough(),
]);

const CronTriggerSchema = z
  .object({
    kind: z.literal("cron"),
    id: z.string().optional(),
    cron: nonEmptyString,
  })
  .passthrough();

const FileArrivalTriggerSchema = z
  .object({
    kind: z.literal("file_arrival"),
    id: z.string().optional(),
    path: nonEmptyString,
    mode: z
      .enum(["mtime_changed", "size_changed", "content_hash_changed"])
      .optional(),
  })
  .passthrough();

const AssetUpdateTriggerSchema = z
  .object({
    kind: z.literal("asset_update"),
    id: z.string().optional(),
    asset: z
      .object({
        kind: nonEmptyString,
        namespace: z.string().optional(),
        partition: z.string().optional(),
      })
      .passthrough(),
  })
  .passthrough();

const OnCompletionTriggerSchema = z
  .object({
    kind: z.literal("on_completion"),
    id: z.string().optional(),
    upstream_workflow_id: nonEmptyString,
    status_filter: z
      .array(z.enum(["success", "failed"]))
      .min(1)
      .optional()
      .describe("Defaults to ['success'] when omitted"),
  })
  .passthrough();

export const TriggerSchema = z.discriminatedUnion("kind", [
  CronTriggerSchema,
  FileArrivalTriggerSchema,
  AssetUpdateTriggerSchema,
  OnCompletionTriggerSchema,
]);

export const TriggerConfigSchema = z.union([
  z.array(TriggerSchema),
  z
    .object({
      triggers: z.array(TriggerSchema),
    })
    .passthrough(),
]);

export const QueueConfigSchema = z
  .object({
    depends_on: z.array(nonEmptyString).optional(),
    waits_for: z.array(nonEmptyString).optional(),
    excludes: z.array(nonEmptyString).optional(),
    tags: z.array(nonEmptyString).optional(),
    queue: nonEmptyString.optional(),
    priority: z.number().int().optional(),
  })
  .passthrough();

export const EmailProfileInputSchema = z
  .object({
    name: nonEmptyString,
    enabled: z.boolean(),
    alert_email: nonEmptyString,
    smtp_host: nonEmptyString,
    smtp_port: z.number().int().positive(),
    smtp_user: z.string(),
    smtp_password: z.string(),
    from_address: nonEmptyString,
    from_name: z.string(),
  })
  .passthrough();

export const IntegrationSchema = z
  .object({
    completion_actions: z.array(ActionSpecSchema).optional(),
    email_profile: EmailProfileInputSchema.optional(),
    git_pull: GitPullOperatorConfigSchema.optional(),
    cursor_agent: CursorAgentOperatorConfigSchema.optional(),
  })
  .passthrough();

export function parseWorkflowSpec(
  value: unknown,
): z.infer<typeof WorkflowSpecSchema> {
  return WorkflowSpecSchema.parse(value);
}
