import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
  type IpcCommand,
} from "./ipc-registry";
import { defaultMcpIntegrationStatus } from "./data";

describe("ipc fixture registry", () => {
  afterEach(() => {
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("covers every registered backend command", () => {
    const commands: IpcCommand[] = [
      "get_app_config",
      "list_workflows",
      "get_workflow",
      "create_workflow",
      "update_workflow",
      "delete_workflow",
      "list_environments",
      "create_environment",
      "update_environment",
      "delete_environment",
      "set_workflow_spec",
      "create_api_key",
      "list_api_keys",
      "revoke_api_key",
      "check_for_update",
      "apply_update",
      "get_app_update_status",
      "set_updater_preferences",
      "get_mcp_integration_status",
      "provision_mcp_integration",
      "remove_mcp_integration",
      "trigger_workflow",
      "enqueue_workflow",
      "rerun_workflow",
      "plan_backfill",
      "dispatch_backfill",
      "list_dead_letters",
      "get_dead_letter",
      "acknowledge_dead_letter",
      "recover_dead_letter",
      "get_run_history",
      "get_run_log",
      "get_run_tasks",
      "get_run_attempts",
      "get_run_metrics",
      "get_run_relationships",
      "get_global_run_history",
      "cleanup_retention",
      "get_workflow_history_buckets",
      "get_sla_violations",
      "query_resource_samples",
      "query_token_usage_rollup",
      "query_stale_assets",
      "get_mission_control_preferences",
      "set_mission_control_preferences",
      "get_mission_control_snapshot",
      "get_dashboard_kpi_summary",
      "get_dashboard_kpi_wow",
      "get_dashboard_status_distribution",
      "get_dashboard_success_fail_trend",
      "get_dashboard_wait_runtime_trend",
      "get_dashboard_queue_health",
      "get_dashboard_queue_utilization_history",
      "get_dashboard_workflow_baselines",
      "get_dashboard_execution_slots",
      "get_dashboard_block_taxonomy",
      "get_dashboard_blast_radius",
      "get_dashboard_failure_recurrence",
      "get_scheduler_status",
      "list_queues",
      "update_queue",
      "list_queued_runs",
      "cancel_queued_run",
      "list_available_scripts",
      "open_dashboard",
      "open_run_detail",
      "hide_popup",
      "open_url",
      "quit_app",
      "get_launch_at_login",
      "set_launch_at_login",
      "set_notification_prefs",
      "get_notification_prefs",
      "analyze_run_error",
      "generate_workflow_description",
      "get_email_config",
      "set_email_config",
      "test_email_config",
      "list_email_profiles",
      "save_email_profile",
      "delete_email_profile",
      "test_email_profile",
      "set_workflow_email_profile",
    ];

    const registry = createDefaultIpcRegistry();
    for (const cmd of commands) {
      expect(registry[cmd], `missing handler for ${cmd}`).toBeTypeOf(
        "function",
      );
    }
    expect(Object.keys(registry)).toHaveLength(commands.length);
  });

  it("throws on unhandled commands", () => {
    const registry = createDefaultIpcRegistry();
    expect(() => resolveIpcInvoke("not_a_command", {}, registry)).toThrow(
      /Unhandled IPC invoke/,
    );
  });

  // Regression test for the "IPC test-fixture reference-mutation" finding:
  // get_mcp_integration_status previously returned the raw shared/constant
  // defaultMcpIntegrationStatus object directly rather than a fresh copy,
  // unlike the already-fixed set_updater_preferences pattern — so mutating
  // one call's result (directly, or via a caller's React state setter)
  // could silently leak into every other test or consumer that reads the
  // same constant.
  it("get_mcp_integration_status returns a fresh copy, not the shared constant", () => {
    const registry = createDefaultIpcRegistry();

    const first = registry.get_mcp_integration_status({});
    expect(first).toEqual(defaultMcpIntegrationStatus);
    expect(first).not.toBe(defaultMcpIntegrationStatus);

    // Mutating the first call's result must not affect a later call or the
    // module-level constant itself.
    (first as { enabled: boolean }).enabled = true;

    const second = registry.get_mcp_integration_status({});
    expect(second.enabled).toBe(false);
    expect(defaultMcpIntegrationStatus.enabled).toBe(false);
  });

  it("returns a fresh object (not the shared reference) from every update-status handler", () => {
    // Real Tauri IPC always deserializes a new object per call. Handlers
    // returning the raw shared snapshot reference would silently break any
    // caller relying on referential inequality to detect a change.
    const registry = createDefaultIpcRegistry();
    const updateHandlers = [
      "apply_update",
      "get_app_update_status",
      "set_updater_preferences",
    ] as const;

    for (const cmd of updateHandlers) {
      const first = registry[cmd]({});
      const second = registry[cmd]({});
      expect(
        first,
        `${cmd} should not return the same reference twice`,
      ).not.toBe(second);
      expect(second).toEqual(first);
    }
  });

  it("honors per-test overrides", () => {
    const registry = createDefaultIpcRegistry();
    window.__CHAOS_IPC_OVERRIDES__ = {
      list_workflows: () => [],
    };
    mockIPC(
      (cmd, args) =>
        resolveIpcInvoke(
          cmd,
          (args ?? {}) as Record<string, unknown>,
          registry,
        ),
      { shouldMockEvents: true },
    );

    expect(registry.list_workflows({})).toHaveLength(1);
    expect(resolveIpcInvoke("list_workflows", {}, registry)).toEqual([]);
  });
});
