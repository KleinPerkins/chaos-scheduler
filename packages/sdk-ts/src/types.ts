/**
 * Shared wire types for the Chaos Scheduler REST API (`/api/v1`).
 *
 * SOURCE OF TRUTH: these interfaces are hand-derived from the Rust backend
 * models. There is no generated OpenAPI document (yet), so when the backend
 * changes, update these to match. The authoritative definitions live in:
 *
 *   - `src-tauri/src/db.rs`            — `Workflow`, `Run`, `Environment`
 *   - `src-tauri/src/workflow_spec.rs` — `WorkflowSpec`, `WorkflowKind`, `StepSpec`,
 *                                        `GenericSpec`, `TypedSpec`, `RetryPolicy`
 *   - `src-tauri/src/actions.rs`       — `ActionSpec`
 *   - `src-tauri/src/scheduler.rs`     — `DispatchOutcome`
 *   - `src-tauri/src/api.rs`           — request bodies + response envelopes
 *
 * Field names use snake_case to match serde's default serialization on the
 * backend (the API does not rename fields).
 */

/** Which execution model a workflow uses (`workflow_spec.rs::WorkflowKind`). */
export type WorkflowKind = "generic" | "typed";

/** Per-step retry policy (`workflow_spec.rs::RetryPolicy`). */
export interface RetryPolicy {
  max_retries?: number;
  backoff_seconds?: number;
}

/** A single step in a generic workflow (`workflow_spec.rs::StepSpec`). */
export interface StepSpec {
  id: string;
  /** A full shell command line (mutually exclusive with `script`). */
  command?: string;
  /** A script path resolved against `working_dir`/workspace root. */
  script?: string;
  args?: string[];
  working_dir?: string;
  depends_on?: string[];
  retry?: RetryPolicy;
  timeout_seconds?: number;
  /** If true, a failure of this step does not fail the run. */
  continue_on_error?: boolean;
}

/** The generic step-flow body (`workflow_spec.rs::GenericSpec`). */
export interface GenericSpec {
  steps: StepSpec[];
}

/** The typed-operator body (`workflow_spec.rs::TypedSpec`). */
export interface TypedSpec {
  operator_type: string;
  config?: unknown;
}

/**
 * On-success / on-failure action (`actions.rs::ActionSpec`), a serde
 * internally-tagged enum keyed on `type` with snake_case variants.
 */
export type ActionSpec =
  | { type: "email"; to?: string }
  | { type: "webhook"; url: string; secret?: string; max_retries?: number }
  | { type: "run_workflow"; workflow_id: string; wait?: boolean }
  | { type: "desktop_notification"; title?: string };

/** Full workflow execution spec stored in `workflows.spec_json`. */
export interface WorkflowSpec {
  kind: WorkflowKind;
  environment?: string;
  generic?: GenericSpec;
  typed?: TypedSpec;
  on_success?: ActionSpec[];
  on_failure?: ActionSpec[];
}

/** A user-managed execution environment (`db.rs::Environment`). */
export interface Environment {
  id: string;
  name: string;
  description: string | null;
  working_dir: string | null;
  default_queue_capacity: number | null;
  default_tag_cap: number | null;
  default_max_queued: number | null;
  managed_externally: boolean;
  created_at: string;
  updated_at: string;
}

/** A registered workflow (`db.rs::Workflow`). */
export interface Workflow {
  id: string;
  name: string;
  description: string | null;
  script_path: string;
  cron_schedule: string;
  enabled: boolean;
  async_mode: boolean;
  email_on_failure: boolean;
  /** First-class environment (partition/queue-scope/filter). */
  environment: string;
  /** Governance flag: definition owned by an external source of truth. */
  managed_externally: boolean;
  /** Execution model: `generic` (step-flow) or `typed` (operator). */
  kind: string;
  /** Serialized `WorkflowSpec` (null for legacy single-script workflows). */
  spec_json: string | null;
  domain: string | null;
  timezone: string;
  trigger_config: string | null;
  queue_config: string | null;
  last_run_at: string | null;
  created_at: string;
  updated_at: string;
}

/** A run record (`db.rs::Run`). */
export interface Run {
  id: string;
  workflow_id: string;
  started_at: string;
  finished_at: string | null;
  exit_code: number | null;
  stdout: string | null;
  stderr: string | null;
  result_url: string | null;
  status: string;
  workflow_name?: string;
  summary?: unknown;
  error_analysis?: unknown;
  trigger_kind: string | null;
  trigger_payload: string | null;
  upstream_run_id: string | null;
  input_json: string | null;
  rerun_of_run_id: string | null;
}

/**
 * The result of an on-demand dispatch (`scheduler.rs::DispatchOutcome`).
 *
 * `status` is one of the backend's dispatch statuses (e.g. `admitted`,
 * `queued`, `skipped`). When an `Idempotency-Key` matches a previous request,
 * the API instead returns `{ status: "duplicate", run_id }` — see
 * {@link DispatchResult}.
 */
export interface DispatchOutcome {
  workflow_id: string;
  status: string;
  run_id: string | null;
  queued_run_id: string | null;
  queue_name: string;
  trigger_kind: string | null;
  trigger_payload: string | null;
  reason: string | null;
}

/** Idempotent-replay shape returned when an `Idempotency-Key` is reused. */
export interface DuplicateDispatch {
  status: "duplicate";
  run_id: string | null;
  queued_run_id?: string | null;
}

/** Union of the two shapes a run/enqueue/dispatch call can return. */
export type DispatchResult = DispatchOutcome | DuplicateDispatch;

/** Narrow a {@link DispatchResult} to the idempotent-replay case. */
export function isDuplicateDispatch(r: DispatchResult): r is DuplicateDispatch {
  return (
    (r as DuplicateDispatch).status === "duplicate" && !("workflow_id" in r)
  );
}

/** Backend `/api/v1/version` payload. */
export interface VersionInfo {
  product: string;
  version: string;
  schema_version: number;
  api: string;
}

/** Input for registering a workflow (`api.rs::RegisterWorkflowBody`). */
export interface RegisterWorkflowInput {
  name: string;
  description?: string;
  script_path: string;
  cron_schedule: string;
  /** Defaults to `"instance"` on the backend when omitted. */
  environment?: string;
  async_mode?: boolean;
  /** Defaults to `true` on the backend when omitted. */
  email_on_failure?: boolean;
  /** Defaults to `"UTC"` on the backend when omitted. */
  timezone?: string;
  domain?: string;
  trigger_config?: string;
  queue_config?: string;
  /** Optional execution spec applied after creation. */
  spec?: WorkflowSpec;
}

/**
 * A named, reusable email-delivery profile (`db.rs::EmailProfile`).
 *
 * `smtp_password` is always masked (`"••••••••"`) on read; a blank value means
 * no password is stored. Echo the mask back on update to keep the stored
 * secret unchanged, or send a new value to replace it.
 */
export interface EmailProfile {
  id: string;
  name: string;
  enabled: boolean;
  alert_email: string;
  smtp_host: string;
  smtp_port: number;
  smtp_user: string;
  /** Masked on read (`"••••••••"`) when a password is stored. */
  smtp_password: string;
  from_address: string;
  from_name: string;
  created_at: string;
  updated_at: string;
}

/** The masking sentinel returned/accepted for a stored SMTP password. */
export const MASKED_SECRET = "••••••••";

/**
 * Input for creating/updating an email profile. Mirrors {@link EmailProfile}
 * minus the server-assigned id/timestamps. On update, echo the masked password
 * back to preserve the stored secret.
 */
export interface EmailProfileInput {
  name: string;
  enabled: boolean;
  alert_email: string;
  smtp_host: string;
  smtp_port: number;
  smtp_user: string;
  smtp_password: string;
  from_address: string;
  from_name: string;
}

/** Run log payload (`api.rs::get_run_logs`). */
export interface RunLogs {
  run_id: string;
  status: string;
  exit_code: number | null;
  stdout: string | null;
  stderr: string | null;
  result_url: string | null;
}

/** Per-step task row (`db.rs::RunTask`). */
export interface RunTask {
  id: string;
  run_id: string;
  attempt_id: string | null;
  task_id: string;
  status: string;
  started_at: string | null;
  finished_at: string | null;
  attempt_number: number;
  parent_task_id: string | null;
  error_type: string | null;
  error_message: string | null;
  details?: unknown;
}

/** Retry attempt row (`db.rs::RunAttempt`). */
export interface RunAttempt {
  id: string;
  run_id: string;
  task_id: string;
  attempt_number: number;
  status: string;
  started_at: string;
  finished_at: string | null;
  exit_code: number | null;
  retry_reason: string | null;
  error_type: string | null;
  error_message: string | null;
  trigger_kind: string | null;
}

/** `GET /runs/{id}/tasks` envelope. */
export interface RunTasksResult {
  tasks: RunTask[];
  attempts: RunAttempt[];
}

/** Emitted metric sample (`db.rs::RunMetric`). */
export interface RunMetric {
  id: string;
  run_id: string;
  task_id: string | null;
  metric_name: string;
  metric_value: number;
  metric_unit: string | null;
  emitted_at: string;
  labels?: unknown;
}

/** Queue capacity snapshot (`db.rs::QueueInfo`). */
export interface QueueInfo {
  name: string;
  environment: string;
  capacity: number;
  tag_cap: number | null;
  max_queued: number | null;
  active_count: number;
  queued_count: number;
  global_parallelism_cap: number;
  updated_at: string;
}

/** Durable queued-run row (`db.rs::QueuedRun`). */
export interface QueuedRun {
  id: string;
  run_id: string | null;
  workflow_id: string;
  workflow_name: string | null;
  queue_name: string;
  environment: string;
  priority: number;
  status: string;
  queued_at: string;
  admitted_at: string | null;
  finished_at: string | null;
  trigger_kind: string | null;
  trigger_payload: string | null;
  upstream_run_id: string | null;
  input_json: string | null;
  rerun_of_run_id: string | null;
}

/** Input for creating an environment (`api.rs::CreateEnvironmentBody`). */
export interface CreateEnvironmentInput {
  name: string;
  description?: string;
  working_dir?: string;
  default_queue_capacity?: number;
  default_tag_cap?: number;
  default_max_queued?: number;
}

/** Scopes recognized by the backend API-key model (`service.rs`). */

/** Partial update body for `PATCH /workflows/{id}` (`api.rs::UpdateWorkflowBody`). */
export interface UpdateWorkflowInput {
  name?: string;
  description?: string | null;
  script_path?: string;
  cron_schedule?: string;
  enabled?: boolean;
  async_mode?: boolean;
  email_on_failure?: boolean;
  timezone?: string;
  environment?: string;
  domain?: string | null;
  trigger_config?: string | null;
  queue_config?: string | null;
}

/** Options for `POST /workflows/{id}/rerun`. */
export interface RerunWorkflowOptions {
  /** Value for the `Idempotency-Key` header (safe replay). */
  idempotencyKey?: string;
  /** Prior run to copy inputs from. */
  sourceRunId?: string;
  /** JSON input override merged into the rerun dispatch. */
  inputOverride?: unknown;
}

export type ApiScope = "read" | "write" | "admin";
