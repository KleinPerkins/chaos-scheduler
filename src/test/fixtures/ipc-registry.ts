import type {
  MissionControlSnapshot,
  Run,
  UpdateSnapshot,
  Workflow,
} from "../../lib/commands";
import {
  dashboardMissionControlSnapshot,
  defaultEmailConfig,
  defaultMcpIntegrationStatus,
  defaultMissionControlPreferences,
  defaultQueues,
  emptyApiKeys,
  emptySchedulerStatus,
  idleUpdateSnapshot,
  sampleDashboardBlastRadius,
  sampleDashboardBlockTaxonomy,
  sampleDashboardExecutionSlots,
  sampleDashboardFailureRecurrence,
  sampleDashboardKpiSummary,
  sampleDashboardKpiWow,
  sampleDashboardQueueHealth,
  sampleDashboardQueueUtilizationHistory,
  sampleDashboardStatusDistribution,
  sampleDashboardSuccessFailTrend,
  sampleDashboardWaitRuntimeTrend,
  sampleDashboardWorkflowBaselines,
  sampleEnvironments,
  sampleRun,
  sampleWorkflow,
} from "./data";

/** Every Tauri command registered in `src-tauri/src/lib.rs`. */
export type IpcCommand =
  | "get_app_config"
  | "list_workflows"
  | "get_workflow"
  | "create_workflow"
  | "update_workflow"
  | "delete_workflow"
  | "list_environments"
  | "create_environment"
  | "update_environment"
  | "delete_environment"
  | "set_workflow_spec"
  | "create_api_key"
  | "list_api_keys"
  | "revoke_api_key"
  | "check_for_update"
  | "apply_update"
  | "get_app_update_status"
  | "set_updater_preferences"
  | "get_mcp_integration_status"
  | "provision_mcp_integration"
  | "remove_mcp_integration"
  | "trigger_workflow"
  | "enqueue_workflow"
  | "rerun_workflow"
  | "plan_backfill"
  | "dispatch_backfill"
  | "list_dead_letters"
  | "get_dead_letter"
  | "acknowledge_dead_letter"
  | "recover_dead_letter"
  | "get_run_history"
  | "get_run_log"
  | "get_run_tasks"
  | "get_run_attempts"
  | "get_run_metrics"
  | "get_run_relationships"
  | "get_global_run_history"
  | "cleanup_retention"
  | "get_workflow_history_buckets"
  | "get_sla_violations"
  | "query_resource_samples"
  | "query_token_usage_rollup"
  | "query_stale_assets"
  | "get_mission_control_preferences"
  | "set_mission_control_preferences"
  | "get_mission_control_snapshot"
  | "get_dashboard_kpi_summary"
  | "get_dashboard_kpi_wow"
  | "get_dashboard_status_distribution"
  | "get_dashboard_success_fail_trend"
  | "get_dashboard_wait_runtime_trend"
  | "get_dashboard_queue_health"
  | "get_dashboard_queue_utilization_history"
  | "get_dashboard_workflow_baselines"
  | "get_dashboard_execution_slots"
  | "get_dashboard_block_taxonomy"
  | "get_dashboard_blast_radius"
  | "get_dashboard_failure_recurrence"
  | "get_scheduler_status"
  | "list_queues"
  | "update_queue"
  | "list_queued_runs"
  | "cancel_queued_run"
  | "list_available_scripts"
  | "open_dashboard"
  | "open_run_detail"
  | "hide_popup"
  | "open_url"
  | "quit_app"
  | "get_launch_at_login"
  | "set_launch_at_login"
  | "set_notification_prefs"
  | "get_notification_prefs"
  | "analyze_run_error"
  | "generate_workflow_description"
  | "get_email_config"
  | "set_email_config"
  | "test_email_config"
  | "list_email_profiles"
  | "save_email_profile"
  | "delete_email_profile"
  | "test_email_profile"
  | "set_workflow_email_profile";

export type IpcArgs = Record<string, unknown>;

export type IpcHandler = (args: IpcArgs) => unknown;

export type IpcFixtureRegistry = Record<IpcCommand, IpcHandler>;

declare global {
  interface Window {
    /** Per-test overrides merged on top of {@link createDefaultIpcRegistry}. */
    __CHAOS_IPC_OVERRIDES__?: Partial<Record<IpcCommand, IpcHandler>>;
    /** Mutable fixture flags for Playwright flows (e.g. simulated delete). */
    __CHAOS_FIXTURE_FLAGS__?: {
      workflowDeleted?: boolean;
    };
  }
}

function workflowById(id: string): Workflow {
  if (id === sampleWorkflow.id) return sampleWorkflow;
  throw new Error(`fixture: unknown workflow id ${id}`);
}

function runById(runId: string): Run {
  if (runId === sampleRun.id) return sampleRun;
  return { ...sampleRun, id: runId, status: "failed", exit_code: 1 };
}

/** Typed default fixture factory used by Playwright and Vitest IPC mocks. */
export function createDefaultIpcRegistry(): IpcFixtureRegistry {
  const snapshot: MissionControlSnapshot = {
    ...dashboardMissionControlSnapshot,
  };
  const updateSnapshot: UpdateSnapshot = { ...idleUpdateSnapshot };

  return {
    get_app_config: () => ({
      workspace_root: "/tmp/chaos-scheduler",
      python_path: "python3",
    }),
    list_workflows: () =>
      window.__CHAOS_FIXTURE_FLAGS__?.workflowDeleted ? [] : [sampleWorkflow],
    get_workflow: (args) => workflowById(String(args.id)),
    create_workflow: () => sampleWorkflow,
    update_workflow: (args) => ({
      ...sampleWorkflow,
      id: String(args.id ?? sampleWorkflow.id),
      enabled: Boolean(args.enabled ?? true),
    }),
    delete_workflow: () => {
      window.__CHAOS_FIXTURE_FLAGS__ = {
        ...window.__CHAOS_FIXTURE_FLAGS__,
        workflowDeleted: true,
      };
    },
    list_environments: () => sampleEnvironments,
    create_environment: (args) => ({
      id: "env-new",
      name: String(args.name ?? "new"),
      created_at: sampleRun.started_at,
      updated_at: sampleRun.started_at,
    }),
    update_environment: (args) => ({
      id: String(args.id),
      name: String(args.name ?? "updated"),
    }),
    delete_environment: () => undefined,
    set_workflow_spec: (args) => workflowById(String(args.id)),
    create_api_key: () => ({
      id: "key-1",
      token: "cs_test_token",
      scopes: "read",
    }),
    list_api_keys: () => emptyApiKeys,
    revoke_api_key: () => undefined,
    check_for_update: () => ({
      available: false,
      current_version: "0.1.0",
    }),
    // Fresh objects, not the mutated reference: real IPC round-trips always
    // deserialize a new object, and callers may rely on referential
    // inequality to detect a change (see set_updater_preferences below).
    apply_update: () => ({ ...updateSnapshot }),
    get_app_update_status: () => ({ ...updateSnapshot }),
    set_updater_preferences: (args) => {
      if (typeof args.backgroundCheckEnabled === "boolean") {
        updateSnapshot.background_check_enabled = args.backgroundCheckEnabled;
      }
      if (args.clearSkip === true) {
        updateSnapshot.skipped_version = null;
      } else if (typeof args.skippedVersion === "string") {
        updateSnapshot.skipped_version = args.skippedVersion;
      }
      return { ...updateSnapshot };
    },
    // A fresh object, not the shared module-level constant: real IPC
    // round-trips always deserialize a new object, and mutating the
    // returned value (directly, or via a caller's React state setter)
    // must never leak back into defaultMcpIntegrationStatus for later tests
    // or other callers. Mirrors the already-fixed set_updater_preferences
    // pattern above.
    get_mcp_integration_status: () => ({ ...defaultMcpIntegrationStatus }),
    provision_mcp_integration: () => ({
      ...defaultMcpIntegrationStatus,
      enabled: true,
      install_status: "installed",
      provisioned_version: defaultMcpIntegrationStatus.pinned_version,
      registered_in_cursor: true,
      cursor_config_conflict: false,
      api_reachable: true,
      managed_key_id: "mcp-key-1",
      matches: true,
      last_error: null,
    }),
    remove_mcp_integration: () => ({
      ...defaultMcpIntegrationStatus,
      enabled: false,
      install_status: "not_installed",
      provisioned_version: null,
      registered_in_cursor: false,
      managed_key_id: null,
      matches: false,
    }),
    trigger_workflow: () => sampleRun.id,
    enqueue_workflow: () => ({
      workflow_id: sampleWorkflow.id,
      status: "queued",
      queued_run_id: "queue-fixture-1",
      queue_name: "default",
    }),
    rerun_workflow: () => "run-rerun-1",
    plan_backfill: (args) => ({
      workflow_id: String(args.workflowId),
      trigger_kind: "backfill",
      chain_suppressed: false,
      logical_dates: [],
      count: 0,
      dry_run: true,
    }),
    dispatch_backfill: (args) => ({
      plan: {
        workflow_id: String(args.workflowId),
        trigger_kind: "backfill",
        chain_suppressed: false,
        logical_dates: [],
        count: 0,
        dry_run: Boolean(args.dryRun),
      },
      outcomes: [],
    }),
    list_dead_letters: () => [],
    get_dead_letter: (args) => ({
      id: String(args.id),
      run_id: sampleRun.id,
      workflow_id: sampleWorkflow.id,
      last_failure_at: sampleRun.started_at,
      last_exception: "demo",
      created_at: sampleRun.started_at,
      updated_at: sampleRun.started_at,
    }),
    acknowledge_dead_letter: (args) => ({
      id: String(args.id),
      run_id: sampleRun.id,
      workflow_id: sampleWorkflow.id,
      last_failure_at: sampleRun.started_at,
      last_exception: "demo",
      acknowledged_at: sampleRun.started_at,
      created_at: sampleRun.started_at,
      updated_at: sampleRun.started_at,
    }),
    recover_dead_letter: () => ({
      workflow_id: sampleWorkflow.id,
      status: "admitted",
      run_id: sampleRun.id,
      queue_name: "default",
    }),
    get_run_history: () => [sampleRun],
    get_run_log: (args) => runById(String(args.runId)),
    get_run_tasks: () => [],
    get_run_attempts: () => [],
    get_run_metrics: () => [],
    get_run_relationships: () => [],
    get_global_run_history: (args) => {
      const statusFilter = String(args.statusFilter ?? "all");
      const pollExhaustedRun: Run = {
        ...sampleRun,
        id: "run-poll-exhausted",
        status: "poll_exhausted",
        exit_code: 1,
      };
      const runs = [sampleRun, pollExhaustedRun];
      if (statusFilter === "all") return runs;
      return runs.filter((run) => run.status === statusFilter);
    },
    cleanup_retention: () => ({
      cutoff: sampleRun.started_at,
      candidate_runs: 0,
      preserved_dead_letter_runs: 0,
      dry_run: true,
      deleted_runs: 0,
    }),
    get_workflow_history_buckets: () => [],
    get_sla_violations: () => [],
    query_resource_samples: () => [],
    query_token_usage_rollup: () => [],
    query_stale_assets: () => [],
    get_mission_control_preferences: () => defaultMissionControlPreferences,
    set_mission_control_preferences: (args) => ({
      default_landing: args.defaultLanding ?? "mission_control",
      environment_filter: String(args.environmentFilter ?? "all"),
      domain_filter: String(args.domainFilter ?? "all"),
    }),
    get_mission_control_snapshot: () => snapshot,
    // v3 Overview dashboard bindings. Fresh copies per call (real IPC always
    // deserializes new objects; callers may rely on referential inequality).
    get_dashboard_kpi_summary: () => ({ ...sampleDashboardKpiSummary }),
    get_dashboard_kpi_wow: () => ({ ...sampleDashboardKpiWow }),
    get_dashboard_status_distribution: () =>
      sampleDashboardStatusDistribution.map((row) => ({ ...row })),
    get_dashboard_success_fail_trend: () => ({
      ...sampleDashboardSuccessFailTrend,
      buckets: sampleDashboardSuccessFailTrend.buckets.map((b) => ({ ...b })),
    }),
    get_dashboard_wait_runtime_trend: () => ({
      ...sampleDashboardWaitRuntimeTrend,
      wait: sampleDashboardWaitRuntimeTrend.wait.map((b) => ({ ...b })),
      runtime: sampleDashboardWaitRuntimeTrend.runtime.map((b) => ({ ...b })),
    }),
    get_dashboard_queue_health: () => ({
      ...sampleDashboardQueueHealth,
      queues: sampleDashboardQueueHealth.queues.map((q) => ({ ...q })),
    }),
    get_dashboard_queue_utilization_history: () => ({
      ...sampleDashboardQueueUtilizationHistory,
      buckets: sampleDashboardQueueUtilizationHistory.buckets.map((b) => ({
        ...b,
      })),
    }),
    get_dashboard_workflow_baselines: () =>
      sampleDashboardWorkflowBaselines.map((b) => ({ ...b })),
    get_dashboard_execution_slots: () => ({
      ...sampleDashboardExecutionSlots,
      queues: sampleDashboardExecutionSlots.queues.map((q) => ({ ...q })),
    }),
    get_dashboard_block_taxonomy: () => ({
      ...sampleDashboardBlockTaxonomy,
      by_reason: sampleDashboardBlockTaxonomy.by_reason.map((r) => ({ ...r })),
      heavy_blockers: sampleDashboardBlockTaxonomy.heavy_blockers.map((b) => ({
        ...b,
      })),
    }),
    get_dashboard_blast_radius: () =>
      sampleDashboardBlastRadius.map((r) => ({ ...r })),
    get_dashboard_failure_recurrence: () =>
      sampleDashboardFailureRecurrence.map((r) => ({ ...r })),
    get_scheduler_status: () => emptySchedulerStatus,
    list_queues: () => defaultQueues,
    update_queue: (args) => ({
      ...defaultQueues[0],
      name: String(args.name),
      capacity: Number(args.capacity),
    }),
    list_queued_runs: () => [],
    cancel_queued_run: () => undefined,
    list_available_scripts: () => [],
    open_dashboard: () => undefined,
    open_run_detail: () => undefined,
    hide_popup: () => undefined,
    open_url: () => undefined,
    quit_app: () => undefined,
    get_launch_at_login: () => false,
    set_launch_at_login: () => "ok",
    set_notification_prefs: () => undefined,
    get_notification_prefs: () => ({
      notify_on_failure: true,
      notify_on_success: false,
    }),
    analyze_run_error: () => ({
      diagnosis: "Demo analysis",
      summary: "Harness fixture",
    }),
    generate_workflow_description: () => "Generated description",
    get_email_config: () => defaultEmailConfig,
    set_email_config: () => undefined,
    test_email_config: () => ({ success: true }),
    list_email_profiles: () => [],
    save_email_profile: (args) => ({
      id: "profile-1",
      name: "Profile",
      enabled: true,
      alert_email: "",
      smtp_host: "smtp.gmail.com",
      smtp_port: 587,
      smtp_user: "",
      smtp_password: "",
      from_address: "",
      from_name: "Chaos Scheduler",
      ...((args?.profile as Record<string, unknown>) ?? {}),
    }),
    delete_email_profile: () => undefined,
    test_email_profile: () => ({ success: true }),
    set_workflow_email_profile: () => undefined,
  };
}

/** mockIPC handler: unhandled commands fail the test by default. */
export function resolveIpcInvoke(
  cmd: string,
  args: IpcArgs,
  registry: IpcFixtureRegistry,
): unknown {
  const overrides = window.__CHAOS_IPC_OVERRIDES__;
  const override = overrides?.[cmd as IpcCommand];
  if (override) return override(args);

  const handler = registry[cmd as IpcCommand];
  if (!handler) {
    throw new Error(`Unhandled IPC invoke: ${cmd}`);
  }
  return handler(args);
}
