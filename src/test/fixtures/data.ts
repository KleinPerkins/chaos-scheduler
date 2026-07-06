import type {
  ApiKey,
  EmailConfig,
  Environment,
  McpIntegrationStatus,
  MissionControlPreferences,
  MissionControlSnapshot,
  QueueInfo,
  Run,
  SchedulerStatus,
  UpdateSnapshot,
  Workflow,
} from "../../lib/commands";

const NOW = "2026-07-04T12:00:00.000Z";

export const sampleWorkflow: Workflow = {
  id: "wf-demo-1",
  name: "Nightly sync",
  description: "Demo workflow for harness tests",
  script_path: "/tmp/demo.sh",
  cron_schedule: "0 9 * * *",
  enabled: true,
  async_mode: false,
  email_on_failure: false,
  environment: "source",
  managed_externally: false,
  kind: "generic",
  spec_json: null,
  domain: "ops",
  timezone: "UTC",
  trigger_config: null,
  queue_config: null,
  last_run_at: null,
  created_at: NOW,
  updated_at: NOW,
};

export const sampleManagedWorkflow: Workflow = {
  ...sampleWorkflow,
  id: "wf-managed-1",
  name: "Managed import",
  managed_externally: true,
  environment: "instance",
};

export const sampleRun: Run = {
  id: "run-demo-1",
  workflow_id: sampleWorkflow.id,
  started_at: NOW,
  finished_at: NOW,
  exit_code: 0,
  stdout: "ok",
  stderr: null,
  result_url: null,
  status: "succeeded",
  workflow_name: sampleWorkflow.name,
  trigger_kind: "manual",
};

export const sampleEnvironments: Environment[] = [
  {
    id: "env-source",
    name: "source",
    description: "Default source environment",
    workflow_count: 1,
    managed_externally: false,
    created_at: NOW,
    updated_at: NOW,
  },
  {
    id: "env-instance",
    name: "instance",
    description: "Managed instance environment",
    workflow_count: 1,
    managed_externally: true,
    created_at: NOW,
    updated_at: NOW,
  },
];

export const defaultMissionControlPreferences: MissionControlPreferences = {
  default_landing: "mission_control",
  environment_filter: "all",
  domain_filter: "all",
};

export const emptyMissionControlSnapshot: MissionControlSnapshot = {
  preferences: defaultMissionControlPreferences,
  domains: [{ value: "ops", label: "ops", workflow_count: 1 }],
  header: {
    active_workflows: 1,
    running_count: 0,
    queued_count: 0,
    recent_failures: 0,
  },
  sla: {
    violations_count: 0,
    success_rate_24h: 1,
    median_wait_seconds: 0,
    long_running_count: 0,
    blocked_count: 0,
  },
  needs_attention: [],
  needs_attention_total: 0,
  needs_attention_truncated: false,
  live_activity: [],
  upcoming_runs: [],
  freshness_ledger: [],
  recent_runs: [sampleRun],
  workflow_telemetry: [],
  availability: [],
};

export const emptySchedulerStatus: SchedulerStatus = {
  active_workflows: 1,
  running_count: 0,
  next_runs: [],
  recent_runs: [sampleRun],
};

export const defaultEmailConfig: EmailConfig = {
  enabled: false,
  alert_email: "",
  smtp_host: "",
  smtp_port: 587,
  smtp_user: "",
  smtp_password: "",
  from_address: "",
  from_name: "Chaos Scheduler",
};

export const defaultQueues: QueueInfo[] = [
  {
    name: "default",
    environment: "source",
    capacity: 2,
    tag_cap: null,
    max_queued: null,
    active_count: 0,
    queued_count: 0,
    global_parallelism_cap: 4,
    updated_at: NOW,
  },
];

export const emptyApiKeys: ApiKey[] = [];

export const idleUpdateSnapshot: UpdateSnapshot = {
  updater_available: true,
  phase: "idle",
  current_version: "0.1.0",
  latest_version: null,
  notes: null,
  last_checked_at: null,
  last_error: null,
  progress: null,
  background_check_enabled: true,
  skipped_version: null,
};

export const availableUpdateSnapshot: UpdateSnapshot = {
  ...idleUpdateSnapshot,
  phase: "available",
  latest_version: "0.2.0",
  notes: "Bug fixes and improvements.",
  last_checked_at: NOW,
};

/** Disabled/not-yet-provisioned managed-MCP integration — the default state
 * for a fresh install before the user opts in from Integrations. */
export const defaultMcpIntegrationStatus: McpIntegrationStatus = {
  enabled: false,
  install_status: "not_installed",
  node_available: true,
  node_path: "/usr/local/bin/node",
  npm_available: true,
  npm_path: "/usr/local/bin/npm",
  provisioned_version: null,
  pinned_version: "0.5.0",
  registered_in_cursor: false,
  cursor_config_conflict: false,
  api_reachable: true,
  managed_key_id: null,
  matches: false,
  last_error: null,
};
