import { invoke } from "@tauri-apps/api/core";

export type WorkflowCorpus = "source" | "instance";

export interface Workflow {
  id: string;
  name: string;
  description: string | null;
  script_path: string;
  cron_schedule: string;
  enabled: boolean;
  async_mode: boolean;
  email_on_failure: boolean;
  corpus: WorkflowCorpus;
  domain?: string | null;
  timezone: string;
  trigger_config?: string | null;
  queue_config?: string | null;
  last_run_at: string | null;
  created_at: string;
  updated_at: string;
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
  corpus: WorkflowCorpus;
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
  corpus?: string | null;
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
  | "corpus"
  | "domain"
  | "queue_name"
  | "provider"
  | "model"
  | "token_kind";

export interface NextRun {
  workflow_id: string;
  workflow_name: string;
  corpus: WorkflowCorpus;
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
  corpus_filter: "all" | WorkflowCorpus;
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
  long_running_count: number;
  blocked_count: number;
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
  corpus: WorkflowCorpus;
  domain: string;
  status: string;
  started_at: string;
  finished_at?: string | null;
  run_id: string;
}

export interface MissionControlUpcomingRun {
  workflow_id: string;
  workflow_name: string;
  corpus: WorkflowCorpus;
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
  corpus?: WorkflowCorpus | null;
  domain: string;
  attribution: string;
}

export interface MissionControlWorkflowTelemetry {
  workflow_id: string;
  workflow_name: string;
  corpus: WorkflowCorpus;
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
  corpus: WorkflowCorpus;
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
  corpus: WorkflowCorpus;
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
  corpus?: WorkflowCorpus;
  domain?: string | null;
  triggerConfig?: string;
  queueConfig?: string;
}

export interface WorkflowUpdatePayload extends WorkflowPayload {
  id: string;
  enabled: boolean;
}

export function getAppConfig(): Promise<{ chaos_labs_root: string; python_path: string }> {
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

export function updateWorkflow(payload: WorkflowUpdatePayload): Promise<Workflow> {
  return invoke("update_workflow", { ...payload });
}

export function deleteWorkflow(id: string): Promise<void> {
  return invoke("delete_workflow", { id });
}

export function triggerWorkflow(id: string): Promise<string> {
  return invoke("trigger_workflow", { id });
}

export function rerunWorkflow(
  workflowId: string,
  sourceRunId?: string,
  inputOverrideJson?: string,
): Promise<string> {
  return invoke("rerun_workflow", { workflowId, sourceRunId, inputOverrideJson });
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
  return invoke("dispatch_backfill", { workflowId, since, until, maxRuns, dryRun });
}

export function listDeadLetters(
  includeAcknowledged = false,
  limit = 50,
): Promise<SchedulerDeadLetter[]> {
  return invoke("list_dead_letters", { includeAcknowledged, limit });
}

export function getDeadLetter(id: string): Promise<SchedulerDeadLetter> {
  return invoke("get_dead_letter", { id });
}

export function acknowledgeDeadLetter(
  id: string,
  reason: string,
  operator?: string,
  reenableWorkflow = false,
): Promise<SchedulerDeadLetter> {
  return invoke("acknowledge_dead_letter", { id, reason, operator, reenableWorkflow });
}

export function recoverDeadLetter(
  id: string,
  reenableWorkflow = false,
): Promise<DispatchOutcome> {
  return invoke("recover_dead_letter", { id, reenableWorkflow });
}

export function getRunHistory(workflowId: string, limit?: number): Promise<Run[]> {
  return invoke("get_run_history", { workflowId, limit });
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


export function queryResourceSamples(
  workflowId: string,
  timeWindow = "24h",
): Promise<WorkflowResourceSample[]> {
  return invoke("query_resource_samples", { workflowId, timeWindow });
}

export function queryTokenUsageRollup(
  groupBy?: TokenRollupDimension[],
  timeWindow = "24h",
  timeBucket: "minute" | "hour" | "day" = "hour",
): Promise<WorkflowTokenUsageRollup[]> {
  return invoke("query_token_usage_rollup", { groupBy, timeWindow, timeBucket });
}

export function getSchedulerStatus(): Promise<SchedulerStatus> {
  return invoke("get_scheduler_status");
}

export function getMissionControlPreferences(): Promise<MissionControlPreferences> {
  return invoke("get_mission_control_preferences");
}

export function setMissionControlPreferences(
  defaultLanding: MissionControlPreferences["default_landing"],
  corpusFilter: MissionControlPreferences["corpus_filter"],
  domainFilter: string,
): Promise<MissionControlPreferences> {
  return invoke("set_mission_control_preferences", {
    defaultLanding,
    corpusFilter,
    domainFilter,
  });
}

export function getMissionControlSnapshot(
  corpusFilter?: MissionControlPreferences["corpus_filter"],
  domainFilter?: string,
): Promise<MissionControlSnapshot> {
  return invoke("get_mission_control_snapshot", { corpusFilter, domainFilter });
}

export function queryStaleAssets(maxAgeSeconds = 24 * 60 * 60, assetKind?: string): Promise<SchedulerAsset[]> {
  return invoke("query_stale_assets", { maxAgeSeconds, assetKind });
}

export function listQueues(): Promise<QueueInfo[]> {
  return invoke("list_queues");
}

export function updateQueue(
  name: string,
  corpus: WorkflowCorpus,
  capacity: number,
  tagCap?: number | null,
  maxQueued?: number | null,
): Promise<QueueInfo> {
  return invoke("update_queue", { name, corpus, capacity, tagCap, maxQueued });
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

export function openRunDetail(runId: string, workflowId: string): Promise<void> {
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

export function generateWorkflowDescription(scriptPath: string): Promise<string> {
  return invoke("generate_workflow_description", { scriptPath });
}

export function getEmailConfig(): Promise<EmailConfig> {
  return invoke("get_email_config");
}

export function setEmailConfig(config: EmailConfig): Promise<void> {
  return invoke("set_email_config", { config });
}

export function testEmailConfig(): Promise<{ success?: boolean; error?: string }> {
  return invoke("test_email_config");
}
