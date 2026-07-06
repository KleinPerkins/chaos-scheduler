/**
 * `@chaos-scheduler/sdk` — typed TypeScript client for the Chaos Scheduler
 * REST API (`/api/v1`), plus webhook signature helpers.
 *
 * @example
 * ```ts
 * import { ChaosSchedulerClient } from "@chaos-scheduler/sdk";
 *
 * const client = new ChaosSchedulerClient({
 *   baseUrl: "http://127.0.0.1:9618",
 *   apiKey: process.env.CHAOS_SCHEDULER_API_KEY,
 * });
 *
 * const wf = await client.registerWorkflow({
 *   name: "Nightly digest",
 *   script_path: "scripts/digest.py",
 *   cron_schedule: "0 6 * * *",
 *   environment: "instance",
 * });
 * const outcome = await client.runWorkflow(wf.id, { idempotencyKey: crypto.randomUUID() });
 * ```
 */

export { ChaosSchedulerClient, TERMINAL_RUN_STATUSES } from "./client.js";
export type {
  ChaosSchedulerClientOptions,
  DispatchOptions,
  InboundDispatchOptions,
  FetchLike,
  WaitForRunOptions,
} from "./client.js";
export { ChaosApiError } from "./errors.js";
export {
  computeWebhookSignature,
  computeInboundDispatchSignature,
  inboundCanonicalPayload,
  inboundDispatchHeaders,
  webhookSignatureHeader,
  verifyWebhookSignature,
  verifyInboundDispatchSignature,
} from "./webhook.js";
export type {
  InboundDispatchHeaderOptions,
  SignaturePayload,
} from "./webhook.js";
export { isDuplicateDispatch, MASKED_SECRET } from "./types.js";
export type {
  ActionSpec,
  ApiScope,
  CreateEnvironmentInput,
  DispatchOutcome,
  DispatchResult,
  DuplicateDispatch,
  EmailProfile,
  EmailProfileInput,
  Environment,
  GenericSpec,
  QueueInfo,
  QueuedRun,
  RegisterWorkflowInput,
  RerunWorkflowOptions,
  RetryPolicy,
  Run,
  RunAttempt,
  RunLogs,
  RunMetric,
  RunTask,
  RunTasksResult,
  StepSpec,
  TypedSpec,
  UpdateWorkflowInput,
  VersionInfo,
  Workflow,
  WorkflowKind,
  WorkflowSpec,
} from "./types.js";
