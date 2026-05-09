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
  timezone: string;
  trigger_config?: string | null;
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
  triggerConfig?: string;
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

export function getSchedulerStatus(): Promise<SchedulerStatus> {
  return invoke("get_scheduler_status");
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
