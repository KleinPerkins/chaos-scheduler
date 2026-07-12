import type {
  ApiKey,
  DashboardBlastRadius,
  DashboardBlockTaxonomy,
  DashboardExecutionSlots,
  DashboardKpiDelta,
  DashboardKpiSummary,
  DashboardQueueHealthSummary,
  DashboardQueueUtilizationHistory,
  DashboardStatusCount,
  DashboardTrendSeries,
  DashboardWaitRuntimeTrend,
  DashboardWorkflowBaseline,
  DashboardWorkflowFailureCount,
  EmailConfig,
  Environment,
  McpIntegrationStatus,
  MissionControlActivityItem,
  MissionControlPreferences,
  MissionControlSnapshot,
  MissionControlUpcomingRun,
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
  environment: "production",
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
  environment: "production",
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
    id: "env-production",
    name: "production",
    description: "Default production environment",
    workflow_count: 1,
    managed_externally: false,
    created_at: NOW,
    updated_at: NOW,
  },
  {
    id: "env-sandbox",
    name: "sandbox",
    description: "Sandbox environment for integration tests",
    workflow_count: 0,
    managed_externally: false,
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
    max_wait_seconds: 0,
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

/** Three running jobs for the Overview race hero (elapsed = NOW − started_at):
 * ~20m, ~44m, and ~78m in, joined to the baselines below by workflow_id. */
export const runningActivity: MissionControlActivityItem[] = [
  {
    id: "act-nightly",
    workflow_id: "wf-nightly-sync",
    workflow_name: "Nightly sync",
    environment: "production",
    domain: "ops",
    status: "running",
    started_at: "2026-07-04T11:40:00.000Z",
    finished_at: null,
    run_id: "run-live-nightly",
  },
  {
    id: "act-etl",
    workflow_id: "wf-etl-rollup",
    workflow_name: "ETL rollup",
    environment: "production",
    domain: "data",
    status: "running",
    started_at: "2026-07-04T11:16:00.000Z",
    finished_at: null,
    run_id: "run-live-etl",
  },
  {
    id: "act-ml",
    workflow_id: "wf-ml-scoring",
    workflow_name: "ML scoring",
    environment: "sandbox",
    domain: "ml",
    status: "running",
    started_at: "2026-07-04T10:42:00.000Z",
    finished_at: null,
    run_id: "run-live-ml",
  },
];

/** Recently-finished failures for the Agent Activity "Recent failures" section.
 * These carry a terminal unsuccessful status + a `finished_at` before NOW so the
 * activity view can compute "N ago". They are appended to `live_activity` after
 * the running jobs; the Overview race hero filters `live_activity` to the
 * running status, so these never perturb the race hero. */
export const recentFailures: MissionControlActivityItem[] = [
  {
    id: "act-fail-export",
    workflow_id: "wf-data-export",
    workflow_name: "Data export",
    environment: "production",
    domain: "data",
    status: "failed",
    started_at: "2026-07-04T11:38:00.000Z",
    finished_at: "2026-07-04T11:50:00.000Z", // 10m ago
    run_id: "run-fail-export",
  },
  {
    id: "act-fail-index",
    workflow_id: "wf-search-index",
    workflow_name: "Search reindex",
    environment: "sandbox",
    domain: "ml",
    status: "timed_out",
    started_at: "2026-07-04T11:05:00.000Z",
    finished_at: "2026-07-04T11:35:00.000Z", // 25m ago
    run_id: "run-fail-index",
  },
];

/** Upcoming fixed-time cron triggers for the Agent Activity "Upcoming" section
 * (next_time after NOW so the ETA is positive). */
export const sampleUpcomingRuns: MissionControlUpcomingRun[] = [
  {
    workflow_id: "wf-nightly-sync",
    workflow_name: "Nightly sync",
    environment: "production",
    domain: "ops",
    trigger_kind: "cron",
    trigger_label: "0 15 * * *",
    next_time: "2026-07-04T15:00:00.000Z", // in 3h
  },
  {
    workflow_id: "wf-weekly-report",
    workflow_name: "Weekly report",
    environment: "production",
    domain: "ops",
    trigger_kind: "cron",
    trigger_label: "0 8 * * 1",
    next_time: "2026-07-05T08:00:00.000Z", // in 20h
  },
];

/** Populated Mission Control snapshot used as the default IPC fixture: the
 * empty snapshot plus the running jobs (so the Overview race hero renders) and
 * the Agent Activity feed (running + upcoming + recent failures). */
export const dashboardMissionControlSnapshot: MissionControlSnapshot = {
  ...emptyMissionControlSnapshot,
  header: { ...emptyMissionControlSnapshot.header, running_count: 3 },
  live_activity: [...runningActivity, ...recentFailures],
  upcoming_runs: sampleUpcomingRuns,
};

/** Per-workflow runtime baselines feeding the race-hero finish lines (p50 =
 * expected). Keyed to `runningActivity` by workflow_id. */
export const sampleDashboardWorkflowBaselines: DashboardWorkflowBaseline[] = [
  {
    workflow_id: "wf-nightly-sync",
    workflow_name: "Nightly sync",
    environment: "production",
    sample_count: 42,
    p50_runtime_seconds: 1800, // 30m → 20m elapsed ≈ 67%
    mean_runtime_seconds: 1920,
  },
  {
    workflow_id: "wf-etl-rollup",
    workflow_name: "ETL rollup",
    environment: "production",
    sample_count: 30,
    p50_runtime_seconds: 3600, // 60m → 44m elapsed ≈ 73%
    mean_runtime_seconds: 3720,
  },
  {
    workflow_id: "wf-ml-scoring",
    workflow_name: "ML scoring",
    environment: "sandbox",
    sample_count: 18,
    p50_runtime_seconds: 3600, // 60m → 78m elapsed = 130% (overrunning)
    mean_runtime_seconds: 3400,
  },
];

/** Windowed KPI roll-up (1d) for the Overview KPI strip. */
export const sampleDashboardKpiSummary: DashboardKpiSummary = {
  total_runs: 128,
  succeeded: 120,
  failed: 8,
  success_rate: 0.9375,
  throughput_per_hour: 5.3,
  avg_runtime_seconds: 372,
  max_runtime_seconds: 5400,
  median_wait_seconds: 42,
  max_wait_seconds: 318,
  window_seconds: 86400,
};

/** Week-over-window KPI deltas (current vs prior equal window). */
export const sampleDashboardKpiWow: DashboardKpiDelta = {
  current: sampleDashboardKpiSummary,
  previous: {
    ...sampleDashboardKpiSummary,
    succeeded: 112,
    failed: 10,
    success_rate: 0.9167,
    throughput_per_hour: 4.9,
    avg_runtime_seconds: 390,
    max_wait_seconds: 292,
  },
  total_runs_delta: 6,
  succeeded_delta: 8,
  failed_delta: -2,
  success_rate_delta: 0.0208,
  throughput_per_hour_delta: 0.4,
  avg_runtime_seconds_delta: -18,
  max_runtime_seconds_delta: 120,
  median_wait_seconds_delta: -3,
  max_wait_seconds_delta: 26,
};

/** Per-status run counts for the status-distribution donut. */
export const sampleDashboardStatusDistribution: DashboardStatusCount[] = [
  { status: "succeeded", count: 120 },
  { status: "failed", count: 8 },
  { status: "running", count: 3 },
  { status: "cancelled", count: 2 },
];

/** Hourly success/fail trend (eight buckets ending at NOW). */
export const sampleDashboardSuccessFailTrend: DashboardTrendSeries = {
  grain: "hour",
  buckets: [
    { bucket: "2026-07-04T05:00:00.000Z", total: 14, failed: 1, succeeded: 13 },
    { bucket: "2026-07-04T06:00:00.000Z", total: 16, failed: 0, succeeded: 16 },
    { bucket: "2026-07-04T07:00:00.000Z", total: 15, failed: 2, succeeded: 13 },
    { bucket: "2026-07-04T08:00:00.000Z", total: 18, failed: 1, succeeded: 17 },
    { bucket: "2026-07-04T09:00:00.000Z", total: 17, failed: 0, succeeded: 17 },
    { bucket: "2026-07-04T10:00:00.000Z", total: 19, failed: 3, succeeded: 16 },
    { bucket: "2026-07-04T11:00:00.000Z", total: 16, failed: 1, succeeded: 15 },
    { bucket: "2026-07-04T12:00:00.000Z", total: 13, failed: 0, succeeded: 13 },
  ],
};

/** Hourly wait + runtime duration trends (eight buckets ending at NOW), each
 * with a 30-day trailing-average baseline. Seconds; the Operational Health
 * drill-down plots them in minutes. Latest runtime avg sits just above its
 * baseline (a mild "up" trend, below the regression margin). */
export const sampleDashboardWaitRuntimeTrend: DashboardWaitRuntimeTrend = {
  grain: "hour",
  wait: [
    {
      bucket: "2026-07-04T05:00:00.000Z",
      avg_seconds: 38,
      max_seconds: 210,
      count: 14,
      baseline_avg_seconds: 45,
    },
    {
      bucket: "2026-07-04T06:00:00.000Z",
      avg_seconds: 41,
      max_seconds: 190,
      count: 16,
      baseline_avg_seconds: 45,
    },
    {
      bucket: "2026-07-04T07:00:00.000Z",
      avg_seconds: 52,
      max_seconds: 264,
      count: 15,
      baseline_avg_seconds: 45,
    },
    {
      bucket: "2026-07-04T08:00:00.000Z",
      avg_seconds: 47,
      max_seconds: 248,
      count: 18,
      baseline_avg_seconds: 46,
    },
    {
      bucket: "2026-07-04T09:00:00.000Z",
      avg_seconds: 44,
      max_seconds: 233,
      count: 17,
      baseline_avg_seconds: 46,
    },
    {
      bucket: "2026-07-04T10:00:00.000Z",
      avg_seconds: 58,
      max_seconds: 312,
      count: 19,
      baseline_avg_seconds: 46,
    },
    {
      bucket: "2026-07-04T11:00:00.000Z",
      avg_seconds: 49,
      max_seconds: 271,
      count: 16,
      baseline_avg_seconds: 47,
    },
    {
      bucket: "2026-07-04T12:00:00.000Z",
      avg_seconds: 42,
      max_seconds: 226,
      count: 13,
      baseline_avg_seconds: 47,
    },
  ],
  runtime: [
    {
      bucket: "2026-07-04T05:00:00.000Z",
      avg_seconds: 352,
      max_seconds: 4200,
      count: 14,
      baseline_avg_seconds: 360,
    },
    {
      bucket: "2026-07-04T06:00:00.000Z",
      avg_seconds: 368,
      max_seconds: 3900,
      count: 16,
      baseline_avg_seconds: 360,
    },
    {
      bucket: "2026-07-04T07:00:00.000Z",
      avg_seconds: 401,
      max_seconds: 5100,
      count: 15,
      baseline_avg_seconds: 362,
    },
    {
      bucket: "2026-07-04T08:00:00.000Z",
      avg_seconds: 389,
      max_seconds: 4700,
      count: 18,
      baseline_avg_seconds: 362,
    },
    {
      bucket: "2026-07-04T09:00:00.000Z",
      avg_seconds: 372,
      max_seconds: 4400,
      count: 17,
      baseline_avg_seconds: 364,
    },
    {
      bucket: "2026-07-04T10:00:00.000Z",
      avg_seconds: 418,
      max_seconds: 5400,
      count: 19,
      baseline_avg_seconds: 364,
    },
    {
      bucket: "2026-07-04T11:00:00.000Z",
      avg_seconds: 395,
      max_seconds: 4900,
      count: 16,
      baseline_avg_seconds: 366,
    },
    {
      bucket: "2026-07-04T12:00:00.000Z",
      avg_seconds: 388,
      max_seconds: 4600,
      count: 13,
      baseline_avg_seconds: 366,
    },
  ],
};

/** Live queue health with one degraded + one warn queue (so the SLA banner
 * renders); queue depth (waiting) sums to 14. */
export const sampleDashboardQueueHealth: DashboardQueueHealthSummary = {
  queues: [
    {
      name: "default",
      environment: "production",
      capacity: 4,
      max_queued: null,
      active_count: 3,
      queued_count: 5,
      utilization: 0.75,
      status: "warn",
    },
    {
      name: "ml",
      environment: "sandbox",
      capacity: 2,
      max_queued: null,
      active_count: 2,
      queued_count: 9,
      utilization: 1,
      status: "degraded",
    },
    {
      name: "batch",
      environment: "production",
      capacity: 6,
      max_queued: null,
      active_count: 1,
      queued_count: 0,
      utilization: 0.17,
      status: "healthy",
    },
  ],
  healthy: 1,
  warn: 1,
  degraded: 1,
  warn_utilization: 0.7,
  degraded_backlog: 8,
};

/** Execution-slot occupancy: three running (matches `runningActivity`). */
export const sampleDashboardExecutionSlots: DashboardExecutionSlots = {
  queues: [
    {
      name: "default",
      environment: "production",
      running: 2,
      capacity: 4,
      available: 2,
      utilization: 0.5,
    },
    {
      name: "ml",
      environment: "sandbox",
      running: 1,
      capacity: 2,
      available: 1,
      utilization: 0.5,
    },
  ],
  global_running: 3,
  global_capacity: 12,
  global_available: 9,
  global_utilization: 0.25,
};

/** Queue-utilization history (hourly) climbing from healthy into the warn and
 * degraded bands mid-window, so the threshold-zone chart shows a meaningful
 * crossing. Utilizations are fractions (0–1); thresholds mirror queue health
 * (warn 0.7, degraded 0.9). Window aligns with the wait/runtime trend. */
export const sampleDashboardQueueUtilizationHistory: DashboardQueueUtilizationHistory =
  {
    grain: "hour",
    warn_utilization: 0.7,
    degraded_utilization: 0.9,
    buckets: [
      {
        bucket: "2026-07-04T05:00:00.000Z",
        avg_running: 6.2,
        max_running: 8,
        avg_queued: 2.1,
        max_queued: 5,
        avg_utilization: 0.52,
        max_utilization: 0.7,
        sample_count: 12,
      },
      {
        bucket: "2026-07-04T06:00:00.000Z",
        avg_running: 6.9,
        max_running: 9,
        avg_queued: 2.8,
        max_queued: 6,
        avg_utilization: 0.58,
        max_utilization: 0.78,
        sample_count: 12,
      },
      {
        bucket: "2026-07-04T07:00:00.000Z",
        avg_running: 7.5,
        max_running: 10,
        avg_queued: 3.4,
        max_queued: 7,
        avg_utilization: 0.63,
        max_utilization: 0.83,
        sample_count: 12,
      },
      {
        bucket: "2026-07-04T08:00:00.000Z",
        avg_running: 8.5,
        max_running: 11,
        avg_queued: 4.6,
        max_queued: 9,
        avg_utilization: 0.71,
        max_utilization: 0.92,
        sample_count: 12,
      },
      {
        bucket: "2026-07-04T09:00:00.000Z",
        avg_running: 8.1,
        max_running: 11,
        avg_queued: 4.0,
        max_queued: 8,
        avg_utilization: 0.68,
        max_utilization: 0.88,
        sample_count: 12,
      },
      {
        bucket: "2026-07-04T10:00:00.000Z",
        avg_running: 9.4,
        max_running: 12,
        avg_queued: 5.8,
        max_queued: 11,
        avg_utilization: 0.79,
        max_utilization: 0.97,
        sample_count: 12,
      },
      {
        bucket: "2026-07-04T11:00:00.000Z",
        avg_running: 8.9,
        max_running: 11,
        avg_queued: 5.1,
        max_queued: 10,
        avg_utilization: 0.74,
        max_utilization: 0.91,
        sample_count: 12,
      },
      {
        bucket: "2026-07-04T12:00:00.000Z",
        avg_running: 7.9,
        max_running: 10,
        avg_queued: 3.9,
        max_queued: 8,
        avg_utilization: 0.66,
        max_utilization: 0.85,
        sample_count: 12,
      },
    ],
  };

/** Blocked/waiting reason taxonomy + heaviest blockers for the Needs Attention
 * drill-down. 9 jobs waiting across three reason categories. */
export const sampleDashboardBlockTaxonomy: DashboardBlockTaxonomy = {
  by_reason: [
    { reason_category: "resource", count: 5, current_wait_seconds_total: 9720 },
    { reason_category: "event", count: 3, current_wait_seconds_total: 4800 },
    { reason_category: "host", count: 1, current_wait_seconds_total: 720 },
  ],
  current_blocked_count: 9,
  current_wait_seconds_total: 15240,
  current_wait_seconds_max: 3600,
  trailing_wait_seconds_avg: 420,
  trailing_wait_seconds_max: 1800,
  heavy_blockers: [
    {
      workflow_id: "wf-etl-rollup",
      workflow_name: "ETL rollup",
      environment: "production",
      blocked_count: 4,
      sigma_wait_seconds: 8100,
    },
    {
      workflow_id: "wf-nightly-sync",
      workflow_name: "Nightly sync",
      environment: "production",
      blocked_count: 3,
      sigma_wait_seconds: 5400,
    },
    {
      workflow_id: "wf-ml-scoring",
      workflow_name: "ML scoring",
      environment: "sandbox",
      blocked_count: 2,
      sigma_wait_seconds: 1740,
    },
  ],
};

/** Downstream blast-radius rollup for the Needs Attention outliers. One row has
 * zero downstream reach (exercises the "not an outlier" filter). */
export const sampleDashboardBlastRadius: DashboardBlastRadius[] = [
  {
    workflow_id: "wf-ingest",
    workflow_name: "Ingest fan-out",
    environment: "production",
    runs_considered: 12,
    max_downstream_count: 9,
    avg_downstream_count: 4.2,
    max_depth: 4,
  },
  {
    workflow_id: "wf-etl-rollup",
    workflow_name: "ETL rollup",
    environment: "production",
    runs_considered: 8,
    max_downstream_count: 5,
    avg_downstream_count: 3,
    max_depth: 3,
  },
  {
    workflow_id: "wf-nightly-sync",
    workflow_name: "Nightly sync",
    environment: "production",
    runs_considered: 20,
    max_downstream_count: 2,
    avg_downstream_count: 1,
    max_depth: 1,
  },
  {
    workflow_id: "wf-standalone-check",
    workflow_name: "Standalone check",
    environment: "production",
    runs_considered: 6,
    max_downstream_count: 0,
    avg_downstream_count: 0,
    max_depth: 0,
  },
];

/** Per-workflow failure recurrence (worst first) for the Needs Attention table:
 * 10 failures across three workflows. */
export const sampleDashboardFailureRecurrence: DashboardWorkflowFailureCount[] =
  [
    {
      workflow_id: "wf-etl-rollup",
      workflow_name: "ETL rollup",
      environment: "production",
      failure_count: 6,
      total_runs: 30,
    },
    {
      workflow_id: "wf-ml-scoring",
      workflow_name: "ML scoring",
      environment: "sandbox",
      failure_count: 3,
      total_runs: 18,
    },
    {
      workflow_id: "wf-nightly-sync",
      workflow_name: "Nightly sync",
      environment: "production",
      failure_count: 1,
      total_runs: 42,
    },
  ];

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
    environment: "production",
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
