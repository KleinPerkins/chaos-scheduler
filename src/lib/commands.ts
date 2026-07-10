import { invoke } from "@tauri-apps/api/core";

/** Which execution model a workflow uses (mirrors `workflow_spec::WorkflowKind`). */
export type WorkflowKind = "generic" | "typed";

export interface Workflow {
  id: string;
  name: string;
  description: string | null;
  script_path: string;
  cron_schedule: string;
  enabled: boolean;
  async_mode: boolean;
  email_on_failure: boolean;
  /** First-class environment name (dynamic; seeded `production`/`sandbox`). */
  environment: string;
  /** True when the definition is owned by an external source of truth (git /
   * API-registered) and is therefore read-only in the app. */
  managed_externally: boolean;
  /** Execution model: multi-step `generic` DAG or single-operator `typed`. */
  kind: WorkflowKind;
  /** Serialized `WorkflowSpec` (steps / operator config / actions), or null for
   * legacy single-script workflows. */
  spec_json: string | null;
  domain?: string | null;
  timezone: string;
  trigger_config?: string | null;
  queue_config?: string | null;
  /** Named email profile used for this workflow's failure alerts; null falls
   * back to the global email config. */
  email_profile_id?: string | null;
  last_run_at: string | null;
  created_at: string;
  updated_at: string;
}

/** Read the effective environment of a workflow / run / queue row. */
export function environmentOf(row: { environment?: string | null }): string {
  return (row.environment ?? "default").toString();
}

// --- Environments (Phase 3) ---

export interface Environment {
  id: string;
  name: string;
  description?: string | null;
  working_dir?: string | null;
  default_queue_capacity?: number | null;
  default_tag_cap?: number | null;
  default_max_queued?: number | null;
  managed_externally?: boolean;
  workflow_count?: number | null;
  created_at?: string | null;
  updated_at?: string | null;
}

// --- Workflow spec model (Phases 4 & 5) ---

export interface RetryPolicy {
  max_retries: number;
  backoff_seconds: number;
}

export interface StepSpec {
  id: string;
  command?: string | null;
  script?: string | null;
  args: string[];
  working_dir?: string | null;
  depends_on: string[];
  retry?: RetryPolicy | null;
  timeout_seconds?: number | null;
  continue_on_error: boolean;
}

export interface GenericSpec {
  steps: StepSpec[];
}

export interface TypedSpec {
  operator_type: string;
  config: Record<string, unknown>;
}

/** Discriminated union matching the serde `#[serde(tag = "type")]` encoding of
 * `actions::ActionSpec`. */
export type ActionSpec =
  | { type: "email"; to?: string | null }
  | {
      type: "webhook";
      url: string;
      secret?: string | null;
      max_retries?: number;
    }
  | { type: "run_workflow"; workflow_id: string; wait?: boolean }
  | { type: "desktop_notification"; title?: string | null };

export type ActionKind = ActionSpec["type"];

export interface WorkflowSpec {
  kind: WorkflowKind;
  environment?: string | null;
  generic?: GenericSpec | null;
  typed?: TypedSpec | null;
  on_success: ActionSpec[];
  on_failure: ActionSpec[];
}

// --- API keys & integrations (Phases 6 & 8) ---

export type ApiKeyScope = "read" | "write" | "admin";

/** Returned once by `create_api_key`; `token` is shown a single time. */
export interface NewApiKey {
  id: string;
  token: string;
  scopes: string;
}

/** Listing shape (never includes the secret). */
export interface ApiKey {
  id: string;
  name?: string | null;
  scopes: string;
  created_at?: string | null;
  last_used_at?: string | null;
  /** Soft-deleted keys stay in the listing (as revoked) so the UI can reflect
   * that a revoke persisted; the backend rejects them on auth. */
  revoked?: boolean;
}

// --- Updater (Phase 11) ---

export type UpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready_to_restart"
  | "error";

export interface UpdateErrorInfo {
  /** One of "network" | "endpoint" | "verification" | "install" | "unknown". */
  kind: string;
  message: string;
}

export interface UpdateProgress {
  percent?: number | null;
}

/** Mirrors `src-tauri/src/update.rs`'s `UpdateSnapshot` — the single source
 * of truth for updater state, hydrated on mount and pushed on every
 * `update-status` event (see `useAppUpdate`). */
export interface UpdateSnapshot {
  updater_available: boolean;
  phase: UpdatePhase;
  current_version: string;
  latest_version?: string | null;
  notes?: string | null;
  last_checked_at?: string | null;
  last_error?: UpdateErrorInfo | null;
  progress?: UpdateProgress | null;
  background_check_enabled: boolean;
  skipped_version?: string | null;
}

export interface UpdaterPreferencesPatch {
  backgroundCheckEnabled?: boolean;
  /** Sets the skipped version. Ignored if `clearSkip` is also `true`. */
  skippedVersion?: string;
  /** Clears any previously skipped version. */
  clearSkip?: boolean;
}

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
  workflow_name?: string | null;
  summary?: unknown;
  error_analysis?: ErrorAnalysis | null;
  trigger_kind?: string | null;
  trigger_payload?: string | null;
  upstream_run_id?: string | null;
  input_json?: string | null;
  rerun_of_run_id?: string | null;
}

export interface RunTask {
  id: string;
  run_id: string;
  attempt_id?: string | null;
  task_id: string;
  status: string;
  started_at?: string | null;
  finished_at?: string | null;
  attempt_number: number;
  parent_task_id?: string | null;
  error_type?: string | null;
  error_message?: string | null;
  details?: unknown;
}

export interface RunAttempt {
  id: string;
  run_id: string;
  task_id: string;
  attempt_number: number;
  status: string;
  started_at: string;
  finished_at?: string | null;
  exit_code?: number | null;
  retry_reason?: string | null;
  error_type?: string | null;
  error_message?: string | null;
  trigger_kind?: string | null;
}

export interface RunMetric {
  id: string;
  run_id: string;
  task_id?: string | null;
  metric_name: string;
  metric_value: number;
  metric_unit?: string | null;
  emitted_at: string;
  labels?: unknown;
}

export interface WorkflowHistoryBucket {
  day: string;
  total: number;
  failed: number;
  succeeded: number;
}

export interface SlaViolation {
  workflow_id: string;
  workflow_name: string;
  violation_type: string;
  message: string;
  severity: string;
}

export interface WorkflowResourceSample {
  id: string;
  run_id?: string | null;
  workflow_id: string;
  queue_name?: string | null;
  environment: string;
  pid?: number | null;
  sampled_at: string;
  cpu_percent?: number | null;
  memory_rss_bytes?: number | null;
  memory_vms_bytes?: number | null;
  swap_bytes?: number | null;
  labels?: unknown;
}

export interface WorkflowTokenUsageRollup {
  time_bucket?: string | null;
  workflow_id?: string | null;
  environment?: string | null;
  domain?: string | null;
  queue_name?: string | null;
  provider?: string | null;
  model?: string | null;
  token_kind?: string | null;
  total_tokens: number;
  call_count: number;
}

export interface SchedulerAsset {
  asset_id: string;
  asset_kind: string;
  asset_namespace: string;
  asset_partition: string;
  last_action?: string | null;
  last_written_at?: string | null;
  last_writer_run_id?: string | null;
  freshness_policy?: unknown;
}

export type TokenRollupDimension =
  | "time_bucket"
  | "workflow_id"
  | "environment"
  | "domain"
  | "queue_name"
  | "provider"
  | "model"
  | "token_kind";

export interface NextRun {
  workflow_id: string;
  workflow_name: string;
  environment: string;
  next_time: string;
}

export interface SchedulerStatus {
  active_workflows: number;
  running_count: number;
  next_runs: NextRun[];
  recent_runs: Run[];
}

export interface DomainOption {
  value: string;
  label: string;
  workflow_count: number;
}

export interface MissionControlPreferences {
  default_landing: "mission_control" | "dashboard";
  /** "all" or an environment name (the active partition filter). */
  environment_filter: string;
  domain_filter: string;
}

export interface MissionControlHeader {
  active_workflows: number;
  running_count: number;
  queued_count: number;
  recent_failures: number;
}

export interface MissionControlSlaSummary {
  violations_count: number;
  success_rate_24h: number | null;
  median_wait_seconds: number | null;
  /** Longest admission wait (seconds) over the summary window. */
  max_wait_seconds: number | null;
  long_running_count: number;
  blocked_count: number;
}

/** Windowed KPI roll-up for the v3 dashboard, keyed by `(environment,
 * lookback)`. Mirrors `db::DashboardKpiSummary`. */
export interface DashboardKpiSummary {
  total_runs: number;
  succeeded: number;
  failed: number;
  /** succeeded / (succeeded + failed) over terminal runs; null when none. */
  success_rate: number | null;
  /** total_runs / window hours; null when the window is non-positive. */
  throughput_per_hour: number | null;
  avg_runtime_seconds: number | null;
  max_runtime_seconds: number | null;
  median_wait_seconds: number | null;
  max_wait_seconds: number | null;
  /** Nominal window length in seconds (echoed from the requested lookback). */
  window_seconds: number;
}

/** Week-over-week KPI comparison: current vs prior equal window + deltas.
 * Mirrors `db::DashboardKpiDelta`. Optional deltas are null when either side is
 * absent. */
export interface DashboardKpiDelta {
  current: DashboardKpiSummary;
  previous: DashboardKpiSummary;
  total_runs_delta: number;
  succeeded_delta: number;
  failed_delta: number;
  success_rate_delta: number | null;
  throughput_per_hour_delta: number | null;
  avg_runtime_seconds_delta: number | null;
  max_runtime_seconds_delta: number | null;
  median_wait_seconds_delta: number | null;
  max_wait_seconds_delta: number | null;
}

/** One slice of the status donut. Mirrors `db::DashboardStatusCount`. The
 * `succeeded` alias is collapsed onto `success` (matching `statusKey`). */
export interface DashboardStatusCount {
  status: string;
  count: number;
}

/** One time bucket of the success/fail trend. Mirrors `db::DashboardTrendBucket`.
 * `bucket` is the ISO-8601 UTC bucket start. */
export interface DashboardTrendBucket {
  bucket: string;
  total: number;
  failed: number;
  succeeded: number;
}

/** Cross-workflow success/fail trend plus the chosen bucket grain. Mirrors
 * `db::DashboardTrendSeries`. */
export interface DashboardTrendSeries {
  grain: "hour" | "day";
  buckets: DashboardTrendBucket[];
}

/** One bucket of a wait/runtime duration trend. Mirrors `db::DashboardMetricBucket`.
 * `baseline_avg_seconds` is the 30-day trailing average ending at the bucket. */
export interface DashboardMetricBucket {
  bucket: string;
  avg_seconds: number | null;
  max_seconds: number | null;
  count: number;
  baseline_avg_seconds: number | null;
}

/** Wait + runtime duration trends over (environment, lookback), each bucketed
 * at the chosen grain with a 30-day trailing-average baseline per bucket.
 * Mirrors `db::DashboardWaitRuntimeTrend`. */
export interface DashboardWaitRuntimeTrend {
  grain: "hour" | "day";
  wait: DashboardMetricBucket[];
  runtime: DashboardMetricBucket[];
}

/** One workflow's in-window failure recurrence. Mirrors
 * `db::DashboardWorkflowFailureCount`. */
export interface DashboardWorkflowFailureCount {
  workflow_id: string;
  workflow_name: string;
  environment: string;
  failure_count: number;
  total_runs: number;
}

/** One queue's current health classification. Mirrors `db::DashboardQueueHealth`. */
export interface DashboardQueueHealth {
  name: string;
  environment: string;
  capacity: number;
  max_queued: number | null;
  active_count: number;
  queued_count: number;
  utilization: number;
  status: "healthy" | "warn" | "degraded";
}

/** Current queue-health summary + tallies + echoed thresholds. Mirrors
 * `db::DashboardQueueHealthSummary`. */
export interface DashboardQueueHealthSummary {
  queues: DashboardQueueHealth[];
  healthy: number;
  warn: number;
  degraded: number;
  warn_utilization: number;
  degraded_backlog: number;
}

/** One workflow's rolling runtime baseline (expected p50 + mean) over a
 * trailing window. Mirrors `db::DashboardWorkflowBaseline`. */
export interface DashboardWorkflowBaseline {
  workflow_id: string;
  workflow_name: string;
  environment: string;
  sample_count: number;
  p50_runtime_seconds: number | null;
  mean_runtime_seconds: number | null;
}

/** One reason-taxonomy slice of the currently-blocked set. Mirrors
 * `db::DashboardBlockReasonCount`. `reason_category` is one of
 * `resource | event | host | workload | user | unknown`. */
export interface DashboardBlockReasonCount {
  reason_category: string;
  count: number;
  current_wait_seconds_total: number;
}

/** A workflow that is a heavy source of current blocking. Mirrors
 * `db::DashboardHeavyBlocker`. */
export interface DashboardHeavyBlocker {
  workflow_id: string;
  workflow_name: string;
  environment: string;
  blocked_count: number;
  sigma_wait_seconds: number;
}

/** Blocked/waiting reason taxonomy over (environment, lookback). Mirrors
 * `db::DashboardBlockTaxonomy`. The current-blocked set is always "now";
 * `lookback` scopes only the trailing admission-wait stats. */
export interface DashboardBlockTaxonomy {
  by_reason: DashboardBlockReasonCount[];
  current_blocked_count: number;
  current_wait_seconds_total: number;
  current_wait_seconds_max: number;
  trailing_wait_seconds_avg: number | null;
  trailing_wait_seconds_max: number | null;
  heavy_blockers: DashboardHeavyBlocker[];
}

/** One time bucket of queue-occupancy history. Mirrors
 * `db::DashboardQueueOccupancyBucket`. Each `(queue, sample)` is one data point;
 * `bucket` is the ISO-8601 UTC bucket start. Utilization is `running/capacity`
 * over samples with a positive capacity (else null). */
export interface DashboardQueueOccupancyBucket {
  bucket: string;
  avg_running: number | null;
  max_running: number | null;
  avg_queued: number | null;
  max_queued: number | null;
  avg_utilization: number | null;
  max_utilization: number | null;
  sample_count: number;
}

/** Queue-utilization history over (environment, lookback) + threshold bands.
 * Mirrors `db::DashboardQueueUtilizationHistory`. Backed by the periodic
 * queue-occupancy sampler. */
export interface DashboardQueueUtilizationHistory {
  grain: "hour" | "day";
  buckets: DashboardQueueOccupancyBucket[];
  warn_utilization: number;
  degraded_utilization: number;
}

/** Execution slots for one queue: running runs vs configured capacity. Mirrors
 * `db::DashboardExecutionSlotQueue`. `available` = max(capacity - running, 0). */
export interface DashboardExecutionSlotQueue {
  name: string;
  environment: string;
  running: number;
  capacity: number;
  available: number;
  utilization: number;
}

/** Execution-slot occupancy per queue + global. Mirrors
 * `db::DashboardExecutionSlots`. Global capacity is the scheduler-wide
 * `global_parallelism_cap`; global running is the sum of the queues' running. */
export interface DashboardExecutionSlots {
  queues: DashboardExecutionSlotQueue[];
  global_running: number;
  global_capacity: number;
  global_available: number;
  global_utilization: number;
}

/** Downstream blast-radius rollup for one workflow. Mirrors
 * `db::DashboardBlastRadius`. `*_downstream_count` is the count of distinct
 * downstream runs reachable via run_relationships chain edges; `max_depth` is
 * the longest such chain. Count/depth only — no DAG shape, no per-edge waits. */
export interface DashboardBlastRadius {
  workflow_id: string;
  workflow_name: string;
  environment: string;
  runs_considered: number;
  max_downstream_count: number;
  avg_downstream_count: number;
  max_depth: number;
}

export interface MissionControlNeedsAttentionItem {
  id: string;
  severity: string;
  title: string;
  detail: string;
  workflow_id?: string | null;
  workflow_name?: string | null;
  run_id?: string | null;
  target: string;
}

export interface MissionControlActivityItem {
  id: string;
  workflow_id: string;
  workflow_name: string;
  environment: string;
  domain: string;
  status: string;
  started_at: string;
  finished_at?: string | null;
  run_id: string;
}

export interface MissionControlUpcomingRun {
  workflow_id: string;
  workflow_name: string;
  environment: string;
  domain: string;
  trigger_kind: string;
  trigger_label: string;
  next_time: string;
}

export interface MissionControlFreshnessItem {
  asset_id: string;
  asset_kind: string;
  asset_namespace: string;
  asset_partition: string;
  last_action?: string | null;
  last_written_at?: string | null;
  workflow_id?: string | null;
  workflow_name?: string | null;
  environment?: string | null;
  domain: string;
  attribution: string;
}

export interface MissionControlWorkflowTelemetry {
  workflow_id: string;
  workflow_name: string;
  environment: string;
  domain: string;
  max_cpu_percent?: number | null;
  max_memory_rss_bytes?: number | null;
  sample_count: number;
  total_tokens: number;
  token_call_count: number;
}

export interface MissionControlPanelAvailability {
  panel: string;
  source_tables: string[];
  command: string;
  filter_behavior: string;
  empty_state: string;
  degraded_state: string;
  click_through_target?: string | null;
  persistence_required: boolean;
}

export interface MissionControlSnapshot {
  preferences: MissionControlPreferences;
  domains: DomainOption[];
  header: MissionControlHeader;
  sla: MissionControlSlaSummary;
  needs_attention: MissionControlNeedsAttentionItem[];
  needs_attention_total: number;
  needs_attention_truncated: boolean;
  live_activity: MissionControlActivityItem[];
  upcoming_runs: MissionControlUpcomingRun[];
  freshness_ledger: MissionControlFreshnessItem[];
  recent_runs: Run[];
  workflow_telemetry: MissionControlWorkflowTelemetry[];
  availability: MissionControlPanelAvailability[];
}

export interface QueueInfo {
  name: string;
  environment: string;
  capacity: number;
  tag_cap?: number | null;
  max_queued?: number | null;
  active_count: number;
  queued_count: number;
  global_parallelism_cap: number;
  updated_at: string;
}

export interface QueuedRun {
  id: string;
  run_id?: string | null;
  workflow_id: string;
  workflow_name?: string | null;
  queue_name: string;
  environment: string;
  priority: number;
  status: string;
  queued_at: string;
  admitted_at?: string | null;
  finished_at?: string | null;
  trigger_kind?: string | null;
  trigger_payload?: string | null;
  upstream_run_id?: string | null;
  input_json?: string | null;
  rerun_of_run_id?: string | null;
}

export interface DispatchOutcome {
  workflow_id: string;
  status: "admitted" | "queued" | "skipped" | "duplicate" | string;
  run_id?: string | null;
  queued_run_id?: string | null;
  queue_name: string;
  trigger_kind?: string | null;
  trigger_payload?: string | null;
  reason?: string | null;
}

export interface BackfillPlan {
  workflow_id: string;
  trigger_kind: "backfill";
  chain_suppressed: boolean;
  logical_dates: string[];
  count: number;
  dry_run: boolean;
}

export interface BackfillDispatchResult {
  plan: BackfillPlan;
  outcomes: DispatchOutcome[];
}

export interface SchedulerDeadLetter {
  id: string;
  run_id: string;
  workflow_id: string;
  workflow_name?: string | null;
  task_id?: string | null;
  last_attempt_id?: string | null;
  last_failure_at: string;
  last_exception: string;
  acknowledged_at?: string | null;
  acknowledged_reason?: string | null;
  acknowledged_by?: string | null;
  recovery_run_id?: string | null;
  run_status?: string | null;
  created_at: string;
  updated_at: string;
}

export interface RunRelationship {
  id: string;
  parent_run_id: string;
  child_run_id?: string | null;
  queued_run_id?: string | null;
  child_workflow_id: string;
  child_workflow_name?: string | null;
  relationship: string;
  task_id?: string | null;
  wait: boolean;
  status: string;
  reason?: string | null;
  details?: unknown;
  created_at: string;
  updated_at: string;
}

export interface RetentionPreview {
  cutoff: string;
  candidate_runs: number;
  preserved_dead_letter_runs: number;
  dry_run: boolean;
  deleted_runs: number;
}

export interface AvailableScript {
  name: string;
  path: string;
  description?: string | null;
}

export interface EmailConfig {
  enabled: boolean;
  alert_email: string;
  smtp_host: string;
  smtp_port: number;
  smtp_user: string;
  smtp_password: string;
  from_address: string;
  from_name: string;
}

/** A named, reusable email-delivery profile that workflows can select for
 * their failure alerts. Mirrors `db::EmailProfile`. */
export interface EmailProfile {
  id: string;
  name: string;
  enabled: boolean;
  alert_email: string;
  smtp_host: string;
  smtp_port: number;
  smtp_user: string;
  smtp_password: string;
  from_address: string;
  from_name: string;
  created_at?: string;
  updated_at?: string;
}

export interface ErrorAnalysis {
  diagnosis?: string;
  summary?: string;
  likely_cause?: string;
  recommended_steps?: string[];
  suggested_fix?: string;
  [key: string]: unknown;
}

export interface WorkflowPayload {
  name: string;
  description?: string;
  scriptPath: string;
  cronSchedule: string;
  asyncMode?: boolean;
  emailOnFailure?: boolean;
  timezone?: string;
  environment?: string;
  domain?: string | null;
  triggerConfig?: string;
  queueConfig?: string;
}

export interface WorkflowUpdatePayload extends WorkflowPayload {
  id: string;
  enabled: boolean;
}

/**
 * Error raised when a Tauri command is not (yet) registered by the backend.
 * The frontend guards forward-looking features (spec persistence, API-key
 * listing/revocation, environment updates, the updater) against this so a UI
 * shipped ahead of a backend command degrades gracefully instead of throwing an
 * opaque error.
 */
export class CommandUnavailableError extends Error {
  readonly command: string;
  constructor(command: string) {
    super(`Backend command "${command}" is not available yet.`);
    this.name = "CommandUnavailableError";
    this.command = command;
  }
}

/** Heuristic: Tauri surfaces an unknown command as a string mentioning it. */
export function isCommandUnavailable(err: unknown): boolean {
  if (err instanceof CommandUnavailableError) return true;
  const msg =
    typeof err === "string"
      ? err
      : err instanceof Error
        ? err.message
        : String(err);
  return /not\s+(?:found|allowed|registered)|unknown command|command .* not/i.test(
    msg,
  );
}

/** Invoke a command that may not exist yet; normalizes the "missing" case into
 * a {@link CommandUnavailableError} so callers can branch cleanly. */
async function invokeOptional<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    if (isCommandUnavailable(err)) throw new CommandUnavailableError(command);
    throw err;
  }
}

export function getAppConfig(): Promise<{
  chaos_labs_root?: string;
  workspace_root?: string;
  python_path: string;
}> {
  return invoke("get_app_config");
}

export function listWorkflows(): Promise<Workflow[]> {
  return invoke("list_workflows");
}

export function getWorkflow(id: string): Promise<Workflow> {
  return invoke("get_workflow", { id });
}

export function createWorkflow(payload: WorkflowPayload): Promise<Workflow> {
  return invoke("create_workflow", { ...payload });
}

export function updateWorkflow(
  payload: WorkflowUpdatePayload,
): Promise<Workflow> {
  return invoke("update_workflow", { ...payload });
}

export function deleteWorkflow(id: string): Promise<void> {
  return invoke("delete_workflow", { id });
}

export function triggerWorkflow(id: string): Promise<string> {
  return invoke("trigger_workflow", { id });
}

export function enqueueWorkflow(
  id: string,
  idempotencyKey?: string,
): Promise<DispatchOutcome> {
  return invoke("enqueue_workflow", { id, idempotencyKey });
}

export function rerunWorkflow(
  workflowId: string,
  sourceRunId?: string,
  inputOverrideJson?: string,
): Promise<string> {
  return invoke("rerun_workflow", {
    workflowId,
    sourceRunId,
    inputOverrideJson,
  });
}

export function planBackfill(
  workflowId: string,
  since: string,
  until: string,
  maxRuns?: number | null,
): Promise<BackfillPlan> {
  return invoke("plan_backfill", { workflowId, since, until, maxRuns });
}

export function dispatchBackfill(
  workflowId: string,
  since: string,
  until: string,
  maxRuns?: number | null,
  dryRun = false,
): Promise<BackfillDispatchResult> {
  return invoke("dispatch_backfill", {
    workflowId,
    since,
    until,
    maxRuns,
    dryRun,
  });
}

export function listDeadLetters(
  includeAcknowledged = false,
  limit = 50,
): Promise<SchedulerDeadLetter[]> {
  return invoke("list_dead_letters", { includeAcknowledged, limit });
}

export function acknowledgeDeadLetter(
  id: string,
  reason: string,
  operator?: string,
  reenableWorkflow = false,
): Promise<SchedulerDeadLetter> {
  return invoke("acknowledge_dead_letter", {
    id,
    reason,
    operator,
    reenableWorkflow,
  });
}

export function recoverDeadLetter(
  id: string,
  reenableWorkflow = false,
): Promise<DispatchOutcome> {
  return invoke("recover_dead_letter", { id, reenableWorkflow });
}

export function getRunHistory(
  workflowId: string,
  limit?: number,
): Promise<Run[]> {
  return invoke("get_run_history", { workflowId, limit });
}

export function getGlobalRunHistory(
  statusFilter = "all",
  triggerKind = "all",
  environmentFilter = "all",
  domainFilter = "all",
  limit = 100,
): Promise<Run[]> {
  return invoke("get_global_run_history", {
    statusFilter,
    triggerKind,
    environmentFilter,
    domainFilter,
    limit,
  });
}

export function cleanupRetention(
  olderThanDays: number,
  dryRun: boolean,
): Promise<RetentionPreview> {
  return invoke("cleanup_retention", { olderThanDays, dryRun });
}

export function getRunLog(runId: string): Promise<Run> {
  return invoke("get_run_log", { runId });
}

export function getRunTasks(runId: string): Promise<RunTask[]> {
  return invoke("get_run_tasks", { runId });
}

export function getRunAttempts(runId: string): Promise<RunAttempt[]> {
  return invoke("get_run_attempts", { runId });
}

export function getRunMetrics(runId: string): Promise<RunMetric[]> {
  return invoke("get_run_metrics", { runId });
}

export function getRunRelationships(runId: string): Promise<RunRelationship[]> {
  return invoke("get_run_relationships", { runId });
}

export function getWorkflowHistoryBuckets(
  workflowId: string,
  days = 30,
): Promise<WorkflowHistoryBucket[]> {
  return invoke("get_workflow_history_buckets", { workflowId, days });
}

export function getSlaViolations(): Promise<SlaViolation[]> {
  return invoke("get_sla_violations");
}

/** Windowed KPI roll-up (throughput/hr, avg + max runtime, success rate,
 * median + max wait) for the v3 dashboard. `lookback` accepts the shared
 * grammar (`1d`, `3d`, `7d`, `30d`, `<n>h`, `all`); defaults to `1d`. */
export function getDashboardKpiSummary(
  environmentFilter?: string,
  lookback?: string,
): Promise<DashboardKpiSummary> {
  return invoke("get_dashboard_kpi_summary", { environmentFilter, lookback });
}

/** Per-status run counts for the v3 status donut, scoped to `(environment,
 * lookback)`. `lookback` accepts the shared grammar; defaults to `1d`. */
export function getDashboardStatusDistribution(
  environmentFilter?: string,
  lookback?: string,
): Promise<DashboardStatusCount[]> {
  return invoke("get_dashboard_status_distribution", {
    environmentFilter,
    lookback,
  });
}

/** Cross-workflow success/fail trend for the v3 dashboard. The bucket grain
 * (`hour`/`day`) is chosen from the lookback and returned in the series.
 * `lookback` accepts the shared grammar; defaults to `1d`. */
export function getDashboardSuccessFailTrend(
  environmentFilter?: string,
  lookback?: string,
): Promise<DashboardTrendSeries> {
  return invoke("get_dashboard_success_fail_trend", {
    environmentFilter,
    lookback,
  });
}

/** Wait + runtime duration trends for the v3 dashboard, each bucketed at a
 * grain chosen from the lookback with a 30-day trailing-average baseline per
 * bucket. `lookback` accepts the shared grammar; defaults to `1d`. */
export function getDashboardWaitRuntimeTrend(
  environmentFilter?: string,
  lookback?: string,
): Promise<DashboardWaitRuntimeTrend> {
  return invoke("get_dashboard_wait_runtime_trend", {
    environmentFilter,
    lookback,
  });
}

/** Per-workflow failure recurrence for the v3 dashboard, worst first. Only
 * workflows with at least one failure in the window are returned. `lookback`
 * accepts the shared grammar; defaults to `1d`. */
export function getDashboardFailureRecurrence(
  environmentFilter?: string,
  lookback?: string,
): Promise<DashboardWorkflowFailureCount[]> {
  return invoke("get_dashboard_failure_recurrence", {
    environmentFilter,
    lookback,
  });
}

/** Current queue-health summary for the v3 dashboard. Reflects live occupancy,
 * so it takes no lookback. */
export function getDashboardQueueHealth(
  environmentFilter?: string,
): Promise<DashboardQueueHealthSummary> {
  return invoke("get_dashboard_queue_health", { environmentFilter });
}

/** Rolling per-workflow runtime baselines (expected p50 + mean) over a trailing
 * window. `lookback` accepts the shared grammar but defaults to `30d` (a
 * baseline needs a longer window than the live dashboard lookback). */
export function getDashboardWorkflowBaselines(
  environmentFilter?: string,
  lookback?: string,
): Promise<DashboardWorkflowBaseline[]> {
  return invoke("get_dashboard_workflow_baselines", {
    environmentFilter,
    lookback,
  });
}

/** Week-over-week KPI comparison (current vs prior equal window + deltas).
 * `lookback` accepts the shared grammar but defaults to `7d` (true
 * week-over-week); pass `1d` for day-over-day. */
export function getDashboardKpiWow(
  environmentFilter?: string,
  lookback?: string,
): Promise<DashboardKpiDelta> {
  return invoke("get_dashboard_kpi_wow", { environmentFilter, lookback });
}

/** Blocked/waiting reason taxonomy: the currently-queued set classified into
 * reason categories with Σ current wait, plus trailing admission-wait stats and
 * the heaviest per-workflow blockers. `lookback` (default `7d`) scopes only the
 * trailing stats; the current-blocked set is always "now". */
export function getDashboardBlockTaxonomy(
  environmentFilter?: string,
  lookback?: string,
): Promise<DashboardBlockTaxonomy> {
  return invoke("get_dashboard_block_taxonomy", {
    environmentFilter,
    lookback,
  });
}

/** Queue-utilization history: a per-bucket occupancy series over (environment,
 * lookback) at a grain chosen from the lookback, plus healthy/warn/degraded
 * utilization thresholds. Backed by the periodic queue-occupancy sampler. */
export function getDashboardQueueUtilizationHistory(
  environmentFilter?: string,
  lookback?: string,
): Promise<DashboardQueueUtilizationHistory> {
  return invoke("get_dashboard_queue_utilization_history", {
    environmentFilter,
    lookback,
  });
}

/** Execution slots: running runs vs configured concurrency capacity, per queue
 * and global. Live snapshot (no lookback). */
export function getDashboardExecutionSlots(
  environmentFilter?: string,
): Promise<DashboardExecutionSlots> {
  return invoke("get_dashboard_execution_slots", { environmentFilter });
}

/** Downstream blast-radius per workflow: from run_relationships chain edges, the
 * downstream dependent count + max chain depth of each in-window run, rolled up
 * per workflow. `lookback` defaults to `7d`. */
export function getDashboardBlastRadius(
  environmentFilter?: string,
  lookback?: string,
): Promise<DashboardBlastRadius[]> {
  return invoke("get_dashboard_blast_radius", { environmentFilter, lookback });
}

export function getSchedulerStatus(): Promise<SchedulerStatus> {
  return invoke("get_scheduler_status");
}

export function getMissionControlPreferences(): Promise<MissionControlPreferences> {
  return invoke("get_mission_control_preferences");
}

export function setMissionControlPreferences(
  defaultLanding: MissionControlPreferences["default_landing"],
  environmentFilter: MissionControlPreferences["environment_filter"],
  domainFilter: string,
): Promise<MissionControlPreferences> {
  return invoke("set_mission_control_preferences", {
    defaultLanding,
    environmentFilter,
    domainFilter,
  });
}

export function getMissionControlSnapshot(
  environmentFilter?: MissionControlPreferences["environment_filter"],
  domainFilter?: string,
): Promise<MissionControlSnapshot> {
  return invoke("get_mission_control_snapshot", {
    environmentFilter,
    domainFilter,
  });
}

export function listQueues(): Promise<QueueInfo[]> {
  return invoke("list_queues");
}

export function updateQueue(
  name: string,
  environment: string,
  capacity: number,
  tagCap?: number | null,
  maxQueued?: number | null,
): Promise<QueueInfo> {
  return invoke("update_queue", {
    name,
    environment,
    capacity,
    tagCap,
    maxQueued,
  });
}

export function listQueuedRuns(limit?: number): Promise<QueuedRun[]> {
  return invoke("list_queued_runs", { limit });
}

export function cancelQueuedRun(id: string): Promise<void> {
  return invoke("cancel_queued_run", { id });
}

export function listAvailableScripts(): Promise<AvailableScript[]> {
  return invoke("list_available_scripts");
}

export function openDashboard(): Promise<void> {
  return invoke("open_dashboard");
}

export function openRunDetail(
  runId: string,
  workflowId: string,
): Promise<void> {
  return invoke("open_run_detail", { runId, workflowId });
}

export function hidePopup(): Promise<void> {
  return invoke("hide_popup");
}

export function openUrl(url: string): Promise<void> {
  return invoke("open_url", { url });
}

export function quitApp(): Promise<void> {
  return invoke("quit_app");
}

export function getLaunchAtLogin(): Promise<boolean> {
  return invoke("get_launch_at_login");
}

export function setLaunchAtLogin(enabled: boolean): Promise<string> {
  return invoke("set_launch_at_login", { enabled });
}

export function setNotificationPrefs(
  notifyOnFailure: boolean,
  notifyOnSuccess: boolean,
): Promise<void> {
  return invoke("set_notification_prefs", { notifyOnFailure, notifyOnSuccess });
}

export function getNotificationPrefs(): Promise<{
  notify_on_failure: boolean;
  notify_on_success: boolean;
}> {
  return invoke("get_notification_prefs");
}

export function analyzeRunError(runId: string): Promise<ErrorAnalysis> {
  return invoke("analyze_run_error", { runId });
}

export function generateWorkflowDescription(
  scriptPath: string,
): Promise<string> {
  return invoke("generate_workflow_description", { scriptPath });
}

export function getEmailConfig(): Promise<EmailConfig> {
  return invoke("get_email_config");
}

export function setEmailConfig(config: EmailConfig): Promise<void> {
  return invoke("set_email_config", { config });
}

export function testEmailConfig(): Promise<{
  success?: boolean;
  error?: string;
}> {
  return invoke("test_email_config");
}

export function listEmailProfiles(): Promise<EmailProfile[]> {
  return invoke("list_email_profiles");
}

export function saveEmailProfile(profile: EmailProfile): Promise<EmailProfile> {
  return invoke("save_email_profile", { profile });
}

export function deleteEmailProfile(id: string): Promise<void> {
  return invoke("delete_email_profile", { id });
}

export function testEmailProfile(id: string): Promise<{
  success?: boolean;
  error?: string;
}> {
  return invoke("test_email_profile", { id });
}

export function setWorkflowEmailProfile(
  workflowId: string,
  profileId: string | null,
): Promise<void> {
  return invoke("set_workflow_email_profile", { workflowId, profileId });
}

// --- Environments (Phase 3) ---

export function listEnvironments(): Promise<Environment[]> {
  return invoke("list_environments");
}

export interface EnvironmentPayload {
  name: string;
  description?: string | null;
  workingDir?: string | null;
  defaultQueueCapacity?: number | null;
  defaultTagCap?: number | null;
  defaultMaxQueued?: number | null;
}

export function createEnvironment(
  payload: EnvironmentPayload,
): Promise<Environment> {
  return invoke("create_environment", { ...payload });
}

export function deleteEnvironment(id: string): Promise<void> {
  return invoke("delete_environment", { id });
}

/** Registered on the backend; still guarded via {@link invokeOptional} for older builds. */
export function updateEnvironment(
  id: string,
  payload: EnvironmentPayload,
): Promise<Environment> {
  return invokeOptional("update_environment", { id, ...payload });
}

// --- API keys & integrations (Phases 6 & 8) ---

export function createApiKey(
  name?: string,
  scopes?: ApiKeyScope[],
): Promise<NewApiKey> {
  return invoke("create_api_key", { name, scopes });
}

/** Registered on the backend; guarded via {@link invokeOptional} for older builds. */
export function listApiKeys(): Promise<ApiKey[]> {
  return invokeOptional("list_api_keys");
}

/** Registered on the backend; guarded via {@link invokeOptional} for older builds. */
export function revokeApiKey(id: string): Promise<void> {
  return invokeOptional("revoke_api_key", { id });
}

// --- Workflow spec (Phases 4 & 5) ---

/**
 * Persist a workflow's execution spec (kind + steps/operator + actions).
 * Registered on the backend; guarded via {@link invokeOptional} so the editor
 * can still save base fields against older builds.
 */
export function setWorkflowSpec(
  id: string,
  spec: WorkflowSpec,
): Promise<Workflow> {
  return invokeOptional("set_workflow_spec", { id, spec });
}

// --- Updater (Phase 11) ---

/** Triggers a manual update check, routed through the same backend
 * `run_check` code path as the launch/6h background timer (so there is only
 * one code path that ever talks to the updater plugin). The resulting
 * snapshot arrives via the `update-status` event (see {@link useAppUpdate}'s
 * `checkNow`) — this call's own return value is intentionally discarded by
 * callers. Registered on the backend; guarded via {@link invokeOptional} so
 * the Settings affordance renders on older builds. */
export function checkForUpdate(): Promise<void> {
  return invokeOptional("check_for_update");
}

/** Hydrates the current updater snapshot without touching the network —
 * used on mount, before the first `update-status` event arrives. */
export function getAppUpdateStatus(): Promise<UpdateSnapshot> {
  return invokeOptional("get_app_update_status");
}

/** Persists background-check / skip preferences and returns the resulting
 * snapshot. `clearSkip: true` takes precedence over `skippedVersion`. */
export function setUpdaterPreferences(
  patch: UpdaterPreferencesPatch,
): Promise<UpdateSnapshot> {
  return invokeOptional("set_updater_preferences", {
    backgroundCheckEnabled: patch.backgroundCheckEnabled,
    skippedVersion: patch.skippedVersion,
    clearSkip: patch.clearSkip,
  });
}

/**
 * Download + install the pending update and relaunch. `expectedVersion`
 * should be the version the user was shown when they clicked "Install" —
 * the backend refuses the call (Section 6 consent guard) if a newer version
 * has since replaced it. Registered on the backend; guarded via
 * {@link invokeOptional} for older builds.
 *
 * Resolves with the snapshot only for the "nothing to install" case; a real
 * install restarts the app, so the underlying `invoke` call never gets a
 * response.
 */
export function applyUpdate(expectedVersion?: string): Promise<UpdateSnapshot> {
  return invokeOptional("apply_update", { expectedVersion });
}

// --- Managed MCP/SDK integration (Section 12) ---

/** Mirrors `src-tauri/src/mcp.rs` `InstallStatus`. */
export type McpInstallStatus =
  "not_installed" | "installed" | "stale" | "node_unavailable";

/** Mirrors `src-tauri/src/mcp.rs` `McpIntegrationStatus`. */
export interface McpIntegrationStatus {
  enabled: boolean;
  install_status: McpInstallStatus;
  node_available: boolean;
  node_path?: string | null;
  npm_available: boolean;
  npm_path?: string | null;
  provisioned_version?: string | null;
  pinned_version: string;
  registered_in_cursor: boolean;
  cursor_config_conflict: boolean;
  api_reachable: boolean;
  managed_key_id?: string | null;
  matches: boolean;
  last_error?: string | null;
}

/** Read-only status for the managed-MCP Integrations card. Registered on the
 * backend; guarded via {@link invokeOptional} for older builds. */
export function getMcpIntegrationStatus(): Promise<McpIntegrationStatus> {
  return invokeOptional("get_mcp_integration_status");
}

/** Enable (or repair/re-provision) the managed integration. `force` takes
 * over a pre-existing unmanaged `chaos-scheduler` Cursor config entry.
 * Registered on the backend; guarded via {@link invokeOptional} for older
 * builds. */
export function provisionMcpIntegration(
  force = false,
): Promise<McpIntegrationStatus> {
  return invokeOptional("provision_mcp_integration", { force });
}

/** Remove the managed integration (config entry, install dir, API key).
 * `prepareToUninstall` additionally removes the launch-at-login agent.
 * Registered on the backend; guarded via {@link invokeOptional} for older
 * builds. */
export function removeMcpIntegration(
  prepareToUninstall = false,
): Promise<McpIntegrationStatus> {
  return invokeOptional("remove_mcp_integration", { prepareToUninstall });
}
