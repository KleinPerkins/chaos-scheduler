import type { MissionControlSnapshot, Run, Workflow } from "../../lib/commands";
import {
  defaultEmailConfig,
  defaultMissionControlPreferences,
  defaultQueues,
  emptyApiKeys,
  emptyMissionControlSnapshot,
  emptySchedulerStatus,
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
  | "trigger_workflow"
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
  | "test_email_config";

export type IpcArgs = Record<string, unknown>;

export type IpcHandler = (args: IpcArgs) => unknown;

export type IpcFixtureRegistry = Record<IpcCommand, IpcHandler>;

declare global {
  interface Window {
    /** Per-test overrides merged on top of {@link createDefaultIpcRegistry}. */
    __CHAOS_IPC_OVERRIDES__?: Partial<Record<IpcCommand, IpcHandler>>;
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
    ...emptyMissionControlSnapshot,
  };

  return {
    get_app_config: () => ({
      workspace_root: "/tmp/chaos-scheduler",
      python_path: "python3",
    }),
    list_workflows: () => [sampleWorkflow],
    get_workflow: (args) => workflowById(String(args.id)),
    create_workflow: () => sampleWorkflow,
    update_workflow: (args) => ({
      ...sampleWorkflow,
      id: String(args.id ?? sampleWorkflow.id),
      enabled: Boolean(args.enabled ?? true),
    }),
    delete_workflow: () => undefined,
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
    apply_update: () => undefined,
    trigger_workflow: () => sampleRun.id,
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
    get_global_run_history: () => [sampleRun],
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
      corpus_filter: String(args.corpusFilter ?? "all"),
      domain_filter: String(args.domainFilter ?? "all"),
    }),
    get_mission_control_snapshot: () => snapshot,
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
