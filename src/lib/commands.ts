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
  return invoke("set_email_config", { ...config });
}

export function testEmailConfig(): Promise<{ success?: boolean; error?: string }> {
  return invoke("test_email_config");
}
