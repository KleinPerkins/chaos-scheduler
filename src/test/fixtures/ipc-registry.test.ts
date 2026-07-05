import { afterEach, describe, expect, it } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
  type IpcCommand,
} from "./ipc-registry";

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
      "trigger_workflow",
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
